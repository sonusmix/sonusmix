use pipewire::{
    spa::{
        param::ParamType,
        pod::{object, Object, Pod, Property, Value, ValueArray},
        sys::{
            SPA_PARAM_ROUTE_device, SPA_PARAM_ROUTE_index, SPA_PARAM_ROUTE_info,
            SPA_PARAM_ROUTE_props, SPA_PARAM_ROUTE_save, SPA_PROP_channelVolumes,
        },
        utils::SpaTypes,
    },
    sys::PW_KEY_DEVICE_ICON_NAME,
};

pub mod parse {
    use std::{ffi::CStr, io::Cursor};

    use pipewire::spa::{
        pod::{
            deserialize::PodDeserializer, serialize::PodSerializer, Object, Pod, Value, ValueArray,
        },
        sys::SPA_KEY_DEVICE_ICON_NAME,
    };

    pub trait PodExt {
        fn deserialize_value(&self) -> Option<Value>;
    }
    impl PodExt for Pod {
        fn deserialize_value(&self) -> Option<Value> {
            PodDeserializer::deserialize_any_from(self.as_bytes())
                .map(|(_, value)| value)
                .ok()
        }
    }

    /// A [`Pod`] is like a [`str`]: it represents a sequence of bytes that are known to be in a
    /// certain format, but may only exist behind a reference. Therefore, a function cannot return
    /// a [`Pod`] directly. Instead, this type owns those bytes until the [`Pod`] is needed,
    /// similar to what a [`String`] does for a [`&str`].
    pub struct PodBytes(Box<[u8]>);

    impl PodBytes {
        pub fn from_value(value: &Value) -> Self {
            let mut bytes = Vec::new();
            PodSerializer::serialize(Cursor::new(&mut bytes), value)
                .expect("A Vec should not have any errors serializing");
            Self(bytes.into_boxed_slice())
        }

        pub fn pod(&self) -> &Pod {
            Pod::from_bytes(&self.0).expect("Internal bytes are known to be a well-formed Pod")
        }
    }

    pub trait PodValueExt {
        fn parse_int(&self) -> Option<i32>;
        fn parse_string(&self) -> Option<&str>;
        fn parse_value_array(&self) -> Option<&ValueArray>;
        fn parse_struct(&self) -> Option<&[Value]>;
        fn parse_object(&self) -> Option<&Object>;
        fn serialize(&self) -> PodBytes;
    }
    impl PodValueExt for Value {
        fn parse_int(&self) -> Option<i32> {
            match self {
                Value::Int(x) => Some(*x),
                _ => None,
            }
        }
        fn parse_string(&self) -> Option<&str> {
            match self {
                Value::String(s) => Some(s),
                _ => None,
            }
        }
        fn parse_value_array(&self) -> Option<&ValueArray> {
            match self {
                Value::ValueArray(value_array) => Some(value_array),
                _ => None,
            }
        }
        fn parse_struct(&self) -> Option<&[Value]> {
            match self {
                Value::Struct(struct_) => Some(struct_),
                _ => None,
            }
        }
        fn parse_object(&self) -> Option<&Object> {
            match self {
                Value::Object(object) => Some(object),
                _ => None,
            }
        }
        fn serialize(&self) -> PodBytes {
            PodBytes::from_value(self)
        }
    }

    pub trait PodValueArrayExt {
        fn parse_floats(&self) -> Option<&[f32]>;
    }
    impl PodValueArrayExt for ValueArray {
        fn parse_floats(&self) -> Option<&[f32]> {
            match self {
                ValueArray::Float(f) => Some(f),
                _ => None,
            }
        }
    }

    pub trait PodStructExt {
        fn get_key(&self, key: &str) -> Option<&Value>;
    }
    impl PodStructExt for [Value] {
        fn get_key(&self, key: &str) -> Option<&Value> {
            let mut iter = self.iter();
            // Consume items of the iterator up to and including the key
            iter.by_ref()
                .take_while(|val| val.parse_string().map(|s| s == key).unwrap_or_default())
                .count();
            // Return the item after the key
            iter.next()
        }
    }

    pub trait PodObjectExt {
        fn get_key(&self, key: u32) -> Option<&Value>;
    }
    impl PodObjectExt for Object {
        fn get_key(&self, key: u32) -> Option<&Value> {
            self.properties
                .iter()
                .find_map(|prop| (prop.key == key).then(|| &prop.value))
        }
    }

    pub const STRUCT_KEY_DEVICE_ICON_NAME: &str =
        if let Ok(s) = CStr::from_bytes_with_nul(b"device.icon-name\0") {
            if let Ok(s) = s.to_str() {
                s
            } else {
                panic!("DEVICE_ICON_NAME key is not valid UTF-8");
            }
        } else {
            panic!("DEVICE_ICON_NAME key is not null-terminated");
        };
}
use parse::*;

#[derive(Debug)]
pub(super) struct NodeProps {
    value: Value,
}

impl NodeProps {
    pub fn new(value: Value) -> Self {
        NodeProps { value }
    }

    pub fn get_channel_volumes(&self) -> Option<&[f32]> {
        self.value
            .parse_object()?
            .get_key(SPA_PROP_channelVolumes)?
            .parse_value_array()?
            .parse_floats()
    }
}

/// `Props '{ channelVolumes: <channel_volumes> }'`
pub fn build_node_volume_pod(channel_volumes: Vec<f32>) -> (ParamType, PodBytes) {
    let pod = Value::Object(object! {
        SpaTypes::ObjectParamProps,
        ParamType::Props,
        Property::new(SPA_PROP_channelVolumes, Value::ValueArray(ValueArray::Float(channel_volumes))),
    }).serialize();
    (ParamType::Props, pod)
}

#[derive(Debug, Clone)]
pub(super) struct DeviceActiveRoute {
    pub route_index: i32,
    pub device_index: i32,
    pub icon_name: Option<String>,
}

impl DeviceActiveRoute {
    pub fn from_value(pod: &Pod) -> Option<Self> {
        let value = pod.deserialize_value()?;
        let obj = value.parse_object()?;
        Some(Self {
            route_index: obj.get_key(SPA_PARAM_ROUTE_index)?.parse_int()?,
            device_index: obj.get_key(SPA_PARAM_ROUTE_device)?.parse_int()?,
            icon_name: obj
                .get_key(SPA_PARAM_ROUTE_info)?
                .parse_struct()?
                .get_key(STRUCT_KEY_DEVICE_ICON_NAME)
                .and_then(|v| v.parse_string())
                .map(ToOwned::to_owned),
        })
    }

    /// `Route '{ index: <route_index>, device: <device_index>, props: { channelVolumes: <channel_volumes> }, save: true }'
    pub fn build_device_volume_pod(&self, channel_volumes: Vec<f32>) -> (ParamType, PodBytes) {
        let pod = Value::Object(object! {
            SpaTypes::ObjectParamRoute,
            ParamType::Route,
            Property::new(SPA_PARAM_ROUTE_index, Value::Int(self.route_index)),
            Property::new(SPA_PARAM_ROUTE_device, Value::Int(self.device_index)),
            Property::new(SPA_PARAM_ROUTE_props, Value::Object(object! {
                SpaTypes::ObjectParamProps,
                ParamType::Route,
                Property::new(SPA_PROP_channelVolumes, Value::ValueArray(ValueArray::Float(channel_volumes)))
            })),
            Property::new(SPA_PARAM_ROUTE_save, Value::Bool(true)),
        }).serialize();
        (ParamType::Route, pod)
    }
}
