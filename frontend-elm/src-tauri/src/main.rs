// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Mutex;

struct Counter(Mutex<i32>);

fn main() {
  tauri::Builder::default()
    .manage(Counter(Default::default()))
    .invoke_handler(tauri::generate_handler![increment, decrement])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}

#[tauri::command]
fn increment(state: tauri::State<Counter>) -> i32 {
  let mut counter = state.0.lock().unwrap();
  *counter += 1;
  *counter
}

#[tauri::command]
fn decrement(state: tauri::State<Counter>) -> i32 {
  let mut counter = state.0.lock().unwrap();
  *counter -= 1;
  *counter
}