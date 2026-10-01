// @vitest-environment jsdom
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { get } from 'svelte/store';

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

  it('starts ungrouped with every section open', async () => {
    const view = await fresh();
    expect(get(view)).toEqual({ groupByTag: false, collapsed: [] });
  });

  it('folds and unfolds sections', async () => {
    const view = await fresh();
    view.toggleCollapsed('prod');
    view.toggleCollapsed('');
    expect(get(view).collapsed).toEqual(['prod', '']);
    view.toggleCollapsed('prod');
    expect(get(view).collapsed).toEqual(['']);
  });

  it('mirrors changes to localStorage and initialises from it', async () => {
    const view = await fresh();
    view.toggleGroupByTag();
    view.toggleCollapsed('db');
    const saved = { groupByTag: true, collapsed: ['db'] };
    expect(JSON.parse(localStorage.getItem(LOCAL_KEY) ?? '{}')).toEqual(saved);
    const reloaded = await fresh();
    expect(get(reloaded)).toEqual(saved);
  });

  it('survives a corrupt mirror', async () => {
    localStorage.setItem(LOCAL_KEY, '{not json');
    const view = await fresh();
    expect(get(view)).toEqual({ groupByTag: false, collapsed: [] });
  });

  it('writes the canonical tauri-plugin-store on a user change', async () => {
    const view = await fresh();
    view.toggleGroupByTag();
    await vi.waitFor(() => {
      expect(backend.set).toHaveBeenCalledWith('dashboardView', { groupByTag: true, collapsed: [] });
      expect(backend.save).toHaveBeenCalled();
    });
  });

  it('hydrate applies the stored value and refreshes the mirror', async () => {
    const saved = { groupByTag: true, collapsed: ['web'] };
    backend.get.mockResolvedValue(saved);
    const view = await fresh();
    await view.hydrate();
    expect(get(view)).toEqual(saved);
    expect(JSON.parse(localStorage.getItem(LOCAL_KEY) ?? '{}')).toEqual(saved);
  });

  it('hydrate does not clobber a fresh user change', async () => {
    backend.get.mockResolvedValue({ groupByTag: false, collapsed: ['web'] });
    const view = await fresh();
    view.toggleGroupByTag(); // user acts before hydrate resolves
    await view.hydrate();
    expect(get(view)).toEqual({ groupByTag: true, collapsed: [] });
  });

  it('hydrate falls back per field for invalid values', async () => {
    backend.get.mockResolvedValue({ groupByTag: 'yes', collapsed: ['db', 1, 'db', null] });
    const view = await fresh();
    await view.hydrate();
    expect(get(view)).toEqual({ groupByTag: false, collapsed: ['db'] });
  });
});
