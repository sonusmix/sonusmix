mod persistence;
mod reducer;

use log::{debug, error, warn};
pub use reducer::SonusmixReducer;

use std::collections::{HashMap, HashSet};

use itertools::Itertools;
use serde::{Deserialize, Serialize};
use ulid::Ulid;

use crate::pipewire_api::{
    Graph, Link as PwLink, Node as PwNode, NodeIdentifier, PortKind, ToPipewireMessage,
};

#[derive(Debug, Clone)]
pub enum SonusmixMsg {
    AddEphemeralNode(u32, PortKind),
    AddApplication(ApplicationId, PortKind),
    AddGroupNode(String, GroupNodeKind),
    RemoveEndpoint(EndpointDescriptor),
    SetVolume(EndpointDescriptor, f32),
    SetMute(EndpointDescriptor, bool),
    SetVolumeLocked(EndpointDescriptor, bool),
    /// If the parameter is None, then reset the name
    RenameEndpoint(EndpointDescriptor, Option<String>),
    Link(EndpointDescriptor, EndpointDescriptor),
    RemoveLink(EndpointDescriptor, EndpointDescriptor),
    SetLinkLocked(EndpointDescriptor, EndpointDescriptor, bool),
}

#[derive(Debug, Clone)]
pub enum SonusmixOutputMsg {
    EndpointAdded(EndpointDescriptor),
    EndpointRemoved(EndpointDescriptor),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SonusmixState {
    pub active_sources: Vec<EndpointDescriptor>,
    pub active_sinks: Vec<EndpointDescriptor>,
    pub endpoints: HashMap<EndpointDescriptor, Endpoint>,
    pub candidates: Vec<(u32, PortKind, NodeIdentifier)>,
    pub links: Vec<Link>,
    /// Stores data for matching Pipewire nodes to persistent nodes. Entries are added to this map
    /// when new nodes are detected and added to the pipewire
    pub persistent_nodes: HashMap<PersistentNodeId, (NodeIdentifier, PortKind)>,
    pub applications: HashMap<ApplicationId, Application>,
    pub devices: HashMap<DeviceId, Device>,
    pub group_nodes: HashMap<GroupNodeId, GroupNode>,
}

impl SonusmixState {
    /// Updates the Sonusmix state based on the incoming message, and generates Pipewire messages
    /// to reflect those changes.
    fn update(
        &mut self,
        graph: &Graph,
        message: SonusmixMsg,
    ) -> (Option<SonusmixOutputMsg>, Vec<ToPipewireMessage>) {
        let mut pipewire_messages = Vec::new();
        let output_message = 'handler: {
            match message {
                SonusmixMsg::AddEphemeralNode(id, kind) => {
                    let Some(node) = graph.nodes.get(&id).filter(|node| node.has_port_kind(kind))
                    else {
                        break 'handler None;
                    };

                    let descriptor = EndpointDescriptor::EphemeralNode(id, kind);

                    self.candidates
                        .retain(|(cand_id, cand_kind, _)| *cand_id != id || *cand_kind != kind);

                    let endpoint = Endpoint::new(descriptor)
                        .with_display_name(node.identifier.human_name().to_owned())
                        .with_icon_name(node.identifier.icon_name().to_string())
                        .with_details(
                            node.identifier
                                .details()
                                .map(ToOwned::to_owned)
                                .into_iter()
                                .collect(),
                        )
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

                    // If the node matches an existing application, add it as an exception
                    if let Some(application) = self.applications.values_mut().find(|application| {
                        application.is_active && application.matches(&node.identifier, kind)
                    }) {
                        application.exceptions.push(descriptor);
                    }

                    Some(SonusmixOutputMsg::EndpointAdded(descriptor))
                }
                SonusmixMsg::AddGroupNode(name, kind) => {
                    let id = GroupNodeId::new();
                    let descriptor = EndpointDescriptor::GroupNode(id);
                    self.group_nodes.insert(
                        id,
                        GroupNode {
                            id,
                            kind,
                            pending: true,
                        },
                    );
                    self.endpoints.insert(
                        descriptor,
                        Endpoint::new(descriptor).with_display_name(name.clone()),
                    );
                    pipewire_messages.push(ToPipewireMessage::CreateGroupNode(name, id.0, kind));
                    Some(SonusmixOutputMsg::EndpointAdded(descriptor))
                }
                SonusmixMsg::AddApplication(id, kind) => {
                    let Some(mut application) = self.applications.get(&id).cloned() else {
                        // If the application doesn't exist, exit
                        error!("Cannot add application {id:?} as it does not exist in the state");
                        break 'handler None;
                    };

                    let descriptor = EndpointDescriptor::Application(id, kind);

                    application.is_active = true;
                    match kind {
                        PortKind::Source => self.active_sources.push(descriptor),
                        PortKind::Sink => self.active_sinks.push(descriptor),
                    }

                    // Add any existing matching endpoints as exceptions
                    application.exceptions = self
                        .active_sources
                        .iter()
                        .chain(self.active_sinks.iter())
                        .copied()
                        .filter(|endpoint| match endpoint {
                            EndpointDescriptor::EphemeralNode(..)
                            | EndpointDescriptor::PersistentNode(..) => self
                                .resolve_endpoint(*endpoint, graph)
                                .into_iter()
                                .flatten()
                                .any(|node| application.matches(&node.identifier, kind)),
                            _ => false,
                        })
                        .collect();

                    // Put the modified application back into the map
                    self.applications.insert(id, application.clone());

                    // Add the endpoint. Properties will be handled by running a diff immediately after
                    // the update
                    self.endpoints.insert(
                        descriptor,
                        Endpoint::new(descriptor)
                            .with_display_name(application.name_with_tag())
                            .with_icon_name(application.icon_name),
                    );

                    Some(SonusmixOutputMsg::EndpointAdded(descriptor))
                }
                SonusmixMsg::RemoveEndpoint(endpoint_desc) => {
                    if self.endpoints.remove(&endpoint_desc).is_none() {
                        error!("Cannot remove endpoint {endpoint_desc:?} as it does not exist");
                        break 'handler None;
                    }

                    self.active_sources
                        .retain(|endpoint| *endpoint != endpoint_desc);
                    self.active_sinks
                        .retain(|endpoint| *endpoint != endpoint_desc);

                    // Remove the endpoint from any applications that might have it as an exception
                    for application in self.applications.values_mut() {
                        application
                            .exceptions
                            .retain(|endpoint| *endpoint != endpoint_desc);
                    }

                    // Handle cleanup specific to each endpoint type
                    match endpoint_desc {
                        EndpointDescriptor::EphemeralNode(id, kind) => {
                            let Some(node) = graph.nodes.get(&id) else {
                                break 'handler None;
                            };
                            self.candidates.push((id, kind, node.identifier.clone()));
                        }
                        EndpointDescriptor::GroupNode(id) => {
                            if self.group_nodes.remove(&id).is_none() {
                                warn!("Group node {id:?} did not exist in the state");
                            };
                            pipewire_messages.push(ToPipewireMessage::RemoveGroupNode(id.0));
                        }
                        EndpointDescriptor::Application(id, _kind) => {
                            // If there are still matching nodes, simply mark the application as
                            // inactive. Otherwise, remove it.
                            if self.resolve_endpoint(endpoint_desc, graph).is_some() {
                                if let Some(application) = self.applications.get_mut(&id) {
                                    application.is_active = false;
                                } else {
                                    error!("Application {id:?} did not exist in the state");
                                }
                            } else {
                                self.applications.remove(&id);
                            }
                        }
                        _ => todo!(),
                    }

                    Some(SonusmixOutputMsg::EndpointRemoved(endpoint_desc))
                }
                SonusmixMsg::SetVolume(endpoint_desc, volume) => {
                    // Resolve here instead of later so we don't have overlapping borrows
                    let nodes = self.resolve_endpoint(endpoint_desc, graph);
                    let Some(endpoint) = self.endpoints.get_mut(&endpoint_desc) else {
                        // If the endpoint doesn't exist, exit
                        break 'handler None;
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
                        pipewire_messages.extend(messages);
                    }

                    None
                }
                SonusmixMsg::SetMute(endpoint_desc, muted) => {
                    // Resolve here instead of later so we don't have overlapping borrows
                    let nodes = self.resolve_endpoint(endpoint_desc, graph);
                    let Some(endpoint) = self.endpoints.get_mut(&endpoint_desc) else {
                        // If the endpoint doesn't exist, exit
                        break 'handler None;
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
                        pipewire_messages.extend(messages);
                    }

                    None
                }
                SonusmixMsg::SetVolumeLocked(endpoint_desc, locked) => {
                    // Resolve here instead of later so we don't have overlapping borrows
                    let nodes = self.resolve_endpoint(endpoint_desc, graph);
                    let Some(endpoint) = self.endpoints.get_mut(&endpoint_desc) else {
                        // If the endpoint doesn't exist, exit
                        break 'handler None;
                    };
                    // If the lock state does not need to be changed, exit
                    if endpoint.volume_locked_muted.is_locked() == locked {
                        break 'handler None;
                    }

                    if locked {
                        if let Some(volume_locked_muted) = endpoint.volume_locked_muted.lock() {
                            endpoint.volume_locked_muted = volume_locked_muted;
                        } else {
                            // Mixed mute states. Cannot lock the volume in this state, so exit
                            break 'handler None;
                        }

                        let Some(nodes) = nodes else {
                            // If the endpoint doesn't resolve then we have nothing else to do, so exit
                            break 'handler None;
                        };

                        // If the volume of all nodes equals the endpoint volume and there are no
                        // pending updates, then we're done, so exit
                        if !endpoint.volume_pending
                            && nodes.iter().all(|node| {
                                node.channel_volumes.iter().all(|v| *v == endpoint.volume)
                            })
                        {
                            break 'handler None;
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
                        if !messages.is_empty() {
                            endpoint.volume_pending = true;
                        }
                        pipewire_messages.extend(messages);
                    } else {
                        endpoint.volume_locked_muted = endpoint.volume_locked_muted.unlock();
                        // No more changes are needed.
                    }

                    None
                }
                SonusmixMsg::Link(source, sink) => {
                    if !source.is_kind(PortKind::Source) || !sink.is_kind(PortKind::Sink) {
                        error!("Cannot link {source:?} to {sink:?}, link may be backwards");
                        break 'handler None;
                    }

                    // If either of these is None, then the loop will iterate 0 times
                    let source_nodes = self.resolve_endpoint(source, graph).unwrap_or_default();
                    let sink_nodes = self.resolve_endpoint(sink, graph).unwrap_or_default();

                    let mut messages: Vec<ToPipewireMessage> = Vec::new();
                    for source in &source_nodes {
                        for sink in &sink_nodes {
                            messages.push(ToPipewireMessage::CreateNodeLinks {
                                start_id: source.id,
                                end_id: sink.id,
                            })
                        }
                    }

                    if let Some(link) = self
                        .links
                        .iter_mut()
                        .find(|link| link.start == source && link.end == sink)
                    {
                        // If the link already exists in the state, update it
                        match link.state {
                            LinkState::PartiallyConnected => {
                                link.state = LinkState::ConnectedUnlocked
                            }
                            LinkState::DisconnectedLocked => {
                                link.state = LinkState::ConnectedLocked
                            }
                            _ => {}
                        }
                        if !messages.is_empty() {
                            link.pending = true;
                        }
                    } else {
                        // Otherwise, add it to the state
                        self.links.push(Link {
                            start: source,
                            end: sink,
                            state: LinkState::ConnectedUnlocked,
                            pending: !messages.is_empty(),
                        });
                    }

                    pipewire_messages.extend(messages);
                    None
                }
                SonusmixMsg::RemoveLink(source, sink) => {
                    if !source.is_kind(PortKind::Source) || !sink.is_kind(PortKind::Sink) {
                        error!("Cannot link {source:?} to {sink:?}, link may be backwards");
                        break 'handler None;
                    }

                    let Some(link_position) = self
                        .links
                        .iter_mut()
                        .position(|link| link.start == source && link.end == sink)
                    else {
                        error!("Cannot remove link as it does not exist");
                        break 'handler None;
                    };

                    match self.links[link_position].state {
                        LinkState::PartiallyConnected | LinkState::ConnectedUnlocked => {
                            // If the link is unlocked, it gets removed entirely
                            self.links.swap_remove(link_position);
                            pipewire_messages
                                .extend(self.remove_pipewire_node_links(graph, source, sink));
                        }
                        LinkState::ConnectedLocked => {
                            // If it is locked, it gets changed to DisconnectedLocked
                            self.links[link_position].state = LinkState::DisconnectedLocked;
                            let messages = self.remove_pipewire_node_links(graph, source, sink);
                            if !messages.is_empty() {
                                self.links[link_position].pending = true;
                            }
                            pipewire_messages.extend(messages);
                        }
                        // If it is locked and disconnected, we don't need to do anything
                        LinkState::DisconnectedLocked => {}
                    }

                    None
                }
                SonusmixMsg::SetLinkLocked(source, sink, locked) => {
                    if !source.is_kind(PortKind::Source) || !sink.is_kind(PortKind::Sink) {
                        error!("Cannot link {source:?} to {sink:?}, link may be backwards");
                        break 'handler None;
                    }

                    let link_position = self
                        .links
                        .iter_mut()
                        .position(|link| link.start == source && link.end == sink);

                    match (
                        link_position.map(|idx| (idx, self.links[idx].state)),
                        locked,
                    ) {
                        (Some((_idx, LinkState::PartiallyConnected)), true) => {
                            // If trying to lock in any way while partially connected.
                            // Should be handled by the UI (to show the user not being able to lock).
                            error!("Cannot lock partially connected link");
                        }
                        (Some((idx, LinkState::ConnectedUnlocked)), true) => {
                            self.links[idx].state = LinkState::ConnectedLocked
                        }
                        (None, true) => {
                            // Link is disconnected and unlocked, make a new one that's disconnected
                            // and locked
                            self.links.push(Link {
                                start: source,
                                end: sink,
                                state: LinkState::DisconnectedLocked,
                                pending: false,
                            });
                        }
                        // The other cases are locked already and don't need to be changed
                        (_, true) => {}

                        (Some((idx, LinkState::ConnectedLocked)), false) => {
                            self.links[idx].state = LinkState::ConnectedUnlocked
                        }
                        (Some((idx, LinkState::DisconnectedLocked)), false) => {
                            // Link is disconnected and locked, unlocking it means just removing it
                            // from the state
                            self.links.swap_remove(idx);
                        }
                        // The other cases are unlocked already and don't need to be changed
                        (_, false) => {}
                    };

                    None
                }
                SonusmixMsg::RenameEndpoint(
                    descriptor @ EndpointDescriptor::GroupNode(id),
                    name,
                ) => {
                    if let (Some(endpoint), Some(group_node)) = (
                        self.endpoints.get_mut(&descriptor),
                        self.group_nodes.get(&id),
                    ) {
                        if let Some(name) = name.filter(|name| *name != endpoint.display_name) {
                            // If the new name exists and is different from the existing name,
                            // re-create the Pipewire node with a new name
                            pipewire_messages.push(ToPipewireMessage::RemoveGroupNode(id.0));
                            pipewire_messages.push(ToPipewireMessage::CreateGroupNode(
                                name,
                                id.0,
                                group_node.kind,
                            ));
                        }
                    }

                    None
                }
                SonusmixMsg::RenameEndpoint(endpoint_desc, name) => {
                    if let Some(endpoint) = self.endpoints.get_mut(&endpoint_desc) {
                        match name {
                            // Unset the custom name if it is the same as the display name
                            Some(name) if name == endpoint.display_name => {
                                endpoint.custom_name = None
                            }
                            _ => endpoint.custom_name = name,
                        }
                    }
                    None
                }
            }
        };

        (output_message, pipewire_messages)
    }

