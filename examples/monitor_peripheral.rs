use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use peripheral_manager::{
        core::{AppOptions,App},
        api::{CoreEvent,PeripheralApi},
    };
    use btleplug::api::{Peripheral as _,bleuuid::uuid_from_u16};

    /// 通信uuid
    const WRITE_READ_NOTIFY_UUID: Uuid = uuid_from_u16(0xFF01);

    let options = AppOptions::new().set_broadcast(true, 10).
    set_usb_filter(Box::new(|x| x.vendor_id == 0x3373 && x.input_report_byte_length == 65)).
    set_ble_filter(Box::new(|peripheral| {
        match peripheral.characteristics().iter()
        .find(|c| c.uuid == WRITE_READ_NOTIFY_UUID) {
            Some(_) => true,
            None => false,
        }
    }));

    let app = App::start(Some(options)).await.unwrap();

    let all_devices = app.peripherals().await.unwrap();
    println!("all device len:{}",all_devices.len());

    let mut channel = app.register_broadcast().unwrap();
    
    loop {
        match channel.recv().await {
            Ok(v) => {
                match v {
                    CoreEvent::DeviceAdd(id) => {
                        println!("add device:{}",id.id());

                        let device = app.peripheral(&id.id()).await.unwrap();
                        
                        println!("device path :{:?}",device.address());
                        let mut buffer = [0; 51];
                        println!("read len:{:?}",device.read(&mut buffer).await?);
                        println!("read data {:?}",buffer);

                        let write_buf = [2;2];
                        let write_len = device.write(&write_buf).await?;
                        println!("write_len:{}",write_len);
                    },
                    CoreEvent::DeviceRemove(id) => {
                        println!("Remove:{:?}",id);
                    },
                }
            },
            Err(e) => println!("error: {:?}",e),
        }
    }
}