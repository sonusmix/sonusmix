use log::debug;
use pipewire::spa::ReadableDict;
use pipewire::types::ObjectType;
use pipewire::{channel as pw_channel, properties, Context, MainLoop};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::str::FromStr;
use std::sync::{mpsc as std_channel, Arc};
use std::thread;
use tokio::sync::{mpsc as tk_channel, RwLock};

use crate::device::VirtualDevice;
use crate::events::{ControllerEvent, PipewireEvent};
use crate::store::PipewireStore;

// TODO: Should these fields be private?
pub(crate) struct PipewireController {
    pub(crate) tx: std_channel::Sender<ControllerEvent>,
    pub(crate) rx: tk_channel::UnboundedReceiver<PipewireEvent>,
}

// TODO: This may not need to be part of a struct
impl PipewireController {
    pub(crate) fn start(store: Arc<RwLock<PipewireStore>>) -> PipewireController {
        let (return_tx, adapter_rx) = std_channel::channel();
        let (adapter_tx, controller_rx) = pw_channel::channel();
        let (controller_tx, return_rx) = tk_channel::unbounded_channel(); // TODO: Should this be bounded or unbounded?
        thread::spawn({
            let adapter_tx = adapter_tx.clone();
            move || Self::adapter_thread(adapter_tx, adapter_rx)
        });
        thread::spawn(move || {
            Self::pipewire_thread(controller_tx, adapter_tx, controller_rx, store)
        });
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
        self_tx: pw_channel::Sender<ControllerEvent>,
        rx: pw_channel::Receiver<ControllerEvent>,
        store: Arc<RwLock<PipewireStore>>,
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

        let virtual_devices: Rc<RefCell<Vec<VirtualDevice>>> = Rc::new(RefCell::new(Vec::new()));

        // Get factories
        let factory_store = Rc::new(RefCell::new(FactoryStore::new()));
        {
            let _factory_listener = registry
                .add_listener_local()
                .global({
                    let factory_store = factory_store.clone();
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
            let store = store.clone();
            let virtual_devices = virtual_devices.clone();
            move |event| match event {
                ControllerEvent::CreateVirtualDevice(kind, name) => {
                    let mut device = VirtualDevice::new_builder(kind, name);
                    device
                        .send(&core, self_tx.clone())
                        .expect("Could not create device");
                    (*virtual_devices).borrow_mut().push(device);
                }
                ControllerEvent::RefreshVirtualDevice(id) => store
                    .blocking_write()
                    .refresh_virtual_device(id, &(*virtual_devices).borrow()),
                ControllerEvent::Exit => main_loop.quit(),
            }
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

        // Add new objects to the store
        let _store_listener = registry
            .add_listener_local()
            .global(move |global| {
                let virtual_devices = virtual_devices.clone();
                debug!("adding object to store");
                // Throw away the error for now
                // TODO: Do something with this
                let _ = store
                    .blocking_write()
                    .add_object(global, &(*virtual_devices).borrow());
            })
            .register();

        main_loop.run();
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
