import { beforeEach, describe, expect, it } from 'vitest';
import { get } from 'svelte/store';
import {
  dismissPassphrasePrompt,
  passphrasePrompt,
  reducePassphraseRequired
} from './passphrase';

describe('passphrase prompt store', () => {
  beforeEach(() => {
    passphrasePrompt.set(null);
  });

  it('opens a prompt when none is showing', () => {
    const next = reducePassphraseRequired(null, 'web-1', '/home/me/.ssh/id_ed25519');
    expect(next).toEqual({ hostName: 'web-1', keyPath: '/home/me/.ssh/id_ed25519' });
  });

  it('keeps the first prompt when another host needs a key', () => {
    const open = { hostName: 'web-1', keyPath: '/home/me/.ssh/id_ed25519' };
    expect(reducePassphraseRequired(open, 'db-1', '/home/me/.ssh/other')).toEqual(open);
  });

  it('dismisses the prompt', () => {
    passphrasePrompt.set({ hostName: 'web-1', keyPath: '/tmp/key' });
    dismissPassphrasePrompt();
    expect(get(passphrasePrompt)).toBeNull();
  });
});
