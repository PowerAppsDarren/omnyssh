// Server-state colour is the only chromatic layer in the design (tech-gui.md §5).
// Each state resolves to a semantic token so it re-themes with the rest; `unknown`
// (not-yet-probed / connecting) stays neutral rather than inventing a colour.
export type Status = 'ok' | 'warn' | 'crit' | 'off' | 'unknown';

const STATUS_TOKEN: Record<Status, string> = {
  ok: 'var(--status-ok)',
  warn: 'var(--status-warn)',
  crit: 'var(--status-crit)',
  off: 'var(--status-off)',
  unknown: 'var(--text-faint)'
};

export function statusToken(status: Status): string {
  return STATUS_TOKEN[status];
}
