<script lang="ts">
  // A brief, unobtrusive support dialog opened from the sidebar footer's paper-plane
  // button. It says OmnySSH is free and open source with no plan to monetize it, then
  // offers the two ways to help: a GitHub star and the Telegram channel. Brandbook
  // language (§04) — a calm light-weight headline with a single bold emphasis, flat
  // hairline action rows, and a pill arrow that leans on hover. It is an overlay, never
  // the active entity, so it leaves the exactly-one-active invariant untouched (§2).
  import { Icon, type IconName } from '$lib/theme';
  import Logo from './Logo.svelte';
  import Modal from './Modal.svelte';
  import { support } from '$lib/stores/support';
  import { openExternal } from '$lib/ipc/openExternal';
  import { lastError } from '$lib/stores/notifications';

  type Link = { icon: IconName; title: string; locator: string; url: string };

  const links: Link[] = [
    {
      icon: 'star',
      title: 'Star it on GitHub',
      locator: 'github.com/timhartmann7/omnyssh',
      url: 'https://github.com/timhartmann7/omnyssh'
    },
    {
      icon: 'telegram',
      title: 'Follow on Telegram',
      locator: '@timhartmanndev',
      url: 'https://t.me/timhartmanndev'
    }
  ];

  async function go(url: string): Promise<void> {
    try {
      await openExternal(url);
    } catch (e) {
      lastError.set(e instanceof Error ? e.message : String(e));
    }
  }
</script>

<Modal label="Support OmnySSH" onClose={support.close}>
  <div class="relative p-6">
    <button
      type="button"
      class="absolute right-4 top-4 rounded-full p-1.5 text-muted transition hover:bg-surface-inset hover:text-fg focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
      title="Close"
      aria-label="Close"
      onclick={support.close}
    >
      <Icon name="close" size={16} />
    </button>

    <div class="mb-5 flex items-center gap-2.5">
      <Logo size={20} />
      <span class="text-sm font-bold tracking-wide">OmnySSH</span>
    </div>

    <h2 class="text-xl font-light leading-snug">
      Free and <span class="font-bold">open source</span>, forever.
    </h2>
    <p class="mt-3 text-sm leading-relaxed text-muted">
      No paid tiers, no ads, no upsells. I build OmnySSH in the open and I do not plan to
      monetize it.
    </p>

    <p class="mt-5 text-[11px] font-medium uppercase tracking-[0.18em] text-faint">
      Two small things help it grow
    </p>

    <div class="mt-2.5 space-y-2.5">
      {#each links as link (link.url)}
        <button
          type="button"
          class="group flex w-full items-center gap-4 rounded-xl border border-default bg-surface px-4 py-3 text-left transition hover:border-strong hover:bg-surface-inset focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
          onclick={() => go(link.url)}
        >
          <span
            class="grid h-10 w-10 shrink-0 place-items-center rounded-full border border-default text-fg transition group-hover:border-strong"
          >
            <Icon name={link.icon} />
          </span>
          <span class="min-w-0 flex-1">
            <span class="block text-sm font-medium">{link.title}</span>
            <span class="block truncate font-mono text-xs text-muted">{link.locator}</span>
          </span>
          <span
            class="shrink-0 text-muted transition-transform group-hover:translate-x-0.5"
            aria-hidden="true">→</span
          >
        </button>
      {/each}
    </div>

    <p class="mt-4 text-xs leading-relaxed text-faint">
      Telegram is where I post release notes and the rest of what I build.
    </p>
  </div>
</Modal>
