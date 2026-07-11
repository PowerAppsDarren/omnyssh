import { writable } from 'svelte/store';

// The metric auto-refresh interval, in seconds (tech-gui.md §4.3). A UI preference —
// persisted via tauri-plugin-store with a localStorage mirror, exactly like the sidebar
// collapse / theme (§4.2, "UI prefs use tauri-plugin-store directly, no bespoke Rust
// command"). The frontend drives an immediate `refresh_metrics` on this cadence; the
// backend keeps its own baseline poll, so this is a floor on how *fresh* the dashboard
// stays, not a throttle.
const LOCAL_KEY = 'omnyssh-refresh-interval';
const STORE_FILE = 'settings.json';
const STORE_KEY = 'refreshInterval';

/** The intervals the settings screen offers, in seconds. */
export const REFRESH_OPTIONS = [10, 30, 60, 120, 300] as const;
const DEFAULT = 30;

/** Coerce any stored/typed value to a sane positive interval (seconds). */
export function clampInterval(value: unknown): number {
  const n = typeof value === 'number' ? value : Number(value);
  return Number.isFinite(n) && n >= 1 ? Math.round(n) : DEFAULT;
}

function mirrored(): number {
  try {
    const raw = localStorage.getItem(LOCAL_KEY);
    return raw == null ? DEFAULT : clampInterval(raw);
  } catch {
    return DEFAULT;
  }
}

function mirrorLocal(seconds: number): void {
  try {
    localStorage.setItem(LOCAL_KEY, String(seconds));
  } catch {
    // localStorage unavailable (hardened webview): the store copy is canonical.
  }
}

async function persistStore(seconds: number): Promise<void> {
  try {
    const { load } = await import('@tauri-apps/plugin-store');
    const store = await load(STORE_FILE);
    await store.set(STORE_KEY, seconds);
    await store.save();
  } catch {
    // Not under Tauri (tests, vite preview): the localStorage mirror suffices.
  }
}

function createRefreshInterval() {
  const initial = mirrored();
  const { subscribe, set: setStore } = writable<number>(initial);
  let current = initial;
  let interacted = false;

  function apply(seconds: number, user: boolean): void {
    const value = clampInterval(seconds);
    current = value;
    setStore(value);
    mirrorLocal(value);
    if (user) {
      interacted = true;
      void persistStore(value);
    }
  }

  return {
    subscribe,
    set: (seconds: number) => apply(seconds, true),
    /** Reconcile with the canonical tauri-plugin-store value once Tauri is reachable. */
    async hydrate(): Promise<void> {
      try {
        const { load } = await import('@tauri-apps/plugin-store');
        const store = await load(STORE_FILE);
        const saved = await store.get<number>(STORE_KEY);
        if (!interacted && typeof saved === 'number') apply(saved, false);
      } catch {
        // Store unreachable: keep the mirrored value.
      }
    }
  };
}

export const refreshInterval = createRefreshInterval();

/** Drive `refresh` on the current interval, re-arming whenever the interval changes.
 *  Returns a disposer. Kept free of the ipc layer so it stays unit-testable — the
 *  layout passes in the actual `refresh_metrics` call. */
export function driveMetricsRefresh(
  refresh: () => void,
  setInterval_: typeof setInterval = setInterval,
  clearInterval_: typeof clearInterval = clearInterval
): () => void {
  let timer: ReturnType<typeof setInterval> | undefined;
  const unsub = refreshInterval.subscribe((seconds) => {
    if (timer !== undefined) clearInterval_(timer);
    timer = setInterval_(refresh, clampInterval(seconds) * 1000);
  });
  return () => {
    unsub();
    if (timer !== undefined) clearInterval_(timer);
  };
}
