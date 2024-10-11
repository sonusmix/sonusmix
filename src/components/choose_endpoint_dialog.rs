use std::convert::Infallible;
use std::i64;
use std::sync::Arc;

use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use gtk::glib::Propagation;
use itertools::Itertools;
use relm4::factory::FactoryVecDeque;
use relm4::gtk::prelude::*;
use relm4::prelude::*;

use crate::{
    pipewire_api::{NodeIdentifier, PortKind},
    state::{Application, EndpointDescriptor, SonusmixMsg, SonusmixReducer, SonusmixState},
};

pub struct ChooseEndpointDialog {
    sonusmix_state: Arc<SonusmixState>,
    list: PortKind,
    nodes: FactoryVecDeque<ChooseEndpointItem>,
    applications: FactoryVecDeque<ChooseEndpointItem>,
    visible: bool,
    search_text: String,
}

#[derive(Debug)]
pub enum ChooseEndpointDialogMsg {
    SonusmixState(Arc<SonusmixState>),
    Show(PortKind),
    #[doc(hidden)]
    ListChanged(PortKind),
    #[doc(hidden)]
    Close,
    #[doc(hidden)]
    EndpointChosen(EndpointDescriptor),
    #[doc(hidden)]
    SearchUpdated(String),
}

#[relm4::component(pub)]
impl SimpleComponent for ChooseEndpointDialog {
    type Init = ();
    type Input = ChooseEndpointDialogMsg;
    type Output = Infallible;

    view! {
        #[root]
        gtk::Window {
            set_modal: true,
            #[watch]
            set_visible: model.visible,
            set_default_size: (-1, 500),

            add_controller = gtk::EventControllerKey {
                connect_key_pressed[sender] => move |_, key, _, _| {
                    if key == gtk::gdk::Key::Escape {
                        sender.input(ChooseEndpointDialogMsg::Close);
                        Propagation::Stop
                    } else {
                        Propagation::Proceed
                    }
                }
            },

            connect_close_request[sender] => move |_| {
                sender.input(ChooseEndpointDialogMsg::Close);
                Propagation::Stop
            },

            #[wrap(Some)]
            set_titlebar = &gtk::HeaderBar {
                #[name(list_switcher_dummy)]
                pack_start = &gtk::Stack {

                    connect_visible_child_name_notify[sender] => move |stack| {
                        match stack.visible_child_name().as_ref().map(|name| name.as_str()) {
                            Some("sources") => sender.input(ChooseEndpointDialogMsg::ListChanged(PortKind::Source)),
                            Some("sinks") => sender.input(ChooseEndpointDialogMsg::ListChanged(PortKind::Sink)),
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
                        sender.input(ChooseEndpointDialogMsg::SearchUpdated(String::from(search.text())));
                    }
                },

                if model.nodes.is_empty() && model.applications.is_empty() {
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

                        gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,

                            #[local_ref]
                            applications_list_box -> gtk::ListBox {
                                set_selection_mode: gtk::SelectionMode::None,
                                set_show_separators: true,
                                set_header_func[applications_label] => move |row, _| {
                                    row.set_header(
                                        // Set the header if the row is the first one
                                        (row.index() == 0).then_some(&applications_label)
                                    )
                                },
                            },
                            #[local_ref]
                            nodes_list_box -> gtk::ListBox {
                                set_selection_mode: gtk::SelectionMode::None,
                                set_show_separators: true,
                                set_header_func[nodes_label] => move |row, _| {
                                    row.set_header(
                                        // Set the header if the row is the first one
                                        (row.index() == 0).then_some(&nodes_label)
                                    )
                                },
                            },
                        },
                    }
                }
            },
        },
        #[name(applications_label)]
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,

