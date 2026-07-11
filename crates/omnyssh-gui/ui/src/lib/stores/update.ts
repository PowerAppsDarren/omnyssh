import { writable } from 'svelte/store';
import type { UpdateInfoDto } from '$lib/bindings';

// The update the banner offers, or null (up to date / dismissed / skipped). Set by the
// `update-available` startup event and by a manual `check_update` from Settings
// (tech-gui.md §4.3). The banner (AppShell) renders whenever this is non-null.
export const availableUpdate = writable<UpdateInfoDto | null>(null);

/** Show an available update (from the event or a manual check). */
export function offerUpdate(info: UpdateInfoDto): void {
  availableUpdate.set(info);
}

/** Dismiss the banner for this session (no persistence — it returns next launch). */
export function dismissUpdate(): void {
  availableUpdate.set(null);
}
