use anyhow::Context;
use pipewire_api::PipewireHandle;
use tauri::{Manager, State};

mod pipewire_api;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::new().build())
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![greet, get_nodes])
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
fn get_nodes(pipewire: State<PipewireHandle>) -> Vec<String> {
    pipewire
        .get_nodes()
        .into_iter()
        .map(|obj| format!("{obj:?}"))
        .collect()
}
