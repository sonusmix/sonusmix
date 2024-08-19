use std::{collections::HashMap, fmt::Debug};

use pipewire::{registry::GlobalObject, spa::utils::dict::DictRef, types::ObjectType};

use super::object::{Node, ObjectConvertError, PipewireObject};

#[derive(Debug)]
pub(super) struct Store {
    client_id: Option<u32>,
    pub(super) nodes: HashMap<u32, PipewireObject<Node>>,
}

impl Store {
    pub(super) fn new() -> Self {
        Self {
            client_id: None,
            nodes: HashMap::new(),
        }
    }

    /// The returned boolean describes whether the given type was supported and thus added
    pub(super) fn add_object(
        &mut self,
        object: &GlobalObject<&DictRef>,
    ) -> Result<bool, ObjectConvertError> {
        match object.type_ {
            ObjectType::Node => self.add_node(object.try_into()?),
            _ => return Ok(false),
        }
        Ok(true)
    }

    pub(super) fn add_node(&mut self, node: PipewireObject<Node>) {
        self.nodes.insert(node.id, node);
    }
}
