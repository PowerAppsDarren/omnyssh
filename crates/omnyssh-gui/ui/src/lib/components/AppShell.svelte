<script lang="ts">
  // The three fixed regions (tech-gui.md §2): sidebar (left), content (one thing),
  // status bar (full-width bottom). Content is passed in; the chrome is fixed.
  import type { Snippet } from 'svelte';
  import Sidebar from './Sidebar.svelte';
  import StatusBar from './StatusBar.svelte';

  let { children }: { children: Snippet } = $props();
</script>

<div class="grid h-screen grid-cols-[15rem_1fr] grid-rows-[1fr_auto] overflow-hidden bg-bg text-fg">
  <Sidebar />
  <main class="col-start-2 row-start-1 overflow-auto">
    {@render children()}
  </main>
  <StatusBar />
</div>
