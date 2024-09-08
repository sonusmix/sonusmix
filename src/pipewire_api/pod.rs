use pipewire::spa::{
    pod::{Value, ValueArray},
    sys::SPA_PROP_channelVolumes,
};

#[derive(Debug)]
pub(super) struct NodeProps {
    value: Value,
}

impl NodeProps {
    pub fn new(value: Value) -> Self {
        NodeProps { value }
    }

    pub fn get_channel_volumes(&self) -> Option<&[f32]> {
        if let Value::Object(object) = &self.value {
            if let Some(Value::ValueArray(ValueArray::Float(channel_volumes))) = object
                .properties
                .iter()
                .find_map(|prop| (prop.key == SPA_PROP_channelVolumes).then(|| &prop.value))
            {
                return Some(channel_volumes);
            }
        }
        return None;
    }
}
