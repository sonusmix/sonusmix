use std::sync::Arc;

use relm4::SharedState;

use crate::pipewire_api::{Graph, PipewireHandle};

static GRAPH_STATE: SharedState<Arc<Graph>> = SharedState::new();

pub fn link_pipewire(pw: &PipewireHandle) {
    let _key = pw.subscribe(|graph| *GRAPH_STATE.write() = graph);
}

/// Subscribes to graph updates, and returns the current state of the graph.
pub fn subscribe_to_pipewire<Msg, F>(sender: &relm4::Sender<Msg>, f: F) -> Arc<Graph>
where
    F: Fn(Arc<Graph>) -> Msg + 'static + Send + Sync,
    Msg: Send + 'static,
{
    GRAPH_STATE.subscribe(sender, move |graph| f(graph.clone()));
    GRAPH_STATE.read().clone()
}
