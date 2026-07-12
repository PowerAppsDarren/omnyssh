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
  const initial = mirroredCollapsed();
  const { subscribe, set: setStore } = writable<boolean>(initial);
  let current = initial;
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

/** The manual sidebar-collapse chord (tech-gui.md §2): ⌘B / Ctrl+B. Auto-repeat is
 *  ignored so a held chord is a single toggle (not an oscillation + write storm),
 *  Alt-composed variants are left for other bindings, and the global chord never
 *  fires while typing in an editable surface (command palette / host forms arrive
 *  in later stages). */
export function isCollapseChord(e: KeyboardEvent): boolean {
  if (e.repeat || e.altKey || e.isComposing) return false;
  if (!(e.metaKey || e.ctrlKey) || (e.key !== 'b' && e.key !== 'B')) return false;
  const t = e.target as HTMLElement | null;
  return !t?.isContentEditable && !/^(input|textarea|select)$/i.test(t?.tagName ?? '');
}

/** The command-palette chord (tech-gui.md §2): ⌘K / Ctrl+K. Unlike collapse it fires
 *  regardless of focus so the palette is reachable from anywhere (including its own
 *  input); auto-repeat and Alt-composed variants are still ignored. */
export function isPaletteChord(e: KeyboardEvent): boolean {
  if (e.repeat || e.altKey || e.isComposing) return false;
  return (e.metaKey || e.ctrlKey) && (e.key === 'k' || e.key === 'K');
}

/** The dashboard metric-refresh hotkey (mirrors the TUI's `r`): a bare r/R with no
 *  modifier, ignored while typing in an editable surface so it never eats a keystroke.
 *  Terminal input is never disrupted because the dashboard — the only mounter of this
 *  listener — unmounts whenever a session is active (tech-gui.md §2). */
export function isRefreshHotkey(e: KeyboardEvent): boolean {
  if (e.repeat || e.metaKey || e.ctrlKey || e.altKey || e.isComposing) return false;
  if (e.key !== 'r' && e.key !== 'R') return false;
  const t = e.target as HTMLElement | null;
  return !t?.isContentEditable && !/^(input|textarea|select)$/i.test(t?.tagName ?? '');
}
