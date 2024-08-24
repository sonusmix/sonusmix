use pipewire::spa::{
    pod::{
        self,
        deserialize::{
            DeserializeError, DeserializeSuccess, ObjectPodDeserializer, PodDeserialize,
            PodDeserializer, Visitor,
        },
        serialize::PodSerialize,
        Property, Value, ValueArray,
    },
    sys::SPA_PROP_channelVolumes,
};

#[derive(Debug)]
pub(super) struct NodeProps {
    value: Value,
}

impl<'de> PodDeserialize<'de> for NodeProps {
    fn deserialize(
        deserializer: PodDeserializer<'de>,
    ) -> Result<(Self, DeserializeSuccess<'de>), DeserializeError<&'de [u8]>>
    where
        Self: Sized,
    {
        let (value, success) = Value::deserialize(deserializer)?;

        if !matches!(
            &value,
            Value::Object(object) if object.properties
                .iter()
                .any(|prop| prop.key == SPA_PROP_channelVolumes
                    && matches!(prop.value, Value::ValueArray(ValueArray::Float(_))))
        ) {
            return Err(DeserializeError::PropertyMissing);
        }

        Ok((Self { value }, success))
    }
}

impl PodSerialize for NodeProps {
    fn serialize<O: std::io::Write + std::io::Seek>(
        &self,
        serializer: pod::serialize::PodSerializer<O>,
    ) -> Result<pod::serialize::SerializeSuccess<O>, pod::serialize::GenError> {
        self.value.serialize(serializer)
    }
}

impl NodeProps {
    pub fn new(value: Value) -> Self {
        NodeProps { value }
    }

    pub fn get_volumes(&self) -> Option<&[f32]> {
        if let Value::Object(object) = &self.value {
            if let Some(Value::ValueArray(ValueArray::Float(channel_volumes))) = object
                .properties
                .iter()
                .find_map(|prop| (prop.key == SPA_PROP_channelVolumes).then(|| &prop.value))
            {
                return Some(channel_volumes);
            }
        }
        return None
    }

    pub fn get_volumes_mut(&mut self) -> &mut Vec<f32> {
            if let Value::Object(object) = &mut self.value {
                if let Some(Value::ValueArray(ValueArray::Float(channel_volumes))) = object
                    .properties
                    .iter_mut()
                    .find_map(|prop| (prop.key == SPA_PROP_channelVolumes).then(|| &mut prop.value))
                {
                    return channel_volumes;
                }
            }
            panic!("We checked that the field existed and had the right type on serialization");
        }
}
