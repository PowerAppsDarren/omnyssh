<script lang="ts">
  // Auto SSH-key setup progress panel (tech-gui.md §4.2). Renders the active run from
  // the keySetup store: a stepped progress bar while running, then the terminal
  // outcome (success / failure / rollback). On the first 'complete' it reloads the
  // host list once — the backend already wrote hosts.toml, so the card must refresh to
  // reflect the new hasKey / passwordAuthDisabled. Mounted globally (AppShell) so it
  // survives navigating away from the Dashboard mid-run.
  import Modal from '$lib/components/Modal.svelte';
  import { Button, Icon, StatusDot } from '$lib/theme';
  import { keySetup, dismissKeySetup } from '$lib/stores/keySetup';
  import { reloadHosts } from '$lib/ipc/commands';
  import { lastError } from '$lib/stores/notifications';

  const message = (e: unknown): string => (e instanceof Error ? e.message : String(e));

  // Reload hosts once per completed run. Cleared while a run is not complete, so
  // re-running setup on the same host reloads again.
  let reloadedFor = $state<string | null>(null);
  $effect(() => {
    const run = $keySetup;
    if (!run) return;
    if (run.phase.kind === 'complete') {
      if (reloadedFor !== run.hostName) {
        reloadedFor = run.hostName;
        reloadHosts().catch((e) => lastError.set(message(e)));
      }
    } else if (reloadedFor === run.hostName) {
      reloadedFor = null;
    }
  });
</script>

{#if $keySetup}
  {@const run = $keySetup}
  {@const phase = run.phase}
  <Modal label="Key setup" onClose={dismissKeySetup}>
    <div class="space-y-4 px-5 py-4">
      <div class="flex items-center gap-2.5">
        <Icon name="key" size={16} />
        <h2 class="min-w-0 truncate text-sm font-semibold">SSH key setup — {run.hostName}</h2>
      </div>

      {#if phase.kind === 'running'}
        {@const index = phase.step?.index ?? 0}
        {@const total = phase.step?.total ?? 6}
        <div class="space-y-2">
          <div class="flex items-center justify-between gap-3 text-xs">
            <span class="min-w-0 truncate text-muted">{phase.step?.description ?? 'Connecting…'}</span>
            <span class="shrink-0 tabular-nums text-faint">{index}/{total}</span>
          </div>
          <div class="h-1.5 overflow-hidden rounded-full bg-surface-inset">
            <div
              class="h-full rounded-full bg-accent transition-[width] duration-300"
              style="width: {total ? Math.round((index / total) * 100) : 0}%"
            ></div>
          </div>
        </div>
        <p class="text-xs text-faint">
          Generating a key, authorising it on the server, and — with sudo — disabling
          password auth. Password auth is never disabled before key auth is verified.
        </p>
      {:else if phase.kind === 'complete'}
        <div class="flex items-start gap-2.5">
          <span class="mt-0.5 shrink-0"><StatusDot status="ok" size={9} /></span>
          <div class="min-w-0 space-y-1">
            <p class="text-sm font-medium">Key authentication configured</p>
            <p class="break-all font-mono text-xs text-muted">{phase.keyPath}</p>
          </div>
        </div>
        <div class="flex justify-end">
          <Button variant="primary" onclick={dismissKeySetup}>Done</Button>
        </div>
      {:else if phase.kind === 'failed'}
        <div class="flex items-start gap-2.5">
          <span class="mt-0.5 shrink-0"><StatusDot status="crit" size={9} /></span>
          <div class="min-w-0 space-y-1">
            <p class="text-sm font-medium">Key setup failed</p>
            <p class="break-words text-xs text-muted">{phase.error}</p>
            <p class="text-xs text-faint">Password authentication was not changed.</p>
          </div>
        </div>
        <div class="flex justify-end">
          <Button variant="ghost" onclick={dismissKeySetup}>Close</Button>
        </div>
      {:else}
        <div class="flex items-start gap-2.5">
          <span class="mt-0.5 shrink-0"><StatusDot status="warn" size={9} /></span>
          <div class="min-w-0 space-y-1">
            <p class="text-sm font-medium">Rolled back</p>
            <p class="break-words text-xs text-muted">{phase.result}</p>
            <p class="text-xs text-faint">The server's password authentication was restored.</p>
          </div>
        </div>
        <div class="flex justify-end">
          <Button variant="ghost" onclick={dismissKeySetup}>Close</Button>
        </div>
      {/if}
    </div>
  </Modal>
{/if}
