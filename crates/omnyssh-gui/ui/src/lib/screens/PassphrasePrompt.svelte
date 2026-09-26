<script lang="ts">
  // Asks for the passphrase of an encrypted identity file (tech-gui.md §4.2). The
  // backend keeps it in memory only; here it is dropped as soon as it is sent.
  import Modal from '$lib/components/Modal.svelte';
  import { Button, Icon } from '$lib/theme';
  import { passphrasePrompt, settlePassphrase } from '$lib/stores/passphrase';
  import { unlockIdentity } from '$lib/ipc/commands';

  const message = (e: unknown): string => (e instanceof Error ? e.message : String(e));

  let passphrase = $state('');
  let submitting = $state(false);
  let error = $state<string | null>(null);

  // A fresh form per key. Keyed on the path, not the prompt object, so a repeat
  // event for the key on screen does not wipe what is being typed.
  const keyPath = $derived($passphrasePrompt?.keyPath);
  $effect(() => {
    void keyPath;
    passphrase = '';
    error = null;
    submitting = false;
  });

  async function submit(): Promise<void> {
    if (!keyPath || submitting || !passphrase) return;
    const secret = passphrase;
    passphrase = '';
    submitting = true;
    error = null;
    try {
      await unlockIdentity(keyPath, secret);
      settlePassphrase(keyPath);
    } catch (e) {
      error = message(e);
    } finally {
      submitting = false;
    }
  }

  function cancel(): void {
    if (keyPath) settlePassphrase(keyPath);
  }

  const field =
    'w-full rounded-lg bg-surface-inset px-3 py-2 text-sm text-fg outline-none ' +
    'focus-visible:ring-2 focus-visible:ring-focus placeholder:text-faint';
</script>

{#if $passphrasePrompt}
  {@const prompt = $passphrasePrompt}
  <Modal label="Unlock SSH key" onClose={cancel}>
    <form
      class="space-y-4 px-5 py-4"
      onsubmit={(e) => {
        e.preventDefault();
        void submit();
      }}
    >
      <div class="space-y-1">
        <div class="flex items-center gap-2.5">
          <Icon name="key" size={16} />
          <h2 class="min-w-0 truncate text-sm font-semibold">Unlock SSH key — {prompt.hostName}</h2>
        </div>
        <p class="break-all font-mono text-xs text-muted">{prompt.keyPath}</p>
      </div>
      <label class="block space-y-1 text-xs font-medium text-muted">
        <span>Passphrase</span>
        <!-- svelte-ignore a11y_autofocus -->
        <input
          type="password"
          bind:value={passphrase}
          class={field}
          autocomplete="off"
          autofocus
        />
      </label>
      <p class="text-xs text-faint">
        Kept in memory until you quit OmnySSH, never written to disk. A terminal or file
        session that stopped on this key has to be opened again.
      </p>
      {#if error}
        <p class="text-xs text-status-crit">{error}</p>
      {/if}
      <div class="flex justify-end gap-2">
        <Button variant="ghost" type="button" onclick={cancel}>Cancel</Button>
        <Button variant="primary" type="submit" disabled={submitting || !passphrase}>
          {submitting ? 'Unlocking…' : 'Unlock'}
        </Button>
      </div>
    </form>
  </Modal>
{/if}
