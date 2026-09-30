// @vitest-environment jsdom
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { get } from 'svelte/store';
import { isGroupHotkey, parseView } from './dashboardView';

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

  it('starts ungrouped with no tag filter', async () => {
    const view = await fresh();
    expect(get(view)).toEqual({ groupByTag: false, tagFilter: [] });
  });

  it('toggles tags in and out of the filter', async () => {
    const view = await fresh();
    view.toggleTag('prod');
    view.toggleTag('dev');
    expect(get(view).tagFilter).toEqual(['prod', 'dev']);
    view.toggleTag('prod');
    expect(get(view).tagFilter).toEqual(['dev']);
    view.clearTags();
    expect(get(view).tagFilter).toEqual([]);
  });

  it('mirrors changes to localStorage and initialises from it', async () => {
    const view = await fresh();
    view.toggleGroupByTag();
    view.toggleTag('prod');
    expect(JSON.parse(localStorage.getItem(LOCAL_KEY) ?? '{}')).toEqual({
      groupByTag: true,
      tagFilter: ['prod']
    });
    const reloaded = await fresh();
    expect(get(reloaded)).toEqual({ groupByTag: true, tagFilter: ['prod'] });
  });

  it('survives a corrupt mirror', async () => {
    localStorage.setItem(LOCAL_KEY, '{not json');
    const view = await fresh();
    expect(get(view)).toEqual({ groupByTag: false, tagFilter: [] });
  });

  it('writes the canonical tauri-plugin-store on a user change', async () => {
    const view = await fresh();
    view.toggleGroupByTag();
    await vi.waitFor(() => {
      expect(backend.set).toHaveBeenCalledWith('dashboardView', { groupByTag: true, tagFilter: [] });
      expect(backend.save).toHaveBeenCalled();
    });
  });

  it('hydrate applies the stored value but never clobbers a fresh user change', async () => {
    backend.get.mockResolvedValue({ groupByTag: true, tagFilter: ['db'] });
    const view = await fresh();
    await view.hydrate();
    expect(get(view)).toEqual({ groupByTag: true, tagFilter: ['db'] });

    backend.get.mockResolvedValue({ groupByTag: true, tagFilter: [] });
    const other = await fresh();
    other.toggleTag('web'); // user acts before hydrate resolves
    await other.hydrate();
    expect(get(other)).toEqual({ groupByTag: false, tagFilter: ['web'] });
  });
});

describe('parseView', () => {
  it('falls back per field and drops non-string and duplicate tags', () => {
    expect(parseView(null)).toEqual({ groupByTag: false, tagFilter: [] });
    expect(parseView({ groupByTag: 'yes', tagFilter: ['a', 1, 'a', 'b'] })).toEqual({
      groupByTag: false,
      tagFilter: ['a', 'b']
    });
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
