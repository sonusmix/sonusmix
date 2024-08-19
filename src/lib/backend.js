import { invoke } from "@tauri-apps/api/core";

export async function getNodes() {
    return await invoke('get_nodes');
}
