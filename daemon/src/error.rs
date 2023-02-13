use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Pipewire error: {0}")]
    Pipewire(pipewire::Error),
    #[error("FACTORY_NAME was not provided for this device")]
    MissingFactory,
    #[error("The device was already created")]
    DeviceAlreadyCreated,
    #[error("The device was not yet created")]
    DeviceNotCreated,
}
