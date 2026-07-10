import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vitest/config';

// Store/router unit tests only — no SvelteKit plugin, no Tauri runtime. The
// $lib alias mirrors svelte.config.js so tests resolve it the same way.
export default defineConfig({
  resolve: {
    alias: { $lib: fileURLToPath(new URL('./src/lib', import.meta.url)) }
  },
  test: {
    environment: 'node',
    include: ['src/**/*.test.ts']
  }
});
