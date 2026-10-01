// @vitest-environment jsdom
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { get } from 'svelte/store';
import { isGroupHotkey } from './dashboardView';

// Same persistence shape as the sidebar collapse (./ui.test.ts): a fake Tauri store
// and a fresh module per test to reset the singleton.
const backend = { get: vi.fn(), set: vi.fn(), save: vi.fn() };
vi.mock('@tauri-apps/plugin-store', () => ({ load: vi.fn(async () => backend) }));

const LOCAL_KEY = 'omnyssh-dashboard-view';

async function fresh() {
  vi.resetModules();
  return (await import('./dashboardView')).dashboardView;
}

describe('dashboard view prefs', () => {
  beforeEach(() => {
    localStorage.clear();
    backend.get.mockReset();
    backend.set.mockReset().mockResolvedValue(undefined);
    backend.save.mockReset().mockResolvedValue(undefined);
  });

  it('starts ungrouped', async () => {
    const view = await fresh();
    expect(get(view)).toEqual({ groupByTag: false });
  });

  it('mirrors changes to localStorage and initialises from it', async () => {
    const view = await fresh();
    view.toggleGroupByTag();
    expect(JSON.parse(localStorage.getItem(LOCAL_KEY) ?? '{}')).toEqual({ groupByTag: true });
    const reloaded = await fresh();
    expect(get(reloaded)).toEqual({ groupByTag: true });
  });

  it('survives a corrupt mirror', async () => {
    localStorage.setItem(LOCAL_KEY, '{not json');
    const view = await fresh();
    expect(get(view)).toEqual({ groupByTag: false });
  });

  it('writes the canonical tauri-plugin-store on a user change', async () => {
    const view = await fresh();
    view.toggleGroupByTag();
    await vi.waitFor(() => {
      expect(backend.set).toHaveBeenCalledWith('dashboardView', { groupByTag: true });
      expect(backend.save).toHaveBeenCalled();
    });
  });

  it('hydrate applies the stored value and refreshes the mirror', async () => {
    backend.get.mockResolvedValue({ groupByTag: true });
    const view = await fresh();
    await view.hydrate();
    expect(get(view)).toEqual({ groupByTag: true });
    expect(JSON.parse(localStorage.getItem(LOCAL_KEY) ?? '{}')).toEqual({ groupByTag: true });
  });

  it('hydrate does not clobber a fresh user change', async () => {
    backend.get.mockResolvedValue({ groupByTag: false });
    const view = await fresh();
    view.toggleGroupByTag(); // user acts before hydrate resolves
    await view.hydrate();
    expect(get(view)).toEqual({ groupByTag: true });
  });

  it('hydrate falls back to defaults for invalid values', async () => {
    backend.get.mockResolvedValue({ groupByTag: 'yes' });
    const view = await fresh();
    await view.hydrate();
    expect(get(view)).toEqual({ groupByTag: false });
  });
});

describe('group hotkey (g)', () => {
  const hot = (init: KeyboardEventInit) => isGroupHotkey(new KeyboardEvent('keydown', init));

  it('matches a bare g/G and rejects modifiers, repeat and other keys', () => {
    expect(hot({ key: 'g' })).toBe(true);
    expect(hot({ key: 'G' })).toBe(true);
    expect(hot({ key: 'g', ctrlKey: true })).toBe(false);
    expect(hot({ key: 'g', repeat: true })).toBe(false);
    expect(hot({ key: 'r' })).toBe(false);
  });

  it('does not fire while typing in an editable field', () => {
    const input = document.createElement('input');
    document.body.append(input);
    let matched = true;
    input.addEventListener('keydown', (e) => (matched = isGroupHotkey(e)));
    input.dispatchEvent(new KeyboardEvent('keydown', { key: 'g', bubbles: true }));
    input.remove();
    expect(matched).toBe(false);
  });
});
