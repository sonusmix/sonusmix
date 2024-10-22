mod components;
mod pipewire_api;
mod state;
mod tray;

use std::{
    convert::Infallible,
    sync::atomic::{AtomicI32, Ordering},
};

use components::app::{App, Msg};
use log::debug;
use pipewire_api::PipewireHandle;
use relm4::{gtk::prelude::*, prelude::*, MessageBroker, Sender};
use state::{settings::SonusmixSettings, SonusmixReducer, SONUSMIX_SETTINGS};
use tray::SonusmixTray;

const SONUSMIX_APP_ID: &str = "org.sonusmix.Sonusmix";
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

    let gtk_app = gtk::Application::builder()
        .application_id(SONUSMIX_APP_ID)
        .build();
    RelmApp::from_app(gtk_app)
        .with_broker(&MAIN_BROKER)
        .visible_on_activate(false)
        .run::<Main>(());
}

/// This a dummy window which stays invisible for the lifetime of the app, since relm4 requires
/// there to be a root window at all times. It handles the lifecycle code for the app (i.e.
/// initializing Pipewire, loading state, handling the tray icon).
pub struct Main {
    settings: SonusmixSettings,
    _pipewire_handle: Option<PipewireHandle>,
    tray_handle: ksni::Handle<SonusmixTray>,
    app: Option<Controller<App>>,
}

#[derive(Debug, Clone)]
pub enum MainMsg {
    #[doc(hidden)]
    UpdateSettings(SonusmixSettings),
    Show,
    Hide,
    Exit,
}

static MAIN_BROKER: MessageBroker<MainMsg> = MessageBroker::new();
static APP_WINDOW_ID: AtomicI32 = AtomicI32::new(0);

#[relm4::component(pub)]
impl SimpleComponent for Main {
    type Init = ();
    type Input = MainMsg;
    type Output = Infallible;

    view! {
        gtk::Window {
            set_visible: false,
        }
    }

    fn init(_init: (), root: Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {
        let (tx, rx) = std::sync::mpsc::channel();
        let update_fn = state::SonusmixReducer::init(tx.clone());
        // Allow testing the app without needing to connect to Pipewire
        let pipewire_handle = if std::env::var("SONUSMIX_NO_PIPEWIRE")
            .ok()
            .filter(|v| !v.is_empty())
            .is_none()
        {
            Some(PipewireHandle::init((tx, rx), update_fn).expect("failed to connect to Pipewire"))
        } else {
            None
        };

        SONUSMIX_SETTINGS.subscribe(sender.input_sender(), |settings| {
            MainMsg::UpdateSettings(settings.clone())
        });
        let settings = { SONUSMIX_SETTINGS.read().clone() };

        let app = (!settings.start_collapsed_to_tray).then(|| {
            App::builder()
                .update_root(|window| relm4::main_application().add_window(window))
                .launch(())
                .detach()
        });

        let tray_service = ksni::TrayService::new(SonusmixTray::new(sender.input_sender().clone()));
        let tray_handle = tray_service.handle();
        tray_service.spawn();

        let model = Main {
            settings,
            _pipewire_handle: pipewire_handle,
            tray_handle,
            app,
        };

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, _sender: ComponentSender<Self>) {
        match msg {
            MainMsg::UpdateSettings(settings) => {
                self.settings = settings;
            }
            MainMsg::Show => {
                if let Some(ref app) = self.app {
                    app.emit(Msg::BringToTop);
                } else {
                    self.app = Some(
                        App::builder()
                            .update_root(|window| relm4::main_application().add_window(window))
                            .launch(())
                            .detach(),
                    );
                }
            }
            MainMsg::Hide => {
                if let Some(ref app) = self.app {
                    app.widget().close();
                    self.app = None;
                    APP_WINDOW_ID.store(0, Ordering::Release);
                }
                SonusmixReducer::save(false, false);
            }
            MainMsg::Exit => {
                relm4::main_application().quit();
            }
        }
    }

    fn shutdown(&mut self, _widgets: &mut Self::Widgets, _output: Sender<Self::Output>) {
        SonusmixReducer::save_and_exit();
        self.tray_handle.shutdown();
    }
}
