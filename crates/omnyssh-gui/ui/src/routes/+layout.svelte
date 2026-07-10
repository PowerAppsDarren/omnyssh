<script lang="ts">
  import '../app.css';
  import { onMount } from 'svelte';
  import { startEventBridge } from '$lib/ipc/subscribe';
  import { theme } from '$lib/stores/theme';

  let { children } = $props();

  onMount(() => {
    let stop: (() => void) | undefined;
    let disposed = false;
    // Reconcile with the canonical tauri-plugin-store theme; the no-FOUC script
    // already painted the localStorage guess (tech-gui.md §5.1).
    void theme.hydrate();
    // No-op outside Tauri (e.g. a plain `vite preview`); the shell still mounts.
    // Dispose even if the layout unmounts before the subscription resolves.
    startEventBridge()
      .then((off) => (disposed ? off() : (stop = off)))
      .catch(() => {});
    return () => {
      disposed = true;
      stop?.();
    };
  });
</script>

{@render children()}
