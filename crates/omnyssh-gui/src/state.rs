//! Backend-managed state (tech-gui.md §3.4). Stage 0.2 seeds only the shared
//! engine channel the bridge drains; the host cache, pollers, PTY/SFTP managers
//! and the session registry arrive with their slices.

use omnyssh_core::event::CoreEvent;
use tokio::sync::mpsc;

pub struct GuiState {
    // Kept alive so the bridge's receiver stays open; cloned to `PollManager` /
    // `PtyManager` as they start (Stage 1.2+).
    #[allow(dead_code)]
    engine_tx: mpsc::Sender<CoreEvent>,
}

impl GuiState {
    pub fn new(engine_tx: mpsc::Sender<CoreEvent>) -> Self {
        Self { engine_tx }
    }
}
