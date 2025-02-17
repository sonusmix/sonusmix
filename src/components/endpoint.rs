use std::sync::Arc;

use gtk::glib::Propagation;
use relm4::actions::RelmAction;
use relm4::factory::FactoryView;
use relm4::prelude::*;
use relm4::{actions::RelmActionGroup, gtk::prelude::*};

use crate::pipewire_api::PortKind;
use crate::state::settings::SonusmixSettings;
use crate::state::{
    Endpoint as PwEndpoint, EndpointDescriptor, SonusmixMsg, SonusmixReducer, SonusmixState,
    SONUSMIX_SETTINGS,
};

use super::connect_endpoints::ConnectEndpoints;

pub struct Endpoint {
    endpoint: PwEndpoint,
    settings: SonusmixSettings,
    renaming: bool,
    custom_name_buffer: gtk::EntryBuffer,
    connect_endpoints: Controller<ConnectEndpoints>,
    details_short: String,
    details_long: String,
}

impl Endpoint {
    pub fn id(&self) -> EndpointDescriptor {
        self.endpoint.descriptor
    }
}

#[derive(Debug, Clone)]
pub enum EndpointMsg {
    UpdateState(Arc<SonusmixState>),
    UpdateSettings(SonusmixSettings),
    Volume(f64),
    ToggleMute,
    ToggleLocked,
    Remove,
    StartRename,
    /// true if confirmed, false if cancelled
    FinishRename(bool),
    ResetName,
}

#[derive(Debug, Clone)]
pub enum EndpointOutput {}

relm4::new_action_group!(EndpointMenuActionGroup, "endpoint-menu");
relm4::new_stateless_action!(RemoveAction, EndpointMenuActionGroup, "remove");
relm4::new_stateless_action!(RenameAction, EndpointMenuActionGroup, "rename");
relm4::new_stateless_action!(ResetNameAction, EndpointMenuActionGroup, "reset-name");

#[relm4::factory(pub)]
impl FactoryComponent for Endpoint {
    type Init = (EndpointDescriptor, PortKind);
    type Input = EndpointMsg;
    type Output = EndpointOutput;
    type CommandOutput = ();
    type ParentWidget = gtk::Box;

