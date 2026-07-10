import { writable } from 'svelte/store';

// Sticky UI-chrome prefs (tech-gui.md §2, §3.5). Sidebar collapse is manual-only
// (header button or ⌘B) and must survive restarts: persist canonically via
// tauri-plugin-store, and mirror to localStorage so the SPA renders the right width
// on first paint — and so a plain browser (Playwright, vite preview) persists it
// too. This mirrors the theme store's persistence shape.
const LOCAL_KEY = 'omnyssh-sidebar-collapsed';
const STORE_FILE = 'settings.json';
const STORE_KEY = 'sidebarCollapsed';

function mirroredCollapsed(): boolean {
  try {
    return localStorage.getItem(LOCAL_KEY) === 'true';
  } catch {
    return false; // localStorage unavailable: default to expanded.
  }
}

function mirrorLocal(collapsed: boolean): void {
  try {
    localStorage.setItem(LOCAL_KEY, String(collapsed));
  } catch {
    // localStorage unavailable (hardened webview): the store copy is canonical.
  }
}

async function persistStore(collapsed: boolean): Promise<void> {
  try {
    const { load } = await import('@tauri-apps/plugin-store');
    const store = await load(STORE_FILE);
    await store.set(STORE_KEY, collapsed);
    await store.save();
  } catch {
    // Not under Tauri (tests, vite preview): the localStorage mirror suffices.
  }
}

function createSidebarCollapsed() {
  const { subscribe, set: setStore } = writable<boolean>(mirroredCollapsed());
  let current = mirroredCollapsed();
  let interacted = false;

  // `user` marks a deliberate flip: it writes the canonical store and blocks a late
  // hydrate from reverting it. The localStorage mirror is refreshed either way so
  // the next boot reads the true width.
  function apply(collapsed: boolean, user: boolean): void {
    current = collapsed;
    setStore(collapsed);
    mirrorLocal(collapsed);
    if (user) {
      interacted = true;
      void persistStore(collapsed);
    }
  }

  return {
    subscribe,
    set: (collapsed: boolean) => apply(collapsed, true),
    toggle: () => apply(!current, true),
    /** Reconcile with the canonical tauri-plugin-store value once the Tauri API is
     *  reachable (called from the layout's onMount). */
    async hydrate(): Promise<void> {
      try {
        const { load } = await import('@tauri-apps/plugin-store');
        const store = await load(STORE_FILE);
        const saved = await store.get<boolean>(STORE_KEY);
        if (!interacted && typeof saved === 'boolean') apply(saved, false);
      } catch {
        // Store unreachable: keep the mirrored value.
      }
    }
  };
}

export const sidebarCollapsed = createSidebarCollapsed();
