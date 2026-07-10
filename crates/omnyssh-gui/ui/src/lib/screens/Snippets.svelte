<script lang="ts">
  // Snippets selector screen (tech-gui.md §2, 2.2): list + search saved snippets,
  // add/edit/delete (round-tripping through `snippets.toml`), and run them on one or
  // more hosts with per-host results. CRUD orchestration refreshes the store from
  // disk after every mutation so it never drifts. Colour is reserved for state.
  import { onMount } from 'svelte';
  import type { SnippetDto } from '$lib/bindings';
  import { Surface, Chip, Icon, Button } from '$lib/theme';
  import { listSnippets, saveSnippet, deleteSnippet, executeSnippet } from '$lib/ipc/commands';
  import { snippets, beginRun } from '$lib/stores/snippets';
  import { lastError } from '$lib/stores/notifications';
  import { filterSnippets, emptyForm, formFromSnippet } from './snippetForm';
  import SnippetEditor from './SnippetEditor.svelte';
  import SnippetRunner from './SnippetRunner.svelte';
  import SnippetResults from './SnippetResults.svelte';
  import Modal from '$lib/components/Modal.svelte';

  type Dialog =
    | { kind: 'add' }
    | { kind: 'edit'; snippet: SnippetDto }
    | { kind: 'run'; snippet: SnippetDto }
    | { kind: 'delete'; snippet: SnippetDto };

  let query = $state('');
  let dialog = $state<Dialog | null>(null);
  const filtered = $derived(filterSnippets($snippets, query));

  const message = (e: unknown): string => (e instanceof Error ? e.message : String(e));

  async function refresh(): Promise<void> {
    try {
      snippets.set(await listSnippets());
    } catch (e) {
      lastError.set(message(e));
    }
  }

  onMount(refresh);

  // Persist add/edit, then re-read from disk. A rename (name changed on edit) drops
  // the old entry first, since the contract keys snippets on `name`. Throws propagate
  // to the editor so a failed save surfaces inline and keeps the form open.
  async function submit(snippet: SnippetDto, previousName: string | undefined): Promise<void> {
    if (previousName && previousName !== snippet.name) await deleteSnippet(previousName);
    await saveSnippet(snippet);
    snippets.set(await listSnippets());
    dialog = null;
  }

  async function confirmDelete(name: string): Promise<void> {
    try {
      await deleteSnippet(name);
      snippets.set(await listSnippets());
    } catch (e) {
      lastError.set(message(e));
    }
    dialog = null;
  }

  async function execute(
    snippet: SnippetDto,
    hostNames: string[],
    params: Record<string, string>
  ): Promise<void> {
    dialog = null;
    beginRun(snippet.name, hostNames);
    try {
      await executeSnippet(snippet.name, hostNames, params);
    } catch (e) {
      lastError.set(message(e));
    }
  }

  const search =
    'w-full rounded-lg bg-surface-inset px-3 py-2 text-sm text-fg outline-none ' +
    'focus-visible:ring-2 focus-visible:ring-focus placeholder:text-faint';
  const pill =
    'inline-flex items-center gap-1.5 rounded-full border border-default px-2.5 py-1 text-xs ' +
    'font-medium text-muted transition hover:border-strong hover:bg-accent hover:text-accent-fg ' +
    'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus';
  const iconBtn =
    'grid h-8 w-8 place-items-center rounded-lg text-muted transition hover:bg-surface-inset ' +
    'hover:text-fg focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus';
</script>

<section class="flex h-full flex-col p-6">
  <div class="mb-5 flex items-center gap-3">
    <h1 class="text-lg font-semibold tracking-tight">Snippets</h1>
    <div class="ml-auto w-full max-w-xs">
      <input bind:value={query} class={search} placeholder="Search snippets…" aria-label="Search snippets" />
    </div>
    <button type="button" class={pill} onclick={() => (dialog = { kind: 'add' })}>
      <Icon name="plus" size={13} />
      New snippet
    </button>
  </div>

  {#if filtered.length === 0}
    <div class="flex flex-1 flex-col items-center justify-center gap-2 text-center">
      {#if $snippets.length === 0}
        <p class="font-medium">No snippets yet</p>
        <p class="text-sm text-muted">Save a command to run it on your hosts in one click.</p>
        <button type="button" class="{pill} mt-2" onclick={() => (dialog = { kind: 'add' })}>
          <Icon name="plus" size={13} />
          New snippet
        </button>
      {:else}
        <p class="text-sm text-muted">No snippets match “{query}”.</p>
      {/if}
    </div>
  {:else}
    <ul class="min-h-0 flex-1 space-y-2 overflow-y-auto">
      {#each filtered as snippet (snippet.name)}
        <li>
          <Surface class="flex items-center gap-4 p-4">
            <div class="min-w-0 flex-1">
              <div class="flex flex-wrap items-center gap-2">
                <span class="truncate font-medium" title={snippet.name}>{snippet.name}</span>
                <Chip>{snippet.scope === 'host' && snippet.host ? `host · ${snippet.host}` : 'global'}</Chip>
                {#each snippet.tags ?? [] as tag (tag)}
                  <Chip variant="outline">{tag}</Chip>
                {/each}
              </div>
              <div class="mt-1 truncate font-mono text-xs text-muted" title={snippet.command}>
                {snippet.command}
              </div>
            </div>
            <div class="flex shrink-0 items-center gap-1.5">
              <button
                type="button"
                class={pill}
                title="Run {snippet.name}"
                aria-label="Run {snippet.name}"
                onclick={() => (dialog = { kind: 'run', snippet })}
              >
                <Icon name="play" size={12} />
                Run
              </button>
              <button type="button" class={iconBtn} title="Edit {snippet.name}" aria-label="Edit {snippet.name}" onclick={() => (dialog = { kind: 'edit', snippet })}>
                <Icon name="edit" size={15} />
              </button>
              <button type="button" class={iconBtn} title="Delete {snippet.name}" aria-label="Delete {snippet.name}" onclick={() => (dialog = { kind: 'delete', snippet })}>
                <Icon name="trash" size={15} />
              </button>
            </div>
          </Surface>
        </li>
      {/each}
    </ul>
  {/if}
</section>

{#if dialog?.kind === 'add'}
  <SnippetEditor mode="add" initial={emptyForm()} onSubmit={submit} onCancel={() => (dialog = null)} />
{:else if dialog?.kind === 'edit'}
  {@const snippet = dialog.snippet}
  <SnippetEditor
    mode="edit"
    initial={formFromSnippet(snippet)}
    previousName={snippet.name}
    onSubmit={submit}
    onCancel={() => (dialog = null)}
  />
{:else if dialog?.kind === 'run'}
  {@const snippet = dialog.snippet}
  <SnippetRunner
    {snippet}
    onExecute={(hostNames, params) => execute(snippet, hostNames, params)}
    onCancel={() => (dialog = null)}
  />
{:else if dialog?.kind === 'delete'}
  {@const snippet = dialog.snippet}
  <Modal label="Delete snippet" onClose={() => (dialog = null)}>
    <div class="space-y-3 px-5 py-4">
      <h2 class="text-sm font-semibold">Delete snippet</h2>
      <p class="text-sm text-muted">
        Delete “{snippet.name}”? This removes it from <span class="font-mono">snippets.toml</span>.
      </p>
      <div class="flex justify-end gap-2 pt-1">
        <Button variant="ghost" onclick={() => (dialog = null)}>Cancel</Button>
        <Button variant="primary" onclick={() => confirmDelete(snippet.name)}>Delete</Button>
      </div>
    </div>
  </Modal>
{/if}

<SnippetResults />
