// Terminal input is sent over `terminal_write` as a `number[]` (§4.2). A single huge
// paste would serialize as one giant array on the main thread and freeze the UI, so the
// view splits it into bounded chunks and awaits each (yielding between). This is the
// pure split; the view owns the ordered dispatch.

/** The per-write byte cap. Small enough that one chunk's `number[]` serialization is
 *  imperceptible, so a multi-MB paste streams without a visible stall (§9). */
export const INPUT_CHUNK = 8192;

/** Split `data` into <=`size` slices, in order. Empty input yields nothing; input at or
 *  below the cap yields a single slice (the ordinary keystroke path). */
export function chunkBytes(data: Uint8Array, size: number = INPUT_CHUNK): Uint8Array[] {
  if (data.length <= size) return data.length ? [data] : [];
  const chunks: Uint8Array[] = [];
  for (let i = 0; i < data.length; i += size) {
    chunks.push(data.subarray(i, i + size));
  }
  return chunks;
}