            gtk::Label {
                set_margin_vertical: 8,
                set_css_classes: &["heading"],

                set_label: "Applications",
            },
            // The easiest way to have a bolder separator is to just use multiple separators
            gtk::Separator {
                set_orientation: gtk::Orientation::Horizontal,
            },
            gtk::Separator {
                set_orientation: gtk::Orientation::Horizontal,
            },
            gtk::Separator {
                set_orientation: gtk::Orientation::Horizontal,
            },
        },
        #[name(nodes_label)]
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,

            gtk::Label {
                set_margin_vertical: 8,
                set_css_classes: &["heading"],

                #[watch]
                set_label: match model.list {
                    PortKind::Source => "Single Sources",
                    PortKind::Sink => "Single Sinks",
                },
            },
            // The easiest way to have a bolder separator is to just use multiple separators
            gtk::Separator {
                set_orientation: gtk::Orientation::Horizontal,
            },
            gtk::Separator {
                set_orientation: gtk::Orientation::Horizontal,
            },
            gtk::Separator {
                set_orientation: gtk::Orientation::Horizontal,
            },
        },
    }

    fn init(_init: (), root: Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {
        let sonusmix_state = SonusmixReducer::subscribe(
            sender.input_sender(),
            ChooseEndpointDialogMsg::SonusmixState,
        );

        let nodes = FactoryVecDeque::builder()
            .launch(gtk::ListBox::new())
            .forward(
                sender.input_sender(),
                ChooseEndpointDialogMsg::EndpointChosen,
            );

        let applications = FactoryVecDeque::builder()
            .launch(gtk::ListBox::new())
            .forward(
                sender.input_sender(),
                ChooseEndpointDialogMsg::EndpointChosen,
            );

        let model = ChooseEndpointDialog {
            sonusmix_state,
            list: PortKind::Source,
            nodes,
            applications,
            visible: false,
            search_text: String::new(),
        };

        let nodes_list_box = model.nodes.widget();
        let applications_list_box = model.applications.widget();
        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: ChooseEndpointDialogMsg, _sender: ComponentSender<Self>) {
        match msg {
            ChooseEndpointDialogMsg::SonusmixState(state) => {
                self.sonusmix_state = state;
                if self.visible {
                    self.update_inactive_endpoints();
                }
            }
            ChooseEndpointDialogMsg::Show(list) => {
                self.list = list;
                self.update_inactive_endpoints();
                self.visible = true;
            }
            ChooseEndpointDialogMsg::ListChanged(list) => {
                self.list = list;
                self.update_inactive_endpoints();
            }
            ChooseEndpointDialogMsg::Close => {
                self.visible = false;
            }
            ChooseEndpointDialogMsg::EndpointChosen(endpoint) => {
                SonusmixReducer::emit(SonusmixMsg::AddEndpoint(endpoint));
            }
            ChooseEndpointDialogMsg::SearchUpdated(search_text) => {
                self.search_text = search_text;
                self.update_inactive_endpoints();
            }
        }
    }
}

impl ChooseEndpointDialog {
    pub fn active_list(&self) -> Option<PortKind> {
        self.visible.then_some(self.list)
    }

