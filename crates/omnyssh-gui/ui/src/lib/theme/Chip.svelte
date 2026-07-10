<script lang="ts">
  // Brandbook pill/tag (§04). Semantic tokens only.
  import type { Snippet } from 'svelte';

  type Variant = 'default' | 'solid' | 'outline';

  let { variant = 'default', children }: { variant?: Variant; children: Snippet } = $props();

  const variants: Record<Variant, string> = {
    default: 'bg-surface-inset text-fg',
    solid: 'bg-accent text-accent-fg',
    outline: 'border border-default text-muted'
  };
</script>

<span class="inline-flex items-center rounded-full px-3 py-1 text-xs font-medium {variants[variant]}">
  {@render children()}
</span>
