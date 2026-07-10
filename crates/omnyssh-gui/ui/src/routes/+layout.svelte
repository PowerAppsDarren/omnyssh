<script lang="ts">
  import '../app.css';
  import { onMount } from 'svelte';
  import { startEventBridge } from '$lib/ipc/subscribe';
  import { reloadHosts } from '$lib/ipc/commands';
  import { theme } from '$lib/stores/theme';
  import { sidebarCollapsed } from '$lib/stores/ui';
  import { lastError } from '$lib/stores/notifications';

  let { children } = $props();

  onMount(() => {
    let stop: (() => void) | undefined;
    let disposed = false;
    // Reconcile the persisted prefs with their canonical tauri-plugin-store values;
    // the synchronous localStorage mirrors already seeded the first paint (§5.1, §2).
    void theme.hydrate();
    void sidebarCollapsed.hydrate();
    // No-op outside Tauri (e.g. a plain `vite preview`); the shell still mounts.
    // Dispose even if the layout unmounts before the subscription resolves. Start
    // the pollers only once listeners are attached, so no status event is missed.
    startEventBridge()
      .then((off) => {
        if (disposed) return off();
        stop = off;
        reloadHosts().catch((err) => lastError.set(err instanceof Error ? err.message : String(err)));
      })
      .catch(() => {});
    return () => {
      disposed = true;
      stop?.();
    };
  });
</script>

{@render children()}
