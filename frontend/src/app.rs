use std::collections::HashMap;
use std::rc::Rc;

use crate::components::device::{self, Device};
use crate::components::grid_layout::{self, GridLayout, PaneId};
use crate::components::sink::SinkState;
use crate::components::source::SourceState;
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

pub type GridState = (&'static str, HashMap<String, device::State>);

/// This is the main application container
pub struct AppContainer {
    panes: grid_layout::State<GridState>,
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
        // sinks (outputs)
        let hardware_sinks: Vec<SinkState> = Vec::from([SinkState::new("My cool speakers".to_string(), 100)]);
        let application_sinks: Vec<SinkState> = Vec::from([SinkState::new("mic used in an application".to_string(), 100), SinkState::new("audio recording application".to_string(), 100)]);

        // all available connections
        let sinks: HashMap<String, bool> = hardware_sinks.iter().chain(application_sinks.iter()).map(|e| (e.name(), false)).collect();

        // sources (inputs)
        let hardware_sources: Vec<SourceState> = Vec::from([SourceState::new_with_connections("My great mic".to_string(), 100, sinks.clone())]);
        let application_sources: Vec<SourceState> = Vec::from([SourceState::new_with_connections("Audio coming from application".to_string(), 100, sinks.clone())]);

        // let hardware_sources_pane: GridState = ("HARDWARE SOURCES", sinks.into_iter().map(|d| {(d.name(), d.clone().into())}).collect());
        let hardware_sources_pane: GridState = ("HARDWARE SOURCES", hardware_sources.into_iter().map(|d| {(d.name(), d.into())}).collect());
        let hardware_sinks_pane: GridState = ("HARDWARE SINKS", hardware_sinks.into_iter().map(|d| {(d.name(), d.into())}).collect());

        let virtual_devices_pane: GridState = ("VIRTUAL DEVICES", HashMap::new());

        let application_sources_pane: GridState = ("APPLICATION SOURCES", application_sources.into_iter().map(|d| {(d.name(), d.into())}).collect());
        let application_sinks_pane: GridState = ("APPLICATION SINKS", application_sinks.into_iter().map(|d| {(d.name(), d.into())}).collect());

        (
            AppContainer {
                panes: grid_layout::State::new(
                    [
                        hardware_sources_pane,
                        hardware_sinks_pane,
                        application_sources_pane,
                        virtual_devices_pane,
                        application_sinks_pane
                    ].into()
                )
            },
            Command::none()
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
                if let Some(c) = self.panes.get_mut(id).1.get_mut(&d).and_then(|d| d.connections.as_mut().unwrap().get_mut(&c)) {
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