    view! {
        #[root]
        gtk::Box {
            set_orientation: gtk::Orientation::Horizontal,
            set_spacing: 8,
            set_margin_all: 4,

            gtk::Box {
                set_hexpand: true,
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 10,

                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,

                    if self.endpoint.is_placeholder {
                        #[name(placeholder_warning_icon)]
                        gtk::Image {
                            set_margin_end: 8,
                            #[watch]
                            set_pixel_size: placeholder_warning_icon.pixel_size().max(24),
                            set_icon_name: Some("dialog-question"),
                            set_tooltip: "Missing node",
                        }
                    } else {
                        #[name(icon_view)]
                        gtk::Image {
                            set_margin_end: 8,
                            // Some icon themes use symbolic-only icons below a certain size.
                            // Unfortunately, because Gtk thinks they aren't symbolic, it doesn't
                            // properly recolor them, so here we let the Gtk theme set the icon size,
                            // while ensuring that the icons don't get too small.
                            #[watch]
                            set_pixel_size: icon_view.pixel_size().max(24),
                            #[watch]
                            set_icon_name: Some(&self.endpoint.icon_name),
                        }
                    },

                    if self.renaming {
                        gtk::Entry::with_buffer(&self.custom_name_buffer) {
                            connect_map => |entry| { entry.grab_focus(); },
                            connect_activate => EndpointMsg::FinishRename(true),

                            // Add an event controller to cancel renaming on Esc
                            add_controller = gtk::EventControllerKey {
                                connect_key_pressed[sender] => move |_, key, _, _| {
                                    if key == gtk::gdk::Key::Escape {
                                        sender.input(EndpointMsg::FinishRename(false));
                                        Propagation::Stop
                                    } else {
                                        Propagation::Proceed
                                    }
                                }
                            },
                            add_controller = gtk::EventControllerFocus {
                                connect_leave => EndpointMsg::FinishRename(false),
                            }
                        }
                    } else {
                        gtk::Label {
                            set_hexpand: true,
                            set_halign: gtk::Align::Start,
                            set_ellipsize: gtk::pango::EllipsizeMode::End,

                            #[watch]
                            set_label: self.endpoint.custom_or_display_name(),
                            #[watch]
                            set_tooltip: self.endpoint.custom_or_display_name(),
                            #[watch]
                            set_css_classes: if self.endpoint.is_placeholder
                                { &["heading", "dim-label"] } else { &["heading"] },

                            // Start renaming when the user double-clicks the endpoint name
                            add_controller = gtk::GestureClick {
                                connect_released[sender] => move |_, num_presses, _, _| {
                                    if num_presses >= 2 {
                                        sender.input(EndpointMsg::StartRename);
                                    }
                                }
                            }
                        }
                    },

                    gtk::Label {
                        #[watch]
                        set_label: &match self.endpoint.descriptor {
                            EndpointDescriptor::EphemeralNode(id, _) => format!("id: {}", id),
                            _ => String::new(),
                        }
                    }
                },
                gtk::Label {
                    set_halign: gtk::Align::Start,
                    set_ellipsize: gtk::pango::EllipsizeMode::End,
                    set_css_classes: &["caption", "dim-label"],

                    #[watch]
                    set_label: if self.endpoint.is_placeholder {
                        "Inactive"
                    } else {
                        &self.details_short
                    },
                    #[watch]
                    set_tooltip: if self.endpoint.is_placeholder {
                        "This endpoint is not active. You may reconnect or recreate this endpoint."
                    } else {
                        &self.details_long
                    }
                },
                gtk::Scale {
                    #[watch]
                    set_range: (0.0, self.settings.volume_limit),
                    set_increments: (1.0, 5.0),
                    set_draw_value: true,
                    #[watch]
                    clear_marks: (),
                    #[watch]
                    add_mark: (100.0, gtk::PositionType::Bottom, None),
                    set_format_value_func => move |_, value| format!("{value:.0}%"),

                    #[watch]
                    #[block_signal(volume_handler)]
                    set_value: volume_to_slider(self.endpoint.volume),
                    connect_value_changed[sender] => move |scale| {
                        sender.input(EndpointMsg::Volume(scale.value()));
                    } @volume_handler
                }
            },

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 5,

                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 5,

                    gtk::MenuButton {
                        set_label: "Connections",
                        set_popover: Some(self.connect_endpoints.widget()),
                    },
                    #[name(endpoint_menu_button)]
                    gtk::MenuButton {
                        set_icon_name: "view-more-symbolic",
                        set_menu_model: Some(&endpoint_menu),
                    },
                },
                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 4,

