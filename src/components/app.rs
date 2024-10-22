use std::convert::Infallible;
use std::sync::Arc;

use log::error;
use relm4::actions::{RelmAction, RelmActionGroup};
use relm4::factory::FactoryVecDeque;
use relm4::gtk::prelude::*;
use relm4::prelude::*;
use tempfile::TempPath;

use crate::pipewire_api::PortKind;
use crate::state::settings::SonusmixSettings;
use crate::state::{
    EndpointDescriptor, GroupNodeKind, SonusmixMsg, SonusmixOutputMsg, SonusmixReducer,
    SonusmixState, SONUSMIX_SETTINGS,
};
use crate::{MainMsg, MAIN_BROKER};

use super::about::{open_third_party_licenses, AboutComponent};
use super::choose_endpoint_dialog::{ChooseEndpointDialog, ChooseEndpointDialogMsg};
use super::debug_view::{DebugView, DebugViewMsg};
use super::endpoint_list::EndpointList;
use super::group::{Group, GroupChangeWarning, GroupChangeWarningMsg, GroupOutput};
use super::settings_page::SettingsPage;

pub struct App {
    sonusmix_state: Arc<SonusmixState>,
    settings: SonusmixSettings,
    page: Page,
    about_component: Option<Controller<AboutComponent>>,
    third_party_licenses_file: Option<TempPath>,
    sources: Controller<EndpointList>,
    sinks: Controller<EndpointList>,
    groups: FactoryVecDeque<Group>,
    choose_endpoint_dialog: Controller<ChooseEndpointDialog>,
    debug_view: Controller<DebugView>,
    settings_page: Controller<SettingsPage>,
    _group_change_warning: Controller<GroupChangeWarning>,
}

#[derive(Debug)]
pub enum Msg {
    UpdateState(Arc<SonusmixState>, Option<SonusmixOutputMsg>),
    UpdateSettings(SonusmixSettings),
    BringToTop,
    AddGroupNode,
    OpenAbout,
    OpenThirdPartyLicenses,
    ChangePage(Page),
}

#[derive(Debug)]
pub enum CommandMsg {
    OpenThirdPartyLicenses(std::io::Result<TempPath>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Main,
    Settings,
}

relm4::new_action_group!(MainMenuActionGroup, "main-menu");
relm4::new_stateless_action!(AboutAction, MainMenuActionGroup, "about");
relm4::new_stateless_action!(
    ThirdPartyLicensesAction,
    MainMenuActionGroup,
    "third-party-licenses"
);
relm4::new_stateless_action!(ShowDebugViewAction, MainMenuActionGroup, "show-debug-view");

#[relm4::component(pub)]
impl Component for App {
    type CommandOutput = CommandMsg;
    type Init = ();
    type Input = Msg;
    type Output = Infallible;

    view! {
        main_window = gtk::ApplicationWindow {
            #[watch]
            set_title: Some(match model.page {
                Page::Main => "Sonusmix",
                Page::Settings => "Settings",
            }),
            set_default_size: (1100, 800),

            connect_close_request => |_| {
                MAIN_BROKER.send(MainMsg::Hide);
                gtk::glib::Propagation::Proceed
            },

            #[wrap(Some)]
            set_titlebar = &gtk::HeaderBar {
                pack_start = &gtk::Button {
                    #[watch]
                    set_visible: model.page != Page::Main,
                    set_icon_name: "go-previous-symbolic",
                    connect_clicked => Msg::ChangePage(Page::Main),
                },
                pack_end = &gtk::MenuButton {
                    set_icon_name: "view-more-symbolic",
                    set_menu_model: Some(&main_menu),
                },
                pack_end = &gtk::Button {
                    #[watch]
                    set_visible: model.page != Page::Settings,
                    set_icon_name: "preferences-system-symbolic",
                    connect_clicked => Msg::ChangePage(Page::Settings),
                },
            },

            #[transition(SlideLeftRight)]
            match model.page {
                Page::Main => gtk::Paned {
                    set_orientation: gtk::Orientation::Vertical,
                    set_margin_all: 8,
                    set_wide_handle: true,
                    set_shrink_start_child: false,
                    set_shrink_end_child: false,

                    #[wrap(Some)]
                    set_start_child = &gtk::Paned {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_wide_handle: true,
                        set_shrink_start_child: false,
                        set_shrink_end_child: false,
                        set_margin_bottom: 4,

                        #[wrap(Some)]
                        set_start_child = &gtk::Box {
                            set_margin_end: 4,

                            append: model.sources.widget(),
                        },
                        #[wrap(Some)]
                        set_end_child = &gtk::Box {
                            set_margin_start: 4,

                            append: model.sinks.widget(),
                        },
                    },

                    #[wrap(Some)]
                    set_end_child = &gtk::Frame {
                        set_margin_top: 4,

                        gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,
                            set_hexpand: true,

                            gtk::CenterBox {
                                set_orientation: gtk::Orientation::Horizontal,
                                set_margin_top: 4,
                                set_margin_start: 4,

                                #[wrap(Some)]
                                set_start_widget = &gtk::Button {
                                    set_icon_name: "list-add-symbolic",
                                    set_has_frame: true,

                                    connect_clicked => Msg::AddGroupNode,
                                },
                                #[wrap(Some)]
                                set_center_widget = &gtk::Label {
                                    set_markup: "<big>Groups/Virtual Devices</big>",
                                },
                            },
                            gtk::Separator {
                                set_orientation: gtk::Orientation::Vertical,
                                set_margin_vertical: 4,
                            },
                            if model.groups.is_empty() {
                                gtk::Label {
                                    set_vexpand: true,
                                    set_valign: gtk::Align::Center,
                                    set_halign: gtk::Align::Center,
                                    set_label: "Add some groups to control them here.",
                                }
                            } else {
                                gtk::ScrolledWindow {
                                    set_hexpand: true,
                                    set_policy: (gtk::PolicyType::Automatic, gtk::PolicyType::Never),

                                    #[local_ref]
                                    groups_list -> gtk::Box {
                                        set_orientation: gtk::Orientation::Horizontal,
                                        set_margin_all: 4,
                                        set_spacing: 8,
                                    }
                                }
                            }
                        }
                    }
                },
                Page::Settings => model.settings_page.widget().clone(),
            }
        }
    }

