use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex, OnceLock, RwLock,
    },
    thread::JoinHandle,
    time::Duration,
};

use log::error;
use relm4::SharedState;

use crate::{
    pipewire_api::{Graph, ToPipewireMessage},
    state::persistence::{autosave_task, PersistentSettings, PersistentState},
};

use super::{settings::SonusmixSettings, SonusmixMsg, SonusmixOutputMsg, SonusmixState};

static SONUSMIX_REDUCER: RwLock<OnceLock<SonusmixReducer>> = RwLock::new(OnceLock::new());
pub static SONUSMIX_SETTINGS: SharedState<SonusmixSettings> = SharedState::new();
const GRAPH_UPDATE_DEBOUNCE_TIME: f64 = 1.0 / 60.0;

#[derive(Debug, Clone)]
enum ReducerMsg {
    Update(SonusmixMsg),
    GraphUpdate(Graph),
    SettingsChanged,
    Save {
        /// Clearing the state should almost always be followed by closing the app!
        clear_state: bool,
        clear_settings: bool,
    },
    SaveAndExit,
}

pub struct SonusmixReducer {
    pw_sender: mpsc::Sender<ToPipewireMessage>,
    reducer_sender: mpsc::Sender<ReducerMsg>,
    thread_handle: JoinHandle<()>,
    state: SharedState<(Arc<SonusmixState>, Option<SonusmixOutputMsg>)>,
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

        // Lock reducer so the spawned thread has to wait when it starts
        let reducer = SONUSMIX_REDUCER
            .write()
            .expect("panic if reducer lock is poisoned");

