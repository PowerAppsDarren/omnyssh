<script lang="ts">
  // Panel container — brandbook default is flat with a hairline border; `raised`
  // adds the single soft elevation reserved for floating elements (§05).
  import type { Snippet } from 'svelte';

  type Variant = 'default' | 'raised' | 'inset';

  let {
    variant = 'default',
    class: klass = '',
    children
  }: { variant?: Variant; class?: string; children: Snippet } = $props();

  const variants: Record<Variant, string> = {
    default: 'bg-surface border border-default',
    raised: 'bg-surface-raised border border-default shadow-soft',
    inset: 'bg-surface-inset'
  };
</script>

<div class="rounded-2xl {variants[variant]} {klass}">
  {@render children()}
</div>
