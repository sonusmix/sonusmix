use std::{cell::RefCell, rc::Rc, sync::mpsc, thread::JoinHandle};

use anyhow::{anyhow, Context, Result};

use log::{debug, error};
use pipewire::{
    context::Context as PwContext, core::Core, keys::*, main_loop::MainLoop,
    properties::properties, proxy::ProxyT, registry::Registry, spa::param::ParamType,
    types::ObjectType,
};
use ulid::Ulid;

use crate::{pipewire_api::SONUSMIX_APP_NAME, state::GroupNodeKind, SONUSMIX_APP_ID};

use super::{object::Port, store::Store, FromPipewireMessage, Graph, PortKind, ToPipewireMessage};

/// # Master
///
/// The Master handles events which then get inserted into the store.
/// Therefore, the store is the slave of the master, processing what
/// it gets.
struct Master {
    store: Rc<RefCell<Store>>,
    pw_core: Rc<Core>,
    registry: Rc<Registry>,
    sender: pipewire::channel::Sender<ToPipewireMessage>,
}

impl Master {
    fn new(
        store: Rc<RefCell<Store>>,
        pw_core: Rc<Core>,
        registry: Rc<Registry>,
        sender: pipewire::channel::Sender<ToPipewireMessage>,
    ) -> Self {
        Master {
            store,
            pw_core,
            registry,
            sender,
        }
    }

    /// Listen for info events on the core.
    /// see [Core::add_listener_local()]
    fn init_core_listeners(&mut self) -> pipewire::core::Listener {
        self.pw_core
            .add_listener_local()
            .info({
                let store = self.store.clone();
                let sender = self.sender.clone();
                move |info| {
                    debug!("info event: {info:?}");
                    store.borrow_mut().set_sonusmix_client_id(info.id());
                    let _ = sender.send(ToPipewireMessage::Update);
                }
            })
            .done(|id, seq| {
                debug!("Pipewire done event: {id}, {seq:?}");
            })
            .error(|id, seq, res, msg| {
                error!("Pipewire error event ({id}, {seq}, {res}): {msg:?}");
            })
            .register()
    }

    /// Listen for new events in the registry.
    /// see [Registry::add_listener_local()]
    fn registry_listener(&mut self) -> pipewire::registry::Listener {
        self.registry
            .add_listener_local()
            .global({
                let store = self.store.clone();
                let registry = self.registry.clone();
                let sender = self.sender.clone();
                move |global| {
                    let result = { store.borrow_mut().add_object(&registry, global) };
                    match result {
                        Ok(_) => {
                            let _ = sender.send(ToPipewireMessage::Update);

                            // Add param listeners for objects
                            match global.type_ {
                                ObjectType::Node => {
                                    init_node_listeners(store.clone(), sender.clone(), global.id);
                                }
                                ObjectType::Device => {
                                    init_device_listeners(store.clone(), global.id);
                                }
                                _ => {}
                            }
                        }
                        Err(err) => error!("Error converting object: {err:?}"),
                    }
                }
            })
            .register()
    }

    /// Listen for remove events in the registry.
    /// See [Registry::add_listener_local()]
    fn registry_remove_listener(&mut self) -> pipewire::registry::Listener {
        self.registry
            .add_listener_local()
            .global_remove({
                let store = self.store.clone();
                let sender = self.sender.clone();
                move |global| {
                    // debug!("Global removed: {:?}", global);
                    let mut store_borrow = store.borrow_mut();
                    store_borrow.remove_object(global);
                    let _ = sender.send(ToPipewireMessage::Update);
                }
            })
            .register()
    }

