use std::{fmt::Debug, str::FromStr};

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

trait PipewireObjectType<'a>:
    TryFrom<&'a GlobalObject<&'a DictRef>, Error = ObjectConvertError>
{
}

// #[derive(Clone, Copy, PartialEq, Eq)]
// pub enum Category {
//     Physical,
//     Virtual,
//     SonusmixVirtual,
// }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipewireObject<T> {
    pub id: u32,
    pub object: T,
}

impl<'a, T> PipewireObject<T>
where
    T: PipewireObjectType<'a>,
{
    fn from_global_object(
        object: &'a GlobalObject<&'a DictRef>,
    ) -> Result<Self, ObjectConvertError> {
        Ok(Self {
            id: object.id,
            object: object.try_into()?,
        })
    }
}

impl<'a, T> TryFrom<&'a GlobalObject<&'a DictRef>> for PipewireObject<T>
where
    T: PipewireObjectType<'a>,
{
    type Error = ObjectConvertError;

    fn try_from(object: &'a GlobalObject<&'a DictRef>) -> Result<Self, Self::Error> {
        PipewireObject::from_global_object(object)
    }
}

// #[derive(Debug)]
// pub enum AnyPipewireObject {
//     Node(PipewireObject<Node>),
// }

// impl AnyPipewireObject {
//     /// Returns None if the object was an unsupported type
//     pub(super) fn from_global_object(
//         object: &GlobalObject<&DictRef>,
//     ) -> Option<Result<Self, ObjectConvertError>> {
//         Some(match object.type_ {
//             ObjectType::Node => PipewireObject::<Node>::from_global_object(object).map(Self::Node),
//             _ => return None,
//         })
//     }
// }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    name: String,
}

impl<'a> TryFrom<&'a GlobalObject<&'a DictRef>> for Node {
    type Error = ObjectConvertError;

    fn try_from(object: &'a GlobalObject<&'a DictRef>) -> Result<Self, Self::Error> {
        object.check_type(ObjectType::Node)?;
        let props = object.get_props()?;

        Ok(Self {
            name: props
                .get(*NODE_NICK)
                .or_else(|| props.get("node.description"))
                .or_else(|| props.get(*APP_NAME))
                .or_else(|| props.get(*NODE_NAME))
                // TODO: List all of the possible field names
                .ok_or_else(|| object.missing_field(*NODE_NAME))?
                .to_owned(),
        })
    }
}

impl<'a> PipewireObjectType<'a> for Node {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Port {
    name: String,
    node: u32,
}

impl<'a> TryFrom<&'a GlobalObject<&'a DictRef>> for Port {
    type Error = ObjectConvertError;

    fn try_from(object: &'a GlobalObject<&'a DictRef>) -> Result<Self, Self::Error> {
        object.check_type(ObjectType::Node)?;
        let props = object.get_props()?;

        Ok(Self {
            name: props
                .get(*PORT_NAME)
                .ok_or_else(|| object.missing_field(*PORT_NAME))?
                .to_owned(),
            node: object.parse_field(*NODE_ID, "integer")?,
        })
    }
}
