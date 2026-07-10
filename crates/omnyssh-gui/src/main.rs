// OmnySSH Desktop entry point. Stage 0 boots a blank window; managed state, the
// IPC commands, and the core bridge arrive in later stages (tech-gui.md §7).
fn main() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("failed to launch OmnySSH Desktop");
}
