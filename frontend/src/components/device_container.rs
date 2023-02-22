use super::device::HardwareSource;
use crate::app::Message;
use crate::theme;
use iced::{
    widget::{
        button, column, container,
        container::{Appearance, StyleSheet},
        scrollable, text, Button, Column, Scrollable, Text,
    },
    Color, Command, Element, Length, Point, Renderer, Size,
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
        column![container(
            column(
                self.devices
                    .iter()
                    .map(|device| device.view().into())
                    .collect::<Vec<Element<Message, Renderer<theme::Theme>>>>(),
            )
            .spacing(20)
        )
        .padding(10)
        .style(theme::Container::Border)]
        .into()
    }
}
