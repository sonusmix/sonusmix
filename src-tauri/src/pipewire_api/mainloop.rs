use std::{
    sync::{Arc, RwLock},
    thread::JoinHandle,
};

use anyhow::{Context, Result};
use log::{debug, error};
use pipewire::{context::Context as PwContext, main_loop::MainLoop};
use tokio::sync::mpsc;

use super::{store::Store, FromPipewireMessage, ToPipewireMessage};

pub(super) fn init_mainloop(
    store: Arc<RwLock<Store>>,
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

        // Initialize Pipewire stuff
        let init_result = (|| {
            let mainloop = MainLoop::new(None).context("Failed to initialize Pipewire mainloop")?;
            let context =
                PwContext::new(&mainloop).context("Failed to iniaizlize Pipewire context")?;
            let pw_core = context
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

        // Initialize Pipewire listeners
        let _listener = registry
            .add_listener_local()
            .global({
                let store = store.clone();
                move |global| {
                    debug!("New object: {global:#?}");
                    match store
                        .write()
                        .expect("store lock poisoned")
                        .add_object(global)
                    {
                        Ok(_) => {}
                        Err(err) => error!("Error converting object: {err:?}"),
                    }
                }
            })
            .register();

        let _remove_listener = registry
            .add_listener_local()
            .global_remove(|global| println!("Global removed: {:?}", global))
            .register();

        println!("mainloop initialization done");

        mainloop.run();
    });

    match init_status_rx.recv() {
        Ok(Ok(_)) => Ok((handle, to_pw_tx, from_pw_rx)),
        Ok(Err(init_error)) => Err(init_error),
        Err(recv_error) => Err(recv_error).context("The Pipewire thread unexpectedly exited early"),
    }
}
