use std::{io::Read, sync::Arc, time::Duration, str::FromStr, mem};


use tokio::{time, sync::{broadcast,broadcast::Sender,broadcast::Receiver }};
use uuid::Uuid;
use anyhow::{Result, anyhow,bail };
use async_trait::async_trait;
use futures::stream::StreamExt;

use usb_manager::hid_device::HidDevice as UsbPeripheral;

use btleplug::{
    
    platform::{Peripheral as BlePeripheral, PeripheralId},
    api::{Peripheral as ApiPeripheral,bleuuid::uuid_from_u16,WriteType::WithResponse}

};

use crate::{
    enums::{Error,ConnectionType,ChipManufacturer,DeviceType,ChipType},
    api::PeripheralApi
};

use lazy_static::lazy_static;

/// BLE 通信的 通信服务id
const SERVICE_UUID: Uuid = uuid_from_u16(0xFF00);
/// 通信uuid
const WRITE_READ_NOTIFY_UUID: Uuid = uuid_from_u16(0xFF01);
/// OTA 重新发送
const _OTA_RETRANSMIT_UUID: Uuid = uuid_from_u16(0xFF02);
/// OTA 重启
const _OTA_RESET_UUID: Uuid = uuid_from_u16(0xFF03);

const DEVICE_INFO_SERVICE_UUID: Uuid = uuid_from_u16(0x180A);
/// PnP ID  获取PID VID
const PNP_ID_UUID: Uuid = uuid_from_u16(0x2A50);
/// Firmware Revision String
const FIRMWARE_REVISION_UUID: Uuid = uuid_from_u16(0x2a26);
/// Hardware Revision String
const HARDWARE_REVISION_UUID: Uuid = uuid_from_u16(0x2a27);
/// Hardware Revision String
const SOFTWARE_REVISION_UUID: Uuid = uuid_from_u16(0x2a28);

const BATTERY_SERVICE_UUID: Uuid = uuid_from_u16(0x180f);
/// PnP ID  获取PID VID
const BATTERY_SERVICE_ID_UUID: Uuid = uuid_from_u16(0x2a19);

use dashmap::DashMap;

/// 外围设备对象
#[derive(Debug, Clone)]
pub enum Device {
    Usb(UsbPeripheral), 
    Ble(BlePeripheral)
}


#[derive(Debug)]
pub struct PeripheralDevice{
    pub device:Device,
    pub sender:Sender<Vec<u8>>,
    // 线程句柄
    _thread_handle: Option<tokio::task::JoinHandle<()>>,
}


impl PeripheralDevice {
    async fn read<'a>(&'a self,buf: &'a mut[u8])->  Result<usize>{
        let len = buf.len();
        return match &self.device {
            Device::Usb(device) => {
                let result = device.get_input_report(0x00, len)?;
                result.as_slice().read(buf).map_err(|e| e.into())
            },
            Device::Ble(device) =>{
                // 检查服务状态
                // if !device.get_services_status() {
                //     device.discover_services().await?;
                // }
                let result = device.read_by_uuid(&SERVICE_UUID,&WRITE_READ_NOTIFY_UUID).await?;
                result.as_slice().read(buf).map_err(|e| e.into())
            }
        }
    }

    async fn write<'a>(&'a self,src: &'a[u8]) ->  Result<usize> {
        let len = src.len();
        return match &self.device {
            Device::Usb(device) => {
                device.set_output_report(0x00, src)?;
                Ok(len)
            },
            Device::Ble(device) =>{
                // 检查服务状态
                // if !device.get_services_status() {
                //     device.discover_services().await?;
                // }
                device.write_by_uuid(&SERVICE_UUID,&WRITE_READ_NOTIFY_UUID,src,WithResponse).await?;
                Ok(len)
            }
        }
    }

    async fn request<'a>(&'a self,src: &'a[u8]) -> Result<Vec<u8>>  {
        let len = src.len();
        return match &self.device {
            Device::Usb(device) => {
                device.set_output_report(0x00, src)?;
                device.get_input_report(0x00, len)
            },
            Device::Ble(device) =>{
                // 检查服务状态
                // if !device.get_services_status() {
                //     device.discover_services().await?;
                // }
                let mut rece = self.sender.subscribe();
                // 写入操作命令
                device.write_by_uuid(&SERVICE_UUID,&WRITE_READ_NOTIFY_UUID,src,WithResponse).await?;
                let result = time::timeout(Duration::from_secs(2), async move{
                    rece.recv().await
                }).await;
                result.map_err(|_| Error::TimedOut(Duration::from_secs(1)))?.map_err(|e| anyhow!(e))
            }
        }
    }
}

/// 外围设备信息
#[derive(Debug,Clone)]
pub struct Peripheral {
    shared:Arc<Shared>
}

#[derive(Debug)]
struct Shared {
    /// 
    pub id: Uuid,
    pub vid: u16,
    pub pid: u16,
    pub address:String,
    /// 厂商
    pub chip_manufacturer: ChipManufacturer,
    /// 设备类型
    pub device_type: DeviceType,
    /// 设备名称
    pub device_name: String,
    /// 芯片类型
    pub chip_type: ChipType,
    /// 软件版本号
    pub software_version: String,
    /// 硬件版本号
    pub hardware_version: String,
    /// 固件版本号
    pub firmware_version: String,
    /// 外围设备 
    pub peripheral_device: PeripheralDevice,
}

