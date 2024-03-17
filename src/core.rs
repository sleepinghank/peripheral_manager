use anyhow::{Result, bail, anyhow};
use dashmap::DashMap;
use std::{sync::Arc,pin::Pin, str::FromStr, f32::consts::E};
use uuid::Uuid;
use tokio::sync::{broadcast,broadcast::Receiver,broadcast::Sender, Mutex };
use tokio_stream::Stream;
use futures::stream::StreamExt;

use usb_manager::{
    hid_device::HidDevice,
    adapter::Adapter as UsbAdapter,
    CentralEvent,
};
use btleplug::{
    winrtble::{
        peripheral::Peripheral as BlePeripheral,
        adapter::Adapter as BleAdapter,
    }, 
    api::{
        Central,
        CentralEvent as BleCentralEvent, 
    }
};

use super::{
    api::PeripheralApi,
    peripheral::Peripheral,
    enums::CoreEvent,
};


pub type UsbFilterHandler = Box<dyn Fn(&HidDevice) -> bool + Send  + Sync>;
pub type BleFilterHandler = Box<dyn Fn(&BlePeripheral) -> bool + Send  + Sync>;

/// 初始化配置参数

pub struct AppOptions
{
    is_broadcast: bool,
    broadcast_buf_len: usize,
    ble_filter: Option<BleFilterHandler>,
    usb_filter:Option<UsbFilterHandler>,
}

impl Default for AppOptions{
    fn default() -> Self {
        AppOptions {
            is_broadcast: true,
            broadcast_buf_len: 108,
            ble_filter:None,
            usb_filter:None,
        }
    }
}

impl AppOptions{
    fn usb_filter(&self, hid_device: &HidDevice) -> bool {
        if let Some(filter_handler) = &self.usb_filter {
            return filter_handler(hid_device);
        }
        true
    }
    fn ble_filter(&self, ble_device: &BlePeripheral) -> bool {
        if let Some(filter_handler) = &self.ble_filter {
            return filter_handler(ble_device);
        }
        true
    }
    pub fn set_broadcast(mut self,is_broadcast: bool, broadcast_buf_len: usize) ->Self{
        self.is_broadcast = is_broadcast;
        self.broadcast_buf_len = broadcast_buf_len;
        self
    }

    pub fn set_usb_filter(mut self,filter_handler:UsbFilterHandler) -> Self{
        self.usb_filter = Some(filter_handler);
        self
    }

    pub fn set_ble_filter(mut self,filter_handler:BleFilterHandler) -> Self{
        self.ble_filter = Some(filter_handler);
        self
    }
}

impl AppOptions {
    pub fn new() -> Self{
        Self { 
            is_broadcast: false, 
            broadcast_buf_len: 0, 
            ble_filter: None,
            usb_filter:None 
        }
    }
}

/// 设备
pub struct App {
    /// 设备过滤
    options: Arc<AppOptions>,
    /// 设备消息广播
    announcer : Option<Sender<CoreEvent>>,
    /// USB 适配器
    usb_adapter: Arc<Mutex<Option<UsbAdapter>>>,
    /// 蓝牙 适配器
    ble_adapter:Arc<Mutex<Option<BleAdapter>>>,
    /// 线程句柄
    _thread_handle: Option<tokio::task::JoinHandle<()>>,
    /// 设备集合
    peripherals: DashMap<Uuid, Peripheral>
}


impl App {
    /// 根据配置启动USB 监听。
    pub async fn start(options: Option<AppOptions>) -> Result<Self> {
        let mut app = App::init(options)?;
        app.run().await?;
        Ok(app)
    }

    /// 内部初始化 
    fn init(options: Option<AppOptions>) -> Result<Self> {
        let options = match options {
            Some(o) => o,
            None => AppOptions::default()
        };
        let mut announcer = None;
        if options.is_broadcast {
            let (broadcast_sender, _) = broadcast::channel(options.broadcast_buf_len.clone());
            announcer = Some(broadcast_sender);
        }
        let app = Self { 
            options: Arc::new(options), 
            announcer,
            usb_adapter:Arc::new(Mutex::new(None)),
            ble_adapter:Arc::new(Mutex::new(None)),
            _thread_handle:None, 
            peripherals: DashMap::new(),
        };
        Ok(app)
    }
    
    /// 获取所有的外围设备（已过滤） 
    pub async fn peripherals(&self) -> Result<Vec<Peripheral>>{
        let adapter = self.usb_adapter.lock().await;
        adapter.as_ref().ok_or(anyhow!("Usb Adapter is null"))?.
        peripherals().map(|x| {
            x.into_iter().filter(|a| self.options.usb_filter(a)).
            map(|a| Peripheral::new_usb(a)).collect()
        })
    }

