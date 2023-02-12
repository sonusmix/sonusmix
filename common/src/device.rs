use crate::error::Error;
use once_cell::sync::OnceCell;
use pipewire::{
    keys::*,
    prelude::{ReadableDict, WritableDict},
    properties,
    proxy::ProxyT,
    Properties,
};

/// represents Pipewire [MEDIA_CLASS](`pipewire::Properties::MEDIA_CLASS`)
pub enum VirtualDeviceType {
    Sink,
    Source,
}

impl ToString for VirtualDeviceType {
    fn to_string(&self) -> String {
        match self {
            VirtualDeviceType::Sink => String::from("Audio/Sink"),
            VirtualDeviceType::Source => String::from("Audio/Source"),
        }
    }
}

impl VirtualDeviceType {
    /// Return the default factory for this device type
    fn factory(&self) -> Factory {
        match self {
            VirtualDeviceType::Sink => Factory::NullAudioSink,
            VirtualDeviceType::Source => Factory::NullAudioSource,
        }
    }
}

/// represents Pipewire [FACTORY_NAME](`pipewire::Properties::FACTORY_NAME`)
pub enum Factory {
    /// A virtual sink
    NullAudioSink,
    /// A virtual source
    NullAudioSource,
}

impl ToString for Factory {
    fn to_string(&self) -> String {
        match self {
            Factory::NullAudioSink => String::from("support.null-audio-sink"),
            Factory::NullAudioSource => String::from("support.null-audio-source"),
        }
    }
}

/// Create virtual Pipewire device (wrapper for [Core::create_object](`Pipewire::Core::create_object()``))
pub struct VirtualDevice<P: ProxyT> {
    pub props: Properties,
    factory: Factory,
    device_type: VirtualDeviceType,
    name: String,
    device: OnceCell<P>,
}

impl<P: ProxyT> VirtualDevice<P> {
    /// Create a new virtual device with stereo output and the default [Factory](`Factory`)
    pub fn new(device_type: VirtualDeviceType, name: String) -> Self {
        let factory = device_type.factory();
        Self {
            props: properties! {
                *FACTORY_NAME => factory.to_string(),
                *NODE_NAME => name.clone(),
            },
            factory,
            device_type,
            name,
            device: OnceCell::new(),
        }
    }

    /// Create in the [Core](pipewire::Core)
    ///
    /// dev:
    /// - [Link](`pipewire::link::Link`)
    /// - [Node](`pipewire::node::Node`)
    /// - [Port](`pipewire::port::Port`)
    /// - [Metadata](`pipewire::metadata::Metadata`)
    pub fn create(&self, core: pipewire::Core) -> Result<&P, Error> {
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

    pub fn audio_position(&mut self, positions: &[String]) {
        self.props.insert(
            "audio.position",
            format!("[{}]", positions.join(" ")).as_str(),
        );
    }

    /*
    pub fn destroy(&self, core: pipewire::Core) -> Result<(), Error> {
        let dev = match self.device.get() {
            Some(v) => v,
            None => return Err(Error::DeviceNotCreated)
        }
        Ok(core.destroy_object(dev));
    }
    */
}
