// Single bootstrap subscribing every backend event into the stores
// (tech-gui.md §3.5). Returns a disposer that drops all listeners; if any
// listen fails, already-registered listeners are disposed so none leak.

import type { UnlistenFn } from '@tauri-apps/api/event';
import { events } from '$lib/bindings';
import { applyError, applyHostsLoaded } from './router';

export async function startEventBridge(): Promise<() => void> {
  const offs: UnlistenFn[] = [];
  try {
    offs.push(await events.hostsLoaded.listen((e) => applyHostsLoaded(e.payload)));
    offs.push(await events.error.listen((e) => applyError(e.payload.message)));
  } catch (err) {
    offs.forEach((off) => off());
    throw err;
  }
  return () => offs.forEach((off) => off());
}
