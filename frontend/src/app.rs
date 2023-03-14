use crate::components::grid_layout::{self, GridLayout};
use crate::theme::{Theme, self};
use iced::Length;
use iced::widget::{pane_grid, text};
use iced::{application, executor, widget::container, Application, Command, Renderer};

#[derive(Debug, Clone)]
pub enum Message {
    GridResize(pane_grid::ResizeEvent),
}

/// This is the main application container
pub struct AppContainer {
    panes: grid_layout::State<&'static str>,
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
                panes: grid_layout::State::new([
                    "MICROPHONE / HARDWARE SOURCE",
                    "HARDWARE SINK",
                    "APPLICATION SOURCE (PLAYBACK)",
                    "VIRTUAL DEVICES",
                    "APPLICATION SINK (RECORDING)",
                ].into())
            },
            Command::none(),
        )
    }

    fn title(&self) -> String {
        String::from("Sonusmix")
    }

    fn update(&mut self, message: Self::Message) -> Command<Self::Message> {
        match message {
            Message::GridResize(pane_grid::ResizeEvent { split, ratio }) => {
                self.panes.resize(split, ratio);
            },
        }
        Command::none()
    }

    fn view(&self) -> iced::Element<Self::Message, Renderer<Self::Theme>> {
        GridLayout::new(&self.panes, |pane| container(text(pane))
            .padding(10)
            .height(Length::Fill)
            .width(Length::Fill)
            .style(theme::Container::Border)
            .into()
        )
            .on_resize(10, Message::GridResize)
            .into()
    }
}
