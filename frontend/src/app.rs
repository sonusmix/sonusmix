use crate::theme::{Theme, self};
use iced::{Length, Size};
use iced::widget::{PaneGrid, pane_grid, text};
use iced::{application, executor, widget::container, Application, Command, Renderer};

#[derive(Debug, Clone)]
pub enum Message {
    GridResize(pane_grid::ResizeEvent),
}

/// This is the main application container
pub struct AppContainer {
    panes: pane_grid::State<&'static str>,
    bottom_left_split: (pane_grid::Split, f32),
    bottom_right_split: (pane_grid::Split, f32),
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
        let (mut panes, top) = State::new("MICROPHONE / HARDWARE SOURCE");
        let (bottom_left, _) = panes.split(Axis::Horizontal, &top, "APPLICATION SOURCE (PLAYBACK)")
            .expect("Initializing known panes should not error");
        let _ = panes.split(Axis::Vertical, &top, "HARDWARE SINK")
            .expect("Initializing known panes should not error");
        let (bottom_right, bottom_left_split) = panes.split(Axis::Vertical, &bottom_left, "VIRTUAL DEVICES")
            .expect("Initializing known panes should not error");
        panes.resize(&bottom_left_split, 1.0 / 3.0);
        let (_, bottom_right_split) = panes.split(Axis::Vertical, &bottom_right, "APPLICATION SINK (RECORDING)")
            .expect("Initializing known panes should not error");

        (
            AppContainer {
                panes,
                bottom_left_split: (bottom_left_split, 1.0 / 3.0),
                bottom_right_split: (bottom_right_split, 0.5),
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
                if split == self.bottom_left_split.0 {
                    let (a, b) = (self.bottom_left_split.1, self.bottom_right_split.1);
                    
                    // b' = (a'+ab-a-b)/(a'-1); a is left side split, b is right side split, a' is "ratio"
                    // https://www.wolframalpha.com/input?i=solve+%281-d%29%281-c%29%3D%281-b%29%281-a%29+for+d
                    // c and d in the link above are a' and b' since WolframAlpha doesn't like apostrophes
                    // This equation took far too long to work out
                    let b_prime = (ratio + a * b - a - b) / (ratio - 1.0);

                    (self.bottom_left_split.1, self.bottom_right_split.1) = (ratio, b_prime);
                    self.panes.resize(&split, ratio);
                    self.panes.resize(&self.bottom_right_split.0, b_prime);
                } else {
                    if split == self.bottom_right_split.0 {
                        self.bottom_right_split.1 = ratio;
                    }
                    self.panes.resize(&split, ratio);
                }
                
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
        )})
            .on_resize(10, Message::GridResize)
            .into()
    }
}
