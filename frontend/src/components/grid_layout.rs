use std::{cell::RefCell, rc::Rc};

use iced::{
    widget::{pane_grid, PaneGrid},
    Element,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PaneId {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

impl From<PaneId> for usize {
    fn from(value: PaneId) -> Self {
        match value {
            PaneId::TopLeft => 0,
            PaneId::TopRight => 1,
            PaneId::BottomLeft => 2,
            PaneId::BottomCenter => 3,
            PaneId::BottomRight => 4,
        }
    }
}

impl TryFrom<usize> for PaneId {
    type Error = ();

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        [
            Self::TopLeft,
            Self::TopRight,
            Self::BottomLeft,
            Self::BottomCenter,
            Self::BottomRight,
        ]
        .get(value)
        .copied()
        .ok_or(())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PaneLayout<T> {
    pub top_left: T,
    pub top_right: T,
    pub bottom_left: T,
    pub bottom_center: T,
    pub bottom_right: T,
}

impl<T> PaneLayout<T> {
    pub fn as_array(&self) -> [&T; 5] {
        let PaneLayout {
            top_left,
            top_right,
            bottom_left,
            bottom_center,
            bottom_right,
        } = self;
        [
            top_left,
            top_right,
            bottom_left,
            bottom_center,
            bottom_right,
        ]
    }

    pub fn get(&self, id: PaneId) -> &T {
        match id {
            PaneId::TopLeft => &self.top_left,
            PaneId::TopRight => &self.top_right,
            PaneId::BottomLeft => &self.bottom_left,
            PaneId::BottomCenter => &self.bottom_center,
            PaneId::BottomRight => &self.bottom_right,
        }
    }
}

impl<T: Eq> PaneLayout<T> {
    pub fn find(&self, item: &T) -> Option<PaneId> {
        self.as_array()
            .into_iter()
            .position(|e| e == item)
            .map(|i| {
                i.try_into()
                    .expect("fixed length array should not be longer than 5")
            })
    }
}

impl PaneLayout<PaneId> {
    pub fn ids() -> Self {
        Self {
            top_left: PaneId::TopLeft,
            top_right: PaneId::TopRight,
            bottom_left: PaneId::BottomLeft,
            bottom_center: PaneId::BottomCenter,
            bottom_right: PaneId::BottomRight,
        }
    }
}

impl<T> From<[T; 5]> for PaneLayout<T> {
    fn from(value: [T; 5]) -> Self {
        let [top_left, top_right, bottom_left, bottom_center, bottom_right] = value;
        Self {
            top_left,
            top_right,
            bottom_left,
            bottom_center,
            bottom_right,
        }
    }
}

impl<T> From<PaneLayout<T>> for [T; 5] {
    fn from(value: PaneLayout<T>) -> Self {
        let PaneLayout {
            top_left,
            top_right,
            bottom_left,
            bottom_center,
            bottom_right,
        } = value;
        [
            top_left,
            top_right,
            bottom_left,
            bottom_center,
            bottom_right,
        ]
    }
}

pub struct State<T> {
    panes: pane_grid::State<T>,
    pane_ids: PaneLayout<pane_grid::Pane>,
    bottom_left_split: (pane_grid::Split, f32),
    bottom_right_split: (pane_grid::Split, f32),
}

impl<T> State<T> {
    pub fn new(state: PaneLayout<T>) -> Self {
        use iced::widget::pane_grid::Axis;

        let PaneLayout {
            top_left,
            top_right,
            bottom_left,
            bottom_center,
            bottom_right,
        } = state;

        let (mut panes, top_left_pane) = pane_grid::State::new(top_left);
        let (bottom_left_pane, _) = panes
            .split(Axis::Horizontal, &top_left_pane, bottom_left)
            .expect("Initializing known panes should not error");
        let (top_right_pane, _) = panes
            .split(Axis::Vertical, &top_left_pane, top_right)
            .expect("Initializing known panes should not error");
        let (bottom_center_pane, bottom_left_split) = panes
            .split(Axis::Vertical, &bottom_left_pane, bottom_center)
            .expect("Initializing known panes should not error");
        panes.resize(&bottom_left_split, 1.0 / 3.0);
        let (bottom_right_pane, bottom_right_split) = panes
            .split(Axis::Vertical, &bottom_center_pane, bottom_right)
            .expect("Initializing known panes should not error");

        Self {
            panes,
            pane_ids: PaneLayout {
                top_left: top_left_pane,
                top_right: top_right_pane,
                bottom_left: bottom_left_pane,
                bottom_center: bottom_center_pane,
                bottom_right: bottom_right_pane,
            },
            bottom_left_split: (bottom_left_split, 1.0 / 3.0),
            bottom_right_split: (bottom_right_split, 0.5),
        }
    }

    pub fn resize(&mut self, split: pane_grid::Split, ratio: f32) {
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
    }

    pub fn get(&self, id: PaneId) -> &T {
        self.panes.get(self.pane_ids.get(id))
            .expect("no panes should be missing")
    }

    pub fn get_mut(&mut self, id: PaneId) -> &mut T {
        self.panes.get_mut(self.pane_ids.get(id))
            .expect("no panes should be missing")
    }
}

impl State<PaneId> {
    pub fn ids() -> Self {
        Self::new(PaneLayout::ids())
    }
}

pub struct GridLayout<'a, Message, Renderer>
where
    Renderer: iced_native::Renderer,
    <Renderer as iced_native::Renderer>::Theme:
        iced::widget::pane_grid::StyleSheet + iced::widget::container::StyleSheet,
{
    grid: PaneGrid<'a, Message, Renderer>,
}

impl<'a, Message, Renderer> GridLayout<'a, Message, Renderer>
where
    Renderer: 'a + iced_native::Renderer,
    <Renderer as iced_native::Renderer>::Theme:
        iced::widget::pane_grid::StyleSheet + iced::widget::container::StyleSheet,
{
    pub fn new<T>(
        state: &'a State<T>,
        view: impl Fn(PaneId, &'a T) -> Element<'a, Message, Renderer>,
    ) -> Self {
        Self {
            grid: PaneGrid::new(&state.panes, |id, p, _| {
                view(
                    state
                        .pane_ids
                        .find(&id)
                        .expect("an incorrect Pane handle was recorded"),
                    p,
                )
                .into()
            }),
        }
    }

    pub fn with_elements(
        state: &'a State<PaneId>,
        elements: PaneLayout<Element<'a, Message, Renderer>>,
    ) -> Self {
        let elements = Rc::new(RefCell::new(<[_; 5]>::from(elements).map(|e| Some(e))));
        Self {
            grid: PaneGrid::new(&state.panes, {
                let elements = elements.clone();
                move |_, pane, _| {
                    pane_grid::Content::new(
                        elements.borrow_mut()[usize::from(*pane)]
                            .take()
                            .expect("view function should only be called once for each pane"),
                    )
                }
            }),
        }
    }

    pub fn on_resize<F>(mut self, leeway: impl Into<iced_native::Pixels>, f: F) -> Self
    where
        F: 'a + Fn(pane_grid::ResizeEvent) -> Message,
    {
        self.grid = self.grid.on_resize(leeway, f);
        self
    }

    pub fn view(self) -> Element<'a, Message, Renderer>
    where
        Message: 'a,
    {
        self.grid.into()
    }
}

impl<'a, Message: 'a, Renderer> From<GridLayout<'a, Message, Renderer>>
    for Element<'a, Message, Renderer>
where
    Renderer: iced_native::Renderer + 'a,
    <Renderer as iced_native::Renderer>::Theme:
        iced::widget::pane_grid::StyleSheet + iced::widget::container::StyleSheet,
{
    fn from(value: GridLayout<'a, Message, Renderer>) -> Self {
        value.view()
    }
}
