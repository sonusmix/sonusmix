use std::{
    cell::OnceCell,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, OnceLock,
    },
    thread::JoinHandle,
};

use relm4::SharedState;

use crate::pipewire_api::{Graph, PortKind, ToPipewireMessage};

use super::{EndpointDescriptor, SonusmixMsg, SonusmixState};

static SONUSMIX_REDUCER: once_cell::sync::OnceCell<SonusmixReducer> =
    once_cell::sync::OnceCell::new();

enum ReducerMsg {
    Update(SonusmixMsg),
    GraphUpdate(Graph),
    Exit,
}

pub struct SonusmixReducer {
    pw_sender: mpsc::Sender<ToPipewireMessage>,
    reducer_sender: mpsc::Sender<ReducerMsg>,
    thread_handle: JoinHandle<()>,
    messages: SharedState<Option<SonusmixMsg>>,
    state: SharedState<Arc<SonusmixState>>,
}

impl SonusmixReducer {
    /// Initializes the reducer and its thread. Returns a function that, when called, will update
    /// the state's copy of the Pipewire graph, diff the state, and send out updates to all
    /// subscribers. May only be called once.
    /// # Panics
    /// This function will panic if it is ever called a second time.
    pub fn init(pw_sender: mpsc::Sender<ToPipewireMessage>) -> impl Fn(Graph) + Send + 'static {
        // Ensure that this function is only ever called once
        static IS_INITIALIZED: AtomicBool = AtomicBool::new(false);
        // I don't really care about performance for this one small part, and SeqCst provides the
        // strongest guarantees, so it's (probably?) the safest
        assert!(
            !IS_INITIALIZED.swap(true, Ordering::SeqCst),
            "SonusmixReducer::init() may only be called once"
        );

        let (tx, rx) = mpsc::channel::<ReducerMsg>();
        let reducer_handle = std::thread::spawn(move || {
            let reducer = SONUSMIX_REDUCER.wait();
            let mut graph = Graph::default();

            for message in rx {
                match message {
                    ReducerMsg::Update(msg) => {
                        let mut state = { reducer.state.read().as_ref().clone() };
                        let messages = state.update(&graph, msg);
                        for message in messages {
                            reducer.pw_sender.send(message);
                        }
                        {
                            // Write the new version of the state
                            *reducer.state.write() = Arc::new(state);
                        }
                    }
                    ReducerMsg::GraphUpdate(new_graph) => {
                        graph = new_graph;
                        let mut state = { reducer.state.read().as_ref().clone() };
                        let messages = state.diff(&graph);
                        for message in messages {
                            reducer.pw_sender.send(message);
                        }
                        {
                            // Write the new version of the state
                            *reducer.state.write() = Arc::new(state);
                        }
                    }
                    ReducerMsg::Exit => {
                        break;
                    }
                }
            }
        });

        // TODO: Use the result value from this instead of the AtomicBool to guarantee the reducer
        // is only initialzed once. This may require sending an exit message to the thread, though.
        let _ = SONUSMIX_REDUCER.set(Self {
            pw_sender,
            reducer_sender: tx,
            thread_handle: reducer_handle,
            messages: SharedState::new(),
            // TODO: initialize from disk
            state: SharedState::new(),
        });

        // Return a function that sends a `GraphUpdate` message when called
        |graph| {
            if let Some(reducer) = SONUSMIX_REDUCER.get() {
                let _ = reducer.reducer_sender.send(ReducerMsg::GraphUpdate(graph));
            }
        }
    }

    pub fn emit(msg: SonusmixMsg) {
        if let Some(reducer) = SONUSMIX_REDUCER.get() {
            let _ = reducer.reducer_sender.send(ReducerMsg::Update(msg));
        }
    }

    /// Subscribe to receive rebroadcasted state update messages. Note that only changes made by
    /// the frontend are reflected in the state update messages, as the reducer does not attempt to
    /// convert Pipewire updates into update messages.
    /// # Returns
    /// Returns the current state.
    /// # Panics
    /// This function will panic if it is called before the reducer has been initialized.
    pub fn subscribe_msg<Msg, F>(sender: &relm4::Sender<Msg>, f: F) -> Arc<SonusmixState>
    where
        F: Fn(&SonusmixMsg) -> Msg + 'static + Send + Sync,
        Msg: Send + 'static,
    {
        let reducer = SONUSMIX_REDUCER
            .get()
            .expect("The reducer must be initialized before subscribing to it");

        reducer
            .messages
            .subscribe_optional(sender, move |msg| msg.as_ref().map(&f));
        reducer.state.read().clone()
    }

    /// Subscribe to receive updates to the Sonusmix state.
    /// # Returns
    /// Returns the current state.
    /// # Panics
    /// This function will panic if it is called before the reducer has been initialized.
    pub fn subscribe<Msg, F>(sender: &relm4::Sender<Msg>, f: F) -> Arc<SonusmixState>
    where
        F: Fn(Arc<SonusmixState>) -> Msg + 'static + Send + Sync,
        Msg: Send + 'static,
    {
        let reducer = SONUSMIX_REDUCER
            .get()
            .expect("The reducer must be initialized before subscribing to it");
        reducer
            .state
            .subscribe(sender, move |state| f(state.clone()));
        reducer.state.read().clone()
    }
}

impl Drop for SonusmixReducer {
    fn drop(&mut self) {
        let _ = self.reducer_sender.send(ReducerMsg::Exit);
    }
}

#[cfg(test)]
mod tests {
    use super::init_reducer;

    #[test]
    #[should_panic]
    fn panics_if_initialized_twice() {
        init_reducer();
        init_reducer();
    }
}
