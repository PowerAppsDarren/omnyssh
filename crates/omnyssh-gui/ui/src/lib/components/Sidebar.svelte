<script lang="ts">
  // Left region (tech-gui.md §2). Stage 0.3 stands up the frame and the working
  // footer toggle; Stage 1.1 turns the entries into real selectors/spawners and
  // the sessions list.
  import Logo from './Logo.svelte';
  import ThemeToggle from './ThemeToggle.svelte';
  import { Button, Icon } from '$lib/theme';

  const entries = ['Dashboard', 'Snippets', 'SFTP', 'Terminal'];
</script>

<aside class="col-start-1 row-start-1 flex h-full flex-col border-r border-default bg-surface">
  <header class="flex items-center gap-2.5 px-5 py-4">
    <Logo size={22} />
    <span class="text-sm font-bold tracking-wide">OmnySSH</span>
  </header>

  <nav class="flex-1 px-3 py-2">
    <ul class="space-y-1">
      {#each entries as entry (entry)}
        <li class="rounded-lg px-3 py-2 text-sm text-muted">{entry}</li>
      {/each}
    </ul>
  </nav>

  <footer class="flex items-center gap-1 border-t border-default px-3 py-3">
    <Button variant="icon" title="Command palette (⌘K)">
      <Icon name="command" />
    </Button>
    <ThemeToggle />
  </footer>
</aside>
