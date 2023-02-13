use std::sync::mpsc as std_channel;
use std::time::Duration;

use events::PipewireEvent;
use tokio::sync::mpsc as tk_channel;

use log::{debug, info};

use crate::events::ExitSignal;
use crate::{controller::PipewireController, events::ControllerEvent};

mod controller;
mod device;
mod error;
mod events;

fn main() {
    simple_logger::SimpleLogger::new()
        .with_level(log::LevelFilter::Debug)
        .env()
        .init()
        .expect("Could not initialize logger");

    info!("Hello, world!");

    // TODO: single- or multi-threaded?
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("Unable to start tokio runtime");
    debug!("Tokio runtime started");

    let PipewireController { tx, rx } = PipewireController::start();
    debug!("Pipewire controller started");

    let (exit1, exit2) = ExitSignal::pair();

    runtime.block_on(async move {
        let command_handle = tokio::spawn(command_listener(exit1, tx.clone()));
        debug!("Started command listener");

        let event_handle = tokio::spawn(event_listener(exit2, tx.clone(), rx));
        debug!("Started event listener");

        command_handle.await;
        event_handle.await;
    })
}

struct ExitMessage;

/// Listens for and handles commands from clients.
async fn command_listener(mut exit: ExitSignal, tx: std_channel::Sender<ControllerEvent>) {
    debug!("Hello from command_listener!");
    tokio::time::sleep(Duration::from_secs(5)).await;
    info!("Created virtual sink");
    tx.send(ControllerEvent::CreateSink(
        "pulsemeeter-daemon".to_string(),
    ))
    .unwrap();
    exit.wait().await;
}

/// Listens for and handles events from the pipewire controller.
async fn event_listener(
    mut exit: ExitSignal,
    tx: std_channel::Sender<ControllerEvent>,
    mut rx: tk_channel::UnboundedReceiver<PipewireEvent>,
) {
    debug!("Hello from event_listener!");
    loop {
        tokio::select! {
            Some(event) = rx.recv() => if let PipewireEvent::NewGlobal(s) = event {
                debug!("{}", s);
            },
            _ = exit.wait() => break,
        };
    }
}