    // Diffs the Sonusmix state and the pipewire state. Returns a list of messages for Pipewire
    // to try and match the Sonusmix state as closely as possible, and marks any endpoints in the
    // Sonusmix state that cannot be found in the Pipewire graph as placeholders. This is only done
    // after updates from the backend.
    fn diff(&mut self, graph: &Graph) -> Vec<ToPipewireMessage> {
        let endpoint_nodes = self.diff_nodes(graph);
        let mut messages = Vec::new();
        // Check that all of the group nodes have a corresponding node. If there isn't one, create it.
        for id in self.group_nodes.keys().copied().collect::<Vec<_>>() {
            let endpoint_desc = EndpointDescriptor::GroupNode(id);
            if self.resolve_endpoint(endpoint_desc, graph).is_some() {
                let group_node = self
                    .group_nodes
                    .get_mut(&id)
                    .expect("We know the group node must exist");
                if group_node.pending {
                    group_node.pending = false;
                }
            } else {
                let group_node = self
                    .group_nodes
                    .get_mut(&id)
                    .expect("We know the group node must exist");
                if !group_node.pending {
                    group_node.pending = true;
                    let endpoint = self
                        .endpoints
                        .get(&endpoint_desc)
                        .expect("We know the endpoint must exist");
                    messages.push(ToPipewireMessage::CreateGroupNode(
                        endpoint.display_name.clone(),
                        id.0,
                        group_node.kind,
                    ));
                }
            }
        }
        messages.extend(self.diff_properties(&endpoint_nodes));
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
        let mut remaining_nodes: HashSet<(u32, PortKind)> = graph
            .nodes
            .values()
            .flat_map(|node| {
                // Add the node as a source and a sink if it has source and sink ports, respectively
                [
                    node.has_port_kind(PortKind::Source)
                        .then_some((node.id, PortKind::Source)),
                    node.has_port_kind(PortKind::Sink)
                        .then_some((node.id, PortKind::Sink)),
                ]
            })
            .flatten()
            .collect();
        let mut endpoint_nodes = HashMap::new();
        for endpoint in self
            .active_sources
            .iter()
            .chain(self.active_sinks.iter())
            .copied()
        {
            if let Some(nodes) = self.resolve_endpoint(endpoint, graph) {
                // Mark the endpoint's nodes as seen
                for node in &nodes {
                    if endpoint.is_single() && endpoint.is_kind(PortKind::Source) {
                        remaining_nodes.remove(&(node.id, PortKind::Source));
                    }
                    if endpoint.is_single() && endpoint.is_kind(PortKind::Sink) {
                        remaining_nodes.remove(&(node.id, PortKind::Sink));
                    }
                }

                if let Some(endpoint) = self.endpoints.get_mut(&endpoint) {
                    // Copy the details from the first resolved node that has any
                    let mut details: Vec<String> = nodes
                        .iter()
                        .filter_map(|node| node.identifier.details())
                        .map(ToOwned::to_owned)
                        .collect();
                    details.sort_unstable();
                    endpoint.details = details;

                    endpoint.is_placeholder = false;
                }

                endpoint_nodes.insert(endpoint, nodes);
            } else if let Some(endpoint) = self.endpoints.get_mut(&endpoint) {
                endpoint.is_placeholder = true;
            }
        }

        // TODO: Check if any of the leftover Pipewire nodes correspond to group nodes. If so, tell
        // the backend to remove them.

        // Any remaining nodes should be recorded as candidates.
        // TODO: When there is more than just ephemeral nodes, add nodes as both ephemeral and
        // persistent, as well as adding applications and devices. At that point it will likely be
        // better to do this incrementally instead of rebuilding the candidates map every time.
        self.candidates = remaining_nodes
            .into_iter()
            .filter_map(|(id, kind)| {
                let node = graph.nodes.get(&id)?;
                Some((id, kind, node.identifier.clone()))
            })
            .collect();

        // Find all unique application name/binary/PortKind combinations. The map values store the
        // icon names.
        let mut applications = HashMap::<(String, String, PortKind), String>::new();
        for node in graph.nodes.values() {
            if let (Some(application), Some(binary)) = (
                node.identifier.application_name.as_ref(),
                node.identifier.binary_name.as_ref(),
            ) {
                if node.has_port_kind(PortKind::Source) {
                    applications.insert(
                        (application.clone(), binary.clone(), PortKind::Source),
                        node.identifier.icon_name().to_owned(),
                    );
                }
                if node.has_port_kind(PortKind::Sink) {
                    applications.insert(
                        (application.clone(), binary.clone(), PortKind::Sink),
                        node.identifier.icon_name().to_owned(),
                    );
                }
            }
        }
        // Remove any combinations that already exist
        for application in self.applications.values() {
            applications.remove(&(
                application.name.clone(),
                application.binary.clone(),
                application.kind,
            ));
        }
        // Add any remaining combinations as new inactive applications
        for ((application_name, binary_name, kind), icon_name) in applications {
            let application =
                Application::new_inactive(application_name, binary_name, icon_name, kind);
            self.applications.insert(application.id, application);
        }

        endpoint_nodes
    }

