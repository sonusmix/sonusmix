use crate::theme::{Theme, self};
use iced::Length;
use iced::widget::{PaneGrid, pane_grid, text};
use iced::{application, executor, widget::container, Application, Command, Renderer};

#[derive(Debug, Clone)]
pub enum Message {
    GridResize(pane_grid::ResizeEvent),
}

/// This is the main application container
pub struct AppContainer {
    panes: pane_grid::State<&'static str>,
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
        use iced::widget::pane_grid::{State, Configuration, Axis};
        (
            AppContainer {
                panes: State::with_configuration(Configuration::Split {
                    axis: Axis::Horizontal,
                    ratio: 0.5,
                    a: Box::new(Configuration::Split {
                        axis: Axis::Vertical,
                        ratio: 0.5,
                        a: Box::new(Configuration::Pane("MICROPHONE / HARDWARE SOURCE")),
                        b: Box::new(Configuration::Pane("HARDWARE SINK")),
                    }),
                    b: Box::new(Configuration::Split {
                        axis: Axis::Vertical,
                        ratio: 1.0 / 3.0,
                        a: Box::new(Configuration::Pane("APPLICATION SOURCE (PLAYBACK)")),
                        b: Box::new(Configuration::Split {
                            axis: Axis::Vertical,
                            ratio: 0.5,
                            a: Box::new(Configuration::Pane("VIRTUAL DEVICES")),
                            b: Box::new(Configuration::Pane("APPLICATION SINK (RECORDING)")),
                        }),
                    }),
                })
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
                self.panes.resize(&split, ratio);
            },
        }
        Command::none()
    }

    fn view(&self) -> iced::Element<Self::Message, Renderer<Self::Theme>> {
        PaneGrid::new(&self.panes, |id, pane, _is_maximized| {
            pane_grid::Content::new(container(text(pane))
                .padding(10)
                .height(Length::Fill)
                .width(Length::Fill)
                .style(theme::Container::Border)
            )
        })
            .on_resize(10, Message::GridResize)
            .into()
        // container(Grid::new([
        //     HardwareSource::new().view(),
        //     HardwareSink::new().view(),
        //     ApplicationSource::new().view(),
        //     Virtual::new().view(),
        //     ApplicationSink::new().view(),
        // ], GridSplit::default()))
        // .into()
    }
}
