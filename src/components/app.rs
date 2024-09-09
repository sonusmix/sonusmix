use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{mpsc, Arc};

use log::debug;
use relm4::actions::{RelmAction, RelmActionGroup};
use relm4::factory::FactoryVecDeque;
use relm4::gtk::prelude::*;
use relm4::prelude::*;

use crate::pipewire_api::{Graph, PortKind, ToPipewireMessage};
use crate::state::{subscribe_to_pipewire, SonusmixMsg, SONUSMIX_STATE};

use super::about::AboutComponent;
use super::choose_node_dialog::{ChooseNodeDialog, ChooseNodeDialogMsg};
use super::node::Node;

pub struct App {
    pw_sender: mpsc::Sender<ToPipewireMessage>,
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
    SonusmixMsg(SonusmixMsg),
    OpenAbout,
    ChooseNode(PortKind),
}

relm4::new_action_group!(MainMenuActionGroup, "main-menu");
relm4::new_stateless_action!(AboutAction, MainMenuActionGroup, "about");

#[relm4::component(pub)]
impl SimpleComponent for App {
    type Init = mpsc::Sender<ToPipewireMessage>;
    type Input = Msg;
    type Output = ();

    view! {
        main_window = gtk::ApplicationWindow {
            set_title: Some("Sonusmix"),
            set_default_size: (800, 600),

            #[wrap(Some)]
            set_titlebar = &gtk::HeaderBar {
                pack_end = &gtk::MenuButton {
                    set_icon_name: "view-more-symbolic",
                    set_menu_model: Some(&main_menu),
                },
            },

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 8,
                set_margin_all: 8,

                gtk::Grid {
                    set_hexpand: true,
                    set_column_homogeneous: true,

                    attach[0, 0, 1, 1] = &gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_margin_end: 4,

                        gtk::Label {
                            set_markup: "<big>Sources</big>",
                            add_css_class: "heading",
                        },
                        gtk::Separator {
                            set_orientation: gtk::Orientation::Vertical,
                            set_margin_vertical: 4,
                        },
                        if model.sources.is_empty() {
                            gtk::Label {
                                set_vexpand: true,
                                set_valign: gtk::Align::Center,
                                set_halign: gtk::Align::Center,

                                #[watch]
                                set_label: "Add some sources below to control them here.",
                            }
                        } else {
                            gtk::ScrolledWindow {
                                set_vexpand: true,
                                set_policy: (gtk::PolicyType::Never, gtk::PolicyType::Automatic),

                                #[local_ref]
                                sources_list -> gtk::Box {
                                    set_orientation: gtk::Orientation::Vertical,
                                    set_margin_vertical: 4,
                                }
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
                        set_margin_start: 4,

                        gtk::Label {
                            set_markup: "<big>Sinks</big>",
                            add_css_class: "heading",
                        },
                        gtk::Separator {
                            set_orientation: gtk::Orientation::Vertical,
                            set_margin_vertical: 4,
                        },
                        if model.sinks.is_empty() {
                            gtk::Label {
                                set_vexpand: true,
                                set_valign: gtk::Align::Center,
                                set_halign: gtk::Align::Center,

                                #[watch]
                                set_label: "Add some sinks below to control them here.",
                            }
                        } else {
                            gtk::ScrolledWindow {
                                set_vexpand: true,
                                set_policy: (gtk::PolicyType::Never, gtk::PolicyType::Automatic),

                                #[local_ref]
                                sinks_list -> gtk::Box {
                                    set_orientation: gtk::Orientation::Vertical,
                                    set_margin_vertical: 4,
                                }
                            }
                        },
                        gtk::Button {
                            set_icon_name: "list-add-symbolic",
                            set_label: "Add Sink",

                            connect_clicked => Msg::ChooseNode(PortKind::Sink),
                        }
                    },
                }
            }
        }
    }

    menu! {
        main_menu: {
            "About" => AboutAction,
        }
    }

    fn init(
        pw_sender: mpsc::Sender<ToPipewireMessage>,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let graph = subscribe_to_pipewire(sender.input_sender(), Msg::UpdateGraph);
        SONUSMIX_STATE.subscribe_msg(sender.input_sender(), |msg| Msg::SonusmixMsg(msg.clone()));

        let sources = FactoryVecDeque::builder()
            .launch(gtk::Box::default())
            .forward(sender.input_sender(), |output| match output {});
        let sinks = FactoryVecDeque::builder()
            .launch(gtk::Box::default())
            .forward(sender.input_sender(), |output| match output {});
        let choose_node_dialog = ChooseNodeDialog::builder()
            .transient_for(&root)
            .launch(())
            .detach();

        let model = App {
            pw_sender,
            about_component: None,
            graph,
            sources,
            sinks,
            choose_node_dialog,
        };

        let sources_list = model.sources.widget();
        let sinks_list = model.sinks.widget();
        let widgets = view_output!();

        // Set up actions
        let mut group = RelmActionGroup::<MainMenuActionGroup>::new();
        let about_action: RelmAction<AboutAction> = RelmAction::new_stateless({
            let sender = sender.clone();
            move |_| {
                sender.input(Msg::OpenAbout);
            }
        });
        group.add_action(about_action);
        group.register_for_widget(&widgets.main_window);


        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            Msg::Ignore => {}
            Msg::UpdateGraph(graph) => {
                self.graph = graph;
                // Update the choose node dialog if it's open
                if let Some(list) = self.choose_node_dialog.model().active_list() {
                    sender.input(Msg::ChooseNode(list));
                }
            }
            Msg::SonusmixMsg(s_msg) => match s_msg {
                SonusmixMsg::AddNode(id, list) => match list {
                    PortKind::Source => {
                        self.sources
                            .guard()
                            .push_back((id, list, self.pw_sender.clone()));
                    }
                    PortKind::Sink => {
                        self.sinks
                            .guard()
                            .push_back((id, list, self.pw_sender.clone()));
                    }
                },
            },
            Msg::OpenAbout => {
                self.about_component = Some(AboutComponent::builder().launch(()).detach());
            }
            Msg::ChooseNode(list) => {
                let _ = self
                    .choose_node_dialog
                    .sender()
                    .send(ChooseNodeDialogMsg::Show(list));
            }
        };
    }
}
