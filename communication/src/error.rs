use std::io;

pub enum Error {
    IoError(std::io::Error),
    CouldNotParseResponse(serde_json::Error)
}

impl From<serde_json::Error> for Error {
    fn from(value: serde_json::Error) -> Self {
        Error::CouldNotParseResponse(value)
    }
}

impl From<io::Error> for Error {
    fn from(value: io::Error) -> Self {
        Error::IoError(value)
    }
}
