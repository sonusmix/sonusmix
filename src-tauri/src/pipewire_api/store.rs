use std::fmt::Debug;

use pipewire::{registry::GlobalObject, spa::utils::dict::DictRef};
use thiserror::Error;

struct Store {
    client_id: Option<u32>,
}

#[derive(Error, Debug)]
enum ObjectConvertError {
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
}

trait ObjectConvertErrorExt: Debug {
    fn missing_field(&self, field: &'static str) -> ObjectConvertError {
        ObjectConvertError::MissingField {
            object_dbg: format!("{:?}", self),
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
            object_dbg: format!("{:?}", self),
            field,
            expected,
            actual: actual.as_ref().to_owned(),
        }
    }
}

trait PipewireObjectType<'a>: TryFrom<GlobalObject<&'a DictRef>, Error = ObjectConvertError> {}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Category {
    Physical,
    Virtual,
    SonusmixVirtual,
}

struct PipewireObject<T>
{
    id: u32,
    category: Category,
    object: T,
}
