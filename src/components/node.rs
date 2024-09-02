use std::{cell::RefCell, rc::Rc, sync::LazyLock};

use relm4::gtk::pango::AttrList;
use relm4::gtk::prelude::*;
use relm4::prelude::*;

use crate::pipewire_api::{Graph, Node as PwNode};

const NAME_STYLE: LazyLock<AttrList> =
    LazyLock::new(|| AttrList::from_string("0 -1 size large").expect("failed to parse style"));

pub struct Node {
    pub node: PwNode,
}

#[derive(Debug, Clone)]
pub enum NodeMsg {
    Refresh,
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

                gtk::Label {
                    #[watch]
                    set_label: &self.node.name,
                    // set_attributes: Some(&*NAME_STYLE),
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
}
