use std::sync::Arc;

use log::error;
use relm4::actions::{RelmAction, RelmActionGroup};
use relm4::factory::FactoryVecDeque;
use relm4::gtk::prelude::*;
use relm4::{prelude::*, Sender};
use tempfile::TempPath;

use crate::pipewire_api::PortKind;
use crate::state::{SonusmixMsg, SonusmixReducer, SonusmixState};

use super::about::{open_third_party_licenses, AboutComponent};
use super::choose_endpoint_dialog::{ChooseEndpointDialog, ChooseEndpointDialogMsg};
use super::endpoint::Endpoint;

pub struct App {
    sonusmix_state: Arc<SonusmixState>,
    about_component: Option<Controller<AboutComponent>>,
    third_party_licenses_file: Option<TempPath>,
    sources: FactoryVecDeque<Endpoint>,
    sinks: FactoryVecDeque<Endpoint>,
    choose_endpoint_dialog: Controller<ChooseEndpointDialog>,
}

#[derive(Debug)]
pub enum Msg {
    UpdateState(Arc<SonusmixState>, Option<SonusmixMsg>),
    OpenAbout,
    OpenThirdPartyLicenses,
    ChooseEndpoint(PortKind),
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
    type Init = ();
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
                                sender.input(Msg::ChooseEndpoint(PortKind::Source));
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

                            connect_clicked => Msg::ChooseEndpoint(PortKind::Sink),
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

    fn init(_init: (), root: Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {
        let sonusmix_state =
            SonusmixReducer::subscribe_msg(sender.input_sender(), Msg::UpdateState);

        let mut sources = FactoryVecDeque::builder()
            .launch(gtk::Box::default())
            .forward(sender.input_sender(), |output| match output {});
        {
            let mut sources = sources.guard();
            for endpoint in &sonusmix_state.active_sources {
                sources.push_back(*endpoint);
            }
        }
        let mut sinks = FactoryVecDeque::builder()
            .launch(gtk::Box::default())
            .forward(sender.input_sender(), |output| match output {});
        {
            let mut sinks = sinks.guard();
            for endpoint in &sonusmix_state.active_sinks {
                sinks.push_back(*endpoint);
            }
        }
        let choose_endpoint_dialog = ChooseEndpointDialog::builder()
            .transient_for(&root)
            .launch(())
            .detach();

        let model = App {
            about_component: None,
            third_party_licenses_file: None,
            sonusmix_state,
            sources,
            sinks,
            choose_endpoint_dialog,
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
            Msg::UpdateState(state, msg) => {
                self.sonusmix_state = state;
                // Update the choose endpoint dialog if it's open
                if let Some(list) = self.choose_endpoint_dialog.model().active_list() {
                    sender.input(Msg::ChooseEndpoint(list));
                }

                match msg {
                    Some(SonusmixMsg::AddEndpoint(endpoint)) => {
                        if endpoint.is_kind(PortKind::Source) {
                            self.sources.guard().push_back(endpoint);
                        } else {
                            self.sinks.guard().push_back(endpoint);
                        }
                        // TODO: Handle groups
                    }
                    Some(SonusmixMsg::RemoveEndpoint(endpoint_desc)) => {
                        let endpoints = if endpoint_desc.is_kind(PortKind::Source) {
                            &mut self.sources
                        } else {
                            &mut self.sinks
                        };
                        // TODO: Handle groups

                        let index = endpoints
                            .iter()
                            .position(|endpoint| endpoint.id() == endpoint_desc);
                        if let Some(index) = index {
                            endpoints.guard().remove(index);
                        }
                    }
                    _ => {}
                }
            }
            Msg::OpenAbout => {
                self.about_component = Some(AboutComponent::builder().launch(()).detach());
            }
            Msg::OpenThirdPartyLicenses => {
                let temp_path = self.third_party_licenses_file.take();
                sender.spawn_oneshot_command(move || {
                    CommandMsg::OpenThirdPartyLicenses(open_third_party_licenses(temp_path))
                });
            }
            Msg::ChooseEndpoint(list) => {
                let _ = self
                    .choose_endpoint_dialog
                    .sender()
                    .send(ChooseEndpointDialogMsg::Show(list));
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

    fn shutdown(&mut self, _widgets: &mut Self::Widgets, _output: Sender<Self::Output>) {
        SonusmixReducer::save_and_exit();
    }
}
