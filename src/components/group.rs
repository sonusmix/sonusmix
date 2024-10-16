use std::convert::Infallible;

use pipewire::sys::PW_VERSION_CORE;
use relm4::{factory::FactoryView, gtk::prelude::*};
use relm4::prelude::*;

use crate::state::{Endpoint as PwEndpoint, EndpointDescriptor};

pub struct Group {
    // endpoint: PwEndpoint,
}

#[relm4::factory(pub)]
impl FactoryComponent for Group {
    type Init = ();
    type Input = Infallible;
    type Output = Infallible;
    type CommandOutput = Infallible;
    type ParentWidget = gtk::Box;

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_spacing: 8,
            set_margin_all: 4,

            gtk::Label {
                set_halign: gtk::Align::Start,
                set_css_classes: &["heading"],

                set_label: "Test Group Device",
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
                },
                gtk::CenterBox {
                    set_orientation: gtk::Orientation::Vertical,

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
                        gtk::Box {
                            set_orientation: gtk::Orientation::Horizontal,
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
                    },
                    #[wrap(Some)]
                    set_end_widget = &gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_margin_top: 4,
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
                        },
                    }
                }
            },
        },
    }

    fn init_model(_init: (), _index: &DynamicIndex, _sender: FactorySender<Self>) -> Self {
        Self {}
    }

    fn init_widgets(
        &mut self,
        _index: &Self::Index,
        _root: Self::Root,
        _returned_widget: &<Self::ParentWidget as FactoryView>::ReturnedWidget,
        _sender: FactorySender<Self>,
    ) -> Self::Widgets {
        let widgets = view_output!();

        widgets
    }
}
