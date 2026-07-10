import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

// Fixed dev port matches tauri.conf.json devUrl so `cargo tauri dev` finds it.
export default defineConfig({
  plugins: [sveltekit()],
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  build: { target: 'esnext' }
});
