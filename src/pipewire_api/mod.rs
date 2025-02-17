mod identifier;
mod mainloop;
#[cfg(not(test))]
mod object;
#[cfg(test)]
pub mod object;
mod pod;
mod store;

use std::{collections::HashMap, sync::mpsc, thread};

use anyhow::{Context, Result};
use log::error;
use mainloop::init_mainloop;
use thiserror::Error;

pub use identifier::NodeIdentifier;
pub use object::PortKind;
use ulid::Ulid;

use crate::state::GroupNodeKind;

const SONUSMIX_APP_NAME: &str = "sonusmix";

pub struct PipewireHandle {
    pipewire_thread_handle: Option<thread::JoinHandle<()>>,
    adapter_thread_handle: Option<thread::JoinHandle<Result<(), PipewireChannelError>>>,
    pipewire_sender: mpsc::Sender<ToPipewireMessage>,
}

impl PipewireHandle {
    pub fn init(
        to_pw_channel: (
            mpsc::Sender<ToPipewireMessage>,
            mpsc::Receiver<ToPipewireMessage>,
        ),
        update_fn: impl Fn(Box<Graph>) + Send + 'static,
    ) -> Result<Self> {
        // TODO: Decide if we actually need a dedicated channel and message type to communicate
        // from Pipewire to the main thread, or if the graph updates are enough
        let (pipewire_thread_handle, pw_sender, _from_pw_receiver) =
            init_mainloop(update_fn).context("Error initializing the Pipewire thread")?;
        let adapter_thread_handle = init_adapter(to_pw_channel.1, pw_sender);
        Ok(Self {
            pipewire_thread_handle: Some(pipewire_thread_handle),
            adapter_thread_handle: Some(adapter_thread_handle),
            pipewire_sender: to_pw_channel.0,
        })
    }
}

impl Drop for PipewireHandle {
    fn drop(&mut self) {
        let _ = self.pipewire_sender.send(ToPipewireMessage::Exit);
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

pub type GroupNode = object::GroupNode<(), ()>;
pub type Client = object::Client<()>;
pub type Device = object::Device<(), ()>;
pub type Node = object::Node<(), ()>;
pub type Port = object::Port<()>;
pub type Link = object::Link<()>;

#[derive(Debug, Clone, Default)]
pub struct Graph {
    pub group_nodes: HashMap<Ulid, GroupNode>,
    pub clients: HashMap<u32, Client>,
    pub devices: HashMap<u32, Device>,
    pub nodes: HashMap<u32, Node>,
    pub ports: HashMap<u32, Port>,
    pub links: HashMap<u32, Link>,
}

#[derive(Debug, PartialEq)]
pub enum ToPipewireMessage {
    Update,
    NodeVolume(u32, Vec<f32>),
    NodeMute(u32, bool),
    #[rustfmt::skip]
    #[allow(dead_code)] // This will be used for individual port mapping
    CreatePortLink { start_id: u32, end_id: u32 },
    #[rustfmt::skip]
    CreateNodeLinks { start_id: u32, end_id: u32 },
    #[rustfmt::skip]
    #[allow(dead_code)] // This will be used for individual port mapping
    RemovePortLink { start_id: u32, end_id: u32 },
    #[rustfmt::skip]
    RemoveNodeLinks { start_id: u32, end_id: u32 },
    CreateGroupNode(String, Ulid, GroupNodeKind),
    RemoveGroupNode(Ulid),
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
    receiver: mpsc::Receiver<ToPipewireMessage>,
    pw_sender: pipewire::channel::Sender<ToPipewireMessage>,
) -> thread::JoinHandle<Result<(), PipewireChannelError>> {
    thread::spawn(move || loop {
        match receiver.recv().unwrap_or(ToPipewireMessage::Exit) {
            ToPipewireMessage::Exit => {
                break pw_sender
                    .send(ToPipewireMessage::Exit)
                    .map_err(PipewireChannelError);
            }
            message => pw_sender.send(message).map_err(PipewireChannelError)?,
        }
    })
}
