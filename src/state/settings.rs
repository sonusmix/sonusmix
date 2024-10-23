use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SonusmixSettings {
    pub collapse_to_tray_on_close: bool,
    pub start_collapsed_to_tray: bool,
    pub lock_endpoint_connections: bool,
    pub lock_group_node_connections: bool,
    pub show_group_node_change_warning: bool,
    pub application_sources_include_monitors: bool,
    pub volume_limit: f64,
}

pub const DEFAULT_SETTINGS: SonusmixSettings = SonusmixSettings {
    collapse_to_tray_on_close: false,
    start_collapsed_to_tray: false,
    lock_endpoint_connections: false,
    lock_group_node_connections: true,
    show_group_node_change_warning: true,
    application_sources_include_monitors: false,
    volume_limit: 100.0,
};

impl Default for SonusmixSettings {
    fn default() -> Self {
        DEFAULT_SETTINGS
    }
}
