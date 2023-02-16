use std::num::ParseIntError;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Pipewire error: {0}")]
    Pipewire(#[from] pipewire::Error),
    #[error("FACTORY_NAME was not provided for this device")]
    MissingFactory,
    #[error("The device was already created")]
    DeviceAlreadyCreated,
    #[error("The device was not yet created")]
    DeviceNotCreated,
    #[error("Could not parse a string into an int: {0}")]
    ParseIntError(#[from] ParseIntError),
    #[error("Could not handle the passed object type")]
    InvalidObjectType(pipewire::types::ObjectType),
    #[error("The object was missing a necessary property")]
    MissingProperty(Option<&'static str>),
    #[error("The object was for a media type other than audio")]
    NotAudio(String),
    #[error("The object was not recognized as any known kind of node")]
    UnknownNode(String),
    #[error("Invalid value for prop {0}: {1}")]
    InvalidPropValue(&'static str, String),
}
