<!-- Content follows the active entity (tech-gui.md §2): exactly one of Dashboard,
     Snippets, or the active session. The selector screens (Stage 2) and live
     sessions (Stage 3) replace these placeholders; the list_hosts call keeps the
     IPC pipe exercised and feeds the status-bar count. -->
<script lang="ts">
  import { onMount } from 'svelte';
  import { listHosts } from '$lib/ipc/commands';
  import { hosts } from '$lib/stores/hosts';
  import { lastError } from '$lib/stores/notifications';
  import { activeEntity } from '$lib/stores/activeEntity';
  import { sessions, sessionLabel } from '$lib/stores/sessions';
  import AppShell from '$lib/components/AppShell.svelte';
  import Logo from '$lib/components/Logo.svelte';
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
  <div class="flex h-full flex-col items-center justify-center gap-6 p-10 text-center">
    {#if $activeEntity.kind === 'dashboard'}
      <Logo size={56} />
      <div class="space-y-2">
        <h1 class="text-3xl font-bold tracking-tight">Dashboard</h1>
        <p class="text-muted">The live server dashboard lands in Stage 2.</p>
      </div>
    {:else if $activeEntity.kind === 'snippets'}
      <div class="space-y-2">
        <h1 class="text-3xl font-bold tracking-tight">Snippets</h1>
        <p class="text-muted">Saved commands land in Stage 2.</p>
      </div>
    {:else if session}
      <div class="flex flex-col items-center gap-3">
        <StatusDot status="unknown" size={12} label="session status" />
        <h1 class="font-mono text-2xl font-medium tracking-tight">
          {sessionLabel(session)}
        </h1>
        <p class="text-muted">Live sessions land in Stage 3.</p>
      </div>
    {/if}
  </div>
</AppShell>
