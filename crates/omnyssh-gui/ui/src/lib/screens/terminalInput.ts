// Terminal input is sent over `terminal_write` as a `number[]` (§4.2). A single huge
// paste would serialize as one giant array on the main thread and freeze the UI, so the
// view splits it into bounded chunks and awaits each (yielding between). This is the
// pure split; the view owns the ordered dispatch. The copy shortcut is decided here too,
// before xterm turns the key into input.

/** The per-write byte cap. Small enough that one chunk's `number[]` serialization is
 *  imperceptible, so a multi-MB paste streams without a visible stall (§9). */
export const INPUT_CHUNK = 8192;

/** The fields of a key event the copy shortcut reads, so tests can pass plain objects. */
export type KeyPress = Pick<
  KeyboardEvent,
  | 'type'
  | 'key'
  | 'code'
  | 'keyCode'
  | 'ctrlKey'
  | 'shiftKey'
  | 'altKey'
  | 'metaKey'
  | 'isComposing'
>;

/** Ctrl+Shift+C on Windows and Linux, as in GNOME Terminal and Windows Terminal: a bare
 *  Ctrl+C has to stay ^C. macOS needs none — Cmd+C copies there through the Edit menu.
 *  Ctrl+Shift+V needs no twin — xterm makes no input of it, so the webview pastes
 *  natively. */
export function isCopyShortcut(e: KeyPress, mac: boolean): boolean {
  // keyCode 229 marks the keydown that starts an IME composition.
  if (mac || e.type !== 'keydown' || e.isComposing || e.keyCode === 229) return false;
  if (e.altKey || e.metaKey || !e.ctrlKey || !e.shiftKey) return false;
  // The physical key stands in only where the layout puts no Latin letter on it, so
  // a Cyrillic layout copies with the same keys while Dvorak's J there stays a J.
  return e.key === 'c' || e.key === 'C' || (e.code === 'KeyC' && !/^[a-z]$/i.test(e.key));
}

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
