import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { statusToken, type Status } from './status';

describe('statusToken', () => {
  it('maps each server state to its semantic token', () => {
    const cases: Record<Status, string> = {
      ok: 'var(--status-ok)',
      warn: 'var(--status-warn)',
      crit: 'var(--status-crit)',
      off: 'var(--status-off)',
      unknown: 'var(--text-faint)'
    };
    for (const [state, token] of Object.entries(cases)) {
      expect(statusToken(state as Status)).toBe(token);
    }
  });
});

// "StatusDot state→colour on both themes": every chromatic state must resolve to a
// token defined in both theme maps, with a theme-specific value (tech-gui.md §5.1).
describe('status tokens are defined and theme-specific', () => {
  const css = readFileSync(fileURLToPath(new URL('../../app.css', import.meta.url)), 'utf8');
  const block = (theme: 'light' | 'dark'): string => {
    const match = css.match(new RegExp(`\\[data-theme='${theme}'\\]\\s*\\{([^}]*)\\}`));
    if (!match) throw new Error(`missing ${theme} token block in app.css`);
    return match[1];
  };
  const value = (body: string, token: string): string | undefined =>
    body.match(new RegExp(`--${token}:\\s*([^;]+);`))?.[1].trim();

  const light = block('light');
  const dark = block('dark');

  for (const state of ['ok', 'warn', 'crit', 'off'] as const) {
    it(`--status-${state} is defined in both themes with distinct values`, () => {
      const l = value(light, `status-${state}`);
      const d = value(dark, `status-${state}`);
      expect(l).toBeTruthy();
      expect(d).toBeTruthy();
      expect(l).not.toBe(d);
    });
  }
});
