// Pure card-state derivation for the dashboard grid (tech-gui.md §2, 2.1). Kept
// free of Svelte components so the mapping is unit-testable; `Dashboard.svelte`
// renders `serverCards` and dispatches the quick actions.

import { derived } from 'svelte/store';
import type {
  ConnectionStatusDto,
  HostDto,
  LocalForwardDto,
  MetricsDto,
  ProcessDto,
  ServiceDto,
  ServiceKindDto,
  TunnelStatusDto
} from '$lib/bindings';
import type { Status } from '$lib/theme';
import type { SessionKind } from '$lib/stores/sessions';
import { hosts } from '$lib/stores/hosts';
import { statuses } from '$lib/stores/statuses';
import { metrics } from '$lib/stores/metrics';
import { services, type HostServices } from '$lib/stores/services';
import { tunnels } from '$lib/stores/tunnels';
import { displayHostname } from '$lib/stores/streamer';
import { bracketed } from './hostForm';

// Metric severity mirrors the core's `metrics::threshold_level` (Ok < 60 <= Warn <=
// 85 < Crit) — the single source of truth for server-state colour (tech-gui.md §5).
export function metricStatus(percent: number): Status {
  if (percent < 60) return 'ok';
  if (percent <= 85) return 'warn';
  return 'crit';
}

export type MetricRow = { label: string; percent: number | null; status: Status };

export type CardService = { kind: ServiceKindDto; name: string; detail: string };

/** Reachability of a `tcpPort` host; `undefined` for an SSH-monitored one. */
type Reachability = 'reachable' | 'unreachable' | 'checking';

export interface ServerCard {
  host: HostDto;
  /** Header dot: connected health, `off` when failed, else neutral. */
  overall: Status;
  /** Down/unprobed with no live metrics — the card shows an offline state. */
  offline: boolean;
  /** Set only for a reachability host, which has no metrics to show. */
  reachability?: Reachability;
  /** Why the last connection attempt failed; unset unless it did. */
  failure?: string;
  metricRows: MetricRow[];
  uptime?: string;
  osInfo?: string;
  topProcesses: ProcessDto[];
  detectedServices: CardService[];
  servicesError?: string;
  /** Set only for a host with port forwards. */
  tunnel?: CardTunnel;
}

export interface CardTunnel {
  forwards: LocalForwardDto[];
  /** Connecting, up or retrying — what a Stop button stops. */
  running: boolean;
  dot: Status;
  label: string;
  /** Why the last attempt failed, while retrying or after giving up. */
  message?: string;
  autostart: boolean;
}

/** A host's tunnel as its card shows it; `undefined` when it has no forwards. */
export function deriveTunnel(host: HostDto, status: TunnelStatusDto | undefined): CardTunnel | undefined {
  if (host.localForwards.length === 0) return undefined;
  const base = { forwards: host.localForwards, autostart: host.tunnelAutostart };
  switch (status?.kind) {
    case 'connecting':
      return { ...base, running: true, dot: 'unknown', label: 'Connecting…' };
    case 'up':
      return { ...base, running: true, dot: 'ok', label: 'Active' };
    case 'retrying':
      return { ...base, running: true, dot: 'warn', label: 'Reconnecting…', message: status.message };
    case 'failed':
      return { ...base, running: false, dot: 'off', label: 'Failed', message: status.message };
    default:
      return { ...base, running: false, dot: 'unknown', label: 'Off' };
  }
}

// Loopback and wildcard addresses reveal nothing, and read wrong disguised.
const REVEALS_NOTHING = /^(localhost|127(\.\d{1,3}){3}|::1|\*|0\.0\.0\.0|::)$/i;

/** An address as a forward shows it: masked in streamer mode like any host address. */
function shown(address: string, streamerOn: boolean): string {
  return REVEALS_NOTHING.test(address) ? address : displayHostname(address, streamerOn);
}

