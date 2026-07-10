import { writable } from 'svelte/store';
import type { MetricsDto } from '$lib/bindings';

/** Latest metrics sample per host, keyed by host name (tech-gui.md §3.5). */
export const metrics = writable<Map<string, MetricsDto>>(new Map());

/**
 * Merge a metrics update into the previous sample. The core emits two shapes of
 * `MetricsUpdate` per host: the poller's full snapshot and discovery's
 * os-info-only partial (cpu/ram/disk left `None`). A plain replace would wipe
 * cpu/ram/disk on every connect — flapping the alert count — so each field
 * prefers the new value and falls back to the previous, mirroring the TUI's
 * `new.or(existing)` merge.
 */
export function mergeMetrics(prev: MetricsDto | undefined, next: MetricsDto): MetricsDto {
  if (!prev) return next;
  return {
    cpuPercent: next.cpuPercent ?? prev.cpuPercent,
    ramPercent: next.ramPercent ?? prev.ramPercent,
    diskPercent: next.diskPercent ?? prev.diskPercent,
    uptime: next.uptime ?? prev.uptime,
    loadAvg: next.loadAvg ?? prev.loadAvg,
    osInfo: next.osInfo ?? prev.osInfo,
    // An empty list means "not in this update" (the core's `None`) — keep prior.
    topProcesses: next.topProcesses.length ? next.topProcesses : prev.topProcesses,
    ageSeconds: next.ageSeconds
  };
}
