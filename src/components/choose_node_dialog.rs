use std::convert::Infallible;
use std::sync::Arc;

use gtk::glib::Propagation;
use log::debug;
use relm4::factory::FactoryVecDeque;
use relm4::gtk::prelude::*;
use relm4::prelude::*;

use crate::{
    graph_events::subscribe_to_pipewire,
    pipewire_api::{Graph, Node, PortKind},
};

pub struct ChooseNodeDialog {
    graph: Arc<Graph>,
    list: PortKind,
    active_nodes: FactoryVecDeque<ChooseNodeItem>,
    entry_buffer: gtk::EntryBuffer,
    visible: bool,
    list_visible: bool,
}

impl ChooseNodeDialog {
    pub fn active_list(&self) -> Option<PortKind> {
        self.visible.then_some(self.list)
    }

    fn build_factory(&mut self) {
        let mut factory = self.active_nodes.guard();
        factory.clear();
        for node in self
            .node_ids
            .iter()
            .filter_map(|id| self.graph.nodes.get(&id))
            .cloned()
        {
            factory.push_back(node);
        }
    }
}

#[derive(Debug)]
pub enum ChooseNodeDialogMsg {
    UpdateGraph(Arc<Graph>),
    Show(PortKind, Vec<u32>),
    #[doc(hidden)]
    Close,
    #[doc(hidden)]
    NodeChosen(u32, DynamicIndex),
    #[doc(hidden)]
    SearchUpdated,
}

#[derive(Debug)]
pub enum ChooseNodeDialogOutput {
    Closed,
    NodeChosen(PortKind, u32),
}

#[relm4::component(pub)]
impl SimpleComponent for ChooseNodeDialog {
    type Init = ();
    type Input = ChooseNodeDialogMsg;
    type Output = ChooseNodeDialogOutput;

    view! {
        gtk::Window {
            set_modal: true,
            #[watch]
            set_title: Some(match model.list {
                PortKind::Source => "Choose Source",
                PortKind::Sink => "Choose Sink",
            }),
            #[watch]
            set_visible: model.visible,

            connect_close_request[sender] => move |_| {
                sender.input(ChooseNodeDialogMsg::Close);
                Propagation::Stop
            },

            gtk::Entry {
                set_visible: true,
                set_buffer: &model.entry_buffer,

                connect_changed[sender] => move |_| {
                    sender.input(ChooseNodeDialogMsg::SearchUpdated)
                }
            },

            #[local_ref]
            nodes_box -> gtk::Box {
                #[watch]
                set_visible: model.list_visible,
                set_orientation: gtk::Orientation::Vertical,
            },

            gtk::Box {
                set_vexpand: true,
                set_valign: gtk::Align::Center,
                set_halign: gtk::Align::Center,

                gtk::Label {
                    #[watch]
                    set_visible: model.active_nodes.is_empty(),
                    set_text: "No object found"
                }
            },
        }
    }

    fn init(_init: (), root: Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {
        let graph = subscribe_to_pipewire(sender.input_sender(), ChooseNodeDialogMsg::UpdateGraph);

        let active_nodes = FactoryVecDeque::builder()
            .launch(gtk::Box::default())
            .forward(sender.input_sender(), |NodeChosen(id, index)| {
                ChooseNodeDialogMsg::NodeChosen(id, index)
            });

        let model = ChooseNodeDialog {
            graph,
            list: PortKind::Source,
            active_nodes,
            entry_buffer: gtk::EntryBuffer::new(Some("")),
            visible: false,
            list_visible: true,
        };

        let nodes_box = model.active_nodes.widget();
        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: ChooseNodeDialogMsg, sender: ComponentSender<Self>) {
        match msg {
            ChooseNodeDialogMsg::UpdateGraph(graph) => {
                self.graph = graph;
            }
            ChooseNodeDialogMsg::Show(list, node_ids) => {
                self.list = list;
                self.build_factory();
                self.visible = true;
            }
            ChooseNodeDialogMsg::Close => {
                self.visible = false;
            }
            ChooseNodeDialogMsg::NodeChosen(id, index) => {
                self.active_nodes.guard().remove(index.current_index());
                let _ = sender.output(ChooseNodeDialogOutput::NodeChosen(self.list, id));
            }
            ChooseNodeDialogMsg::SearchUpdated => {
                let text = self.entry_buffer.text();

                if text.is_empty() {
                    self.build_factory();
                    return;
                }

                // hide current list
                self.list_visible = false;

                // search only by node id if only number
                if let Ok(num) = &text.parse::<u32>() {
                    let mut factory = self.active_nodes.guard();
                    factory.clear();
                    if let Some(node) = self.graph.nodes.get(num) {
                        factory.push_back(node.clone());
                    }
                }

                self.list_visible = true;
                // TODO: fuzzy search
            }
        }
    }
}

struct ChooseNodeItem(Node);

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