    fn update_inactive_endpoints(&mut self) {
        // Get candidate nodes
        let mut node_factory = self.nodes.guard();
        node_factory.clear();

        let mut nodes: Vec<&(u32, PortKind, NodeIdentifier)> = self
            .sonusmix_state
            .candidates
            .iter()
            .filter(|(_, kind, _)| *kind == self.list)
            .collect();
        nodes.sort_by(|a, b| a.2.human_name().cmp(b.2.human_name()));

        // Get candidate applications
        let mut application_factory = self.applications.guard();
        application_factory.clear();

        let mut applications: Vec<(&Application, Vec<&(u32, PortKind, NodeIdentifier)>)> = self
            .sonusmix_state
            .applications
            .values()
            .filter(|application| application.kind == self.list)
            .map(|application| (application, Vec::new()))
            .collect();
        applications.sort_by_key(|(application, _)| &application.name);

        // Sort and filter the nodes
        if !self.search_text.is_empty() {
            let fuzzy_matcher = SkimMatcherV2::default().smart_case();
            // Computing the match twice is simpler by far, and probably isn't a performance
            // issue. If it is, we can come back to this later.
            nodes.retain(|node| {
                fuzzy_matcher
                    .fuzzy_match(node.2.human_name(), &self.search_text)
                    .is_some()
            });
            nodes.sort_by_cached_key(|node| {
                std::cmp::Reverse(
                    fuzzy_matcher
                        .fuzzy_match(node.2.human_name(), &self.search_text)
                        .expect("No non-matching nodes should be remaining in the vec"),
                )
            });
        }

        // Associate nodes with applications
        // TODO: extract_if() would be perfect here, but requires nightly. If the performance
        // difference is big enough, it might be worth it.
        let mut i = 0;
        while i < nodes.len() {
            // If the node matches an application, remove it from nodes and add it to that
            // application
            if let Some((_, app_nodes)) = applications
                .iter_mut()
                .find(|(application, _)| application.matches(&nodes[i].2, nodes[i].1))
            {
                app_nodes.push(nodes.remove(i));
            } else {
                i += 1;
            }
        }

        // Sort and filter the applications
        if !self.search_text.is_empty() {
            let fuzzy_matcher = SkimMatcherV2::default().smart_case();

            applications.retain(|(application, nodes)| {
                // Keep an application if it matches, or if it has any matching nodes
                !nodes.is_empty()
                    || fuzzy_matcher
                        .fuzzy_match(&application.name, &self.search_text)
                        .is_some()
            });
            applications.sort_by_cached_key(|(application, nodes)| {
                let search_name = std::iter::once(application.name.as_str())
                    .chain(nodes.iter().map(|node| node.2.human_name()))
                    .join(" ");
                std::cmp::Reverse(
                    fuzzy_matcher
                        .fuzzy_match(&search_name, &self.search_text)
                        .unwrap_or(i64::MIN),
                )
            });
        }

        for (id, kind, identifier) in nodes {
            node_factory.push_back((
                EndpointDescriptor::EphemeralNode(*id, *kind),
                identifier.human_name().to_owned(),
                identifier.details().map(ToOwned::to_owned),
                ChooseEndpointItemMode::Normal,
            ));
        }

        for (application, nodes) in applications {
            if application.is_active && nodes.is_empty() {
                continue;
            }

            application_factory.push_back((
                EndpointDescriptor::Application(application.id, application.kind),
                application.name_with_tag(),
                None,
                // Add active applications as text only so they display their children but cannot
                // be selected
                if application.is_active {
                    ChooseEndpointItemMode::TextOnly
                } else {
                    ChooseEndpointItemMode::Normal
                },
            ));

            for (id, kind, identifier) in nodes {
                application_factory.push_back((
                    EndpointDescriptor::EphemeralNode(*id, *kind),
                    identifier.human_name().to_owned(),
                    identifier.details().map(ToOwned::to_owned),
                    ChooseEndpointItemMode::Nested,
                ));
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChooseEndpointItemMode {
    Normal,
    Nested,
    TextOnly,
}

struct ChooseEndpointItem {
    descriptor: EndpointDescriptor,
    name: String,
    details: Option<String>,
    tooltip: String,
    mode: ChooseEndpointItemMode,
}

#[relm4::factory]
impl FactoryComponent for ChooseEndpointItem {
    type Init = (
        EndpointDescriptor,
        String,
        Option<String>,
        ChooseEndpointItemMode,
    );
    type Input = Infallible;
    type Output = EndpointDescriptor;
    type CommandOutput = ();
    type ParentWidget = gtk::ListBox;

    view! {
        gtk::ListBoxRow {
            set_tooltip: &self.tooltip,

            match self.mode {
                ChooseEndpointItemMode::Normal | ChooseEndpointItemMode::Nested => gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_margin_end: 8,
                    set_spacing: 8,
                    set_margin_start: if self.mode == ChooseEndpointItemMode::Nested { 16 } else { 0 },

                    gtk::Button {
                        set_has_frame: false,
                        set_icon_name: "list-add-symbolic",
                        connect_clicked[sender, descriptor = self.descriptor] => move |_| {
                            let _ = sender.output(descriptor);
                        },
                    },
                    if let Some(details) = &self.details {
                        gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,

                            gtk::Label {
                                set_halign: gtk::Align::Start,
                                set_label: &self.name,
                            },
                            gtk::Label {
                                set_halign: gtk::Align::Start,
                                set_ellipsize: gtk::pango::EllipsizeMode::End,
                                set_css_classes: &["caption", "dim-label"],

                                #[watch]
                                set_label: &details,
                            },
                        }
                    } else {
                        gtk::Label {
                            set_label: &self.name,
                        }
                    }
                }
                ChooseEndpointItemMode::TextOnly => gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_margin_horizontal: 8,

                    gtk::Label {
                        set_label: &self.name,
                    }
                }
            }
        }
    }

    fn init_model(
        (descriptor, name, details, mode): (
            EndpointDescriptor,
            String,
            Option<String>,
            ChooseEndpointItemMode,
        ),
        _index: &DynamicIndex,
        _sender: FactorySender<Self>,
    ) -> Self {
        let tooltip = if let Some(details) = &details {
            format!("{}\n\n{}", name, details)
        } else {
            name.clone()
        };

        Self {
            descriptor,
            name,
            details,
            tooltip,
            mode,
        }
    }
}
