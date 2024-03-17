
use async_trait::async_trait;
use uuid::Uuid;
use anyhow::Result;
use std::fmt::Debug;
use crate::{
    enums::{ChipType,ConnectionType, ChipManufacturer, DeviceType}
};


#[async_trait]
pub trait PeripheralApi: Send + Sync + Clone + Debug {
    /// 返回设备里记录的id uuid,None 为未经过认证的
    fn id(&self) -> Uuid;
    /// 返回设备的地址
    fn address(&self) -> String;
    /// 返回设备的连接类型
    fn conn_type(&self) -> ConnectionType;
    /// 返回设备的厂商id
    fn vendor_id(&self) -> u16;
    /// 返回设备的产品id
    fn product_id(&self) -> u16;
    fn chip_manufacturer(&self) -> ChipManufacturer;
    fn device_type(&self) -> DeviceType;
    fn device_name(&self) -> String;
    fn chip_type(&self) -> ChipType;
    fn software_version(&self) -> String;
    fn hardware_version(&self) -> String;
    fn firmware_version(&self) -> String;
    /// 根据设备的uuid连接设备
    async fn connect(&self,u: Uuid) -> Result<()>;
    /// 读取设备的数据
    async fn read<'a>(&'a self, buf: &'a mut [u8]) -> Result<usize>;
    /// 写入设备的数据
    async fn write<'a>(&'a self, src: &'a [u8]) -> Result<usize>;
    /// 发起一次请求，直接返回数据
    async fn request<'a>(&'a self, src: &'a [u8]) -> Result<Vec<u8>>;
    /// 断开设备的连接
    async fn disconnect(&mut self) -> Result<()>;
}

