use std::{cell::OnceCell, sync::OnceLock};

use log::debug;
use pipewire::{keys::*, spa::utils::dict::DictRef};
use serde::{Deserialize, Serialize};

/// Handles parsing all of the identifying fields on a Node, and uses them to generate different
/// identifiers. Only serializes fields relevant to identifying the node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeIdentifier {
    node_name: Option<String>,
    node_nick: Option<String>,
    node_description: Option<String>,
    object_path: Option<String>,
    #[serde(skip)]
    pub application_name: Option<String>,
    #[serde(skip)]
    pub binary_name: Option<String>,
    #[serde(skip)]
    media_name: Option<String>,
    #[serde(skip)]
    media_title: Option<String>,
    #[serde(skip)]
    device_id: Option<u32>,
    #[serde(skip)]
    route_name: Option<String>,
    #[serde(skip)]
    app_icon_name: Option<String>,
    #[serde(skip)]
    icon_name_: OnceLock<String>,
    #[serde(skip)]
    identifier_: OnceLock<String>,
    #[serde(skip)]
    human_name_: OnceLock<String>,
    #[serde(skip)]
    details_: OnceLock<Option<String>>,
}

impl NodeIdentifier {
    pub fn from_props(props: &DictRef) -> Self {
        Self {
            node_name: props.get(*NODE_NAME).map(ToOwned::to_owned),
            node_nick: props.get(*NODE_NICK).map(ToOwned::to_owned),
            node_description: props.get(*NODE_DESCRIPTION).map(ToOwned::to_owned),
            object_path: props.get(*OBJECT_PATH).map(ToOwned::to_owned),
            application_name: props.get(*APP_NAME).map(ToOwned::to_owned),
            binary_name: props.get(*APP_PROCESS_BINARY).map(ToOwned::to_owned),
            media_name: props.get(*OBJECT_PATH).map(ToOwned::to_owned),
            media_title: props.get(*MEDIA_TITLE).map(ToOwned::to_owned),
            device_id: props.get(*DEVICE_ID).and_then(|id| id.parse().ok()),
            route_name: None,
            app_icon_name: props.get(*APP_ICON_NAME).map(ToOwned::to_owned),
            icon_name_: OnceLock::new(),
            identifier_: OnceLock::new(),
            human_name_: OnceLock::new(),
            details_: OnceLock::new(),
        }
    }

    #[cfg(test)]
    pub fn new_test() -> Self {
        Self {
            node_name: None,
            node_nick: None,
            node_description: None,
            object_path: None,
            application_name: None,
            binary_name: None,
            media_name: None,
            media_title: None,
            device_id: None,
            route_name: None,
            app_icon_name: None,
            icon_name_: OnceLock::new(),
            identifier_: OnceLock::new(),
            human_name_: OnceLock::new(),
            details_: OnceLock::new(),
        }
    }

    #[rustfmt::skip]
    pub fn update_from_props(&mut self, props: &DictRef) {
        self.node_name         = props.get(*NODE_NAME)          .map(ToOwned::to_owned).or(self.node_name.take());
        self.node_nick         = props.get(*NODE_NICK)          .map(ToOwned::to_owned).or(self.node_nick.take());
        self.node_description  = props.get(*NODE_DESCRIPTION)   .map(ToOwned::to_owned).or(self.node_description.take());
        self.object_path       = props.get(*OBJECT_PATH)        .map(ToOwned::to_owned).or(self.object_path.take());
        self.application_name  = props.get(*APP_NAME)           .map(ToOwned::to_owned).or(self.application_name.take());
        self.binary_name       = props.get(*APP_PROCESS_BINARY) .map(ToOwned::to_owned).or(self.binary_name.take());
        self.media_name        = props.get(*MEDIA_NAME)         .map(ToOwned::to_owned).or(self.media_name.take());
        self.media_title       = props.get(*MEDIA_TITLE)        .map(ToOwned::to_owned).or(self.media_title.take());
        self.app_icon_name     = props.get(*APP_ICON_NAME)      .map(ToOwned::to_owned).or(self.app_icon_name.take());
        self.device_id         = props.get(*DEVICE_ID)          .and_then(|id| id.parse().ok()).or(self.device_id);

        self.icon_name_.take();
        self.identifier_.take();
        self.human_name_.take();
        self.details_.take();
    }

    pub fn icon_name(&self) -> &str {
        self.icon_name_.get_or_init(|| {
            self.app_icon_name
                .as_ref()
                .map(AsRef::as_ref)
                .unwrap_or_else(|| {
                    if self.device_id.is_some() {
                        "audio-card"
                    } else {
                        "preferences-desktop-multimedia"
                    }
                })
                .to_owned()
        })
    }

    pub fn identifier(&self) -> &str {
        self.identifier_.get_or_init(|| {
            self.node_name
                .as_ref()
                .or_else(|| self.object_path.as_ref())
                .or_else(|| self.node_description.as_ref())
                .or_else(|| self.node_nick.as_ref())
                .map(String::clone)
                .unwrap_or_default()
        })
    }

    pub fn human_name(&self) -> &str {
        self.human_name_.get_or_init(|| {
            self.node_description
                .as_ref()
                .or_else(|| self.node_nick.as_ref())
                .or_else(|| self.application_name.as_ref())
                .or_else(|| self.route_name.as_ref())
                .or_else(|| self.node_name.as_ref())
                .map(String::clone)
                .unwrap_or_default()
        })
    }

    pub fn details(&self) -> Option<&str> {
        self.details_
            .get_or_init(|| {
                self.route_name
                    .as_ref()
                    .or_else(|| self.media_name.as_ref())
                    .or_else(|| self.media_title.as_ref())
                    .or_else(|| {
                        self.application_name
                            .as_ref()
                            .filter(|app_name| *app_name != self.human_name())
                    })
                    .map(|s| Some(s.clone()))
                    .unwrap_or_default()
            })
            .as_ref()
            .map(AsRef::as_ref)
    }

    #[rustfmt::skip] // Rustfmt inconsistently expands the lines and it's really hard to read
    pub fn matches(&self, other: &NodeIdentifier) -> bool {
        // Compare the first property that exist on both identifiers
        let ids = self.node_name.as_ref().zip(other.node_name.as_ref())
            .or_else(|| self.object_path.as_ref().zip(other.object_path.as_ref()))
            .or_else(|| self.node_description.as_ref().zip(other.node_description.as_ref()))
            .or_else(|| self.node_nick.as_ref().zip(other.node_nick.as_ref()));

        if let Some((left, right)) = ids {
            left == right
        } else {
            false
        }
    }
}