    /// Check if the properties on the backend nodes match the Sonusmix endpoints, and change one
    /// or the other appropriately based on whether the endpoint is locked.
    fn diff_properties(
        &mut self,
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
                if are_endpoints_connected(source, sink, &node_links) == link.state.is_connected() {
                    link.pending = false;
                }
                continue;
            }

            let num_messages_before = messages.len();

            match link.state {
                LinkState::PartiallyConnected => {
                    // Check if link should actually now be disconnected or fully connected
                    match are_endpoints_connected(source, sink, &node_links) {
                        Some(true) => link.state = LinkState::ConnectedUnlocked,
                        Some(false) => to_remove_indices.push(i),
                        None => {}
                    }
                }
                LinkState::ConnectedUnlocked => {
                    // Check if all necessary links are still there, if not, change to partially
                    // connected or disconnected
                    match are_endpoints_connected(source, sink, &node_links) {
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
                                are_nodes_connected(source, sink, &node_links) != Some(true)
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
                                are_nodes_connected(source, sink, &node_links) != Some(false)
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
            match are_endpoints_connected(source, sink, &node_links) {
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
                    .filter(|node| node.has_port_kind(kind))
                    .map(|node| vec![node])
            }
            EndpointDescriptor::PersistentNode(_id, _kind) => {
                // let (identifier, _) = self.persistent_nodes.get(&id)?;
                // let nodes: Vec<&PwNode> = graph
                //     .nodes
                //     .values()
                //     .filter(|node| node.identifier.matches(identifier))
                //     .filter(|node| {
                //         node.ports.iter().any(|port_id| {
                //             graph
                //                 .ports
                //                 .get(port_id)
                //                 .map(|port| port.kind == kind)
                //                 .unwrap_or(false)
                //         })
                //     })
                //     .collect();
                // (!nodes.is_empty()).then_some(nodes)
                todo!()
            }
            EndpointDescriptor::GroupNode(id) => {
                let result = graph
                    .group_nodes
                    .get(&id.0)
                    .and_then(|(id, _)| graph.nodes.get(id))
                    .map(|node| vec![node]);
                debug!("resolve group node, id {id:?}, {:?}, {result:?}", graph.group_nodes);
                result
            }
            EndpointDescriptor::Application(id, kind) => {
                let application = self.applications.get(&id)?;
                // Resolve all the exceptions. Exceptions should only be an ephemeral or persistent
                // node.
                let exceptions: Vec<&PwNode> = application
                    .exceptions
                    .iter()
                    .filter_map(|exception| match exception {
                        EndpointDescriptor::EphemeralNode(..)
                        | EndpointDescriptor::PersistentNode(..) => {
                            self.resolve_endpoint(*exception, graph)
                        }
                        _ => None,
                    })
                    .flatten()
                    .collect();

                let nodes: Vec<&PwNode> = graph
                    .nodes
                    .values()
                    // Filter to matching nodes
                    .filter(|node| application.matches(&node.identifier, kind))
                    // Filter out nodes matching the exceptions
                    .filter(|node| !exceptions.iter().any(|n| n.id == node.id))
                    .collect();

                (!nodes.is_empty()).then_some(nodes)
            }
            EndpointDescriptor::Device(_id, _kind) => todo!(),
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
                .or_insert_with(Vec::new)
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

    /// Remove the node links from pipewire. Does not remove link from [`Self`].
    fn remove_pipewire_node_links(
        &self,
        graph: &Graph,
        source: EndpointDescriptor,
        sink: EndpointDescriptor,
    ) -> Vec<ToPipewireMessage> {
        // If either of these is None, then the loop will iterate 0 times
        let source_nodes = self.resolve_endpoint(source, graph).unwrap_or_default();
        let sink_nodes = self.resolve_endpoint(sink, graph).unwrap_or_default();

        let mut messages = Vec::new();
        for source in &source_nodes {
            for sink in &sink_nodes {
                messages.push(ToPipewireMessage::RemoveNodeLinks {
                    start_id: source.id,
                    end_id: sink.id,
                })
            }
        }

        messages
    }

    /// If a persistent node matching the given Pipewire node already exists, return a descriptor
    /// for it. Otherwise, create one and return a descriptor for it.
    #[allow(dead_code)] // This will be used when persistent nodes are implemented
    fn get_persistent_node(&mut self, node: &PwNode, kind: PortKind) -> EndpointDescriptor {
        let id = self
            .persistent_nodes
            .iter()
            .find(|(_, (identifier, node_kind))| {
                *node_kind == kind && identifier.matches(&node.identifier)
            })
            .map(|(id, _)| *id);
        // If we found a matching persistent node, return it. Otherwise, create a new one.
        if let Some(id) = id {
            EndpointDescriptor::PersistentNode(id, kind)
        } else {
            let id = PersistentNodeId::new();
            self.persistent_nodes
                .insert(id, (node.identifier.clone(), kind));
            EndpointDescriptor::PersistentNode(id, kind)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Endpoint {
    pub descriptor: EndpointDescriptor,
    pub is_placeholder: bool,
    pub display_name: String,
    pub custom_name: Option<String>,
    pub icon_name: String,
    pub details: Vec<String>,
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
            custom_name: None,
            icon_name: String::new(),
            details: Vec::new(),
            volume: 1.0,
            volume_mixed: false,
            volume_locked_muted: VolumeLockMuteState::UnmutedUnlocked,
            volume_pending: false,
        }
    }

    fn with_display_name(mut self, display_name: String) -> Self {
        self.display_name = display_name;
        self
    }

    fn with_icon_name(mut self, icon_name: String) -> Self {
        self.icon_name = icon_name;
        self
    }

    fn with_details(mut self, details: Vec<String>) -> Self {
        self.details = details;
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
            VolumeLockMuteState::UnmutedUnlocked
        };
        self
    }

    pub fn custom_or_display_name(&self) -> &str {
        self.custom_name.as_ref().unwrap_or(&self.display_name)
    }

    pub fn details_short(&self) -> String {
        match self.details.first() {
            Some(details) => details.clone(),
            None => String::new(),
        }
    }

    pub fn details_long(&self) -> String {
        self.details.join("\n\n")
    }

    #[cfg(test)]
    pub fn new_test(descriptor: EndpointDescriptor) -> Self {
        Self::new(descriptor)
            .with_display_name("TESTING_ENDPOINT".to_owned())
            .with_icon_name("applications-development".to_owned())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Link {
    pub start: EndpointDescriptor,
    pub end: EndpointDescriptor,
    pub state: LinkState,
    pending: bool,
}

/// Describes the state of the links between two endpoints. There is no "DisconnectedUnlocked"
/// state, a link in that state will simply not be recorded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LinkState {
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
    pub fn is_locked(self) -> bool {
        matches!(self, Self::ConnectedLocked | Self::DisconnectedLocked)
    }

    pub fn is_connected(self) -> Option<bool> {
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
pub enum VolumeLockMuteState {
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
    pub fn is_locked(self) -> bool {
        matches!(self, Self::MutedLocked | Self::UnmutedLocked)
    }

    pub fn is_muted(self) -> Option<bool> {
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

impl Default for VolumeLockMuteState {
    fn default() -> Self {
        Self::UnmutedUnlocked
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
    pub fn is_kind(&self, kind: PortKind) -> bool {
        match self {
            Self::GroupNode(_) => true,
            Self::EphemeralNode(_, kind_)
            | Self::PersistentNode(_, kind_)
            | Self::Application(_, kind_)
            | Self::Device(_, kind_) => *kind_ == kind,
        }
    }

    pub fn is_list(&self, kind: PortKind) -> bool {
        match self {
            Self::GroupNode(_) => false,
            Self::EphemeralNode(_, kind_)
            | Self::PersistentNode(_, kind_)
            | Self::Application(_, kind_)
            | Self::Device(_, kind_) => *kind_ == kind,
        }
    }

    pub fn is_single(&self) -> bool {
        match self {
            Self::EphemeralNode(..) | Self::PersistentNode(..) | Self::GroupNode(_) => true,
            Self::Application(..) | Self::Device(..) => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PersistentNodeId(Ulid);

#[allow(dead_code)] // This will be used when persistent nodes are implemented
impl PersistentNodeId {
    fn new() -> Self {
        Self(Ulid::new())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GroupNodeId(Ulid);

impl GroupNodeId {
    fn new() -> Self {
        Self(Ulid::new())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupNode {
    pub id: GroupNodeId,
    pub kind: GroupNodeKind,
    pub pending: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum GroupNodeKind {
    Source,
    #[default]
    Duplex,
    Sink,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ApplicationId(Ulid);

impl ApplicationId {
    fn new() -> Self {
        Self(Ulid::new())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Application {
    pub id: ApplicationId,
    pub kind: PortKind,
    pub is_active: bool,
    pub name: String,
    pub binary: String,
    pub icon_name: String,
    pub exceptions: Vec<EndpointDescriptor>,
}

impl Application {
    fn new_inactive(
        application_name: String,
        binary: String,
        icon_name: String,
        kind: PortKind,
    ) -> Self {
        Self {
            id: ApplicationId::new(),
            kind,
            is_active: false,
            name: application_name,
            binary,
            icon_name,
            exceptions: Vec::new(),
        }
    }

    pub fn matches(&self, identifier: &NodeIdentifier, kind: PortKind) -> bool {
        self.kind == kind
            && identifier.application_name.as_ref() == Some(&self.name)
            && identifier.binary_name.as_ref() == Some(&self.binary)
    }

    pub fn name_with_tag(&self) -> String {
        // Uses unicode "fullwidth" brackets which I personally think look nicer
        format!("［App］{}", self.name)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DeviceId(Ulid);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device;

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
        _ => return false,
    };
    iterator.all(|x| x == first)
}

/// Aggregate an iterator of booleans into an `Option<bool>`. Some if all booleans are the same,
/// None if any are different from each other.
fn aggregate_bools<'a>(bools: impl IntoIterator<Item = &'a bool>) -> Option<bool> {
    let mut iter = bools.into_iter();
    let first = iter.next()?;
    iter.all(|b| b == first).then_some(*first)
}

/// Returns whether the two node are connected. Some(bool) if the nodes are completely connected or
/// not, None if they are partially connected.
/// For now, "completely connected" means that every port on one of the nodes (either one) is
/// connected to a port on the other. "not connected" means there are no links from any port on one
/// node to any port on the other. "partially connected" is any other state.
fn are_nodes_connected(
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
        .filter(|(_, kind)| *kind == PortKind::Source)
        .all(|(id, _)| relevant_links.iter().any(|link| link.start_port == *id))
        || sink
            .ports
            .iter()
            .filter(|(_, kind)| *kind == PortKind::Sink)
            .all(|(id, _)| relevant_links.iter().any(|link| link.end_port == *id))
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
    source: &[&PwNode],
    sink: &[&PwNode],
    node_links: &HashMap<(u32, u32), Vec<&PwLink>>,
) -> Option<bool> {
    let mut iter = source
        .iter()
        .cartesian_product(sink.iter())
        .map(|(source_node, sink_node)| are_nodes_connected(source_node, sink_node, node_links));
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

        pipewire_node.ports = vec![(2, PortKind::Source)];

        let pipewire_state = Graph {
            group_nodes: HashMap::new(),
            clients: HashMap::from([(0, client_of_node); 1]),
            devices: HashMap::new(),
            nodes: HashMap::from([(1, pipewire_node); 1]),
            ports: HashMap::from([(2, port_of_node); 1]),
            links: HashMap::new(),
        };

        let sonusmix_node = EndpointDescriptor::EphemeralNode(1, PortKind::Source);
        let sonusmix_node_endpoint = Endpoint::new_test(sonusmix_node);
        let sonusmix_state = SonusmixState {
            group_nodes: HashMap::new(),
            active_sources: Vec::from([sonusmix_node]),
            active_sinks: Vec::new(),
            endpoints: HashMap::from([(sonusmix_node, sonusmix_node_endpoint); 1]),
            candidates: Vec::new(),
            links: Vec::new(),
            persistent_nodes: HashMap::new(),
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
            source_node.ports = vec![(3, PortKind::Source)];
            let source_client = Client::new_test(2, false, Vec::from([1]));
            let source_port = Port::new_test(3, 1, PortKind::Source);

            let mut sink_node = Node::new_test(2, EndpointId::Client(4));
            sink_node.ports = vec![(5, PortKind::Sink)];
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
                group_nodes: HashMap::new(),
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

            let link = super::Link {
                start: source_node,
                end: sink_node,
                state: LinkState::ConnectedUnlocked,
                pending: false,
            };

            let active_sources = vec![source_node];

            let active_sinks = vec![sink_node];

            let mut endpoints = HashMap::new();
            endpoints.insert(source_node, source_endpoint);
            endpoints.insert(sink_node, sink_endpoint);

            let links = vec![link];

            SonusmixState {
                active_sources,
                active_sinks,
                endpoints,
                links,
                ..Default::default()
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
        assert!(!endpoint.is_placeholder);
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
        assert!(endpoint.is_placeholder);
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
        let pipewire_messages = sonusmix_state.diff_properties(&endpoint_nodes);

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
        let pipewire_messages = sonusmix_state.diff_properties(&endpoint_nodes);

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
        let pipewire_messages = sonusmix_state.diff_properties(&endpoint_nodes);

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
        assert!(volumes_mixed(&[0.1, 0.12, 0.18]));
        assert!(!volumes_mixed(&[0.1, 0.1, 0.1]));
        assert!(!volumes_mixed(&[]));
    }

    #[test]
    fn diff_properties_mixed_volume_locked() {
        diff_properties_mixed_volume_unlocked_or_locked(true);
    }

    #[test]
    fn diff_properties_mixed_volume_unlocked() {
        diff_properties_mixed_volume_unlocked_or_locked(false);
    }

    #[test]
    fn create_link() {
        let (mut pipewire_state, mut sonusmix_state) = advanced_graph_ephermal_node_setup();

        // remove the link from default state
        sonusmix_state.links.clear();
        pipewire_state.links.clear();

        let source = sonusmix_state.active_sources[0];
        let sink = sonusmix_state.active_sinks[0];

        {
            // create a link
            let (output_msg, messages) =
                sonusmix_state.update(&pipewire_state, SonusmixMsg::Link(source, sink));
            let expected_message = ToPipewireMessage::CreateNodeLinks {
                start_id: 1,
                end_id: 2,
            };
            assert!(messages.contains(&expected_message));
        }

        let expected_link = super::Link {
            start: source,
            end: sink,
            state: LinkState::ConnectedUnlocked,
            pending: true,
        };

        assert!(sonusmix_state.links.contains(&expected_link));

        // simulate the link being successfully created by pipewire
        pipewire_state
            .links
            .insert(6, Link::new_test(6, 1, 3, 2, 5));

        // run the diff
        let endpoint_nodes = sonusmix_state.diff_nodes(&pipewire_state);
        let messages = sonusmix_state.diff_links(&pipewire_state, &endpoint_nodes);

        assert!(messages.is_empty());

        // (there is only a single link at this point)
        assert!(!sonusmix_state.links[0].pending);
    }

    #[test]
    fn remove_link() {
        let (pipewire_state, mut sonusmix_state) = advanced_graph_ephermal_node_setup();

        let source = sonusmix_state.active_sources[0];
        let sink = sonusmix_state.active_sinks[0];

        {
            // disconnect
            let (output_msg, messages) =
                sonusmix_state.update(&pipewire_state, SonusmixMsg::RemoveLink(source, sink));
            let expected_message = ToPipewireMessage::RemoveNodeLinks {
                start_id: 1,
                end_id: 2,
            };
            assert!(output_msg.is_none());
            assert!(messages.contains(&expected_message));
        }

        // in the next update the link will be removed.
        // see test pipewire_remove_link_unlocked
    }

    #[test]
    fn disconnect_locked_link() {
        let (mut pipewire_state, mut sonusmix_state) = advanced_graph_ephermal_node_setup();

        let source = sonusmix_state.active_sources[0];
        let sink = sonusmix_state.active_sinks[0];
        let _link = sonusmix_state.links[0];

        {
            // disconnect locked
            let (output_msg, messages) = sonusmix_state.update(
                &pipewire_state,
                SonusmixMsg::SetLinkLocked(source, sink, false),
            );
            let expected_message = ToPipewireMessage::RemoveNodeLinks {
                start_id: 1,
                end_id: 2,
            };
            assert!(output_msg.is_none());
            assert!(messages.contains(&expected_message));
        }

        let expected_link = super::Link {
            start: source,
            end: sink,
            state: LinkState::DisconnectedLocked,
            pending: false,
        };

        assert!(sonusmix_state.links.contains(&expected_link));

        // simulate trying to establish a connection between these nodes
        pipewire_state
            .links
            .insert(6, Link::new_test(6, 1, 3, 2, 5));

        // run the diff
        let endpoint_nodes = sonusmix_state.diff_nodes(&pipewire_state);
        let messages = sonusmix_state.diff_links(&pipewire_state, &endpoint_nodes);

        // sonusmix should tell pipewire to delete that link again
        let expected_message = ToPipewireMessage::RemoveNodeLinks {
            start_id: 1,
            end_id: 2,
        };
        assert!(messages.contains(&expected_message));
    }

    #[test]
    fn connect_locked_link() {
        let (mut pipewire_state, mut sonusmix_state) = advanced_graph_ephermal_node_setup();

        let source = sonusmix_state.active_sources[0];
        let sink = sonusmix_state.active_sinks[0];
        let _link = sonusmix_state.links[0];

        {
            // disconnect locked
            let (output_msg, messages) = sonusmix_state.update(
                &pipewire_state,
                SonusmixMsg::SetLinkLocked(source, sink, true),
            );
            assert!(output_msg.is_none());
            assert!(messages.is_empty());
        }

        let expected_link = super::Link {
            start: source,
            end: sink,
            state: LinkState::ConnectedLocked,
            pending: false,
        };

        assert!(sonusmix_state.links.contains(&expected_link));

        // simulate pipewire trying to delete this link
        pipewire_state.links.clear();

        // run the diff
        let endpoint_nodes = sonusmix_state.diff_nodes(&pipewire_state);
        let messages = sonusmix_state.diff_links(&pipewire_state, &endpoint_nodes);

        // sonusmix should tell pipewire to create that link again
        let expected_message = ToPipewireMessage::CreateNodeLinks {
            start_id: 1,
            end_id: 2,
        };
        assert!(messages.contains(&expected_message));
    }

    #[test]
    /// Event is coming from pipewire with a new link.
    /// The link should be added to sonusmix.
    fn new_pipewire_link() {
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
    /// Event is coming from pipewire with no link.
    /// The sonusmix state should remove its unlocked.
    fn pipewire_remove_link_unlocked() {
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
}
