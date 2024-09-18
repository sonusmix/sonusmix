use std::sync::{mpsc, Arc};

use log::debug;
use relm4::actions::RelmAction;
use relm4::factory::FactoryView;
use relm4::prelude::*;
use relm4::{actions::RelmActionGroup, gtk::prelude::*};

use crate::state::{
    Endpoint as PwEndpoint, EndpointDescriptor, SonusmixMsg, SonusmixReducer, SonusmixState,
};

use super::connect_endpoints::ConnectEndpoints;

pub struct Endpoint {
    endpoint: PwEndpoint,
    enabled: bool,
    connect_endpoints: Controller<ConnectEndpoints>,
}

impl Endpoint {
    pub fn id(&self) -> EndpointDescriptor {
        self.endpoint.descriptor
    }
}

#[derive(Debug, Clone)]
pub enum EndpointMsg {
    UpdateState(Arc<SonusmixState>),
    #[doc(hidden)]
    Volume(f64),
    #[doc(hidden)]
    ToggleMute,
    #[doc(hidden)]
    ToggleLocked,
    #[doc(hidden)]
    Remove,
}

#[derive(Debug, Clone)]
pub enum EndpointOutput {}

relm4::new_action_group!(EndpointMenuActionGroup, "endpoint-menu");
relm4::new_stateless_action!(RemoveAction, EndpointMenuActionGroup, "remove");

#[relm4::factory(pub)]
impl FactoryComponent for Endpoint {
    type Init = EndpointDescriptor;
    type Input = EndpointMsg;
    type Output = EndpointOutput;
    type CommandOutput = ();
    type ParentWidget = gtk::Box;

    view! {
        #[root]
        gtk::Box {
            set_orientation: gtk::Orientation::Horizontal,
            set_spacing: 8,
            set_margin_vertical: 4,
            set_margin_horizontal: 4,

            #[watch]
            set_sensitive: self.enabled,

            gtk::Box {
                set_hexpand: true,
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 10,

                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,

                    #[name(icon_view)]
                    gtk::Image {
                        set_margin_end: 4,
                        // Some icon themes use symbolic-only icons below a certain size.
                        // Unfortunately, because Gtk thinks they aren't symbolic, it doesn't
                        // properly recolor them, so here we let the Gtk theme set the icon size,
                        // while ensuring that the icons don't get too small.
                        #[watch]
                        set_pixel_size: icon_view.pixel_size().max(24),
                        #[watch]
                        set_icon_name: Some(&self.endpoint.icon_name),
                    },

                    gtk::Label {
                        set_hexpand: true,
                        set_halign: gtk::Align::Start,
                        set_ellipsize: gtk::pango::EllipsizeMode::End,

                        #[watch]
                        set_label: &self.endpoint.display_name,
                        #[watch]
                        set_tooltip: &self.endpoint.display_name,
                        set_css_classes: &["heading"],
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
                    set_label: &self.endpoint.details.as_ref().map(|s| s.as_str()).unwrap_or(""),
                    #[watch]
                    set_tooltip?: self.endpoint.details.as_ref(),
                },
                gtk::Scale {
                    set_range: (0.0, 100.0),
                    set_increments: (1.0, 5.0),
                    #[watch]
                    #[block_signal(volume_handler)]
                    set_value: volume_to_slider(self.endpoint.volume),
                    set_draw_value: true,
                    set_format_value_func => move |_, value| format!("{value:.0}%"),

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
                    set_spacing: 5,

                    #[name(mute_button)]
                    gtk::ToggleButton {
                        #[watch]
                        set_active: self.endpoint.volume_locked_muted.is_muted().unwrap_or(false),
                        #[watch]
                        set_icon_name: if mute_button.is_active()
                            { "audio-volume-muted-symbolic" } else { "audio-volume-high-symbolic" },
                        #[watch]
                        set_tooltip: if mute_button.is_active()
                            { "Unmute" } else { "Mute" },
                        #[watch]
                        set_css_classes: if mute_button.is_active()
                            { &["destructive-action", "image-button"] } else { &["flat", "image-button"] },

                        connect_clicked => EndpointMsg::ToggleMute,
                    },
                    #[name(volume_lock_button)]
                    gtk::ToggleButton {
                        add_css_class: "flat",

                        #[watch]
                        set_active: self.endpoint.volume_locked_muted.is_locked(),
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
        }
    }

    fn init_model(
        endpoint_desc: EndpointDescriptor,
        _index: &DynamicIndex,
        sender: FactorySender<Self>,
    ) -> Self {
        let endpoint = SonusmixReducer::subscribe(sender.input_sender(), EndpointMsg::UpdateState)
            .endpoints
            .get(&endpoint_desc)
            .expect("endpoint component failed to find matching endpoint on init")
            .clone();

        let connect_endpoints = ConnectEndpoints::builder()
            .launch(endpoint.descriptor)
            .forward(sender.input_sender(), |msg| match msg {});
        Self {
            endpoint,
            enabled: true,
            connect_endpoints,
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
        group.register_for_widget(&widgets.endpoint_menu_button);

        widgets
    }

    fn update(&mut self, msg: EndpointMsg, _sender: FactorySender<Self>) {
        match msg {
            EndpointMsg::UpdateState(state) => {
                if let Some(endpoint) = state.endpoints.get(&self.endpoint.descriptor) {
                    self.endpoint = endpoint.clone();
                }
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
        }
    }
}

fn volume_to_slider(volume: f32) -> f64 {
    (volume.powf(1.0 / 3.0) * 100.0) as f64
}

fn slider_to_volume(volume: f64) -> f32 {
    (volume as f32 / 100.0).powf(3.0) as f32
}
