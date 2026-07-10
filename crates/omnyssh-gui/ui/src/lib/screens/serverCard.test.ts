import { describe, expect, it, vi } from 'vitest';
import { get } from 'svelte/store';
import type { ConnectionStatusDto, HostDto, MetricsDto } from '$lib/bindings';
import type { HostServices } from '$lib/stores/services';
import { deriveCard, metricStatus, QUICK_ACTIONS } from './serverCard';

function host(name = 'web-1'): HostDto {
  return { name, hostname: '10.0.0.1', user: 'root', port: 22, tags: [], source: 'manual', hasKey: false };
}

function metrics(partial: Partial<MetricsDto>): MetricsDto {
  return { topProcesses: [], ageSeconds: 0, ...partial };
}

const CONNECTED: ConnectionStatusDto = { kind: 'connected' };

describe('metricStatus — mirrors metrics::threshold_level', () => {
  it('classifies against the 60 / 85 boundaries', () => {
    expect(metricStatus(0)).toBe('ok');
    expect(metricStatus(59.9)).toBe('ok');
    expect(metricStatus(60)).toBe('warn');
    expect(metricStatus(85)).toBe('warn');
    expect(metricStatus(85.1)).toBe('crit');
    expect(metricStatus(100)).toBe('crit');
  });
});

describe('deriveCard — health state', () => {
  it('a connected, healthy host is ok and not offline', () => {
    const card = deriveCard(host(), CONNECTED, metrics({ cpuPercent: 10, ramPercent: 20, diskPercent: 30 }), undefined);
    expect(card.overall).toBe('ok');
    expect(card.offline).toBe(false);
    expect(card.metricRows.map((r) => r.status)).toEqual(['ok', 'ok', 'ok']);
  });

  it('a connected host takes the worst metric severity as its overall state', () => {
    const warn = deriveCard(host(), CONNECTED, metrics({ cpuPercent: 10, ramPercent: 70, diskPercent: 10 }), undefined);
    expect(warn.overall).toBe('warn');
    const crit = deriveCard(host(), CONNECTED, metrics({ cpuPercent: 95, ramPercent: 70, diskPercent: 10 }), undefined);
    expect(crit.overall).toBe('crit');
  });

  it('a connected host with no metrics yet is ok and not offline', () => {
    const card = deriveCard(host(), CONNECTED, undefined, undefined);
    expect(card.overall).toBe('ok');
    expect(card.offline).toBe(false);
    expect(card.metricRows.map((r) => r.percent)).toEqual([null, null, null]);
    expect(card.metricRows.map((r) => r.status)).toEqual(['unknown', 'unknown', 'unknown']);
  });

  it('a failed host with no metrics is off and offline', () => {
    const card = deriveCard(host(), { kind: 'failed', message: 'refused' }, undefined, undefined);
    expect(card.overall).toBe('off');
    expect(card.offline).toBe(true);
  });

  it('a failed host keeps showing its last metrics rather than an offline state', () => {
    const card = deriveCard(host(), { kind: 'failed', message: 'refused' }, metrics({ cpuPercent: 40 }), undefined);
    expect(card.overall).toBe('off');
    expect(card.offline).toBe(false);
  });

  it('a connecting host is neutral and not offline', () => {
    const card = deriveCard(host(), { kind: 'connecting' }, undefined, undefined);
    expect(card.overall).toBe('unknown');
    expect(card.offline).toBe(false);
  });

  it('an unprobed host with no status is neutral and offline', () => {
    const card = deriveCard(host(), undefined, undefined, undefined);
    expect(card.overall).toBe('unknown');
    expect(card.offline).toBe(true);
  });

  it('a single reported metric drives severity while the others stay unknown', () => {
    const card = deriveCard(host(), CONNECTED, metrics({ diskPercent: 90 }), undefined);
    expect(card.overall).toBe('crit');
    expect(card.metricRows).toEqual([
      { label: 'CPU', percent: null, status: 'unknown' },
      { label: 'RAM', percent: null, status: 'unknown' },
      { label: 'Disk', percent: 90, status: 'crit' }
    ]);
  });

  it('carries uptime, os info and top processes through', () => {
    const card = deriveCard(
      host(),
      CONNECTED,
      metrics({ uptime: '3 days', osInfo: 'Ubuntu 22.04', topProcesses: [{ name: 'pg', cpuPercent: 30, memPercent: 12 }] }),
      undefined
    );
    expect(card.uptime).toBe('3 days');
    expect(card.osInfo).toBe('Ubuntu 22.04');
    expect(card.topProcesses).toHaveLength(1);
  });
});

describe('deriveCard — detected services', () => {
  it('names each kind and summarises docker container counts', () => {
    const svc: HostServices = {
      kind: 'detected',
      services: [
        { kind: 'docker', metrics: [{ name: 'containers_running', value: 4 }, { name: 'containers_stopped', value: 1 }] },
        { kind: 'postgresql', metrics: [{ name: 'replication_lag_seconds', value: 2 }] }
      ]
    };
    const card = deriveCard(host(), CONNECTED, undefined, svc);
    expect(card.detectedServices).toEqual([
      { kind: 'docker', name: 'Docker', detail: '4 running, 1 stopped' },
      { kind: 'postgresql', name: 'PostgreSQL', detail: 'lag 2s' }
    ]);
    expect(card.servicesError).toBeUndefined();
  });

  it('surfaces a discovery failure and shows no service chips', () => {
    const svc: HostServices = { kind: 'failed', message: 'scan timed out' };
    const card = deriveCard(host(), CONNECTED, undefined, svc);
    expect(card.detectedServices).toEqual([]);
    expect(card.servicesError).toBe('scan timed out');
  });
});

// --- Quick-action dispatch: the card's sh/files buttons use the shared spawn path. ---

async function freshNav() {
  vi.resetModules();
  const { QUICK_ACTIONS: actions } = await import('./serverCard');
  const { spawnSession } = await import('$lib/stores/navigation');
  const { sessions } = await import('$lib/stores/sessions');
  const { activeEntity } = await import('$lib/stores/activeEntity');
  return { actions, spawnSession, sessions, activeEntity };
}

describe('quick actions', () => {
  it('map sh to a terminal and files to an SFTP session', () => {
    expect(QUICK_ACTIONS.map((a) => [a.id, a.kind])).toEqual([
      ['sh', 'terminal'],
      ['files', 'sftp']
    ]);
  });

  it('dispatch through the shared spawn path, appending an active session of the right kind', async () => {
    const { actions, spawnSession, sessions, activeEntity } = await freshNav();
    for (const action of actions) {
      const s = spawnSession(action.kind, 'web-1');
      expect(s.kind).toBe(action.kind);
      expect(get(activeEntity)).toEqual({ kind: 'session', id: s.id });
    }
    expect(get(sessions).map((s) => s.kind)).toEqual(['terminal', 'sftp']);
  });
});
