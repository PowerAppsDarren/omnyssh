<script lang="ts">
  // Asks before a key setup (tech-gui.md §4.3 note, 2026-10-09): it changes this
  // computer and, with sudo, the server's login for everyone. `rerun` is the "Password
  // on" badge's second try, which reuses the key OmnySSH made before.
  import { onMount } from 'svelte';
  import type { HostDto } from '$lib/bindings';
  import Modal from '$lib/components/Modal.svelte';
  import { Button } from '$lib/theme';
  import { keyFilePath } from '$lib/stores/keySetup';
  import { streamerMode, displayHostname } from '$lib/stores/streamer';
  import { isWindows } from '$lib/platform';

  let {
    host,
    rerun,
    onConfirm,
    onCancel
  }: { host: HostDto; rerun: boolean; onConfirm: () => void; onCancel: () => void } = $props();

  const keyPath = $derived(keyFilePath(host.name, isWindows));

  // Cancel is first in `actions`. Focused by hand: Modal sets no initial focus, and
  // `autofocus` is skipped while the opening button still holds focus.
  let actions = $state<HTMLElement>();
  onMount(() => actions?.querySelector('button')?.focus());
</script>

<Modal label={rerun ? 'Turn off password login' : 'Set up key login'} onClose={onCancel}>
  <div class="space-y-3 px-5 py-4">
    <h2 class="text-sm font-semibold">
      {rerun ? `Turn off password login on ${host.name}?` : `Set up key login for ${host.name}?`}
    </h2>
    <ul class="list-disc space-y-1.5 pl-5 text-sm text-muted">
      {#if rerun}
        <li>
          Reuse the login key OmnySSH made before, or create it again if it is gone:
          <span class="break-all font-mono text-xs">{keyPath}</span>
        </li>
      {:else}
        <li>
          Create a login key on this computer:
          <span class="break-all font-mono text-xs">{keyPath}</span> (no passphrase)
        </li>
      {/if}
      <li>
        Add it to
        <span class="break-all font-mono text-xs">{host.user}@{displayHostname(host.hostname, $streamerMode)}</span>,
        so this computer logs in without a password
      </li>
      <li>
        Turn off password login on the server for every account and device, if your account
        can use sudo without a password. A backup of the server's SSH settings is kept.
      </li>
    </ul>
    <p class="text-xs text-faint">
      Password login is turned off only after the new key is tested. Keep the key file: if
      you lose it, you'll need your hosting provider's console to get back in.
    </p>
    <div class="flex justify-end gap-2 pt-1" bind:this={actions}>
      <Button variant="ghost" onclick={onCancel}>Cancel</Button>
      <Button variant="primary" onclick={onConfirm}>
        {rerun ? 'Turn off password login' : 'Set up key login'}
      </Button>
    </div>
  </div>
</Modal>
