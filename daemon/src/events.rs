use std::future::Future;

use tokio::sync::oneshot;

/// Contains events sent to the pipewire control thread.
pub(crate) enum ControllerEvent {
    CreateSink(String),
    Exit,
}

/// Contains events returned from the pipewire control thread.
pub(crate) enum PipewireEvent {
    NewGlobal(String),
}

/// Uses a tokio oneshot channel to link two futures together, and notify each when the other has exited.
pub(crate) struct ExitSignal {
    tx: Option<oneshot::Sender<()>>,
    rx: oneshot::Receiver<()>,
}

impl ExitSignal {
    pub(crate) fn pair() -> (Self, Self) {
        let (tx1, rx1) = oneshot::channel();
        let (tx2, rx2) = oneshot::channel();
        (Self { tx: Some(tx1), rx: rx2 }, Self { tx: Some(tx2), rx: rx1 })
    }

    /// Does nothing, guaranteeing a drop.
    pub(crate) fn exit(self) { }

    pub(crate) fn wait(&mut self) -> &mut impl Future {
        &mut self.rx
    }
}

impl Drop for ExitSignal {
    fn drop(&mut self) {
        if let Some(tx) = self.tx.take() {
            let _ = tx.send(());
        }
    }
}