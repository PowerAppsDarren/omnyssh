//! Typed IPC events (tech-gui.md §4.3). Each `tauri_specta::Event` derives its
//! wire name from the struct name (`HostsLoaded` -> "hosts-loaded",
//! `MetricsUpdated` -> "metrics-updated"), so the frontend consumes them through
//! the generated `events` object.

use serde::{Deserialize, Serialize};

use crate::dto::{ConnectionStatusDto, HostDto, MetricsDto, ServiceDto};

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

/// Services detected on a host by the discovery quick-scan (tech-gui.md §4.3).
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct ServicesDetected {
    pub host_name: String,
    pub services: Vec<ServiceDto>,
}

/// Discovery failed for a host (tech-gui.md §4.3).
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct ServicesFailed {
    pub host_name: String,
    pub message: String,
}

/// Result of running a snippet on one host (tech-gui.md §4.3). Emitted directly by
/// `execute_snippet` per host (one-shot `SshSession::run_command`), not via the
/// shared bridge — the same "the command owns the result" pattern the SFTP
/// per-session forwarder uses (§3.4). The core's `Result<String, String>` is
/// flattened to `ok` + `output` (stdout on success, the error message on failure).
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct SnippetResult {
    pub host_name: String,
    pub snippet_name: String,
    pub ok: bool,
    pub output: String,
}

/// A terminal session's remote shell exited or its connection dropped (tech-gui.md
/// §4.3). Carries the **public** registry id (the bridge maps the core's inner PTY
/// id, §3.4); the frontend tears the tab down. User-initiated closes never emit this.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct TerminalExited {
    pub session_id: u64,
}

/// A background error surfaced to the user.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, tauri_specta::Event)]
pub struct Error {
    pub message: String,
}
