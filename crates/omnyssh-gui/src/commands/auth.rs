//! Unlock passphrase-protected identity files (in-memory cache only).

use tauri::State;

use crate::error::CommandError;
use crate::state::GuiState;

/// Decrypt `key_path` with `passphrase` and remember it for this process.
/// On success, SSH pollers waiting on that key retry immediately.
#[tauri::command]
#[specta::specta]
pub async fn unlock_identity(
    state: State<'_, GuiState>,
    key_path: String,
    passphrase: String,
) -> Result<(), CommandError> {
    tauri::async_runtime::spawn_blocking(move || {
        omnyssh_core::ssh::identity::unlock(&key_path, &passphrase)
    })
    .await
    .map_err(|e| CommandError {
        message: format!("unlock task failed: {e}"),
    })?
    .map_err(|e| CommandError {
        message: e.to_string(),
    })?;
    state.retry_connections();
    Ok(())
}
