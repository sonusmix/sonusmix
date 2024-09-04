use std::{
    cell::{RefCell, RefMut},
    ops::Deref,
    rc::Rc,
    sync::{mpsc, Arc},
    thread::JoinHandle,
};

use anyhow::{Context, Result};

use log::{debug, error};
use pipewire::{
    context::Context as PwContext,
    keys::*,
    main_loop::MainLoop,
    properties::properties,
    registry::Registry,
    spa::{
        param::ParamType,
        pod::{deserialize::PodDeserializer, PodObject},
        sys::{SPA_PARAM_Props, SPA_PROP_channelVolumes},
        utils::Id,
    },
    types::ObjectType,
};

use crate::pipewire_api::pod::NodeProps;

use super::{store::Store, FromPipewireMessage, Graph, ToPipewireMessage};

/// # Master
///
/// The Master handles events which then get inserted into the store.
/// Therefore, the store is the slave of the master, processing what
/// it gets.
struct Master {
    store: Rc<RefCell<Store>>,
    registry: Rc<Registry>,
    sender: pipewire::channel::Sender<ToPipewireMessage>,
}

impl Master {
    fn new(
        store: Rc<RefCell<Store>>,
        registry: Rc<Registry>,
        sender: pipewire::channel::Sender<ToPipewireMessage>,
    ) -> Self {
        Master {
            store,
            registry,
            sender,
        }
    }

    /// Listen for new events in the registry.
    /// see [Registry::add_listener_local()]
    fn registry_listener(&mut self) -> pipewire::registry::Listener {
        self.registry
            .add_listener_local()
            .global({
                let store = self.store.clone();
                let registry = self.registry.clone();
                let sender = self.sender.clone();
                move |global| {
                    let result = { store.borrow_mut().add_object(&registry, global) };
                    match result {
                        Ok(_) => {
                            sender.send(ToPipewireMessage::Update);

                            // Add param listeners for objects
                            match global.type_ {
                                ObjectType::Node => {
                                    node_listener(store.clone(), sender.clone(), global.id)
                                }
                                _ => {}
                            }
                        }
                        Err(err) => error!("Error converting object: {err:?}"),
                    }
                }
            })
            .register()
    }

    /// Listen for remove events in the registry.
    /// See [Registry::add_listener_local()]
    fn remove_registry_listener(&mut self) -> pipewire::registry::Listener {
        self.registry
            .add_listener_local()
            .global_remove({
                let store = self.store.clone();
                let sender = self.sender.clone();
                move |global| {
                    println!("Global removed: {:?}", global);
                    let mut store_borrow = store.borrow_mut();
                    store_borrow.remove_object(global);
                    sender.send(ToPipewireMessage::Update);
                }
            })
            .register()
    }
}

pub fn node_listener(
    store: Rc<RefCell<Store>>,
    sender: pipewire::channel::Sender<ToPipewireMessage>,
    id: u32,
) {
    if let Some(node) = store.clone().borrow_mut().nodes.get_mut(&id) {
        node.listener = Some(
            node.proxy
                .add_listener_local()
                .param({
                    move |_, type_, _, _, pod| {
                        let mut store_borrow = store.borrow_mut();
                        store_borrow.change_node(type_, id, pod);
                        sender.send(ToPipewireMessage::Update);
                    }
                })
                .register(),
        );
        node.proxy
            .enum_params(0, Some(ParamType::Props), 0, u32::MAX);
        node.proxy.subscribe_params(&[ParamType::Props]);
    }
}

pub(super) fn init_mainloop(
    update_fn: impl Fn(Graph) + Send + 'static,
) -> Result<(
    JoinHandle<()>,
    pipewire::channel::Sender<ToPipewireMessage>,
    mpsc::Receiver<FromPipewireMessage>,
)> {
    let (to_pw_tx, to_pw_rx) = pipewire::channel::channel();
    let (from_pw_tx, from_pw_rx) = mpsc::channel();
    let (init_status_tx, init_status_rx) = oneshot::channel::<Result<()>>();

    let to_pw_tx_clone = to_pw_tx.clone();
    let handle = std::thread::spawn(move || {
        let sender = from_pw_tx;
        let receiver = to_pw_rx;
        let store = Rc::new(RefCell::new(Store::new()));

        // Initialize Pipewire stuff
        let init_result = (|| {
            let mainloop = MainLoop::new(None).context("Failed to initialize Pipewire mainloop")?;
            let context =
                PwContext::new(&mainloop).context("Failed to iniaizlize Pipewire context")?;
            let pw_core = context
                // .connect(Some(properties! {
                //     *MEDIA_CATEGORY => "Manager",
                // }))
                .connect(None)
                .context("Failed to connect to Pipewire")?;
            let registry = pw_core
                .get_registry()
                .context("Failed to get Pipewire registry")?;
            Ok((mainloop, context, pw_core, registry))
        })();
        // If there was an error, report it and exit
        let (mainloop, context, pw_core, registry) = match init_result {
            Ok(result) => {
                init_status_tx.send(Ok(()));
                result
            }
            Err(err) => {
                init_status_tx.send(Err(err));
                return;
            }
        };
        let mainloop = Rc::new(mainloop);
        let registry = Rc::new(registry);

        // init registry listener
        let mut master = Master::new(store.clone(), registry, to_pw_tx_clone);

        let _listener = master.registry_listener();
        let _remove_listener = master.remove_registry_listener();

        let _receiver = receiver.attach(mainloop.loop_(), {
            let mainloop = mainloop.clone();
            let store = store.clone();
            move |message| match message {
                ToPipewireMessage::Update => update_fn(store.borrow().dump_graph()),
                ToPipewireMessage::Exit => mainloop.quit(),
            }
        });

        println!("mainloop initialization done");

        mainloop.run();
    });

    match init_status_rx.recv() {
        Ok(Ok(_)) => Ok((handle, to_pw_tx, from_pw_rx)),
        Ok(Err(init_error)) => Err(init_error),
        Err(recv_error) => Err(recv_error).context("The Pipewire thread unexpectedly exited early"),
    }
}
