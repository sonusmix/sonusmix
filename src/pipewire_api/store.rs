use anyhow::{anyhow, Result};
use log::debug;
use std::{collections::HashMap, fmt::Debug};

use pipewire::{
    node::NodeInfoRef,
    registry::{GlobalObject, Registry},
    spa::{
        param::ParamType,
        pod::{deserialize::PodDeserializer, Pod},
        utils::dict::DictRef,
    },
    types::ObjectType,
};

use super::{
    actions::NodeAction,
    object::{Client, Device, EndpointId, Link, Node, ObjectConvertError, Port, PortKind},
    pod::NodeProps,
    Graph,
};

#[derive(Debug)]
pub(super) struct Store {
    pub(super) sonusmix_client_id: Option<u32>,
    pub(super) clients: HashMap<u32, Client>,
    pub(super) devices: HashMap<u32, Device>,
    pub(super) nodes: HashMap<u32, Node>,
    pub(super) ports: HashMap<u32, Port>,
    pub(super) links: HashMap<u32, Link>,
}

impl Store {
    pub(super) fn new() -> Self {
        Self {
            sonusmix_client_id: None,
            clients: HashMap::new(),
            devices: HashMap::new(),
            nodes: HashMap::new(),
            ports: HashMap::new(),
            links: HashMap::new(),
        }
    }

    /// The returned boolean describes whether the given type was supported and thus added.
    pub(super) fn add_object(
        &mut self,
        registry: &Registry,
        object: &GlobalObject<&DictRef>,
    ) -> Result<bool, ObjectConvertError> {
        match object.type_ {
            ObjectType::Client => self.add_client(registry, object)?,
            ObjectType::Device => self.add_device(registry, object)?,
            ObjectType::Node => self.add_node(registry, object)?,
            ObjectType::Port => self.add_port(registry, object)?,
            ObjectType::Link => self.add_link(registry, object)?,
            _ => return Ok(false),
        }
        Ok(true)
    }

    pub(super) fn remove_object(&mut self, id: u32) {
        if let Some(client) = self.clients.remove(&id) {
            // Check if the client being removed is Sonusmix. If so, remove its id.
            if client.is_sonusmix {
                self.sonusmix_client_id = None;
            }
        } else if let Some(_device) = self.devices.remove(&id) {
            // Nothing else to do (for now)
        } else if let Some(node) = self.nodes.remove(&id) {
            // If the endpoint the node belongs to exists, remove the node from it
            match node.endpoint {
                EndpointId::Device(id) => {
                    if let Some(device) = self.devices.get_mut(&id) {
                        device.nodes.retain(|id| *id != node.id);
                    }
                }
                EndpointId::Client(id) => {
                    if let Some(client) = self.clients.get_mut(&id) {
                        client.nodes.retain(|id| *id != node.id);
                    }
                }
            }
        } else if let Some(port) = self.ports.remove(&id) {
            // If the node the port belongs to exists, remove the port from it
            if let Some(node) = self.nodes.get_mut(&port.node) {
                node.ports.retain(|id| *id != port.id);
            }
        } else if let Some(link) = self.links.remove(&id) {
            // If the ports the link belongs to exist, remove the link from them
            if let Some(port) = self.ports.get_mut(&link.start_port) {
                port.links.retain(|id| *id != link.id);
            }
            if let Some(port) = self.ports.get_mut(&link.end_port) {
                port.links.retain(|id| *id != link.id);
            }
        }
    }

    pub(super) fn add_client(
        &mut self,
        registry: &Registry,
        object: &GlobalObject<&DictRef>,
    ) -> Result<(), ObjectConvertError> {
        // Create the client
        let mut client = Client::from_global(registry, object)?;

        // Find and add any nodes belonging to the client
        client.nodes = self
            .nodes
            .values()
            .filter_map(|node| (node.endpoint == EndpointId::Client(client.id)).then_some(node.id))
            .collect();

        // Check if the client is Sonusmix. If so, record its id.
        if client.is_sonusmix {
            self.sonusmix_client_id = Some(client.id);
        }

        // Add the client
        self.clients.insert(client.id, client);
        Ok(())
    }

    pub(super) fn add_device(
        &mut self,
        registry: &Registry,
        object: &GlobalObject<&DictRef>,
    ) -> Result<(), ObjectConvertError> {
        // Create the endpoint
        let mut device = Device::from_global(registry, object)?;

        // TODO: Sort out the relationship between devices and clients

        // Find and add any nodes belonging to the device
        device.nodes = self
            .nodes
            .values()
            .filter_map(|node| (node.endpoint == EndpointId::Device(device.id)).then_some(node.id))
            .collect();

        // Add the device
        self.devices.insert(device.id, device);
        Ok(())
    }

