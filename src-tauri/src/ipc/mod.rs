//! Tauri IPC commands. A command is callable from the webview only if it is listed in BOTH
//! `build.rs` (AppManifest) and `capabilities/default.json`; anything else is denied by the ACL.

use crate::core::archive_io::resolve_restore_path;
use crate::core::device_pool::{DevicePool, DiscoveredUsbDevice, SlotStatus};
use crate::core::pal::{
    BackupMode, BackupResult, DeviceSlot, DiagnosticsReport, RestoreMode, RestoreResult,
};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Manager, State};
use zeroize::Zeroizing;

pub struct AppState {
    pub pool: Arc<DevicePool>,
}

fn map_slot(slot_idx: u8) -> Result<DeviceSlot, String> {
    match slot_idx {
        0 => Ok(DeviceSlot::Slot0),
        1 => Ok(DeviceSlot::Slot1),
        _ => Err("Invalid device slot: expected 0 or 1".to_string()),
    }
}

/// `<app data dir>/backups`: writable for installed apps, independent of the working directory.
/// `Manager` must be in scope for `app.path()`.
fn backups_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|dir| dir.join("backups"))
        .map_err(|e| format!("Cannot resolve the application data directory: {e}"))
}

// NOTE: not in build.rs / capabilities (the frontend does not use it), so the webview cannot call it.
// To expose it: add "scan_usb" to build.rs and "allow-scan-usb" to capabilities/default.json.
#[tauri::command]
pub async fn scan_usb() -> Result<Vec<DiscoveredUsbDevice>, String> {
    // Was a sync command doing blocking libusb I/O; the async variant uses spawn_blocking.
    DevicePool::scan_usb_devices_async()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_slots(state: State<'_, AppState>) -> Result<Vec<SlotStatus>, String> {
    Ok(state.pool.list_slots().await)
}

#[tauri::command]
pub async fn run_diagnostics(
    state: State<'_, AppState>,
    slot_idx: u8,
) -> Result<DiagnosticsReport, String> {
    let slot = map_slot(slot_idx)?;
    let device = state
        .pool
        .get_device(slot)
        .await
        .map_err(|e| e.to_string())?;
    // One operation at a time per slot; the guard is held until the function returns.
    let _guard = state.pool.lock_slot(slot).await;
    device.get_diagnostics().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn run_backup(
    app: AppHandle,
    state: State<'_, AppState>,
    slot_idx: u8,
    mode: BackupMode,
    passphrase: String,
) -> Result<BackupResult, String> {
    // Wipe our copy of the passphrase when this command returns.
    let passphrase = Zeroizing::new(passphrase);
    let slot = map_slot(slot_idx)?;
    let device = state
        .pool
        .get_device(slot)
        .await
        .map_err(|e| e.to_string())?;
    let out_dir = backups_dir(&app)?;
    let _guard = state.pool.lock_slot(slot).await;
    device
        .backup(mode, passphrase.as_str(), &out_dir)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn run_restore(
    app: AppHandle,
    state: State<'_, AppState>,
    slot_idx: u8,
    backup_path: PathBuf,
    mode: RestoreMode,
    passphrase: String,
    expected_sha256: Option<String>, // from the UI; enables whole-file integrity check
) -> Result<RestoreResult, String> {
    let passphrase = Zeroizing::new(passphrase);
    let slot = map_slot(slot_idx)?;
    let device = state
        .pool
        .get_device(slot)
        .await
        .map_err(|e| e.to_string())?;

    // Confine the path to the backups dir, require the .ombackup extension, cap the size.
    // The webview can no longer make the backend read arbitrary files.
    let safe_path = resolve_restore_path(&backups_dir(&app)?, &backup_path)
        .await
        .map_err(|e| e.to_string())?;
    let _guard = state.pool.lock_slot(slot).await;
    device
        .restore(
            safe_path,
            mode,
            passphrase.as_str(),
            expected_sha256.as_deref(),
        )
        .await
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `map_slot` είναι το μόνο φίλτρο ανάμεσα στο index του webview και το pool.
    #[test]
    fn map_slot_accepts_only_zero_and_one() {
        assert_eq!(map_slot(0), Ok(DeviceSlot::Slot0));
        assert_eq!(map_slot(1), Ok(DeviceSlot::Slot1));
        for bad in [2u8, 3, 127, 255] {
            assert!(map_slot(bad).is_err(), "slot index {bad} must be rejected");
        }
    }
}
