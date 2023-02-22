use crate::app::Message;
use crate::theme;
use iced::{
    widget::{button, column, container, row, scrollable, text, Button, Column, Scrollable, Text},
    Command, Element, Length, Padding, Point, Renderer, Size,
};

pub struct HardwareSource {
    name: String,
}

impl HardwareSource {
    pub fn new(name: String) -> Self {
        Self { name }
    }

    pub fn view(&self) -> Element<Message, Renderer<theme::Theme>> {
        row![container(text(&self.name).size(30)), {
            container(column![
                button(text("my virt 1").style(theme::Text::Default)).style(theme::Button::Default),
                button(text("my virt 2").style(theme::Text::Default)).style(theme::Button::Default),
                button(text("my virt 3").style(theme::Text::Default)).style(theme::Button::Default),
            ])
            .padding(Padding::from([0, 0, 0, 20]))
            .align_x(iced::alignment::Horizontal::Right)
            // button(label).style(theme::Button::Primary).into()
        },]
        .into()
    }
}
