<script lang="ts">
  // Bottom region (tech-gui.md §2): context + host summary, and where background
  // errors surface (§3.5). The summary counts total / online / alert / offline
  // (§4.1); colour lives only in the status dots, per the brandbook.
  import { lastError } from '$lib/stores/notifications';
  import { hostSummary } from '$lib/stores/hostSummary';
  import { maskText, streamerMode } from '$lib/stores/streamer';
  import { streamerChordLabel } from '$lib/stores/ui';
  import { Icon, StatusDot } from '$lib/theme';

  const chipLabel = 'Streamer mode is on — click to turn it off';
</script>

<footer
  class="col-span-2 col-start-1 row-start-2 flex items-center justify-between gap-4 border-t border-default bg-surface px-5 py-2 text-xs text-muted"
>
  {#if $lastError}
    {@const text = $maskText($lastError)}
    <!-- A long error (a hint to act on) is cut to one line; hover shows all of it. -->
    <span class="min-w-0 truncate text-status-crit" title={text}>{text}</span>
  {:else}
    <span class="min-w-0 truncate">Ready</span>
  {/if}
  <div class="flex shrink-0 items-center gap-3">
    {#if $streamerMode}
      <!-- Always in view, so nobody records believing addresses are hidden when they
           are not, or forgets the mode is on. -->
      <button
        type="button"
        class="flex items-center gap-1.5 rounded-full border border-default px-2 py-0.5 text-fg transition hover:border-strong hover:bg-accent hover:text-accent-fg focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
        title="{chipLabel} ({streamerChordLabel})"
        aria-label={chipLabel}
        onclick={() => streamerMode.set(false)}
      >
        <Icon name="eye" size={13} />Streamer mode
      </button>
    {/if}
    <span>{$hostSummary.total} {$hostSummary.total === 1 ? 'host' : 'hosts'}</span>
    <span class="flex items-center gap-1.5">
      <StatusDot status="ok" label="online" />{$hostSummary.online} online
    </span>
    <span class="flex items-center gap-1.5">
      <StatusDot status="warn" label="alert" />{$hostSummary.alert} alert
    </span>
    <span class="flex items-center gap-1.5">
      <StatusDot status="off" label="offline" />{$hostSummary.offline} offline
    </span>
  </div>
</footer>
