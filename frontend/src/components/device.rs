use std::collections::HashMap;
use std::rc::Rc;

use iced::{
    widget::{checkbox, column, slider, text, Column, Row},
    Element,
};

pub enum DeviceKind {
    Source,
    Sink,
}

#[derive(Debug, Clone)]
pub struct State {
    pub name: String,
    pub volume: u32,
    pub connections: Option<HashMap<String, bool>>,
}

impl State {
    pub fn new_with_connections(
        name: String,
        volume: u32,
        connections: Option<HashMap<String, bool>>,
    ) -> Self {
        Self {
            name,
            volume,
            connections,
        }
    }

    pub fn new(name: String, volume: u32) -> Self {
        Self {
            name,
            volume,
            connections: None,
        }
    }
}

pub struct Device<'a, Message, Fvc, Fcc>
where
    Fvc: 'a + Fn(u32) -> Message,
    Fcc: 'a + Fn(&str, bool) -> Message,
{
    state: &'a State,
    on_volume_change_fn: Fvc,
    on_connection_change_fn: Fcc,
    on_volume_release_msg: Option<Message>,
}

impl<'a, Message, Fvc, Fcc> Device<'a, Message, Fvc, Fcc>
where
    Fvc: 'a + Fn(u32) -> Message,
    Fcc: 'a + Fn(&str, bool) -> Message,
{
    pub fn new(state: &'a State, on_volume_change: Fvc, on_connection_change: Fcc) -> Self {
        Self {
            state,
            on_volume_change_fn: on_volume_change,
            on_connection_change_fn: on_connection_change,
            on_volume_release_msg: None,
        }
    }

    pub fn on_volume_release<F>(mut self, on_volume_release: Message) -> Self {
        self.on_volume_release_msg = Some(on_volume_release);
        self
    }

    pub fn view<Renderer>(self) -> Element<'a, Message, Renderer>
    where
        Message: 'a + Clone,
        Renderer: 'a + iced_native::text::Renderer,
        <Renderer as iced_native::Renderer>::Theme:
            text::StyleSheet + slider::StyleSheet + checkbox::StyleSheet,
    {
        let mut children: Vec<Element<Message, Renderer>> = Vec::new();

        // title
        children.push(text(&self.state.name).into());

        // slider
        children.push(slider(0..=100, self.state.volume, self.on_volume_change_fn).into());

        // connection checkboxes (if device can make connections)
        if let Some(connections) = &self.state.connections {
            children.push(
                <Element<_, _>>::from(Row::with_children(
                    connections
                        .iter()
                        .map(|(name, is_active)| {
                            checkbox(name.clone(), *is_active, move |s| (name, s)).into()
                        })
                        .collect(),
                ))
                .map(move |(name, s)| (self.on_connection_change_fn)(name, s)),
            )
        }

        Column::with_children(children).into()
    }
}

impl<'a, Message, Renderer, Fvc, Fcc> From<Device<'a, Message, Fvc, Fcc>>
    for Element<'a, Message, Renderer>
where
    Fvc: 'a + Fn(u32) -> Message,
    Fcc: 'a + Fn(&str, bool) -> Message,
    Message: 'a + Clone,
    Renderer: 'a + iced_native::text::Renderer,
    <Renderer as iced_native::Renderer>::Theme:
        text::StyleSheet + slider::StyleSheet + checkbox::StyleSheet,
{
    fn from(value: Device<'a, Message, Fvc, Fcc>) -> Self {
        value.view()
    }
}
