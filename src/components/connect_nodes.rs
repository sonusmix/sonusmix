use log::debug;
use relm4::prelude::*;
use relm4::{factory::FactoryVecDeque, gtk::prelude::*};

use std::convert::Infallible;
use std::sync::Arc;

use crate::{
    pipewire_api::{Graph, Node, PortKind},
    state::{subscribe_to_pipewire, SonusmixState, SONUSMIX_STATE},
};

pub struct ConnectNodes {
    graph: Arc<Graph>,
    base_node: (Node, PortKind),
    items: FactoryVecDeque<ConnectNodeItem>,
}

#[derive(Debug)]
pub enum ConnectNodesMsg {
    UpdateGraph(Arc<Graph>),
    SonusmixState(Arc<SonusmixState>),
}

#[relm4::component(pub)]
impl SimpleComponent for ConnectNodes {
    type Init = (u32, PortKind);
    type Input = ConnectNodesMsg;
    type Output = Infallible;

    view! {
        gtk::Popover {
            set_autohide: true,
            set_visible: true,

            // #[local_ref]
            // item_box -> gtk::Box {
            //     set_orientation: gtk::Orientation::Vertical,
            // }
        }
    }

    fn init(
        (id, node_kind): (u32, PortKind),
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        debug!("popover created");
        let graph = subscribe_to_pipewire(sender.input_sender(), ConnectNodesMsg::UpdateGraph);
        let sonusmix_state =
            SONUSMIX_STATE.subscribe(sender.input_sender(), ConnectNodesMsg::SonusmixState);

        let base_node = (
            graph
                .nodes
                .get(&id)
                .expect("connect nodes component failed to find matching node on init")
                .clone(),
            node_kind,
        );

        let items = FactoryVecDeque::builder()
            .launch(gtk::Box::default())
            .forward(sender.input_sender(), |msg| match msg {});

        let mut model = Self {
            graph,
            base_node,
            items,
        };
        model.update_items(sonusmix_state.as_ref());

        let item_box = model.items.widget();
        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: ConnectNodesMsg, _sender: ComponentSender<Self>) {
        match msg {
            ConnectNodesMsg::UpdateGraph(graph) => {
                self.graph = graph;
            }
            ConnectNodesMsg::SonusmixState(state) => self.update_items(state.as_ref()),
        }
    }
}

impl ConnectNodes {
    fn update_items(&mut self, sonusmix_state: &SonusmixState) {
        let candidates = match self.base_node.1 {
            PortKind::Source => sonusmix_state.active_sinks.clone(),
            PortKind::Sink => sonusmix_state.active_sources.clone(),
        };
        let mut factory = self.items.guard();
        factory.clear();
        for candidate in candidates
            .iter()
            .filter_map(|id| self.graph.nodes.get(id))
            .cloned()
        {
            factory.push_back((self.base_node.0.clone(), self.base_node.1, candidate));
        }
    }
}

struct ConnectNodeItem {
    graph: Arc<Graph>,
    base_node: (Node, PortKind),
    node: Node,
    connected: Option<bool>,
    enabled: bool,
}

#[derive(Debug)]
enum ConnectNodeItemOutput {
    // NodeChanged(u32, bool),
}

#[relm4::factory]
impl FactoryComponent for ConnectNodeItem {
    type Init = (Node, PortKind, Node);
    type Input = Arc<Graph>;
    type Output = ConnectNodeItemOutput;
    type CommandOutput = ();
    type ParentWidget = gtk::Box;

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Horizontal,
            set_spacing: 8,

            gtk::CheckButton {
                #[watch]
                set_label: Some(&self.node.name),
                #[watch]
                set_active?: self.connected,
                #[watch]
                set_inconsistent: self.connected.is_some(),

                connect_toggled[sender, id = self.node.id] => move |check| {
                    // let _ = sender.output(ConnectNodeItemOutput::NodeChanged(id, check.is_active()));
                }
            },
            gtk::Label {
                set_label: &self.node.name,
            }
        }
    }

    fn init_model(
        (base_node, base_kind, node): (Node, PortKind, Node),
        _index: &DynamicIndex,
        sender: FactorySender<Self>,
    ) -> Self {
        let graph = subscribe_to_pipewire(sender.input_sender(), |graph| graph);

        let base_node = (base_node, base_kind);

        let connected = are_nodes_connected(graph.as_ref(), &base_node, &node);

        let enabled = graph.nodes.contains_key(&base_node.0.id);

        Self {
            graph,
            base_node,
            node,
            connected,
            enabled,
        }
    }

    fn update(&mut self, graph: Arc<Graph>, _sender: FactorySender<Self>) {
        self.graph = graph;
        // If the base node is removed, in theory this component will also be removed. So,
        // momentarily having an old state shouldn't be a problem... right???
        // TODO: Maybe find a better solution for this lol
        if let Some(base_node) = self.graph.nodes.get(&self.base_node.0.id) {
            self.base_node.0 = base_node.clone();
        }
        if let Some(node) = self.graph.nodes.get(&self.node.id) {
            self.node = node.clone();
            self.enabled = true;
        } else {
            // TODO: connecting nodes should still be allowed, but we need to find a way to handle
            // temporarily missing nodes. This is for the far future, though.
            self.enabled = false;
        }
        self.connected = are_nodes_connected(self.graph.as_ref(), &self.base_node, &self.node);
    }
}

/// Returns whether the two node are connected. Some(bool) if the nodes are completely connected or
/// not, None if they are partially connected.
/// For now, "completely connected" means that every port on one of the nodes (either one) is
/// connected to a port on the other. "not connected" means there are no links from any port on one
/// node to any port on the other. "partially connected" is any other state.
fn are_nodes_connected(
    graph: &Graph,
    base_node: &(Node, PortKind),
    this_node: &Node,
) -> Option<bool> {
    let (source, sink) = match base_node.1 {
        PortKind::Source => (&base_node.0, this_node),
        PortKind::Sink => (this_node, &base_node.0),
    };
    // Find all links connected between the two nodes we're searching for
    let relevant_links = graph
        .links
        .values()
        .filter(|link| link.start_node == source.id && link.end_node == sink.id)
        .collect::<Vec<_>>();

    // If no links, nodes are not connected
    if relevant_links.is_empty() {
        return Some(false);
    }

    // If all of one node's ports are connected to by relevant links, nodes are completely connected
    if source
        .ports
        .iter()
        .all(|id| relevant_links.iter().any(|link| link.start_port == *id))
        || sink
            .ports
            .iter()
            .all(|id| relevant_links.iter().any(|link| link.end_port == *id))
    {
        return Some(true);
    }

    // Otherwise, nodes are partially connected
    None
}