    /// Create a link between two ports. Checks that the ports exist, and their direction. Does
    /// nothing if a link between those two ports already exists.
    fn create_port_link(&self, start_id: u32, end_id: u32) -> Result<()> {
        let store = self.store.borrow();
        let Some(start_port) = store.ports.get(&start_id) else {
            return Err(anyhow!(
                "start_id {start_id} did not exist or was not a port"
            ));
        };
        if start_port.kind != PortKind::Source {
            return Err(anyhow!("Port {start_id} was not a source port"));
        }
        let Some(end_port) = store.ports.get(&end_id) else {
            return Err(anyhow!("end_id {end_id} did not exist or was not a port"));
        };
        if end_port.kind != PortKind::Sink {
            return Err(anyhow!("Port {end_id} was not a sink port"));
        }
        if start_port.links.iter().any(|link_id| {
            store
                .links
                .get(link_id)
                .map(|link| link.start_port == start_port.id && link.end_port == end_port.id)
                .unwrap_or(false)
        }) {
            // The link already exists
            return Ok(());
        }
        self.pw_core
            .create_object::<pipewire::link::Link>(
                "link-factory",
                &properties! {
                    *LINK_OUTPUT_NODE => start_port.node.to_string(),
                    *LINK_OUTPUT_PORT => start_port.id.to_string(),
                    *LINK_INPUT_NODE => end_port.node.to_string(),
                    *LINK_INPUT_PORT => end_port.id.to_string(),
                    *OBJECT_LINGER => "true",
                    *NODE_PASSIVE => "true",
                },
            )
            .context("Failed to create link")?;
        Ok(())
    }

    #[cfg(test)]
    fn create_node_links(&self, start_id: u32, end_id: u32) -> Result<()> {
        return Err(anyhow!("NO TESTING"));
    }

    /// Create links between all matching ports of two nodes. Checks that both ids are nodes, and
    /// skips links that do not already exist. Only connects nodes in the specified direction.
    #[cfg(not(test))]
    fn create_node_links(&self, start_id: u32, end_id: u32) -> Result<()> {
        let store = self.store.borrow();
        let Some(start_node) = store.nodes.get(&start_id) else {
            return Err(anyhow!(
                "start_id {start_id} did not exist or was not a node"
            ));
        };
        let Some(end_node) = store.nodes.get(&end_id) else {
            return Err(anyhow!("end_id {end_id} did not exist or was not a node"));
        };
        let end_ports: Vec<&Port> = end_node
            .ports
            .iter()
            .filter(|(_, kind, _)| *kind == PortKind::Sink)
            .filter_map(|(port_id, _, _)| store.ports.get(port_id))
            .collect();
        let start_ports: Vec<&Port> = start_node
            .ports
            .iter()
            .filter(|(_, kind, _)| *kind == PortKind::Source)
            .filter_map(|(port_id, _, _)| store.ports.get(port_id))
            .collect();
        let port_pairs = map_ports(start_ports, end_ports);
        if port_pairs.is_empty() {
            return Err(anyhow!(
                "No port pairs to connect between nodes {start_id} and {end_id}"
            ));
        }
        for (start_port, end_port) in port_pairs {
            self.create_port_link(start_port, end_port)?;
        }
        Ok(())
    }

    fn remove_port_link(&self, start_id: u32, end_id: u32) -> Result<()> {
        let store = self.store.borrow_mut();
        // There shouldn't be more than one link between the same two ports, but loop just in case
        // there is for some reason.
        for link_id in store.links.values().filter_map(|link| {
            (link.start_port == start_id && link.end_port == end_id).then_some(link.id)
        }) {
            self.registry.destroy_global(link_id);
        }
        Ok(())
    }

    fn remove_node_links(&self, start_id: u32, end_id: u32) -> Result<()> {
        let store = self.store.borrow_mut();
        for link_id in store.links.values().filter_map(|link| {
            (link.start_node == start_id && link.end_node == end_id).then_some(link.id)
        }) {
            self.registry.destroy_global(link_id);
        }
        Ok(())
    }

