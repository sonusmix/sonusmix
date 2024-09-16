mod reducer;

pub use reducer::SonusmixReducer;

use std::collections::{HashMap, HashSet};

use itertools::Itertools;
use serde::{Deserialize, Serialize};
use ulid::Ulid;

use crate::pipewire_api::{Graph, Link as PwLink, Node as PwNode, PortKind, ToPipewireMessage};

#[derive(Debug, Clone, Copy)]
pub enum SonusmixMsg {
    AddEphemeralNode(u32, PortKind),
    RemoveEndpoint(EndpointDescriptor),
    SetVolume(EndpointDescriptor, f32),
    SetMute(EndpointDescriptor, bool),
    SetVolumeLocked(EndpointDescriptor, bool),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SonusmixState {
    pub active_sources: Vec<EndpointDescriptor>,
    pub active_sinks: Vec<EndpointDescriptor>,
    pub endpoints: HashMap<EndpointDescriptor, Endpoint>,
    pub links: Vec<Link>,
    pub applications: HashMap<ApplicationId, Application>,
    pub devices: HashMap<DeviceId, Device>,
}

impl SonusmixState {
    /// Updates the Sonusmix state based on the incoming message, and generates Pipewire messages
    /// to reflect those changes.
    fn update(&mut self, graph: &Graph, message: SonusmixMsg) -> Vec<ToPipewireMessage> {
        match message {
            SonusmixMsg::AddEphemeralNode(id, kind) => {
                let Some(node) = graph.nodes.get(&id).filter(|node| {
                    // Verify the node has ports of the correct kind
                    node.ports.iter().any(|port_id| {
                        graph
                            .ports
                            .get(port_id)
                            .map(|port| port.kind == kind)
                            .unwrap_or(false)
                    })
                }) else {
                    return Vec::new();
                };

                let descriptor = EndpointDescriptor::EphemeralNode(id, kind);
                let endpoint = Endpoint::new(descriptor)
                    .with_display_name(node.identifier.human_name().to_owned())
                    .with_volume(
                        average_volumes(&node.channel_volumes),
                        !node.channel_volumes.iter().all_equal(),
                    )
                    .with_mute_unlocked(node.mute);

                self.endpoints.insert(descriptor, endpoint);
                match kind {
                    PortKind::Source => self.active_sources.push(descriptor),
                    PortKind::Sink => self.active_sinks.push(descriptor),
                }
                // TODO: Handle initializing groups when we add them

                Vec::new()
            }
            SonusmixMsg::RemoveEndpoint(endpoint_desc) => {
                let Some(endpoint) = self.endpoints.remove(&endpoint_desc) else {
                    // If the endpoint doesn't exist, exit
                    return Vec::new();
                };
                self.active_sources.retain(|endpoint| *endpoint != endpoint_desc);
                self.active_sinks.retain(|endpoint| *endpoint != endpoint_desc);

                // TODO: Handle cleanup specific to each endpoint type here. AFAIK the only type
                // that needs extra handling will be group nodes (i.e., remove the backing
                // Pipewire node)

                Vec::new()
            }
            SonusmixMsg::SetVolume(endpoint_desc, volume) => {
                // Resolve here instead of later so we don't have overlapping borrows
                let nodes = self.resolve_endpoint(endpoint_desc, graph);
                let Some(endpoint) = self.endpoints.get_mut(&endpoint_desc) else {
                    // If the endpoint doesn't exist, exit
                    return Vec::new();
                };
                endpoint.volume = volume;
                endpoint.volume_mixed = false;

                if let Some(nodes) = nodes {
                    // Set all channels on all nodes to the volume
                    let messages: Vec<ToPipewireMessage> = nodes
                        .into_iter()
                        .map(|node| {
                            ToPipewireMessage::NodeVolume(
                                node.id,
                                vec![volume; node.channel_volumes.len()],
                            )
                        })
                        .collect();
                    if !messages.is_empty() {
                        endpoint.volume_pending = true;
                    }
                    messages
                } else {
                    Vec::new()
                }
            }
            SonusmixMsg::SetMute(endpoint_desc, muted) => {
                // Resolve here instead of later so we don't have overlapping borrows
                let nodes = self.resolve_endpoint(endpoint_desc, graph);
                let Some(endpoint) = self.endpoints.get_mut(&endpoint_desc) else {
                    // If the endpoint doesn't exist, exit
                    return Vec::new();
                };
                endpoint.volume_locked_muted = endpoint.volume_locked_muted.with_mute(muted);

                if let Some(nodes) = nodes {
                    // Set all nodes to the mute state
                    let messages: Vec<ToPipewireMessage> = nodes
                        .into_iter()
                        .map(|node| ToPipewireMessage::NodeMute(node.id, muted))
                        .collect();
                    if !messages.is_empty() {
                        endpoint.volume_pending = true;
                    }
                    messages
                } else {
                    Vec::new()
                }
            }
            SonusmixMsg::SetVolumeLocked(endpoint_desc, locked) => {
                // Resolve here instead of later so we don't have overlapping borrows
                let nodes = self.resolve_endpoint(endpoint_desc, graph);
                let Some(endpoint) = self.endpoints.get_mut(&endpoint_desc) else {
                    // If the endpoint doesn't exist, exit
                    return Vec::new();
                };
                // If the lock state does not need to be changed, exit
                if endpoint.volume_locked_muted.is_locked() == locked {
                    return Vec::new();
                }

                if locked {
                    if let Some(volume_locked_muted) = endpoint.volume_locked_muted.lock() {
                        endpoint.volume_locked_muted = volume_locked_muted;
                    } else {
                        // Mixed mute states. Cannot lock the volume in this state, so exit
                        return Vec::new();
                    }

                    let Some(nodes) = nodes else {
                        // If the endpoint doesn't resolve then we have nothing else to do, so exit
                        return Vec::new();
                    };

                    // If the volume of all nodes equals the endpoint volume and there are no
                    // pending updates, then we're done, so exit
                    if !endpoint.volume_pending
                        && nodes
                            .iter()
                            .all(|node| node.channel_volumes.iter().all(|v| *v == endpoint.volume))
                    {
                        return Vec::new();
                    }

                    // Otherwise, change all of the volumes to the endpoint volume
                    endpoint.volume_mixed = false;
                    let messages: Vec<ToPipewireMessage> = nodes
                        .iter()
                        .map(|node| {
                            ToPipewireMessage::NodeVolume(
                                node.id,
                                vec![endpoint.volume; node.channel_volumes.len()],
                            )
                        })
                        .collect();
                    if messages.len() > 0 {
                        endpoint.volume_pending = true;
                    }
                    messages
                } else {
                    endpoint.volume_locked_muted = endpoint.volume_locked_muted.unlock();
                    // No more changes are needed.
                    Vec::new()
                }
            }
        }
    }

