<!-- Content follows the active entity (tech-gui.md §2): exactly one of Dashboard,
     Snippets, or the active session. Live sessions (Stage 3) replace their
     placeholder; the list_hosts call keeps the IPC pipe exercised and feeds the
     status-bar count. -->
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
  import { StatusDot } from '$lib/theme';

  onMount(async () => {
    try {
      hosts.set(await listHosts());
    } catch (err) {
      lastError.set(err instanceof Error ? err.message : String(err));
    }
  });

  // The active session's row, resolved for the placeholder heading.
  const session = $derived(
    $activeEntity.kind === 'session'
      ? $sessions.find((s) => s.id === $activeEntity.id)
      : undefined
  );
</script>

<AppShell>
  {#if $activeEntity.kind === 'dashboard'}
    <Dashboard />
  {:else if $activeEntity.kind === 'snippets'}
    <Snippets />
  {:else if session}
    <div class="flex h-full flex-col items-center justify-center gap-6 p-10 text-center">
      <div class="flex flex-col items-center gap-3">
        <StatusDot status="unknown" size={12} label="session status" />
        <h1 class="font-mono text-2xl font-medium tracking-tight">
          {sessionLabel(session)}
        </h1>
        <p class="text-muted">Live sessions land in Stage 3.</p>
      </div>
    </div>
  {/if}
</AppShell>
