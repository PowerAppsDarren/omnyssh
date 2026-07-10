//! Core-event bridge (tech-gui.md §3.4): one long-lived task drains the shared
//! engine channel and maps each `CoreEvent` to a typed IPC event. Status and
//! metrics land here; discovery, key-setup and PTY-exit mappings arrive with
//! their producers.

use omnyssh_core::event::CoreEvent;
use tauri::AppHandle;
use tauri_specta::Event;
use tokio::sync::mpsc;

use crate::events;

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
            CoreEvent::Error(message) => {
                let _ = events::Error { message }.emit(&app);
            }
            // Other variants are mapped as their producers start. `HostsLoaded`
            // is emitted directly by its command, SFTP results by the
            // per-session forwarder, and PTY bytes by the raw tap (§3.4/§3.6).
            _ => {}
        }
    }
}
