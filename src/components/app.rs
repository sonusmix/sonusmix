use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use log::debug;
use relm4::factory::FactoryVecDeque;
use relm4::gtk::prelude::*;
use relm4::prelude::*;
use tokio::sync::mpsc::UnboundedSender;

use crate::graph_events::subscribe_to_pipewire;
use crate::pipewire_api::{Graph, PortKind, ToPipewireMessage};

use super::about::AboutComponent;
use super::choose_node_dialog::{ChooseNodeDialog, ChooseNodeDialogMsg, ChooseNodeDialogOutput};
use super::node::Node;

pub struct App {
    pipewire_sender: UnboundedSender<ToPipewireMessage>,
    graph: Arc<Graph>,
    about_component: Option<Controller<AboutComponent>>,
    sources: FactoryVecDeque<Node>,
    sinks: FactoryVecDeque<Node>,
    choose_node_dialog: Controller<ChooseNodeDialog>,
}

#[derive(Debug)]
pub enum Msg {
    Ignore,
    UpdateGraph(Arc<Graph>),
    OpenAbout,
    ChooseNode(PortKind),
    NodeChosen(PortKind, u32),
}

#[relm4::component(pub)]
impl SimpleComponent for App {
    type Init = UnboundedSender<ToPipewireMessage>;
    type Input = Msg;
    type Output = ();

    view! {
        gtk::Window {
            set_title: Some("Sonusmix"),
            set_default_size: (800, 600),

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 8,
                set_margin_all: 8,

                gtk::Label {
                    set_markup: r#"<span size="xx-large">Hello from Sonusmix!</span>"#,
                },
                gtk::Button {
                    set_label: "About",
                    connect_clicked[sender] => move |_| {
                        sender.input(Msg::OpenAbout)
                    },
                },

                gtk::Grid {
                    set_hexpand: true,
                    set_column_homogeneous: true,

                    attach[0, 0, 1, 1] = &gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,

                        gtk::ScrolledWindow {
                            set_vexpand: true,

                            #[local_ref]
                            sources_list -> gtk::Box {
                                set_orientation: gtk::Orientation::Vertical,
                                set_margin_vertical: 4,
                            }
                        },
                        gtk::Button {
                            set_icon_name: "list-add-symbolic",
                            set_label: "Add Source",

                            connect_clicked[sender] => move |_| {
                                sender.input(Msg::ChooseNode(PortKind::Source));
                            }
                        }
                    },
                    attach[1, 0, 1, 1] = &gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,

                        gtk::ScrolledWindow {
                            set_vexpand: true,

                            #[local_ref]
                            sinks_list -> gtk::Box {
                                set_orientation: gtk::Orientation::Vertical,
                                set_margin_vertical: 4,
                            }
                        },
                        gtk::Button {
                            set_icon_name: "list-add-symbolic",
                            set_label: "Add Sink",

                            connect_clicked[sender] => move |_| {
                                sender.input(Msg::ChooseNode(PortKind::Sink));
                            }
                        }
                    },
                }
            }
        }
    }

    fn init(
        pipewire_sender: UnboundedSender<ToPipewireMessage>,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let graph = subscribe_to_pipewire(sender.input_sender(), |graph| Msg::UpdateGraph(graph));

        let sources = FactoryVecDeque::builder()
            .launch(gtk::Box::default())
            .forward(sender.input_sender(), |output| match output {});
        let sinks = FactoryVecDeque::builder()
            .launch(gtk::Box::default())
            .forward(sender.input_sender(), |output| match output {});
        let choose_node_dialog = ChooseNodeDialog::builder()
            .transient_for(&root)
            .launch(graph.clone())
            .forward(sender.input_sender(), |msg| match msg {
                ChooseNodeDialogOutput::Closed => Msg::Ignore,
                ChooseNodeDialogOutput::NodeChosen(list, id) => Msg::NodeChosen(list, id),
            });

        let model = App {
            pipewire_sender,
            about_component: None,
            graph,
            sources,
            sinks,
            choose_node_dialog,
        };

        let sources_list = model.sources.widget();
        let sinks_list = model.sinks.widget();
        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, _sender: ComponentSender<Self>) {
        match msg {
            Msg::Ignore => {}
            Msg::UpdateGraph(graph) => {
                self.graph = graph;
            }
            Msg::OpenAbout => {
                self.about_component = Some(AboutComponent::builder().launch(()).detach());
            }
            Msg::ChooseNode(list) => {
                let active = match list {
                    PortKind::Source => &self.sources,
                    PortKind::Sink => &self.sinks,
                };
                let _ = self
                    .choose_node_dialog
                    .sender()
                    .send(ChooseNodeDialogMsg::Show(
                        list,
                        get_inactive_nodes(self.graph.as_ref(), active.iter(), list),
                    ));
            }
            Msg::NodeChosen(list, id) => match list {
                PortKind::Source => {
                    self.sources.guard().push_back(id);
                }
                PortKind::Sink => {
                    self.sinks.guard().push_back(id);
                }
            },
        };
    }
}

fn get_inactive_nodes<'a>(
    graph: &Graph,
    active: impl IntoIterator<Item = &'a Node>,
    list: PortKind,
) -> Vec<u32> {
    let active: Vec<u32> = active.into_iter().map(|node| node.node.id).collect();
    graph
        .nodes
        .values()
        .filter_map(|node| {
            (!active.contains(&node.id)).then_some(node.id).filter(|_| {
                // Check if any of the node's ports correspond to `list`
                node.ports.iter().any(|id| {
                    graph
                        .ports
                        .get(&id)
                        .map(|port| port.kind == list)
                        .unwrap_or(false)
                })
            })
        })
        .collect()
}
