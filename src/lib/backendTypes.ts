export type RawGraph = {
    clients: Client[];
    devices: Device[];
    nodes: Node[];
    ports: Port[];
    links: Link[];
};

export type Graph = {
    clients: Map<number, Client>;
    devices: Map<number, Device>;
    nodes: Map<number, Node>;
    ports: Map<number, Port>;
    links: Map<number, Link>;
};

export type Client = {
    id: number;
    name: string;
    isSonusmix: boolean;
    nodes: number[];
};

// export type EndpointKind = "physical" | "application" | "sonusmix";

export type Device = {
    id: number;
    name: string;
    client: number;
    nodes: number[];
}

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
