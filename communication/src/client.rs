use std::{
    os::unix::net::UnixStream,
    io::{
        self,
        prelude::*
    }
};
use serde_json::json;

use crate::*;

static STREAM_PATH: &'static str = "/tmp/com.sonusmix.socket";

/// A synchronous client for the UnixStream that handles the connection between the frontend and
/// daemon.
pub struct Client {
    stream: UnixStream
}

impl Client {
    fn new() -> Result<Client, io::Error> {
        let stream = UnixStream::connect(STREAM_PATH)?;
        Ok(Client {
            stream
        })

    }

    /// Retrieve a list of all hardware sources
    pub fn get_hardware_source_list(&mut self) -> Result<response!(Request::GetHardwareSourceList), Error> {
        self.raw_request(Request::GetHardwareSourceList)
    }
    
    /// Send a raw request to the daemon
    pub fn raw_request<R: DeserializeOwned>(&mut self, request: Request) -> Result<R, Error> {
        let text = json!(request);

        self.stream.write_all(text.to_string().as_bytes())?;

        let mut response = String::new();
        self.stream.read_to_string(&mut response)?;

        Ok(serde_json::from_str(&response)?)
    }
}

