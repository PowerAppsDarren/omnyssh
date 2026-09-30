import { describe, expect, it, vi } from 'vitest';

// The About row (issue #125) is the one place the running build is named, and a bug
// report has to quote it. The value is Tauri's own `getVersion` — the crate's
// `CARGO_PKG_VERSION`, which is the same number `omny --version` prints — so nothing
// on the frontend can drift from the binary. The mock stands in for the native call.
const getVersion = vi.fn(async () => '1.1.4');
vi.mock('@tauri-apps/api/app', () => ({ getVersion: () => getVersion() }));

describe('installedVersion', () => {
  it('reports the version the native call returns', async () => {
    const { installedVersion } = await import('./appInfo');
    expect(await installedVersion()).toBe('1.1.4');
  });

  // The import is dynamic, so a failed call surfaces as a rejection the Settings screen
  // catches — it must not throw at module load, where there is no Tauri runtime at all
  // (Vitest, `vite preview`) and nothing could catch it.
  it('propagates a failed call instead of swallowing it', async () => {
    getVersion.mockRejectedValueOnce('no Tauri runtime');
    const { installedVersion } = await import('./appInfo');
    await expect(installedVersion()).rejects.toThrow();
  });
});
