use std::{cell::RefCell, fmt::Debug, rc::Rc, str::FromStr, sync::{Arc, RwLock}};

use pipewire::{keys::*, registry::GlobalObject, spa::utils::dict::DictRef, types::ObjectType};
use serde::{Deserialize, Serialize};
use thiserror::Error;

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
        expected: ObjectType,
        actual: ObjectType,
    },
    #[error("object has no props")]
    NoProps { object_dbg: String },
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

    fn check_type(&self, expected: ObjectType) -> Result<(), ObjectConvertError>;
    fn get_props(&self) -> Result<&DictRef, ObjectConvertError>;
    fn parse_field<T: FromStr>(
        &self,
        field: &'static str,
        expected: &'static str,
    ) -> Result<T, ObjectConvertError>;
}

impl<'a> ObjectConvertErrorExt for GlobalObject<&'a DictRef> {
    fn check_type(&self, expected: ObjectType) -> Result<(), ObjectConvertError> {
        if self.type_ == expected {
            Ok(())
        } else {
            Err(ObjectConvertError::IncorrectType {
                object_dbg: format!("{self:?}"),
                expected,
                actual: self.type_.clone(),
            })
        }
    }

    fn get_props(&self) -> Result<&DictRef, ObjectConvertError> {
        self.props.ok_or_else(|| ObjectConvertError::NoProps {
            object_dbg: format!("{self:?}"),
        })
    }

    fn parse_field<T: FromStr>(
        &self,
        field: &'static str,
        expected: &'static str,
    ) -> Result<T, ObjectConvertError> {
        let str_value = self
            .get_props()?
            .get(field)
            .ok_or_else(|| self.missing_field(field))?;
        str_value
            .parse()
            .map_err(|_| self.invalid_value(field, expected, str_value))
    }
}

// #[derive(Clone, Copy, PartialEq, Eq)]
// pub enum Category {
//     Physical,
//     Virtual,
//     SonusmixVirtual,
// }


#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    pub id: u32,
    pub name: String,
    pub device_kind: DeviceKind,
    pub nodes: RefCell<Vec<Arc<Node>>>,
}


#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DeviceKind {
    Physical,
    Application,
    Virtual,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Node {
    pub id: u32,
    pub name: String,
    pub ports: RwLock<Vec<Arc<Port>>>,
}

impl<'a> TryFrom<&'a GlobalObject<&'a DictRef>> for Node {
    type Error = ObjectConvertError;

    fn try_from(object: &'a GlobalObject<&'a DictRef>) -> Result<Self, Self::Error> {
        object.check_type(ObjectType::Node)?;
        let props = object.get_props()?;

        Ok(Self {
            id: object.id,
            name: props
                .get(*NODE_NICK)
                .or_else(|| props.get("node.description"))
                .or_else(|| props.get(*APP_NAME))
                .or_else(|| props.get(*NODE_NAME))
                // TODO: List all of the possible field names
                .ok_or_else(|| object.missing_field(*NODE_NAME))?
                .to_owned(),
            ports: RwLock::new(Vec::new()),
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Port {
    pub id: u32,
    pub name: String,
    pub node: u32,
    pub kind: PortKind,
    pub links: RwLock<Vec<Arc<Link>>>,
}

impl<'a> TryFrom<&'a GlobalObject<&'a DictRef>> for Port {
    type Error = ObjectConvertError;

    fn try_from(object: &'a GlobalObject<&'a DictRef>) -> Result<Self, Self::Error> {
        object.check_type(ObjectType::Node)?;
        let props = object.get_props()?;

        Ok(Self {
            id: object.id,
            name: props
                .get(*PORT_NAME)
                .ok_or_else(|| object.missing_field(*PORT_NAME))?
                .to_owned(),
            node: object.parse_field(*NODE_ID, "integer")?,
            kind: object.parse_field(*PORT_DIRECTION, "'in' or 'out'")?,
            links: RwLock::new(Vec::new()),
        })
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Link {
    pub id: u32,
    pub start_node: u32,
    pub start_port: u32,
    pub end_node: u32,
    pub end_port: u32,
}

impl<'a> TryFrom<&'a GlobalObject<&'a DictRef>> for Link {
    type Error = ObjectConvertError;

    fn try_from(object: &'a GlobalObject<&'a DictRef>) -> Result<Self, Self::Error> {
        object.check_type(ObjectType::Node)?;

        Ok(Self {
            id: object.id,
            start_node: object.parse_field(*LINK_INPUT_NODE, "integer")?,
            start_port: object.parse_field(*LINK_INPUT_PORT, "integer")?,
            end_node: object.parse_field(*LINK_OUTPUT_NODE, "integer")?,
            end_port: object.parse_field(*LINK_OUTPUT_PORT, "integer")?,
        })
    }
}
