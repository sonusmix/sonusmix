use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    ops::BitAnd,
};

use itertools::Itertools;
use serde::{Deserialize, Serialize};
use ulid::Ulid;

use crate::pipewire_api::{Graph, Link as PwLink, Node as PwNode, PortKind, ToPipewireMessage};

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
    ///
    /// Basically, this means that every hardware endpoint that exists on both states is returned, while other
    /// hardware endpoints (only existing on the sonusmix state) are marked as a placeholder, so the UI can
    /// display the endpoint as deactivated.
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
                endpoint.volume_mixed = false;
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
                // check if the volume is mixed. An unlocked volume can be in both states.
                // A locked volume can not.
                for node in nodes {
                    if node.channel_volumes.is_empty() {
                        endpoint.volume_mixed = false;
                    } else {
                        let first = node.channel_volumes[0];
                        if node.channel_volumes.iter().all(|&x| x == first) {
                            endpoint.volume_mixed = false;
                        } else {
                            endpoint.volume_mixed = true;
                        }
                    }
                }
            }

            // If any messages were queued for this endpoint, mark its volume as having pending
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
        let (node_links, endpoint_links) = self.find_relevant_links(graph, endpoint_nodes);

        let mut messages = Vec::new();
        let mut to_remove_indices = Vec::new();
        for (i, link) in self.links.iter_mut().enumerate() {
            // If either of the link's endpoints cannot be resolved, skip this link. The
            // unresolvable endpoint is currently a placeholder and so it only exists in the
            // Sonusmix state, and so does the link
            let (Some(source), Some(sink)) = (
                endpoint_nodes.get(&link.start),
                endpoint_nodes.get(&link.end),
            ) else {
                continue;
            };

            // If either of the link's endpoints no longer exist, remove this link
            // TODO: We should never actually have this case. This should be handled by the update function.
            // if !self.endpoints.contains_key(&link.start) || !self.endpoints.contains_key(&link.end) {
            //     to_remove_indices.push(i);
            //     continue;
            // }

            // If the link has pending changes, simply check if the states match, and if so, remove
            // the pending marker
            if link.pending {
                if are_endpoints_connected(graph, source, sink, &node_links)
                    == link.state.is_connected()
                {
                    link.pending = false;
                }
                continue;
            }

            let num_messages_before = messages.len();

            match link.state {
                LinkState::PartiallyConnected => {
                    // Check if link should actually now be disconnected or fully connected
                    match are_endpoints_connected(graph, source, sink, &node_links) {
                        Some(true) => link.state = LinkState::ConnectedUnlocked,
                        Some(false) => to_remove_indices.push(i),
                        None => {}
                    }
                }
                LinkState::ConnectedUnlocked => {
                    // Check if all necessary links are still there, if not, change to partially
                    // connected or disconnected
                    match are_endpoints_connected(graph, source, sink, &node_links) {
                        Some(true) => {}
                        Some(false) => to_remove_indices.push(i),
                        None => link.state = LinkState::PartiallyConnected,
                    }
                }
                LinkState::ConnectedLocked => {
                    // Check if any necessary links are missing. If so, create them.
                    messages.extend(
                        source
                            .iter()
                            .cartesian_product(sink.iter())
                            .filter(|(source, sink)| {
                                are_nodes_connected(graph, source, sink, &node_links) != Some(true)
                            })
                            .map(|(source, sink)| {
                                // TODO: Maybe handle figuring out which exact ports to connect
                                // here instead of offloading it to the backend? Maybe that's
                                // unnecessary though.
                                ToPipewireMessage::CreateNodeLinks {
                                    start_id: source.id,
                                    end_id: sink.id,
                                }
                            }),
                    );
                }
                LinkState::DisconnectedLocked => {
                    // Check if any links exist. If so, remove them.
                    messages.extend(
                        source
                            .iter()
                            .cartesian_product(sink.iter())
                            .filter(|(source, sink)| {
                                are_nodes_connected(graph, source, sink, &node_links) != Some(false)
                            })
                            .map(|(source, sink)| {
                                // TODO: Maybe handle figuring out which exact ports to disconnect
                                // here instead of offloading it to the backend? Maybe that's
                                // unnecessary though.
                                ToPipewireMessage::RemoveNodeLinks {
                                    start_id: source.id,
                                    end_id: sink.id,
                                }
                            }),
                    );
                }
            }

            // If any messages were queued for this link, mark its volume as having pending
            // changes.
            if messages.len() > num_messages_before {
                link.pending = true;
            }
        }
        // Remove any links whose endpoints no longer exist. Iterate in reverse to preserve
        // the indices of the remaining elements to remove.
        for i in to_remove_indices.into_iter().rev() {
            self.links.swap_remove(i);
        }

        // Check if any links now exist between two endpoints that were not previously connected.
        // If so, mark those as partially connected or fully connected

        messages
    }

    /// Resolve an endpoint to a set of nodes in the Pipewire graph.
    ///
    /// Returns a list of [`PwNode`], which are present on both states.
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

    fn find_relevant_links<'a>(
        &self,
        graph: &'a Graph,
        endpoint_nodes: &HashMap<EndpointDescriptor, Vec<&'a PwNode>>,
    ) -> (
        HashMap<(u32, u32), Vec<&'a PwLink>>,
        HashMap<(EndpointDescriptor, EndpointDescriptor), Vec<&'a PwLink>>,
    ) {
        // TODO: Benchmark if hashmap or btreemap is faster here
        let mut node_links = HashMap::new();
        for link in graph.links.values() {
            node_links
                .entry((link.start_node, link.end_node))
                .or_insert_with(|| Vec::new())
                .push(link);
        }

        let endpoint_links = endpoint_nodes
            .iter()
            .filter(|(endpoint, _)| endpoint.is_kind(PortKind::Source))
            .cartesian_product(
                endpoint_nodes
                    .iter()
                    .filter(|(endpoint, _)| endpoint.is_kind(PortKind::Sink)),
            )
            .filter_map(|((source_desc, source_nodes), (sink_desc, sink_nodes))| {
                let links: Vec<&PwLink> = source_nodes
                    .iter()
                    .map(|node| node.id)
                    .cartesian_product(sink_nodes.iter().map(|node| node.id))
                    .filter_map(|ids| node_links.get(&ids))
                    .flat_map(|links| links.iter())
                    .copied()
                    .collect();
                (!links.is_empty()).then(|| ((*source_desc, *sink_desc), links))
            })
            .collect();

        (node_links, endpoint_links)
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

impl LinkState {
    fn is_locked(self) -> bool {
        match self {
            Self::ConnectedLocked | Self::DisconnectedLocked => true,
            _ => false,
        }
    }

    fn is_connected(self) -> Option<bool> {
        match self {
            Self::PartiallyConnected => None,
            Self::ConnectedUnlocked | Self::ConnectedLocked => Some(true),
            Self::DisconnectedLocked => Some(false),
        }
    }
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
            Self::MutedLocked | Self::UnmutedLocked => true,
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

impl EndpointDescriptor {
    fn is_kind(self, kind: PortKind) -> bool {
        match self {
            Self::GroupNode(_) => true,
            Self::EphemeralNode(_, kind_)
            | Self::PersistentNode(_, kind_)
            | Self::Application(_, kind_)
            | Self::Device(_, kind_) => kind_ == kind,
        }
    }
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

/// Returns whether the two node are connected. Some(bool) if the nodes are completely connected or
/// not, None if they are partially connected.
/// For now, "completely connected" means that every port on one of the nodes (either one) is
/// connected to a port on the other. "not connected" means there are no links from any port on one
/// node to any port on the other. "partially connected" is any other state.
fn are_nodes_connected(
    graph: &Graph,
    source: &PwNode,
    sink: &PwNode,
    node_links: &HashMap<(u32, u32), Vec<&PwLink>>,
) -> Option<bool> {
    // Find all links connected between the two nodes we're searching for
    let relevant_links = node_links
        .get(&(source.id, sink.id))
        .map(|links| links.as_slice())
        .unwrap_or(&[]);

    // If no links, nodes are not connected
    if relevant_links.is_empty() {
        return Some(false);
    }

    // If all of one node's ports are connected to by relevant links, nodes are completely connected
    // TODO: Maybe save ports as "source ports" and "sink ports" on the node. That might help with
    // being able to still connect to nodes that are temporarily missing
    if source
        .ports
        .iter()
        .filter(|id| {
            graph
                .ports
                .get(&id)
                .map(|port| port.kind == PortKind::Source)
                .unwrap_or(false)
        })
        .all(|id| relevant_links.iter().any(|link| link.start_port == *id))
        || sink
            .ports
            .iter()
            .filter(|id| {
                graph
                    .ports
                    .get(&id)
                    .map(|port| port.kind == PortKind::Sink)
                    .unwrap_or(false)
            })
            .all(|id| relevant_links.iter().any(|link| link.end_port == *id))
    {
        return Some(true);
    }

    // Otherwise, nodes are partially connected
    None
}

// Has similar semantics to [`are_nodes_connected`], but for endpoints. Not connected
// (`Some(false)`) means there are no links between the nodes making up the source endpoint and
// those making up the sink. Completely connected (`Some(true)`) means every node in the source
// endpoint is completely connected to every node in the sink (see [`are_nodes_connected`]).
// Partially connected (`None`) is any other state.
fn are_endpoints_connected(
    graph: &Graph,
    source: &[&PwNode],
    sink: &[&PwNode],
    node_links: &HashMap<(u32, u32), Vec<&PwLink>>,
) -> Option<bool> {
    let mut iter = source
        .iter()
        .cartesian_product(sink.iter())
        .map(|(source_node, sink_node)| {
            are_nodes_connected(graph, source_node, sink_node, node_links)
        });
    let first = iter.next()??;
    iter.all(|x| x == Some(first)).then_some(first)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipewire_api::object::*;

    /// Basic setup for a graph:
    ///
    /// 0 = client
    /// 1 = node (source)
    /// 2 = port (source)
    fn basic_graph_ephermal_node_setup() -> (Graph, SonusmixState) {
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
        let sonusmix_state = SonusmixState {
            active_sources: Vec::from([sonusmix_node]),
            active_sinks: Vec::new(),
            endpoints: HashMap::from([(sonusmix_node, sonusmix_node_endpoint); 1]),
            links: Vec::new(),
            applications: HashMap::new(),
            devices: HashMap::new(),
        };

        (pipewire_state, sonusmix_state)
    }

    #[test]
    fn diff_nodes_1() {
        let (pipewire_state, mut sonusmix_state) = basic_graph_ephermal_node_setup();

        let sonusmix_node = EndpointDescriptor::EphemeralNode(1, PortKind::Source);

        let endpoint_nodes = sonusmix_state.diff_nodes(&pipewire_state);

        // Node exists on pipewire state and sonusmix state.
        // Output has to include this node.
        assert!(endpoint_nodes.get(&sonusmix_node).is_some());

        // Should not be marked as placeholder
        let endpoint = sonusmix_state
            .endpoints
            .get(&sonusmix_node)
            .expect("Endpoint was removed from state");
        assert_eq!(endpoint.is_placeholder, false);
    }

    #[test]
    fn diff_nodes_2() {
        let (mut pipewire_state, mut sonusmix_state) = basic_graph_ephermal_node_setup();

        // remove the node from the pipewire state to emulate the node being
        // removed.
        pipewire_state.nodes.clear();
        pipewire_state.ports.clear();

        let sonusmix_node = EndpointDescriptor::EphemeralNode(1, PortKind::Source);

        let endpoint_nodes = sonusmix_state.diff_nodes(&pipewire_state);

        // Node only exists on sonusmix state.
        // Output should not include this node.
        assert!(endpoint_nodes.get(&sonusmix_node).is_none());

        // Should be marked as placeholder
        let endpoint = sonusmix_state
            .endpoints
            .get(&sonusmix_node)
            .expect("Endpoint was removed from state");
        assert_eq!(endpoint.is_placeholder, true);
    }

    /// Scenario: The user changes the volume of a node in the UI, which
    /// marks the volume on that node as pending. PipeWire however, returns a wrong
    /// volume. Ideally, a message with that volume should be sent again.
    // TODO: make it more reliable by checking if pipewire maybe
    // not accepted our volume, which means it will be set as
    // pending forever. This is basically a placeholder for
    // that rn.
    #[test]
    fn diff_properties_1() {
        let expected_volume = 0.2; // this is what the UI expects
        let got_volume = 0.125; // this is what it gets from pipewire

        let (mut pipewire_state, mut sonusmix_state) = basic_graph_ephermal_node_setup();

        // get the node
        let pipewire_node = pipewire_state
            .nodes
            .get_mut(&1)
            .expect("Could not find node. NOT AN ISSUE WITH DIFF.");
        // here we set the wrong volume
        pipewire_node.channel_volumes = Vec::from([got_volume]);
        let node_id = pipewire_node.id;

        let sonusmix_node = EndpointDescriptor::EphemeralNode(1, PortKind::Source);

        let endpoint = sonusmix_state
            .endpoints
            .get_mut(&sonusmix_node)
            .expect("Could not find node. NOT AN ISSUE WITH DIFF.");
        // this is the value set in the UI
        endpoint.volume = expected_volume;
        endpoint.volume_pending = true;

        // pipewire sent an update with the pipewire state...

        let endpoint_nodes = sonusmix_state.diff_nodes(&pipewire_state);

        // Node exists on pipewire state and sonusmix state.
        // Therefore, output has to include this node.
        assert!(endpoint_nodes.get(&sonusmix_node).is_some());

        // Compare properties.
        let pipewire_messages = sonusmix_state.diff_properties(&pipewire_state, &endpoint_nodes);

        assert!(pipewire_messages.is_empty());
    }

    /// Scenario: The user did not change the volume of the node in the UI,
    /// which means the volume should be applied to the Sonusmix state.
    #[test]
    fn diff_properties_2() {
        let current_volume = 0.2; // the current volume in the UI
        let new_volume = 0.125; // this is what it gets from pipewire

        let (mut pipewire_state, mut sonusmix_state) = basic_graph_ephermal_node_setup();

        // get the node
        let pipewire_node = pipewire_state
            .nodes
            .get_mut(&1)
            .expect("Could not find node. NOT AN ISSUE WITH DIFF.");
        // here we set the new volume
        pipewire_node.channel_volumes = Vec::from([new_volume]);

        let sonusmix_node = EndpointDescriptor::EphemeralNode(1, PortKind::Source);
        {
            let endpoint = sonusmix_state
                .endpoints
                .get_mut(&sonusmix_node)
                .expect("Could not find node. NOT AN ISSUE WITH DIFF.");
            // this is the value set in the UI
            endpoint.volume = current_volume;
            // the volume is not pending
            endpoint.volume_pending = false;
        }

        // pipewire sent an update with the pipewire state...

        let endpoint_nodes = sonusmix_state.diff_nodes(&pipewire_state);

        // Node exists on pipewire state and sonusmix state.
        // Therefore, output has to include this node.
        assert!(endpoint_nodes.get(&sonusmix_node).is_some());

        // Compare properties.
        let pipewire_messages = sonusmix_state.diff_properties(&pipewire_state, &endpoint_nodes);

        // message should be empty and sonusmix state should be updated
        assert!(pipewire_messages.is_empty());
        let endpoint = sonusmix_state
            .endpoints
            .get(&sonusmix_node)
            .expect("Could not find node. NOT AN ISSUE WITH DIFF.");
        assert_eq!(endpoint.volume, new_volume);
    }

    fn diff_properties_mixed_volume_unlocked_or_locked(locked: bool) {
        let current_volume = 0.2; // the current volume in the UI
        let new_volume = Vec::from([0.125, 0.225]); // this is what it gets from pipewire

        let (mut pipewire_state, mut sonusmix_state) = basic_graph_ephermal_node_setup();

        // get the node
        let pipewire_node = pipewire_state
            .nodes
            .get_mut(&1)
            .expect("Could not find node. NOT AN ISSUE WITH DIFF.");
        // here we set the new volume
        // the new volume is mixed
        pipewire_node.channel_volumes = new_volume.clone();

        let sonusmix_node = EndpointDescriptor::EphemeralNode(1, PortKind::Source);
        {
            let endpoint = sonusmix_state
                .endpoints
                .get_mut(&sonusmix_node)
                .expect("Could not find node. NOT AN ISSUE WITH DIFF.");
            // this is the value set in the UI
            endpoint.volume = current_volume;
            // the volume is not pending
            endpoint.volume_pending = false;
            // TODO: check muted
            if locked {
                endpoint.volume_locked_muted = VolumeLockMuteState::UnmutedLocked;
            } else {
                endpoint.volume_locked_muted = VolumeLockMuteState::UnmutedUnlocked;
            }
        }

        // pipewire sent an update with the pipewire state...

        let endpoint_nodes = sonusmix_state.diff_nodes(&pipewire_state);

        // Node exists on pipewire state and sonusmix state.
        // Therefore, output has to include this node.
        assert!(endpoint_nodes.get(&sonusmix_node).is_some());

        // Compare properties.
        let pipewire_messages = sonusmix_state.diff_properties(&pipewire_state, &endpoint_nodes);

        let endpoint = sonusmix_state
            .endpoints
            .get(&sonusmix_node)
            .expect("Could not find node. NOT AN ISSUE WITH DIFF.");

        if locked {
            // revert to the old value in this case
            assert_eq!(endpoint.volume, current_volume);
            // notify pipewire of this changed volume.
            // TODO: figure out why this is failing. The node id is 1
            // but the event is getting sent to 0 (the client).
            assert!(pipewire_messages.contains(&ToPipewireMessage::NodeVolume(
                1,
                Vec::from([current_volume, current_volume])
            )));
            // no pending needed since this will be done on every iteration.
            // However, for consistency it should be done anyways.
            assert!(endpoint.volume_pending);
        } else {
            // the sonusmix state should mark the volume as mixed.
            assert!(endpoint.volume_mixed);
            // create an average of both values
            assert_eq!(endpoint.volume, average_volumes(&new_volume));
            // message should be empty as there is nothing to do
            assert!(pipewire_messages.is_empty());
        }
    }

    #[test]
    fn diff_properties_mixed_volume_locked() {
        diff_properties_mixed_volume_unlocked_or_locked(true);
    }

    #[test]
    fn diff_properties_mixed_volume_unlocked() {
        diff_properties_mixed_volume_unlocked_or_locked(false);
    }
}
