use crate::app::Message;
use crate::theme;
use iced::{
    widget::{container, text},
    Element, Length, Renderer,
};

/// Virtual device container
pub struct Virtual;

impl Virtual {
    pub fn new() -> Virtual {
        Virtual
    }

    pub fn view(&self) -> Element<Message, Renderer<theme::Theme>> {
        container(text("VIRTUAL DEVICES"))
            .padding(10)
            .height(Length::Fill)
            .width(Length::Fill)
            .style(theme::Container::Border)
            .into()
    }
}