/** Where a forward listens: `localhost` unless the rule names an address, `*` for all. */
export function forwardListen(f: LocalForwardDto, streamerOn: boolean): string {
  const bind = f.bindAddress == null ? 'localhost' : f.bindAddress === '' ? '*' : f.bindAddress;
  return `${bracketed(shown(bind, streamerOn))}:${f.bindPort}`;
}

/** Where a forward leads, resolved on the server. */
export function forwardTarget(f: LocalForwardDto, streamerOn: boolean): string {
  return `${bracketed(shown(f.remoteHost, streamerOn))}:${f.remotePort}`;
}

const SEVERITY: Status[] = ['ok', 'warn', 'crit'];

const SERVICE_NAMES: Record<ServiceKindDto, string> = {
  docker: 'Docker',
  nginx: 'Nginx',
  postgresql: 'PostgreSQL',
  redis: 'Redis',
  nodejs: 'Node.js'
};

function metricRow(label: string, value: number | null | undefined): MetricRow {
  const percent = value ?? null;
  return { label, percent, status: percent == null ? 'unknown' : metricStatus(percent) };
}

/** The worst severity among the metrics that reported a value; `ok` when none did. */
function worstSeverity(rows: MetricRow[]): Status {
  let worst: Status = 'ok';
  for (const row of rows) {
    if (row.percent == null) continue;
    if (SEVERITY.indexOf(row.status) > SEVERITY.indexOf(worst)) worst = row.status;
  }
  return worst;
}

function metricValue(service: ServiceDto, name: string): number | undefined {
  return service.metrics.find((m) => m.name === name)?.value;
}

// The discovery quick-scan only carries Docker container counts (see the core's
// `docker::quick_metrics`); the other kinds arrive with no quick metrics, so their
// chip shows just the service name. Empty detail => name only (tech-gui.md §4.1).
function serviceDetail(service: ServiceDto): string {
  if (service.kind !== 'docker') return '';
  const total = metricValue(service, 'containers_total');
  if (total == null) return '';
  if (total === 0) return 'no containers';
  const running = metricValue(service, 'containers_running') ?? 0;
  return `${running}/${total} running`;
}

/** Build a card's view state from a host and its live status/metrics/services. */
export function deriveCard(
  host: HostDto,
  status: ConnectionStatusDto | undefined,
  m: MetricsDto | undefined,
  svc: HostServices | undefined,
  tunnel?: TunnelStatusDto
): ServerCard {
  // A reachability host is probed by a TCP connect and never reports metrics, so
  // the tiles would be a fiction — the card shows the probe result instead.
  if (host.monitoring !== 'ssh') {
    const kind = status?.kind;
    return {
      host,
      overall: kind === 'connected' ? 'ok' : kind === 'failed' ? 'off' : 'unknown',
      offline: kind === 'failed',
      reachability: kind === 'connected' ? 'reachable' : kind === 'failed' ? 'unreachable' : 'checking',
      failure: failureOf(status),
      metricRows: [],
      topProcesses: [],
      detectedServices: [],
      tunnel: deriveTunnel(host, tunnel)
    };
  }

  const metricRows: MetricRow[] = [
    metricRow('CPU', m?.cpuPercent),
    metricRow('RAM', m?.ramPercent),
    metricRow('Disk', m?.diskPercent)
  ];
  const kind = status?.kind;
  const connected = kind === 'connected';
  const overall: Status = connected ? worstSeverity(metricRows) : kind === 'failed' ? 'off' : 'unknown';
  // Mirrors the TUI's `is_offline` (crates/omnyssh/src/ui/card.rs): down/unprobed with
  // no metrics sample at all reads as offline; any sample — even os-info-only from
  // discovery — still renders. Connecting stays live.
  const offline = (kind === undefined || kind === 'unknown' || kind === 'failed') && m === undefined;

  const detectedServices: CardService[] =
    svc?.kind === 'detected'
      ? svc.services.map((s) => ({ kind: s.kind, name: SERVICE_NAMES[s.kind], detail: serviceDetail(s) }))
      : [];

  return {
    host,
    overall,
    offline,
    failure: failureOf(status),
    metricRows,
    uptime: m?.uptime ?? undefined,
    osInfo: m?.osInfo ?? undefined,
    topProcesses: m?.topProcesses ?? [],
    detectedServices,
    servicesError: svc?.kind === 'failed' ? svc.message : undefined,
    tunnel: deriveTunnel(host, tunnel)
  };
}

