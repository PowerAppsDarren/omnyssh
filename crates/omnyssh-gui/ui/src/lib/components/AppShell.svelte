<script lang="ts">
  // The three fixed regions (tech-gui.md §2): sidebar (left), content (one thing),
  // status bar (full-width bottom). Content is passed in; the chrome is fixed. The
  // sidebar column width follows the collapse store; collapse is manual only — the
  // header button or ⌘B/Ctrl+B, never navigation.
  import type { Snippet } from 'svelte';
  import Sidebar from './Sidebar.svelte';
  import StatusBar from './StatusBar.svelte';
  import CommandPalette from './CommandPalette.svelte';
  import KeySetupProgress from '$lib/screens/KeySetupProgress.svelte';
  import UpdateBanner from './UpdateBanner.svelte';
  import { sidebarCollapsed, isCollapseChord } from '$lib/stores/ui';

  let { children }: { children: Snippet } = $props();

  function onKeydown(e: KeyboardEvent): void {
    if (isCollapseChord(e)) {
      e.preventDefault();
      sidebarCollapsed.toggle();
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

<div
  class="grid h-screen grid-rows-[1fr_auto] overflow-hidden bg-bg text-fg transition-[grid-template-columns] duration-200 ease-out {$sidebarCollapsed
    ? 'grid-cols-[3.5rem_1fr]'
    : 'grid-cols-[15rem_1fr]'}"
>
  <Sidebar />
  <main class="col-start-2 row-start-1 overflow-auto">
    {@render children()}
  </main>
  <StatusBar />
  <CommandPalette />
  <KeySetupProgress />
  <UpdateBanner />
</div>
