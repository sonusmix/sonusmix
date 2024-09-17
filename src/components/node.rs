use std::sync::{mpsc, Arc};

use log::debug;
use relm4::actions::RelmAction;
use relm4::factory::FactoryView;
use relm4::prelude::*;
use relm4::{actions::RelmActionGroup, gtk::prelude::*};

use crate::pipewire_api::{Graph, Node as PwNode, PortKind, ToPipewireMessage};
use crate::state2::{Endpoint as PwEndpoint, EndpointDescriptor, SonusmixMsg, SonusmixReducer, SonusmixState};

use super::connect_nodes::ConnectNodes;

pub struct Node {
    endpoint: PwEndpoint,
    list: PortKind,
    enabled: bool,
    pw_sender: mpsc::Sender<ToPipewireMessage>,
    connect_nodes: Controller<ConnectNodes>,
}

impl Node {
    pub fn id(&self) -> EndpointDescriptor {
        self.endpoint.descriptor
    }
}

#[derive(Debug, Clone)]
pub enum NodeMsg {
    UpdateState(Arc<SonusmixState>),
    #[doc(hidden)]
    Volume(f64),
    ToggleMute,
    #[doc(hidden)]
    Remove,
}

#[derive(Debug, Clone)]
pub enum NodeOutput {}

relm4::new_action_group!(NodeMenuActionGroup, "node-menu");
relm4::new_stateless_action!(RemoveAction, NodeMenuActionGroup, "remove");

#[relm4::factory(pub)]
impl FactoryComponent for Node {
    type Init = (EndpointDescriptor, mpsc::Sender<ToPipewireMessage>);
    type Input = NodeMsg;
    type Output = NodeOutput;
    type CommandOutput = ();
    type ParentWidget = gtk::Box;

    view! {
        #[root]
        gtk::Box {
            set_orientation: gtk::Orientation::Horizontal,
            set_spacing: 8,
            set_margin_vertical: 4,
            set_margin_horizontal: 4,

            #[watch]
            set_sensitive: self.enabled,

            gtk::Box {
                set_hexpand: true,
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 10,

                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,

                    #[name(icon_view)]
                    gtk::Image {
                        set_margin_end: 4,
                        // Some icon themes use symbolic-only icons below a certain size.
                        // Unfortunately, because Gtk thinks they aren't symbolic, it doesn't
                        // properly recolor them, so here we let the Gtk theme set the icon size,
                        // while ensuring that the icons don't get too small.
                        #[watch]
                        set_pixel_size: icon_view.pixel_size().max(24),
                        #[watch]
                        set_icon_name: Some(&self.endpoint.icon_name),
                    },

                    gtk::Label {
                        set_hexpand: true,
                        set_halign: gtk::Align::Start,
                        set_ellipsize: gtk::pango::EllipsizeMode::End,

                        #[watch]
                        set_label: &self.endpoint.display_name,
                        #[watch]
                        set_tooltip: &self.endpoint.display_name,
                        set_css_classes: &["heading"],
                    },

                    gtk::Label {
                        #[watch]
                        set_label: &match self.endpoint.descriptor {
                            EndpointDescriptor::EphemeralNode(id, _) => format!("id: {}", id),
                            _ => String::new(),
                        }
                    }
                },
                gtk::Label {
                    set_halign: gtk::Align::Start,
                    set_ellipsize: gtk::pango::EllipsizeMode::End,
                    set_css_classes: &["caption", "dim-label"],

                    #[watch]
                    // set_label: &self.endpoint.identifier.details().unwrap_or_default(),
                    set_label: "",
                    // #[watch]
                    // set_tooltip?: self.endpoint.identifier.details(),
                },
                gtk::Scale {
                    set_range: (0.0, 100.0),
                    #[watch]
                    #[block_signal(volume_handler)]
                    set_value: volume_to_slider(self.endpoint.volume),
                    set_draw_value: true,
                    set_format_value_func => move |_, value| format!("{value:.0}%"),

                    connect_value_changed[sender] => move |scale| {
                        sender.input(NodeMsg::Volume(scale.value()));
                        } @volume_handler
                }
            },

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 5,

                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 5,

                    gtk::MenuButton {
                        set_label: "Connections",
                        set_popover: Some(self.connect_nodes.widget()),
                    },
                    #[name(node_menu_button)]
                    gtk::MenuButton {
                        set_icon_name: "view-more-symbolic",
                        set_menu_model: Some(&node_menu),
                    },
                },
                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 5,

