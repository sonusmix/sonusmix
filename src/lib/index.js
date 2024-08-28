import { spring } from "svelte/motion";
import { readable } from "svelte/store";
/** @import { Spring } from "svelte/motion";} */

/** @returns {string} */
export function randomId() {
    return Math.random().toString(16).slice(2);
}

/** @returns {Spring<number>} */
export function randomVumeter() {
    const randomStore = readable(0, (set) => {
        const interval = setInterval(() => {
            set(Math.floor(Math.pow(Math.random(), 4) * 101));
        }, 100);
        return () => {
            clearInterval(interval);
        }
    });
    const springStore = spring(0);
    randomStore.subscribe((value) => {
        springStore.set(value, { soft: 1 });
    });
    return springStore;
}
