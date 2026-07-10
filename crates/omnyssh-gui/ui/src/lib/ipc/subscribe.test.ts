import { beforeEach, describe, expect, it, vi } from 'vitest';
import { get } from 'svelte/store';

type Listener = (e: { payload: unknown }) => void;

// Capture the callback each event is wired to, replacing the Tauri-backed
// bindings so the event-name -> handler mapping can be exercised off-runtime.
const { listeners } = vi.hoisted(() => ({ listeners: {} as Record<string, Listener> }));

vi.mock('$lib/bindings', () => {
  const channel = (name: string) => ({
    listen: (cb: Listener) => {
      listeners[name] = cb;
      return Promise.resolve(() => delete listeners[name]);
    }
  });
  return { events: { hostsLoaded: channel('hostsLoaded'), error: channel('error') } };
});

import { hosts } from '$lib/stores/hosts';
import { lastError } from '$lib/stores/notifications';
import { startEventBridge } from './subscribe';

describe('startEventBridge', () => {
  beforeEach(() => {
    hosts.set([]);
    lastError.set(null);
  });

  it('routes each event to its matching store', async () => {
    await startEventBridge();

    listeners.hostsLoaded({
      payload: [
        { name: 'web-1', hostname: '10.0.0.1', user: 'root', port: 22, tags: [], source: 'manual', hasKey: false }
      ]
    });
    listeners.error({ payload: { message: 'nope' } });

    expect(get(hosts)).toHaveLength(1);
    expect(get(lastError)).toBe('nope');
  });
});
