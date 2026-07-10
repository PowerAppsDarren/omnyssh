import type { Config } from 'tailwindcss';

// Brand tokens (tech-gui.md §5) land in Stage 0.3; Stage 0.1 wires Tailwind so
// the frontend builds and the shell renders.
export default {
  content: ['./src/**/*.{html,svelte,ts}'],
  theme: { extend: {} },
  plugins: []
} satisfies Config;
