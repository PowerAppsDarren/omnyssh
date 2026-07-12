import { writable } from 'svelte/store';

// The support/about overlay (opened from the paper-plane button in the sidebar footer).
// A one-flag store so the footer button and the AppShell-level modal share the open
// state, mirroring the command palette's open/close pattern. It is an overlay, not a
// Content entity, so it never touches the exactly-one-active invariant (tech-gui.md §2).
function createSupport() {
  const { subscribe, set } = writable(false);
  return {
    subscribe,
    open: () => set(true),
    close: () => set(false)
  };
}

export const support = createSupport();
