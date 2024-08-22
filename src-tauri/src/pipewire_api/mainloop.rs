use std::{cell::RefCell, rc::Rc, sync::Arc, thread::JoinHandle};

use anyhow::{Context, Result};
use log::{debug, error};
use pipewire::{
    context::Context as PwContext,
    keys::*,
    main_loop::MainLoop,
    properties::properties,
    spa::{param::ParamType, pod::PodObject},
    types::ObjectType,
};
use tokio::sync::mpsc;

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

        // Initialize Pipewire listeners
        let _listener = registry
            .add_listener_local()
            .global({
                let subscriptions = subscriptions.clone();
                let store = store.clone();
                // let sender = sender.clone();
                let registry = registry.clone();
                move |global| {
                    // debug!("New object: {global:?}");
                    let mut store_borrow = store.borrow_mut();
                    match store_borrow.add_object(&registry, global) {
                        Ok(_) => {
                            update_subscriptions(&subscriptions, &store_borrow);

                            // Add param listeners for volume
                            match global.type_ {
                                ObjectType::Node => {
                                    if let Some(node) = store_borrow.nodes.get_mut(&global.id) {
                                        node.listener = Some(
                                            node.proxy
                                                .add_listener_local()
                                                .param({
                                                    // comment to hold the formatting
                                                    let id = global.id;
                                                    move |_, type_, _, _, param| {
                                                        if let Some(param) = param {
                                                            debug!(
                                                                "id: {id}, type: {type_:?}, param: {:?}",
                                                                param.type_()
                                                            );
                                                        }
                                                    }
                                                })
                                                .register(),
                                        );
                                        node.proxy.enum_params(0, Some(ParamType::Props), 0, u32::MAX);
                                    }
                                }
                                _ => {}
                            }
                        }
                        Err(err) => error!("Error converting object: {err:?}"),
                    }
                }
            })
            .register();

        let _remove_listener = registry
            .add_listener_local()
            .global_remove({
                let subscriptions = subscriptions.clone();
                let store = store.clone();
                // let sender = sender.clone();
                // let registry = registry.clone();
                move |global| {
                    println!("Global removed: {:?}", global);
                    let mut store_borrow = store.borrow_mut();
                    store_borrow.remove_object(global);
                    update_subscriptions(&subscriptions, &store_borrow);
                }
            })
            .register();

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
