use std::collections::HashMap;

use crate::components::device::{self, Device};
use crate::components::grid_layout::{self, GridLayout, PaneId};
use crate::theme::{self, Theme};
use iced::widget::{column, pane_grid, text, Column};
use iced::Length;
use iced::{application, executor, widget::container, Application, Command, Renderer};
use tracing::trace;

#[derive(Debug, Clone)]
pub enum Message {
    GridResize(pane_grid::ResizeEvent),
    VolumeChange(PaneId, String, u32),
    ConnectionChange(PaneId, String, String, bool),
}

/// This is the main application container
pub struct AppContainer {
    panes: grid_layout::State<(&'static str, HashMap<String, device::State>)>,
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
        let devices = ["Microphone", "Discord", "Speakers"];
        let connections = devices
            .iter()
            .map(|s| (s.to_string(), false))
            .collect::<HashMap<_, _>>();
        (
            AppContainer {
                panes: grid_layout::State::new(
                    [
                        "MICROPHONE / HARDWARE SOURCE",
                        "HARDWARE SINK",
                        "APPLICATION SOURCE (PLAYBACK)",
                        "VIRTUAL DEVICES",
                        "APPLICATION SINK (RECORDING)",
                    ]
                    .map(|name| {
                        (
                            name,
                            devices
                                .iter()
                                .map(|d| {
                                    (
                                        d.to_string(),
                                        device::State::new(d.to_string(), 0, connections.clone()),
                                    )
                                })
                                .collect::<HashMap<_, _>>(),
                        )
                    })
                    .into(),
                ),
            },
            Command::none(),
        )
    }

    fn title(&self) -> String {
        String::from("Sonusmix")
    }

    fn update(&mut self, message: Self::Message) -> Command<Self::Message> {
        trace!(?message);
        match message {
            Message::GridResize(pane_grid::ResizeEvent { split, ratio }) => {
                self.panes.resize(split, ratio);
            }
            Message::VolumeChange(id, d, v) => {
                if let Some(d) = self.panes.get_mut(id).1.get_mut(&d) {
                    d.volume = v;
                }
            }
            Message::ConnectionChange(id, d, c, s) => {
                if let Some(c) = self.panes.get_mut(id).1.get_mut(&d).and_then(|d| d.connections.get_mut(&c)) {
                    *c = s;
                }
            }
        }
        Command::none()
    }

    fn view(&self) -> iced::Element<Self::Message, Renderer<Self::Theme>> {
        GridLayout::new(&self.panes, |id, (name, state)| {
            container(
                column![
                    text(name),
                    Column::with_children(state.iter().map(|(name, state)| Device::new(
                        state,
                        move |v| Message::VolumeChange(id, name.clone(), v),
                        move |c, s| Message::ConnectionChange(id, name.clone(), c.to_string(), s),
                    ).view()).collect())
                ],
            )
            .padding(10)
            .height(Length::Fill)
            .width(Length::Fill)
            .style(theme::Container::Border)
            .into()
        })
        .on_resize(10, Message::GridResize)
        .into()
    }
}
