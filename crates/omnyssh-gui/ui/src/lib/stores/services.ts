import { writable } from 'svelte/store';
import type { ServiceDto } from '$lib/bindings';

// Discovery result per host (tech-gui.md §2, 2.1): a host either carries a detected
// service set or a discovery failure. The dashboard shows the services or a hint.
export type HostServices =
  | { kind: 'detected'; services: ServiceDto[] }
  | { kind: 'failed'; message: string };

/** Latest discovery result per host, keyed by host name (tech-gui.md §3.5). */
export const services = writable<Map<string, HostServices>>(new Map());
