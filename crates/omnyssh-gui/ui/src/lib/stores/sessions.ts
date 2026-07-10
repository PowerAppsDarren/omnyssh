import { writable } from 'svelte/store';

// The open terminal/SFTP tabs (tech-gui.md §2, §3.5). Spawners append a row here;
// Stage 3 makes the sessions real (live PTY / SFTP). Ids come from one monotonic
// space so a closed tab's id is never reused and terminal/SFTP ids never collide in
// the frontend.
export type SessionKind = 'terminal' | 'sftp';
export type SessionStatus = 'connecting' | 'connected' | 'failed' | 'unknown';

export interface Session {
  id: number;
  kind: SessionKind;
  hostName: string;
  status: SessionStatus;
}

/** How a session is labelled wherever it is shown (sidebar row + Content heading),
 *  kept next to the type so both surfaces stay in step. */
export function sessionLabel(s: Session): string {
  return `${s.hostName} · ${s.kind}`;
}

function createSessions() {
  const { subscribe, update } = writable<Session[]>([]);
  let nextId = 1;
  return {
    subscribe,
    spawn(kind: SessionKind, hostName: string): Session {
      const session: Session = { id: nextId++, kind, hostName, status: 'connecting' };
      update((list) => [...list, session]);
      return session;
    },
    close(id: number): void {
      update((list) => list.filter((s) => s.id !== id));
    }
  };
}

export const sessions = createSessions();
