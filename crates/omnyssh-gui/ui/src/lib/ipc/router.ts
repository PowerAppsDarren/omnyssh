// Pure event->store routing (tech-gui.md §3.5). Kept free of the Tauri runtime
// so it is unit-testable; `subscribe.ts` wires these to the generated events.

import { get } from 'svelte/store';
import type {
  ConnectionStatusDto,
  HostDto,
  MetricsDto,
  ServiceDto,
  SnippetResult
} from '$lib/bindings';
import { hosts } from '$lib/stores/hosts';
import { statuses } from '$lib/stores/statuses';
import { metrics, mergeMetrics } from '$lib/stores/metrics';
import { services } from '$lib/stores/services';
import { snippetRun, reduceRunResult } from '$lib/stores/snippets';
import { sessions } from '$lib/stores/sessions';
import { closeSession } from '$lib/stores/navigation';
import { lastError } from '$lib/stores/notifications';

export function applyHostsLoaded(payload: HostDto[]): void {
  hosts.set(payload);
  // Drop status/metrics for hosts that are gone, so a name reused by a new host
  // never inherits the old host's stale sample.
  const names = new Set(payload.map((h) => h.name));
  const prune = <V>(m: Map<string, V>) => new Map([...m].filter(([name]) => names.has(name)));
  statuses.update(prune);
  metrics.update(prune);
  services.update(prune);
}

export function applyHostStatusChanged(payload: {
  hostName: string;
  status: ConnectionStatusDto;
}): void {
  statuses.update((m) => new Map(m).set(payload.hostName, payload.status));
}

export function applyMetricsUpdated(payload: { hostName: string; metrics: MetricsDto }): void {
  metrics.update((m) => new Map(m).set(payload.hostName, mergeMetrics(m.get(payload.hostName), payload.metrics)));
}

export function applyServicesDetected(payload: { hostName: string; services: ServiceDto[] }): void {
  services.update((m) =>
    new Map(m).set(payload.hostName, { kind: 'detected', services: payload.services })
  );
}

export function applyServicesFailed(payload: { hostName: string; message: string }): void {
  services.update((m) => new Map(m).set(payload.hostName, { kind: 'failed', message: payload.message }));
}

export function applySnippetResult(payload: SnippetResult): void {
  snippetRun.update((run) => reduceRunResult(run, payload));
}

// A terminal's remote shell exited or its connection dropped (tech-gui.md §3.4). The
// backend already tore down its session; drop the matching tab (by its backend id).
// A user-initiated close never emits this, so there is no double-teardown.
//
// An instant-fail connect can emit terminal-exited before terminalOpen resolves, so
// the tab has no termId yet: park the id and let the tab reconcile once it records
// its backend id (`terminalDidExit`), rather than stranding a dead tab open.
const exitedBeforeMapped = new Set<number>();

export function applyTerminalExited(sessionId: number): void {
  const target = get(sessions).find((s) => s.termId === sessionId);
  if (target) closeSession(target.id);
  else exitedBeforeMapped.add(sessionId);
}

/** Whether backend session `termId` already exited before its tab recorded it (the
 *  fast-fail race); consumes the pending flag. Called right after a tab sets termId. */
export function terminalDidExit(termId: number): boolean {
  return exitedBeforeMapped.delete(termId);
}

export function applyError(message: string): void {
  lastError.set(message);
}
