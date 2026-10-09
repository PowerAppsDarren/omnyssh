import { writable } from 'svelte/store';
import type { KeySetupStepDto } from '$lib/bindings';

// Auto SSH key-setup progress (tech-gui.md §4.2). One active run at a time, mirroring
// the TUI's single-popup model — a new run replaces any prior one. The reducers are
// pure so the ipc router and the tests share one definition; the terminal phases
// (complete / failed / rolledBack) keep the panel up until the user dismisses it.

/** The phase a run is in: streaming steps, or one of the three terminal outcomes. */
export type KeySetupPhase =
  | { kind: 'running'; step: KeySetupStepDto | null }
  | { kind: 'complete'; keyPath: string; passwordOff: boolean }
  | { kind: 'failed'; error: string; passwordMayBeOff: boolean }
  | { kind: 'rolledBack'; result: string };

export interface KeySetupRun {
  hostName: string;
  phase: KeySetupPhase;
}

// The active run, or null when no progress panel is shown.
export const keySetup = writable<KeySetupRun | null>(null);

/** Open the panel for `hostName` in the initial running state (the step is not known
 *  until the first `key-setup-progress` arrives — the backend is still connecting).
 *  Called the moment `start_key_setup` fires. */
export function beginKeySetup(hostName: string): void {
  keySetup.set({ hostName, phase: { kind: 'running', step: null } });
}

/** Dismiss the panel. The background task keeps running if it was mid-flight; a later
 *  terminal event re-opens the panel on its outcome (and drives the card refresh). */
export function dismissKeySetup(): void {
  keySetup.set(null);
}

/** Fold a `key-setup-progress` event into the active run. Only the active host's
 *  running run advances — a stray progress for another host is ignored (single active
 *  run), and an outcome is never turned back into a running panel. Pure. */
export function reduceProgress(
  run: KeySetupRun | null,
  hostName: string,
  step: KeySetupStepDto
): KeySetupRun | null {
  if (run?.hostName !== hostName || run.phase.kind !== 'running') return run;
  return { hostName, phase: { kind: 'running', step } };
}

/** A terminal outcome always shows for its host, even if the running panel was
 *  dismissed — so the result (and, for `complete`, the card refresh) is never missed.
 *  Pure. */
export function reduceComplete(hostName: string, keyPath: string, passwordOff: boolean): KeySetupRun {
  return { hostName, phase: { kind: 'complete', keyPath, passwordOff } };
}

export function reduceFailed(hostName: string, error: string, passwordMayBeOff: boolean): KeySetupRun {
  return { hostName, phase: { kind: 'failed', error, passwordMayBeOff } };
}

export function reduceRollback(hostName: string, result: string): KeySetupRun {
  return { hostName, phase: { kind: 'rolledBack', result } };
}
