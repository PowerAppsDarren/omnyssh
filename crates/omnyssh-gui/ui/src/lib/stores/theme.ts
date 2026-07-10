import { writable } from 'svelte/store';

// The app-side source of truth for the active theme (tech-gui.md §5.1). Flipping
// it swaps `data-theme` on the document root — every CSS-variable surface follows
// — then persists the choice (tauri-plugin-store canonical + localStorage mirror
// for the no-FOUC boot) and points the native window decorations at it.
export type Theme = 'light' | 'dark';

const LOCAL_KEY = 'omnyssh-theme';
const STORE_FILE = 'settings.json';
const STORE_KEY = 'theme';

/** The theme the app.html no-FOUC script already painted, so the store agrees
 *  with the first frame. Defaults to dark. */
function paintedTheme(): Theme {
  if (typeof document !== 'undefined') {
    const attr = document.documentElement.getAttribute('data-theme');
    if (attr === 'light' || attr === 'dark') return attr;
  }
  return 'dark';
}

function reflectAttribute(theme: Theme): void {
  if (typeof document !== 'undefined') {
    document.documentElement.setAttribute('data-theme', theme);
  }
}

function mirrorLocal(theme: Theme): void {
  try {
    localStorage.setItem(LOCAL_KEY, theme);
  } catch {
    // localStorage unavailable (hardened webview): the store copy is canonical.
  }
}

async function persistStore(theme: Theme): Promise<void> {
  try {
    const { load } = await import('@tauri-apps/plugin-store');
    const store = await load(STORE_FILE);
    await store.set(STORE_KEY, theme);
    await store.save();
  } catch {
    // Not under Tauri (tests, vite preview): the localStorage mirror suffices.
  }
}

async function syncNativeWindow(theme: Theme): Promise<void> {
  try {
    const { getCurrentWindow } = await import('@tauri-apps/api/window');
    await getCurrentWindow().setTheme(theme);
  } catch {
    // No native window (tests, vite preview): nothing to follow.
  }
}

function createTheme() {
  const { subscribe, set: setStore } = writable<Theme>(paintedTheme());
  let current = paintedTheme();

  function apply(theme: Theme, persist = true): void {
    current = theme;
    reflectAttribute(theme);
    setStore(theme);
    if (persist) {
      mirrorLocal(theme);
      void persistStore(theme);
    }
    void syncNativeWindow(theme);
  }

  return {
    subscribe,
    set: (theme: Theme) => apply(theme),
    toggle: () => apply(current === 'dark' ? 'light' : 'dark'),
    /** Reconcile with the canonical tauri-plugin-store value once the Tauri API is
     *  reachable (called from the layout's onMount). */
    async hydrate(): Promise<void> {
      try {
        const { load } = await import('@tauri-apps/plugin-store');
        const store = await load(STORE_FILE);
        const saved = await store.get<Theme>(STORE_KEY);
        if (saved === 'light' || saved === 'dark') apply(saved, false);
      } catch {
        // Store unreachable: keep the painted/localStorage theme.
      }
    }
  };
}

export const theme = createTheme();
