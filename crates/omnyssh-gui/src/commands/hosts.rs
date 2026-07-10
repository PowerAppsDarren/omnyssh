use crate::dto::HostDto;
use crate::error::CommandError;

/// Stage 0.2 proves the command pipe end-to-end. Real loading + caching from the
/// shared config lands in Stage 1.2 (tech-gui.md §4.2, §7).
#[tauri::command]
#[specta::specta]
pub fn list_hosts() -> Result<Vec<HostDto>, CommandError> {
    Ok(Vec::new())
}
