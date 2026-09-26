//! Unlock passphrase-protected identity files (in-memory cache only).

use crate::error::CommandError;

/// Decrypt `key_path` with `passphrase` and remember it for this process. Only a
/// key the core reported in `key-passphrase-required` is accepted; connections
/// waiting on it retry at once.
#[tauri::command]
#[specta::specta]
pub async fn unlock_identity(key_path: String, passphrase: String) -> Result<(), CommandError> {
    tauri::async_runtime::spawn_blocking(move || {
        omnyssh_core::ssh::identity::unlock(&key_path, &passphrase)
    })
    .await
    .map_err(|e| CommandError {
        message: format!("unlock task failed: {e}"),
    })?
    .map_err(|e| CommandError {
        message: e.to_string(),
    })
}
