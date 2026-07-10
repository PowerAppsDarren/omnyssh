<!-- Stage 0.3: the three-region shell with placeholder content. The selector
     screens and sessions replace this content from Stage 1 on (tech-gui.md §2, §7).
     The list_hosts call keeps the IPC pipe exercised and feeds the status-bar count. -->
<script lang="ts">
  import { onMount } from 'svelte';
  import { listHosts } from '$lib/ipc/commands';
  import { hosts } from '$lib/stores/hosts';
  import { lastError } from '$lib/stores/notifications';
  import AppShell from '$lib/components/AppShell.svelte';
  import Logo from '$lib/components/Logo.svelte';
  import { Chip, StatusDot, Surface } from '$lib/theme';

  onMount(async () => {
    try {
      hosts.set(await listHosts());
    } catch (err) {
      lastError.set(err instanceof Error ? err.message : String(err));
    }
  });
</script>

<AppShell>
  <div class="flex h-full flex-col items-center justify-center gap-8 p-10 text-center">
    <Logo size={64} />
    <div class="space-y-2">
      <h1 class="text-3xl font-bold tracking-tight">OmnySSH Desktop</h1>
      <p class="text-muted">Manage your servers from one surface.</p>
    </div>

    <Surface variant="raised" class="p-6">
      <div class="flex flex-wrap items-center justify-center gap-x-6 gap-y-2 text-sm text-muted">
        <span class="inline-flex items-center gap-2"><StatusDot status="ok" /> Online</span>
        <span class="inline-flex items-center gap-2"><StatusDot status="warn" /> Alert</span>
        <span class="inline-flex items-center gap-2"><StatusDot status="crit" /> Critical</span>
        <span class="inline-flex items-center gap-2"><StatusDot status="off" /> Offline</span>
      </div>
    </Surface>

    <div class="flex flex-wrap items-center justify-center gap-2">
      <Chip variant="solid">Dashboard</Chip>
      <Chip>Snippets</Chip>
      <Chip variant="outline">SFTP</Chip>
    </div>
  </div>
</AppShell>
