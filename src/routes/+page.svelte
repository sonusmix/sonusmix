<script>
    import { pipewireStore } from "$lib/backend";
    import { invoke } from "@tauri-apps/api/core";
</script>

<ul>
    {#each $pipewireStore.endpoints.values() as endpoint (endpoint.id)}
        <li>
            {JSON.stringify(endpoint)}
            <ul>
                {#each endpoint.nodes.map( (id) => $pipewireStore.nodes.get(id) ) as node (node.id)}
                    <li>
                        {JSON.stringify(node)}
                        <ul>
                            {#each node.ports.map( (id) => $pipewireStore.ports.get(id), ) as port (port.id)}
                                <li>{JSON.stringify(port)}</li>
                            {/each}
                        </ul>
                    </li>
                {/each}
            </ul>
        </li>
    {/each}
</ul>
