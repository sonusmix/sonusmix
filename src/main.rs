mod components;
mod pipewire_api;

use components::app::App;
use log::debug;
use relm4::RelmApp;

fn main() {
    colog::default_builder()
        .filter_level(log::LevelFilter::Debug)
        .init();

    debug!("Hello, world!");

    let app = RelmApp::new("sonusmix");
    app.run::<App>(());
}
