use crate::{error::Error, events::ControllerEvent};
use log::debug;
use once_cell::sync::OnceCell;
use pipewire::{
    keys::*,
    prelude::WritableDict,
    properties,
    proxy::{Proxy, ProxyT},
    Properties,
};
use std::{fmt, rc::Rc};

/// [MEDIA_CLASS](`pipewire::keys::MEDIA_CLASS`)
#[derive(Clone, Debug)]
pub enum VirtualDeviceKind {
    Sink,
    // TODO: Source creation doesn't seem to work yet. I think the factory name should be the same as Sink but then
    // TODO: there's other weird stuff going on too
    Source,
}

impl fmt::Display for VirtualDeviceKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                VirtualDeviceKind::Sink => String::from("Audio/Sink"),
                VirtualDeviceKind::Source => String::from("Audio/Source"),
            }
        )
    }
}

impl VirtualDeviceKind {
    /// Return the default factory for this device type
    fn factory(&self) -> Factory {
        match self {
            VirtualDeviceKind::Sink => Factory::NullAudioSink,
            VirtualDeviceKind::Source => Factory::NullAudioSource,
        }
    }
}

/// [FACTORY_NAME](`pipewire::keys::FACTORY_NAME`)
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

/// audio.position
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
            AudioPosition::Mono => String::from("[MONO]"),
            AudioPosition::Stereo => String::from("[FL FR]"),
            AudioPosition::Sourround5_1 => String::from("[FL FR FC SL SR]"),
            AudioPosition::Sourround7_1 => String::from("[FL FR FC LFE RL RR SL SR]"),
            AudioPosition::Quad => String::from("[FL FR SL SR]"),
            AudioPosition::Custom(e) => {
                format!("[{}]", e.join(" "))
            }
        }
    }
}

impl Default for AudioPosition {
    fn default() -> Self {
        AudioPosition::Stereo
    }
}

/// Create a virtual Pipewire device
// #[derive(Debug)]
pub struct VirtualDevice {
    pub props: Properties,
    device_type: VirtualDeviceKind,
    name: String,
    device: OnceCell<pipewire::node::Node>,
    listener: OnceCell<pipewire::node::NodeListener>,
    node_id: Rc<OnceCell<u32>>,
}

impl VirtualDevice {
    /// Create a new virtual device with stereo output and the default [Factory](`Factory`) using a builder.
    pub fn new_builder(device_type: VirtualDeviceKind, name: String) -> Self {
        let factory = device_type.factory();
        Self {
            props: properties! {
                *FACTORY_NAME => factory.to_string(),
                *NODE_NAME => name.clone(),
                *MEDIA_CLASS => device_type.to_string(),
                *OBJECT_LINGER => "false",
                "audio.position" => AudioPosition::default().to_prop()
            },
            device_type,
            name,
            device: OnceCell::new(),
            listener: OnceCell::new(),
            node_id: Rc::new(OnceCell::new()),
        }
    }

    /// Create a new virtual device by manually specifying the [Properties](`pipewire::Properties`) for more advanced usage cases.
    /// This will also automatically [send](VirtualDevice::send()) the device.
    /// **NOTICE:** If the props are not right there will be no warning about it.
    pub fn new_with_props(
        props: Properties,
        device_type: VirtualDeviceKind,
        core: &pipewire::Core,
        name: String,
        refresh_channel: pipewire::channel::Sender<ControllerEvent>,
    ) -> Result<Self, Error> {
        let mut device = Self {
            props,
            device_type,
            name,
            device: OnceCell::new(),
            listener: OnceCell::new(),
            node_id: Rc::new(OnceCell::new()),
        };
        match device.send(core, refresh_channel) {
            Ok(_) => Ok(device),
            Err(e) => Err(e),
        }
    }

    /// Add a property to the device
    pub fn add_prop(mut self, key: String, value: String) -> Self {
        self.props.insert(key, value);
        self
    }

    /// Prevents the device from being destroyed when this struct gets dropped
    pub fn linger(mut self) -> Self {
        self.props.insert(*OBJECT_LINGER, "true");
        self
    }

    /// Modify the channels
    pub fn audio_position(mut self, position: AudioPosition) -> Self {
        self.props.insert("audio.position", &position.to_prop());
        self
    }

    /// retrieve the device link (proxy)
    pub fn device_link(&self) -> Result<&Proxy, Error> {
        match self.device.get() {
            Some(v) => Ok(v.upcast_ref()),
            None => Err(Error::DeviceNotCreated),
        }
    }

    /// Retrieve the proxy id
    pub fn id(&self) -> Result<u32, Error> {
        self.device_link().map(|v| v.id())
    }
    
    /// Get the device's id
    pub fn node_id(&self) -> Option<u32> {
        self.node_id.get().copied()
    }

    /// Send to the [Core](pipewire::Core)
    pub fn send(
        &mut self,
        core: &pipewire::Core,
        refresh_channel: pipewire::channel::Sender<ControllerEvent>,
    ) -> Result<&pipewire::node::Node, Error> {
        match core.create_object::<pipewire::node::Node, _>("adapter", &self.props) {
            Ok(n) => {
                let n = self.device.try_insert(n).map_err(|_| Error::DeviceAlreadyCreated)?;
                
                self.listener.set(
                    n.add_listener_local()
                    .info({
                        let node_id = self.node_id.clone();
                        move |node_info| {
                            let id = node_info.id();
                            let _ = node_id.set(id);
                            refresh_channel.send(ControllerEvent::RefreshVirtualDevice(id))
                                .map_err(|_| ())
                                .expect("Pipewire controller thread hung up unexpectedly");
                            debug!("Set node_id: {}", id);
                        }
                    })
                    .register()
                )
                    .map_err(|_| ())
                    .expect("running send() twice should already have been caught at self.device.try_insert() above");
                
                Ok(n)
            },
            Err(e) => Err(Error::Pipewire(e)),
        }
    }

    /// Destroy this object from the [Core](pipewire::Core).
    /// Can be added again using [send()](VirtualDevice::send()).
    pub fn destroy(&mut self, core: &pipewire::Core) -> Result<pipewire::spa::AsyncSeq, Error> {
        let dev = self.device.take().ok_or(Error::DeviceNotCreated)?;
        Ok(core.destroy_object(dev)?)
    }
}
