use std::{cell::{RefCell, RefMut}, ops::Deref, rc::Rc, sync::Arc, thread::JoinHandle};

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
        sys::SPA_PROP_channelVolumes,
        utils::Id,
    },
    types::ObjectType,
};
use tokio::sync::mpsc;

use crate::pipewire_api::pod::NodeProps;

use super::{store::Store, FromPipewireMessage, Subscriptions, ToPipewireMessage};

fn update_subscriptions(subscriptions: &Subscriptions, store: &Store) {
    let graph = Arc::new(store.dump_graph());

    for subscription in subscriptions
        .lock()
        .expect("subscriptions lock poisoned")
        .values()
    {
        subscription(graph.clone());
    }
}

/// # Master
///
/// The Master handles events which then get inserted into the store.
/// Therefore, the store is the slave of the master, processing what
/// it gets.
struct Master {
    subscriptions: Subscriptions,
    store: Rc<RefCell<Store>>,
    registry: Rc<Registry>
}

impl Master {
    fn new(subscriptions: Subscriptions, store: Rc<RefCell<Store>>, registry: Rc<Registry>) -> Self {
        Master {
            subscriptions,
            store,
            registry
        }
    }

    /// Listen for new events in the registry.
    /// see [registry::add_listener_local()]
    fn registry_listener(&mut self) -> pipewire::registry::Listener {
        self
            .registry
            .add_listener_local()
            .global({
                let subscriptions = self.subscriptions.clone();
                let store = self.store.clone();
                let registry = self.registry.clone();
                move |global| {
                    let mut store_borrow = store.borrow_mut();
                    match store_borrow.add_object(&registry, global) {
                        Ok(_) => {
                            update_subscriptions(&subscriptions, &store_borrow);

                            // Add param listeners for objects 
                            match global.type_ {
                                ObjectType::Node => {
                                    node_listener(&mut store_borrow, store.clone(), global.id)
                                }
                                _ => {}
                            }
                        }
                        Err(err) => error!("Error converting object: {err:?}"),
                    }
                }
            }).register()
    }

    /// Listen for remove events in the registry.
    /// See [Registry::add_listener_local()]
    fn remove_registry_listener(&mut self) -> pipewire::registry::Listener {
        self.registry
            .add_listener_local()
            .global_remove({
                let subscriptions = self.subscriptions.clone();
                let store = self.store.clone();
                move |global| {
                    println!("Global removed: {:?}", global);
                    let mut store_borrow = store.borrow_mut();
                    store_borrow.remove_object(global);
                    update_subscriptions(&subscriptions, &store_borrow);
                }
            })
            .register()
    }
}

pub fn node_listener(store_borrow: &mut RefMut<Store>, store: Rc<RefCell<Store>>, id: u32) {
    if let Some(node) = store_borrow.nodes.get_mut(&id) {
        node.listener = Some(
            node.proxy
                .add_listener_local()
                .param({
                    let id = id;
                    move |_, type_, _, _, pod| {
                        let mut store_borrow = store.borrow_mut();
                        store_borrow.change_node(type_, id, pod)
                    }
                })
                .register(),
        );
        node.proxy.enum_params(0, Some(ParamType::Props), 0, u32::MAX);
    }
}

pub(super) fn init_mainloop(
    subscriptions: Subscriptions,
) -> Result<(
    JoinHandle<()>,
    pipewire::channel::Sender<ToPipewireMessage>,
    tokio::sync::mpsc::UnboundedReceiver<FromPipewireMessage>,
)> {
    let (to_pw_tx, to_pw_rx) = pipewire::channel::channel();
    let (from_pw_tx, from_pw_rx) = mpsc::unbounded_channel();
    let (init_status_tx, init_status_rx) = oneshot::channel::<Result<()>>();

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
        let mut master = Master::new(subscriptions.clone(), store.clone(), registry);

        let _listener = master.registry_listener();
        let _remove_listener = master.remove_registry_listener();

        let _receiver = receiver.attach(mainloop.loop_(), {
            let mainloop = mainloop.clone();
            let store = store.clone();
            move |message| match message {
                ToPipewireMessage::UpdateOne(key) => {
                    if let Some(subscription) = subscriptions
                        .lock()
                        .expect("subscriptions lock poisoned")
                        .get(key)
                    {
                        subscription({ Arc::new(store.borrow().dump_graph()) });
                    }
                }
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
