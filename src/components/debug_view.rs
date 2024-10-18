use std::convert::Infallible;
use std::sync::Arc;

use gtk::glib::Propagation;
use relm4::gtk::prelude::*;
use relm4::prelude::*;

use crate::state::{SonusmixReducer, SonusmixState};

pub struct DebugView {
    sonusmix_state: Arc<SonusmixState>,
    visible: bool,
    state_text_buffer: gtk::TextBuffer,
}

#[derive(Debug, Clone)]
pub enum DebugViewMsg {
    UpdateState(Arc<SonusmixState>),
    SetVisible(bool),
}

#[relm4::component(pub)]
impl SimpleComponent for DebugView {
    type Init = ();
    type Input = DebugViewMsg;
    type Output = Infallible;

    view! {
        gtk::Window {
            #[watch]
            set_visible: model.visible,
            set_default_size: (800, 600),

            connect_close_request[sender] => move |_| {
                sender.input(DebugViewMsg::SetVisible(false));
                Propagation::Stop
            },

            gtk::ScrolledWindow {
                set_margin_all: 8,
                set_policy: (gtk::PolicyType::Automatic, gtk::PolicyType::Automatic),

                gtk::TextView::with_buffer(&model.state_text_buffer) {
                    set_editable: false,
                    set_monospace: true,
                },
            }
        }
    }

    fn init(_init: (), root: Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {
        let sonusmix_state =
            SonusmixReducer::subscribe(sender.input_sender(), DebugViewMsg::UpdateState);

        let state_text_buffer = gtk::TextBuffer::new(None);

        let model = DebugView {
            sonusmix_state,
            visible: false,
            state_text_buffer,
        };

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: DebugViewMsg, _sender: ComponentSender<Self>) {
        match msg {
            DebugViewMsg::UpdateState(state) => {
                self.sonusmix_state = state;
                if self.visible {
                    self.update_text();
                }
            }
            DebugViewMsg::SetVisible(visible) => {
                self.visible = visible;
                if self.visible {
                    self.update_text();
                }
            }
        }
    }
}

impl DebugView {
    fn update_text(&mut self) {
        self.state_text_buffer
            .set_text(&format!("{:#?}", self.sonusmix_state));
    }
}
