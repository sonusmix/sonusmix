mod components;
mod pipewire_api;
mod state;

use components::app::App;
use log::debug;
use pipewire_api::PipewireHandle;
use relm4::RelmApp;

const APP_IDENTIFIER: &str = "org.sonusmix.Sonusmix";
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

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
    let _pipewire_handle =
        PipewireHandle::init((tx, rx), update_fn).expect("failed to connect to Pipewire");

    let app = RelmApp::new("sonusmix");

    app.run::<App>(());

    // Comment the above lines and uncomment these to test without the frontend
    // let mut s = String::new();
    // let _ = std::io::stdin().read_line(&mut s);
    // std::hint::black_box(pipewire_handle);
}
