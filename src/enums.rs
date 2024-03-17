use thiserror::Error;
use std::time::Duration;
use strum_macros::{EnumString, Display, FromRepr};
use uuid::Uuid;

use crate:: peripheral::Peripheral;

/// The main error type returned by most methods in btleplug.
#[derive(Error, Debug)]
pub enum Error {
    #[error("This device is not supported")]
    NonSupport,

    #[error("Permission denied")]
    PermissionDenied,

    #[error("Device not found")]
    DeviceNotFound,

    #[error("Not connected")]
    NotConnected,

    #[error("Timed out after {:?}", _0)]
    TimedOut(Duration),

    #[error("{}", _0)]
    Other(Box<dyn std::error::Error + Send + Sync>),
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, EnumString, Display, FromRepr)]
#[repr(u8)]
pub enum ConnectionType{
    /// USB 类型连接
    USB,
    /// BLE 类型连接
    BLE
}
impl ConnectionType {
    fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::USB,
            1 => Self::BLE,
            _ => Self::USB,
        }
    }

    pub fn num(&self) -> u8 {
        match *self {
            ConnectionType::USB => 0 ,
            ConnectionType::BLE => 1 ,
        }
    }

}

impl From<u8> for ConnectionType {
    fn from(item: u8) -> Self {
        Self::from_u8(item)
    }
}

/// 具体设备类型
#[derive(Debug, Clone, Eq, PartialEq, EnumString, Display, FromRepr)]
#[repr(u16)]
pub enum DeviceType {
    #[strum(serialize = "Keyboard")]
    Keyboard = 1,
    #[strum(serialize = "Mouse")]
    Mouse = 2,
    #[strum(serialize = "Touchpad")]
    Touchpad = 3,
    /// 复合 键盘触摸板
    #[strum(serialize = "MulKeyboardTouchpad")]
    MulKeyboardTouchpad = 4,
    #[strum(serialize = "Other")]
    Other = 100,
}

pub fn device_type_from_repr(d: u16) -> DeviceType {
    DeviceType::from_repr(d).unwrap_or_default()
}

impl Default for DeviceType {
    fn default() -> Self {
        DeviceType::Other
    }
}

/// 具体芯片型号，TODO: 不同的芯片厂商，采用不同的分类
#[derive(Debug, Clone, Copy, Eq, PartialEq, EnumString, Display )]
pub enum ChipType {
    #[strum(serialize = "AC632N")]
    AC632N,
    #[strum(serialize = "AC635N")]
    AC635N,
    #[strum(serialize = "PAR2860")]
    PAR2860,
}

impl Default for ChipType {
    fn default() -> Self {
        Self::PAR2860
    }
}

/// 芯片制造厂商
#[derive(Debug, Clone, Copy, Eq, PartialEq, EnumString, Display, FromRepr)]
#[repr(u16)]
pub enum ChipManufacturer {
    /// 杰理
    #[strum(serialize = "JL")]
    JL,
    /// 原相
    #[strum(serialize = "PAR")]
    PAR,
}

impl Default for ChipManufacturer {
    fn default() -> Self {
        Self::PAR
    }
}

impl ChipManufacturer {
    fn from_u8(v: u16) -> Self {
        match v {
            0 => Self::JL,
            1 => Self::PAR,
            _ => Self::PAR,
        }
    }

    pub fn num(&self) -> u16 {
        match *self {
            Self::JL => 0 ,
            Self::PAR => 1 ,
        }
    }

}

#[derive(Debug, Clone)]
pub enum CoreEvent
{
    /// 设备连接
    DeviceAdd(Peripheral),
    /// 设备断开
    DeviceRemove(Uuid),
}