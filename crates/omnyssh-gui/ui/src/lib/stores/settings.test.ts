import { beforeEach, describe, expect, it, vi } from 'vitest';
import { clampInterval, driveMetricsRefresh, refreshInterval } from './settings';

describe('clampInterval', () => {
  it('accepts positive numbers and rounds them', () => {
    expect(clampInterval(30)).toBe(30);
    expect(clampInterval('60')).toBe(60);
    expect(clampInterval(12.7)).toBe(13);
  });

  it('falls back to the default for junk or non-positive values', () => {
    expect(clampInterval('abc')).toBe(30);
    expect(clampInterval(0)).toBe(30);
    expect(clampInterval(-5)).toBe(30);
    expect(clampInterval(Number.NaN)).toBe(30);
  });
});

describe('driveMetricsRefresh', () => {
  beforeEach(() => refreshInterval.set(30));

  it('arms on the current interval, re-arms when it changes, and disposes cleanly', () => {
    const refresh = vi.fn();
    let nextId = 1;
    const cleared: number[] = [];
    const armed: Array<{ ms: number; id: number }> = [];
    const fakeSet = ((_fn: () => void, ms?: number) => {
      const id = nextId++;
      armed.push({ ms: ms ?? 0, id });
      return id;
    }) as unknown as typeof setInterval;
    const fakeClear = ((id?: number) => {
      if (id !== undefined) cleared.push(id);
    }) as unknown as typeof clearInterval;

    refreshInterval.set(30);
    const stop = driveMetricsRefresh(refresh, fakeSet, fakeClear);
    // Subscribing arms immediately at the current interval.
    expect(armed.at(-1)?.ms).toBe(30_000);
    const firstTimer = armed.at(-1)!.id;

    // Changing the interval clears the old timer and re-arms at the new one.
    refreshInterval.set(10);
    expect(cleared).toContain(firstTimer);
    expect(armed.at(-1)?.ms).toBe(10_000);

    // Disposing clears the live timer.
    const lastTimer = armed.at(-1)!.id;
    stop();
    expect(cleared).toContain(lastTimer);
  });

  it('invokes the provided refresh callback as the timer body', () => {
    const refresh = vi.fn();
    let body: (() => void) | undefined;
    const fakeSet = ((fn: () => void) => {
      body = fn;
      return 1;
    }) as unknown as typeof setInterval;
    const fakeClear = (() => {}) as unknown as typeof clearInterval;

    const stop = driveMetricsRefresh(refresh, fakeSet, fakeClear);
    body?.();
    expect(refresh).toHaveBeenCalledTimes(1);
    stop();
  });
});
