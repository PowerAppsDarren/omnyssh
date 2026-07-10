import { beforeEach, describe, expect, it } from 'vitest';
import { get } from 'svelte/store';
import type { HostDto } from '$lib/bindings';
import { hosts } from '$lib/stores/hosts';
import { statuses } from '$lib/stores/statuses';
import { metrics } from '$lib/stores/metrics';
import { lastError } from '$lib/stores/notifications';
import {
  applyError,
  applyHostStatusChanged,
  applyHostsLoaded,
  applyMetricsUpdated
} from './router';

describe('ipc event router', () => {
  beforeEach(() => {
    hosts.set([]);
    statuses.set(new Map());
    metrics.set(new Map());
    lastError.set(null);
  });

  it('routes a hosts-loaded payload into the hosts store', () => {
    const payload: HostDto[] = [
      {
        name: 'web-1',
        hostname: '10.0.0.1',
        user: 'root',
        port: 22,
        tags: [],
        source: 'manual',
        hasKey: false
      }
    ];

    applyHostsLoaded(payload);

    expect(get(hosts)).toEqual(payload);
  });

  it('routes a host-status-changed payload into the statuses store', () => {
    applyHostStatusChanged({ hostName: 'web-1', status: { kind: 'connected' } });
    applyHostStatusChanged({ hostName: 'web-2', status: { kind: 'failed', message: 'down' } });

    const map = get(statuses);
    expect(map.get('web-1')).toEqual({ kind: 'connected' });
    expect(map.get('web-2')).toEqual({ kind: 'failed', message: 'down' });
  });

  it('replaces a host status on the next change', () => {
    applyHostStatusChanged({ hostName: 'web-1', status: { kind: 'connecting' } });
    applyHostStatusChanged({ hostName: 'web-1', status: { kind: 'connected' } });

    expect(get(statuses).get('web-1')).toEqual({ kind: 'connected' });
    expect(get(statuses).size).toBe(1);
  });

  it('routes a metrics-updated payload into the metrics store', () => {
    applyMetricsUpdated({
      hostName: 'web-1',
      metrics: { cpuPercent: 12.5, topProcesses: [], ageSeconds: 0 }
    });

    expect(get(metrics).get('web-1')?.cpuPercent).toBe(12.5);
  });

  it('routes an error payload into the notifications store', () => {
    applyError('boom');

    expect(get(lastError)).toBe('boom');
  });
});
