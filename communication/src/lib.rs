#![allow(dead_code)]

use serde::{Deserialize, Serialize, de::DeserializeOwned};

pub mod client;
mod error;
pub use error::Error;

mod device;
pub use device::HardwareSource;

#[macro_export]
macro_rules! response {
    (Request::GetHardwareSourceList) => {
        HardwareSourceList
    };
}

/// Requests that a client can execute.
/// Every Request has a response counterpart.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Request {
    GetHardwareSourceList
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub struct HardwareSourceList {
    list: Vec<HardwareSource>
}
