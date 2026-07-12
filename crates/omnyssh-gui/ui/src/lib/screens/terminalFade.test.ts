import { describe, expect, it } from 'vitest';
import { shouldFadeTop } from './terminalFade';

// The top-edge fade dissolves scrolled history under the title bar but must never blur
// the live prompt — the `clear` / Ctrl+L regression (tech-gui.md §5).
describe('terminal top-edge fade', () => {
  it('is off at the buffer start (fresh terminal)', () => {
    expect(shouldFadeTop(0, 0, 0)).toBe(false);
    expect(shouldFadeTop(0, 0, 5)).toBe(false);
  });

  it('is on while following live output at the bottom of a long buffer', () => {
    // Scrolled to the bottom of a 500-line buffer, cursor on the last visible row.
    expect(shouldFadeTop(500, 500, 23)).toBe(true);
  });

  it('is on while scrolled up reading history', () => {
    // Viewport up in the scrollback, cursor far below it.
    expect(shouldFadeTop(120, 500, 23)).toBe(true);
  });

  it('stays off after `clear` resets the scrollback (viewport back at 0)', () => {
    expect(shouldFadeTop(0, 0, 0)).toBe(false);
  });

  it('stays off after Ctrl+L when scrollback is kept but the prompt is homed to the top', () => {
    // baseY still high (scrollback preserved), viewport at the bottom, cursor at row 0.
    expect(shouldFadeTop(500, 500, 0)).toBe(false);
  });
});
