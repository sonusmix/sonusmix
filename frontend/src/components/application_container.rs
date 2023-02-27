use crate::app::Message;
use crate::theme;
use iced::{
    widget::{container, text},
    Element, Length, Renderer,
};

/// An application source container (playback)
pub struct ApplicationSource;

impl ApplicationSource {
    pub fn new() -> ApplicationSource {
        ApplicationSource
    }

    pub fn view(&self) -> Element<Message, Renderer<theme::Theme>> {
        container(text("APPLICATION SOURCE (PLAYBACK)"))
            .padding(10)
            .height(Length::Fill)
            .width(Length::Fill)
            .style(theme::Container::Border)
            .into()
    }
}

/// An application sink container (recording)
pub struct ApplicationSink;

impl ApplicationSink {
    pub fn new() -> ApplicationSink {
        ApplicationSink
    }

    pub fn view(&self) -> Element<Message, Renderer<theme::Theme>> {
        container(text("APPLICATION SINK (RECORDING)"))
            .padding(10)
            .height(Length::Fill)
            .width(Length::Fill)
            .style(theme::Container::Border)
            .into()
    }
}
