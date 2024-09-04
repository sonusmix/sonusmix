use relm4::gtk::prelude::*;
use relm4::prelude::*;

use std::sync::Arc;

use crate::pipewire_api::{Graph, PortKind};

pub struct ConnectNodes {
    graph: Arc<Graph>,
}

#[relm4::component]
impl SimpleComponent for ConnectNodes {
    type Init = (u32, PortKind, Vec<u32>);
    type Input = ();
    type Output = ();

    view! {
        gtk::Popover {

        }
    }

    fn init(
        (id, node_kind, options): Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
    }

    fn update(&mut self, msg: (), sender: ComponentSender<Self>) {}
}

struct ConnectNodeItem {
    node: Node,
}

#[derive(Debug)]
struct NodeChosen(u32, DynamicIndex);

#[relm4::factory]
impl FactoryComponent for ChooseNodeItem {
    type Init = Node;
    type Input = Infallible;
    type Output = NodeChosen;
    type CommandOutput = ();
    type ParentWidget = gtk::Box;

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Horizontal,
            set_spacing: 8,

            gtk::Button {
                set_icon_name: "list-add-symbolic",
                connect_clicked[sender, index, id = self.0.id] => move |_| {
                    let _ = sender.output(NodeChosen(id, index.clone()));
                },
            },
            gtk::Label {
                set_label: &self.0.name,
            }
        }
    }

    fn init_model(node: Node, _index: &DynamicIndex, _sender: FactorySender<Self>) -> Self {
        Self(node)
    }
}
