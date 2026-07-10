//! Typed IPC events (tech-gui.md §4.3). Each `tauri_specta::Event` derives its
//! wire name from the struct name (`HostsLoaded` -> "hosts-loaded",
//! `MetricsUpdated` -> "metrics-updated"), so the frontend consumes them through
//! the generated `events` object.

use serde::{Deserialize, Serialize};

use crate::dto::{ConnectionStatusDto, HostDto, MetricsDto};

/// Full host list broadcast. Emitted by `reload_hosts` after refreshing the
/// cache; the bridge does not map `HostsLoaded` (tech-gui.md §3.4).
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, tauri_specta::Event)]
pub struct HostsLoaded(pub Vec<HostDto>);

/// A host's connection status changed (tech-gui.md §4.3).
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct HostStatusChanged {
    pub host_name: String,
    pub status: ConnectionStatusDto,
}

/// A fresh metrics sample for a host (tech-gui.md §4.3).
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct MetricsUpdated {
    pub host_name: String,
    pub metrics: MetricsDto,
}

/// A background error surfaced to the user.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, tauri_specta::Event)]
pub struct Error {
    pub message: String,
}
