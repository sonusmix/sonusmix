use std::sync::{mpsc, Arc};

use log::debug;
use relm4::gtk::prelude::*;
use relm4::prelude::*;

use crate::{
    state::subscribe_to_pipewire,
    pipewire_api::{Graph, Node as PwNode, ToPipewireMessage},
};

pub struct Node {
    node: PwNode,
    enabled: bool,
    pw_sender: mpsc::Sender<ToPipewireMessage>,
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
                    set_value: calculate_slider_value(&self.node.channel_volumes),

                    connect_value_changed[sender] => move |scale| {
                        sender.input(NodeMsg::Volume(scale.value()));
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
            node: subscribe_to_pipewire(sender.input_sender(), NodeMsg::UpdateGraph)
                .nodes
                .get(&id)
                .expect("node component failed to find matching node on init")
                .clone(),
            enabled: true,
            pw_sender,
        }
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
                if calculate_slider_value(&self.node.channel_volumes) == volume {
                    return;
                }
                self.pw_sender.send(ToPipewireMessage::ChangeVolume(
                    self.node.id,
                    volume.powf(3.0) as f32,
                ));
                // self.sent = true;
            }
        }
    }
}

fn calculate_slider_value(channel_volumes: &Vec<f32>) -> f64 {
    ((channel_volumes.iter().sum::<f32>() / channel_volumes.len().max(1) as f32) as f64)
        .powf(1.0 / 3.0)
}
