// Open a URL in the user's default browser via tauri-plugin-opener. Dynamically
// imported so Vitest / `vite preview` (no Tauri runtime) don't fail at load, matching
// the theme store's native-call pattern (tech-gui.md §5.1). Rejects on failure so the
// caller can surface it via the status bar.
export async function openExternal(url: string): Promise<void> {
  const { openUrl } = await import('@tauri-apps/plugin-opener');
  await openUrl(url);
}
