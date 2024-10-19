use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SonusmixSettings {
    pub lock_endpoint_connections: bool,
    pub lock_group_node_connections: bool,
    pub show_group_node_change_warning: bool,
    pub volume_limit: f64,
}

pub const DEFAULT_SETTINGS: SonusmixSettings = SonusmixSettings {
    lock_endpoint_connections: false,
    lock_group_node_connections: true,
    show_group_node_change_warning: true,
    volume_limit: 100.0,
};

impl Default for SonusmixSettings {
    fn default() -> Self {
        DEFAULT_SETTINGS
    }
}
