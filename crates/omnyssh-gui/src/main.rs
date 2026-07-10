//! OmnySSH Desktop entry point. Wires the tauri-specta IPC boundary (commands +
//! events), regenerates the TypeScript bindings in dev, spawns the core-event
//! bridge, and boots the window (tech-gui.md §3.3–§3.4).

mod bridge;
mod commands;
mod dto;
mod error;
mod events;
mod state;

use commands::hosts::{list_hosts, reload_hosts};
use omnyssh_core::event::CoreEvent;
use state::GuiState;
use tauri::Manager;
use tauri_specta::{collect_commands, collect_events, Builder};

// Absolute at build time, so the export target is independent of the run CWD.
const BINDINGS_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/ui/src/lib/bindings.ts");

/// The single definition of the IPC surface. Shared by `main` (dev export +
/// wiring) and the drift test so they can never disagree.
fn specta_builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new()
        .commands(collect_commands![list_hosts, reload_hosts])
        .events(collect_events![
            events::HostsLoaded,
            events::HostStatusChanged,
            events::MetricsUpdated,
            events::Error
        ])
}

/// Writes `bindings.ts` for the current IPC surface. Dev/test only — release
/// builds ship no exporter and never regenerate.
#[cfg(debug_assertions)]
fn export_bindings(path: impl AsRef<std::path::Path>) {
    // Emit `number` for u64 fields (`ageSeconds`, later the session/transfer ids);
    // their values stay well within JS's safe-integer range.
    let ts = specta_typescript::Typescript::default()
        .bigint(specta_typescript::BigIntExportBehavior::Number);
    specta_builder()
        .export(ts, path)
        .expect("failed to export TypeScript bindings");
}

fn main() {
    let builder = specta_builder();

    #[cfg(debug_assertions)]
    export_bindings(BINDINGS_PATH);

    tauri::Builder::default()
        // Persists UI prefs (theme, later sidebar collapse) from the frontend JS
        // API — no bespoke command (tech-gui.md §4.2, §5.1).
        .plugin(tauri_plugin_store::Builder::new().build())
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);

            let (engine_tx, engine_rx) = tokio::sync::mpsc::channel::<CoreEvent>(256);
            tauri::async_runtime::spawn(bridge::forward_core_events(
                app.handle().clone(),
                engine_rx,
            ));

            // Pre-load the shared host config so the first `list_hosts` paints
            // immediately. A load failure here is non-fatal — the frontend's
            // `reload_hosts` re-attempts and surfaces the error (tech-gui.md §3.4).
            let gui_state = GuiState::new(engine_tx);
            if let Ok(hosts) = omnyssh_core::config::load_all_hosts() {
                gui_state.set_hosts(hosts);
            }
            app.manage(gui_state);

            // The pollers are started by the frontend via `reload_hosts` once its
            // event bridge is listening, so no HostStatusChanged is emitted before
            // the webview can receive it (starting them here would race listener
            // registration and strand a host as offline).
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to launch OmnySSH Desktop");
}

#[cfg(test)]
mod tests {
    use super::{export_bindings, BINDINGS_PATH};

    /// The committed bindings must match a fresh export — fails loudly on drift
    /// without mutating the tracked file (tech-gui.md §0.2 acceptance, §3.3).
    #[test]
    fn committed_bindings_are_in_sync() {
        let tmp = std::env::temp_dir().join(format!("omnyssh-bindings-{}.ts", std::process::id()));
        export_bindings(&tmp);
        let generated = std::fs::read_to_string(&tmp).expect("read fresh bindings");
        let _ = std::fs::remove_file(&tmp);

        let committed = std::fs::read_to_string(BINDINGS_PATH).expect("read committed bindings");
        assert_eq!(
            committed, generated,
            "ui/src/lib/bindings.ts is out of date — regenerate with `cargo tauri dev`"
        );
    }
}
