<!-- Stage 0.2 proves the IPC pipe: the typed `list_hosts` command populates the
     hosts store, and the event bridge (subscribed in +layout) routes fired
     events into their stores. The three-region shell replaces this in Stage 0.3
     (tech-gui.md §2, §7). -->
<script lang="ts">
  import { onMount } from 'svelte';
  import { listHosts } from '$lib/ipc/commands';
  import { hosts } from '$lib/stores/hosts';
  import { lastError } from '$lib/stores/notifications';

  onMount(async () => {
    try {
      hosts.set(await listHosts());
    } catch (err) {
      lastError.set(err instanceof Error ? err.message : String(err));
    }
  });
</script>

<main class="shell">
  <p>OmnySSH Desktop</p>
  <p class="proof">list_hosts → {$hosts.length} hosts · {$lastError ?? 'no errors'}</p>
</main>

<style>
  .shell {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 100vh;
    gap: 0.5rem;
  }

  p {
    opacity: 0.6;
    letter-spacing: 0.02em;
  }

  .proof {
    font-size: 0.8rem;
    opacity: 0.4;
  }
</style>
