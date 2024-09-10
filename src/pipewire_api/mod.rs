mod mainloop;
mod object;
mod pod;
mod store;
mod actions;
mod identifier;

use std::{collections::HashMap, sync::mpsc, thread};

use anyhow::{Context, Result};
use log::error;
use mainloop::init_mainloop;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use object::PortKind;

const SONUSMIX_APP_NAME: &'static str = "sonusmix";

pub struct PipewireHandle {
    pipewire_thread_handle: Option<thread::JoinHandle<()>>,
    adapter_thread_handle: Option<thread::JoinHandle<Result<(), PipewireChannelError>>>,
    pipewire_sender: mpsc::Sender<ToPipewireMessage>,
}

impl PipewireHandle {
    pub fn init(update_fn: impl Fn(Graph) + Send + 'static) -> Result<Self> {
        // TODO: Decide if we actually need a dedicated channel and message type to communicate
        // from Pipewire to the main thread, or if the graph updates are enough
        let (pipewire_thread_handle, pw_sender, pw_receiver) =
            init_mainloop(update_fn).context("Error initializing the Pipewire thread")?;
        let (adapter_thread_handle, adapter_sender) = init_adapter(pw_sender);
        Ok(Self {
            pipewire_thread_handle: Some(pipewire_thread_handle),
            adapter_thread_handle: Some(adapter_thread_handle),
            pipewire_sender: adapter_sender,
        })
    }

    pub fn sender(&self) -> mpsc::Sender<ToPipewireMessage> {
        self.pipewire_sender.clone()
    }
}

impl Drop for PipewireHandle {
    fn drop(&mut self) {
        self.pipewire_sender.send(ToPipewireMessage::Exit);
        if let Some(adapter_thread_handle) = self.adapter_thread_handle.take() {
            if let Err(err) = adapter_thread_handle.join() {
                error!("Adapter thread panicked: {err:?}");
            }
        }
        if let Some(pipewire_thread_handle) = self.pipewire_thread_handle.take() {
            if let Err(err) = pipewire_thread_handle.join() {
                error!("Pipewire thread panicked: {err:?}");
            }
        }
    }
}

pub type Client = object::Client<()>;
pub type Device = object::Device<(), ()>;
pub type Node = object::Node<(), ()>;
pub type Port = object::Port<()>;
pub type Link = object::Link<()>;

#[derive(Debug, Clone, Default)]
pub struct Graph {
    pub clients: HashMap<u32, Client>,
    pub devices: HashMap<u32, Device>,
    pub nodes: HashMap<u32, Node>,
    pub ports: HashMap<u32, Port>,
    pub links: HashMap<u32, Link>,
}

#[derive(Debug)]
pub enum ToPipewireMessage {
    Update,
    // TODO: set channel volumes individually
    ChangeVolume(u32, f32),
    Exit,
}

#[derive(Debug)]
enum FromPipewireMessage {}

#[derive(Error, Debug)]
#[error("failed to send message to Pipewire: {0:?}")]
struct PipewireChannelError(ToPipewireMessage);

/// This thread takes events from a stdlib mpsc channel and puts them into a pipewire::channel,
/// because pipewire::channel uses a synchronous mutex and thus could cause deadlocks if called
/// from async code. This might not be needed, but it'd probably be pretty annoying to debug if it
/// turned out that the small block to send messages is actually a problem.
fn init_adapter(
    pw_sender: pipewire::channel::Sender<ToPipewireMessage>,
) -> (
    thread::JoinHandle<Result<(), PipewireChannelError>>,
    mpsc::Sender<ToPipewireMessage>,
) {
    let (sender, receiver) = mpsc::channel();
    let handle = thread::spawn(move || loop {
        match receiver.recv().unwrap_or(ToPipewireMessage::Exit) {
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
