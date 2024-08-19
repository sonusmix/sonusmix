mod mainloop;
mod object;
mod store;
mod tree;

use std::sync::{Arc, RwLock};

use anyhow::{Context, Result};
use mainloop::init_mainloop;
use object::{Node, ObjectConvertError, PipewireObject};
use store::Store;
use thiserror::Error;
use tokio::sync::mpsc;

const SONUSMIX_APP_NAME: &'static str = "sonusmix";

pub struct PipewireHandle {
    pipewire_thread_handle: std::thread::JoinHandle<()>,
    adapter_thread_handle: std::thread::JoinHandle<Result<(), PipewireChannelError>>,
    store: Arc<RwLock<Store>>,
}

impl PipewireHandle {
    pub fn init() -> Result<Self> {
        let store = Arc::new(RwLock::new(Store::new()));
        let (pipewire_thread_handle, pw_sender, pw_receiver) =
            init_mainloop(store.clone()).context("Error initializing the Pipewire thread")?;
        let (adapter_thread_handle, adapter_receiver) = init_adapter(pw_sender);
        Ok(Self {
            pipewire_thread_handle,
            adapter_thread_handle,
            store,
        })
    }

    pub fn get_nodes(&self) -> Vec<PipewireObject<Node>> {
        self.store
            .read()
            .expect("store lock poisoned")
            .nodes
            .values()
            .cloned()
            .collect()
    }
}

#[derive(Debug)]
enum ToPipewireMessage {
    Exit,
}

#[derive(Debug)]
enum FromPipewireMessage {}

#[derive(Error, Debug)]
#[error("failed to send message to Pipewire: {0:?}")]
struct PipewireChannelError(ToPipewireMessage);

/// This thread takes events from a stdlib mpsc channel and puts them into a pipewire::channel,
/// because pipewire::channel uses a synchronous mutex and thus could cause deadlocks if called
/// from async code. This might not be needed, but it'd probably be annoying to debug if it turned
/// out that the small block to send messages is actually a problem.
fn init_adapter(
    pw_sender: pipewire::channel::Sender<ToPipewireMessage>,
) -> (
    std::thread::JoinHandle<Result<(), PipewireChannelError>>,
    mpsc::UnboundedSender<ToPipewireMessage>,
) {
    let (sender, mut receiver) = mpsc::unbounded_channel();
    let handle = std::thread::spawn(move || loop {
        match receiver.blocking_recv().unwrap_or(ToPipewireMessage::Exit) {
            ToPipewireMessage::Exit => {
                break pw_sender
                    .send(ToPipewireMessage::Exit)
                    .map_err(PipewireChannelError);
            }
            message => pw_sender.send(message).map_err(PipewireChannelError)?,
        }
    });
    (handle, sender)
}
