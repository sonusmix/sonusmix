use std::{
    fmt::Debug,
    rc::Rc,
    str::FromStr,
    sync::{Arc, Mutex},
};

use derivative::Derivative;
use log::debug;
use pipewire::{
    keys::*,
    registry::{GlobalObject, Registry},
    spa::{
        param::ParamType,
        pod::{deserialize::PodDeserializer, object, Property, Value, ValueArray},
        sys::SPA_PROP_channelVolumes,
        utils::{dict::DictRef, SpaTypes},
    },
    types::ObjectType,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{identifier::Identifier, pod::DeviceActiveRoute, SONUSMIX_APP_NAME};

#[derive(Error, Debug)]
pub enum ObjectConvertError {
    #[error("missing field: {field}")]
    MissingField {
        object_dbg: String,
        field: &'static str,
    },
    #[error("invalid value on field '{field}': expected {expected}, got '{actual}'")]
    InvalidValue {
        object_dbg: String,
        field: &'static str,
        expected: &'static str,
        actual: String,
    },
    #[error("incorrect object type: expected {expected}, got {actual}")]
    IncorrectType {
        object_dbg: String,
        expected: &'static str,
        actual: ObjectType,
    },
    #[error("object has no props")]
    NoProps { object_dbg: String },
    #[error("Pipewire error: {0:?}")]
    PipewireError(#[from] pipewire::Error),
}

trait ObjectConvertErrorExt: Debug {
    fn missing_field(&self, field: &'static str) -> ObjectConvertError {
        ObjectConvertError::MissingField {
            object_dbg: format!("{self:?}"),
            field,
        }
    }

    fn invalid_value(
        &self,
        field: &'static str,
        expected: &'static str,
        actual: impl AsRef<str>,
    ) -> ObjectConvertError {
        ObjectConvertError::InvalidValue {
            object_dbg: format!("{self:?}"),
            field,
            expected,
            actual: actual.as_ref().to_owned(),
        }
    }

    fn check_type(
        &self,
        expected: ObjectType,
        expected_str: &'static str,
    ) -> Result<(), ObjectConvertError>;
    fn get_props(&self) -> Result<&DictRef, ObjectConvertError>;
    fn parse_fields<T: FromStr, const N: usize>(
        &self,
        field: [&'static str; N],
        expected: &'static str,
    ) -> Result<T, ObjectConvertError>;
}

impl<'a> ObjectConvertErrorExt for GlobalObject<&'a DictRef> {
    fn check_type(
        &self,
        expected: ObjectType,
        expected_str: &'static str,
    ) -> Result<(), ObjectConvertError> {
        if self.type_ == expected {
            Ok(())
        } else {
            Err(ObjectConvertError::IncorrectType {
                object_dbg: format!("{self:?}"),
                expected: expected_str,
                actual: self.type_.clone(),
            })
        }
    }

    fn get_props(&self) -> Result<&DictRef, ObjectConvertError> {
        self.props.ok_or_else(|| ObjectConvertError::NoProps {
            object_dbg: format!("{self:?}"),
        })
    }

    fn parse_fields<T: FromStr, const N: usize>(
        &self,
        fields: [&'static str; N],
        expected: &'static str,
    ) -> Result<T, ObjectConvertError> {
        let props = self.get_props()?;
        let (field, str_value) = fields
            .into_iter()
            .filter_map(|field| props.get(field).map(|str_value| (field, str_value)))
            .next()
            .ok_or_else(|| self.missing_field(fields[fields.len() - 1]))?;
        str_value
            .parse()
            .map_err(|_| self.invalid_value(field, expected, str_value))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Client<P = pipewire::client::Client> {
    pub id: u32,
    pub name: String,
    pub is_sonusmix: bool,
    pub nodes: Vec<u32>,
    #[serde(skip)]
    pub(super) proxy: P,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EndpointKind {
    Physical,
    Application,
    Sonusmix,
}

impl Client<pipewire::client::Client> {
    pub(super) fn from_global(
        registry: &Registry,
        object: &GlobalObject<&DictRef>,
    ) -> Result<Self, ObjectConvertError> {
        object.check_type(ObjectType::Client, "Client")?;
        let props = object.get_props()?;
        let proxy = registry.bind(object)?;

        let name = props
            .get(*APP_NAME)
            .ok_or_else(|| object.missing_field(*APP_NAME))?;

        Ok(Self {
            id: object.id,
            name: name.to_owned(),
            is_sonusmix: name == SONUSMIX_APP_NAME,
            nodes: Vec::new(),
            proxy,
        })
    }

    pub fn without_proxy(&self) -> Client<()> {
        Client {
            id: self.id,
            name: self.name.clone(),
            is_sonusmix: self.is_sonusmix,
            nodes: self.nodes.clone(),
            proxy: (),
        }
    }

    #[cfg(test)]
    pub fn new_test(id: u32, is_sonusmix: bool, nodes: Vec<u32>) -> Client<()> {
        Client {
            id,
            name: "TESTING CLIENT".to_string(),
            is_sonusmix,
            nodes,
            proxy: (),
        }
    }
}

#[derive(Derivative, Clone)]
#[derivative(Debug)]
pub struct Device<P = pipewire::device::Device, L = Option<pipewire::device::DeviceListener>> {
    pub id: u32,
    pub name: String,
    pub client: u32,
    pub nodes: Vec<u32>,
    pub active_routes: Vec<DeviceActiveRoute>,
    pub(super) proxy: P,
    #[derivative(Debug = "ignore")]
    pub(super) listener: L,
}

impl Device {
    pub(super) fn from_global(
        registry: &Registry,
        object: &GlobalObject<&DictRef>,
    ) -> Result<Self, ObjectConvertError> {
        object.check_type(ObjectType::Device, "Device")?;
        let proxy: pipewire::device::Device = registry.bind(object)?;
        let props = object.get_props()?;

        Ok(Self {
            id: object.id,
            name: props
                .get(*DEVICE_NICK)
                .or_else(|| props.get(*DEVICE_DESCRIPTION))
                .or_else(|| props.get(*DEVICE_NAME))
                // TODO: List all of the possible field names
                .ok_or_else(|| object.missing_field(*DEVICE_NAME))?
                .to_owned(),
            client: object.parse_fields([*CLIENT_ID], "integer")?,
            nodes: Vec::new(),
            active_routes: Vec::new(),
            proxy,
            listener: None,
        })
    }

    pub fn without_proxy(&self) -> Device<(), ()> {
        Device {
            id: self.id,
            name: self.name.clone(),
            client: self.client,
            nodes: self.nodes.clone(),
            active_routes: self.active_routes.clone(),
            proxy: (),
            listener: (),
        }
    }
}

#[derive(Derivative, Clone)]
#[derivative(Debug)]
pub struct Node<P = pipewire::node::Node, L = Option<pipewire::node::NodeListener>> {
    pub id: u32,
    pub identifier: Identifier,
    pub endpoint: EndpointId,
    pub ports: Vec<u32>,
    // #[serde(skip)]
    pub channel_volumes: Vec<f32>,
    pub mute: bool,
    pub(super) proxy: P,
    // listener is set by mainloop
    #[derivative(Debug = "ignore")]
    pub(super) listener: L,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")] // Defaults to externally tagged
pub enum EndpointId {
    Device { id: u32, device_index: Option<i32> },
    Client(u32),
}

impl Node {
    pub(super) fn from_global(
        registry: &Registry,
        object: &GlobalObject<&DictRef>,
    ) -> Result<Self, ObjectConvertError> {
        object.check_type(ObjectType::Node, "Node")?;
        let props = object.get_props()?;
        let proxy: pipewire::node::Node = registry.bind(object)?;

        Ok(Self {
            id: object.id,
            identifier: Identifier::from_props(props),
            endpoint: if let Some(id) = props.get(*DEVICE_ID) {
                EndpointId::Device {
                    id: id
                        .parse()
                        .map_err(|_| object.invalid_value(*DEVICE_ID, "integer", id))?,
                    device_index: props
                        .get("card.profile.device")
                        .map(|device_index| {
                            device_index
                                .parse()
                                .map_err(|_| object.invalid_value(*DEVICE_ID, "integer", id))
                        })
                        .transpose()?,
                }
            } else if let Some(id) = props.get(*CLIENT_ID) {
                id.parse()
                    .map(EndpointId::Client)
                    .map_err(|_| object.invalid_value(*CLIENT_ID, "integer", id))?
            } else {
                // TODO: Better error message, maybe listing both device and client field names
                return Err(object.missing_field(*CLIENT_ID));
            },
            ports: Vec::new(),
            channel_volumes: Vec::new(),
            mute: false,
            proxy,
            listener: None,
        })
    }

    pub fn without_proxy(&self) -> Node<(), ()> {
        Node {
            id: self.id,
            identifier: self.identifier.clone(),
            endpoint: self.endpoint,
            ports: self.ports.clone(),
            channel_volumes: self.channel_volumes.clone(),
            mute: self.mute,
            proxy: (),
            listener: (),
        }
    }

    #[cfg(test)]
    pub fn new_test(id: u32, endpoint: EndpointId) -> Node<(), ()> {
        Node {
            id: 0,
            identifier: Identifier::new_test(),
            endpoint: EndpointId::Client(0),
            ports: Vec::new(),
            channel_volumes: Vec::new(),
            mute: false,
            proxy: (),
            listener: (),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Port<P = pipewire::port::Port> {
    pub id: u32,
    pub name: String,
    pub channel: String,
    pub node: u32,
    pub kind: PortKind,
    pub links: Vec<u32>,
    #[serde(skip)]
    pub(super) proxy: P,
}

impl Port {
    pub(super) fn from_global(
        registry: &Registry,
        object: &GlobalObject<&DictRef>,
    ) -> Result<Self, ObjectConvertError> {
        object.check_type(ObjectType::Port, "Port")?;
        let props = object.get_props()?;
        let proxy = registry.bind(object)?;

        Ok(Self {
            id: object.id,
            name: props
                .get(*PORT_NAME)
                .ok_or_else(|| object.missing_field(*PORT_NAME))?
                .to_owned(),
            channel: props
                .get(*AUDIO_CHANNEL)
                .ok_or_else(|| object.missing_field(*AUDIO_CHANNEL))?
                .to_owned(),
            node: object.parse_fields([*NODE_ID], "integer")?,
            kind: object.parse_fields([*PORT_DIRECTION], "'in' or 'out'")?,
            links: Vec::new(),
            proxy,
        })
    }

    pub fn without_proxy(&self) -> Port<()> {
        Port {
            id: self.id,
            name: self.name.clone(),
            channel: self.channel.clone(),
            node: self.node,
            kind: self.kind,
            links: self.links.clone(),
            proxy: (),
        }
    }

    #[cfg(test)]
    pub fn new_test(id: u32, node: u32, kind: PortKind) -> Port<()> {
        Port {
            id,
            name: "TESTING PORT".to_string(),
            channel: "L".to_string(),
            node,
            kind,
            links: Vec::new(),
            proxy: (),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PortKind {
    Source,
    Sink,
}

impl FromStr for PortKind {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "in" => Ok(Self::Sink),
            "out" => Ok(Self::Source),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Link<P = pipewire::link::Link> {
    pub id: u32,
    pub start_node: u32,
    pub start_port: u32,
    pub end_node: u32,
    pub end_port: u32,
    #[serde(skip)]
    pub(super) proxy: P,
}

impl Link {
    pub(super) fn from_global(
        registry: &Registry,
        object: &GlobalObject<&DictRef>,
    ) -> Result<Self, ObjectConvertError> {
        object.check_type(ObjectType::Link, "Link")?;
        let proxy = registry.bind(object)?;

        Ok(Self {
            id: object.id,
            // The "input" is the node/port acting as an input, not the input to the link, and the
            // same for output.
            start_node: object.parse_fields([*LINK_OUTPUT_NODE], "integer")?,
            start_port: object.parse_fields([*LINK_OUTPUT_PORT], "integer")?,
            end_node: object.parse_fields([*LINK_INPUT_NODE], "integer")?,
            end_port: object.parse_fields([*LINK_INPUT_PORT], "integer")?,
            proxy,
        })
    }

    pub fn without_proxy(&self) -> Link<()> {
        Link {
            id: self.id,
            start_node: self.start_node,
            start_port: self.start_port,
            end_node: self.end_node,
            end_port: self.end_port,
            proxy: (),
        }
    }
}
