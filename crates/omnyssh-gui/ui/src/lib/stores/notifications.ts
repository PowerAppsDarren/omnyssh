import { writable } from 'svelte/store';

/** Last background error surfaced by the `error` event (tech-gui.md §3.5). */
export const lastError = writable<string | null>(null);
