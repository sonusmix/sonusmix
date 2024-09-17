use std::collections::BTreeMap;
use std::convert::Infallible;
use std::sync::Arc;

use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use gtk::glib::Propagation;
use log::debug;
use relm4::factory::FactoryVecDeque;
use relm4::gtk::prelude::*;
use relm4::prelude::*;

use crate::{
    pipewire_api::{Node, PortKind},
    state2::{Endpoint, EndpointDescriptor, SonusmixMsg, SonusmixReducer, SonusmixState},
};

pub struct ChooseNodeDialog {
    sonusmix_state: Arc<SonusmixState>,
    list: PortKind,
    nodes: FactoryVecDeque<ChooseNodeItem>,
    visible: bool,
    search_text: String,
}

#[derive(Debug)]
pub enum ChooseNodeDialogMsg {
    SonusmixState(Arc<SonusmixState>),
    Show(PortKind),
    #[doc(hidden)]
    ListChanged(PortKind),
    #[doc(hidden)]
    Close,
    #[doc(hidden)]
    NodeChosen(EndpointDescriptor),
    #[doc(hidden)]
    SearchUpdated(String),
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
            set_default_size: (-1, 500),

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
                            Some("sources") => sender.input(ChooseNodeDialogMsg::ListChanged(PortKind::Source)),
                            Some("sinks") => sender.input(ChooseNodeDialogMsg::ListChanged(PortKind::Sink)),
                            _ => {}
                        }
                    } @list_change_handler,

                    add_titled[Some("sources"), "Add Sources"] = &gtk::Box {},
                    add_titled[Some("sinks"), "Add Sinks"] = &gtk::Box {},

                    // This needs to be set after the children are added, so it's at the bottom
                    // instead of the top
                    #[watch]
                    #[block_signal(list_change_handler)]
                    set_visible_child_name: match model.list {
                        PortKind::Source => "sources",
                        PortKind::Sink => "sinks",
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
                // set_margin_all: 8,

                gtk::SearchEntry {
                    set_margin_all: 8,
                    set_placeholder_text: Some("Search..."),

                    connect_search_changed[sender] => move |search| {
                        sender.input(ChooseNodeDialogMsg::SearchUpdated(String::from(search.text())));
                    }
                },

                if model.nodes.is_empty() {
                    gtk::Label {
                        set_vexpand: true,
                        set_valign: gtk::Align::Center,
                        set_halign: gtk::Align::Center,

                        #[watch]
                        set_label: &format!("No matching {} found", match model.list {
                            PortKind::Source => "sources",
                            PortKind::Sink => "sinks",
                        }),
                    }
                } else {
                    gtk::ScrolledWindow {
                        set_policy: (gtk::PolicyType::Never, gtk::PolicyType::Automatic),
                        set_propagate_natural_height: true,

                        #[local_ref]
                        nodes_list_box -> gtk::ListBox {
                            set_selection_mode: gtk::SelectionMode::None,
                            set_show_separators: true,
                        }
                    }
                }
            },
        }
    }

    fn init(_init: (), root: Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {
        let sonusmix_state =
            SonusmixReducer::subscribe(sender.input_sender(), ChooseNodeDialogMsg::SonusmixState);

        let nodes = FactoryVecDeque::builder()
            .launch(gtk::ListBox::new())
            .forward(sender.input_sender(), ChooseNodeDialogMsg::NodeChosen);

        let model = ChooseNodeDialog {
            sonusmix_state,
            list: PortKind::Source,
            nodes,
            visible: false,
            search_text: String::new(),
        };

        let nodes_list_box = model.nodes.widget();
        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: ChooseNodeDialogMsg, _sender: ComponentSender<Self>) {
        match msg {
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
            ChooseNodeDialogMsg::NodeChosen(endpoint) => {
                SonusmixReducer::emit(SonusmixMsg::AddEndpoint(endpoint));
            }
            ChooseNodeDialogMsg::SearchUpdated(search_text) => {
                self.search_text = search_text;
                self.update_inactive_nodes();
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

        let mut endpoints = self
            .sonusmix_state
            .candidates
            .iter()
            .filter(|(endpoint, _)| endpoint.is_kind(self.list))
            .collect::<Vec<_>>();

        endpoints.sort_by(|a, b| a.1.cmp(&b.1));
        if !self.search_text.is_empty() {
            let fuzzy_matcher = SkimMatcherV2::default().smart_case();
            // Computing the match twice is simpler by far, and probably isn't a performance
            // issue. If it is, we can come back to this later.
            endpoints.retain(|endpoint| {
                fuzzy_matcher
                    .fuzzy_match(endpoint.1, &self.search_text)
                    .is_some()
            });
            endpoints.sort_by_cached_key(|endpoint| {
                std::cmp::Reverse(
                    fuzzy_matcher
                        .fuzzy_match(endpoint.1, &self.search_text)
                        .expect("No non-matching nodes should be remaining in the vec"),
                )
            })
        }

        for (endpoint, name) in endpoints {
            factory.push_back((*endpoint, name.clone()));
        }
    }
}

struct ChooseNodeItem(EndpointDescriptor, String);

#[relm4::factory]
impl FactoryComponent for ChooseNodeItem {
    type Init = (EndpointDescriptor, String);
    type Input = Infallible;
    type Output = EndpointDescriptor;
    type CommandOutput = ();
    type ParentWidget = gtk::ListBox;

    view! {
        gtk::ListBoxRow {
            gtk::Box {
                set_orientation: gtk::Orientation::Horizontal,
                set_margin_end: 8,
                set_spacing: 8,

                gtk::Button {
                    set_has_frame: false,
                    set_icon_name: "list-add-symbolic",
                    connect_clicked[sender, id = self.0] => move |_| {
                        let _ = sender.output(id);
                    },
                },
                gtk::Label {
                    set_label: &self.1,
                }
            }
        }
    }

    fn init_model(
        (endpoint, name): (EndpointDescriptor, String),
        _index: &DynamicIndex,
        _sender: FactorySender<Self>,
    ) -> Self {
        Self(endpoint, name)
    }
}