    fn create_group_node(&self, name: String, id: Ulid, kind: GroupNodeKind) -> Result<()> {
        let proxy = self
            .pw_core
            .create_object::<pipewire::node::Node>(
                "adapter",
                &properties! {
                    *FACTORY_NAME => "support.null-audio-sink",
                    *NODE_NAME => format!("sonusmix.group.{id}"),
                    *NODE_NICK => &*name,
                    *NODE_DESCRIPTION => &*name,
                    *APP_ICON_NAME => SONUSMIX_APP_ID,
                    *MEDIA_ICON_NAME => SONUSMIX_APP_ID,
                    *DEVICE_ICON_NAME => SONUSMIX_APP_ID,
                    "icon_name" => SONUSMIX_APP_ID,
                    *APP_NAME => SONUSMIX_APP_NAME,
                    *MEDIA_CLASS => match kind {
                        GroupNodeKind::Source => "Audio/Source/Virtual",
                        GroupNodeKind::Duplex => "Audio/Duplex",
                        GroupNodeKind::Sink => "Audio/Sink",
                    },
                    "audio.position" => "FL,FR",
                    "monitor.channel-volumes" => "true",
                    "monitor.passthrough" => "true",
                },
            )
            .with_context(|| format!("Failed to create group node '{name}'"))?;
        let listener = proxy
            .upcast_ref()
            .add_listener_local()
            .bound({
                let store = self.store.clone();
                move |global_id| {
                    if let Some(group_node) = store.borrow_mut().group_nodes.get_mut(&id) {
                        group_node.id = Some(global_id);
                    }
                }
            })
            .removed({
                let store = self.store.clone();
                move || {
                    store.borrow_mut().group_nodes.remove(&id);
                }
            })
            .register();
        self.store.borrow_mut().group_nodes.insert(
            id,
            super::object::GroupNode {
                id: None,
                name,
                kind,
                proxy,
                listener,
            },
        );
        Ok(())
    }

    fn remove_group_node(&self, id: Ulid) -> Result<()> {
        let mut store = self.store.borrow_mut();
        let group_node = store
            .group_nodes
            .remove(&id)
            .with_context(|| format!("Group node with id '{id}' does not exist"))?;

        // Dropping the proxy deletes the object on the server
        drop(group_node);

        Ok(())
    }
}

/// Maps two different list of ports to a list of mappings.
/// These are made at best guess but by no means are always correct.
/// Standard cases such as sorround sound, stereo and MONO ports should
/// always be correctly mapped.
///
/// | Situation | Output |
/// |-----------|--------|
/// | start = 1 | map single port to all end ports |
/// | otherwise | map by channel names |
// TODO: Maybe announce a possibly incorrect map?
// TODO: Remove MapFunctionport (currently needed for testing)
#[cfg(not(test))]
type MapFunctionPort = Port;
#[cfg(test)]
type MapFunctionPort = Port<()>;
fn map_ports(start: Vec<&MapFunctionPort>, end: Vec<&MapFunctionPort>) -> Vec<(u32, u32)> {
    if start.len() == 1 {
        return end
            .iter()
            .map(|end_port| (start[0].id, end_port.id))
            .collect();
    }
    return start
        .iter()
        .enumerate()
        .filter_map(|(index, start_port)| {
            let start_port_id: u32 = start_port.id;
            // assume the channel will be at the same index and otherwise search
            let end_port_id: Option<u32> = end
                .get(index)
                .and_then(|port| (port.channel == start_port.channel).then(|| port.id))
                .or_else(|| {
                    Some(
                        end.iter()
                            .find(|end_port| end_port.channel == start_port.channel)?
                            .id,
                    )
                });
            if end_port_id.is_none() {
                debug!("Could not find matching end port for {}", start_port_id);
            }
            Some((start_port_id, end_port_id?))
        })
        .collect();
}

pub fn init_node_listeners(
    store: Rc<RefCell<Store>>,
    sender: pipewire::channel::Sender<ToPipewireMessage>,
    id: u32,
) {
    if let Some(node) = store.clone().borrow_mut().nodes.get_mut(&id) {
        node.listener = Some(
            node.proxy
                .add_listener_local()
                .info({
                    let store = store.clone();
                    let sender = sender.clone();
                    move |info| {
                        store.borrow_mut().update_node_info(info);
                        let _ = sender.send(ToPipewireMessage::Update);
                    }
                })
                .param({
                    move |_, type_, _, _, pod| {
                        let mut store_borrow = store.borrow_mut();
                        store_borrow.update_node_param(type_, id, pod);
                        let _ = sender.send(ToPipewireMessage::Update);
                    }
                })
                .register(),
        );
        node.proxy
            .enum_params(0, Some(ParamType::Props), 0, u32::MAX);
        node.proxy.subscribe_params(&[ParamType::Props]);
    }
}

pub fn init_device_listeners(store: Rc<RefCell<Store>>, id: u32) {
    if let Some(device) = store.clone().borrow_mut().devices.get_mut(&id) {
        device.listener = Some(
            device
                .proxy
                .add_listener_local()
                // .info(...)
                .param({
                    move |_seq, type_, index, _next, pod| {
                        store
                            .borrow_mut()
                            .update_device_param(type_, id, index, pod);
                    }
                })
                .register(),
        );
        device
            .proxy
            .enum_params(0, Some(ParamType::Route), 0, u32::MAX);
        device.proxy.subscribe_params(&[ParamType::Route]);
    }
}

