mod components;
mod pipewire_api;

use components::app::App;
use log::debug;
use pipewire_api::PipewireHandle;
use relm4::RelmApp;

fn main() {
    colog::default_builder()
        .filter_level(log::LevelFilter::Debug)
        .init();

    debug!("Hello, world!");

    let pipewire_handle = PipewireHandle::init().expect("failed to connect to Pipewire");
    let app = RelmApp::new("sonusmix");
    app.run::<App>(pipewire_handle);
}
