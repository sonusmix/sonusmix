use iced::{Application, Settings};
mod app;
mod components;
mod theme;

fn main() {
    app::AppContainer::run(Settings::default()).expect("Unable to run application");
}
