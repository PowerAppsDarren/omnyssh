//! Terminal session commands (tech-gui.md §4.2). Thin wrappers over the PTY manager
//! and session registry in `GuiState`. Output flows out-of-band on the per-session
//! raw `Channel` (§3.3/§3.6) — never as a command return or a global event; the
//! frontend-facing ids are the registry's public ids.

use tauri::ipc::Channel;
use tauri::State;

use crate::dto::TerminalBytes;
use crate::error::CommandError;
use crate::state::GuiState;

/// Open a terminal for `host_name`, streaming raw output into `on_output`. Returns
/// the public session id used by the write/resize/close commands (tech-gui.md §4.2).
#[tauri::command]
#[specta::specta]
pub fn terminal_open(
    state: State<'_, GuiState>,
    host_name: String,
    cols: u16,
    rows: u16,
    on_output: Channel<TerminalBytes>,
) -> Result<u64, CommandError> {
    state
        .open_terminal(&host_name, cols, rows, on_output)
        .map_err(|message| CommandError { message })
}

/// Send keystrokes / pasted bytes to a terminal (tech-gui.md §4.2). Input is
/// low-volume, so the ordinary `number[]` path is fine here (only output is raw).
#[tauri::command]
#[specta::specta]
pub fn terminal_write(
    state: State<'_, GuiState>,
    session_id: u64,
    data: Vec<u8>,
) -> Result<(), CommandError> {
    state.write_terminal(session_id, &data);
    Ok(())
}

/// Reflow a terminal to `cols` x `rows` (tech-gui.md §4.2).
#[tauri::command]
#[specta::specta]
pub fn terminal_resize(
    state: State<'_, GuiState>,
    session_id: u64,
    cols: u16,
    rows: u16,
) -> Result<(), CommandError> {
    state.resize_terminal(session_id, cols, rows);
    Ok(())
}

/// Close a terminal and its connection (tech-gui.md §4.2). The frontend has already
/// dropped the tab, so the resulting `PtyExited` emits no `terminal-exited` (§3.4).
#[tauri::command]
#[specta::specta]
pub fn terminal_close(state: State<'_, GuiState>, session_id: u64) -> Result<(), CommandError> {
    state.close_terminal(session_id);
    Ok(())
}
