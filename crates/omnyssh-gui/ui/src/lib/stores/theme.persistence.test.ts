// @vitest-environment jsdom
import { beforeEach, describe, expect, it, vi } from 'vitest';

// The canonical layer (tech-gui.md §5.1) needs a fake Tauri store: Vitest has no
// runtime. A fresh module per test isolates the store singleton's `interacted`.
const backend = { get: vi.fn(), set: vi.fn(), save: vi.fn() };
vi.mock('@tauri-apps/plugin-store', () => ({ load: vi.fn(async () => backend) }));

async function freshTheme() {
  vi.resetModules();
  return (await import('./theme')).theme;
}

describe('theme canonical persistence', () => {
  beforeEach(() => {
    localStorage.clear();
    document.documentElement.setAttribute('data-theme', 'dark');
    backend.get.mockReset();
    backend.set.mockReset().mockResolvedValue(undefined);
    backend.save.mockReset().mockResolvedValue(undefined);
  });

  it('writes the canonical store on a user flip', async () => {
    const theme = await freshTheme();
    theme.set('light');
    await vi.waitFor(() => {
      expect(backend.set).toHaveBeenCalledWith('theme', 'light');
      expect(backend.save).toHaveBeenCalled();
    });
  });

  it('hydrate applies the stored value and refreshes the no-FOUC mirror', async () => {
    backend.get.mockResolvedValue('light');
    const theme = await freshTheme();
    await theme.hydrate();
    expect(document.documentElement.getAttribute('data-theme')).toBe('light');
    // Mirror must be rewritten or the next boot script repaints the stale theme.
    expect(localStorage.getItem('omnyssh-theme')).toBe('light');
  });

  it('hydrate does not clobber a fresh user toggle', async () => {
    backend.get.mockResolvedValue('dark'); // stale persisted value
    const theme = await freshTheme();
    theme.set('light'); // user acts before the async hydrate resolves
    await theme.hydrate();
    expect(document.documentElement.getAttribute('data-theme')).toBe('light');
  });
});
