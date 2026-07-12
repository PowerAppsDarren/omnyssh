import { writable } from 'svelte/store';

/** How long a surfaced error stays in the status bar before it clears itself (ms). */
export const ERROR_TTL_MS = 8000;

/** Last error surfaced to the status bar (tech-gui.md §3.5, §4.2). It auto-clears after
 *  `ERROR_TTL_MS` so the bar returns to the host summary instead of pinning a resolved
 *  error for the whole session; a fresh error restarts the window, `set(null)` clears now. */
function createLastError() {
  const { subscribe, set } = writable<string | null>(null);
  let timer: ReturnType<typeof setTimeout> | undefined;

  return {
    subscribe,
    set(message: string | null): void {
      if (timer !== undefined) clearTimeout(timer);
      timer = undefined;
      set(message);
      if (message) {
        timer = setTimeout(() => {
          timer = undefined;
          set(null);
        }, ERROR_TTL_MS);
        // Node's timers keep the test event loop alive; don't hold it open (no-op in the
        // webview, whose timers have no `unref`).
        (timer as { unref?: () => void }).unref?.();
      }
    }
  };
}

export const lastError = createLastError();
