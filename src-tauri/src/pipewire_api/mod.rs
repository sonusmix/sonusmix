mod mainloop;
mod object;
mod store;

use std::{
    future::Future,
    sync::{Arc, Mutex, RwLock},
    thread,
};

use anyhow::{Context, Result};
use log::error;
use mainloop::init_mainloop;
use object::{Client, Device, Link, Node, Port};
use serde::{Deserialize, Serialize};
use slotmap::{new_key_type, DenseSlotMap};
use thiserror::Error;
use tokio::sync::mpsc;

const SONUSMIX_APP_NAME: &'static str = "sonusmix";

type Subscriptions =
    Arc<Mutex<DenseSlotMap<PipewireSubscriptionKey, Box<dyn Fn(Arc<Graph>) + Send + 'static>>>>;

pub struct PipewireHandle {
    pipewire_thread_handle: Option<thread::JoinHandle<()>>,
    adapter_thread_handle: Option<thread::JoinHandle<Result<(), PipewireChannelError>>>,
    // pipewire_listener_handle: Option<tauri::async_runtime::JoinHandle<()>>,
    pipewire_sender: mpsc::UnboundedSender<ToPipewireMessage>,
    // subscription_sender: mpsc::UnboundedSender<SubscriptionMessage>,
    subscriptions: Subscriptions,
}

impl PipewireHandle {
    pub fn init() -> Result<Self> {
        let subscriptions = Arc::new(Mutex::new(DenseSlotMap::with_key()));
        let (pipewire_thread_handle, pw_sender, pw_receiver) = init_mainloop(subscriptions.clone())
            .context("Error initializing the Pipewire thread")?;
        let (adapter_thread_handle, adapter_sender) = init_adapter(pw_sender);
        // let (subscription_sender, pipewire_listener_handle) =
        //     init_pipewire_listener(store.clone(), subscriptions.clone(), pw_receiver);
        Ok(Self {
            pipewire_thread_handle: Some(pipewire_thread_handle),
            adapter_thread_handle: Some(adapter_thread_handle),
            // pipewire_listener_handle: Some(pipewire_listener_handle),
            pipewire_sender: adapter_sender,
            // subscription_sender,
            subscriptions,
        })
    }

    pub fn subscribe(&self, f: impl Fn(Arc<Graph>) + Send + 'static) -> PipewireSubscriptionKey {
        // Subscribe to changes
        let key = {
            self.subscriptions
                .lock()
                .expect("subscriptions lock poisoned")
                .insert(Box::new(f))
        };

        // Send one update now
        self.update_subscriber(key);
        key
    }

    pub fn update_subscriber(&self, key: PipewireSubscriptionKey) {
        self.pipewire_sender
            .send(ToPipewireMessage::UpdateOne(key))
            .expect("Pipewire channel closed");
    }

    pub fn unsubscribe(&self, key: PipewireSubscriptionKey) {
        self.subscriptions
            .lock()
            .expect("subscriptions lock poisoned")
            .remove(key);
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
        // TODO: Figure out how to dcleanly cancel and join/await the listener task, if anything
        // actually needs to be done at all
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Graph {
    pub clients: Vec<Client<()>>,
    pub devices: Vec<Device<()>>,
    pub nodes: Vec<Node<()>>,
    pub ports: Vec<Port<()>>,
    pub links: Vec<Link<()>>,
}

#[derive(Debug)]
enum ToPipewireMessage {
    UpdateOne(PipewireSubscriptionKey),
    Exit,
}

#[derive(Debug)]
enum FromPipewireMessage {}

#[derive(Debug)]
enum SubscriptionMessage {
    UpdateOne(PipewireSubscriptionKey),
    Exit,
}

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
    mpsc::UnboundedSender<ToPipewireMessage>,
) {
    let (sender, mut receiver) = mpsc::unbounded_channel();
    let handle = thread::spawn(move || loop {
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

// fn init_pipewire_listener(
//     store: Arc<RwLock<Store>>,
//     subscriptions: Subscriptions,
//     mut pw_receiver: mpsc::UnboundedReceiver<FromPipewireMessage>,
// ) -> (
//     mpsc::UnboundedSender<SubscriptionMessage>,
//     tauri::async_runtime::JoinHandle<()>,
// ) {
//     let (tx, mut control_receiver) = tokio::sync::mpsc::unbounded_channel();

//     let handle = tauri::async_runtime::spawn(async move {
//         loop {
//             tokio::select! {
//                 message = pw_receiver.recv() => {
//                     if let Some(message) = message {
//                         match message {
//                             FromPipewireMessage::Update => {
//                                 let graph =
//                                     { Arc::new(store.read().expect("store lock poisoned").dump_graph()) };

//                                 for subscription in subscriptions
//                                     .lock()
//                                     .expect("subscriptions lock poisoned")
//                                     .values()
//                                 {
//                                     subscription(graph.clone());
//                                 }
//                             }
//                         }
//                     }
//                 }
//                 message = control_receiver.recv() => {
//                     if let Some(message) = message {
//                         match message {
//                             SubscriptionMessage::UpdateOne(key) => {
//                                 if let Some(f) = subscriptions.lock().expect("subscriptions lock poisoned").get(key) {
//                                     f({ Arc::new(store.read().expect("store lock poisoned").dump_graph()) });
//                                 }
//                             }
//                             SubscriptionMessage::Exit => break,
//                         }
//                     }
//                 }
//             }
//         }
//     });
//     (tx, handle)
// }

new_key_type! {
    pub struct PipewireSubscriptionKey;
}
