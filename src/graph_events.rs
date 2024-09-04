use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use relm4::SharedState;

use crate::pipewire_api::Graph;

static GRAPH_STATE: SharedState<Arc<Graph>> = SharedState::new();

/// Returns a function that, when called, will write to the state, and send out updates to all
/// subscribers. May only be called once.
/// # Panics
/// This function will panic if it is ever called a second time.
pub fn link_pipewire() -> impl Fn(Graph) + Send + 'static {
    // Ensure that this function is only ever called once
    static IS_LINKED: AtomicBool = AtomicBool::new(false);
    // I don't really care about performance for this one small part, and SeqCst provides the
    // strongest guarantees, so it's (probably?) the safest
    assert!(
        !IS_LINKED.swap(true, Ordering::SeqCst),
        "link_pipewire() may only be called once"
    );

    |graph| *GRAPH_STATE.write() = Arc::new(graph)
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
