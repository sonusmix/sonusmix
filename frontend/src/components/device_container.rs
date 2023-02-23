use super::device::HardwareSource;
use crate::app::Message;
use crate::theme;
use iced::{
    widget::{column, container},
    Element, Length, Renderer,
};

/// This is a container for a device
pub struct DeviceContainer {
    devices: Vec<HardwareSource>,
}

impl DeviceContainer {
    pub fn new(devices: Vec<HardwareSource>) -> DeviceContainer {
        DeviceContainer { devices }
    }

    pub fn view(&self) -> Element<Message, Renderer<theme::Theme>> {
        container(
            column(
                self.devices
                    .iter()
                    .map(|device| device.view().into())
                    .collect::<Vec<Element<Message, Renderer<theme::Theme>>>>(),
            )
            .spacing(20),
        )
        .padding(10)
        .height(Length::Fill)
        .width(Length::Fill)
        .style(theme::Container::Border)
        .into()
    }
}
