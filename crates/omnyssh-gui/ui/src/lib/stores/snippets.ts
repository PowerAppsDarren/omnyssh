import { writable } from 'svelte/store';
import type { SnippetDto, SnippetResult } from '$lib/bindings';

// Saved snippets, mirroring `snippets.toml` (tech-gui.md §2.2). Refreshed from
// `list_snippets` after every mutation so the store never drifts from the shared
// on-disk source of truth (the same file the TUI reads/writes).
export const snippets = writable<SnippetDto[]>([]);

/** One target host's slot in a running snippet's results panel. */
export interface SnippetResultEntry {
  hostName: string;
  /** True until the host's `snippet-result` arrives. */
  pending: boolean;
  ok: boolean;
  output: string;
}

/** The in-flight (or just-finished) execution the results panel renders. */
export interface SnippetRun {
  snippetName: string;
  entries: SnippetResultEntry[];
}

// The active execution, or null when no results panel is open (tech-gui.md §2.2).
export const snippetRun = writable<SnippetRun | null>(null);

/** Seed a run with one pending entry per target host — called the moment execute
 *  fires, so every host shows as pending before its result streams back. */
export function beginRun(snippetName: string, hostNames: string[]): void {
  snippetRun.set({
    snippetName,
    entries: hostNames.map((hostName) => ({ hostName, pending: true, ok: false, output: '' }))
  });
}

/** Dismiss the results panel. */
export function clearRun(): void {
  snippetRun.set(null);
}

/** Fold a `snippet-result` into the active run, filling the matching host's entry.
 *  A result for a different snippet (name mismatch) or an unknown host is ignored,
 *  so a late event from a prior execution can't corrupt the current panel. Pure, so
 *  the router and the tests share one definition. */
export function reduceRunResult(run: SnippetRun | null, payload: SnippetResult): SnippetRun | null {
  if (!run || run.snippetName !== payload.snippetName) return run;
  return {
    ...run,
    entries: run.entries.map((e) =>
      e.hostName === payload.hostName
        ? { hostName: e.hostName, pending: false, ok: payload.ok, output: payload.output }
        : e
    )
  };
}
