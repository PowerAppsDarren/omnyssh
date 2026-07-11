<!-- Content follows the active entity (tech-gui.md §2): exactly one of Dashboard,
     Snippets, or one session. Terminal tabs live in a persistent layer so their
     scrollback and byte stream survive switching away (only visibility toggles);
     SFTP sessions land in Stage 3.2. The list_hosts call feeds the status-bar count. -->
<script lang="ts">
  import { onMount } from 'svelte';
  import { listHosts } from '$lib/ipc/commands';
  import { hosts } from '$lib/stores/hosts';
  import { lastError } from '$lib/stores/notifications';
  import { activeEntity } from '$lib/stores/activeEntity';
  import { sessions, sessionLabel } from '$lib/stores/sessions';
  import AppShell from '$lib/components/AppShell.svelte';
  import Dashboard from '$lib/screens/Dashboard.svelte';
  import Snippets from '$lib/screens/Snippets.svelte';
  import TerminalView from '$lib/screens/TerminalView.svelte';
  import { StatusDot } from '$lib/theme';

  onMount(async () => {
    try {
      hosts.set(await listHosts());
    } catch (err) {
      lastError.set(err instanceof Error ? err.message : String(err));
    }
  });

  const activeSessionId = $derived($activeEntity.kind === 'session' ? $activeEntity.id : null);
  const activeSession = $derived(
    activeSessionId != null ? $sessions.find((s) => s.id === activeSessionId) : undefined
  );
  // Terminals render in the persistent layer; the overlay covers every other case.
  const terminalActive = $derived(activeSession?.kind === 'terminal');
  const terminalSessions = $derived($sessions.filter((s) => s.kind === 'terminal'));
</script>

<AppShell>
  <div class="relative h-full">
    {#each terminalSessions as s (s.id)}
      <TerminalView session={s} active={activeSessionId === s.id} />
    {/each}

    {#if !terminalActive}
      <div class="absolute inset-0 overflow-auto">
        {#if $activeEntity.kind === 'dashboard'}
          <Dashboard />
        {:else if $activeEntity.kind === 'snippets'}
          <Snippets />
        {:else if activeSession}
          <div class="flex h-full flex-col items-center justify-center gap-6 p-10 text-center">
            <div class="flex flex-col items-center gap-3">
              <StatusDot status="unknown" size={12} label="session status" />
              <h1 class="font-mono text-2xl font-medium tracking-tight">
                {sessionLabel(activeSession)}
              </h1>
              <p class="text-muted">SFTP sessions land in Stage 3.2.</p>
            </div>
          </div>
        {/if}
      </div>
    {/if}
  </div>
</AppShell>
