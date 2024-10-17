use std::convert::Infallible;
use std::sync::Arc;

use gtk::glib::Propagation;
use log::debug;
use relm4::actions::{RelmAction, RelmActionGroup};
use relm4::prelude::*;
use relm4::{factory::FactoryView, gtk::prelude::*};

use crate::state::{
    Endpoint as PwEndpoint, EndpointDescriptor, GroupNode, GroupNodeId, SonusmixMsg,
    SonusmixReducer, SonusmixState,
};

use super::endpoint::{slider_to_volume, volume_to_slider};

pub struct Group {
    pub endpoint: PwEndpoint,
    pub group_node: GroupNode,
    renaming: bool,
    name_buffer: gtk::EntryBuffer,
}

#[derive(Debug, Clone)]
pub enum GroupMsg {
    UpdateState(Arc<SonusmixState>),
    Volume(f64),
    ToggleMute,
    ToggleLocked,
    Remove,
    StartRename,
    FinishRename(bool),
}

relm4::new_action_group!(GroupMenuActionGroup, "group-menu");
relm4::new_stateless_action!(RemoveAction, GroupMenuActionGroup, "remove");
relm4::new_stateless_action!(RenameAction, GroupMenuActionGroup, "rename");

#[relm4::factory(pub)]
impl FactoryComponent for Group {
    type Init = GroupNodeId;
    type Input = GroupMsg;
    type Output = Infallible;
    type CommandOutput = Infallible;
    type ParentWidget = gtk::Box;

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_hexpand: false,
            set_spacing: 8,
            set_margin_all: 4,