                    #[name(mute_button)]
                    gtk::ToggleButton {
                        #[watch]
                        set_icon_name: if mute_button.is_active()
                            { "audio-volume-muted-symbolic" } else { "audio-volume-high-symbolic" },
                        #[watch]
                        set_tooltip: if mute_button.is_active()
                            { "Unmute" } else { "Mute" },
                        #[watch]
                        set_css_classes: if mute_button.is_active()
                            { &["destructive-action", "image-button"] } else { &["flat", "image-button"] },

                        #[watch]
                        set_active: self.endpoint.volume_locked_muted.is_muted().unwrap_or(false),
                        connect_clicked => EndpointMsg::ToggleMute,
                    },
                    #[name(volume_lock_button)]
                    gtk::ToggleButton {
                        add_css_class: "flat",

                        #[watch]
                        set_icon_name: if volume_lock_button.is_active()
                            { "changes-prevent-symbolic" } else { "changes-allow-symbolic" },
                        #[watch]
                        set_tooltip: if volume_lock_button.is_active()
                        {
                            "Allow volume changes outside of Sonusmix"
                        } else {
                            "Prevent volume changes outside of Sonusmix"
                        },

                        #[watch]
                        set_active: self.endpoint.volume_locked_muted.is_locked(),
                        connect_clicked => EndpointMsg::ToggleLocked,
                    },
                    gtk::Button {
                        set_label: "P",
                        set_tooltip: "Primary",
                    }
                }
            }
        },
    }

    menu! {
        endpoint_menu: {
            "Remove" => RemoveAction,
            "Rename" => RenameAction,
            "Reset Name" => ResetNameAction,
        }
    }

    fn init_model(
        (endpoint_desc, list): (EndpointDescriptor, PortKind),
        _index: &DynamicIndex,
        sender: FactorySender<Self>,
    ) -> Self {
        let endpoint = SonusmixReducer::subscribe(sender.input_sender(), EndpointMsg::UpdateState)
            .endpoints
            .get(&endpoint_desc)
            .expect("endpoint component failed to find matching endpoint on init")
            .clone();
        SONUSMIX_SETTINGS.subscribe(sender.input_sender(), |settings| {
            EndpointMsg::UpdateSettings(settings.clone())
        });
        let settings = { SONUSMIX_SETTINGS.read().clone() };
        let details_short = endpoint.details_short();
        let details_long = endpoint.details_long();

        let connect_endpoints = ConnectEndpoints::builder()
            .launch((endpoint.descriptor, list))
            .forward(sender.input_sender(), |msg| match msg {});

        let custom_name_buffer = gtk::EntryBuffer::new(None::<&str>);

        Self {
            endpoint,
            settings,
            renaming: false,
            custom_name_buffer,
            connect_endpoints,
            details_short,
            details_long,
        }
    }

    fn init_widgets(
        &mut self,
        _index: &Self::Index,
        root: Self::Root,
        _returned_widget: &<Self::ParentWidget as FactoryView>::ReturnedWidget,
        sender: FactorySender<Self>,
    ) -> Self::Widgets {
        let widgets = view_output!();

        let mut group = RelmActionGroup::<EndpointMenuActionGroup>::new();
        let remove_action: RelmAction<RemoveAction> = RelmAction::new_stateless({
            let sender = sender.clone();
            move |_| {
                sender.input(EndpointMsg::Remove);
            }
        });
        group.add_action(remove_action);
        let rename_action: RelmAction<RenameAction> = RelmAction::new_stateless({
            let sender = sender.clone();
            move |_| {
                sender.input(EndpointMsg::StartRename);
            }
        });
        group.add_action(rename_action);
        let reset_name_action: RelmAction<ResetNameAction> = RelmAction::new_stateless({
            let sender = sender.clone();
            move |_| {
                sender.input(EndpointMsg::ResetName);
            }
        });
        group.add_action(reset_name_action);
        group.register_for_widget(&widgets.endpoint_menu_button);

        widgets
    }

    fn update(&mut self, msg: EndpointMsg, _sender: FactorySender<Self>) {
        match msg {
            EndpointMsg::UpdateState(state) => {
                if let Some(endpoint) = state.endpoints.get(&self.endpoint.descriptor) {
                    self.endpoint = endpoint.clone();
                    self.details_short = self.endpoint.details_short();
                    self.details_long = self.endpoint.details_long();
                }
            }
            EndpointMsg::UpdateSettings(settings) => {
                self.settings = settings;
            }
            EndpointMsg::Volume(volume) => SonusmixReducer::emit(SonusmixMsg::SetVolume(
                self.endpoint.descriptor,
                slider_to_volume(volume),
            )),
            EndpointMsg::ToggleMute => {
                let mute = self
                    .endpoint
                    .volume_locked_muted
                    .is_muted()
                    .map(|mute| !mute)
                    .unwrap_or(true);
                SonusmixReducer::emit(SonusmixMsg::SetMute(self.endpoint.descriptor, mute));
            }
            EndpointMsg::ToggleLocked => {
                SonusmixReducer::emit(SonusmixMsg::SetVolumeLocked(
                    self.endpoint.descriptor,
                    !self.endpoint.volume_locked_muted.is_locked(),
                ));
            }
            EndpointMsg::Remove => {
                SonusmixReducer::emit(SonusmixMsg::RemoveEndpoint(self.endpoint.descriptor));
            }
            EndpointMsg::StartRename => {
                self.renaming = true;
                self.custom_name_buffer
                    .set_text(self.endpoint.custom_or_display_name());
            }
            EndpointMsg::FinishRename(confirm) => {
                self.renaming = false;
                if confirm {
                    SonusmixReducer::emit(SonusmixMsg::RenameEndpoint(
                        self.endpoint.descriptor,
                        Some(self.custom_name_buffer.text().to_string()),
                    ));
                }
            }
            EndpointMsg::ResetName => {
                self.renaming = false;
                SonusmixReducer::emit(SonusmixMsg::RenameEndpoint(self.endpoint.descriptor, None));
            }
        }
    }
}

pub fn volume_to_slider(volume: f32) -> f64 {
    (volume.powf(1.0 / 3.0) * 100.0) as f64
}

pub fn slider_to_volume(volume: f64) -> f32 {
    (volume as f32 / 100.0).powf(3.0)
}
