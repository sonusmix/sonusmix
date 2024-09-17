use log::debug;
use relm4::prelude::*;
use relm4::{factory::FactoryVecDeque, gtk::prelude::*};

use std::convert::Infallible;
use std::sync::{mpsc, Arc};

use crate::pipewire_api::ToPipewireMessage;
use crate::pipewire_api::{Graph, Node, PortKind};
use crate::state2::{
    Endpoint, EndpointDescriptor, LinkState, SonusmixMsg, SonusmixReducer, SonusmixState,
};

pub struct ConnectNodes {
    sonusmix_state: Arc<SonusmixState>,
    base_endpoint: Endpoint,
    items: FactoryVecDeque<ConnectNodeItem>,
}

#[derive(Debug)]
pub enum ConnectNodesMsg {
    StateUpdate(Arc<SonusmixState>),
    ConnectionChanged(ConnectNodeItemOutput),
}

#[relm4::component(pub)]
impl SimpleComponent for ConnectNodes {
    type Init = EndpointDescriptor;
    type Input = ConnectNodesMsg;
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
                *item_box -> gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
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
            SonusmixReducer::subscribe(sender.input_sender(), ConnectNodesMsg::StateUpdate);

        let base_endpoint = sonusmix_state
            .endpoints
            .get(&endpoint_desc)
            .expect("connect endpoints component failed to find matching endpoint on init")
            .clone();

        let items = FactoryVecDeque::builder()
            .launch(gtk::Box::default())
            .forward(sender.input_sender(), ConnectNodesMsg::ConnectionChanged);

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

    fn update(&mut self, msg: ConnectNodesMsg, _sender: ComponentSender<Self>) {
        match msg {
            ConnectNodesMsg::StateUpdate(sonusmix_state) => {
                self.sonusmix_state = sonusmix_state;
                self.update_items();
            }
            ConnectNodesMsg::ConnectionChanged(msg) => {
                // TODO: Handle groups
                let msg = if self.base_endpoint.descriptor.is_kind(PortKind::Source) {
                    match msg {
                        ConnectNodeItemOutput::ConnectEndpoint(endpoint) => {
                            SonusmixMsg::Link(self.base_endpoint.descriptor, endpoint)
                        }
                        ConnectNodeItemOutput::DisconnectEndpoint(endpoint) => {
                            SonusmixMsg::RemoveLink(self.base_endpoint.descriptor, endpoint)
                        }
                    }
                } else {
                    match msg {
                        ConnectNodeItemOutput::ConnectEndpoint(endpoint) => {
                            SonusmixMsg::Link(self.base_endpoint.descriptor, endpoint)
                        }
                        ConnectNodeItemOutput::DisconnectEndpoint(endpoint) => {
                            SonusmixMsg::RemoveLink(self.base_endpoint.descriptor, endpoint)
                        }
                    }
                };
                SonusmixReducer::emit(msg);
            }
        }
    }
}

impl ConnectNodes {
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
            factory.push_back((self.base_endpoint.descriptor, candidate, self.sonusmix_state.clone()));
        }
    }
}

struct ConnectNodeItem {
    base_endpoint: EndpointDescriptor,
    candidate_endpoint: Endpoint,
    link_state: Option<LinkState>,
}

#[derive(Debug)]
enum ConnectNodeItemOutput {
    ConnectEndpoint(EndpointDescriptor),
    DisconnectEndpoint(EndpointDescriptor),
}

#[relm4::factory]
impl FactoryComponent for ConnectNodeItem {
    type Init = (EndpointDescriptor, Endpoint, Arc<SonusmixState>);
    type Input = Infallible;
    type Output = ConnectNodeItemOutput;
    type CommandOutput = ();
    type ParentWidget = gtk::Box;

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Horizontal,
            set_spacing: 8,

            gtk::CheckButton {
                #[watch]
                set_label: Some(&self.candidate_endpoint.display_name),
                #[watch]
                #[block_signal(endpoint_toggled_handler)]
                set_active: self.link_state.and_then(|link| link.is_connected()).unwrap_or(false),
                #[watch]
                #[block_signal(endpoint_toggled_handler)]
                set_inconsistent: self.link_state.map(|link| link.is_connected().is_none()).unwrap_or(false),

                connect_toggled[sender, descriptor = self.candidate_endpoint.descriptor] => move |check| {
                    if check.is_active() {
                        sender.output(ConnectNodeItemOutput::ConnectEndpoint(descriptor));
                    } else {
                        sender.output(ConnectNodeItemOutput::DisconnectEndpoint(descriptor));
                    }
                } @endpoint_toggled_handler
            }
        }
    }

    fn init_model(
        (base_endpoint, candidate_endpoint, sonusmix_state): (EndpointDescriptor, Endpoint, Arc<SonusmixState>),
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
            base_endpoint,
            candidate_endpoint,
            link_state,
        }
    }
}
