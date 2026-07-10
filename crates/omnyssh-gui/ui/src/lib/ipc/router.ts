// Pure event->store routing (tech-gui.md §3.5). Kept free of the Tauri runtime
// so it is unit-testable; `subscribe.ts` wires these to the generated events.

import type {
  ConnectionStatusDto,
  HostDto,
  MetricsDto
} from '$lib/bindings';
import { hosts } from '$lib/stores/hosts';
import { statuses } from '$lib/stores/statuses';
import { metrics } from '$lib/stores/metrics';
import { lastError } from '$lib/stores/notifications';

export function applyHostsLoaded(payload: HostDto[]): void {
  hosts.set(payload);
}

export function applyHostStatusChanged(payload: {
  hostName: string;
  status: ConnectionStatusDto;
}): void {
  statuses.update((m) => new Map(m).set(payload.hostName, payload.status));
}

export function applyMetricsUpdated(payload: { hostName: string; metrics: MetricsDto }): void {
  metrics.update((m) => new Map(m).set(payload.hostName, payload.metrics));
}

export function applyError(message: string): void {
  lastError.set(message);
}
