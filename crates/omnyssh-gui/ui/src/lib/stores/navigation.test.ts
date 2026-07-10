import { describe, expect, it, vi } from 'vitest';
import { get } from 'svelte/store';

// A fresh module graph per test isolates the sessions id counter and the active
// entity singleton (tech-gui.md §2). navigation, sessions and activeEntity share
// the graph, so re-importing all three keeps them consistent.
async function fresh() {
  vi.resetModules();
  const nav = await import('./navigation');
  const { sessions } = await import('./sessions');
  const { activeEntity } = await import('./activeEntity');
  return { ...nav, sessions, activeEntity };
}

describe('navigation actions', () => {
  it('spawning a session appends it and makes it active', async () => {
    const { spawnSession, sessions, activeEntity } = await fresh();
    const s = spawnSession('terminal', 'web-1');
    expect(get(sessions)).toHaveLength(1);
    expect(get(activeEntity)).toEqual({ kind: 'session', id: s.id });
  });

  it('allocates unique, monotonic ids', async () => {
    const { spawnSession } = await fresh();
    const a = spawnSession('terminal', 'a');
    const b = spawnSession('sftp', 'b');
    expect(b.id).toBeGreaterThan(a.id);
  });

  it('closing the active session removes only it and falls back to the dashboard', async () => {
    const { spawnSession, closeSession, sessions, activeEntity } = await fresh();
    const a = spawnSession('terminal', 'a');
    const b = spawnSession('sftp', 'b'); // b becomes the active session
    closeSession(b.id);
    expect(get(sessions).map((s) => s.id)).toEqual([a.id]);
    expect(get(activeEntity)).toEqual({ kind: 'dashboard' });
  });

  it('closing an inactive session leaves the active entity untouched', async () => {
    const { spawnSession, closeSession, activeEntity } = await fresh();
    const a = spawnSession('terminal', 'a');
    const b = spawnSession('sftp', 'b'); // b active
    closeSession(a.id);
    expect(get(activeEntity)).toEqual({ kind: 'session', id: b.id });
  });
});
