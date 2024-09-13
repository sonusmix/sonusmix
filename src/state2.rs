use std::{collections::HashMap, ops::BitAnd};

use serde::{Deserialize, Serialize};
use ulid::Ulid;

use crate::pipewire_api::{Graph, Node as PwNode, PortKind, ToPipewireMessage};

#[derive(Debug, Default, Serialize, Deserialize)]
struct SonusmixState {
    active_sources: Vec<EndpointDescriptor>,
    active_sinks: Vec<EndpointDescriptor>,
    endpoints: HashMap<EndpointDescriptor, Endpoint>,
    links: Vec<Link>,
    applications: HashMap<ApplicationId, Application>,
    devices: HashMap<DeviceId, Device>,
}

impl SonusmixState {
    // Diffs the Sonusmix state and the pipewire state. Returns a list of messages for Pipewire
    // to try and match the Sonusmix state as closely as possible, and marks any endpoints in the
    // Sonusmix state that cannot be found in the Pipewire graph as placeholders. This is only done
    // after updates from the backend.
    fn diff(&mut self, graph: &Graph) -> Vec<ToPipewireMessage> {
        let endpoint_nodes = self.diff_nodes(graph);
        let mut messages = self.diff_properties(graph, &endpoint_nodes);

        todo!()
    }

