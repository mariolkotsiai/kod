fn main() {
    // Tauri 2 allows every app command for every window by default.
    // Restrict the IPC surface to this explicit list (keep in sync with capabilities/default.json).
    let attributes = tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "get_slots",
            "run_diagnostics",
            "run_backup",
            "run_restore",
        ]),
    );

    if let Err(e) = tauri_build::try_build(attributes) {
        eprintln!("tauri-build failed: {e}");
        std::process::exit(1);
    }
}
