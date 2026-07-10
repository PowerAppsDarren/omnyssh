//! Backend-managed state (tech-gui.md §3.4). Holds the host cache and the
//! metrics/status poller; the shared engine channel feeds the bridge. PTY/SFTP
//! managers and the session registry arrive with their slices.

use std::sync::{Mutex, RwLock};
use std::time::Duration;

use omnyssh_core::event::CoreEvent;
use omnyssh_core::ssh::client::Host;
use omnyssh_core::ssh::pool::PollManager;
use tokio::sync::mpsc;

use crate::dto::HostDto;

/// Metric poll cadence. Mirrors the TUI's fixed interval; a configurable refresh
/// interval lands with settings in Stage 4.3 (tech-gui.md §4.3).
const POLL_INTERVAL: Duration = Duration::from_secs(30);

pub struct GuiState {
    /// Cached host list, mapped to `HostDto` on demand for `list_hosts`.
    hosts: RwLock<Vec<Host>>,
    /// Metrics/status/discovery poller; replaced on reload, dropped on exit.
    poll: Mutex<Option<PollManager>>,
    /// Shared engine channel the bridge drains; cloned to each `PollManager`.
    engine_tx: mpsc::Sender<CoreEvent>,
}

impl GuiState {
    pub fn new(engine_tx: mpsc::Sender<CoreEvent>) -> Self {
        Self {
            hosts: RwLock::new(Vec::new()),
            poll: Mutex::new(None),
            engine_tx,
        }
    }

    /// Replace the host cache with a freshly loaded list.
    pub fn set_hosts(&self, hosts: Vec<Host>) {
        *self.hosts.write().expect("hosts lock poisoned") = hosts;
    }

    /// Snapshot the cached hosts as wire DTOs (secret fields dropped by the map).
    pub fn host_dtos(&self) -> Vec<HostDto> {
        self.hosts
            .read()
            .expect("hosts lock poisoned")
            .iter()
            .map(HostDto::from)
            .collect()
    }

    /// Resolve `names` to their full `Host` records for a backend-only op (snippet
    /// execute, key setup). Secret material rides along here and never crosses the
    /// IPC boundary. Unknown names are skipped; order follows `names`.
    pub fn hosts_by_name(&self, names: &[String]) -> Vec<Host> {
        let hosts = self.hosts.read().expect("hosts lock poisoned");
        names
            .iter()
            .filter_map(|name| hosts.iter().find(|h| &h.name == name).cloned())
            .collect()
    }

    /// Start (or restart) the pollers for the cached hosts. Must run inside the
    /// Tauri async runtime — `PollManager::start` spawns tokio tasks (§3.4).
    pub fn restart_pollers(&self) {
        let hosts = self.hosts.read().expect("hosts lock poisoned").clone();
        let mut poll = self.poll.lock().expect("poll lock poisoned");
        if let Some(old) = poll.take() {
            old.shutdown();
        }
        *poll = Some(PollManager::start(
            hosts,
            self.engine_tx.clone(),
            POLL_INTERVAL,
        ));
    }
}
