use log::info;
use pipewire::spa::ReadableDict;
use pipewire::types::ObjectType;
use pipewire::{channel as pw_channel, keys::*, properties, Context, Core, MainLoop, Properties};
use std::borrow::BorrowMut;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::str::FromStr;
use std::sync::mpsc as std_channel;
use std::thread;
use tokio::sync::mpsc as tk_channel;

use crate::events::{ControllerEvent, PipewireEvent};

// TODO: Should these fields be private?
pub(crate) struct PipewireController {
    pub(crate) tx: std_channel::Sender<ControllerEvent>,
    pub(crate) rx: tk_channel::UnboundedReceiver<PipewireEvent>,
}

// TODO: This may not need to be part of a struct
impl PipewireController {
    pub(crate) fn start() -> PipewireController {
        let (return_tx, adapter_rx) = std_channel::channel();
        let (adapter_tx, controller_rx) = pw_channel::channel();
        let (controller_tx, return_rx) = tk_channel::unbounded_channel(); // TODO: Should this be bounded or unbounded?
        thread::spawn(move || Self::adapter_thread(adapter_tx, adapter_rx));
        thread::spawn(move || Self::pipewire_thread(controller_tx, controller_rx));
        Self {
            tx: return_tx,
            rx: return_rx,
        }
    }

    /// This thread takes events from a stdlib mpsc channel and puts them into a pipewire::channel, because
    /// pipewire::channel uses a synchronous mutex and thus could cause deadlocks if called from async code.
    ///
    /// It's stupid, I know. (An alternative might be to wrap the pipewire::channel Sender in an async mutex. That would
    /// prevent deadlocks between async tasks, but could still block, albeit not for long, if the pipewire controlle
    /// thread is trying to receive a message.)
    // TODO: Possibly do the above
    fn adapter_thread(
        tx: pw_channel::Sender<ControllerEvent>,
        rx: std_channel::Receiver<ControllerEvent>,
    ) {
        for message in rx.into_iter() {
            let exit = matches!(message, ControllerEvent::Exit);
            tx.send(message)
                .map_err(|_| ())
                .expect("Pipewire controller thread hung up unexpectedly");
            if exit {
                break;
            }
        }
    }

    fn pipewire_thread(
        tx: tk_channel::UnboundedSender<PipewireEvent>,
        rx: pw_channel::Receiver<ControllerEvent>,
    ) {
        let main_loop = MainLoop::new().expect("Failed to create pipewire main loop");
        let context = Context::with_properties(
            &main_loop,
            properties! {
                *pipewire::keys::MODULE_NAME => "libpipewire-module-loopback"
            },
        )
        .expect("Could not get pipewire context");

        let core = context.connect(None).expect("Could not get pipewire core");
        let registry = core
            .get_registry()
            .expect("Could not get pipewire registry");

        // Get factories
        let mut factory_store = Rc::new(RefCell::new(FactoryStore::new()));
        {
            let factory_listener = registry
                .add_listener_local()
                .global({
                    let factory_store = factory_store.clone();
                    let main_loop = main_loop.clone();
                    move |global| {
                        if global.type_ == ObjectType::Factory {
                            if let Some(props) = &global.props {
                                if let (Some(type_name), Some(name)) =
                                    (props.get("factory.type.name"), props.get("factory.name"))
                                {
                                    let mut factory_store = (*factory_store).borrow_mut();
                                    factory_store.set_from_str(type_name, name.to_string());
                                }
                            }
                        }
                    }
                })
                .register();
        }

        let _rx = rx.attach(&main_loop, {
            let main_loop = main_loop.clone();
            let core = core.clone();
            let tx = tx.clone();
            move |event| Self::handle_controller_event(event, &main_loop, &core, &tx)
        });

        let _listener = registry
            .add_listener_local()
            .global({
                let tx = tx.clone();
                move |global| {
                    tx.send(PipewireEvent::NewGlobal(format!("{:?}", global)))
                        .map_err(|_| ())
                        .expect("Pipewire event receiver hung up")
                }
            })
            .register();

        main_loop.run();
    }

    fn handle_controller_event(
        event: ControllerEvent,
        main_loop: &MainLoop,
        core: &Core,
        channel: &tk_channel::UnboundedSender<PipewireEvent>,
    ) {
        // TODO: It might be better to merge this into pipewire_thread().
        match event {
            ControllerEvent::CreateSink(s) => {
                std::mem::forget(
                    core.create_object::<pipewire::node::Node, _>(
                        "adapter",
                        &properties! {
                            *FACTORY_NAME => "support.null-audio-sink",
                            *NODE_NAME => s,
                            *MEDIA_CLASS => "Audio/Sink",
                            *OBJECT_LINGER => "false",
                            "audio.position" => "[FL FR]",
                        },
                    )
                    .expect("Could not create sink"),
                );
            }
            ControllerEvent::Exit => main_loop.quit(),
        }
    }
}

#[derive(Hash, PartialEq, Eq)]
enum PipewireType {
    Link,
    Node,
}

impl PipewireType {
    fn from_ot(ot: ObjectType) -> Option<Self> {
        match ot {
            ObjectType::Node => Some(Self::Node),
            ObjectType::Link => Some(Self::Link),
            _ => None,
        }
    }
}

impl FromStr for PipewireType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "PipeWire:Interface:Link" => Ok(Self::Link),
            "PipeWire:Interface:Node" => Ok(Self::Node),
            _ => Err(()),
        }
    }
}

struct FactoryStore(HashMap<PipewireType, String>);

impl FactoryStore {
    const NEEDED_TYPES: [PipewireType; 2] = [PipewireType::Link, PipewireType::Node];

    fn new() -> Self {
        Self(HashMap::new())
    }

    fn set(&mut self, ot: ObjectType, name: String) {
        if let Some(pt) = PipewireType::from_ot(ot) {
            self.0.insert(pt, name);
        }
    }

    fn set_from_str(&mut self, type_name: &str, name: String) {
        if let Ok(pt) = type_name.parse() {
            self.0.insert(pt, name);
        }
    }

    fn get(&self, pt: PipewireType) -> Option<&str> {
        self.0.get(&pt).map(|s| s.as_str())
    }

    fn is_complete(&self) -> bool {
        Self::NEEDED_TYPES.iter().all(|pt| self.0.contains_key(pt))
    }
}

