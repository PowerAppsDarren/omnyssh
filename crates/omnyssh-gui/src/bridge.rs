//! Core-event bridge (tech-gui.md §3.4): one long-lived task drains the shared
//! engine channel and maps each `CoreEvent` to a typed IPC event. Stage 0.2
//! wires the `Error` mapping; metrics, status, discovery, key-setup and
//! PTY-exit mappings land with their producers.

use omnyssh_core::event::CoreEvent;
use tauri::AppHandle;
use tauri_specta::Event;
use tokio::sync::mpsc;

use crate::events;

pub async fn forward_core_events(app: AppHandle, mut rx: mpsc::Receiver<CoreEvent>) {
    while let Some(event) = rx.recv().await {
        // A match with an explicit ignore arm (§3.4), grown one variant per slice.
        #[allow(clippy::single_match)]
        match event {
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