    /// 根据id 获取外围设备 
    pub async fn peripheral(&self,id: &Uuid) -> Result<Peripheral>{
        let adapter = self.usb_adapter.lock().await;
        adapter.as_ref().ok_or(anyhow!("Usb Adapter is null"))?.
        peripheral(id).map(|x| Peripheral::new_usb(x))
    }


    /// 启动 
    async fn run(&mut self) -> Result<()> {
        {
            let adapter = UsbAdapter::new();
            adapter.start()?;
            let mut mut_adapter = self.usb_adapter.lock().await;
            *mut_adapter = Some(adapter);

            let ble_adapter = BleAdapter::new();
            // 设置蓝牙连接的事件监听器
            ble_adapter.start_conn_watcher().await?;
            let mut mut_ble_adapter = self.ble_adapter.lock().await;
            *mut_ble_adapter = Some(ble_adapter);
        }
        {
            let options =Arc::clone(&self.options);
            let sender = self.announcer.clone().ok_or(anyhow!("Usb Adapter is null"))?;

            let adapter_clone = Arc::clone(&self.usb_adapter);
            tokio::spawn(async {
                if let Err(err) = usb_event(adapter_clone,options,sender).await {
                    println!("{:?}", err);
                }
            });
        }
        {
            let options =Arc::clone(&self.options);
            let sender = self.announcer.clone().ok_or(anyhow!("Usb Adapter is null"))?;
            let ble_adapter_clone = Arc::clone(&self.ble_adapter);
            tokio::spawn(async {
                if let Err(err) = ble_event(ble_adapter_clone,options,sender).await {
                    println!("{:?}", err);
                }
            });
        }

        Ok(())
    }

    /// 是否支持广播，由 start 参数options 决定
    pub fn is_support_broadcast(&self) -> bool {
        self.options.is_broadcast
    }

    /// 获取监听设备变动广播
    pub fn register_broadcast(&self) -> Result<Receiver<CoreEvent>>{
        match &self.announcer {
            Some(tx) => {
                return Ok(tx.subscribe())
            },
            None => {
                bail!("无广播器，请设置启动参数 is_broadcast = true");
            }
        }
    }
}

async fn usb_event(adapter:Arc<Mutex<Option<UsbAdapter>>>,options: Arc<AppOptions>,sender:Sender<CoreEvent>) -> Result<()>{
    let read: crossbeam_channel::Receiver<CentralEvent>;
    {
        let adapter = adapter.lock().await;
        read = adapter.as_ref().ok_or(anyhow!("Usb Adapter is null"))?.events()?;
    }
    loop {
        match read.recv() {
            Ok(v) => {
                match v {
                    CentralEvent::DeviceAdd(id) => {
                        let adapter = adapter.lock().await;
                        let device = adapter.as_ref().ok_or(anyhow!("Usb Adapter is null"))?.peripheral(&id)?;
                        if options.usb_filter(&device){
                            sender.send(CoreEvent::DeviceAdd(Peripheral::new_usb(device)))?;
                        }
                    },
                    CentralEvent::DeviceRemove(device) => {
                        if options.usb_filter(&device){
                            sender.send(CoreEvent::DeviceRemove(Peripheral::new_usb(device).id()))?;
                        }
                    },
                }
            },
            Err(err) => println!("Err:{:?}",err),
        }
    }
}

async fn ble_event(adapter:Arc<Mutex<Option<BleAdapter>>>,options: Arc<AppOptions>,sender:Sender<CoreEvent>) -> Result<()>{
    let mut events: Pin<Box<dyn Stream<Item=BleCentralEvent> + Send>>;
    {
        let adapter = adapter.lock().await;

        events = adapter.as_ref().ok_or(anyhow!("Usb Adapter is null"))?.events().await?;
    }
    while let Some(event) = events.next().await {
        match event {
            BleCentralEvent::DeviceConnected(id) => {
                let adapter = adapter.lock().await;
                let device = adapter.as_ref().ok_or(anyhow!("Usb Adapter is null"))?.peripheral(&id).await?;
                if options.ble_filter(&device){
                    match Peripheral::new_ble(device).await {
                        Ok(ble) => {
                            
                            sender.send(CoreEvent::DeviceAdd(ble))?;
                        },
                        Err(e) => { println!("{:?}",e);},
                    };
                }
            }
            BleCentralEvent::DeviceDisconnected(id) => {
                let mut slice = [0u8; 16];
                slice[..6].clone_from_slice(&id.0.into_inner());
                sender.send(CoreEvent::DeviceRemove(Uuid::from_bytes(slice)))?;
            }
            // BleCentralEvent::DeviceUpdated(id) => {
            //     // if let Err(e) = set_status(&id, true).await {
            //     //     error!("BLE DeviceUpdated: {:?}", e);
            //     // } else {
            //     //     info!("BLE DeviceUpdated: {:?}", id);
            //     // }
            // }
            _ => {}
        }
    }
    Ok(())
}