#[cfg(test)]
pub(super) fn init_mainloop(
    update_fn: impl Fn(Box<Graph>) + Send + 'static,
) -> Result<(
    JoinHandle<()>,
    pipewire::channel::Sender<ToPipewireMessage>,
    mpsc::Receiver<FromPipewireMessage>,
)> {
    return Err(anyhow!("NO TESTING"));
}

#[cfg(not(test))]
pub(super) fn init_mainloop(
    update_fn: impl Fn(Box<Graph>) + Send + 'static,
) -> Result<(
    JoinHandle<()>,
    pipewire::channel::Sender<ToPipewireMessage>,
    mpsc::Receiver<FromPipewireMessage>,
)> {
    let (to_pw_tx, to_pw_rx) = pipewire::channel::channel();
    let (from_pw_tx, from_pw_rx) = mpsc::channel();
    let (init_status_tx, init_status_rx) = oneshot::channel::<Result<()>>();

    let to_pw_tx_clone = to_pw_tx.clone();
    let handle = std::thread::spawn(move || {
        let _sender = from_pw_tx;
        let receiver = to_pw_rx;
        let store = Rc::new(RefCell::new(Store::new()));

        // Initialize Pipewire stuff
        let init_result = (|| {
            let mainloop = MainLoop::new(None).context("Failed to initialize Pipewire mainloop")?;
            let context =
                PwContext::new(&mainloop).context("Failed to iniaizlize Pipewire context")?;
            let pw_core = context
                .connect(Some(properties! {
                    *MEDIA_CATEGORY => "Manager",
                    *APP_ICON_NAME => SONUSMIX_APP_ID,
                }))
                .context("Failed to connect to Pipewire")?;
            let registry = pw_core
                .get_registry()
                .context("Failed to get Pipewire registry")?;
            Ok((mainloop, context, pw_core, registry))
        })();
        // If there was an error, report it and exit
        let (mainloop, _context, pw_core, registry) = match init_result {
            Ok(result) => {
                init_status_tx.send(Ok(())).expect(
                    "If the init_status receiver has been dropped something has gone very wrong",
                );
                result
            }
            Err(err) => {
                init_status_tx.send(Err(err)).expect(
                    "If the init_status receiver has been dropped something has gone very wrong",
                );
                return;
            }
        };
        let mainloop = Rc::new(mainloop);
        let pw_core = Rc::new(pw_core);
        let registry = Rc::new(registry);

        // init registry listener
        let mut master = Master::new(store.clone(), pw_core.clone(), registry, to_pw_tx_clone);

        let _listener = master.registry_listener();
        let _remove_listener = master.registry_remove_listener();
        let _core_listeners = master.init_core_listeners();

        let _receiver = receiver.attach(mainloop.loop_(), {
            let mainloop = mainloop.clone();
            let store = store.clone();
            move |message| match message {
                ToPipewireMessage::Update => update_fn(Box::new(store.borrow().dump_graph())),
                ToPipewireMessage::NodeVolume(id, volume) => {
                    if let Err(err) = store.borrow_mut().set_node_volume(id, volume) {
                        error!("Error setting volume: {err:?}");
                    }
                }
                ToPipewireMessage::NodeMute(id, mute) => {
                    if let Err(err) = store.borrow_mut().set_node_mute(id, mute) {
                        error!("Error setting mute: {err:?}");
                    }
                }
                ToPipewireMessage::CreatePortLink { start_id, end_id } => {
                    if let Err(err) = master.create_port_link(start_id, end_id) {
                        error!("Error creating port link: {err:?}");
                    };
                }
                ToPipewireMessage::CreateNodeLinks { start_id, end_id } => {
                    if let Err(err) = master.create_node_links(start_id, end_id) {
                        error!("Error creating node links: {err:?}");
                    };
                }
                ToPipewireMessage::RemovePortLink { start_id, end_id } => {
                    if let Err(err) = master.remove_port_link(start_id, end_id) {
                        error!("Error removing port link: {err:?}");
                    };
                }
                ToPipewireMessage::RemoveNodeLinks { start_id, end_id } => {
                    if let Err(err) = master.remove_node_links(start_id, end_id) {
                        error!("Error removing node links: {err:?}");
                    };
                }
                ToPipewireMessage::CreateGroupNode(name, id, kind) => {
                    if let Err(err) = master.create_group_node(name, id, kind) {
                        error!("Error creating group node: {err:?}");
                    }
                }
                ToPipewireMessage::RemoveGroupNode(name) => {
                    if let Err(err) = master.remove_group_node(name) {
                        error!("Error removing group node: {err:?}");
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

#[cfg(test)]
mod tests {
    use super::*;

    fn ch5_1() -> (Vec<Port<()>>, Vec<Port<()>>) {
        let mut start = vec![
            Port::new_test(1, 0, PortKind::Source, false),
            Port::new_test(2, 0, PortKind::Source, false),
            Port::new_test(3, 0, PortKind::Source, false),
            Port::new_test(4, 0, PortKind::Source, false),
            Port::new_test(5, 0, PortKind::Source, false),
            Port::new_test(6, 0, PortKind::Source, false),
        ];
        start[0].channel = "FL".to_string();
        start[1].channel = "FR".to_string();
        start[2].channel = "RL".to_string();
        start[3].channel = "RR".to_string();
        start[4].channel = "FC".to_string();
        start[5].channel = "LFE".to_string();

        let mut end = vec![
            Port::new_test(7, 0, PortKind::Source, false),
            Port::new_test(8, 0, PortKind::Source, false),
            Port::new_test(9, 0, PortKind::Source, false),
            Port::new_test(10, 0, PortKind::Source, false),
            Port::new_test(11, 0, PortKind::Source, false),
            Port::new_test(12, 0, PortKind::Source, false),
        ];
        end[0].channel = "FL".to_string();
        end[1].channel = "FR".to_string();
        end[2].channel = "RL".to_string();
        end[3].channel = "RR".to_string();
        end[4].channel = "FC".to_string();
        end[5].channel = "LFE".to_string();

        return (start, end);
    }

    #[test]
    fn stereo_port_mapping() {
        let mut start = vec![
            Port::new_test(1, 0, PortKind::Source, false),
            Port::new_test(2, 0, PortKind::Source, false),
        ];
        start[0].channel = "FL".to_string();
        start[1].channel = "FR".to_string();

        let mut end = vec![
            Port::new_test(3, 0, PortKind::Source, false),
            Port::new_test(4, 0, PortKind::Source, false),
        ];
        end[0].channel = "FL".to_string();
        end[1].channel = "FR".to_string();

        let start_refs = start.iter().collect();
        let end_refs = end.iter().collect();

        assert_eq!(map_ports(start_refs, end_refs), vec![(1, 3), (2, 4)])
    }

    #[test]
    fn ch5_1_port_mapping() {
        let (start, end) = ch5_1();

        let start_refs = start.iter().collect();
        let end_refs = end.iter().collect();

        assert_eq!(
            map_ports(start_refs, end_refs),
            vec![(1, 7), (2, 8), (3, 9), (4, 10), (5, 11), (6, 12)]
        )
    }

    #[test]
    fn ch5_1_with_missing_port_in_end_mapping() {
        let (start, mut end) = ch5_1();

        end.remove(0);

        let start_refs = start.iter().collect();
        let end_refs = end.iter().collect();

        assert_eq!(
            map_ports(start_refs, end_refs),
            vec![
                // (1, 7),
                (2, 8),
                (3, 9),
                (4, 10),
                (5, 11),
                (6, 12)
            ]
        )
    }

    #[test]
    fn mono_to_stereo_port_mapping() {
        let mut start = vec![Port::new_test(1, 0, PortKind::Source, false)];
        start[0].channel = "MONO".to_string();

        let mut end = vec![
            Port::new_test(2, 0, PortKind::Source, false),
            Port::new_test(3, 0, PortKind::Source, false),
        ];
        end[0].channel = "FL".to_string();
        end[1].channel = "FR".to_string();

        let start_refs = start.iter().collect();
        let end_refs = end.iter().collect();

        assert_eq!(map_ports(start_refs, end_refs), vec![(1, 2), (1, 3)])
    }
}
