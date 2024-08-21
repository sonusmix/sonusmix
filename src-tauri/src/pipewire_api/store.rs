use std::{collections::HashMap, fmt::Debug, rc::Rc, sync::Arc};

use pipewire::{registry::GlobalObject, spa::utils::dict::DictRef, types::ObjectType};

use super::object::{Link, Node, ObjectConvertError, Port};

#[derive(Debug)]
pub(super) struct Store {
    client_id: Option<u32>,
    pub(super) nodes: HashMap<u32, Arc<Node>>,
    pub(super) ports: HashMap<u32, Arc<Port>>,
    pub(super) links: HashMap<u32, Arc<Link>>,
}

impl Store {
    pub(super) fn new() -> Self {
        Self {
            client_id: None,
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

    pub(super) fn add_node(
        &mut self,
        object: &GlobalObject<&DictRef>,
    ) -> Result<(), ObjectConvertError> {
        // Create the node
        let node: Node = object.try_into()?;

        // Find and add any ports belonging to the node
        {
            let mut node_ports = node.ports.write().expect("node ports lock poisoned");
            for port in self.ports.values().filter(|port| port.node == node.id) {
                node_ports.push(port.clone());
            }
        }

        // TODO: If the device the node belongs to exists, add the node to it
        let node = Arc::new(node);

        // Add the node
        self.nodes.insert(node.id, node);
        Ok(())
    }

    pub(super) fn add_port(
        &mut self,
        object: &GlobalObject<&DictRef>,
    ) -> Result<(), ObjectConvertError> {
        // Create the port
        let port: Port = object.try_into()?;

        // TODO: Find and add any links belonging to the port

        // If the node the port belongs to exists, add the port to it
        let port = Arc::new(port);
        if let Some(node) = self.nodes.get(&port.node) {
            node.ports
                .write()
                .expect("node ports lock poisoned")
                .push(port.clone());
        }

        // Add the port
        self.ports.insert(port.id, port);
        Ok(())
    }
}
