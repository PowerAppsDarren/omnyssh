<script lang="ts">
  import '../app.css';
  import { onMount } from 'svelte';
  import { startEventBridge } from '$lib/ipc/subscribe';

  let { children } = $props();

  onMount(() => {
    let stop: (() => void) | undefined;
    let disposed = false;
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
