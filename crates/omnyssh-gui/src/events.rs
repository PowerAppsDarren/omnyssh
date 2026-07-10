//! Typed IPC events (tech-gui.md §4.3). Each `tauri_specta::Event` derives its
//! wire name from the struct name (`HostsLoaded` -> "hosts-loaded",
//! `Error` -> "error"), so the frontend consumes them through the generated
//! `events` object.

use serde::{Deserialize, Serialize};

use crate::dto::HostDto;

/// Full host list broadcast. Emitted directly by whoever loaded it (startup
/// today, `reload_hosts` in Stage 1.2); the bridge does not map `HostsLoaded`.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, tauri_specta::Event)]
pub struct HostsLoaded(pub Vec<HostDto>);

/// A background error surfaced to the user.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, tauri_specta::Event)]
pub struct Error {
    pub message: String,
}