impl Peripheral {
    /// 创建USB设备
    pub fn new_usb(device: UsbPeripheral) -> Self {
        Peripheral {
            shared: Arc::new(Shared {
                id:Uuid::new_v4(),
                vid:device.vendor_id,
                pid:device.product_id,
                address:device.path.clone().into_string().unwrap_or_default(),
                chip_manufacturer: ChipManufacturer::JL,
                device_type: DeviceType::MulKeyboardTouchpad,
                device_name: "default".to_string(),
                chip_type: ChipType::AC635N,
                software_version:"0.0.0".to_string(),
                hardware_version: "0.0.0".to_string(),
                firmware_version: "0.0.0".to_string(),
                /// 外围设备 
                peripheral_device: PeripheralDevice{
                    device:Device::Usb(device),
                    sender:broadcast::channel(0).0,
                    _thread_handle: None,
                },
            })
        }
    }
    pub async fn new_ble(device: BlePeripheral) -> Result<Self> {
        if device.characteristics().iter()
        .filter(|c| c.uuid == WRITE_READ_NOTIFY_UUID || c.uuid == PNP_ID_UUID ).count() != 2{
            bail!(Error::NonSupport)
        }
        let id: PeripheralId = device.id();
        let mut slice = [0u8; 16];
        slice[..6].clone_from_slice(&id.0.into_inner());
        let uniid = Uuid::from_bytes(slice);

        let (sender,_) = broadcast::channel(5);
        let ble = device.clone();
        let send = sender.clone();

        let thread_handle = tokio::spawn(async move{
            // 订阅notify返回
            if let Err(e) = ble.subscribe_by_uuid(&SERVICE_UUID,&WRITE_READ_NOTIFY_UUID).await{
                println!("subscribe error:{}",e);
            }
            if let Ok(mut stream) = ble.notifications().await{
                // Process while the BLE connection is not broken or stopped.
                while let Some(data) = stream.next().await {
                    // println!("recv data:{:?}",data.value);
                    if let Err(e) = send.send(data.value){
                        println!("send error:{}",e);
                        break;
                    }
                }
            }
        });

        // get device info
        let pnp = device.read_by_uuid(&DEVICE_INFO_SERVICE_UUID, &PNP_ID_UUID).await?;
        let firmware_revision = device.read_by_uuid(&DEVICE_INFO_SERVICE_UUID, &FIRMWARE_REVISION_UUID).await?;
        let hardware_revision = device.read_by_uuid(&DEVICE_INFO_SERVICE_UUID, &HARDWARE_REVISION_UUID).await?;
        let software_revision = device.read_by_uuid(&DEVICE_INFO_SERVICE_UUID, &SOFTWARE_REVISION_UUID).await?;
        let vid:u16 = ((pnp[2] as u16) << 8) | pnp[1] as u16;
        let pid:u16 = ((pnp[4] as u16) << 8) | pnp[3] as u16;

        let mut name = "default".to_string();
        if let Ok(propert) = device.properties().await {
            name = propert.unwrap_or_default().local_name.unwrap_or_default();
            println!("ble name:{}",name);
        }

        Ok(Peripheral {
            shared: Arc::new(Shared {
                id:uniid,
                vid:vid,
                pid:pid,
                address:device.address().to_string(),
                chip_manufacturer: ChipManufacturer::PAR,
                device_type: DeviceType::MulKeyboardTouchpad,
                device_name: name,
                chip_type: ChipType::PAR2860,
                software_version:String::from_utf8(firmware_revision).unwrap_or_default(),
                hardware_version: String::from_utf8(hardware_revision).unwrap_or_default(),
                firmware_version: String::from_utf8(software_revision).unwrap_or_default(),
                /// 外围设备 
                peripheral_device: PeripheralDevice{
                    device:Device::Ble(device),
                    sender:sender,
                    _thread_handle:Some(thread_handle),
                },
            })
        })
    }
}

#[async_trait]
impl PeripheralApi for Peripheral {
    fn id(&self) -> Uuid {
        self.shared.id.clone()
    }

    fn address(&self) -> String {
        self.shared.address.clone()
    }

    fn conn_type(&self) -> ConnectionType{
        match &self.shared.peripheral_device.device {
            Device::Usb(_) => ConnectionType::USB,
            Device::Ble(_) => ConnectionType::BLE,
        }
    }

    fn vendor_id(&self) -> u16 {
        self.shared.vid
    }

    fn product_id(&self) -> u16 {
        self.shared.pid
    }

    fn chip_manufacturer(&self) -> ChipManufacturer {
        self.shared.chip_manufacturer.clone()
    }

    fn device_type(&self) -> DeviceType {
        self.shared.device_type.clone()
    }

    fn device_name(&self) -> String {
        self.shared.device_name.clone()
    }

    fn chip_type(&self) -> ChipType {
        self.shared.chip_type.clone()
    }

    fn software_version(&self) -> String {
        self.shared.software_version.clone()
    }

    fn hardware_version(&self) -> String {
        self.shared.hardware_version.clone()
    }

    fn firmware_version(&self) -> String {
        self.shared.firmware_version.clone()
    }
    async fn connect(&self,_u:uuid::Uuid) ->  Result<()>  {
        Ok(())
    }

    async fn read<'a>(&'a self,buf: &'a mut[u8])->  Result<usize>  {
        self.shared.peripheral_device.read(buf).await
    }

    async fn write<'a>(&'a self,src: &'a[u8]) ->  Result<usize>  {
        self.shared.peripheral_device.write(src).await
    }

    async fn request<'a>(&'a self,src: &'a[u8]) -> Result<Vec<u8>>  {
        self.shared.peripheral_device.request(src).await
    }

    async fn disconnect(&mut self) ->  Result<()>  {
        Ok(())
    }
}