                    gtk::Button {
                        set_label: "M",
                        #[watch]
                        set_tooltip: if self.endpoint
                            .volume_locked_muted
                            .is_muted()
                            .unwrap_or(false)
                        { "Unmute" } else { "Mute" },
                        #[watch]
                        set_css_classes: if self.endpoint
                            .volume_locked_muted
                            .is_muted()
                            .unwrap_or(false)
                        { &["mute-node-button-active", "text-button"] } else { &["", "text-button"] },
                        connect_clicked => NodeMsg::ToggleMute,
                    },
                    gtk::Button {
                        set_label: "P",
                        set_tooltip: "Primary",
                    }
                }
            }
        },
    }

    menu! {
        node_menu: {
            "Remove" => RemoveAction,
        }
    }

    fn init_model(
        (endpoint_desc, pw_sender): (EndpointDescriptor, mpsc::Sender<ToPipewireMessage>),
        _index: &DynamicIndex,
        sender: FactorySender<Self>,
    ) -> Self {
        let endpoint = SonusmixReducer::subscribe(sender.input_sender(), NodeMsg::UpdateState)
            .endpoints
            .get(&endpoint_desc)
            .expect("endpoint component failed to find matching endpoint on init")
            .clone();
        let EndpointDescriptor::EphemeralNode(node_id, list) = endpoint.descriptor else {
            todo!("migrate connect_nodes component");
        };

        let connect_nodes = ConnectNodes::builder()
            .launch((node_id, list, pw_sender.clone()))
            .forward(sender.input_sender(), |msg| match msg {});
        Self {
            endpoint,
            list,
            enabled: true,
            pw_sender,
            connect_nodes,
        }
    }

    fn init_widgets(
        &mut self,
        _index: &Self::Index,
        root: Self::Root,
        _returned_widget: &<Self::ParentWidget as FactoryView>::ReturnedWidget,
        sender: FactorySender<Self>,
    ) -> Self::Widgets {
        let widgets = view_output!();

        let mut group = RelmActionGroup::<NodeMenuActionGroup>::new();
        let remove_action: RelmAction<RemoveAction> = RelmAction::new_stateless({
            let sender = sender.clone();
            move |_| {
                sender.input(NodeMsg::Remove);
            }
        });
        group.add_action(remove_action);
        group.register_for_widget(&widgets.node_menu_button);

        widgets
    }

    fn update(&mut self, msg: NodeMsg, _sender: FactorySender<Self>) {
        match msg {
            NodeMsg::UpdateState(state) => {
                if let Some(endpoint) = state.endpoints.get(&self.endpoint.descriptor) {
                    self.endpoint = endpoint.clone();
                }
            }
            NodeMsg::Volume(volume) => {
                SonusmixReducer::emit(SonusmixMsg::SetVolume(self.endpoint.descriptor, slider_to_volume(volume)))
            }
            NodeMsg::ToggleMute => {
                let mute = self.endpoint.volume_locked_muted.is_muted().map(|mute| !mute).unwrap_or(true);
                SonusmixReducer::emit(SonusmixMsg::SetMute(self.endpoint.descriptor, mute));
            }
            NodeMsg::Remove => {
                SonusmixReducer::emit(SonusmixMsg::RemoveEndpoint(self.endpoint.descriptor));
            }
        }
    }
}

fn volume_to_slider(volume: f32) -> f64 {
    (volume.powf(1.0 / 3.0) * 100.0) as f64
}

fn slider_to_volume(volume: f64) -> f32 {
    (volume as f32 / 100.0).powf(3.0) as f32
}
