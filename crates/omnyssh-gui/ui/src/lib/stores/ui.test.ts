// @vitest-environment jsdom
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { get } from 'svelte/store';
import { isCollapseChord, isPaletteChord } from './ui';

// Collapse persistence (tech-gui.md §2, §3.5). The canonical layer needs a fake
// Tauri store — Vitest has no runtime — and a fresh module per test isolates the
// singleton's `interacted` latch and the localStorage-seeded initial value.
const backend = { get: vi.fn(), set: vi.fn(), save: vi.fn() };
vi.mock('@tauri-apps/plugin-store', () => ({ load: vi.fn(async () => backend) }));

async function fresh() {
  vi.resetModules();
  return (await import('./ui')).sidebarCollapsed;
}

describe('sidebar collapse persistence', () => {
  beforeEach(() => {
    localStorage.clear();
    backend.get.mockReset();
    backend.set.mockReset().mockResolvedValue(undefined);
    backend.save.mockReset().mockResolvedValue(undefined);
  });

  it('mirrors a toggle to localStorage so the next boot paints the right width', async () => {
    const sidebarCollapsed = await fresh();
    sidebarCollapsed.toggle();
    expect(get(sidebarCollapsed)).toBe(true);
    expect(localStorage.getItem('omnyssh-sidebar-collapsed')).toBe('true');
  });

  it('initialises from the localStorage mirror', async () => {
    localStorage.setItem('omnyssh-sidebar-collapsed', 'true');
    const sidebarCollapsed = await fresh();
    expect(get(sidebarCollapsed)).toBe(true);
  });

  it('writes the canonical tauri-plugin-store on a user flip', async () => {
    const sidebarCollapsed = await fresh();
    sidebarCollapsed.set(true);
    await vi.waitFor(() => {
      expect(backend.set).toHaveBeenCalledWith('sidebarCollapsed', true);
      expect(backend.save).toHaveBeenCalled();
    });
  });

  it('hydrate applies the stored value and refreshes the mirror', async () => {
    backend.get.mockResolvedValue(true);
    const sidebarCollapsed = await fresh();
    await sidebarCollapsed.hydrate();
    expect(get(sidebarCollapsed)).toBe(true);
    // Mirror must be rewritten or the next boot script paints the stale width.
    expect(localStorage.getItem('omnyssh-sidebar-collapsed')).toBe('true');
  });

  it('hydrate does not clobber a fresh user toggle', async () => {
    backend.get.mockResolvedValue(false); // stale persisted value
    const sidebarCollapsed = await fresh();
    sidebarCollapsed.set(true); // user acts before the async hydrate resolves
    await sidebarCollapsed.hydrate();
    expect(get(sidebarCollapsed)).toBe(true);
  });
});

describe('collapse chord (⌘B / Ctrl+B)', () => {
  const chord = (init: KeyboardEventInit) => isCollapseChord(new KeyboardEvent('keydown', init));

  it('matches ⌘B and Ctrl+B (either modifier, either case)', () => {
    expect(chord({ key: 'b', metaKey: true })).toBe(true);
    expect(chord({ key: 'b', ctrlKey: true })).toBe(true);
    expect(chord({ key: 'B', metaKey: true })).toBe(true);
  });

  it('ignores auto-repeat so a held chord is a single toggle', () => {
    expect(chord({ key: 'b', metaKey: true, repeat: true })).toBe(false);
  });

  it('rejects Alt-composed, IME-composing, unmodified, and other keys', () => {
    expect(chord({ key: 'b', metaKey: true, altKey: true })).toBe(false);
    expect(chord({ key: 'b', metaKey: true, isComposing: true })).toBe(false);
    expect(chord({ key: 'b' })).toBe(false);
    expect(chord({ key: 'k', metaKey: true })).toBe(false);
  });

  it('does not fire while typing in an editable field', () => {
    const input = document.createElement('input');
    document.body.append(input);
    let matched = true;
    input.addEventListener('keydown', (e) => (matched = isCollapseChord(e)));
    input.dispatchEvent(new KeyboardEvent('keydown', { key: 'b', metaKey: true, bubbles: true }));
    input.remove();
    expect(matched).toBe(false);
  });
});

describe('palette chord (⌘K / Ctrl+K)', () => {
  const chord = (init: KeyboardEventInit) => isPaletteChord(new KeyboardEvent('keydown', init));

  it('matches ⌘K and Ctrl+K (either modifier, either case)', () => {
    expect(chord({ key: 'k', metaKey: true })).toBe(true);
    expect(chord({ key: 'k', ctrlKey: true })).toBe(true);
    expect(chord({ key: 'K', metaKey: true })).toBe(true);
  });

  it('rejects auto-repeat, Alt-composed, IME-composing, unmodified, and other keys', () => {
    expect(chord({ key: 'k', metaKey: true, repeat: true })).toBe(false);
    expect(chord({ key: 'k', metaKey: true, altKey: true })).toBe(false);
    expect(chord({ key: 'k', metaKey: true, isComposing: true })).toBe(false);
    expect(chord({ key: 'k' })).toBe(false);
    expect(chord({ key: 'b', metaKey: true })).toBe(false);
  });

  it('fires even while typing in an input, unlike the collapse chord', () => {
    const input = document.createElement('input');
    document.body.append(input);
    let matched = false;
    input.addEventListener('keydown', (e) => (matched = isPaletteChord(e)));
    input.dispatchEvent(new KeyboardEvent('keydown', { key: 'k', metaKey: true, bubbles: true }));
    input.remove();
    expect(matched).toBe(true);
  });
});
