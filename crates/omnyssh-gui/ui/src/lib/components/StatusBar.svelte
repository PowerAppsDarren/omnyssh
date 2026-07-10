<script lang="ts">
  // Bottom region (tech-gui.md §2): context + host summary, and where background
  // errors surface (§3.5). Stage 1.2 lands the live total/online/alert/offline
  // derivation; here it reflects the count and any load error.
  import { hosts } from '$lib/stores/hosts';
  import { lastError } from '$lib/stores/notifications';
</script>

<footer
  class="col-span-2 col-start-1 row-start-2 flex items-center justify-between gap-4 border-t border-default bg-surface px-5 py-2 text-xs text-muted"
>
  {#if $lastError}
    <span class="min-w-0 truncate text-status-crit">{$lastError}</span>
  {:else}
    <span class="min-w-0 truncate">Ready</span>
  {/if}
  <span class="shrink-0">{$hosts.length} hosts</span>
</footer>
