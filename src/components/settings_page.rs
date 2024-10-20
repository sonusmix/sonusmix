use std::convert::Infallible;
use std::fmt::Debug;
use std::ops::Deref;

use relm4::binding::{Binding, BoolBinding, ConnectBinding, F64Binding, StringBinding};
use relm4::gtk::prelude::*;
use relm4::{prelude::*, view};

use crate::state::settings::{SonusmixSettings, DEFAULT_SETTINGS};
use crate::state::{SonusmixReducer, SONUSMIX_SETTINGS};

/// Generates code to update a binding on `self` iff a given property on `settings` has changed.
macro_rules! update_property {
    ( $self:ident, $settings:ident, $property:ident ) => {
        paste::paste! {
            if $self.[<$property _binding>].get() != $settings.$property {
                $self.[<$property _binding>].set($settings.$property);
            }
        }
    };
}

pub struct SettingsPage {
    lock_endpoint_connections_binding: BoolBinding,
    lock_group_node_connections_binding: BoolBinding,
    show_group_node_change_warning_binding: BoolBinding,
    volume_limit_binding: F64Binding,
    confirm_clear_dialog: gtk::AlertDialog,
}

#[derive(Debug, Clone)]
pub enum SettingsMsg {
    SettingsChanged(SonusmixSettings),
    Save {
        clear_state: bool,
        clear_settings: bool,
    },
}

#[relm4::component(pub)]
impl Component for SettingsPage {
    type CommandOutput = Infallible;
    type Init = ();
    type Input = SettingsMsg;
    type Output = Infallible;

    view! {
        gtk::ScrolledWindow {
            set_policy: (gtk::PolicyType::Never, gtk::PolicyType::Automatic),

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_halign: gtk::Align::Center,
                set_spacing: 24,
                set_margin_all: 16,

                #[template]
                ConfigSection("General") {
                    #[template_child]
                    contents {
                        #[template]
                        ConfigRow<gtk::Switch, BoolBinding> ((
                            "Lock new connections between sources and sinks",
                            model.lock_endpoint_connections_binding.clone(),
                            DEFAULT_SETTINGS.lock_endpoint_connections,
                        )),
                        #[template]
                        ConfigRow<gtk::Switch, BoolBinding> ((
                            "Lock new connections to and from group nodes",
                            model.lock_group_node_connections_binding.clone(),
                            DEFAULT_SETTINGS.lock_group_node_connections,
                        )),
                        #[template]
                        ConfigRow<gtk::Switch, BoolBinding> ((
                            "Show the warning that connections will be broken when changing properties of a \
                                group node",
                            model.show_group_node_change_warning_binding.clone(),
                            DEFAULT_SETTINGS.show_group_node_change_warning,
                        )),
                        #[template]
                        ConfigRow<gtk::SpinButton, F64Binding> ((
                            "Volume limit of the volume sliders (%)",
                            model.volume_limit_binding.clone(),
                            DEFAULT_SETTINGS.volume_limit,
                        )) {
                            #[template_child]
                            control {
                                set_range: (0.0, 200.0),
                                set_increments: (5.0, 5.0),
                                set_value: model.volume_limit_binding.get(),
                            }
                        },
                    }
                },

                #[template]
                ConfigSection("State") {
                    #[template_child]
                    contents {
                        gtk::Box {
                            set_orientation: gtk::Orientation::Horizontal,
                            set_halign: gtk::Align::Center,
                            set_spacing: 24,

                            gtk::Button {
                                set_label: "Save state",
                                add_css_class: "suggested-action",
                                connect_clicked => SettingsMsg::Save { clear_state: false, clear_settings: false },
                            },
                            gtk::Button {
                                set_label: "Clear both and exit",
                                add_css_class: "destructive-action",
                                connect_clicked => SettingsMsg::Save { clear_state: true, clear_settings: true },
                            },
                            gtk::Button {
                                set_label: "Clear state and exit",
                                add_css_class: "destructive-action",
                                connect_clicked => SettingsMsg::Save { clear_state: true, clear_settings: false },
                            },
                            gtk::Button {
                                set_label: "Clear settings",
                                add_css_class: "destructive-action",
                                connect_clicked => SettingsMsg::Save { clear_state: false, clear_settings: true },
                            },
                        }
                    }
                }
            }
        }
    }

    fn init(_init: (), root: Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {
        SONUSMIX_SETTINGS.subscribe(sender.input_sender(), |settings| {
            SettingsMsg::SettingsChanged(settings.clone())
        });
        let settings = { SONUSMIX_SETTINGS.read().clone() };

        let lock_endpoint_connections_binding =
            BoolBinding::new(settings.lock_endpoint_connections);
        lock_endpoint_connections_binding.connect_value_notify(|b| {
            SONUSMIX_SETTINGS.write().lock_endpoint_connections = b.get()
        });
        let lock_group_node_connections_binding =
            BoolBinding::new(settings.lock_group_node_connections);
        lock_group_node_connections_binding.connect_value_notify(|b| {
            SONUSMIX_SETTINGS.write().lock_group_node_connections = b.get()
        });
        let show_group_node_change_warning_binding =
            BoolBinding::new(settings.show_group_node_change_warning);
        show_group_node_change_warning_binding.connect_value_notify(|b| {
            SONUSMIX_SETTINGS.write().show_group_node_change_warning = b.get()
        });
        let volume_limit_binding = F64Binding::new(settings.volume_limit);
        volume_limit_binding
            .connect_value_notify(|v| SONUSMIX_SETTINGS.write().volume_limit = v.get());

        let model = SettingsPage {
            lock_endpoint_connections_binding,
            lock_group_node_connections_binding,
            show_group_node_change_warning_binding,
            volume_limit_binding,
            confirm_clear_dialog: gtk::AlertDialog::builder()
                .message("Confirm clear")
                .detail("Are you sure you want to clear state and/or settings?")
                .buttons(["Cancel", "Confirm"])
                .cancel_button(0)
                .default_button(1)
                .modal(true)
                .build(),
        };

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: SettingsMsg, _sender: ComponentSender<Self>, root: &Self::Root) {
        match msg {
            SettingsMsg::SettingsChanged(settings) => {
                update_property!(self, settings, lock_endpoint_connections);
                update_property!(self, settings, lock_group_node_connections);
                update_property!(self, settings, show_group_node_change_warning);
                update_property!(self, settings, volume_limit);
            }
            SettingsMsg::Save {
                clear_state,
                clear_settings,
            } => {
                if clear_state || clear_settings {
                    self.confirm_clear_dialog.choose(
                        root.toplevel_window().as_ref(),
                        None::<&gtk::gio::Cancellable>,
                        move |result| {
                            let button = result.expect("Failed to get alert dialog result");
                            if button == 1 {
                                SonusmixReducer::save(clear_state, clear_settings);
                                if clear_state {
                                    relm4::main_application().quit();
                                }
                            }
                        },
                    );
                } else {
                    SonusmixReducer::save(false, false);
                }
            }
        }
    }
}

