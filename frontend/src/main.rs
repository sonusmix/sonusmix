use iced::{Application, Settings};
mod app;
mod components;
mod theme;

fn main() {
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .compact()
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("Could not init logging");

    app::AppContainer::run(Settings::default()).expect("Unable to run application");
}
