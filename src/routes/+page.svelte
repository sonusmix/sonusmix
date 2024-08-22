<script>
    import { pipewireStore } from "$lib/backend";
    import { invoke } from "@tauri-apps/api/core";
</script>

<h3>Clients</h3>
<ul>
    {#each $pipewireStore.clients.values() as client (client.id)}
        <li>
            {JSON.stringify(client)}
            <ul>
                {#each client.nodes.map( (id) => $pipewireStore.nodes.get(id) ) as node (node.id)}
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

<h3>Devices</h3>
<ul>
    {#each $pipewireStore.devices.values() as device (device.id)}
        <li>
            {JSON.stringify(device)}
            <ul>
                {#each device.nodes.map( (id) => $pipewireStore.nodes.get(id) ) as node (node.id)}
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