    /// Try to resolve each endpoint in the Sonusmix state to one or mode nodes in the Pipewire
    /// graph, and mark endpoints that could not be resolved as placeholders.
    fn diff_nodes<'a>(&mut self, graph: &'a Graph) -> HashMap<EndpointDescriptor, Vec<&'a PwNode>> {
        let mut endpoint_nodes = HashMap::new();
        // Keep a record of which endpoints are placeholders or not so that we're not mutating
        // while iterating
        let mut placeholders = Vec::new();
        for endpoint in self
            .active_sources
            .iter()
            .chain(self.active_sinks.iter())
            .copied()
        {
            if let Some(nodes) = self.resolve_endpoint(endpoint, graph) {
                endpoint_nodes.insert(endpoint, nodes);
                placeholders.push((endpoint, false));
            } else {
                placeholders.push((endpoint, true));
                // TODO: If the endpoint is a group node, tell the backend to create a virtual
                // device for it.
            }
        }
        for (endpoint, is_placeholder) in placeholders {
            if let Some(endpoint) = self.endpoints.get_mut(&endpoint) {
                endpoint.is_placeholder = is_placeholder;
            }
        }
        // TODO: Check if any of the leftover Pipewire nodes correspond to group nodes. If so, tell
        // the backend to remove them.

        endpoint_nodes
    }

    /// Check if the properties on the backend nodes match the Sonusmix endpoints, and change one
    /// or the other appropriately based on whether the endpoint is locked.
    fn diff_properties(
        &mut self,
        graph: &Graph,
        endpoint_nodes: &HashMap<EndpointDescriptor, Vec<&PwNode>>,
    ) -> Vec<ToPipewireMessage> {
        let mut messages = Vec::new();
        for (endpoint, nodes) in endpoint_nodes {
            let Some(endpoint) = self.endpoints.get_mut(endpoint) else {
                continue;
            };
            let num_messages_before = messages.len();
            // If the volume has pending changes, only check if the states match. If they do,
            // remove the pending marker.
            if endpoint.volume_pending {
                // Locked endpoints require that every channel on every node has the same volume.
                let volumes_match = if endpoint.volume_locked_muted.is_locked() {
                    nodes
                        .iter()
                        .flat_map(|node| &node.channel_volumes)
                        .all(|vol| *vol == endpoint.volume)
                } else {
                    // Unlocked endpoints are a little less strict, and only require that the
                    // average volume matches.
                    average_volumes(nodes.iter().flat_map(|node| &node.channel_volumes))
                        == endpoint.volume
                };
                let mute_states_match = endpoint.volume_locked_muted.is_muted()
                    == aggregate_bools(nodes.iter().map(|node| &node.mute));
                if volumes_match && mute_states_match {
                    endpoint.volume_pending = false;
                }

                // If the volume is locked, ensure all channels on all nodes are set to the endpoint
                // volume, and all the nodes' mute states are the same as the endpoint's. Otherwise,
                // make the endpoint's state match the average volume and mute state of the nodes.
            } else if endpoint.volume_locked_muted.is_locked() {
                // Tell any nodes that don't have all channels matching the endpoint volume to set
                // their channel volumes to the endpoint volume
                messages.extend(
                    nodes
                        .iter()
                        .filter(|node| node.channel_volumes.iter().any(|cv| *cv != endpoint.volume))
                        .map(|node| {
                            ToPipewireMessage::NodeVolume(
                                node.id,
                                vec![endpoint.volume; node.channel_volumes.len()],
                            )
                        }),
                );
                // Tell any nodes whose mute state doesn't match the endpoint's to change it
                let endpoint_muted = endpoint
                    .volume_locked_muted
                    .is_muted()
                    .expect("mute should not be mixed as we know it's locked");
                messages.extend(
                    nodes
                        .iter()
                        .filter(|node| node.mute != endpoint_muted)
                        .map(|node| ToPipewireMessage::NodeMute(node.id, endpoint_muted)),
                );
            } else {
                endpoint.volume_locked_muted =
                    VolumeLockMuteState::from_bools_unlocked(nodes.iter().map(|node| &node.mute));
                endpoint.volume =
                    average_volumes(nodes.iter().flat_map(|node| &node.channel_volumes));
            }

            // If any messages were queued from this endpoint, mark its volume as having pending
            // changes.
            if messages.len() > num_messages_before {
                endpoint.volume_pending = true;
            }
        }

        messages
    }

    fn diff_links(
        &mut self,
        graph: &Graph,
        endpoint_nodes: &HashMap<EndpointDescriptor, Vec<&PwNode>>,
    ) -> Vec<ToPipewireMessage> {
        todo!()
    }

    /// Resolve an endpoint to a set of nodes in the Pipewire graph.
    fn resolve_endpoint<'a>(
        &self,
        endpoint: EndpointDescriptor,
        graph: &'a Graph,
    ) -> Option<Vec<&'a PwNode>> {
        match endpoint {
            EndpointDescriptor::EphemeralNode(id, kind) => {
                // Check if a node with this ID exists, and if so, that it has ports in the
                // specified direction.
                graph
                    .nodes
                    .get(&id)
                    .filter(|node| {
                        node.ports.iter().any(|port_id| {
                            graph
                                .ports
                                .get(port_id)
                                .map(|port| port.kind == kind)
                                .unwrap_or(false)
                        })
                    })
                    .map(|node| vec![node])
            }
            EndpointDescriptor::PersistentNode(id, kind) => todo!(),
            EndpointDescriptor::GroupNode(id) => todo!(),
            EndpointDescriptor::Application(id, kind) => todo!(),
            EndpointDescriptor::Device(id, kind) => todo!(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Endpoint {
    descriptor: EndpointDescriptor,
    is_placeholder: bool,
    display_name: String,
    volume: f32,
    /// This will be true if all of the channels across all of the nodes this endpoint represents
    /// are not set to the same volume. This state is allowed, but the user will be notified of it,
    /// as it could cause unexpected behavior otherwise.
    volume_mixed: bool,
    volume_locked_muted: VolumeLockMuteState,
    volume_pending: bool,
}

impl Endpoint {
    #[cfg(test)]
    pub fn new_test(descriptor: EndpointDescriptor) -> Self {
        Endpoint {
            descriptor,
            is_placeholder: false,
            display_name: "TESTING ENDPOINT".to_string(),
            volume: 0.0,
            volume_mixed: false,
            volume_locked_muted: VolumeLockMuteState::UnmutedUnlocked,
            volume_pending: false,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
struct Link {
    start: EndpointDescriptor,
    end: EndpointDescriptor,
    state: LinkState,
    pending: bool,
}

/// Describes the state of the links between two endpoints. There is no "DisconnectedUnlocked"
/// state, a link in that state will simply not be recorded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum LinkState {
    /// Two endpoints have some links between them, but not all matching nodes/ports are connected.
    /// A user may not lock a link to this state (for now).
    PartiallyConnected,
    /// Two endpoints have the matching links between them for Sonusmix to consider them to be
    /// connected, but Sonusmix will not attempt to restore this state if something outside of
    /// Sonusmix changes it.
    ConnectedUnlocked,
    /// Sonusmix will ensure that these two endpoints have matching links between them, and will
    /// restore this state if something outside of Sonusmix changes it.
    ConnectedLocked,
    /// Sonusmix will remove any links between these two endpoints.
    DisconnectedLocked,
}

/// This enum encodes the possibility that if an endpoint represents multiple Pipewire nodes, some
/// may be muted while others are not. A user may not input this state, and also may not lock an
/// endpoint's volume in this state: In order to lock the volume of an endpoint, the backend nodes
/// it represents must all be muted or all be unmuted. They may have different volumes, though;
/// they will all be set to the overall average volume when they are locked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum VolumeLockMuteState {
    /// Some of the nodes this endpoint represents are muted, and some are not. A user may not
    /// input this state, and may not lock the volume in this state.
    MuteMixed,
    /// The endpoint is muted and the volume is locked.
    MutedLocked,
    /// The endpoint is muted and the volume is unlocked.
    MutedUnlocked,
    /// The endpoint is unmuted and the volume is locked.
    UnmutedLocked,
    /// The endpoint is unmuted and the volume is unlocked.
    UnmutedUnlocked,
}

impl VolumeLockMuteState {
    fn is_locked(self) -> bool {
        match self {
            Self::MutedLocked | Self::MutedUnlocked => true,
            _ => false,
        }
    }

    fn is_muted(self) -> Option<bool> {
        match self {
            Self::MuteMixed => None,
            Self::MutedLocked | Self::MutedUnlocked => Some(true),
            Self::UnmutedLocked | Self::UnmutedUnlocked => Some(false),
        }
    }

    /// Calculate the resulting state from multiple channel mute states, assuming the volume is
    /// unlocked.
    fn from_bools_unlocked<'a>(bools: impl IntoIterator<Item = &'a bool>) -> Self {
        match aggregate_bools(bools) {
            Some(true) => Self::MutedUnlocked,
            Some(false) => Self::UnmutedUnlocked,
            None => Self::MuteMixed,
        }
    }
}

/// Represents anything that can have audio routed to or from it in Sonusmix. This might be a
/// single node, a group, or all sources or sinks belonging to an application or device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
enum EndpointDescriptor {
    /// Represents a single node that has no way to identify itself from others in the same
    /// application or device other than its ID. These can be selected once they are created, but
    /// cannot be persisted across Pipewire restarts. Theoretically they might be able to
    /// persist across Sonusmix restarts, though. If they cannot be restored, it might be
    /// possible to leave a placeholder for the user to fill in.
    EphemeralNode(u32, PortKind),
    /// Represents a single node that can be identified by its name, path, or some other
    /// property. These can be (hopefully) easily persisted, and are not reliant on their
    /// Pipewire node ID for identification.
    PersistentNode(PersistentNodeId, PortKind),
    /// Represents a single node created and managed by Sonusmix. These only exist while
    /// Sonusmix is running, so they are identified using their Pipewire node ID, but can be
    /// persisted across Sonusmix and even Pipewire restarts.
    GroupNode(GroupNodeId),
    /// Represents all sources or sinks (except those that are explicitly excluded) belonging to a
    /// particular application. These will be managed and routed together.
    Application(ApplicationId, PortKind),
    /// Represents all sources or sinks (except those that are explicitly excluded) belonging to a
    /// particular device. These will be managed and routed together.
    Device(DeviceId, PortKind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
struct PersistentNodeId(Ulid);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
struct GroupNodeId(Ulid);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
struct ApplicationId(Ulid);

#[derive(Debug, Serialize, Deserialize)]
struct Application;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
struct DeviceId(Ulid);

#[derive(Debug, Serialize, Deserialize)]
struct Device;

mod reducer {
    use std::sync::{mpsc, Arc};

    use relm4::SharedState;

    use crate::pipewire_api::{PortKind, ToPipewireMessage};

    use super::{EndpointDescriptor, SonusmixState};

    #[derive(Debug, Clone)]
    pub enum SonusmixMsg {
        AddEndpoint(EndpointDescriptor, PortKind),
        RemoveEndpoint(EndpointDescriptor, PortKind),
    }

    pub struct SonusmixReducer {
        pw_sender: mpsc::Sender<ToPipewireMessage>,
        messages: SharedState<Option<SonusmixMsg>>,
        state: SharedState<Arc<SonusmixState>>,
    }

    impl SonusmixReducer {
        // pub fn emit(&self, msg: SonusmixMsg) {
        //     let mut state = { self.state.read().as_ref().clone() };
        // }

        // pub fn subscribe_msg<Msg, F>(&self, sender: &relm4::Sender<Msg>, f: F) -> Arc<SonusmixState>
        // where
        //     F: Fn(&SonusmixMsg) -> Msg + 'static + Send + Sync,
        //     Msg: Send + 'static,
        // {
        //     self.messages
        //         .subscribe_optional(sender, move |msg| msg.as_ref().map(&f));
        //     self.state.read().clone()
        // }

        // pub fn subscribe<Msg, F>(&self, sender: &relm4::Sender<Msg>, f: F) -> Arc<SonusmixState>
        // where
        //     F: Fn(Arc<SonusmixState>) -> Msg + 'static + Send + Sync,
        //     Msg: Send + 'static,
        // {
        //     self.messages
        //         .subscribe(sender, move |state| f(state.clone()));
        //     self.state.read().clone()
        // }
    }
}

fn average_volumes<'a>(volumes: impl IntoIterator<Item = &'a f32>) -> f32 {
    let mut count: usize = 0;
    let mut total = 0.0;
    for volume in volumes {
        count += 1;
        total += volume.powf(1.0 / 3.0);
    }
    (total / count.max(1) as f32).powf(3.0)
}

/// Aggregate an iterator of booleans into an `Option<bool>`. Some if all booleans are the same,
/// None if any are different from each other.
fn aggregate_bools<'a>(bools: impl IntoIterator<Item = &'a bool>) -> Option<bool> {
    let mut iter = bools.into_iter();
    let Some(first) = iter.next() else {
        return None;
    };
    iter.all(|b| b == first).then_some(*first)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipewire_api::object::*;

    #[test]
    fn diff_nodes_1() {
        // 0 = client
        // 1 = node (source)
        // 2 = port (source)
        let client_of_node = Client::new_test(0, false, Vec::from([1]));
        let port_of_node = Port::new_test(2, 1, PortKind::Source);
        let mut pipewire_node = Node::new_test(1, EndpointId::Client(0));

        pipewire_node.ports = Vec::from([2]);

        let pipewire_state = Graph {
            clients: HashMap::from([(0, client_of_node); 1]),
            devices: HashMap::new(),
            nodes: HashMap::from([(1, pipewire_node); 1]),
            ports: HashMap::from([(2, port_of_node); 1]),
            links: HashMap::new(),
        };

        let sonusmix_node = EndpointDescriptor::EphemeralNode(1, PortKind::Source);
        let sonusmix_node_endpoint = Endpoint::new_test(sonusmix_node);
        let mut sonusmix_state = SonusmixState {
            active_sources: Vec::from([sonusmix_node]),
            active_sinks: Vec::new(),
            endpoints: HashMap::from([(sonusmix_node, sonusmix_node_endpoint); 1]),
            links: Vec::new(),
            applications: HashMap::new(),
            devices: HashMap::new(),
        };

        let endpoint_nodes = sonusmix_state.diff_nodes(&pipewire_state);

        // Node exists on pipewire state and sonusmix state.
        // If the properties match would be checked next.
        // Therefore, output has to include this node.
        assert!(endpoint_nodes.get(&sonusmix_node).is_some());

        // Should not be marked as placeholder
        let endpoint = sonusmix_state
            .endpoints
            .get(&sonusmix_node)
            .expect("Endpoint was removed from state");
        assert_eq!(endpoint.is_placeholder, false);
    }
}
