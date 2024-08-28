<script>
    import { randomId, randomVumeter } from "$lib";

    /** @type {boolean} */
    export let vertical = false;
    export let node;

    // TODO: Replace this with id or serial from Pipewire
    const id = randomId();

    let vumeter = randomVumeter();

    /** @type {number} */
    let verticalVumeterHeight = 0;
    /** @type {number} */
    let verticalSliderHeight = 0;
</script>

{#if vertical}
<div class="flex flex-col w-40 h-full min-h-96 p-3 gap-3 rounded-box">
    <span class="text-xl">{node.name}</span>
    <div class="grow skeleton"></div>
    <div class="grow flex flex-row items-center justify-evenly">
        <div class="flex items-center justify-center w-2 h-full relative" bind:clientHeight={verticalVumeterHeight}>
            <progress class="progress progress-accent -rotate-90 absolute" style="width: {verticalVumeterHeight}px;" value={$vumeter} max={100}></progress>
        </div>
        <div class="flex flex-col items-center h-full gap-3">
            <div class="grow flex items-center justify-center w-6 relative" bind:clientHeight={verticalSliderHeight}>
            <input
                id="{id}-volume"
                type="range"
                class="range range-primary -rotate-90 absolute"
                style="width: {verticalSliderHeight}px;"
                min="0"
                max="100"
                bind:value={node.volume}
            />
            </div>
            <input type="number" class="input input-bordered input-sm font-mono w-[4.5rem]" bind:value={node.volume}/>
        </div>
    </div>
</div>
{:else}
<div class="flex flex-row w-full h-28 p-3 gap-3">
    <div class="grow flex flex-col justify-between">
        <span class="text-xl">{node.name}</span>
        <progress class="progress progress-accent" value={$vumeter} max={100}></progress>
        <div class="flex flex-row items-center gap-3">
            <input
                id="{id}-volume"
                type="range"
                class="range range-primary shrink"
                min="0"
                max="100"
                bind:value={node.volume}
            />
            <input type="number" class="input input-bordered input-sm font-mono w-[4.5rem]" bind:value={node.volume}/>
        </div>
    </div>
    <div class="grow skeleton"></div>
</div>
{/if}
