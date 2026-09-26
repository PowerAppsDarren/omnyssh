<script lang="ts">
  // Asks for the login password a connection waits on (tech-gui.md §4.2). The
  // server checks it; a refused one comes back as a new prompt marked `retry`.
  // The password is dropped here as soon as it is sent.
  import Modal from '$lib/components/Modal.svelte';
  import { Button, Icon } from '$lib/theme';
  import { displayLogin, passwordPrompt, settlePassword } from '$lib/stores/password';
  import { streamerMode } from '$lib/stores/streamer';
  import { lastError } from '$lib/stores/notifications';
  import { answerPassword } from '$lib/ipc/commands';

  const message = (e: unknown): string => (e instanceof Error ? e.message : String(e));

  let password = $state('');
  let input = $state<HTMLInputElement>();

  // A fresh, focused form per request. Focus is taken outright: autofocus yields to
  // whatever holds it, often a live terminal, which would then get the password.
  const requestId = $derived($passwordPrompt?.requestId);
  $effect(() => {
    void requestId;
    password = '';
    input?.focus();
  });

  // The dialog opens unbidden: focus goes back when the last prompt closes.
  // Pre-effect, so it is read before the input takes it.
  const open = $derived($passwordPrompt !== null);
  let opener: Element | null = null;
  $effect.pre(() => {
    if (open) {
      opener = document.activeElement;
    } else if (opener instanceof HTMLElement) {
      opener.focus();
      opener = null;
    }
  });

  async function answer(secret: string | null): Promise<void> {
    const id = requestId;
    if (id === undefined) return;
    password = '';
    // Settled first either way: the answer goes once, and a connection that
    // stopped waiting has nothing to come back with.
    settlePassword(id);
    try {
      await answerPassword(id, secret);
    } catch (e) {
      lastError.set(message(e));
    }
  }

  function submit(): void {
    if (password) void answer(password);
  }

  const field =
    'w-full rounded-lg bg-surface-inset px-3 py-2 text-sm text-fg outline-none ' +
    'focus-visible:ring-2 focus-visible:ring-focus placeholder:text-faint';
</script>

{#if $passwordPrompt}
  {@const prompt = $passwordPrompt}
  <Modal label="SSH login" onClose={() => void answer(null)}>
    <form
      class="space-y-4 px-5 py-4"
      onsubmit={(e) => {
        e.preventDefault();
        submit();
      }}
    >
      <div class="space-y-1">
        <div class="flex items-center gap-2.5">
          <Icon name="key" size={16} />
          <h2 class="min-w-0 truncate text-sm font-semibold">SSH login — {prompt.hostName}</h2>
        </div>
        <p class="break-all font-mono text-xs text-muted">
          {displayLogin(prompt.login, $streamerMode)}
        </p>
      </div>
      <label class="block space-y-1 text-xs font-medium text-muted">
        <span>Password</span>
        <input
          type="password"
          bind:this={input}
          bind:value={password}
          class={field}
          autocomplete="off"
        />
      </label>
      <p class="text-xs text-faint">
        Kept in memory until you quit OmnySSH, never written to disk.
      </p>
      {#if prompt.retry}
        <p class="text-xs text-status-crit">Permission denied, please try again.</p>
      {/if}
      <div class="flex justify-end gap-2">
        <Button variant="ghost" type="button" onclick={() => void answer(null)}>Cancel</Button>
        <Button variant="primary" type="submit" disabled={!password}>Log in</Button>
      </div>
    </form>
  </Modal>
{/if}
