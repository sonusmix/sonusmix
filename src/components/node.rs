use std::{cell::RefCell, rc::Rc, sync::LazyLock};

use gtk::glib::property::PropertyGet;
use log::debug;
use relm4::gtk::pango::AttrList;
use relm4::gtk::prelude::*;
use relm4::prelude::*;

use crate::pipewire_api::{Graph, Node as PwNode};

pub struct Node {
    pub node: PwNode,
}

#[derive(Debug, Clone)]
pub enum NodeMsg {
    Refresh(Rc<RefCell<Graph>>),
}

#[derive(Debug, Clone)]
pub enum NodeOutput {}

#[relm4::factory(pub)]
impl FactoryComponent for Node {
    type Init = (Rc<RefCell<Graph>>, u32);
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
                    set_value: (self.node.channel_volumes.iter().sum::<f32>()
                        / self.node.channel_volumes.len().min(1) as f32) as f64,
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
        (graph, id): (Rc<RefCell<Graph>>, u32),
        _index: &DynamicIndex,
        _sender: FactorySender<Self>,
    ) -> Self {
        Self {
            node: graph
                .borrow()
                .nodes
                .get(&id)
                .expect("node component failed to find matching key on init")
                .clone(),
        }
    }

    fn update(&mut self, msg: NodeMsg, sender: FactorySender<Self>) {
        match msg {
            NodeMsg::Refresh(graph) => {
                self.node = graph.borrow().nodes.get(&self.node.id).expect("node removed").clone();
                // TODO: Handle what happens if the node is not found
            }
        }
    }
}
