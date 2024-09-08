use std::io::Cursor;

use anyhow::Result;
use pipewire::spa::{
    param::ParamType,
    pod::{object, serialize::PodSerializer, Property, Value, ValueArray},
    sys::SPA_PROP_channelVolumes,
    utils::SpaTypes,
};

use super::object::Node;

#[derive(Debug, Copy, Clone)]
pub(super) enum NodeAction {
    ChangeVolume(f32),
}

impl NodeAction {
    /// Create a new Pod based on the provided [NodeAction].
    ///
    /// A pod can be constructed with the provided bytes.
    ///
    /// Why is not just a Pod returned? Because a Pod cannot be owned as it's
    /// always referencing the bytes.
    // TODO: Try to return a Pod directly
    pub(super) fn apply(&self, node: &Node) -> Result<Vec<u8>> {
        let value = match *self {
            NodeAction::ChangeVolume(volume) => {
                let num_volume_channels = node.channel_volumes.len();

                // assume that if a node has no volume channels, it does not
                // support changing the volume.
                if num_volume_channels == 0 {
                    return Err(anyhow::Error::msg("The Node has no volume channels"));
                }

                // create a new vec of channels based on the length of the count of the existing channels
                let mut new_channels = Vec::with_capacity(num_volume_channels);

                for _ in 0..num_volume_channels {
                    new_channels.push(volume)
                }

                Node::volume_channels_value(new_channels)
            }
        };

        let pod_bytes: Cursor<Vec<u8>> = Cursor::new(Vec::new());
        // write the bytes
        let (pod_bytes, _) =
            PodSerializer::serialize(pod_bytes, &value).expect("Unable to serialize NodeProps");
        let pod_bytes = pod_bytes.into_inner();

        Ok(pod_bytes)
    }
}
