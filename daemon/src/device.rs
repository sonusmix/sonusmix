use crate::error::Error;
use once_cell::sync::OnceCell;
use pipewire::{
    keys::*,
    prelude::{ReadableDict, WritableDict},
    properties,
    proxy::ProxyT,
    Properties,
};
use std::collections::HashMap;
use std::fmt;

/// # Pipewire virtual devices
pub enum DeviceType {
    Sink,
    Source,
}

impl fmt::Display for DeviceType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                DeviceType::Sink => String::from("Audio/Sink"),
                DeviceType::Source => String::from("Audio/Source"),
            }
        )
    }
}

impl DeviceType {
    /// Return the default factory for this device type
    fn factory(&self) -> Factory {
        match self {
            DeviceType::Sink => Factory::NullAudioSink,
            DeviceType::Source => Factory::NullAudioSource,
        }
    }
}

/// represents Pipewire [FACTORY_NAME](`pipewire::Properties::FACTORY_NAME`)
pub enum Factory {
    /// A virtual sink
    NullAudioSink,
    /// A virtual source
    NullAudioSource,
    /// A custom Factory
    Custom(String),
}

impl fmt::Display for Factory {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Factory::NullAudioSink => String::from("support.null-audio-sink"),
                Factory::NullAudioSource => String::from("support.null-audio-source"),
                Factory::Custom(v) => v.clone(),
            }
        )
    }
}

/// Audio position setups
pub enum AudioPosition {
    /// MONO
    Mono,
    /// FL, FR
    Stereo,
    /// FL, FR, FC, SL, SR
    Sourround5_1,
    /// FL, FR, FC, LFE, RL, RR, SL, SR
    Sourround7_1,
    /// FL, FR, SL, SR
    Quad,
    /// Example: `vec!["FL", "FR"]`
    Custom(Vec<String>),
}

impl AudioPosition {
    /// Get the value as a property
    fn to_prop(&self) -> String {
        match self {
            AudioPosition::Mono => String::from("[ MONO ]"),
            AudioPosition::Stereo => String::from("[ FL FR ]"),
            AudioPosition::Sourround5_1 => String::from("[ FL FR FC SL SR ]"),
            AudioPosition::Sourround7_1 => String::from("[ FL FR FC LFE RL RR SL SR ]"),
            AudioPosition::Quad => String::from("[ FL FR SL SR ]"),
            AudioPosition::Custom(e) => {
                format!("[ {} ]", e.join(" "))
            }
        }
    }
}

impl Default for AudioPosition {
    fn default() -> Self {
        AudioPosition::Stereo
    }
}

/// Create virtual Pipewire device (wrapper for [Core::create_object](`Pipewire::Core::create_object()``))
pub struct VirtualDevice<P: ProxyT> {
    pub props: Properties,
    device_type: DeviceType,
    name: String,
    device: OnceCell<P>,
}

impl<P: ProxyT> VirtualDevice<P> {
    /// Create a new virtual device with stereo output and the default [Factory](`Factory`) using a builder.
    pub fn new_builder(device_type: DeviceType, name: String) -> Self {
        let factory = device_type.factory();
        Self {
            props: properties! {
                *FACTORY_NAME => factory.to_string(),
                *NODE_NAME => name.clone(),
                "audio.position" => AudioPosition::default().to_prop()
            },
            device_type,
            name,
            device: OnceCell::new(),
        }
    }

    /// Create a new virtual device by manually specifying the [Properties](`pipewire::Properties`) for more advanced usage cases.
    /// **NOTICE:** If the props are not right there will be no warning about it.
    pub fn new_with_props(
        props: Properties,
        device_type: DeviceType,
        core: pipewire::Core,
        name: String,
    ) -> Result<Self, Error> {
        let device = Self {
            props,
            device_type,
            name,
            device: OnceCell::new(),
        };
        match device.send(core) {
            Ok(_) => Ok(device),
            Err(e) => Err(e),
        }
    }

    /// Prevents the device from being destroyed when this struct gets dropped
    pub fn linger(&mut self) -> &Self {
        self.props.insert(*OBJECT_LINGER, "true");
        self
    }

    /// Modify the channels
    pub fn audio_position(&mut self, position: AudioPosition) {
        self.props.insert("audio.position", &position.to_prop());
    }

    /// Send in the [Core](pipewire::Core)
    pub fn send(&self, core: pipewire::Core) -> Result<&P, Error> {
        let factory = match self.props.get("FACTORY_NAME") {
            Some(f) => f,
            None => return Err(Error::MissingFactory(self.name.clone())),
        };

        match core.create_object::<P, _>(factory, &self.props) {
            Ok(n) => match self.device.try_insert(n) {
                Ok(n) => Ok(n),
                Err(_) => Err(Error::DeviceAlreadyCreated),
            },
            Err(e) => Err(Error::Pipewire(e)),
        }
    }

    pub fn destroy(&mut self, core: pipewire::Core) -> Result<pipewire::spa::AsyncSeq, Error> {
        let dev = match self.device.take() {
            Some(v) => v,
            None => return Err(Error::DeviceNotCreated),
        };
        match core.destroy_object(dev) {
            Ok(v) => Ok(v),
            Err(e) => Err(Error::Pipewire(e)),
        }
    }
}
