pub mod companion; // companion/mod.rs: pub mod apple_relay;
pub mod core; // core/mod.rs: pub mod crypto; pub mod pal; pub mod device_pool; pub mod exec; pub mod archive_io;
pub mod ipc; // Tauri commands + AppState (src/ipc/mod.rs)
pub mod protocols; // protocols/mod.rs: pub mod android; pub mod apple;
pub mod tui; // standalone terminal UI, --tui (src/tui/mod.rs)

use crate::core::device_pool::DevicePool;
use crate::ipc::AppState;
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BackendError {
    #[error("TUI error: {0}")]
    Tui(#[from] tui::TuiError),
    #[error("Tauri runtime error: {0}")]
    Tauri(String),
}

/// Launches the standalone TUI mode (`--tui`).
pub async fn start_tui_mode(pool: Arc<DevicePool>) -> Result<(), BackendError> {
    tui::run_tui(pool).await.map_err(BackendError::from)
}

/// Launches the Tauri GUI desktop application.
pub fn start_gui_mode(pool: Arc<DevicePool>) -> Result<(), BackendError> {
    tauri::Builder::default()
        .manage(AppState { pool })
        .invoke_handler(tauri::generate_handler![
            ipc::scan_usb,
            ipc::get_slots,
            ipc::run_diagnostics,
            ipc::run_backup,
            ipc::run_restore,
        ])
        .run(tauri::generate_context!())
        .map_err(|e| BackendError::Tauri(e.to_string()))
}
