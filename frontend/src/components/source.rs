use super::{sink::SinkState, device::State};
use std::rc::Rc;
use std::collections::HashMap;

pub struct SourceState {
    name: String,
    volume: u32,
    connections: HashMap<String, bool>
}

impl SourceState {
    pub fn new_with_connections(name: String, volume: u32, connections: HashMap<String, bool>) -> Self {
        SourceState { name, volume, connections }
    }

    pub fn new(name: String, volume: u32) -> Self {
        SourceState { name, volume, connections: HashMap::new() }
    }

    pub fn name(&self) -> String {
        self.name.to_string()
    }
}

impl Into<State> for SourceState {
    fn into(self) -> State {
        State { name: self.name, volume: self.volume, connections: Some(self.connections) }
    }
}
