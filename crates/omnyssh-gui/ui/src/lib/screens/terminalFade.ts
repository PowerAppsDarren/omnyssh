// Pure predicate for the terminal's top-edge fade (tech-gui.md §5). The fade
// dissolves scrolled-past output under the title-bar strip, but it must never touch
// the live prompt: after `clear` (or Ctrl+L) the cursor lands at the top of the
// viewport, and fading that row blurs the input line. So fade only when the buffer is
// scrolled past its first line AND the cursor sits below the fade band. Kept out of the
// component so the clear/Ctrl+L invariant is unit-testable without an xterm instance.
// Rows near the top edge the fade dissolves; the cursor within this band reads as the
// live prompt and must stay crisp.
const FADE_ROWS = 2;

/**
 * Whether the top-edge fade should apply, from an xterm buffer's scroll geometry.
 * `viewportY` is the top visible line; `baseY + cursorY` is the cursor's absolute row.
 */
export function shouldFadeTop(viewportY: number, baseY: number, cursorY: number): boolean {
  return viewportY > 0 && baseY + cursorY > viewportY + FADE_ROWS;
}
