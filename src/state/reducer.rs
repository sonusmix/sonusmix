use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    thread::JoinHandle,
    time::Instant,
};

use log::{debug, error};
use relm4::SharedState;

use crate::{
    pipewire_api::{Graph, ToPipewireMessage},
    state::persistence::{autosave_task, PersistentState},
};

use super::{SonusmixMsg, SonusmixState};

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
    state: SharedState<(Arc<SonusmixState>, Option<SonusmixMsg>)>,
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
        let reducer_handle = std::thread::Builder::new()
            .name("state-diff".to_string())
            .spawn(move || {
                let reducer = SONUSMIX_REDUCER.wait();
                let mut graph = Graph::default();

                for message in rx {
                    match message {
                        ReducerMsg::Update(msg) => {
                            let mut state = { reducer.state.read().0.as_ref().clone() };
                            let messages = state.update(&graph, msg.clone());
                            for message in messages {
                                reducer.pw_sender.send(message);
                            }
                            let state = (Arc::new(state), Some(msg));
                            {
                                // Write the new version of the state
                                *reducer.state.write() = state;
                            }
                        }
                        ReducerMsg::GraphUpdate(new_graph) => {
                            graph = new_graph;
                            let mut state = { reducer.state.read().0.as_ref().clone() };
                            let t0 = Instant::now();
                            let messages = state.diff(&graph);
                            let t1 = Instant::now();
                            for message in messages {
                                reducer.pw_sender.send(message);
                            }
                            let state = (Arc::new(state), None);
                            {
                                // Write the new version of the state
                                *reducer.state.write() = state;
                            }
                        }
                        ReducerMsg::Exit => {
                            let state = { reducer.state.read().0.as_ref().clone() };
                            let persistent_state = PersistentState::from_state(state);
                            if let Err(err) = persistent_state.save() {
                                error!("Error saving state: {err:#}");
                            }
                            break;
                        }
                    }
                }
            })
            .expect("failed to spawn state diff thread");

        let state = SharedState::new();
        // TODO: Inform the user in the UI if this failed
        match PersistentState::load() {
            Ok(persistent_state) => {
                *state.write_inner() = (Arc::new(persistent_state.into_state()), None);
            }
            Err(err) => {
                error!("Failed to load persistent state: {err:#}");
            }
        }

        // TODO: Use the result value from this instead of the AtomicBool to guarantee the reducer
        // is only initialzed once. This may require sending an exit message to the thread, though.
        let _ = SONUSMIX_REDUCER.set(Self {
            pw_sender,
            reducer_sender: tx,
            thread_handle: reducer_handle,
            // TODO: initialize from disk
            state,
        });

        // Start the autosave task
        relm4::spawn(autosave_task());

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

    // Save the state to a file, and stop the reducer thread. Any updates after this will not be
    // processed.
    pub fn save_and_exit() {
        if let Some(reducer) = SONUSMIX_REDUCER.get() {
            let _ = reducer.reducer_sender.send(ReducerMsg::Exit);
        }
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
            .subscribe(sender, move |(state, _)| f(state.clone()));
        reducer.state.read().0.clone()
    }

    /// Subscribe to receive updates to the Sonusmix state, along with a copy of the message that
    /// caused the update, if there was one. Note that only changes made by the frontend will
    /// include a state update message, as the reducer does not attempt to convert Pipewire updates
    /// into state update messages.
    /// # Returns
    /// Returns the current state.
    /// # Panics
    /// This function will panic if it is called before the reducer has been initialized.
    pub fn subscribe_msg<Msg, F>(sender: &relm4::Sender<Msg>, f: F) -> Arc<SonusmixState>
    where
        F: Fn(Arc<SonusmixState>, Option<SonusmixMsg>) -> Msg + 'static + Send + Sync,
        Msg: Send + 'static,
    {
        let reducer = SONUSMIX_REDUCER
            .get()
            .expect("The reducer must be initialized before subscribing to it");

        reducer
            .state
            .subscribe(sender, move |(state, msg)| f(state.clone(), msg.clone()));
        reducer.state.read().0.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::SonusmixReducer;

    #[test]
    #[should_panic]
    fn panics_if_initialized_twice() {
        let (tx, _) = std::sync::mpsc::channel();
        SonusmixReducer::init(tx.clone());
        SonusmixReducer::init(tx);
    }
}
