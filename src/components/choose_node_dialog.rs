use std::convert::Infallible;
use std::sync::Arc;

use gtk::glib::Propagation;
use log::debug;
use relm4::factory::FactoryVecDeque;
use relm4::gtk::prelude::*;
use relm4::prelude::*;

use crate::{
    pipewire_api::{Graph, Node, PortKind},
    state::{subscribe_to_pipewire, SonusmixMsg, SonusmixState, SONUSMIX_STATE},
};

pub struct ChooseNodeDialog {
    graph: Arc<Graph>,
    sonusmix_state: Arc<SonusmixState>,
    list: PortKind,
    nodes: FactoryVecDeque<ChooseNodeItem>,
    entry_buffer: gtk::EntryBuffer,
    visible: bool,
    list_visible: bool,
}

#[derive(Debug)]
pub enum ChooseNodeDialogMsg {
    UpdateGraph(Arc<Graph>),
    SonusmixState(Arc<SonusmixState>),
    Show(PortKind),
    #[doc(hidden)]
    ListChanged(PortKind),
    #[doc(hidden)]
    Close,
    #[doc(hidden)]
    NodeChosen(u32),
    #[doc(hidden)]
    SearchUpdated,
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
            set_visible: model.visible,

            add_controller = gtk::EventControllerKey {
                connect_key_pressed[sender] => move |_, key, _, _| {
                    if key == gtk::gdk::Key::Escape {
                        sender.input(ChooseNodeDialogMsg::Close);
                        Propagation::Stop
                    } else {
                        Propagation::Proceed
                    }
                }
            },

            connect_close_request[sender] => move |_| {
                sender.input(ChooseNodeDialogMsg::Close);
                Propagation::Stop
            },

            #[wrap(Some)]
            set_titlebar = &gtk::HeaderBar {
                #[name(list_switcher_dummy)]
                pack_start = &gtk::Stack {

                    connect_visible_child_name_notify[sender] => move |stack| {
                        match stack.visible_child_name().as_ref().map(|name| name.as_str()) {
                            Some("Sources") => sender.input(ChooseNodeDialogMsg::ListChanged(PortKind::Source)),
                            Some("Sinks") => sender.input(ChooseNodeDialogMsg::ListChanged(PortKind::Sink)),
                            _ => {}
                        }
                    } @list_change_handler,

                    add_titled[Some("Sources"), "Sources"] = &gtk::Box {},
                    add_titled[Some("Sinks"), "Sinks"] = &gtk::Box {},

                    // This needs to be set after the children are added, so it's at the bottom
                    // instead of the top
                    #[watch]
                    #[block_signal(list_change_handler)]
                    set_visible_child_name: match model.list {
                        PortKind::Source => "Sources",
                        PortKind::Sink => "Sinks",
                    },
                },

                #[wrap(Some)]
                set_title_widget = &gtk::StackSwitcher {
                    set_visible: true,
                    set_stack: Some(&list_switcher_dummy),
                }
            },

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,

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

                gtk::Label {
                    set_vexpand: true,
                    set_valign: gtk::Align::Center,
                    set_halign: gtk::Align::Center,
                    #[watch]
                    set_visible: model.nodes.is_empty(),
                    set_text: "No object found"
                }
            },
        }
    }

    fn init(_init: (), root: Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {
        let graph = subscribe_to_pipewire(sender.input_sender(), ChooseNodeDialogMsg::UpdateGraph);
        let sonusmix_state =
            SONUSMIX_STATE.subscribe(sender.input_sender(), ChooseNodeDialogMsg::SonusmixState);

        let nodes = FactoryVecDeque::builder()
            .launch(gtk::Box::default())
            .forward(sender.input_sender(), ChooseNodeDialogMsg::NodeChosen);

        let model = ChooseNodeDialog {
            graph,
            sonusmix_state,
            list: PortKind::Source,
            nodes,
            entry_buffer: gtk::EntryBuffer::new(Some("")),
            visible: false,
            list_visible: true,
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
            ChooseNodeDialogMsg::ListChanged(list) => {
                self.list = list;
                self.update_inactive_nodes();
            }
            ChooseNodeDialogMsg::Close => {
                self.visible = false;
            }
            ChooseNodeDialogMsg::NodeChosen(id) => {
                SONUSMIX_STATE.emit(SonusmixMsg::AddNode(id, self.list));
            }
            ChooseNodeDialogMsg::SearchUpdated => {
                let text = self.entry_buffer.text();

                if text.is_empty() {
                    self.update_inactive_nodes();
                    return;
                }

                // hide current list
                self.list_visible = false;

                let active = match self.list {
                    PortKind::Source => self.sonusmix_state.active_sources.as_slice(),
                    PortKind::Sink => self.sonusmix_state.active_sinks.as_slice(),
                };

                // search only by node id if only number
                if let Ok(num) = &text.parse::<u32>() {
                    if active.contains(num) {
                        return;
                    }
                    let mut factory = self.nodes.guard();
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

#[relm4::factory]
impl FactoryComponent for ChooseNodeItem {
    type Init = Node;
    type Input = Infallible;
    type Output = u32;
    type CommandOutput = ();
    type ParentWidget = gtk::Box;

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Horizontal,
            set_spacing: 8,

            gtk::Button {
                set_icon_name: "list-add-symbolic",
                connect_clicked[sender, id = self.0.id] => move |_| {
                    let _ = sender.output(id);
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
