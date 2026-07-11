//! Auto SSH key-setup command (tech-gui.md §4.2). `start_key_setup` mirrors the TUI's
//! orchestration (crates/omnyssh `ConfirmKeySetup`): connect a password session, drive
//! `setup_key_for_host`, and report progress + the terminal outcome as
//! `CoreEvent::KeySetup*` on the shared engine channel — the bridge maps those to
//! `key-setup-*` IPC events (§3.4). On success it writes the generated key back onto the
//! manual host in `hosts.toml`, the same completion write the TUI performs, so a
//! `reload_hosts` afterwards refreshes `hasKey`/`passwordAuthDisabled` on the card.

use tauri::State;
use tokio::sync::mpsc;

use omnyssh_core::config::{load_hosts, save_hosts};
use omnyssh_core::event::CoreEvent;
use omnyssh_core::ssh::client::Host;
use omnyssh_core::ssh::key_setup::{setup_key_for_host, KeySetupState, KeySetupStep, KeyType};
use omnyssh_core::ssh::session::SshSession;

use crate::error::CommandError;
use crate::state::GuiState;

/// Start auto key-setup for `host_name` (tech-gui.md §4.2). Fire-and-forget: the flow
/// runs on a background task and reports via `key-setup-*` events, mirroring the core's
/// own model. Resolving the full host record (secrets included) stays backend-side
/// (§3.4); an unknown host is the one synchronous error.
#[tauri::command]
#[specta::specta]
pub fn start_key_setup(state: State<'_, GuiState>, host_name: String) -> Result<(), CommandError> {
    let host = state.host_by_name(&host_name).ok_or_else(|| CommandError {
        message: format!("unknown host '{host_name}'"),
    })?;
    tauri::async_runtime::spawn(run_key_setup(host, state.engine_sender()));
    Ok(())
}

/// The background flow: forward each step as `KeySetupProgress`, connect with the
/// password, run `setup_key_for_host`, then map the final state to exactly one terminal
/// event (`KeySetupComplete` / `KeySetupRollback` / `KeySetupFailed`).
async fn run_key_setup(host: Host, engine_tx: mpsc::Sender<CoreEvent>) {
    // Drain the core's per-step channel onto the shared engine channel, tagging each
    // step with the host name (the core's `Sender<KeySetupStep>` carries no host).
    let (progress_tx, mut progress_rx) = mpsc::channel::<KeySetupStep>(8);
    let step_tx = engine_tx.clone();
    let step_host = host.name.clone();
    tokio::spawn(async move {
        while let Some(step) = progress_rx.recv().await {
            let _ = step_tx
                .send(CoreEvent::KeySetupProgress(step_host.clone(), step))
                .await;
        }
    });

    let session = match SshSession::connect(&host).await {
        Ok(session) => session,
        Err(e) => {
            let _ = engine_tx
                .send(CoreEvent::KeySetupFailed(
                    host.name.clone(),
                    format!("Connection failed: {e}"),
                ))
                .await;
            return;
        }
    };

    let outcome = setup_key_for_host(&host, &session, KeyType::Ed25519, Some(progress_tx)).await;
    session.disconnect().await;

    match outcome {
        // Both Success and PartialSuccess mean the key is generated, copied, and
        // verified — the card should show `hasKey`. Only full Success disabled password
        // auth (PartialSuccess is "key works, no sudo"), so only it marks that + clears
        // the stored password. Persist BEFORE emitting so the frontend's reload sees it.
        Ok(result)
            if matches!(
                result.state,
                KeySetupState::Success | KeySetupState::PartialSuccess
            ) =>
        {
            let disabled = matches!(result.state, KeySetupState::Success);
            persist_key(&host.name, &result.key_path.to_string_lossy(), disabled).await;
            let _ = engine_tx
                .send(CoreEvent::KeySetupComplete(
                    host.name.clone(),
                    result.key_path,
                ))
                .await;
        }
        Ok(result) if result.state == KeySetupState::RolledBack => {
            let reason = result
                .error_message
                .unwrap_or_else(|| "Rolled back.".to_string());
            let _ = engine_tx
                .send(CoreEvent::KeySetupRollback(host.name.clone(), reason))
                .await;
        }
        Ok(result) => {
            let reason = result
                .error_message
                .unwrap_or_else(|| "Key setup failed.".to_string());
            let _ = engine_tx
                .send(CoreEvent::KeySetupFailed(host.name.clone(), reason))
                .await;
        }
        Err(e) => {
            let _ = engine_tx
                .send(CoreEvent::KeySetupFailed(
                    host.name.clone(),
                    format!("{e:#}"),
                ))
                .await;
        }
    }
}

/// Write the generated key onto the manual host in `hosts.toml`, mirroring the TUI's
/// completion write: set `identity_file` + `key_setup_date`, and — only when password
/// auth was actually disabled — mark `password_auth_disabled` and drop the stored
/// password (key auth supersedes it). Only manual hosts live in `hosts.toml`, so an
/// SSH-config name is a no-op. Best-effort: the completion event fires regardless, and
/// this is awaited so a following `reload_hosts` observes the write (no read/write race).
async fn persist_key(host_name: &str, key_path: &str, password_disabled: bool) {
    let name = host_name.to_string();
    let key_path = key_path.to_string();
    let _ = tauri::async_runtime::spawn_blocking(move || -> Option<()> {
        let mut hosts = load_hosts().ok()?;
        let host = hosts.iter_mut().find(|h| h.name == name)?;
        host.identity_file = Some(key_path);
        host.key_setup_date = Some(chrono::Utc::now().to_rfc3339());
        if password_disabled {
            host.password_auth_disabled = Some(true);
            host.password = None;
        }
        save_hosts(&hosts).ok()
    })
    .await;
}
