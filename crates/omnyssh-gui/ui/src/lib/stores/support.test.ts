import { describe, expect, it } from 'vitest';
import { get } from 'svelte/store';
import { support } from './support';

// The support overlay is a simple open/close flag; assert it starts closed and toggles
// so the footer button and modal stay in sync.
describe('support store', () => {
  it('starts closed', () => {
    expect(get(support)).toBe(false);
  });

  it('open() and close() flip the flag', () => {
    support.open();
    expect(get(support)).toBe(true);
    support.close();
    expect(get(support)).toBe(false);
  });
});
