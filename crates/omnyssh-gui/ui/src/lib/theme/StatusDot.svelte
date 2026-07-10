<script lang="ts">
  // Server-state indicator. Colour comes from a semantic token, so the dot follows
  // the theme and never hardcodes a hue (tech-gui.md §5.1).
  import { statusToken, type Status } from './status';

  let {
    status,
    size = 8,
    label
  }: { status: Status; size?: number; label?: string } = $props();
</script>

<span
  class="inline-block shrink-0 rounded-full"
  role="img"
  aria-label={label ?? status}
  style="width: {size}px; height: {size}px; background-color: {statusToken(status)};"
></span>
