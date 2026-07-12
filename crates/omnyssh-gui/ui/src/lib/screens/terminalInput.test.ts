import { describe, expect, it } from 'vitest';
import { chunkBytes, INPUT_CHUNK } from './terminalInput';

const seq = (n: number) => new Uint8Array(Array.from({ length: n }, (_, i) => i & 0xff));

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
