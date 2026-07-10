<script lang="ts">
  // Brandbook button language (§04): pill shape, one decisive accent action, the
  // rest quiet. Colours are semantic tokens only, so both themes are covered.
  import type { Snippet } from 'svelte';

  type Variant = 'primary' | 'secondary' | 'ghost' | 'icon';

  let {
    variant = 'secondary',
    type = 'button',
    disabled = false,
    title,
    onclick,
    children
  }: {
    variant?: Variant;
    type?: 'button' | 'submit';
    disabled?: boolean;
    title?: string;
    onclick?: (event: MouseEvent) => void;
    children: Snippet;
  } = $props();

  const base =
    'inline-flex items-center justify-center gap-2 rounded-full text-sm font-medium transition ' +
    'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus ' +
    'disabled:cursor-not-allowed disabled:opacity-50';

  const variants: Record<Variant, string> = {
    primary: 'bg-accent text-accent-fg px-7 py-2.5 hover:opacity-90 active:scale-[0.98]',
    secondary: 'border border-strong text-fg px-7 py-2.5 hover:bg-accent hover:text-accent-fg',
    ghost: 'text-muted px-4 py-2.5 hover:bg-surface-inset hover:text-fg',
    icon: 'text-muted h-9 w-9 hover:bg-surface-inset hover:text-fg'
  };
</script>

<button {type} {title} {disabled} {onclick} aria-label={title} class="{base} {variants[variant]}">
  {@render children()}
</button>
