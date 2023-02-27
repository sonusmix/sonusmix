use crate::components::grid::Grid;
use crate::components::{application_container, hardware_container, virtual_container};
use crate::theme::Theme;
use iced::{application, executor, widget::container, Application, Command, Renderer};

#[derive(Debug, Clone)]
pub enum Message {}

/// This is the main application container
pub struct AppContainer {
    hardware_source: hardware_container::HardwareSource,
    hardware_sink: hardware_container::HardwareSink,
    application_source: application_container::ApplicationSource,
    application_sink: application_container::ApplicationSink,
    virtual_sink_source: virtual_container::Virtual,
}

#[derive(Default, Debug, Copy, Clone)]
pub enum AppContainerStyle {
    #[default]
    Regular,
}

impl application::StyleSheet for Theme {
    type Style = AppContainerStyle;

    fn appearance(&self, _style: &Self::Style) -> application::Appearance {
        application::Appearance {
            background_color: self.palette().background,
            text_color: self.palette().foreground,
        }
    }
}

impl Application for AppContainer {
    type Theme = Theme;
    type Executor = executor::Default;
    type Message = Message;
    type Flags = ();

    fn new(_flags: ()) -> (Self, Command<Self::Message>) {
        (
            AppContainer {
                hardware_source: hardware_container::HardwareSource::new(),
                hardware_sink: hardware_container::HardwareSink::new(),
                application_source: application_container::ApplicationSource::new(),
                application_sink: application_container::ApplicationSink::new(),
                virtual_sink_source: virtual_container::Virtual::new(),
            },
            Command::none(),
        )
    }

    fn title(&self) -> String {
        String::from("Sonusmix")
    }

    fn update(&mut self, _message: Self::Message) -> Command<Self::Message> {
        Command::none()
    }

    fn view(&self) -> iced::Element<Self::Message, Renderer<Self::Theme>> {
        container(Grid::new([
            self.hardware_source.view(),
            self.hardware_sink.view(),
            self.application_source.view(),
            self.virtual_sink_source.view(),
            self.application_sink.view(),
        ]))
        .into()
    }
}
