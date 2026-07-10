<script lang="ts">
  // The ⌘K overlay and the host-picker, one component in two modes (tech-gui.md §2,
  // 1.3). Navigator: jump to an open session or open a host (default: a shell). Picker:
  // hand a chosen host back to its caller (the spawner buttons). Keyboard-first — type
  // to filter, ↑/↓ to move, ↵ to select, esc to dismiss. Glass/blur per the brandbook.
  import { tick } from 'svelte';
  import { Icon, StatusDot, type Status } from '$lib/theme';
  import { palette, paletteItems, nextIndex, hostStatusDot } from '$lib/stores/palette';
  import { hosts } from '$lib/stores/hosts';
  import { statuses } from '$lib/stores/statuses';
  import { sessions, sessionLabel, type SessionStatus } from '$lib/stores/sessions';
  import { activeEntity } from '$lib/stores/activeEntity';
  import { spawnSession } from '$lib/stores/navigation';
  import { isPaletteChord } from '$lib/stores/ui';

  // Session state maps onto the shared server-state palette (mirrors the sidebar row).
  const SESSION_DOT: Record<SessionStatus, Status> = {
    connecting: 'unknown',
    connected: 'ok',
    failed: 'crit',
    unknown: 'unknown'
  };

  let inputEl = $state<HTMLInputElement>();
  let listEl = $state<HTMLUListElement>();
  let query = $state('');
  let selected = $state(0);

  const items = $derived(paletteItems($palette.mode, $hosts, $sessions, query));
  const firstSession = $derived(items.findIndex((it) => it.kind === 'session'));
  const firstHost = $derived(items.findIndex((it) => it.kind === 'host'));

  const placeholder = $derived(
    $palette.mode === 'pickHost' ? 'Pick a host…' : 'Search hosts and sessions…'
  );
  const emptyMessage = $derived(
    $palette.mode === 'pickHost'
      ? query
        ? 'No matching hosts.'
        : 'No hosts configured.'
      : query
        ? 'No matches.'
        : 'No hosts or sessions yet.'
  );

  // Fresh query + top selection, and focus the input, each time the overlay opens or
  // switches mode.
  $effect(() => {
    if ($palette.open) {
      query = '';
      selected = 0;
      void tick().then(() => inputEl?.focus());
    }
  });

  // Keep the highlighted row visible as arrow-nav moves it.
  $effect(() => {
    listEl?.querySelector<HTMLElement>(`[data-index="${selected}"]`)?.scrollIntoView({
      block: 'nearest'
    });
  });

  function selectAt(i: number): void {
    const item = items[i];
    if (!item) return;
    if (item.kind === 'session') {
      activeEntity.activateSession(item.session.id);
      palette.close();
    } else if ($palette.mode === 'pickHost') {
      palette.choose(item.host);
    } else {
      // Navigator default action for a host: open a shell (the primary connect path).
      spawnSession('terminal', item.host.name);
      palette.close();
    }
  }

  function onKeydown(e: KeyboardEvent): void {
    if (!$palette.open) {
      if (isPaletteChord(e)) {
        e.preventDefault();
        palette.open();
      }
      return;
    }
    if (isPaletteChord(e)) {
      // ⌘K inside a picker returns to the navigator.
      e.preventDefault();
      palette.open();
      return;
    }
    switch (e.key) {
      case 'Escape':
        e.preventDefault();
        palette.close();
        break;
      case 'ArrowDown':
        e.preventDefault();
        selected = nextIndex(selected, 1, items.length);
        break;
      case 'ArrowUp':
        e.preventDefault();
        selected = nextIndex(selected, -1, items.length);
        break;
      case 'Home':
        e.preventDefault();
        selected = 0;
        break;
      case 'End':
        e.preventDefault();
        selected = items.length - 1;
        break;
      case 'Enter':
        e.preventDefault();
        selectAt(selected);
        break;
    }
  }

  const rowBase = 'flex w-full items-center gap-3 rounded-lg px-3 py-2 text-left text-sm transition';
  const rowState = (active: boolean): string =>
    active ? 'bg-accent text-accent-fg' : 'text-muted hover:bg-surface-inset hover:text-fg';
  const sectionHead = 'px-3 pb-1 pt-2 text-[11px] font-medium uppercase tracking-[0.18em] text-faint';
</script>

<svelte:window onkeydown={onKeydown} />

{#if $palette.open}
  <div
    class="fixed inset-0 z-50 flex items-start justify-center px-4 pt-[14vh]"
    role="dialog"
    aria-modal="true"
    aria-label={$palette.mode === 'pickHost' ? 'Pick a host' : 'Command palette'}
  >
    <button
      type="button"
      tabindex="-1"
      aria-label="Dismiss"
      class="absolute inset-0 bg-overlay"
      onclick={() => palette.close()}
    ></button>

    <div
      class="relative w-full max-w-xl overflow-hidden rounded-2xl border border-default bg-glass shadow-soft backdrop-blur-md"
    >
      <div class="flex items-center gap-3 border-b border-default px-4">
        <span class="text-faint"><Icon name="search" /></span>
        <input
          bind:this={inputEl}
          bind:value={query}
          oninput={() => (selected = 0)}
          type="text"
          {placeholder}
          aria-label={placeholder}
          autocomplete="off"
          spellcheck="false"
          class="w-full bg-transparent py-3.5 text-sm text-fg outline-none placeholder:text-faint"
        />
      </div>

      <ul bind:this={listEl} class="max-h-[min(24rem,50vh)] overflow-y-auto p-2">
        {#if items.length === 0}
          <li class="px-3 py-6 text-center text-sm text-muted">{emptyMessage}</li>
        {:else}
          {#each items as item, i (item.kind === 'session' ? `s${item.session.id}` : `h${item.host.name}`)}
            {#if $palette.mode === 'navigate' && i === firstSession}
              <li class={sectionHead}>Sessions</li>
            {/if}
            {#if $palette.mode === 'navigate' && i === firstHost}
              <li class={sectionHead}>Hosts</li>
            {/if}
            <li>
              <button
                type="button"
                data-index={i}
                aria-current={selected === i ? 'true' : undefined}
                class="{rowBase} {rowState(selected === i)}"
                onclick={() => selectAt(i)}
                onmousemove={() => (selected = i)}
              >
                {#if item.kind === 'session'}
                  <StatusDot status={SESSION_DOT[item.session.status]} />
                  <Icon name={item.session.kind} size={16} />
                  <span class="min-w-0 flex-1 truncate">{sessionLabel(item.session)}</span>
                {:else}
                  <StatusDot status={hostStatusDot($statuses.get(item.host.name))} />
                  <span class="min-w-0 flex-1 truncate font-medium">{item.host.name}</span>
                  <span class="shrink-0 truncate font-mono text-xs {selected === i ? '' : 'text-faint'}">
                    {item.host.user}@{item.host.hostname}
                  </span>
                {/if}
              </button>
            </li>
          {/each}
        {/if}
      </ul>

      <div
        class="flex items-center gap-4 border-t border-default px-4 py-2 font-mono text-[11px] text-faint"
      >
        <span>↑↓ navigate</span>
        <span>↵ select</span>
        <span>esc close</span>
      </div>
    </div>
  </div>
{/if}
