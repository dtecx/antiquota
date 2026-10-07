fn main() {
    // The engine's page (a remote origin to Tauri) may call these app commands; the capability
    // in capabilities/engine-ui.json grants them by these generated permission names.
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(&["save_file", "app_info", "quit_app", "retry_engine"])),
    )
    .expect("failed to run tauri-build");
}
