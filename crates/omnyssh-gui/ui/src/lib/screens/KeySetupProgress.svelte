<script lang="ts">
  // Auto SSH-key setup progress panel (tech-gui.md §4.2). Renders the active run from
  // the keySetup store: a stepped progress bar while running, then the terminal
  // outcome (full or partial success / failure / rollback), each saying what happened
  // to the server's password login. On the first 'complete', or a failure that may
  // have left password login off, it reloads the host list once — the backend already
  // wrote hosts.toml, so the card must refresh to reflect the new hasKey /
  // passwordAuthDisabled, and "Open terminal" must use the saved key. Mounted globally
  // (AppShell) so it survives navigating away from the Dashboard mid-run.
  import Modal from '$lib/components/Modal.svelte';
  import { Button, Icon, StatusDot } from '$lib/theme';
  import { keySetup, dismissKeySetup } from '$lib/stores/keySetup';
  import { reloadHosts } from '$lib/ipc/commands';
  import { spawnSession } from '$lib/stores/navigation';
  import { lastError } from '$lib/stores/notifications';

  const message = (e: unknown): string => (e instanceof Error ? e.message : String(e));

  // Reload hosts once per run that may have saved a key. Cleared while a run has not, so
  // re-running setup on the same host reloads again.
  let reloadedFor = $state<string | null>(null);
  // The terminal waits for it, so it connects with the key the backend just saved.
  let reloaded: Promise<unknown> = Promise.resolve();
  const BACKUP = '/etc/ssh/sshd_config.omnyssh_backup.*';

  async function openTerminal(hostName: string): Promise<void> {
    await reloaded;
    spawnSession('terminal', hostName);
    dismissKeySetup();
  }

  $effect(() => {
    const run = $keySetup;
    if (!run) return;
    const phase = run.phase;
    if (phase.kind === 'complete' || (phase.kind === 'failed' && phase.passwordMayBeOff)) {
      if (reloadedFor !== run.hostName) {
        reloadedFor = run.hostName;
        reloaded = reloadHosts().catch((e) => lastError.set(message(e)));
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
          <span class="mt-0.5 shrink-0"><StatusDot status={phase.passwordOff ? 'ok' : 'warn'} size={9} /></span>
          <div class="min-w-0 space-y-1">
            {#if phase.passwordOff}
              <p class="text-sm font-medium">Key login is set up and password login is off</p>
              <p class="break-all font-mono text-xs text-muted">{phase.keyPath}</p>
              <p class="text-xs text-faint">
                The server's SSH settings were changed; a backup is kept at
                <span class="break-all font-mono">{BACKUP}</span>.
              </p>
              <p class="text-xs text-faint">The password saved in OmnySSH was removed.</p>
            {:else}
              <p class="text-sm font-medium">
                Key login works. Password login is still on — OmnySSH needs sudo without a
                password prompt to change server settings.
              </p>
              <p class="break-all font-mono text-xs text-muted">{phase.keyPath}</p>
              <p class="text-xs text-faint">Your saved password was kept.</p>
            {/if}
          </div>
        </div>
        <div class="flex justify-end">
          <Button variant="primary" onclick={dismissKeySetup}>Done</Button>
        </div>
      {:else if phase.kind === 'failed'}
        <div class="flex items-start gap-2.5">
          <span class="mt-0.5 shrink-0"><StatusDot status="crit" size={9} /></span>
          <div class="min-w-0 space-y-1">
            {#if phase.passwordMayBeOff}
              <p class="text-sm font-medium">Key setup failed — password login may be off</p>
              <p class="text-xs text-muted">
                The server's SSH settings may have been changed. Password login may be off
                now, or after the SSH service next restarts. The new key was saved for this
                host.
              </p>
              <p class="text-xs text-muted">
                To see, open a terminal and run
                <span class="break-all font-mono">sudo sshd -T | grep -i passwordauthentication</span>.
              </p>
              <p class="text-xs text-muted">
                To turn passwords back on, restore
                <span class="font-mono">/etc/ssh/sshd_config</span> from the newest
                <span class="break-all font-mono">{BACKUP}</span> and reload sshd. If you
                can't log in, use your hosting provider's console.
              </p>
            {:else}
              <p class="text-sm font-medium">Key setup failed</p>
            {/if}
            <p class="break-words text-xs text-muted">{phase.error}</p>
            {#if !phase.passwordMayBeOff}
              <p class="text-xs text-faint">Password login was not changed.</p>
            {/if}
          </div>
        </div>
        <div class="flex justify-end gap-2">
          {#if phase.passwordMayBeOff}
            <Button variant="secondary" onclick={() => openTerminal(run.hostName)}>Open terminal</Button>
          {/if}
          <Button variant="ghost" onclick={dismissKeySetup}>Close</Button>
        </div>
      {:else}
        <div class="flex items-start gap-2.5">
          <span class="mt-0.5 shrink-0"><StatusDot status="warn" size={9} /></span>
          <div class="min-w-0 space-y-1">
            <p class="text-sm font-medium">Password login was turned back on</p>
            <p class="text-xs text-muted">
              A step after password login was turned off failed, so OmnySSH restored the
              server's SSH settings from its backup. Password login works as before.
            </p>
            <p class="break-words text-xs text-faint">Cause: {phase.result}</p>
          </div>
        </div>
        <div class="flex justify-end">
          <Button variant="ghost" onclick={dismissKeySetup}>Close</Button>
        </div>
      {/if}
    </div>
  </Modal>
{/if}
