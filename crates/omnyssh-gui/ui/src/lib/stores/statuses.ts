import { writable } from 'svelte/store';
import type { ConnectionStatusDto } from '$lib/bindings';

/** Live connection status per host, keyed by host name (tech-gui.md §3.5). */
export const statuses = writable<Map<string, ConnectionStatusDto>>(new Map());