    pub(super) fn add_node(
        &mut self,
        registry: &Registry,
        object: &GlobalObject<&DictRef>,
    ) -> Result<(), ObjectConvertError> {
        // Create the node
        let mut node = Node::from_global(registry, object)?;

        // Find and add any ports belonging to the node
        node.ports = self
            .ports
            .values()
            .filter_map(|port| (port.node == node.id).then_some(port.id))
            .collect();

        // If the endpoint the node belongs to exists, add the node to it
        match node.endpoint {
            EndpointId::Device(id) => {
                if let Some(device) = self.devices.get_mut(&id) {
                    device.nodes.push(node.id);
                }
            }
            EndpointId::Client(id) => {
                if let Some(client) = self.clients.get_mut(&id) {
                    client.nodes.push(node.id);
                }
            }
        }

        // Add the node
        self.nodes.insert(node.id, node);
        Ok(())
    }

    pub(super) fn add_port(
        &mut self,
        registry: &Registry,
        object: &GlobalObject<&DictRef>,
    ) -> Result<(), ObjectConvertError> {
        // Create the port
        let mut port = Port::from_global(registry, object)?;

        // Find and add any links belonging to the port
        let matching_id: fn(&Link) -> u32 = match port.kind {
            PortKind::Source => |link| link.start_port,
            PortKind::Sink => |link| link.end_port,
        };
        port.links = self
            .links
            .values()
            .filter_map(|link| (matching_id(link) == port.id).then_some(link.id))
            .collect();

        // If the node the port belongs to exists, add the port to it
        if let Some(node) = self.nodes.get_mut(&port.node) {
            node.ports.push(port.id);
        }

        // Add the port
        self.ports.insert(port.id, port);

        Ok(())
    }

    pub(super) fn add_link(
        &mut self,
        registry: &Registry,
        object: &GlobalObject<&DictRef>,
    ) -> Result<(), ObjectConvertError> {
        // Create the link
        let link = Link::from_global(registry, object)?;

        // If the ports the link belongs to exist, add the link to them
        if let Some(port) = self.ports.get_mut(&link.start_port) {
            port.links.push(link.id);
        }
        if let Some(port) = self.ports.get_mut(&link.end_port) {
            port.links.push(link.id);
        }

        // Add the link
        self.links.insert(link.id, link);
        Ok(())
    }

    pub(super) fn change_node(&mut self, _type_: ParamType, id: u32, pod: Option<&Pod>) {
        // abort if no pod is available
        let pod = match pod {
            Some(p) => p,
            None => return,
        };

        let node = self
            .nodes
            .get_mut(&id)
            .expect("The node was destroyed unexpectedly");

        // deserialize the pod
        let (_, value) =
            PodDeserializer::deserialize_any_from(pod.as_bytes()).expect("Deserialization failed");

        let node_props = NodeProps::new(value);

        // save volume if node has volume
        if let Some(volume) = node_props.get_channel_volumes() {
            node.channel_volumes = volume.to_vec();
        }
    }

    pub(super) fn update_node_info(&mut self, node_info: &NodeInfoRef) {
        let Some(props) = node_info.props() else {
            return;
        };
        let node = self
            .nodes
            .get_mut(&node_info.id())
            .expect("The node was destroyed unexpectedly");
        node.identifier.update_from_props(props);
    }

    /// Send an action to a pipewire node.
    pub(super) fn node_action(&mut self, id: u32, action: NodeAction) -> Result<()> {
        let node = self
            .nodes
            .get(&id)
            .ok_or_else(|| anyhow!("Node {id} not found"))?;

        debug!("got action node {id} to {:?}", action);

        let action_param_bytes = action.apply(node)?;
        let action_param_pod =
            Pod::from_bytes(action_param_bytes.as_slice()).expect("apply returned invalid pod");

        // send parameter to pipewire
        node.proxy.set_param(ParamType::Props, 0, action_param_pod);

        node.proxy.enum_params(7, Some(ParamType::Props), 0, 1);

        Ok(())
    }

    #[rustfmt::skip] // Rustfmt puts each call on its own line which is really hard to read
    pub fn dump_graph(&self) -> Graph {
        Graph {
            clients: self.clients.iter().map(|(id, client)| (*id, client.without_proxy())).collect(),
            devices: self.devices.iter().map(|(id, device)| (*id, device.without_proxy())).collect(),
            nodes: self.nodes.iter().map(|(id, node)| (*id, node.without_proxy())).collect(),
            ports: self.ports.iter().map(|(id, port)| (*id, port.without_proxy())).collect(),
            links: self.links.iter().map(|(id, link)| (*id, link.without_proxy())).collect(),
        }
    }
}
