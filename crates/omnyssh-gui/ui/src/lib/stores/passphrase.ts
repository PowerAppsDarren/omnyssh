import { writable } from 'svelte/store';

// One in-memory passphrase prompt at a time. A second host that shares the same
// key is ignored until this one is dismissed; unlocking caches the passphrase
// for every host that uses the file.

export interface PassphrasePrompt {
  hostName: string;
  keyPath: string;
}

export const passphrasePrompt = writable<PassphrasePrompt | null>(null);

/** Open the prompt unless one is already showing. Pure so the router can test it. */
export function reducePassphraseRequired(
  current: PassphrasePrompt | null,
  hostName: string,
  keyPath: string
): PassphrasePrompt | null {
  return current ?? { hostName, keyPath };
}

export function applyKeyPassphraseRequired(payload: PassphrasePrompt): void {
  passphrasePrompt.update((current) =>
    reducePassphraseRequired(current, payload.hostName, payload.keyPath)
  );
}

export function dismissPassphrasePrompt(): void {
  passphrasePrompt.set(null);
}
