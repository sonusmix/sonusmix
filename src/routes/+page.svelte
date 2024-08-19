<script>
    import { invoke } from "@tauri-apps/api/core";
    import { getNodes } from "../lib/backend";

    let nodes = [];

    async function refreshNodes() {
        nodes = await getNodes();
    }
</script>

<button on:click={refreshNodes}>Refresh Nodes</button>

<ul>
    {#each nodes as node}
        <li>{node}</li>
    {/each}
</ul>
