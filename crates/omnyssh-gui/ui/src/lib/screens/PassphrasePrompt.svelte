<script lang="ts">
  // Prompt for the passphrase of an encrypted identity file. The value is sent
  // to the backend and cached in process memory only — never written to disk.
  import Modal from '$lib/components/Modal.svelte';
  import { Button } from '$lib/theme';
  import { passphrasePrompt, dismissPassphrasePrompt } from '$lib/stores/passphrase';
  import { unlockIdentity } from '$lib/ipc/commands';
  import { lastError } from '$lib/stores/notifications';

  const message = (e: unknown): string => (e instanceof Error ? e.message : String(e));

  let passphrase = $state('');
  let submitting = $state(false);
  let localError = $state<string | null>(null);

  $effect(() => {
    if ($passphrasePrompt) {
      passphrase = '';
      localError = null;
      submitting = false;
    }
  });

  async function submit(): Promise<void> {
    const prompt = $passphrasePrompt;
    if (!prompt || submitting) return;
    submitting = true;
    localError = null;
    try {
      await unlockIdentity(prompt.keyPath, passphrase);
      dismissPassphrasePrompt();
    } catch (e) {
      localError = message(e);
      lastError.set(message(e));
    } finally {
      submitting = false;
    }
  }
</script>

{#if $passphrasePrompt}
  {@const prompt = $passphrasePrompt}
  <Modal label="Unlock SSH key" onClose={dismissPassphrasePrompt}>
    <form
      class="space-y-4 px-5 py-4"
      onsubmit={(e) => {
        e.preventDefault();
        void submit();
      }}
    >
      <div class="space-y-1">
        <h2 class="text-sm font-semibold">Unlock SSH key — {prompt.hostName}</h2>
        <p class="break-all font-mono text-xs text-muted">{prompt.keyPath}</p>
        <p class="text-xs text-faint">
          The passphrase is kept in memory until you quit OmnySSH. It is never written to disk.
        </p>
      </div>
      <label class="block space-y-1">
        <span class="text-xs text-muted">Passphrase</span>
        <input
          type="password"
          bind:value={passphrase}
          class="w-full rounded-lg border border-default bg-surface-inset px-3 py-2 text-sm text-fg outline-none focus-visible:ring-2 focus-visible:ring-focus"
          autocomplete="off"
          autofocus
        />
      </label>
      {#if localError}
        <p class="text-xs text-muted">{localError}</p>
      {/if}
      <div class="flex justify-end gap-2">
        <Button variant="ghost" type="button" onclick={dismissPassphrasePrompt}>Cancel</Button>
        <Button variant="primary" type="submit" disabled={submitting || !passphrase}>
          Unlock
        </Button>
      </div>
    </form>
  </Modal>
{/if}
