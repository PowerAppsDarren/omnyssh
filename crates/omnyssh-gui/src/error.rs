//! Command error carried across the IPC boundary (tech-gui.md §4.2). Commands
//! return `Result<T, CommandError>`; the frontend surfaces `message`.

use serde::Serialize;

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct CommandError {
    pub message: String,
}
