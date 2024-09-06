use std::convert::Infallible;
use std::sync::Arc;

use gtk::glib::Propagation;
use log::debug;
use relm4::factory::FactoryVecDeque;
use relm4::gtk::prelude::*;
use relm4::prelude::*;

use crate::{
    state::{subscribe_to_pipewire, SonusmixMsg, SonusmixState, SONUSMIX_STATE},
    pipewire_api::{Graph, Node, PortKind},
};

pub struct ChooseNodeDialog {
    graph: Arc<Graph>,
    sonusmix_state: Arc<SonusmixState>,
    list: PortKind,
    nodes: FactoryVecDeque<ChooseNodeItem>,
    visible: bool,
}

#[derive(Debug)]
pub enum ChooseNodeDialogMsg {
    UpdateGraph(Arc<Graph>),
    SonusmixState(Arc<SonusmixState>),
    Show(PortKind),
    #[doc(hidden)]
    Close,
    #[doc(hidden)]
    NodeChosen(u32, DynamicIndex),
}

#[relm4::component(pub)]
impl SimpleComponent for ChooseNodeDialog {
    type Init = ();
    type Input = ChooseNodeDialogMsg;
    type Output = Infallible;

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

            #[local_ref]
            nodes_box -> gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
            },
        }
    }

    fn init(_init: (), root: Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {
        let graph = subscribe_to_pipewire(sender.input_sender(), ChooseNodeDialogMsg::UpdateGraph);
        let sonusmix_state = SONUSMIX_STATE.subscribe(sender.input_sender(), ChooseNodeDialogMsg::SonusmixState);

        let nodes = FactoryVecDeque::builder()
            .launch(gtk::Box::default())
            .forward(sender.input_sender(), |NodeChosen(id, index)| {
                ChooseNodeDialogMsg::NodeChosen(id, index)
            });

        let model = ChooseNodeDialog {
            graph,
            sonusmix_state,
            list: PortKind::Source,
            nodes,
            visible: false,
        };

        let nodes_box = model.nodes.widget();
        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: ChooseNodeDialogMsg, _sender: ComponentSender<Self>) {
        match msg {
            ChooseNodeDialogMsg::UpdateGraph(graph) => {
                self.graph = graph;
            }
            ChooseNodeDialogMsg::SonusmixState(state) => {
                self.sonusmix_state = state;
                self.update_inactive_nodes();
            }
            ChooseNodeDialogMsg::Show(list) => {
                self.list = list;
                self.update_inactive_nodes();
                self.visible = true;
            }
            ChooseNodeDialogMsg::Close => {
                self.visible = false;
            }
            ChooseNodeDialogMsg::NodeChosen(id, index) => {
                SONUSMIX_STATE.emit(SonusmixMsg::AddNode(id, self.list));
            }
        }
    }
}

impl ChooseNodeDialog {
    pub fn active_list(&self) -> Option<PortKind> {
        self.visible.then_some(self.list)
    }

    fn update_inactive_nodes(&mut self) {
        let active = match self.list {
            PortKind::Source => self.sonusmix_state.active_sources.as_slice(),
            PortKind::Sink => self.sonusmix_state.active_sinks.as_slice(),
        };
        let mut factory = self.nodes.guard();
        factory.clear();
        for node in self
            .graph
            .nodes
            .values()
            .filter(|node| {
                !active.contains(&node.id)
                    && node.ports.iter().any(|id| {
                        self.graph
                            .ports
                            .get(&id)
                            .map(|port| port.kind == self.list)
                            .unwrap_or(false)
                    })
            })
            .cloned()
        {
            factory.push_back(node);
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
