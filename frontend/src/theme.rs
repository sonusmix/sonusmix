use iced::widget::{button, container, text, pane_grid, toggler, slider, checkbox};
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

#[derive(Default, Debug, Clone, Copy)]
pub enum PaneGrid {
    #[default]
    Default,
}

impl pane_grid::StyleSheet for Theme {
    type Style = PaneGrid;

    fn picked_split(&self, style: &Self::Style) -> Option<pane_grid::Line> {
        None
    }

    fn hovered_split(&self, style: &Self::Style) -> Option<pane_grid::Line> {
        None
    }
}

#[derive(Default, Debug, Clone, Copy)]
pub enum Slider {
    #[default]
    Default,
}

impl slider::StyleSheet for Theme {
    type Style = Slider;

    fn active(&self, style: &Self::Style) -> iced::widget::vertical_slider::Appearance {
        match style {
            Slider::Default => slider::Appearance {
                rail_colors: (Color::from_rgb(1.0, 0.0, 0.0), Color::from_rgb(0.0, 1.0, 0.0)),
                handle: slider::Handle {
                    shape: slider::HandleShape::Circle { radius: 10.0 },
                    color: Color::from_rgb(0.0, 0.0, 1.0),
                    border_width: 0.0,
                    border_color: Color::TRANSPARENT,
                },
            }
        }
    }

    fn hovered(&self, style: &Self::Style) -> iced::widget::vertical_slider::Appearance {
        match style {
            Slider::Default => slider::Appearance {
                rail_colors: (Color::from_rgb(1.0, 0.0, 0.0), Color::from_rgb(0.0, 1.0, 0.0)),
                handle: slider::Handle {
                    shape: slider::HandleShape::Circle { radius: 10.0 },
                    color: Color::from_rgb(0.0, 0.0, 1.0),
                    border_width: 0.0,
                    border_color: Color::TRANSPARENT,
                },
            }
        }
    }

    fn dragging(&self, style: &Self::Style) -> iced::widget::vertical_slider::Appearance {
        match style {
            Slider::Default => slider::Appearance {
                rail_colors: (Color::from_rgb(1.0, 0.0, 0.0), Color::from_rgb(0.0, 1.0, 0.0)),
                handle: slider::Handle {
                    shape: slider::HandleShape::Circle { radius: 10.0 },
                    color: Color::from_rgb(0.0, 0.0, 1.0),
                    border_width: 0.0,
                    border_color: Color::TRANSPARENT,
                },
            }
        }
    }
}

#[derive(Default, Debug, Clone, Copy)]
pub enum Toggler {
    #[default]
    Default,
}

impl toggler::StyleSheet for Theme {
    type Style = Toggler;

    fn active(&self, style: &Self::Style, is_active: bool) -> toggler::Appearance {
        match style {
            Toggler::Default => toggler::Appearance {
                background: self.palette().primary,
                background_border: None,
                foreground: self.palette().foreground,
                foreground_border: None,
            },
        }
    }

    fn hovered(&self, style: &Self::Style, is_active: bool) -> toggler::Appearance {
        match style {
            Toggler::Default => toggler::Appearance {
                background: self.palette().secondary,
                background_border: None,
                foreground: self.palette().foreground,
                foreground_border: None,
            },
        }
    }
}

#[derive(Default, Debug, Clone, Copy)]
pub enum Checkbox {
    #[default]
    Default,
}

impl checkbox::StyleSheet for Theme {
    type Style = Checkbox;

    fn active(&self, style: &Self::Style, is_active: bool) -> checkbox::Appearance {
        match style {
            Checkbox::Default => <iced_native::Theme as checkbox::StyleSheet>::active(&iced_native::Theme::Dark, &iced_native::theme::Checkbox::Primary, is_active),
        }
    }

    fn hovered(&self, style: &Self::Style, is_active: bool) -> checkbox::Appearance {
        match style {
            Checkbox::Default => <iced_native::Theme as checkbox::StyleSheet>::hovered(&iced_native::Theme::Dark, &iced_native::theme::Checkbox::Primary, is_active),
        }
    }
}
