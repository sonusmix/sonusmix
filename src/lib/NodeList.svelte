<script>
    import Node from "./Node.svelte";

    /** @type {boolean} */
    export let row = false;

    const dummyData = [
        { name: "Microphone", volume: 70 },
        { name: "Line In", volume: 50 },
        { name: "Chromium", volume: 100 },
    ];
</script>

{#if row}
    <div class="flex flex-row justify-center h-full">
        {#each dummyData as node, i}
            {#if i === 0}
                <div class="bg-base-content w-0.5"></div>
            {/if}
            <Node vertical {node} />
            <div class="bg-base-content w-0.5"></div>
        {/each}
    </div>
{:else}
<div class="flex flex-col">
    {#each dummyData as node, i}
        <Node {node} />
        <div class="bg-base-content h-px"></div>
    {/each}
</div>
{/if}
