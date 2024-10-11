use relm4::prelude::*;
use relm4::{factory::FactoryVecDeque, gtk::prelude::*};

use std::convert::Infallible;
use std::sync::Arc;

use crate::pipewire_api::PortKind;
use crate::state::{
    Endpoint, EndpointDescriptor, LinkState, SonusmixMsg, SonusmixReducer, SonusmixState,
};

pub struct ConnectEndpoints {
    sonusmix_state: Arc<SonusmixState>,
    base_endpoint: Endpoint,
    items: FactoryVecDeque<ConnectEndpointItem>,
}

#[derive(Debug)]
pub enum ConnectEndpointsMsg {
    StateUpdate(Arc<SonusmixState>),
    ConnectionChanged(ConnectEndpointItemOutput),
}

#[relm4::component(pub)]
impl SimpleComponent for ConnectEndpoints {
    type Init = EndpointDescriptor;
    type Input = ConnectEndpointsMsg;
    type Output = Infallible;

    view! {
        gtk::Popover {
            set_autohide: true,

            if model.items.is_empty() {
                gtk::Label {
                    set_valign: gtk::Align::Center,
                    set_halign: gtk::Align::Center,

                    #[watch]
                    set_label: "Nothing to connect to",
                }
            } else {
                #[local_ref]
                *item_box -> gtk::ListBox {
                    set_selection_mode: gtk::SelectionMode::None,
                    set_show_separators: true,
                }
            }
        }
    }

    fn init(
        endpoint_desc: EndpointDescriptor,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let sonusmix_state =
            SonusmixReducer::subscribe(sender.input_sender(), ConnectEndpointsMsg::StateUpdate);

        let base_endpoint = sonusmix_state
            .endpoints
            .get(&endpoint_desc)
            .expect("connect endpoints component failed to find matching endpoint on init")
            .clone();

        let items = FactoryVecDeque::builder()
            .launch(gtk::ListBox::default())
            .forward(
                sender.input_sender(),
                ConnectEndpointsMsg::ConnectionChanged,
            );

        let mut model = Self {
            sonusmix_state,
            base_endpoint,
            items,
        };
        model.update_items();

        let item_box = model.items.widget();
        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: ConnectEndpointsMsg, _sender: ComponentSender<Self>) {
        match msg {
            ConnectEndpointsMsg::StateUpdate(sonusmix_state) => {
                self.sonusmix_state = sonusmix_state;
                self.update_items();
            }
            ConnectEndpointsMsg::ConnectionChanged(msg) => {
                // TODO: Handle groups
                let msg = if self.base_endpoint.descriptor.is_kind(PortKind::Source) {
                    match msg {
                        ConnectEndpointItemOutput::ConnectEndpoint(endpoint) => {
                            SonusmixMsg::Link(self.base_endpoint.descriptor, endpoint)
                        }
                        ConnectEndpointItemOutput::DisconnectEndpoint(endpoint) => {
                            SonusmixMsg::RemoveLink(self.base_endpoint.descriptor, endpoint)
                        }
                        ConnectEndpointItemOutput::SetEndpointLocked(endpoint, locked) => {
                            SonusmixMsg::SetLinkLocked(
                                self.base_endpoint.descriptor,
                                endpoint,
                                locked,
                            )
                        }
                    }
                } else {
                    match msg {
                        ConnectEndpointItemOutput::ConnectEndpoint(endpoint) => {
                            SonusmixMsg::Link(endpoint, self.base_endpoint.descriptor)
                        }
                        ConnectEndpointItemOutput::DisconnectEndpoint(endpoint) => {
                            SonusmixMsg::RemoveLink(endpoint, self.base_endpoint.descriptor)
                        }
                        ConnectEndpointItemOutput::SetEndpointLocked(endpoint, locked) => {
                            SonusmixMsg::SetLinkLocked(
                                endpoint,
                                self.base_endpoint.descriptor,
                                locked,
                            )
                        }
                    }
                };
                SonusmixReducer::emit(msg);
            }
        }
    }
}