            gtk::Box {
                set_orientation: gtk::Orientation::Horizontal,
                set_spacing: 8,

                if self.renaming {
                    gtk::Entry::with_buffer(&self.name_buffer) {
                        set_width_chars: 15,
                        set_max_width_chars: 15,
                        connect_map => |entry| { entry.grab_focus(); },
                        connect_activate => GroupMsg::FinishRename(true),

                        // Add an event controller to cancel renaming on Esc
                        add_controller = gtk::EventControllerKey {
                            connect_key_pressed[sender] => move |_, key, _, _| {
                                if key == gtk::gdk::Key::Escape {
                                    sender.input(GroupMsg::FinishRename(false));
                                    Propagation::Stop
                                } else {
                                    Propagation::Proceed
                                }
                            }
                        },
                        add_controller = gtk::EventControllerFocus {
                            connect_leave => GroupMsg::FinishRename(false),
                        }
                    }
                } else {
                    gtk::Label {
                        set_halign: gtk::Align::Fill,
                        set_justify: gtk::Justification::Center,
                        set_width_chars: 15,
                        set_max_width_chars: 15,
                        set_lines: 3,
                        // set_wrap: true,
                        set_wrap_mode: gtk::pango::WrapMode::WordChar,
                        set_ellipsize: gtk::pango::EllipsizeMode::End,
                        set_css_classes: &["heading"],

                        #[watch]
                        set_label: &self.endpoint.display_name,
                        #[watch]
                        set_tooltip: &self.endpoint.display_name,

                        // Start renaming when the user double-clicks the group name
                        add_controller = gtk::GestureClick {
                            connect_released[sender] => move |_, num_presses, _, _| {
                                if num_presses >= 2 {
                                    sender.input(GroupMsg::StartRename);
                                }
                            }
                        }
                    }
                },

                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_spacing: 4,

                    #[name(group_menu_button)]
                    gtk::MenuButton {
                        set_halign: gtk::Align::End,
                        set_icon_name: "view-more-symbolic",
                        set_menu_model: Some(&group_menu)
                    },
                    gtk::Label {
                        set_label: "id: 1234",
                    }
                }
            },
            gtk::Box {
                set_orientation: gtk::Orientation::Horizontal,
                set_vexpand: true,

                gtk::Scale {
                    set_orientation: gtk::Orientation::Vertical,
                    set_inverted: true,
                    set_range: (0.0, 100.0),
                    set_increments: (1.0, 5.0),
                    set_draw_value: true,
                    set_value_pos: gtk::PositionType::Bottom,
                    set_format_value_func => move |_, value| format!("{value:.0}%"),

                    #[watch]
                    #[block_signal(volume_handler)]
                    set_value: volume_to_slider(self.endpoint.volume),
                    connect_value_changed[sender] => move |scale| {
                        sender.input(GroupMsg::Volume(scale.value()));
                    } @ volume_handler
                },
                gtk::CenterBox {
                    set_orientation: gtk::Orientation::Vertical,
                    set_hexpand: true,

                    #[wrap(Some)]
                    set_start_widget = &gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 4,

                        gtk::MenuButton {
                            #[wrap(Some)]
                            set_child = &gtk::Label {
                                set_justify: gtk::Justification::Center,
                                set_label: "Connect\nSources",
                            }
                        },
                        gtk::MenuButton {
                            #[wrap(Some)]
                            set_child = &gtk::Label {
                                set_justify: gtk::Justification::Center,
                                set_label: "Connect\nSinks",
                            }
                        },
                    },
                    #[wrap(Some)]
                    set_center_widget = &gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_halign: gtk::Align::Center,
                        set_margin_vertical: 4,
                        add_css_class: "linked",

                        #[name(mode_group)]
                        gtk::ToggleButton {
                            set_icon_name: "audio-input-microphone-symbolic",
                            set_tooltip: "Input (acts like a microphone)",
                        },
                        gtk::ToggleButton {
                            set_icon_name: "object-flip-horizontal-symbolic",
                            set_tooltip: "Duplex (acts like a microphone and headphones at the same time)",
                            set_group: Some(&mode_group),
                            set_active: true,
                        },
                        gtk::ToggleButton {
                            set_icon_name: "audio-headphones-symbolic",
                            set_tooltip: "Output (acts like headphones)",
                            set_group: Some(&mode_group),
                        }
                    },
                    #[wrap(Some)]
                    set_end_widget = &gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_halign: gtk::Align::Start,
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
                            connect_clicked => GroupMsg::ToggleMute,
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
                            connect_clicked => GroupMsg::ToggleLocked,
                        },
                    }
                }
            },
        },
    }

    menu! {
        group_menu: {
            "Remove" => RemoveAction,
            "Rename" => RenameAction,
        }
    }

    fn init_model(id: GroupNodeId, _index: &DynamicIndex, sender: FactorySender<Self>) -> Self {
        let sonusmix_state =
            SonusmixReducer::subscribe(sender.input_sender(), GroupMsg::UpdateState);
        let endpoint = sonusmix_state
            .endpoints
            .get(&EndpointDescriptor::GroupNode(id))
            .expect("group componend failed to find matching endpoint on init")
            .clone();
        let group_node = sonusmix_state
            .group_nodes
            .get(&id)
            .expect("group componend failed to find matching group node on init")
            .clone();

        let name_buffer = gtk::EntryBuffer::new(None::<&str>);

        Self {
            endpoint,
            group_node,
            renaming: false,
            name_buffer,
        }
    }

    fn init_widgets(
        &mut self,
        _index: &Self::Index,
        _root: Self::Root,
        _returned_widget: &<Self::ParentWidget as FactoryView>::ReturnedWidget,
        sender: FactorySender<Self>,
    ) -> Self::Widgets {
        let widgets = view_output!();

        let mut group = RelmActionGroup::<GroupMenuActionGroup>::new();
        let remove_action: RelmAction<RemoveAction> = RelmAction::new_stateless({
            let sender = sender.clone();
            move |_| {
                sender.input(GroupMsg::Remove);
            }
        });
        group.add_action(remove_action);
        let rename_action: RelmAction<RenameAction> = RelmAction::new_stateless({
            let sender = sender.clone();
            move |_| {
                sender.input(GroupMsg::StartRename);
            }
        });
        group.add_action(rename_action);
        group.register_for_widget(&widgets.group_menu_button);

        widgets
    }

    fn update(&mut self, msg: GroupMsg, _sender: FactorySender<Self>) {
        match msg {
            GroupMsg::UpdateState(state) => {
                if let (Some(endpoint), Some(group_node)) = (
                    state.endpoints.get(&self.endpoint.descriptor),
                    state.group_nodes.get(&self.group_node.id),
                ) {
                    self.endpoint = endpoint.clone();
                    self.group_node = group_node.clone();
                }
            }
            GroupMsg::Volume(volume) => SonusmixReducer::emit(SonusmixMsg::SetVolume(
                self.endpoint.descriptor,
                slider_to_volume(volume),
            )),
            GroupMsg::ToggleMute => {
                let mute = self
                    .endpoint
                    .volume_locked_muted
                    .is_muted()
                    .map(|mute| !mute)
                    .unwrap_or(true);
                SonusmixReducer::emit(SonusmixMsg::SetMute(self.endpoint.descriptor, mute));
            }
            GroupMsg::ToggleLocked => {
                SonusmixReducer::emit(SonusmixMsg::SetVolumeLocked(
                    self.endpoint.descriptor,
                    !self.endpoint.volume_locked_muted.is_locked(),
                ));
            }
            GroupMsg::Remove => {
                SonusmixReducer::emit(SonusmixMsg::RemoveEndpoint(self.endpoint.descriptor));
            }
            GroupMsg::StartRename => {
                self.renaming = true;
                self.name_buffer.set_text(&self.endpoint.display_name);
            }
            GroupMsg::FinishRename(confirm) => {
                self.renaming = false;
                if confirm {
                    SonusmixReducer::emit(SonusmixMsg::RenameEndpoint(
                        self.endpoint.descriptor,
                        Some(self.name_buffer.text().to_string()),
                    ));
                }
            }
        }
    }
}
