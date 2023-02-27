use iced::widget::{button, container, text};
use iced::{color, Color};

// This file is mainly a wrapper around the standard Appearance to support own customizations

#[derive(Default, Debug, Clone, Copy)]
pub enum Theme {
    #[default]
    Dark,
}

pub struct ColorBase {
    pub background: Color,
    pub foreground: Color,

    pub primary: Color,
    pub secondary: Color,
    pub secondary_low: Color,
}

impl Theme {
    pub fn palette(&self) -> ColorBase {
        match self {
            Self::Dark => ColorBase {
                background: color!(0x404258),
                foreground: color!(0x474E68),
                primary: color!(0x6B728E),
                secondary: color!(0x50577A),
                secondary_low: Color::from_rgba(80.0, 87.0, 122.0, 50.0),
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
                border_color: self.palette().foreground,
                ..container::Appearance::default()
            },
        }
    }
}

#[derive(Default, Debug, Clone, Copy)]
pub enum Button {
    #[default]
    Default,
}

impl button::StyleSheet for Theme {
    type Style = Button;

    fn active(&self, style: &Self::Style) -> button::Appearance {
        match style {
            Button::Default => button::Appearance::default(),
        }
    }
}

#[derive(Default, Debug, Clone, Copy)]
pub enum Text {
    #[default]
    Default,
}

impl text::StyleSheet for Theme {
    type Style = Text;

    fn appearance(&self, style: Self::Style) -> text::Appearance {
        match style {
            Text::Default => text::Appearance {
                color: Some(self.palette().primary),
            },
        }
    }
}
