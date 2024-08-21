import { Channel, invoke } from "@tauri-apps/api/core";
import { readable } from "svelte/store";
/**
 * @import { RawGraph, Graph, PipewireSubscriptionKey } from "./backendTypes.ts"
 * @import { Readable } from "svelte/store"
 */

/**
 * @template T
 * @param {T[]} objects
 * @returns {Map<number, T>}
 */
function mapFromObjects(objects) {
    let map = new Map();
    for (const object of objects) {
        map.set(object.id, object);
    }
    return map;
}

/**
 * @param {RawGraph} rawGraph
 * @returns {Graph}
 */
function buildGraph(rawGraph) {
    return {
        endpoints: mapFromObjects(rawGraph.endpoints),
        nodes: mapFromObjects(rawGraph.nodes),
        ports: mapFromObjects(rawGraph.ports),
        links: mapFromObjects(rawGraph.links),
    };
}

/** @returns {Promise<Graph>} */
export async function dumpGraph() {
    let rawGraph = await invoke("dump_graph");
    return buildGraph(rawGraph);
}

/** @type {PipewireSubscriptionKey | null} */
let pipewireSubscriptionKey = null;
/** @type {Readable<Graph>} */
export const pipewireStore = readable(
    {
        endpoints: new Map(),
        nodes: new Map(),
        ports: new Map(),
        links: new Map(),
    },
    function (set) {
        /** @type {Channel<RawGraph>} */
        const pipewireSubscription = new Channel();
        pipewireSubscription.onmessage = function (message) {
            console.log(message);
            set(buildGraph(message));
        };

        invoke("subscribe_to_pipewire", {
            channel: pipewireSubscription,
        }).then(function (key) {
            pipewireSubscriptionKey = key;
            console.log(pipewireSubscriptionKey);
        });

        return function () {
            if (pipewireSubscriptionKey !== null) {
                invoke("unsubscribe_from_pipewire", {
                    key: pipewireSubscriptionKey,
                });
                pipewireSubscriptionKey = null;
            }
        };
    },
);
