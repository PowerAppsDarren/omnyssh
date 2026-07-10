import type { Config } from 'tailwindcss';

// Colour utilities resolve to the semantic CSS variables defined in app.css
// (tech-gui.md §5.1). The default palette is replaced, not extended, so a stray
// `bg-red-500` or literal hex simply has no class — token discipline is
// structural, not a lint afterthought. Values live once in app.css; a theme
// switch swaps them.
export default {
  content: ['./src/**/*.{html,svelte,ts}'],
  theme: {
    colors: {
      transparent: 'transparent',
      current: 'currentColor',
      inherit: 'inherit',
      bg: 'var(--bg)',
      surface: 'var(--surface)',
      'surface-raised': 'var(--surface-raised)',
      'surface-inset': 'var(--surface-inset)',
      fg: 'var(--text)',
      muted: 'var(--text-muted)',
      faint: 'var(--text-faint)',
      default: 'var(--border)',
      strong: 'var(--border-strong)',
      accent: 'var(--accent)',
      'accent-fg': 'var(--accent-fg)',
      focus: 'var(--focus-ring)',
      glass: 'var(--glass)',
      overlay: 'var(--overlay)',
      'status-ok': 'var(--status-ok)',
      'status-warn': 'var(--status-warn)',
      'status-crit': 'var(--status-crit)',
      'status-off': 'var(--status-off)'
    },
    extend: {
      fontFamily: {
        sans: ['Helvetica Neue', 'Helvetica', 'Arial', 'sans-serif'],
        mono: ['ui-monospace', 'SFMono-Regular', 'Menlo', 'Monaco', 'Consolas', 'monospace']
      },
      // Bare `border` resolves to the token; preflight's universal border-color
      // follows too (tech-gui.md §5.1).
      borderColor: { DEFAULT: 'var(--border)' },
      // The one soft elevation for floating elements (brandbook §05 Elevation).
      boxShadow: { soft: '0 10px 30px -12px rgba(33, 33, 33, 0.18)' }
    }
  },
  plugins: []
} satisfies Config;
