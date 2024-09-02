use relm4::{gtk, ComponentParts, ComponentSender, SimpleComponent};
use gtk::prelude::*;

use crate::LICENSE_STRING;

pub struct AboutComponent;

#[relm4::component(pub)]
impl SimpleComponent for AboutComponent {
    type Input = ();
    type Output = ();
    type Init = ();

    view! {
        gtk::AboutDialog {
            set_visible: true,
            set_program_name: Some("Sonusmix"),
            set_copyright: Some("2023 - 2024"),
            set_authors: &["dacid44 and Fl1tzi"],
            set_website: Some("https://codeberg.org/sonusmix/"),
            set_license: Some(LICENSE_STRING)
        }
    }

    fn init(
        _: Self::Init,
        root: Self::Root,
        _sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = Self;

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }
}
