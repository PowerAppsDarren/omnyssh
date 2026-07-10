// @vitest-environment jsdom
import { beforeEach, describe, expect, it } from 'vitest';
import { get } from 'svelte/store';
import { theme } from './theme';

// Tauri persistence (tauri-plugin-store) and the native window follow lazily and
// no-op off-runtime; the unit-testable contract is the document attribute and the
// localStorage mirror the no-FOUC boot reads (tech-gui.md §5.1).
describe('theme store', () => {
  beforeEach(() => {
    localStorage.clear();
    theme.set('dark');
  });

  it('sets data-theme and mirrors to localStorage', () => {
    theme.set('light');
    expect(document.documentElement.getAttribute('data-theme')).toBe('light');
    expect(localStorage.getItem('omnyssh-theme')).toBe('light');
    expect(get(theme)).toBe('light');
  });

  it('toggle flips dark↔light and re-persists each time', () => {
    theme.toggle();
    expect(get(theme)).toBe('light');
    expect(document.documentElement.getAttribute('data-theme')).toBe('light');
    expect(localStorage.getItem('omnyssh-theme')).toBe('light');

    theme.toggle();
    expect(get(theme)).toBe('dark');
    expect(document.documentElement.getAttribute('data-theme')).toBe('dark');
    expect(localStorage.getItem('omnyssh-theme')).toBe('dark');
  });
});
