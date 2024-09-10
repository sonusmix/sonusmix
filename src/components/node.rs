use std::sync::{mpsc, Arc};

use log::debug;
use relm4::actions::RelmAction;
use relm4::factory::FactoryView;
use relm4::prelude::*;
use relm4::{actions::RelmActionGroup, gtk::prelude::*};

use crate::state::{SonusmixMsg, SONUSMIX_STATE};
use crate::{
    pipewire_api::{Graph, Node as PwNode, PortKind, ToPipewireMessage},
    state::subscribe_to_pipewire,
};

use super::connect_nodes::ConnectNodes;

pub struct Node {
    node: PwNode,
    list: PortKind,
    enabled: bool,
    pw_sender: mpsc::Sender<ToPipewireMessage>,
    connect_nodes: Controller<ConnectNodes>,
}

impl Node {
    pub fn id(&self) -> u32 {
        self.node.id
    }
}

#[derive(Debug, Clone)]
pub enum NodeMsg {
    UpdateGraph(Arc<Graph>),
    #[doc(hidden)]
    Volume(f64),
    #[doc(hidden)]
    Remove,
}

#[derive(Debug, Clone)]
pub enum NodeOutput {}

relm4::new_action_group!(NodeMenuActionGroup, "node-menu");
relm4::new_stateless_action!(RemoveAction, NodeMenuActionGroup, "remove");

#[relm4::factory(pub)]
impl FactoryComponent for Node {
    type Init = (u32, PortKind, mpsc::Sender<ToPipewireMessage>);
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
                        set_icon_name: Some(self.node.identifier.icon_name()),
                    },

                    gtk::Label {
                        set_hexpand: true,
                        set_halign: gtk::Align::Start,
                        set_ellipsize: gtk::pango::EllipsizeMode::End,

                        #[watch]
                        set_label: &self.node.identifier.human_name(),
                        #[watch]
                        set_tooltip: &self.node.identifier.human_name(),
                        set_css_classes: &["heading"],
                    },

                    gtk::Label {
                        #[watch]
                        set_label: &format!("id: {}", self.node.id),
                    }
                },
                gtk::Label {
                    set_halign: gtk::Align::Start,
                    set_ellipsize: gtk::pango::EllipsizeMode::End,
                    set_css_classes: &["caption", "dim-label"],

                    #[watch]
                    set_label: &self.node.identifier.details().unwrap_or_default(),
                    #[watch]
                    set_tooltip?: self.node.identifier.details(),
                },
                gtk::Scale {
                    set_range: (0.0, 1.0),
                    #[watch]
                    #[block_signal(volume_handler)]
                    set_value: calculate_slider_value(&self.node.channel_volumes),

                    connect_value_changed[sender] => move |scale| {
                        sender.input(NodeMsg::Volume(scale.value()));
                        } @volume_handler
                }
            },

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,

                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,

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
                gtk::Label {
                    set_label: "THIS IS WHERE SOME\nMORE STUFF WILL GO",
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
        (id, list, pw_sender): (u32, PortKind, mpsc::Sender<ToPipewireMessage>),
        _index: &DynamicIndex,
        sender: FactorySender<Self>,
    ) -> Self {
        let node = subscribe_to_pipewire(sender.input_sender(), NodeMsg::UpdateGraph)
            .nodes
            .get(&id)
            .expect("node component failed to find matching node on init")
            .clone();

        let connect_nodes = ConnectNodes::builder()
            .launch((node.id, list))
            .forward(sender.input_sender(), |msg| match msg {});
        Self {
            node,
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
            NodeMsg::UpdateGraph(graph) => {
                if let Some(node) = graph.nodes.get(&self.node.id) {
                    self.node = node.clone();
                    self.enabled = true;
                } else {
                    self.enabled = false;
                    // TODO: Handle what (else) happens if the node is not found
                }
            }
            NodeMsg::Volume(volume) => {
                self.pw_sender.send(ToPipewireMessage::ChangeVolume(
                    self.node.id,
                    volume.powf(3.0) as f32,
                ));
            }
            NodeMsg::Remove => {
                SONUSMIX_STATE.emit(SonusmixMsg::RemoveNode(self.node.id, self.list));
            }
        }
    }
}

fn calculate_slider_value(channel_volumes: &Vec<f32>) -> f64 {
    ((channel_volumes.iter().sum::<f32>() / channel_volumes.len().max(1) as f32) as f64)
        .powf(1.0 / 3.0)
}
