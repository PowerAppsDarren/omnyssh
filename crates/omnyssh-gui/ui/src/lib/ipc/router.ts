// Pure event->store routing (tech-gui.md §3.5). Kept free of the Tauri runtime
// so it is unit-testable; `subscribe.ts` wires these to the generated events.

import type { HostDto } from '$lib/bindings';
import { hosts } from '$lib/stores/hosts';
import { lastError } from '$lib/stores/notifications';

export function applyHostsLoaded(payload: HostDto[]): void {
  hosts.set(payload);
}

export function applyError(message: string): void {
  lastError.set(message);
}