    menu! {
        main_menu: {
            "About" => AboutAction,
            "View Third-Party Licenses" => ThirdPartyLicensesAction,
            "Show Debug View" => ShowDebugViewAction,
        }
    }

    fn init(_init: (), root: Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {
        let sonusmix_state =
            SonusmixReducer::subscribe_msg(sender.input_sender(), Msg::UpdateState);
        SONUSMIX_SETTINGS.subscribe(sender.input_sender(), |settings| {
            Msg::UpdateSettings(settings.clone())
        });
        let settings = { SONUSMIX_SETTINGS.read().clone() };

        let choose_endpoint_dialog = ChooseEndpointDialog::builder()
            .transient_for(&root)
            .launch(())
            .detach();
        let sources = EndpointList::builder()
            .launch(PortKind::Source)
            .forward(choose_endpoint_dialog.sender(), |_| {
                ChooseEndpointDialogMsg::Show(PortKind::Source)
            });
        let sinks = EndpointList::builder()
            .launch(PortKind::Sink)
            .forward(choose_endpoint_dialog.sender(), |_| {
                ChooseEndpointDialogMsg::Show(PortKind::Sink)
            });
        let debug_view = DebugView::builder().launch(()).detach();
        let settings_page = SettingsPage::builder().launch(()).detach();
        let group_change_warning = GroupChangeWarning::builder()
            .transient_for(&root)
            .launch(())
            .detach();

        let mut groups = FactoryVecDeque::builder()
            .launch(gtk::Box::default())
            .forward(group_change_warning.sender(), |msg| match msg {
                GroupOutput::MessageWithWarning(message) => GroupChangeWarningMsg::Show(message),
            });
        {
            let mut groups = groups.guard();
            for group in sonusmix_state.group_nodes.keys() {
                groups.push_back(*group);
            }
        }

        let model = App {
            sonusmix_state,
            settings,
            page: Page::Main,
            about_component: None,
            third_party_licenses_file: None,
            sources,
            sinks,
            groups,
            choose_endpoint_dialog,
            debug_view,
            settings_page,
            _group_change_warning: group_change_warning,
        };

        let groups_list = model.groups.widget();
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
        let show_debug_view_action: RelmAction<ShowDebugViewAction> = RelmAction::new_stateless({
            let sender = model.debug_view.sender().clone();
            move |_| {
                let _ = sender.send(DebugViewMsg::SetVisible(true));
            }
        });
        group.add_action(show_debug_view_action);
        group.register_for_widget(&widgets.main_window);

        widgets.main_window.set_visible(true);

        ComponentParts { model, widgets }
    }

    fn update(
        &mut self,
        msg: Self::Input,
        sender: ComponentSender<Self>,
        root: &gtk::ApplicationWindow,
    ) {
        match msg {
            Msg::UpdateState(state, msg) => {
                self.sonusmix_state = state;
                // Update the choose endpoint dialog if it's open
                if let Some(list) = self.choose_endpoint_dialog.model().active_list() {
                    let _ = self
                        .choose_endpoint_dialog
                        .sender()
                        .send(ChooseEndpointDialogMsg::Show(list));
                }

                match msg {
                    Some(SonusmixOutputMsg::EndpointAdded(EndpointDescriptor::GroupNode(id))) => {
                        self.groups.guard().push_back(id);
                    }
                    Some(SonusmixOutputMsg::EndpointRemoved(
                        endpoint_desc @ EndpointDescriptor::GroupNode(_),
                    )) => {
                        let index = self
                            .groups
                            .iter()
                            .position(|group| group.endpoint.descriptor == endpoint_desc);
                        if let Some(index) = index {
                            self.groups.guard().remove(index);
                        }
                    }
                    _ => {}
                }
            }
            Msg::UpdateSettings(settings) => {
                self.settings = settings;
            }
            Msg::BringToTop => root.present(),
            Msg::AddGroupNode => {
                for num in 1.. {
                    let name = format!("Group {num}");
                    if self.sonusmix_state.group_nodes.values().all(|group| {
                        self.sonusmix_state
                            .endpoints
                            .get(&EndpointDescriptor::GroupNode(group.id))
                            .map(|endpoint| endpoint.display_name != name)
                            .unwrap_or(false)
                    }) {
                        SonusmixReducer::emit(SonusmixMsg::AddGroupNode(
                            name,
                            GroupNodeKind::Duplex,
                        ));
                        break;
                    }
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
            Msg::ChangePage(page) => {
                self.page = page;
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
