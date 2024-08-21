use std::{collections::HashMap, fmt::Debug, rc::Rc, sync::Arc};

use pipewire::{registry::GlobalObject, spa::utils::dict::DictRef, types::ObjectType};

use super::object::{Device, Link, Node, ObjectConvertError, Port, PortKind};

#[derive(Debug)]
pub(super) struct Store {
    pub(super) devices: HashMap<u32, Device>,
    pub(super) nodes: HashMap<u32, Node>,
    pub(super) ports: HashMap<u32, Port>,
    pub(super) links: HashMap<u32, Link>,
}

impl Store {
    pub(super) fn new() -> Self {
        Self {
            devices: HashMap::new(),
            nodes: HashMap::new(),
            ports: HashMap::new(),
            links: HashMap::new(),
        }
    }

    /// The returned boolean describes whether the given type was supported and thus added
    pub(super) fn add_object(
        &mut self,
        object: &GlobalObject<&DictRef>,
    ) -> Result<bool, ObjectConvertError> {
        match object.type_ {
            ObjectType::Node => self.add_node(object)?,
            _ => return Ok(false),
        }
        Ok(true)
    }

    pub(super) fn add_device(&mut self, object: &GlobalObject<&DictRef>) -> Result<(), ObjectConvertError> {
        // Create the device

        // Find and add any nodes belonging to the device

        // Add the device

        todo!()
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

        // TODO: If the device the node belongs to exists, add the node to it

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

        // If the ports the link belongs to exists, add the link to them
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
}
