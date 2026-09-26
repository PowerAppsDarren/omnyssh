import { describe, expect, it } from 'vitest';
import { chunkBytes, INPUT_CHUNK, isCopyShortcut, type KeyPress } from './terminalInput';

const seq = (n: number) => new Uint8Array(Array.from({ length: n }, (_, i) => i & 0xff));

const press = (over: Partial<KeyPress>): KeyPress => ({
  type: 'keydown',
  key: 'C',
  code: 'KeyC',
  keyCode: 67,
  ctrlKey: true,
  shiftKey: true,
  altKey: false,
  metaKey: false,
  isComposing: false,
  ...over
});

describe('isCopyShortcut — Ctrl+Shift+C copies on Windows and Linux', () => {
  it('copies on Ctrl+Shift+C, whichever case the key reports', () => {
    expect(isCopyShortcut(press({}), false)).toBe(true);
    expect(isCopyShortcut(press({ key: 'c' }), false)).toBe(true);
  });

  it('copies on the same physical key under a non-Latin layout', () => {
    // What the C key types on a Russian layout, with the keyCode WebKitGTK gives it.
    expect(isCopyShortcut(press({ key: '\u0421', keyCode: 0 }), false)).toBe(true);
  });

  it('follows the letter, not the key, on a Latin layout like Dvorak', () => {
    expect(isCopyShortcut(press({ key: 'J', code: 'KeyC' }), false)).toBe(false);
    expect(isCopyShortcut(press({ key: 'C', code: 'KeyI' }), false)).toBe(true);
  });

  it('leaves bare Ctrl+C to the shell as ^C', () => {
    expect(isCopyShortcut(press({ shiftKey: false, key: 'c' }), false)).toBe(false);
  });

  it('leaves Ctrl+Shift+V, other keys and extra modifiers alone', () => {
    expect(isCopyShortcut(press({ key: 'V', code: 'KeyV' }), false)).toBe(false);
    expect(isCopyShortcut(press({ altKey: true }), false)).toBe(false);
    expect(isCopyShortcut(press({ metaKey: true }), false)).toBe(false);
  });

  it('acts on keydown only, and never mid-composition', () => {
    expect(isCopyShortcut(press({ type: 'keyup' }), false)).toBe(false);
    expect(isCopyShortcut(press({ type: 'keypress' }), false)).toBe(false);
    expect(isCopyShortcut(press({ isComposing: true }), false)).toBe(false);
    expect(isCopyShortcut(press({ key: 'Process', keyCode: 229 }), false)).toBe(false);
  });

  it('does nothing on macOS, where Cmd+C already copies', () => {
    expect(isCopyShortcut(press({}), true)).toBe(false);
  });
});

describe('chunkBytes — bounded terminal input', () => {
  it('yields nothing for empty input', () => {
    expect(chunkBytes(new Uint8Array(0))).toEqual([]);
  });

  it('yields a single slice for input at or below the cap (the keystroke path)', () => {
    expect(chunkBytes(seq(1), 8)).toHaveLength(1);
    expect(chunkBytes(seq(8), 8)).toHaveLength(1);
    expect(chunkBytes(seq(8), 8)[0]).toHaveLength(8);
  });

  it('splits a large paste into ordered <=size slices that reassemble to the input', () => {
    const data = seq(21);
    const chunks = chunkBytes(data, 8);
    expect(chunks.map((c) => c.length)).toEqual([8, 8, 5]);
    expect(chunks.every((c) => c.length <= 8)).toBe(true);
    expect(new Uint8Array(chunks.flatMap((c) => [...c]))).toEqual(data);
  });

  it('splits an exact multiple into full slices only', () => {
    expect(chunkBytes(seq(16), 8).map((c) => c.length)).toEqual([8, 8]);
  });

  it('defaults to the INPUT_CHUNK cap', () => {
    expect(chunkBytes(seq(INPUT_CHUNK + 1)).map((c) => c.length)).toEqual([INPUT_CHUNK, 1]);
  });
});