    // Diffs the Sonusmix state and the pipewire state. Returns a list of messages for Pipewire
    // to try and match the Sonusmix state as closely as possible, and marks any endpoints in the
    // Sonusmix state that cannot be found in the Pipewire graph as placeholders. This is only done
    // after updates from the backend.
    fn diff(&mut self, graph: &Graph) -> Vec<ToPipewireMessage> {
        let endpoint_nodes = self.diff_nodes(graph);
        let mut messages = self.diff_properties(graph, &endpoint_nodes);
        messages.extend(self.diff_links(graph, &endpoint_nodes));
        messages
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
                    endpoint.volume_mixed = volumes_mixed(&node.channel_volumes);
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
        let (node_links, mut remaining_endpoint_links) =
            self.find_relevant_links(graph, endpoint_nodes);

        let mut messages = Vec::new();
        let mut to_remove_indices = Vec::new();
        for (i, link) in self.links.iter_mut().enumerate() {
            // Remove the link from `remaining_endpoint_links` because it is now known to be in the
            // state
            remaining_endpoint_links.remove(&(link.start, link.end));

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

            // If any messages were queued for this link, mark it as having pending changes.
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
        // If so, mark those as partially connected or fully connected.
        for (source_desc, sink_desc) in remaining_endpoint_links {
            let (Some(source), Some(sink)) = (
                endpoint_nodes.get(&source_desc),
                endpoint_nodes.get(&sink_desc),
            ) else {
                continue;
            };
            match are_endpoints_connected(graph, source, sink, &node_links) {
                Some(true) => self.links.push(Link {
                    start: source_desc,
                    end: sink_desc,
                    state: LinkState::ConnectedUnlocked,
                    pending: false,
                }),
                None => self.links.push(Link {
                    start: source_desc,
                    end: sink_desc,
                    state: LinkState::PartiallyConnected,
                    pending: false,
                }),
                Some(false) => {}
            }
        }

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

    /// Find all of the Pipewire links between any two active endpoints and collect them into the
    /// returned data structures.
    fn find_relevant_links<'a>(
        &self,
        graph: &'a Graph,
        endpoint_nodes: &HashMap<EndpointDescriptor, Vec<&'a PwNode>>,
    ) -> (
        HashMap<(u32, u32), Vec<&'a PwLink>>,
        HashSet<(EndpointDescriptor, EndpointDescriptor)>,
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
            // For every combination of a source and a sink...
            .filter(|(endpoint, _)| endpoint.is_kind(PortKind::Source))
            .cartesian_product(
                endpoint_nodes
                    .iter()
                    .filter(|(endpoint, _)| endpoint.is_kind(PortKind::Sink)),
            )
            .filter_map(|((source_desc, source_nodes), (sink_desc, sink_nodes))| {
                source_nodes
                    .iter()
                    .map(|node| node.id)
                    // Record the pairs where any source node connects to any sink node
                    .cartesian_product(sink_nodes.iter().map(|node| node.id))
                    .any(|ids| node_links.contains_key(&ids))
                    .then_some((*source_desc, *sink_desc))
            })
            .collect();

        (node_links, endpoint_links)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Endpoint {
    pub descriptor: EndpointDescriptor,
    pub is_placeholder: bool,
    pub display_name: String,
    pub volume: f32,
    /// This will be true if all of the channels across all of the nodes this endpoint represents
    /// are not set to the same volume. This state is allowed, but the user will be notified of it,
    /// as it could cause unexpected behavior otherwise.
    pub volume_mixed: bool,
    pub volume_locked_muted: VolumeLockMuteState,
    pub volume_pending: bool,
}

impl Endpoint {
    fn new(descriptor: EndpointDescriptor) -> Self {
        Self {
            descriptor,
            is_placeholder: false,
            // String::new() does not allocate until data is added
            display_name: String::new(),
            volume: 0.0,
            volume_mixed: false,
            volume_locked_muted: VolumeLockMuteState::UnmutedUnlocked,
            volume_pending: false,
        }
    }

    fn with_display_name(mut self, display_name: String) -> Self {
        self.display_name = display_name;
        self
    }

    fn with_volume(mut self, volume: f32, volume_mixed: bool) -> Self {
        self.volume = volume;
        self.volume_mixed = volume_mixed;
        self
    }

    fn with_mute_unlocked(mut self, muted: bool) -> Self {
        self.volume_locked_muted = if muted {
            VolumeLockMuteState::MutedUnlocked
        } else {
            VolumeLockMuteState::UnmutedLocked
        };
        self
    }

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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
struct Link {
    start: EndpointDescriptor,
    end: EndpointDescriptor,
    state: LinkState,
    pending: bool,
}

impl Link {
    #[cfg(test)]
    pub fn new_test(start: EndpointDescriptor, end: EndpointDescriptor, state: LinkState) -> Link {
        Link {
            start,
            end,
            state,
            pending: false,
        }
    }
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

    fn with_mute(self, muted: bool) -> Self {
        match (muted, self) {
            (true, Self::MutedLocked | Self::UnmutedLocked) => Self::MutedLocked,
            (true, Self::MuteMixed | Self::MutedUnlocked | Self::UnmutedUnlocked) => {
                Self::MutedUnlocked
            }
            (false, Self::MutedLocked | Self::UnmutedLocked) => Self::UnmutedLocked,
            (false, Self::MuteMixed | Self::MutedUnlocked | Self::UnmutedUnlocked) => {
                Self::UnmutedUnlocked
            }
        }
    }

    fn lock(self) -> Option<Self> {
        match self {
            Self::MuteMixed => None,
            Self::MutedLocked | Self::MutedUnlocked => Some(Self::MutedLocked),
            Self::UnmutedLocked | Self::UnmutedUnlocked => Some(Self::UnmutedLocked),
        }
    }

    fn unlock(self) -> Self {
        match self {
            Self::MuteMixed => Self::MuteMixed,
            Self::MutedLocked | Self::MutedUnlocked => Self::MutedUnlocked,
            Self::UnmutedLocked | Self::UnmutedUnlocked => Self::UnmutedUnlocked,
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
pub enum EndpointDescriptor {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Application;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
struct DeviceId(Ulid);

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Device;

fn average_volumes<'a>(volumes: impl IntoIterator<Item = &'a f32>) -> f32 {
    let mut count: usize = 0;
    let mut total = 0.0;
    for volume in volumes {
        count += 1;
        total += volume.powf(1.0 / 3.0);
    }
    (total / count.max(1) as f32).powf(3.0)
}

fn volumes_mixed<'a>(volumes: impl IntoIterator<Item = &'a f32>) -> bool {
    let mut iterator = volumes.into_iter();
    let first = match iterator.next() {
        Some(first) => first,
        _ => return false
    };
    if iterator.all(|x| x == first) {
        false
    } else {
        true
    }
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
    use crate::pipewire_api::object::{Link, *};

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

    /// Advanced setup for a graph with links:
    ///
    /// - 1 node (source)
    ///   - 2 = client
    ///   - 3 port (source)
    /// - 2 node (sink)
    ///   - 4 = client
    ///   - 5 = port (sink)
    /// - 6 link ([`LinkState::ConnectedUnlocked`])
    fn advanced_graph_ephermal_node_setup() -> (Graph, SonusmixState) {
        let pipewire_state = {
            let mut source_node = Node::new_test(1, EndpointId::Client(2));
            source_node.ports = Vec::from([3]);
            let source_client = Client::new_test(2, false, Vec::from([1]));
            let source_port = Port::new_test(3, 1, PortKind::Source);

            let mut sink_node = Node::new_test(2, EndpointId::Client(4));
            sink_node.ports = Vec::from([5]);
            let sink_client = Client::new_test(4, false, Vec::from([2]));
            let sink_port = Port::new_test(5, 2, PortKind::Sink);

            let link = Link::new_test(
                6,
                source_node.id,
                source_port.id,
                sink_node.id,
                sink_port.id,
            );

            let mut nodes = HashMap::new();
            nodes.insert(1, source_node);
            nodes.insert(2, sink_node);

            let mut clients = HashMap::new();
            clients.insert(2, source_client);
            clients.insert(4, sink_client);

            let mut ports = HashMap::new();
            ports.insert(3, source_port);
            ports.insert(5, sink_port);

            let mut links = HashMap::new();
            links.insert(6, link);

            Graph {
                clients,
                devices: HashMap::new(),
                nodes,
                ports,
                links,
            }
        };

        let sonusmix_state = {
            let source_node = EndpointDescriptor::EphemeralNode(1, PortKind::Source);
            let source_endpoint = Endpoint::new_test(source_node);

            let sink_node = EndpointDescriptor::EphemeralNode(2, PortKind::Sink);
            let sink_endpoint = Endpoint::new_test(sink_node);

            let link = super::Link::new_test(source_node, sink_node, LinkState::ConnectedUnlocked);

            let mut active_sources = Vec::new();
            active_sources.push(source_node);

            let mut active_sinks = Vec::new();
            active_sinks.push(sink_node);

            let mut endpoints = HashMap::new();
            endpoints.insert(source_node, source_endpoint);
            endpoints.insert(sink_node, sink_endpoint);

            let mut links = Vec::new();
            links.push(link);

            SonusmixState {
                active_sources,
                active_sinks,
                endpoints,
                links,
                applications: HashMap::new(),
                devices: HashMap::new(),
            }
        };

        (pipewire_state, sonusmix_state)
    }

    #[test]
    fn diff_links_1() {
        let (pipewire_state, mut sonusmix_state) = advanced_graph_ephermal_node_setup();

        // fully connected
        assert_eq!(sonusmix_state.links[0].state.is_connected(), Some(true));

        let endpoint_nodes = sonusmix_state.diff_nodes(&pipewire_state);

        // messages should be empty as state is correct
        let messages = sonusmix_state.diff_links(&pipewire_state, &endpoint_nodes);
        assert!(messages.is_empty());
    }

    #[test]
    /// Event is coming from pipewire with no link.
    /// The sonusmix state should remove its link.
    fn diff_links_pipewire_remove_link_unlocked() {
        let (mut pipewire_state, mut sonusmix_state) = advanced_graph_ephermal_node_setup();

        let link = sonusmix_state.links[0];

        // fully connected
        assert_eq!(link.state.is_connected(), Some(true));

        // now we assume that these nodes are not anymore connected in pipewire
        pipewire_state.links.clear();

        let endpoint_nodes = sonusmix_state.diff_nodes(&pipewire_state);

        // Since the link is not locked, the sonusmix state should be updated
        assert!(!link.state.is_locked());
        let messages = sonusmix_state.diff_links(&pipewire_state, &endpoint_nodes);
        assert!(messages.is_empty());
        assert!(sonusmix_state.links.is_empty());
    }

    #[test]
    /// Event is coming from pipewire with no link.
    /// An event should be created to create that link again.
    fn diff_links_connected_locked() {
        let (mut pipewire_state, mut sonusmix_state) = advanced_graph_ephermal_node_setup();

        // fully connected
        assert_eq!(sonusmix_state.links[0].state.is_connected(), Some(true));

        // now we assume that these nodes are not anymore connected in pipewire
        pipewire_state.links.clear();
        // we also lock it
        sonusmix_state.links[0].state = LinkState::ConnectedLocked;

        let endpoint_nodes = sonusmix_state.diff_nodes(&pipewire_state);

        // since pipewire does not have the link anymore, it should be created again.
        let messages = sonusmix_state.diff_links(&pipewire_state, &endpoint_nodes);
        let expected_message = ToPipewireMessage::CreateNodeLinks {
            start_id: 1,
            end_id: 2,
        };
        assert!(messages.contains(&expected_message));
    }

    #[test]
    /// Event is coming from pipewire with a new link.
    /// An event should be created to remove that link.
    fn diff_links_disconnected_locked() {
        let (pipewire_state, mut sonusmix_state) = advanced_graph_ephermal_node_setup();

        // fully connected
        assert_eq!(sonusmix_state.links[0].state.is_connected(), Some(true));

        // pipewire has a link (pipewire_state.node[0]),
        // but the link should be disconnected.
        // we also lock it
        sonusmix_state.links[0].state = LinkState::DisconnectedLocked;

        let endpoint_nodes = sonusmix_state.diff_nodes(&pipewire_state);

        // since pipewire does not have the link anymore, it should be created again.
        let messages = sonusmix_state.diff_links(&pipewire_state, &endpoint_nodes);
        let expected_message = ToPipewireMessage::RemoveNodeLinks {
            start_id: 1,
            end_id: 2,
        };
        assert!(messages.contains(&expected_message));
    }

    #[test]
    /// Event is coming from pipewire with a new link.
    /// The link should be added to sonusmix.
    fn diff_links_new_pipewire_link() {
        let (pipewire_state, mut sonusmix_state) = advanced_graph_ephermal_node_setup();

        // fully connected
        assert_eq!(sonusmix_state.links[0].state.is_connected(), Some(true));

        let link_to_be_added = sonusmix_state.links[0];

        // pipewire has a link (pipewire_state.node[0]),
        // which is not yet in sonusmix.
        sonusmix_state.links.clear();

        let endpoint_nodes = sonusmix_state.diff_nodes(&pipewire_state);

        // since pipewire does not have the link anymore, it should be created again.
        let messages = sonusmix_state.diff_links(&pipewire_state, &endpoint_nodes);
        assert!(messages.is_empty());
        assert!(sonusmix_state.links.contains(&link_to_be_added));
    }

    #[test]
    fn find_relevant_links() {
        let (pipewire_state, mut sonusmix_state) = advanced_graph_ephermal_node_setup();
        let source_endpoint = EndpointDescriptor::EphemeralNode(1, PortKind::Source);
        let sink_endpoint = EndpointDescriptor::EphemeralNode(2, PortKind::Sink);

        let endpoint_nodes = sonusmix_state.diff_nodes(&pipewire_state);

        let link = pipewire_state.links.get(&6).expect("Link was destroyed");
        // These are the returns expected
        let expected_link = ((1, 2), link);
        // TODO: Is this correctly defined? What should be returned?
        let expected_link_endpoints = (source_endpoint, sink_endpoint);

        // find the relevant links
        let relevant_links = sonusmix_state.find_relevant_links(&pipewire_state, &endpoint_nodes);

        let returned_nodes = relevant_links
            .0
            .get(&expected_link.0)
            .expect("Link was not found in returned nodes");
        // there should only be a single link
        assert_eq!(returned_nodes.len(), 1);
        assert!(returned_nodes.contains(&expected_link.1));

        let returned_nodes_endpoints = relevant_links.1;
        // there should only be a single link
        assert_eq!(returned_nodes_endpoints.len(), 1);
        assert!(returned_nodes_endpoints.contains(&expected_link_endpoints));
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
    fn diff_nodes_placeholder() {
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
    /// volume.
    // TODO: make it more reliable by checking if pipewire maybe
    // not accepted our volume, which means it will be set as
    // pending forever. This is basically a placeholder for
    // that rn.
    #[test]
    fn diff_properties_wrong_volume_while_pending() {
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
    fn diff_properties_new_volume_pipewire() {
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
    fn volume_mixed() {
        assert_eq!(volumes_mixed(&[0.1, 0.12, 0.18]), true);
        assert_eq!(volumes_mixed(&[0.1, 0.1, 0.1]), false);
        assert_eq!(volumes_mixed(&[]), false);
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
