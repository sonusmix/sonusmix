use std::{collections::{HashMap, hash_map}};

use pipewire::{prelude::ReadableDict, keys::*, registry::GlobalObject};

// TODO: Figure out if it would be advantageous to have a way to more easily find all the links for a given node or
// TODO: port, and if so, figure out a way to do it

/// This type is a local store for all of the pipewire objects that this program is notified about. Because it needs
/// to keep its stores for each of the types in sync with each other, it only exposes its own methods for mutation,
/// however, it provides methods for accessing the internal maps for reading only.
#[derive(Default, Debug)]
pub(crate) struct PipewireStore {
    nodes: HashMap<u32, Node>,
    ports: HashMap<u32, Port>,
    // links is indexed by port ids
    links: HashMap<LinkEnds, Link>,
    // orphan_ports is node id -> port ids
    orphan_ports: HashMap<u32, Vec<u32>>,
}

impl PipewireStore {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    // TODO: Fix error handling on these methods
    pub(crate) fn add_object(&mut self, object: &GlobalObject<impl ReadableDict>) -> Result<(), ()> {
        match object.type_ {
            pipewire::types::ObjectType::Node => self.add_node(object.id, object.props.as_ref().ok_or(())?),
            pipewire::types::ObjectType::Port => self.add_port(object.id, object.props.as_ref().ok_or(())?),
            pipewire::types::ObjectType::Link => self.add_link(object.id, object.props.as_ref().ok_or(())?),
            _ => Err(()),
        }
    }

    fn add_node(&mut self, id: u32, props: &impl ReadableDict) -> Result<(), ()> {
        // Create the node
        let name = props.get(*NODE_NICK)
            .or_else(|| props.get("node.description"))
            .or_else(|| props.get(*APP_NAME))
            .or_else(||props.get(*NODE_NAME))
            .ok_or(())?
            .to_string();

        // TODO: Maybe improve detection of what is an "application"
        // TODO: Add virtual device detection (if the id matches a virtual device this application created)
        let kind = if props.get(*APP_NAME).is_some() {
            NodeKind::Application
        } else {
            NodeKind::Device
        };

        // Find any old nodes that the new one is replacing, and grab their ports
        let mut ports = if let Some(old_node) = self.nodes.remove(&id) {
            old_node.ports
        } else {
            Vec::new()
        };

        // Find any ports associated with the device
        if let hash_map::Entry::Occupied(orphan_ports) = self.orphan_ports.entry(id) {
            // extend vs append? It shouldn't matter, mostly a style difference, as the compiler should be smart enough
            // to specialize extend into append.
            ports.extend(orphan_ports.remove());
        }

        let class = {
            let mut media_class = props.get(*MEDIA_CLASS).ok_or(())?
                .to_string();
            if !media_class.contains("Audio") {
                return Err(());
            }

            if !media_class.contains('/') {
                if let Some(category) = props.get(*MEDIA_CATEGORY) {
                    media_class.push_str("/");
                    media_class.push_str(category);
                }
            }

            if media_class.contains("Duplex") || media_class.contains("Source/Virtual") {
                NodeClass::Duplex
            } else if media_class.contains("Source") || media_class.contains("Output") {
                NodeClass::Source
            } else if media_class.contains("Sink") || media_class.contains("Input") {
                // TODO: The "port.monitor" prop may not be the most reliable way of determining if a port is a monitor.
                // TODO: All monitor ports seem to be marked as monitors, but some ports that are marked as monitors
                // TODO: seem to be capture or other kinds of ports.
                if ports.iter()
                    .flat_map(|port_id| self.ports.get(port_id))
                    .any(|port| port.is_monitor)
                {
                    NodeClass::SinkMonitor
                } else {
                    NodeClass::Sink
                }
            } else {
                return Err(())
            }
        };

        self.nodes.insert(id, Node { id, name, kind, class, ports });

        Ok(())
    }

    fn add_port(&mut self, id: u32, props: &impl ReadableDict) -> Result<(), ()> {
        let name = props.get(*PORT_NAME)
            .ok_or(())?
            .to_string();
        
        let channel = props.get(*AUDIO_CHANNEL)
            .ok_or(())?
            .to_string();
        
        let direction = match props.get(*PORT_DIRECTION) {
            Some("in") => PortDirection::In,
            Some("out") => PortDirection::Out,
            _ => return Err(()),
        };

        // TODO: The "port.monitor" prop may not be the most reliable way of determining if a port is a monitor. All
        // TODO: monitor ports seem to be marked as monitors, but some ports that are marked as monitors seem to be
        // TODO: capture or other kinds of ports.
        let is_monitor = matches!(props.get(*PORT_MONITOR), Some("true"));

        let node = props.get(*NODE_ID)
            .ok_or(())?
            .parse::<u32>()
            .map_err(|_| ())?;

        self.ports.insert(id, Port { id, name, channel, direction, is_monitor, node });

        // Check if the node this port belongs to is known. If so, add this port to it, and handle changing its class to
        // SinkMonitor, if applicable. Otherwise, save this port to orphaned_ports.
        if let hash_map::Entry::Occupied(mut node) = self.nodes.entry(node) {
            let node = node.get_mut();
            node.ports.push(id);
            if is_monitor && node.class == NodeClass::Sink {
                node.class = NodeClass::SinkMonitor;
            }
        } else {
            self.orphan_ports.entry(node)
                .or_insert_with(Vec::new)
                .push(id);
        }

        Ok(())
    }

    fn add_link(&mut self, id: u32, props: &impl ReadableDict) -> Result<(), ()> {
        let ports = LinkEnds {
            input: props.get(*LINK_INPUT_PORT)
                .ok_or(())?
                .parse()
                .map_err(|_| ())?,
            output: props.get(*LINK_OUTPUT_PORT)
                .ok_or(())?
                .parse()
                .map_err(|_| ())?,
        };

        let nodes = LinkEnds {
            input: props.get(*LINK_INPUT_NODE)
                .ok_or(())?
                .parse()
                .map_err(|_| ())?,
            output: props.get(*LINK_OUTPUT_NODE)
                .ok_or(())?
                .parse()
                .map_err(|_| ())?,
        };

        self.links.insert(ports, Link { id, ports, nodes });

        Ok(())
    }

    pub(crate) fn nodes(&self) -> &HashMap<u32, Node> {
        &self.nodes
    }

    pub(crate) fn ports(&self) -> &HashMap<u32, Port> {
        &self.ports
    }

    pub(crate) fn links(&self) -> &HashMap<LinkEnds, Link> {
        &self.links
    }
}

#[derive(Debug)]
pub(crate) struct Node {
    id: u32,
    name: String,
    kind: NodeKind,
    class: NodeClass,
    ports: Vec<u32>,
}

#[derive(Debug)]
pub(crate) enum NodeKind {
    Device,
    Virtual,
    Application,
}

#[derive(PartialEq, Debug)]
pub(crate) enum NodeClass {
    Source,
    Sink,
    SinkMonitor,
    Duplex,
}

#[derive(Debug)]
pub(crate) struct Port {
    id: u32,
    name: String,
    channel: String,
    direction: PortDirection,
    is_monitor: bool,
    node: u32,
}

#[derive(Debug)]
pub(crate) enum PortDirection {
    In,
    Out,
}

#[derive(Debug)]
pub(crate) struct Link {
    id: u32,
    ports: LinkEnds,
    nodes: LinkEnds,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) struct LinkEnds {
    input: u32,
    output: u32,
}