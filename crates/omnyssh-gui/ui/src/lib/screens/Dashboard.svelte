<script lang="ts">
  // Server-card grid (tech-gui.md §2, 2.1): one card per host with live health and
  // detected services. Colour is reserved for semantic state — the header dot and
  // the metric fills read from `statusToken`; everything else is ink-on-paper. The
  // per-card `sh`/`files` buttons are the host-first spawn path (§2).
  import { Surface, Chip, StatusDot, Icon, statusToken } from '$lib/theme';
  import { serverCards, QUICK_ACTIONS } from './serverCard';
  import { spawnSession } from '$lib/stores/navigation';

  const quickAction =
    'inline-flex items-center gap-1.5 rounded-full border border-default px-2.5 py-1 text-xs ' +
    'font-medium text-muted transition hover:border-strong hover:bg-accent hover:text-accent-fg ' +
    'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus';
</script>

<section class="h-full p-6">
  <h1 class="mb-5 text-lg font-semibold tracking-tight">Dashboard</h1>

  {#if $serverCards.length === 0}
    <div class="flex flex-col items-center justify-center gap-2 py-20 text-center">
      <p class="font-medium">No servers yet</p>
      <p class="text-sm text-muted">Add hosts to your SSH config or OmnySSH to see them here.</p>
    </div>
  {:else}
    <div class="grid gap-4 [grid-template-columns:repeat(auto-fill,minmax(19rem,1fr))]">
      {#each $serverCards as card, i (i)}
        <Surface class="flex flex-col gap-4 p-5">
          <!-- Identity + host-first quick actions -->
          <div class="flex items-start justify-between gap-3">
            <div class="flex min-w-0 items-start gap-2.5">
              <span class="mt-1 shrink-0">
                <StatusDot status={card.overall} size={9} label="{card.host.name} status" />
              </span>
              <div class="min-w-0">
                <div class="truncate font-medium" title={card.host.name}>{card.host.name}</div>
                <div class="truncate font-mono text-xs text-faint">
                  {card.host.user}@{card.host.hostname}:{card.host.port}
                </div>
              </div>
            </div>
            <div class="flex shrink-0 gap-1.5">
              {#each QUICK_ACTIONS as action (action.id)}
                <button
                  type="button"
                  class={quickAction}
                  title="{action.label} on {card.host.name}"
                  onclick={() => spawnSession(action.kind, card.host.name)}
                >
                  <Icon name={action.kind} size={13} />
                  {action.label}
                </button>
              {/each}
            </div>
          </div>

          <!-- Live metrics, or an offline state -->
          {#if card.offline}
            <div class="rounded-lg bg-surface-inset px-3 py-3 text-center text-xs text-faint">offline</div>
          {:else}
            <div class="space-y-2">
              {#each card.metricRows as row (row.label)}
                <div class="flex items-center gap-3">
                  <span class="w-9 shrink-0 text-[11px] uppercase tracking-wider text-faint">{row.label}</span>
                  <div class="h-1.5 flex-1 overflow-hidden rounded-full bg-surface-inset">
                    {#if row.percent != null}
                      <div
                        class="h-full rounded-full"
                        style="width: {Math.min(row.percent, 100)}%; background-color: {statusToken(row.status)};"
                      ></div>
                    {/if}
                  </div>
                  <span
                    class="w-10 shrink-0 text-right text-xs tabular-nums {row.percent == null
                      ? 'text-faint'
                      : 'text-muted'}"
                  >
                    {row.percent != null ? `${Math.round(row.percent)}%` : '—'}
                  </span>
                </div>
              {/each}
            </div>

            {#if card.uptime || card.osInfo}
              <div class="flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-muted">
                {#if card.uptime}<span>up {card.uptime}</span>{/if}
                {#if card.uptime && card.osInfo}<span class="text-faint">·</span>{/if}
                {#if card.osInfo}<span class="min-w-0 truncate">{card.osInfo}</span>{/if}
              </div>
            {/if}

            {#if card.topProcesses.length}
              <ul class="space-y-1">
                {#each card.topProcesses as proc, p (p)}
                  <li class="flex items-center justify-between gap-3 text-xs">
                    <span class="min-w-0 truncate font-mono text-muted">{proc.name}</span>
                    <span class="shrink-0 tabular-nums text-faint">{Math.round(proc.cpuPercent)}%</span>
                  </li>
                {/each}
              </ul>
            {/if}
          {/if}

          <!-- Detected services -->
          {#if card.detectedServices.length}
            <div class="flex flex-wrap gap-1.5">
              {#each card.detectedServices as service, s (s)}
                <Chip>{service.detail ? `${service.name} · ${service.detail}` : service.name}</Chip>
              {/each}
            </div>
          {:else if card.servicesError}
            <div class="text-xs text-faint">Service scan unavailable</div>
          {/if}
        </Surface>
      {/each}
    </div>
  {/if}
</section>
