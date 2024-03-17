// 

//! ### For example
//! ```rust
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     use peripheral_manager::{
//!         core::{AppOptions,App},
//!         api::{CoreEvent,PeripheralApi},
//!     };
//! 
//!     let options = AppOptions::new().set_broadcast(true, 10).
//!     set_usb_filter(Box::new(|x| x.vendor_id == 0x3373 && && x.input_report_byte_length == 65));
//! 
//!     let app = App::start(Some(options)).await.unwrap();
//! 
//!     let all_devices = app.peripherals().await.unwrap();
//!     println!("all device len:{}",all_devices.len());
//! 
//!     let mut channl = app.register_broadcast().unwrap();
//!     
//!     loop {
//!         match channl.recv().await {
//!             Ok(v) => {
//!                 match v {
//!                     CoreEvent::DeviceAdd(id) => {
//!                         println!("add device:{}",id);
//! 
//!                         let device = app.peripheral(&id).await.unwrap();
//!                         
//!                         println!("device path :{:?}",device.address());
//!                         let mut buffer = [0; 51];
//!                         println!("read len:{:?}",device.read(&mut buffer).await?);
//!                         println!("read data {:?}",buffer);
//! 
//!                         let write_buf = [0;2];
//!                         device.write(&write_buf).await?;
//!                         println!("over");
//!                     },
//!                     CoreEvent::DeviceRemove(deivce) => {
//!                         println!("Remove:{}",deivce.id());
//!                     },
//!                 }
//!             },
//!             Err(e) => println!("error: {:?}",e),
//!         }
//!     }
//! }
//! ```



pub mod peripheral;
pub mod api;
pub mod core;
pub mod enums;


#[cfg(test)]
mod tests {
    use crate::core::App;

    use super::*;


    macro_rules! aw {
        ($e:expr) => {
            tokio_test::block_on($e)
        };
      }


    #[test]
    fn it_works() {

        
        let app = aw!(App::start(None)).unwrap();

        let mut channl = app.register_broadcast().unwrap();

        loop {
            match aw!(channl.recv()) {
                Ok(v) => {
                    match v {
                        api::CoreEvent::DeviceAdd(id) => {
                            println!("Add:{:?}",id);
                        },
                        api::CoreEvent::DeviceRemove(id) => {
                            println!("Remove:{}",id);
                        },
                    }
                },
                Err(e) => println!("error: {:?}",e),
            }
        }
    }
}