function failureOf(status: ConnectionStatusDto | undefined): string | undefined {
  return status?.kind === 'failed' ? status.message : undefined;
}

/** Live dashboard cards, one per host, recomputed as any live store changes. */
export const serverCards = derived(
  [hosts, statuses, metrics, services, tunnels],
  ([$hosts, $statuses, $metrics, $services, $tunnels]) =>
    $hosts.map((host) =>
      deriveCard(
        host,
        $statuses.get(host.name),
        $metrics.get(host.name),
        $services.get(host.name),
        $tunnels.get(host.name)
      )
    )
);

// Case-insensitive substring filter over a card's name / hostname / tags / notes,
// mirroring the TUI host search (crates/omnyssh/src/app/host.rs `filter_hosts`). An
// empty query keeps every card.
export function filterHosts(cards: ServerCard[], query: string): ServerCard[] {
  const q = query.trim().toLowerCase();
  if (!q) return cards;
  return cards.filter(
    ({ host }) =>
      host.name.toLowerCase().includes(q) ||
      host.hostname.toLowerCase().includes(q) ||
      host.tags.some((t) => t.toLowerCase().includes(q)) ||
      (host.notes?.toLowerCase().includes(q) ?? false)
  );
}

// Tag filter, mirroring the TUI (`HostListView::rebuild_filter`): a card is kept when it
// carries any selected tag. An empty selection keeps every card.
export function filterByTags(cards: ServerCard[], selected: ReadonlySet<string>): ServerCard[] {
  if (selected.size === 0) return cards;
  return cards.filter(({ host }) => host.tags.some((t) => selected.has(t)));
}

const byTagName = (a: string, b: string): number => {
  const la = a.toLowerCase();
  const lb = b.toLowerCase();
  if (la !== lb) return la < lb ? -1 : 1;
  return a < b ? -1 : a > b ? 1 : 0;
};

/** Every distinct tag across the cards, sorted case-insensitively. */
export function allTags(cards: ServerCard[]): string[] {
  return [...new Set(cards.flatMap((c) => c.host.tags))].sort(byTagName);
}

/** One dashboard section; `tag` is `null` for the "Untagged" section. */
export type CardGroup = { tag: string | null; cards: ServerCard[] };

// Group-by-tag, mirroring the TUI: one section per tag (a card with several tags shows
// in each), sorted case-insensitively, then "Untagged" last. With a tag filter active
// only the selected tags get a section. Input order is kept inside a section.
export function groupByTag(cards: ServerCard[], selected: ReadonlySet<string> = new Set()): CardGroup[] {
  const tags = allTags(cards).filter((t) => selected.size === 0 || selected.has(t));
  const groups: CardGroup[] = tags.map((tag) => ({
    tag,
    cards: cards.filter((c) => c.host.tags.includes(tag))
  }));
  const untagged = cards.filter((c) => c.host.tags.length === 0);
  if (untagged.length) groups.push({ tag: null, cards: untagged });
  return groups;
}

// Host-first quick actions (tech-gui.md §2): `sh` opens a terminal, `files` opens
// SFTP — both through the shared spawn path. The kind is a valid icon name too.
export type QuickAction = { id: 'sh' | 'files'; label: string; kind: SessionKind };

export const QUICK_ACTIONS: readonly QuickAction[] = [
  { id: 'sh', label: 'sh', kind: 'terminal' },
  { id: 'files', label: 'files', kind: 'sftp' }
];
