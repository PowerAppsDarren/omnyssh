import { writable } from 'svelte/store';
import type { HostDto } from '$lib/bindings';

/** The known hosts, kept in sync by the `hosts-loaded` event (tech-gui.md §3.5). */
export const hosts = writable<HostDto[]>([]);
