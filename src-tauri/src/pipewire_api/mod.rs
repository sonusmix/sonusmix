mod mainloop;

use anyhow::{Context, Result};
use mainloop::init_mainloop;
use thiserror::Error;
use tokio::sync::mpsc;

pub struct PipewireHandle {
    pipewire_thread_handle: std::thread::JoinHandle<()>,
    adapter_thread_handle: std::thread::JoinHandle<Result<(), PipewireChannelError>>,
}

impl PipewireHandle {
    pub fn init() -> Result<Self> {
        let (pipewire_thread_handle, pw_sender, pw_receiver) =
            init_mainloop().context("Error initializing the Pipewire thread")?;
        let (adapter_thread_handle, adapter_receiver) = init_adapter(pw_sender);
        Ok(Self {
            pipewire_thread_handle,
            adapter_thread_handle,
        })
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
