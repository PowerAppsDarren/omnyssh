import { writable } from 'svelte/store';

// Dashboard organisation prefs: group-by-tag and the folded sections. Like the sidebar
// collapse (./ui.ts) they survive restarts: persisted canonically via
// tauri-plugin-store, mirrored to localStorage for first paint and for a plain
// browser (Playwright, vite preview).
const LOCAL_KEY = 'omnyssh-dashboard-view';
const STORE_FILE = 'settings.json';
const STORE_KEY = 'dashboardView';

interface DashboardView {
  /** Split the grid into one section per first tag. */
  groupByTag: boolean;
  /** Folded sections by tag; '' is "Untagged", which no trimmed tag can be. */
  collapsed: string[];
}

const strings = (v: unknown): string[] =>
  Array.isArray(v) ? [...new Set(v.filter((s): s is string => typeof s === 'string'))] : [];

/** Coerce an untrusted persisted value into a valid view, falling back per field. */
function parseView(raw: unknown): DashboardView {
  const v = (typeof raw === 'object' && raw !== null ? raw : {}) as Record<string, unknown>;
  return {
    groupByTag: typeof v.groupByTag === 'boolean' ? v.groupByTag : false,
    collapsed: strings(v.collapsed)
  };
}

function mirroredView(): DashboardView {
  try {
    const raw = localStorage.getItem(LOCAL_KEY);
    return parseView(raw ? JSON.parse(raw) : null);
  } catch {
    return parseView(null); // localStorage unavailable or corrupt: defaults.
  }
}

function mirrorLocal(view: DashboardView): void {
  try {
    localStorage.setItem(LOCAL_KEY, JSON.stringify(view));
  } catch {
    // localStorage unavailable (hardened webview): the store copy is canonical.
  }
}

async function persistStore(view: DashboardView): Promise<void> {
  try {
    const { load } = await import('@tauri-apps/plugin-store');
    const store = await load(STORE_FILE);
    await store.set(STORE_KEY, view);
    await store.save();
  } catch {
    // Not under Tauri (tests, vite preview): the localStorage mirror suffices.
  }
}

function createDashboardView() {
  const initial = mirroredView();
  const { subscribe, set: setStore } = writable<DashboardView>(initial);
  let current = initial;
  let interacted = false;

  // `user` marks a deliberate change: it writes the canonical store and blocks a late
  // hydrate from reverting it.
  function apply(view: DashboardView, user: boolean): void {
    current = view;
    setStore(view);
    mirrorLocal(view);
    if (user) {
      interacted = true;
      void persistStore(view);
    }
  }

  return {
    subscribe,
    toggleGroupByTag: () => apply({ ...current, groupByTag: !current.groupByTag }, true),
    /** Fold or unfold a section. */
    toggleCollapsed: (key: string) =>
      apply(
        {
          ...current,
          collapsed: current.collapsed.includes(key)
            ? current.collapsed.filter((k) => k !== key)
            : [...current.collapsed, key]
        },
        true
      ),
    /** Reconcile with the canonical tauri-plugin-store value (called from the layout's onMount). */
    async hydrate(): Promise<void> {
      try {
        const { load } = await import('@tauri-apps/plugin-store');
        const store = await load(STORE_FILE);
        const saved = await store.get<unknown>(STORE_KEY);
        if (!interacted && saved !== undefined) apply(parseView(saved), false);
      } catch {
        // Store unreachable: keep the mirrored value.
      }
    }
  };
}

export const dashboardView = createDashboardView();
