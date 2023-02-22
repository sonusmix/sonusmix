use iced::{color, Color};
use iced::widget::{container, button, text};

// This file is mainly a wrapper around the standard Appearance to support own customizations

#[derive(Default, Debug, Clone, Copy)]
pub enum Theme {
    #[default]
    Dark,
}

pub struct ColorBase {
    pub background: iced::Color,
    pub foreground: iced::Color,
}

impl Theme {
    pub fn palette(&self) -> ColorBase {
        match self {
            Self::Dark => ColorBase {
                background: color!(0x636e72),
                foreground: color!(0xdfe6e9),
            },
        }
    }
}

#[derive(Default, Debug, Clone, Copy)]
pub enum Container {
    #[default]
    Default,
    Border,
}

impl container::StyleSheet for Theme {
    type Style = Container;

    fn appearance(&self, style: &Self::Style) -> container::Appearance {
        match style {
            Container::Default => container::Appearance::default(),
            Container::Border => container::Appearance {
                border_width: 3.0,
                border_color: Color::from(self.palette().foreground),
                ..container::Appearance::default()
            },
        }
    }
}

#[derive(Default, Debug, Clone, Copy)]
pub enum Button {
    #[default]
    Default
}

impl button::StyleSheet for Theme {
    type Style = Button;

    fn active(&self, style: &Self::Style) -> button::Appearance {
        match style {
            Button::Default => button::Appearance::default()
        }
    }
}

#[derive(Default, Debug, Clone, Copy)]
pub enum Text {
    #[default]
    Default
}

impl text::StyleSheet for Theme {
    type Style = Text;

    fn appearance(&self, style: Self::Style) -> text::Appearance {
        match style {
            Text::Default => text::Appearance::default(),
        }
    }
}
