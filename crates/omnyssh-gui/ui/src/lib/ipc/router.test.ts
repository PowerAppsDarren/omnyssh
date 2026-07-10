import { beforeEach, describe, expect, it } from 'vitest';
import { get } from 'svelte/store';
import type { HostDto } from '$lib/bindings';
import { hosts } from '$lib/stores/hosts';
import { lastError } from '$lib/stores/notifications';
import { applyError, applyHostsLoaded } from './router';

describe('ipc event router', () => {
  beforeEach(() => {
    hosts.set([]);
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

  it('routes an error payload into the notifications store', () => {
    applyError('boom');

    expect(get(lastError)).toBe('boom');
  });
});