        let (tx, rx) = mpsc::channel::<ReducerMsg>();
        let reducer_handle = std::thread::Builder::new()
            .name("state-diff".to_string())
            .spawn(move || {
                let reducer_guard = SONUSMIX_REDUCER
                    .read()
                    .expect("panic if reducer lock is poisoned");
                let reducer = reducer_guard
                    .get()
                    .expect("reducer was not initialized by SonusmixReducer::init()");
                let mut graph = Graph::default();

                let save = || {
                    let state = { reducer.state.read().0.as_ref().clone() };
                    let persistent_state = PersistentState::from_state(state);
                    if let Err(err) = persistent_state.save() {
                        error!("Error saving state: {err:#}");
                    }
                    let settings = { SONUSMIX_SETTINGS.read().clone() };
                    let persistent_settings = PersistentSettings::from_settings(settings);
                    if let Err(err) = persistent_settings.save() {
                        error!("Error saving settings: {err:#}");
                    }
                };

                for message in rx {
                    match message {
                        ReducerMsg::Update(msg) => {
                            let mut state = { reducer.state.read().0.as_ref().clone() };
                            let settings = { SONUSMIX_SETTINGS.read().clone() };
                            let (output_msg, mut messages) =
                                state.update(&graph, msg.clone(), &settings);
                            messages.extend(state.diff(&graph, &settings));
                            for message in messages {
                                reducer
                                    .pw_sender
                                    .send(message)
                                    .expect("Failed to send message to Pipewire thread");
                            }
                            let state = (Arc::new(state), output_msg);
                            {
                                // Write the new version of the state
                                *reducer.state.write() = state;
                            }
                        }
                        ReducerMsg::GraphUpdate(new_graph) => {
                            graph = new_graph;
                            let settings = { SONUSMIX_SETTINGS.read().clone() };
                            let mut state = { reducer.state.read().0.as_ref().clone() };
                            let messages = state.diff(&graph, &settings);
                            for message in messages {
                                reducer
                                    .pw_sender
                                    .send(message)
                                    .expect("Failed to send message to Pipewire thread");
                            }
                            let state = (Arc::new(state), None);
                            {
                                // Write the new version of the state
                                *reducer.state.write() = state;
                            }
                        }
                        ReducerMsg::SettingsChanged => {
                            let settings = { SONUSMIX_SETTINGS.read().clone() };
                            let mut state = { reducer.state.read().0.as_ref().clone() };
                            let messages = state.diff(&graph, &settings);
                            for message in messages {
                                reducer
                                    .pw_sender
                                    .send(message)
                                    .expect("Failed to send message to Pipewire thread");
                            }
                            let state = (Arc::new(state), None);
                            {
                                // Write the new version of the state
                                *reducer.state.write() = state;
                            }
                        }
                        ReducerMsg::Save {
                            clear_state,
                            clear_settings,
                        } => {
                            if clear_state {
                                *reducer.state.write() = Default::default();
                            }
                            if clear_settings {
                                *SONUSMIX_SETTINGS.write() = Default::default();
                            }
                            save();
                        }
                        ReducerMsg::SaveAndExit => {
                            save();
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
        {
            match PersistentSettings::load() {
                Ok(persistent_settings) => {
                    *SONUSMIX_SETTINGS.write() = persistent_settings.into_settings();
                }
                Err(err) => {
                    error!("Failed to load settings: {err:#}");
                }
            }
        }

        // TODO: Use the result value from this instead of the AtomicBool to guarantee the reducer
        // is only initialzed once. This may require sending an exit message to the thread, though.
        let _ = reducer.set(Self {
            pw_sender,
            reducer_sender: tx,
            thread_handle: reducer_handle,
            state,
        });

        // Tell the reducer to diff the graph again whenever there is a settings update
        relm4::spawn(async {
            let (tx, rx) = relm4::channel();
            SONUSMIX_SETTINGS.subscribe(&tx, |_| ());
            while rx.recv().await.is_some() {
                if let Some(reducer) = SONUSMIX_REDUCER
                    .read()
                    .expect("panic if reducer lock is poisoned")
                    .get()
                {
                    let _ = reducer.reducer_sender.send(ReducerMsg::SettingsChanged);
                }
            }
        });

        // Start the autosave task
        relm4::spawn(autosave_task());

        // Return a function that, after a short delay, sends a `GraphUpdate` message. The message
        // will contain the most recent of the graphs given by calls of this function during the
        // delay.
        |new_graph| {
            static GRAPH: Mutex<Option<Graph>> = Mutex::new(None);

            let mut graph = GRAPH.lock().expect("graph lock poisoned");
            if let Some(graph) = graph.as_mut() {
                *graph = new_graph;
            } else {
                *graph = Some(new_graph);
                relm4::spawn(async {
                    tokio::time::sleep(Duration::from_secs_f64(GRAPH_UPDATE_DEBOUNCE_TIME)).await;
                    if let Some(graph) = GRAPH.lock().expect("graph lock poisoned").take() {
                        if let Some(reducer) = SONUSMIX_REDUCER
                            .read()
                            .expect("panic if reducer lock is poisoned")
                            .get()
                        {
                            let _ = reducer.reducer_sender.send(ReducerMsg::GraphUpdate(graph));
                        }
                    }
                });
            }
        }
    }

    pub fn emit(msg: SonusmixMsg) {
        if let Some(reducer) = SONUSMIX_REDUCER
            .read()
            .expect("panic if reducer lock is poisoned")
            .get()
        {
            let _ = reducer.reducer_sender.send(ReducerMsg::Update(msg));
        }
    }

    /// Clearing the state should almost always be followed by exiting the app!
    pub fn save(clear_state: bool, clear_settings: bool) {
        if let Some(reducer) = SONUSMIX_REDUCER
            .read()
            .expect("panic if reducer lock is poisoned")
            .get()
        {
            let _ = reducer.reducer_sender.send(ReducerMsg::Save {
                clear_state,
                clear_settings,
            });
        }
    }

    // Save the state to a file, and stop the reducer thread. Any updates after this will not be
    // processed.
    pub fn save_and_exit() {
        // Get a read lock, tell the reducer thread to exit, and then get a write lock in order to
        // join it
        {
            if let Some(reducer) = SONUSMIX_REDUCER
                .read()
                .expect("panic if reducer lock is poisoned")
                .get()
            {
                let _ = reducer.reducer_sender.send(ReducerMsg::SaveAndExit);
            } else {
                return;
            }
        }
        if let Some(reducer) = SONUSMIX_REDUCER
            .write()
            .expect("panic if reducer lock is poisoned")
            .take()
        {
            reducer
                .thread_handle
                .join()
                .expect("panic if reducer thread panicked");
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
        let reducer_guard = SONUSMIX_REDUCER
            .read()
            .expect("panic if reducer lock is poisoned");
        let reducer = reducer_guard
            .get()
            .expect("The reducer must be initialized before subscribing to it");
        reducer
            .state
            .subscribe(sender, move |(state, _)| f(state.clone()));
        let state = reducer.state.read().0.clone();
        state
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
        F: Fn(Arc<SonusmixState>, Option<SonusmixOutputMsg>) -> Msg + 'static + Send + Sync,
        Msg: Send + 'static,
    {
        let reducer_guard = SONUSMIX_REDUCER
            .read()
            .expect("panic if reducer lock is poisoned");
        let reducer = reducer_guard
            .get()
            .expect("The reducer must be initialized before subscribing to it");

        reducer
            .state
            .subscribe(sender, move |(state, msg)| f(state.clone(), msg.clone()));
        let state = reducer.state.read().0.clone();
        state
    }
}

#[cfg(test)]
mod tests {
    use super::SonusmixReducer;

    #[test]
    #[should_panic]
    fn panics_if_initialized_twice() {
        let (tx, _) = std::sync::mpsc::channel();
        let _ = SonusmixReducer::init(tx.clone());
        let _ = SonusmixReducer::init(tx);
    }
}
