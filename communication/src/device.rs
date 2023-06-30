use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct HardwareSource {
    name: String,
    volume: u8
}
