use std::{collections::HashMap, fmt::Debug, rc::Rc, sync::Arc};

use pipewire::{registry::GlobalObject, spa::utils::dict::DictRef, types::ObjectType};
use serde::{Deserialize, Serialize};

use super::{
    object::{Endpoint, Link, Node, ObjectConvertError, Port, PortKind},
    Graph,
};

#[derive(Debug)]
pub(super) struct Store {
    pub(super) endpoints: HashMap<u32, Endpoint>,
    pub(super) nodes: HashMap<u32, Node>,
    pub(super) ports: HashMap<u32, Port>,
    pub(super) links: HashMap<u32, Link>,
}

impl Store {
    pub(super) fn new() -> Self {
        Self {
            endpoints: HashMap::new(),
            nodes: HashMap::new(),
            ports: HashMap::new(),
            links: HashMap::new(),
        }
    }

    /// The returned boolean describes whether the given type was supported and thus added.
    pub(super) fn add_object(
        &mut self,
        object: &GlobalObject<&DictRef>,
    ) -> Result<bool, ObjectConvertError> {
        match object.type_ {
            ObjectType::Client | ObjectType::Device => self.add_endpoint(object)?,
            ObjectType::Node => self.add_node(object)?,
            ObjectType::Port => self.add_port(object)?,
            ObjectType::Link => self.add_link(object)?,
            _ => return Ok(false),
        }
        Ok(true)
    }

    pub(super) fn remove_object(&mut self, id: u32) {
        if let Some(_endpoint) = self.endpoints.remove(&id) {
            // Nothing else needs to be done
        } else if let Some(node) = self.nodes.remove(&id) {
            // If the endpoint the node belongs to exists, remove the node from it
            if let Some(endpoint) = self.endpoints.get_mut(&node.endpoint) {
                endpoint.nodes.retain(|id| *id != node.id);
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

    pub(super) fn add_endpoint(
        &mut self,
        object: &GlobalObject<&DictRef>,
    ) -> Result<(), ObjectConvertError> {
        // Create the endpoint
        let mut endpoint: Endpoint = object.try_into()?;

        // Find and add any nodes belonging to the endpoint
        endpoint.nodes = self
            .nodes
            .values()
            .filter_map(|node| (node.endpoint == endpoint.id).then_some(node.id))
            .collect();

        // Add the endpoint
        self.endpoints.insert(endpoint.id, endpoint);
        Ok(())
    }

    pub(super) fn add_node(
        &mut self,
        object: &GlobalObject<&DictRef>,
    ) -> Result<(), ObjectConvertError> {
        // Create the node
        let mut node: Node = object.try_into()?;

        // Find and add any ports belonging to the node
        node.ports = self
            .ports
            .values()
            .filter_map(|port| (port.node == node.id).then_some(port.id))
            .collect();

        // If the endpoint the node belongs to exists, add the node to it
        if let Some(endpoint) = self.endpoints.get_mut(&node.endpoint) {
            endpoint.nodes.push(node.id);
        }

        // Add the node
        self.nodes.insert(node.id, node);
        Ok(())
    }

    pub(super) fn add_port(
        &mut self,
        object: &GlobalObject<&DictRef>,
    ) -> Result<(), ObjectConvertError> {
        // Create the port
        let mut port: Port = object.try_into()?;

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
        object: &GlobalObject<&DictRef>,
    ) -> Result<(), ObjectConvertError> {
        // Create the link
        let link: Link = object.try_into()?;

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

    pub fn dump_graph(&self) -> Graph {
        Graph {
            endpoints: self.endpoints.values().cloned().collect(),
            nodes: self.nodes.values().cloned().collect(),
            ports: self.ports.values().cloned().collect(),
            links: self.links.values().cloned().collect(),
        }
    }
}
