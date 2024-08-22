use std::sync::Arc;

use anyhow::Context;
use log::error;
use pipewire_api::{Graph, PipewireHandle, PipewireSubscriptionKey};
use tauri::{ipc::Channel, Manager, State};

mod pipewire_api;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::new().build())
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            greet,
            subscribe_to_pipewire,
            update_pipewire_subscriber,
            unsubscribe_from_pipewire,
        ])
        .setup(|app| {
            app.manage(
                PipewireHandle::init().context("Failed to initialize the Pipewire connection")?,
            );
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
fn subscribe_to_pipewire(
    channel: Channel<Arc<Graph>>,
    pipewire: State<PipewireHandle>,
) -> PipewireSubscriptionKey {
    pipewire.subscribe(move |graph| {
        if let Err(err) = channel.send(graph) {
            error!("Error sending Pipewire store update: {err:?}");
        }
    })
}

#[tauri::command]
fn update_pipewire_subscriber(key: PipewireSubscriptionKey, pipewire: State<PipewireHandle>) {
    pipewire.update_subscriber(key);
}

#[tauri::command]
fn unsubscribe_from_pipewire(key: PipewireSubscriptionKey, pipewire: State<PipewireHandle>) {
    pipewire.unsubscribe(key);
}
