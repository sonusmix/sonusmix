export type RawGraph = {
    endpoints: Endpoint[];
    nodes: Node[];
    ports: Port[];
    links: Link[];
};

export type Graph = {
    endpoints: Map<number, Endpoint>;
    nodes: Map<number, Node>;
    ports: Map<number, Port>;
    links: Map<number, Link>;
};

export type Endpoint = {
    id: number;
    name: string;
    kind: EndpointKind;
    nodes: number[];
};

export type EndpointKind = "physical" | "application" | "sonusmix";

export type Node = {
    id: number;
    name: string;
    endpoint: number;
    ports: number[];
};

export type Port = {
    id: number;
    name: string;
    node: number;
    kind: PortKind;
    links: number[];
};

export type PortKind = "source" | "sink";

export type Link = {
    id: number;
    startNode: number;
    startPort: number;
    endNode: number;
    endPort: number;
};

/** The key returned from subscribing to Pipewire updates. Should be treated as an opaque type. */
export type PipewireSubscriptionKey = unknown;
