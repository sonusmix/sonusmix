mod components;
mod pipewire_api;
mod state;
mod state2;

use components::app::App;
use log::debug;
use pipewire_api::PipewireHandle;
use relm4::RelmApp;

fn main() {
    colog::default_builder()
        .filter_level(log::LevelFilter::Debug)
        .init();

    debug!("Hello, world!");

    let update_fn = state::link_pipewire();
    let pipewire_handle = PipewireHandle::init(update_fn).expect("failed to connect to Pipewire");

    let app = RelmApp::new("sonusmix");
    relm4::set_global_css(include_str!("components/app.css"));

    app.run::<App>(pipewire_handle.sender());

    // Comment the above lines and uncomment these to test without the frontend
    // let mut s = String::new();
    // let _ = std::io::stdin().read_line(&mut s);
    // std::hint::black_box(pipewire_handle);
}
