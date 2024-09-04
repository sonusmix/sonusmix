use std::io::{BufWriter, Cursor};

use pipewire::spa::pod::{deserialize::PodDeserializer, serialize::PodSerializer, Pod, Value};

use super::pod::NodeProps;


#[derive(Debug, Copy, Clone)]
pub(super) enum NodeAction {
    ChangeVolume(f32)
}

impl NodeAction {
    /// Apply the provided [NodeAction] on the Value.
    /// Returns None if the provided Value does not have the
    /// corresponding prop to change (eg. no volume prop for changing volume).
    ///
    /// A pod can be constructed with the provided bytes.
    ///
    /// Why is not just a Pod returned? Because a Pod cannot be owned as it's
    /// always referencing the bytes.
    // TODO: Try to return a Pod directly
    pub(super) fn apply(&self, value: Value) -> Option<(Vec<u8>, Value)> {
        let mut node_props = NodeProps::new(value);
        match *self {
            NodeAction::ChangeVolume(volume) => {
                let volume_channels = node_props.get_volumes()?.len();

                // create a new vec of channels based on the length of the count of the existing channels
                let mut new_channels = Vec::with_capacity(volume_channels);
                
                for _ in 1..volume_channels {
                    new_channels.push(volume)
                }

                node_props.set_volumes(new_channels);
            }
        };

        let value = node_props.value();
        let pod_bytes: Cursor<Vec<u8>> = Cursor::new(Vec::new());
        // write the bytes
        let (pod_bytes, _) = PodSerializer::serialize(pod_bytes, &value).expect("Unable to serialize NodeProps");
        let pod_bytes = pod_bytes.into_inner();

        Some((pod_bytes, value))
    }
}
