use std::{fmt::Debug, str::FromStr};

use pipewire::{
    keys::*,
    registry::{GlobalObject, Registry},
    spa::utils::dict::DictRef,
    types::ObjectType,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::SONUSMIX_APP_NAME;

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

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Client<T = pipewire::client::Client> {
    pub id: u32,
    pub name: String,
    pub is_sonusmix: bool,
    pub nodes: Vec<u32>,
    #[serde(skip)]
    pub(super) proxy: T,
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
        object.check_type(ObjectType::Client, "Client");
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
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Device<T = pipewire::device::Device> {
    pub id: u32,
    pub name: String,
    pub client: u32,
    pub nodes: Vec<u32>,
    #[serde(skip)]
    pub(super) proxy: T,
}

impl Device<pipewire::device::Device> {
    pub(super) fn from_global(
        registry: &Registry,
        object: &GlobalObject<&DictRef>,
    ) -> Result<Self, ObjectConvertError> {
        object.check_type(ObjectType::Device, "Device");
        let proxy = registry.bind(object)?;
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
            proxy,
        })
    }

    pub fn without_proxy(&self) -> Device<()> {
        Device {
            id: self.id,
            name: self.name.clone(),
            client: self.client,
            nodes: self.nodes.clone(),
            proxy: (),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Node<T = pipewire::node::Node> {
    pub id: u32,
    pub name: String,
    pub endpoint: u32,
    pub ports: Vec<u32>,
    #[serde(skip)]
    pub(super) proxy: T,
}

impl Node<pipewire::node::Node> {
    pub(super) fn from_global(
        registry: &Registry,
        object: &GlobalObject<&DictRef>,
    ) -> Result<Self, ObjectConvertError> {
        object.check_type(ObjectType::Node, "Node")?;
        let props = object.get_props()?;
        let proxy = registry.bind(object)?;

        Ok(Self {
            id: object.id,
            name: props
                .get(*NODE_NICK)
                .or_else(|| props.get(*NODE_DESCRIPTION))
                .or_else(|| props.get(*APP_NAME))
                .or_else(|| props.get(*NODE_NAME))
                // TODO: List all of the possible field names
                .ok_or_else(|| object.missing_field(*NODE_NAME))?
                .to_owned(),
            endpoint: object.parse_fields([*DEVICE_ID, *CLIENT_ID], "integer")?,
            ports: Vec::new(),
            proxy,
        })
    }

    pub fn without_proxy(&self) -> Node<()> {
        Node {
            id: self.id,
            name: self.name.clone(),
            endpoint: self.endpoint,
            ports: self.ports.clone(),
            proxy: (),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Port<P = pipewire::port::Port> {
    pub id: u32,
    pub name: String,
    pub node: u32,
    pub kind: PortKind,
    pub links: Vec<u32>,
    #[serde(skip)]
    pub(super) proxy: P,
}

impl Port<pipewire::port::Port> {
    pub (super) fn from_global(
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
            node: self.node,
            kind: self.kind,
            links: self.links.clone(),
            proxy: (),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Serialize, Deserialize)]
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
            start_node: object.parse_fields([*LINK_INPUT_NODE], "integer")?,
            start_port: object.parse_fields([*LINK_INPUT_PORT], "integer")?,
            end_node: object.parse_fields([*LINK_OUTPUT_NODE], "integer")?,
            end_port: object.parse_fields([*LINK_OUTPUT_PORT], "integer")?,
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
