// Frontend-derived status-bar summary (tech-gui.md §4.1). No wire DTO crosses the
// boundary — the counts come from `hosts` + `statuses` + `metrics`.

import { derived } from 'svelte/store';
import type { ConnectionStatusDto, HostDto, MetricsDto } from '$lib/bindings';
import { hosts } from './hosts';
import { statuses } from './statuses';
import { metrics } from './metrics';

export type HostSummary = { total: number; online: number; alert: number; offline: number };

// A metric at or above this percentage is "over its threshold" — the boundary of
// `metrics::threshold_level` (Ok < 60), the single source of truth in the core.
const ALERT_PERCENT = 60;

/** A connected host is in alert when any of cpu/ram/disk is over its threshold. */
function isBreaching(m: MetricsDto | undefined): boolean {
  if (!m) return false;
  return [m.cpuPercent, m.ramPercent, m.diskPercent].some(
    (v) => v != null && v >= ALERT_PERCENT
  );
}

/**
 * Bucket every host into exactly one of online / alert / offline, so the three
 * counts partition the host list and always sum to `total` (tech-gui.md §7, 1.2):
 * - `online`  — connected and healthy.
 * - `alert`   — connected but a metric is over its threshold (§4.1).
 * - `offline` — anything not connected: failed (§4.1), plus not-yet-probed
 *   (unknown) and connecting, which are not up so they count as offline.
 */
export function deriveHostSummary(
  hostList: HostDto[],
  statusMap: Map<string, ConnectionStatusDto>,
  metricMap: Map<string, MetricsDto>
): HostSummary {
  let online = 0;
  let alert = 0;
  let offline = 0;
  for (const host of hostList) {
    if (statusMap.get(host.name)?.kind === 'connected') {
      if (isBreaching(metricMap.get(host.name))) alert++;
      else online++;
    } else {
      offline++;
    }
  }
  return { total: hostList.length, online, alert, offline };
}

/** Live status-bar summary; recomputes as hosts, statuses and metrics change. */
export const hostSummary = derived(
  [hosts, statuses, metrics],
  ([$hosts, $statuses, $metrics]) => deriveHostSummary($hosts, $statuses, $metrics)
);
