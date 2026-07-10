<script lang="ts">
  // Bottom region (tech-gui.md §2): context + host summary. Stage 1.2 lands the
  // live total/online/alert/offline derivation; here it just reflects the count.
  import { hosts } from '$lib/stores/hosts';
</script>

<footer
  class="col-span-2 col-start-1 row-start-2 flex items-center justify-between border-t border-default bg-surface px-5 py-2 text-xs text-muted"
>
  <span>Ready</span>
  <span>{$hosts.length} hosts</span>
</footer>
