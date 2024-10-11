use std::{convert::Infallible, io::Write, process::Command};

use gtk::prelude::*;
use relm4::prelude::*;
use tempfile::TempPath;

const LICENSE_STRING: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/LICENSE"));

pub struct AboutComponent;

#[relm4::component(pub)]
impl SimpleComponent for AboutComponent {
    type Input = Infallible;
    type Output = Infallible;
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

const THIRD_PARTY_LICENSES_HTML: &str =
    include_str!(concat!(env!("OUT_DIR"), "/third_party_licenses.html"));

/// Write the third-party licenses html to a temporary file and open it using `xdg-open`. Returns a
/// handle to the file that will delete it when dropped. Optionally pass a handle that has already
/// been created to avoid creating an additional temp file.
pub fn open_third_party_licenses(path: Option<TempPath>) -> std::io::Result<TempPath> {
    let temp_path = if let Some(path) = path {
        path
    } else {
        let mut temp_file = tempfile::Builder::new()
            .prefix("sonusmix_third_party_licenses-")
            .suffix(".html")
            .tempfile()?;
        temp_file.write_all(THIRD_PARTY_LICENSES_HTML.as_bytes())?;
        temp_file.flush()?;
        temp_file.into_temp_path()
    };
    Command::new("xdg-open")
        .arg(temp_path.canonicalize()?)
        .status()?;
    Ok(temp_path)
}
