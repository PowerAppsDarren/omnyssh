<script lang="ts">
  // Minimal settings (tech-gui.md §4.3): the light/dark theme mirrored from the sidebar
  // (§5.1), the metric auto-refresh interval (a tauri-plugin-store UI pref), and the
  // update preferences (`check_on_startup`) + a manual update check. Update prefs persist
  // to the shared config via `save_update_config`; theme/interval are frontend prefs.
  import { onMount } from 'svelte';
  import type { UpdateConfigDto } from '$lib/bindings';
  import { Surface, Icon } from '$lib/theme';
  import { theme } from '$lib/stores/theme';
  import { refreshInterval, REFRESH_OPTIONS } from '$lib/stores/settings';
  import { offerUpdate } from '$lib/stores/update';
  import { lastError } from '$lib/stores/notifications';
  import { checkUpdate, loadUpdateConfig, saveUpdateConfig } from '$lib/ipc/commands';

  const message = (e: unknown): string => (e instanceof Error ? e.message : String(e));
  const formatInterval = (secs: number): string => (secs < 60 ? `${secs}s` : `${secs / 60}m`);

  let updateConfig = $state<UpdateConfigDto | null>(null);
  type CheckState =
    | { kind: 'idle' }
    | { kind: 'checking' }
    | { kind: 'upToDate' }
    | { kind: 'available'; version: string }
    | { kind: 'error'; message: string };
  let check = $state<CheckState>({ kind: 'idle' });

  onMount(async () => {
    try {
      updateConfig = await loadUpdateConfig();
    } catch (e) {
      lastError.set(message(e));
    }
  });

  // Persist immediately; revert the optimistic flip if the write fails.
  async function toggleCheckOnStartup(): Promise<void> {
    if (!updateConfig) return;
    const previous = updateConfig;
    const next = { ...previous, checkOnStartup: !previous.checkOnStartup };
    updateConfig = next;
    try {
      await saveUpdateConfig(next);
    } catch (e) {
      updateConfig = previous;
      lastError.set(message(e));
    }
  }

  async function checkNow(): Promise<void> {
    check = { kind: 'checking' };
    try {
      const info = await checkUpdate();
      if (info) {
        offerUpdate(info); // raise the banner too
        check = { kind: 'available', version: info.version };
      } else {
        check = { kind: 'upToDate' };
      }
    } catch (e) {
      check = { kind: 'error', message: message(e) };
    }
  }

  const seg =
    'rounded-lg px-3 py-1.5 text-sm transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus';
  const segState = (active: boolean): string =>
    active ? 'bg-accent text-accent-fg' : 'text-muted hover:bg-surface-inset hover:text-fg';
</script>

<section class="mx-auto h-full max-w-2xl p-6">
  <h1 class="mb-5 text-lg font-semibold tracking-tight">Settings</h1>

  <div class="space-y-4">
    <!-- Appearance -->
    <Surface class="p-5">
      <h2 class="mb-3 text-sm font-semibold">Appearance</h2>
      <div class="flex items-center justify-between gap-4">
        <div>
          <p class="text-sm">Theme</p>
          <p class="text-xs text-muted">Mirrors the sidebar toggle.</p>
        </div>
        <div class="flex gap-1 rounded-xl bg-surface-inset p-1">
          <button
            type="button"
            class="{seg} {segState($theme === 'light')}"
            aria-pressed={$theme === 'light'}
            onclick={() => theme.set('light')}
          >
            <span class="flex items-center gap-1.5"><Icon name="sun" size={14} /> Light</span>
          </button>
          <button
            type="button"
            class="{seg} {segState($theme === 'dark')}"
            aria-pressed={$theme === 'dark'}
            onclick={() => theme.set('dark')}
          >
            <span class="flex items-center gap-1.5"><Icon name="moon" size={14} /> Dark</span>
          </button>
        </div>
      </div>
    </Surface>

    <!-- Dashboard -->
    <Surface class="p-5">
      <h2 class="mb-3 text-sm font-semibold">Dashboard</h2>
      <div class="flex items-center justify-between gap-4">
        <div>
          <p class="text-sm">Auto-refresh interval</p>
          <p class="text-xs text-muted">How often the dashboard forces a metric refresh.</p>
        </div>
        <div class="flex flex-wrap justify-end gap-1 rounded-xl bg-surface-inset p-1">
          {#each REFRESH_OPTIONS as secs (secs)}
            <button
              type="button"
              class="{seg} tabular-nums {segState($refreshInterval === secs)}"
              aria-pressed={$refreshInterval === secs}
              onclick={() => refreshInterval.set(secs)}
            >
              {formatInterval(secs)}
            </button>
          {/each}
        </div>
      </div>
    </Surface>

    <!-- Updates -->
    <Surface class="p-5">
      <h2 class="mb-3 text-sm font-semibold">Updates</h2>
      <div class="space-y-4">
        <div class="flex items-center justify-between gap-4">
          <div>
            <p class="text-sm">Check for updates on startup</p>
            <p class="text-xs text-muted">Look for a newer release when the app launches.</p>
          </div>
          <button
            type="button"
            role="switch"
            aria-checked={updateConfig?.checkOnStartup ?? false}
            aria-label="Check for updates on startup"
            disabled={!updateConfig}
            onclick={toggleCheckOnStartup}
            class="relative h-6 w-11 shrink-0 rounded-full transition disabled:opacity-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus {updateConfig?.checkOnStartup
              ? 'bg-accent'
              : 'bg-surface-inset'}"
          >
            <span
              class="absolute top-0.5 h-5 w-5 rounded-full bg-surface shadow-soft transition-[left] {updateConfig?.checkOnStartup
                ? 'left-[1.375rem]'
                : 'left-0.5'}"
            ></span>
          </button>
        </div>

        <div class="flex items-center justify-between gap-4 border-t border-default pt-4">
          <div class="min-w-0">
            <p class="text-sm">Manual check</p>
            <p class="text-xs text-muted">
              {#if check.kind === 'checking'}Checking…
              {:else if check.kind === 'upToDate'}You're on the latest version.
              {:else if check.kind === 'available'}Version {check.version} is available.
              {:else if check.kind === 'error'}{check.message}
              {:else}Check GitHub for a newer release.
              {/if}
            </p>
          </div>
          <button
            type="button"
            class="inline-flex shrink-0 items-center gap-1.5 rounded-full border border-strong px-4 py-1.5 text-sm text-fg transition hover:bg-accent hover:text-accent-fg disabled:opacity-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
            disabled={check.kind === 'checking'}
            onclick={checkNow}
          >
            <Icon name="refresh" size={14} />
            Check now
          </button>
        </div>
      </div>
    </Surface>
  </div>
</section>
