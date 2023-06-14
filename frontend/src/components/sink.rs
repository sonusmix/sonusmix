use super::device::{Device, State};
use std::rc::Rc;

pub struct SinkState {
    name: String,
    volume: u32,
}

impl SinkState {
    pub fn new(name: String, volume: u32) -> Self {
        SinkState { name, volume }
    }

    pub fn name(&self) -> String {
        self.name.clone()
    }
}

impl Into<State> for SinkState {
    fn into(self) -> State {
        State { name: self.name, volume: self.volume, connections: None }
    }
}
