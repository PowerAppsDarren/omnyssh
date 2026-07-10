<script lang="ts">
  // OmnySSH mark. The logo is a single-colour vector, so it is rendered via a CSS
  // mask and tinted from a theme token — it stays legible on both light and dark
  // (ink on light, paper on dark) and never disappears into a same-colour surface.
  export let size = 24;
  export let color = 'var(--accent)'; // any theme token: --accent | --text | ...
  export let title = 'OmnySSH';
</script>

<span
  class="logo"
  role="img"
  aria-label={title}
  style="--logo-size:{size}px; --logo-color:{color};"
></span>

<style>
  .logo {
    display: inline-block;
    width: var(--logo-size);
    height: var(--logo-size);
    background-color: var(--logo-color);
    -webkit-mask: url(/logo.svg) center / contain no-repeat;
    mask: url(/logo.svg) center / contain no-repeat;
  }
</style>
