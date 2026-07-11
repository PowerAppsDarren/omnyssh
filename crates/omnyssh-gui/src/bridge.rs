//! Core-event bridge (tech-gui.md §3.4): one long-lived task drains the shared
//! engine channel and maps each `CoreEvent` to a typed IPC event. Status, metrics,
//! discovered services and PTY-exit land here; the raw PTY byte stream rides its own
//! forwarder (`forward_terminal_output`) into the per-session channels (§3.6).

use omnyssh_core::event::{CoreEvent, SessionId};
use tauri::{AppHandle, Manager};
use tauri_specta::Event;
use tokio::sync::mpsc;

use crate::events;
use crate::state::GuiState;

pub async fn forward_core_events(app: AppHandle, mut rx: mpsc::Receiver<CoreEvent>) {
    while let Some(event) = rx.recv().await {
        // A match with an explicit ignore arm (§3.4), grown one variant per slice.
        match event {
            CoreEvent::HostStatusChanged(host_name, status) => {
                let _ = events::HostStatusChanged {
                    host_name,
                    status: (&status).into(),
                }
                .emit(&app);
            }
            CoreEvent::MetricsUpdate(host_name, metrics) => {
                let _ = events::MetricsUpdated {
                    host_name,
                    metrics: (&metrics).into(),
                }
                .emit(&app);
            }
            CoreEvent::DiscoveryQuickScanDone(host_name, services) => {
                let _ = events::ServicesDetected {
                    host_name,
                    services: services.iter().map(crate::dto::ServiceDto::from).collect(),
                }
                .emit(&app);
            }
            CoreEvent::DiscoveryFailed(host_name, message) => {
                let _ = events::ServicesFailed { host_name, message }.emit(&app);
            }
            CoreEvent::Error(message) => {
                let _ = events::Error { message }.emit(&app);
            }
            // Remote shell exit / dropped connection. Map the inner PTY id to its
            // public id (dropping routing state); `None` means the user already
            // closed the tab, so nothing is emitted (§3.4).
            CoreEvent::PtyExited(inner_id) => {
                if let Some(session_id) = app.state::<GuiState>().terminal_exited(inner_id) {
                    let _ = events::TerminalExited { session_id }.emit(&app);
                }
            }
            // Other variants are mapped as their producers start. `HostsLoaded`
            // is emitted directly by its command, SFTP results by the per-session
            // forwarder, and `PtyOutput` is superseded by the raw tap (§3.4/§3.6).
            _ => {}
        }
    }
}

/// Drains the PTY raw-output tap (§3.6) and demuxes each chunk into its tab's
/// channel, keyed by the core's inner PTY id. Spawned once at startup.
pub async fn forward_terminal_output(app: AppHandle, mut rx: mpsc::Receiver<(SessionId, Vec<u8>)>) {
    while let Some((inner_id, bytes)) = rx.recv().await {
        app.state::<GuiState>()
            .send_terminal_output(inner_id, bytes);
    }
}
