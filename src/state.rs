use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use relm4::SharedState;

use crate::pipewire_api::{Graph, PortKind};

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

pub static SONUSMIX_STATE: SonusmixReducer =
    SonusmixReducer(SharedState::new(), SharedState::new());

#[derive(Debug, Clone, Default)]
pub struct SonusmixState {
    pub active_sources: Vec<u32>,
    pub active_sinks: Vec<u32>,
}

#[derive(Debug, Clone)]
pub enum SonusmixMsg {
    AddNode(u32, PortKind),
}

pub struct SonusmixReducer(
    SharedState<Option<SonusmixMsg>>,
    SharedState<Arc<SonusmixState>>,
);

impl SonusmixReducer {
    pub fn emit(&self, msg: SonusmixMsg) {
        let mut state = { self.1.read().as_ref().clone() };
        match msg {
            SonusmixMsg::AddNode(id, list) => match list {
                PortKind::Source => state.active_sources.push(id),
                PortKind::Sink => state.active_sinks.push(id),
            },
        }
        {
            *self.0.write() = Some(msg);
        }
        *self.1.write() = Arc::new(state);
    }

    pub fn subscribe_msg<Msg, F>(&self, sender: &relm4::Sender<Msg>, f: F)
    where
        F: Fn(&SonusmixMsg) -> Msg + 'static + Send + Sync,
        Msg: Send + 'static,
    {
        self.0
            .subscribe_optional(sender, move |msg| msg.as_ref().map(&f));
    }

    pub fn subscribe<Msg, F>(&self, sender: &relm4::Sender<Msg>, f: F) -> Arc<SonusmixState>
    where
        F: Fn(Arc<SonusmixState>) -> Msg + 'static + Send + Sync,
        Msg: Send + 'static,
    {
        self.1.subscribe(sender, move |state| f(state.clone()));
        self.1.read().clone()
    }
}