#[relm4::widget_template(pub)]
impl WidgetTemplate for ConfigSection {
    type Init = &'static str;

    view! {
        #[name(contents)]
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_hexpand: true,
            set_spacing: 8,

            gtk::Label {
                set_markup: &format!( r#"<span size="xx-large" weight="bold">{}</span>"#, init),
            },
            gtk::Separator {
                set_orientation: gtk::Orientation::Horizontal,
                set_margin_vertical: 8,
            },
        }
    }
}

/// A widget template that uses bindings to handle configuration options. Any widget that
/// implements relm4::ConnectBinding is supported. It will show a label, the control widget,
/// and a reset button, which will only be visible if the property is different from the default.
#[derive(Debug)]
pub struct ConfigRow<C, B> {
    row: gtk::Box,
    pub control: C,
    _binding: B,
}

impl<C, B> WidgetTemplate for ConfigRow<C, B>
where
    C: WidgetExt + ConnectBinding + Debug + Default + AsRef<gtk::Widget>,
    B: Binding<Target = C::Target>,
    C::Target: PartialEq + Clone + 'static,
{
    type Init = (&'static str, B, C::Target);
    type Root = gtk::Box;

    fn init((label, binding, default): Self::Init) -> Self {
        let child = |is_default| if is_default { "none" } else { "reset-button" };

        let visible_child = StringBinding::new(child(binding.get() == default));
        binding.connect_notify_local(Some("value"), {
            let visible_child = visible_child.clone();
            let default = default.clone();
            move |b, _| visible_child.set(child(b.get() == default).to_owned())
        });

        view! {
            #[name(row)]
            gtk::Box {
                set_orientation: gtk::Orientation::Horizontal,

                gtk::Label {
                    set_hexpand: true,
                    set_halign: gtk::Align::Start,
                    set_margin_end: 48,
                    set_markup: label,
                },
                #[name(stack)]
                gtk::Stack {
                    #[name(reset_button)]
                    add_named[Some("reset-button")] = &gtk::Button {
                        set_icon_name: "edit-undo-symbolic",
                        connect_clicked[binding, default] => move |_| {
                            binding.set(default.clone());
                        }
                    },
                    add_named[Some("none")] = &gtk::Box,
                    set_visible_child_name: child(binding.get() == default),
                },
                #[name(control)]
                C {
                    set_margin_start: 8,
                    bind: &binding,
                },
            }
        }

        visible_child
            .bind_property("value", &stack, "visible-child-name")
            .build();

        Self {
            row,
            control,
            _binding: binding,
        }
    }
}

impl<C, B> AsRef<gtk::Box> for ConfigRow<C, B> {
    fn as_ref(&self) -> &gtk::Box {
        &self.row
    }
}

impl<C, B> Deref for ConfigRow<C, B> {
    type Target = gtk::Box;

    fn deref(&self) -> &Self::Target {
        &self.row
    }
}
