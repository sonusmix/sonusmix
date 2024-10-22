mod components;
mod pipewire_api;
mod state;
mod tray;

use std::sync::atomic::{AtomicBool, Ordering};

use components::app::App;
use gtk::gio::ApplicationFlags;
use log::debug;
use pipewire_api::PipewireHandle;
use relm4::RelmApp;
use tray::SonusmixTray;

const SONUSMIX_APP_ID: &str = "org.sonusmix.Sonusmix";
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
static GLOBAL_EXIT: AtomicBool = AtomicBool::new(false);

fn main() {
    // Setting env vars will be marked as unsafe in the 2024 edition, because it may race with
    // other threads and there isn't really a way to synchronize it. So, we do it first thing in
    // main(), before doing anything else that uses env variables or starting any other threads.
    let _ = dotenvy::from_filename("dev-env");

    colog::default_builder()
        .filter_level(log::LevelFilter::Debug)
        .init();

    debug!("Hello, world!");

    let (tx, rx) = std::sync::mpsc::channel();
    let update_fn = state::SonusmixReducer::init(tx.clone());
    // Allow testing the app without needing to connect to Pipewire
    let _pipewire_handle = if std::env::var("SONUSMIX_NO_PIPEWIRE")
        .ok()
        .filter(|v| !v.is_empty())
        .is_none()
    {
        Some(PipewireHandle::init((tx, rx), update_fn).expect("failed to connect to Pipewire"))
    } else {
        None
    };

    let (tray, mut status_rx) = SonusmixTray::new();
    let tray_service = ksni::TrayService::new(tray);
    let tray_handle = tray_service.handle();
    tray_service.spawn();

    loop {
        let gtk_app = gtk::Application::builder()
            .application_id(SONUSMIX_APP_ID)
            .build();
        RelmApp::from_app(gtk_app).run::<App>(status_rx);
        state::SonusmixReducer::save(false, false);

        status_rx = tray_handle.update(|tray| tray.status());
        // I don't really care about performance for this one small part, and SeqCst provides the
        // strongest guarantees, so it's (probably?) the safest
        if GLOBAL_EXIT.load(Ordering::SeqCst)
            || status_rx.recv_sync().expect("System tray service exited") == StatusMsg::Exit
        {
            break;
        }
    }

    state::SonusmixReducer::save_and_exit();
    tray_handle.shutdown();
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusMsg {
    Show,
    Exit,
}
