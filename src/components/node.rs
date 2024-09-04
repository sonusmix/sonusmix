use std::sync::{mpsc, Arc};

use log::debug;
use relm4::gtk::prelude::*;
use relm4::prelude::*;

use crate::{
    graph_events::subscribe_to_pipewire,
    pipewire_api::{Graph, Node as PwNode, ToPipewireMessage},
};

pub struct Node {
    pub node: PwNode,
    enabled: bool,
    dedupe_volume_update: bool,
    sent: bool,
    pw_sender: mpsc::Sender<ToPipewireMessage>,
}

#[derive(Debug, Clone)]
pub enum NodeMsg {
    Refresh(Arc<Graph>),
    #[doc(hidden)]
    Volume(f64),
    SetToFifty,
}

#[derive(Debug, Clone)]
pub enum NodeOutput {}

#[relm4::factory(pub)]
impl FactoryComponent for Node {
    type Init = (u32, mpsc::Sender<ToPipewireMessage>);
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

                    gtk::Label {
                        set_hexpand: true,
                        #[watch]
                        set_label: &self.node.name,
                        set_css_classes: &["heading"],
                    },

                    gtk::Label {
                        #[watch]
                        set_label: &format!("id: {}", self.node.id),
                    }
                },

                gtk::Scale {
                    set_range: (0.0, 1.0),
                    #[watch]
                    set_value: ((self.node.channel_volumes.iter().sum::<f32>()
                        / self.node.channel_volumes.len().max(1) as f32) as f64).powf(1.0 / 3.0),

                    connect_value_changed[sender] => move |scale| {
                        sender.input(NodeMsg::Volume(scale.value()));
                    }
                },

                gtk::Button {
                    set_label: "set to 0.5",

                    connect_clicked[sender] => move |_| {
                        sender.input(NodeMsg::SetToFifty);
                    }
                }
            },

            gtk::Box {
                gtk::Label {
                    set_label: "THIS IS WHERE SOME\nMORE STUFF WILL GO",
                }
            }
        }
    }

    fn init_model(
        (id, pw_sender): (u32, mpsc::Sender<ToPipewireMessage>),
        _index: &DynamicIndex,
        sender: FactorySender<Self>,
    ) -> Self {
        Self {
            node: subscribe_to_pipewire(sender.input_sender(), NodeMsg::Refresh)
                .nodes
                .get(&id)
                .expect("node component failed to find matching key on init")
                .clone(),
            enabled: true,
            dedupe_volume_update: false,
            sent: false,
            pw_sender,
        }
    }

    fn update(&mut self, msg: NodeMsg, _sender: FactorySender<Self>) {
        match msg {
            NodeMsg::Refresh(graph) => {
                if let Some(node) = graph.nodes.get(&self.node.id) {
                    let old_node = std::mem::replace(&mut self.node, node.clone());
                    debug!("node refresh, volume: {:?}", self.node.channel_volumes);
                    if self.node.channel_volumes != old_node.channel_volumes {
                        self.dedupe_volume_update = true;
                    }
                    self.enabled = true;
                } else {
                    self.enabled = false;
                    // TODO: Handle what (else) happens if the node is not found
                }
            }
            NodeMsg::Volume(volume) => {
                if !self.dedupe_volume_update {
                    debug!("slider moved to {}", volume);
                    self.pw_sender.send(ToPipewireMessage::ChangeVolume(
                        self.node.id,
                        volume.powf(3.0) as f32,
                    ));
                    // self.sent = true;
                } else {
                    self.dedupe_volume_update = false;
                }
            }
            NodeMsg::SetToFifty => {
                self.pw_sender.send(ToPipewireMessage::ChangeVolume(
                    self.node.id,
                    (0.5f32).powf(3.0),
                ));
            }
        }
    }
}
