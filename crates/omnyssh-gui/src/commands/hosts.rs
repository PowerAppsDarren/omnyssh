use tauri::{AppHandle, State};
use tauri_specta::Event;

use crate::dto::HostDto;
use crate::error::CommandError;
use crate::events;
use crate::state::GuiState;

/// Return the cached host list (populated at startup / on reload, tech-gui.md §4.2).
#[tauri::command]
#[specta::specta]
pub fn list_hosts(state: State<'_, GuiState>) -> Result<Vec<HostDto>, CommandError> {
    Ok(state.host_dtos())
}

/// Reload hosts from the shared config, refresh the cache, restart the pollers,
/// and broadcast the new list via `hosts-loaded` (tech-gui.md §4.2). Also the
/// startup entry point: the frontend calls it once its event bridge is up.
#[tauri::command]
#[specta::specta]
pub async fn reload_hosts(app: AppHandle, state: State<'_, GuiState>) -> Result<(), CommandError> {
    // Parsing hosts.toml + ~/.ssh/config is blocking I/O — keep it off the async
    // worker so in-flight commands and the event bridge don't stall.
    let hosts = tauri::async_runtime::spawn_blocking(omnyssh_core::config::load_all_hosts)
        .await
        .map_err(|e| CommandError {
            message: format!("host load task failed: {e}"),
        })?
        .map_err(|e| CommandError {
            message: e.to_string(),
        })?;
    state.set_hosts(hosts);
    state.restart_pollers();
    let _ = events::HostsLoaded(state.host_dtos()).emit(&app);
    Ok(())
}
