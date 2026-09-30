// The running build's version, from Tauri's own `getVersion` (the `CARGO_PKG_VERSION`
// tauri-codegen bakes into the context — the same 1.1.4 the terminal app's
// `omny --version` prints, since both crates take the workspace version).
//
// `core:default` already carries `core:app:allow-version`, so this needs no new
// capability. Dynamically imported so Vitest / `vite preview` (no Tauri runtime)
// don't fail at load, matching the theme store's native-call pattern (tech-gui.md
// §5.1). Rejects off-runtime; the caller decides what an unreadable version means.
export async function installedVersion(): Promise<string> {
  const { getVersion } = await import('@tauri-apps/api/app');
  return getVersion();
}
