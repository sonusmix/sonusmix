mod components;
mod pipewire_api;
mod state;

use components::app::App;
use log::debug;
use pipewire_api::PipewireHandle;
use relm4::RelmApp;

fn main() {
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
