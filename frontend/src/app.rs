use crate::components::device::HardwareSource;
use crate::components::device_container::DeviceContainer;
use crate::components::grid::Grid;
use crate::theme::Theme;
use iced::{application, executor, widget::container, Application, Command, Renderer};

#[derive(Debug, Clone)]
pub enum Message {}

/// This is the main application container
pub struct AppContainer {
    device_container1: DeviceContainer,
    device_container2: DeviceContainer,
    device_container3: DeviceContainer,
    device_container4: DeviceContainer,
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
                device_container1: DeviceContainer::new(Vec::from([
                    HardwareSource::new(String::from("CONTAINER 1")),
                    HardwareSource::new(String::from("SOURCE 2")),
                ])),
                device_container2: DeviceContainer::new(Vec::from([
                    HardwareSource::new(String::from("CONTAINER 2")),
                    HardwareSource::new(String::from("SOURCE 2")),
                ])),
                device_container3: DeviceContainer::new(Vec::from([
                    HardwareSource::new(String::from("CONTAINER 3")),
                    HardwareSource::new(String::from("SOURCE 2")),
                ])),
                device_container4: DeviceContainer::new(Vec::from([
                    HardwareSource::new(String::from("CONTAINER 4")),
                    HardwareSource::new(String::from("SOURCE 2")),
                ])),
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
            self.device_container1.view(),
            self.device_container2.view(),
            self.device_container3.view(),
            self.device_container4.view(),
        ]))
        .into()
    }
}