impl ConnectEndpoints {
    fn update_items(&mut self) {
        // TODO: Handle groups
        let candidates = if self.base_endpoint.descriptor.is_kind(PortKind::Source) {
            &self.sonusmix_state.active_sinks
        } else {
            &self.sonusmix_state.active_sources
        };
        let mut factory = self.items.guard();
        factory.clear();
        for candidate in candidates
            .iter()
            .filter_map(|id| self.sonusmix_state.endpoints.get(id))
            .cloned()
        {
            factory.push_back((
                self.base_endpoint.descriptor,
                candidate,
                self.sonusmix_state.clone(),
            ));
        }
    }
}

struct ConnectEndpointItem {
    candidate_endpoint: Endpoint,
    link_state: Option<LinkState>,
}

#[derive(Debug)]
pub enum ConnectEndpointItemOutput {
    ConnectEndpoint(EndpointDescriptor),
    DisconnectEndpoint(EndpointDescriptor),
    SetEndpointLocked(EndpointDescriptor, bool),
}

#[relm4::factory]
impl FactoryComponent for ConnectEndpointItem {
    type Init = (EndpointDescriptor, Endpoint, Arc<SonusmixState>);
    type Input = Infallible;
    type Output = ConnectEndpointItemOutput;
    type CommandOutput = ();
    type ParentWidget = gtk::ListBox;

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Horizontal,
            set_spacing: 8,

            #[name(link_lock_button)]
            gtk::ToggleButton {
                add_css_class: "flat",

                set_sensitive: self.link_state != Some(LinkState::PartiallyConnected),
                set_active: self.link_state.map(|link| link.is_locked()).unwrap_or(false),
                set_icon_name: if link_lock_button.is_active()
                    { "changes-prevent-symbolic" } else { "changes-allow-symbolic" },
                set_tooltip: if link_lock_button.is_active()
                {
                    "Allow this link to be changed outside of Sonusmix"
                } else if link_lock_button.is_sensitive() {
                    "Prevent this link from being changed outside of Sonusmix"
                } else {
                    "Link cannot be locked while it is partially connected"
                },

                connect_clicked[sender, descriptor = self.candidate_endpoint.descriptor] => move |button| {
                    let _ = sender.output(ConnectEndpointItemOutput::SetEndpointLocked(descriptor, button.is_active()));
                },
            },


            gtk::CheckButton {
                set_label: Some(self.candidate_endpoint.custom_or_display_name()),
                set_active: self.link_state.and_then(|link| link.is_connected()).unwrap_or(false),
                set_inconsistent: self.link_state.map(|link| link.is_connected().is_none()).unwrap_or(false),

                connect_toggled[sender, descriptor = self.candidate_endpoint.descriptor] => move |check| {
                    if check.is_active() {
                        let _ = sender.output(ConnectEndpointItemOutput::ConnectEndpoint(descriptor));
                    } else {
                        let _ = sender.output(ConnectEndpointItemOutput::DisconnectEndpoint(descriptor));
                    }
                } @endpoint_toggled_handler
            }
        }
    }

    fn init_model(
        (base_endpoint, candidate_endpoint, sonusmix_state): (
            EndpointDescriptor,
            Endpoint,
            Arc<SonusmixState>,
        ),
        _index: &DynamicIndex,
        _sender: FactorySender<Self>,
    ) -> Self {
        // TODO: Handle groups
        let (source, sink) = if base_endpoint.is_kind(PortKind::Source) {
            (base_endpoint, candidate_endpoint.descriptor)
        } else {
            (candidate_endpoint.descriptor, base_endpoint)
        };

        let link_state = sonusmix_state
            .links
            .iter()
            .find(|link| link.start == source && link.end == sink)
            .map(|link| link.state);

        Self {
            candidate_endpoint,
            link_state,
        }
    }
}
