use log::debug;
use relm4::prelude::*;
use relm4::gtk::{self, prelude::*};
mod components;

use components::about::AboutComponent;

mod pipewire_api;

const LICENSE_STRING: &'static str = include_str!("../LICENSE");

#[derive(Default)]
struct App {
    about_component: Option<Controller<AboutComponent>>
}

#[derive(Debug)]
enum Msg {
    OpenAbout
}

#[relm4::component]
impl SimpleComponent for App {
    type Init = ();
    type Input = Msg;
    type Output = ();

    view! {
        gtk::Window {
            set_title: Some("Sonusmix"),
            set_default_size: (800, 600),

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 10,
                set_margin_all: 10,

                append = &gtk::Label {
                    set_markup: r#"<span size="xx-large">Hello from Sonusmix!</span>"#,
                },
                append = &gtk::Button {
                    set_label: "About",
                    connect_clicked[sender] => move |_| {
                        sender.input(Msg::OpenAbout)
                    }
                }
            }
        }
    }

    fn init(_init: (), root: Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {
        let model = App::default();
        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, _sender: ComponentSender<Self>) {
        match msg {
            Msg::OpenAbout => self.about_component = Some(AboutComponent::builder().launch(()).detach())
        };
    }
}

fn main() {
    colog::default_builder()
        .filter_level(log::LevelFilter::Debug)
        .init();

    debug!("Hello, world!");

    let app = RelmApp::new("sonusmix");
    app.run::<App>(());
}
