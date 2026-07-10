// The icon registry's name set, shared so callers get compile-time-checked glyphs
// (a typo fails svelte-check rather than rendering nothing).
export type IconName =
  | 'sun'
  | 'moon'
  | 'command'
  | 'dashboard'
  | 'snippets'
  | 'sftp'
  | 'terminal'
  | 'close'
  | 'collapse'
  | 'expand';
