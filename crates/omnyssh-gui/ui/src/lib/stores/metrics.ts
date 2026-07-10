import { writable } from 'svelte/store';
import type { MetricsDto } from '$lib/bindings';

/** Latest metrics sample per host, keyed by host name (tech-gui.md §3.5). */
export const metrics = writable<Map<string, MetricsDto>>(new Map());
