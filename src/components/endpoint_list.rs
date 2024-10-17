use std::sync::Arc;

use relm4::factory::FactoryVecDeque;
use relm4::gtk::prelude::*;
use relm4::prelude::*;

use crate::pipewire_api::PortKind;
use crate::state::{SonusmixOutputMsg, SonusmixReducer, SonusmixState};

use super::endpoint::Endpoint;

pub struct EndpointList {
    list: PortKind,
    endpoints: FactoryVecDeque<Endpoint>,
}

#[derive(Debug)]
pub enum EndpointListMsg {
    UpdateState(Arc<SonusmixState>, Option<SonusmixOutputMsg>),
}

#[derive(Debug)]
pub struct ChooseEndpoint;

#[relm4::component(pub)]
impl SimpleComponent for EndpointList {
    type Init = PortKind;
    type Input = EndpointListMsg;
    type Output = ChooseEndpoint;

    view! {
        gtk::Frame {
            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_hexpand: true,

                gtk::CenterBox {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_margin_top: 4,
                    set_margin_start: 4,

                    #[wrap(Some)]
                    set_start_widget = &gtk::Button {
                        set_icon_name: "list-add-symbolic",
                        set_has_frame: true,

                        connect_clicked[sender] => move |_| {
                            let _ = sender.output(ChooseEndpoint);
                        }
                    },
                    #[wrap(Some)]
                    set_center_widget = &gtk::Label {
                        set_markup: match model.list {
                            PortKind::Source => "<big>Sources</big>",
                            PortKind::Sink => "<big>Sinks</big>",
                        }
                    },
                },
                gtk::Separator {
                    set_orientation: gtk::Orientation::Vertical,
                    set_margin_vertical: 4,
                },
                if model.endpoints.is_empty() {
                    gtk::Label {
                        set_vexpand: true,
                        set_valign: gtk::Align::Center,
                        set_halign: gtk::Align::Center,

                        set_label: match model.list {
                            PortKind::Source => "Add some sources to control them here.",
                            PortKind::Sink => "Add some sinks to control them here.",
                        }
                    }
                } else {
                    gtk::ScrolledWindow {
                        set_vexpand: true,
                        set_policy: (gtk::PolicyType::Never, gtk::PolicyType::Automatic),

                        #[local_ref]
                        endpoints_list -> gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,
                            set_margin_all: 4,
                        }
                    }
                }
            },
        },
    }

    fn init(
        list: PortKind,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let sonusmix_state =
            SonusmixReducer::subscribe_msg(sender.input_sender(), EndpointListMsg::UpdateState);

        let mut endpoints = FactoryVecDeque::builder()
            .launch(gtk::Box::default())
            .forward(sender.input_sender(), |output| match output {});
        {
            let mut endpoints = endpoints.guard();
            let active_endpoints = match list {
                PortKind::Source => &sonusmix_state.active_sources,
                PortKind::Sink => &sonusmix_state.active_sinks,
            };
            for endpoint in active_endpoints {
                endpoints.push_back(*endpoint);
            }
        }

        let model = EndpointList { list, endpoints };

        let endpoints_list = model.endpoints.widget();
        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: EndpointListMsg, _sender: ComponentSender<Self>) {
        match msg {
            EndpointListMsg::UpdateState(_state, msg) => match msg {
                Some(SonusmixOutputMsg::EndpointAdded(descriptor))
                    if descriptor.is_list(self.list) =>
                {
                    self.endpoints.guard().push_back(descriptor);
                }
                Some(SonusmixOutputMsg::EndpointRemoved(descriptor)) => {
                    if descriptor.is_list(self.list) {
                        let index = self
                            .endpoints
                            .iter()
                            .position(|endpoint| endpoint.id() == descriptor);
                        if let Some(index) = index {
                            self.endpoints.guard().remove(index);
                        }
                    }
                }
                _ => {}
            },
        }
    }
}
