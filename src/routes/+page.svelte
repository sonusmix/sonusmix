<script>
    import { pipewireStore } from "$lib/backend";
    import Node from "$lib/Node.svelte";
    import NodeList from "$lib/NodeList.svelte";
    import { invoke } from "@tauri-apps/api/core";

    /** @type {number} */
    let vSplit = 0.6;
    let hSplit = 0.5;

    $: vBasis = vSplit / (1 - vSplit);
    $: hBasis = hSplit / (1 - hSplit);

    const dummyData = [
        { name: "Microphone", volume: 70 },
        { name: "Line In", volume: 50 },
        { name: "Chromium", volume: 100 },
    ];
</script>

<div class="flex flex-col min-h-screen h-screen">
    <div
        class="flex-1 flex flex-row min-h-0"
        id="v-split-top"
        style="flex-basis: calc(100% * {vSplit})"
    >
        <div
            class="flex-1 overflow-y-scroll"
            style="flex-basis: calc(100% * {hSplit})"
        >
            <NodeList />
        </div>
        <div class="bg-base-content w-1"></div>
        <div
            class="flex-1 overflow-y-scroll"
            style="flex-basis: calc(100% * {1 - hSplit})"
        >
            <NodeList />
        </div>
    </div>
    <div class="flex-none bg-base-content h-1"></div>
    <div
        class="flex-1 overflow-x-scroll min-h-96"
        style="flex-basis: calc(100% * {1 - vSplit})"
    >
        <NodeList row />
    </div>
</div>

<style>
</style>
