use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{mpsc, Arc};

use log::{debug, error};
use relm4::actions::{RelmAction, RelmActionGroup};
use relm4::factory::FactoryVecDeque;
use relm4::gtk::prelude::*;
use relm4::prelude::*;
use tempfile::TempPath;

use crate::pipewire_api::{Graph, PortKind, ToPipewireMessage};
use crate::state::{subscribe_to_pipewire, SonusmixMsg, SONUSMIX_STATE};

use super::about::{open_third_party_licenses, AboutComponent};
use super::choose_node_dialog::{ChooseNodeDialog, ChooseNodeDialogMsg};
use super::node::Node;

pub struct App {
    pw_sender: mpsc::Sender<ToPipewireMessage>,
    graph: Arc<Graph>,
    about_component: Option<Controller<AboutComponent>>,
    third_party_licenses_file: Option<TempPath>,
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
    OpenThirdPartyLicenses,
    ChooseNode(PortKind),
}

#[derive(Debug)]
pub enum CommandMsg {
    OpenThirdPartyLicenses(std::io::Result<TempPath>),
}

relm4::new_action_group!(MainMenuActionGroup, "main-menu");
relm4::new_stateless_action!(AboutAction, MainMenuActionGroup, "about");
relm4::new_stateless_action!(
    ThirdPartyLicensesAction,
    MainMenuActionGroup,
    "third-party-licenses"
);

#[relm4::component(pub)]
impl Component for App {
    type CommandOutput = CommandMsg;
    type Init = mpsc::Sender<ToPipewireMessage>;
    type Input = Msg;
    type Output = ();

    view! {
        main_window = gtk::ApplicationWindow {
            set_title: Some("Sonusmix"),
            set_default_size: (1000, 700),

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
            "View Third-Party Licenses" => ThirdPartyLicensesAction,
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
            third_party_licenses_file: None,
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
        let third_party_licenses_action: RelmAction<ThirdPartyLicensesAction> =
            RelmAction::new_stateless({
                let sender = sender.clone();
                move |_| {
                    sender.input(Msg::OpenThirdPartyLicenses);
                }
            });
        group.add_action(third_party_licenses_action);
        group.register_for_widget(&widgets.main_window);

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>, _root: &Self::Root) {
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
                SonusmixMsg::RemoveNode(id, list) => {
                    let nodes = match list {
                        PortKind::Source => &mut self.sources,
                        PortKind::Sink => &mut self.sinks,
                    };
                    let index = nodes.iter().position(|node| node.id() == id);
                    if let Some(index) = index {
                        nodes.guard().remove(index);
                    }
                }
            },
            Msg::OpenAbout => {
                self.about_component = Some(AboutComponent::builder().launch(()).detach());
            }
            Msg::OpenThirdPartyLicenses => {
                let temp_path = self.third_party_licenses_file.take();
                sender.spawn_oneshot_command(move || {
                    CommandMsg::OpenThirdPartyLicenses(open_third_party_licenses(temp_path))
                });
            }
            Msg::ChooseNode(list) => {
                let _ = self
                    .choose_node_dialog
                    .sender()
                    .send(ChooseNodeDialogMsg::Show(list));
            }
        };
    }

    fn update_cmd(
        &mut self,
        message: CommandMsg,
        _sender: ComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match message {
            CommandMsg::OpenThirdPartyLicenses(result) => match result {
                Ok(temp_path) => self.third_party_licenses_file = Some(temp_path),
                Err(err) => error!("Failed to show third party licenses: {:?}", err),
            },
        }
    }
}
