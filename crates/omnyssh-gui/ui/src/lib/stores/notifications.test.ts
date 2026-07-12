import { afterEach, describe, expect, it, vi } from 'vitest';
import { get } from 'svelte/store';
import { lastError, ERROR_TTL_MS } from './notifications';

describe('lastError — auto-clearing status-bar error', () => {
  afterEach(() => {
    lastError.set(null);
    vi.useRealTimers();
  });

  it('clears itself after the TTL so the bar does not pin a resolved error', () => {
    vi.useFakeTimers();
    lastError.set('permission denied');
    expect(get(lastError)).toBe('permission denied');
    vi.advanceTimersByTime(ERROR_TTL_MS - 1);
    expect(get(lastError)).toBe('permission denied');
    vi.advanceTimersByTime(1);
    expect(get(lastError)).toBeNull();
  });

  it('a fresh error restarts the window instead of inheriting the old deadline', () => {
    vi.useFakeTimers();
    lastError.set('first');
    vi.advanceTimersByTime(ERROR_TTL_MS - 100);
    lastError.set('second');
    // The old deadline would have fired here; the fresh one has not.
    vi.advanceTimersByTime(200);
    expect(get(lastError)).toBe('second');
    vi.advanceTimersByTime(ERROR_TTL_MS);
    expect(get(lastError)).toBeNull();
  });

  it('set(null) clears immediately and cancels the pending timer', () => {
    vi.useFakeTimers();
    lastError.set('boom');
    lastError.set(null);
    expect(get(lastError)).toBeNull();
    // No lingering timer resurrects a cleared error.
    vi.advanceTimersByTime(ERROR_TTL_MS * 2);
    expect(get(lastError)).toBeNull();
  });
});
