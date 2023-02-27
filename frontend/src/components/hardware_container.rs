use crate::app::Message;
use crate::theme;
use iced::{
    widget::{container, text},
    Element, Length, Renderer,
};

/// Hardware source container (microphones)
pub struct HardwareSource;

impl HardwareSource {
    pub fn new() -> HardwareSource {
        HardwareSource
    }

    pub fn view(&self) -> Element<Message, Renderer<theme::Theme>> {
        container(text("MICROPHONE / HARDWARE SOURCE"))
            .padding(10)
            .height(Length::Fill)
            .width(Length::Fill)
            .style(theme::Container::Border)
            .into()
    }
}

/// Hardware sink container (speaker)
pub struct HardwareSink;

impl HardwareSink {
    pub fn new() -> HardwareSink {
        HardwareSink
    }

    pub fn view(&self) -> Element<Message, Renderer<theme::Theme>> {
        container(text("HARDWARE SOURCE"))
            .padding(10)
            .height(Length::Fill)
            .width(Length::Fill)
            .style(theme::Container::Border)
            .into()
    }
}
