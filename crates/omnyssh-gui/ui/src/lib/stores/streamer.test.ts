// @vitest-environment jsdom
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { get } from 'svelte/store';
import { maskHostname, displayHostname } from './streamer';

// Streamer mode disguises host addresses on screen (tech-gui.md §4.3). The mask must be
// deterministic (same host → same disguise all recording), never leak the real value,
// and keep the address shape so cards still read like real servers.
describe('address masking', () => {
  it('is deterministic for a given host', () => {
    expect(maskHostname('203.0.113.7')).toBe(maskHostname('203.0.113.7'));
    expect(maskHostname('db.example.com')).toBe(maskHostname('db.example.com'));
  });

  it('maps an IPv4 to a different, valid public-looking IPv4', () => {
    const masked = maskHostname('192.168.1.10');
    expect(masked).not.toBe('192.168.1.10');
    const octets = masked.split('.').map(Number);
    expect(octets).toHaveLength(4);
    expect(octets.every((o) => o >= 0 && o <= 255)).toBe(true);
    expect(octets[3]).toBeGreaterThanOrEqual(1);
    expect(octets[3]).toBeLessThanOrEqual(254);
  });

  it('maps a domain to a fake domain that keeps the TLD', () => {
    const masked = maskHostname('prod.internal.example.com');
    expect(masked).not.toBe('prod.internal.example.com');
    expect(masked.endsWith('.com')).toBe(true);
  });

  it('maps an IPv6 to a fake IPv6', () => {
    const masked = maskHostname('2001:db8::1');
    expect(masked).not.toBe('2001:db8::1');
    expect(masked.includes(':')).toBe(true);
  });

  it('gives distinct hosts distinct disguises', () => {
    expect(maskHostname('10.0.0.1')).not.toBe(maskHostname('10.0.0.2'));
  });

  it('displayHostname passes through when streamer mode is off', () => {
    expect(displayHostname('203.0.113.7', false)).toBe('203.0.113.7');
    expect(displayHostname('203.0.113.7', true)).toBe(maskHostname('203.0.113.7'));
  });
});

// Persistence mirrors the sidebar-collapse pref: a fake Tauri store, a fresh module per
// test to reset the singleton, and the localStorage mirror seeding the initial value.
const backend = { get: vi.fn(), set: vi.fn(), save: vi.fn() };
vi.mock('@tauri-apps/plugin-store', () => ({ load: vi.fn(async () => backend) }));

async function fresh() {
  vi.resetModules();
  return (await import('./streamer')).streamerMode;
}

describe('streamer mode persistence', () => {
  beforeEach(() => {
    localStorage.clear();
    backend.get.mockReset();
    backend.set.mockReset().mockResolvedValue(undefined);
    backend.save.mockReset().mockResolvedValue(undefined);
  });

  it('defaults to off and mirrors a toggle to localStorage', async () => {
    const streamerMode = await fresh();
    expect(get(streamerMode)).toBe(false);
    streamerMode.toggle();
    expect(get(streamerMode)).toBe(true);
    expect(localStorage.getItem('omnyssh-streamer-mode')).toBe('true');
  });

  it('writes the canonical tauri-plugin-store on a user flip', async () => {
    const streamerMode = await fresh();
    streamerMode.set(true);
    await vi.waitFor(() => {
      expect(backend.set).toHaveBeenCalledWith('streamerMode', true);
      expect(backend.save).toHaveBeenCalled();
    });
  });

  it('hydrate applies the stored value without clobbering a fresh user flip', async () => {
    backend.get.mockResolvedValue(false); // stale persisted value
    const streamerMode = await fresh();
    streamerMode.set(true); // user acts before hydrate resolves
    await streamerMode.hydrate();
    expect(get(streamerMode)).toBe(true);
  });
});
