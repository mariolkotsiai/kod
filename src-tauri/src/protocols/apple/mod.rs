use crate::companion::apple_relay::{run_imobile_cmd, verify_lockdownd_pairing};
use crate::core::archive_io::{read_and_verify_archive, sanitize_component, write_archive_atomic};
use crate::core::crypto::{compute_sha256, encrypt_payload, ArchiveMeta};
use crate::core::pal::{
    find_plist_u64, find_u64, find_value, BackupMode, BackupResult, BatteryTelemetry, DeviceSlot,
    DiagnosticsReport, MobileDevice, PalError, Platform, RestoreMode, RestoreResult,
    StorageBreakdown,
};
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use zeroize::Zeroizing;

/// `idevicediagnostics` prints XML plist; `ideviceinfo` prints `key: value` lines.
/// Try the plist form first, then the line form, so either output shape works.
fn gauge_u64(raw: &str, key: &str) -> Option<u64> {
    find_plist_u64(raw, key).or_else(|| find_u64(raw, key))
}

pub struct AppleDevice {
    udid: String,
    slot: DeviceSlot,
}

impl AppleDevice {
    pub fn new(udid: String, slot: DeviceSlot) -> Self {
        Self { udid, slot }
    }

    /// Best-effort battery data; anything the device does not report stays `None`.
    async fn read_battery(&self) -> BatteryTelemetry {
        let battery_raw = run_imobile_cmd(
            "ideviceinfo",
            &self.udid,
            &["-q", "com.apple.mobile.battery"],
        )
        .await
        .unwrap_or_default();
        let gauge_raw = run_imobile_cmd(
            "idevicediagnostics",
            &self.udid,
            &["diagnostics", "GasGauge"],
        )
        .await
        .unwrap_or_default();

        // "BatteryCurrentCapacity" is the charge level, not battery health.
        let level_percent = find_value(&battery_raw, "BatteryCurrentCapacity")
            .and_then(|v| v.parse::<u8>().ok())
            .map(|v| v.min(100));
        let cycle_count = gauge_u64(&gauge_raw, "CycleCount").and_then(|v| u32::try_from(v).ok());
        let health_percent = match (
            gauge_u64(&gauge_raw, "NominalChargeCapacity"),
            gauge_u64(&gauge_raw, "DesignCapacity"),
        ) {
            (Some(nominal), Some(design)) if design > 0 => {
                u8::try_from((nominal.saturating_mul(100) / design).min(100)).ok()
            }
            _ => None,
        };

        BatteryTelemetry {
            level_percent,
            health_percent,
            cycle_count,
            temperature_celsius: None,
        }
    }

    /// Best-effort storage figures from the `com.apple.disk_usage` lockdown domain.
    async fn read_storage(&self) -> Option<StorageBreakdown> {
        let raw = run_imobile_cmd("ideviceinfo", &self.udid, &["-q", "com.apple.disk_usage"])
            .await
            .ok()?;

        let total =
            find_u64(&raw, "TotalDiskCapacity").or_else(|| find_u64(&raw, "TotalDataCapacity"))?;
        let available = find_u64(&raw, "TotalDataAvailable")?;

        Some(StorageBreakdown {
            total_bytes: total,
            // Approximation: everything that is not available user-data space counts as used.
            used_bytes: total.saturating_sub(available),
            system_bytes: find_u64(&raw, "TotalSystemCapacity"),
        })
    }
}

#[async_trait]
impl MobileDevice for AppleDevice {
    fn device_id(&self) -> &str {
        &self.udid
    }

    fn slot(&self) -> DeviceSlot {
        self.slot
    }

    fn platform(&self) -> Platform {
        Platform::Apple
    }

    async fn get_diagnostics(&self) -> Result<DiagnosticsReport, PalError> {
        verify_lockdownd_pairing(&self.udid).await?;

        let battery = self.read_battery().await;
        let storage = self.read_storage().await;

        // Thermals, sensors and the digitizer matrix are not probed: "not measured",
        // never fabricated values.
        Ok(DiagnosticsReport {
            device_id: self.udid.clone(),
            slot: self.slot,
            platform: Platform::Apple,
            thermals: Vec::new(),
            battery,
            storage,
            sensors: Vec::new(),
            digitizer: None,
        })
    }

    async fn backup(
        &self,
        mode: BackupMode,
        passphrase: &str,
        out_dir: &Path,
    ) -> Result<BackupResult, PalError> {
        verify_lockdownd_pairing(&self.udid).await?;

        let mode_label = mode.as_str();
        let raw_payload = Zeroizing::new(match mode {
            BackupMode::Selective => run_imobile_cmd("ideviceinfo", &self.udid, &["-s"])
                .await?
                .into_bytes(),
            BackupMode::FullData | BackupMode::FullImage => {
                return Err(PalError::UnsupportedState(format!(
                    "Backup mode '{mode_label}' is not implemented for Apple devices"
                )));
            }
        });

        let meta = ArchiveMeta {
            device_id: &self.udid,
            platform: Platform::Apple.as_str(),
            mode: mode_label,
        };
        let archive = encrypt_payload(&raw_payload, passphrase, &meta)?;
        let serialized = serde_json::to_vec_pretty(&archive)
            .map_err(|e| PalError::Transport(format!("Archive serialization failed: {e}")))?;

        let stem = format!(
            "apple_{}_slot{}",
            sanitize_component(&self.udid),
            self.slot as u8
        );
        let archive_path = write_archive_atomic(out_dir, &stem, &serialized).await?;

        Ok(BackupResult {
            archive_path,
            sha256_hex: compute_sha256(&serialized),
            size_bytes: serialized.len() as u64,
            encrypted: true,
        })
    }

    async fn restore(
        &self,
        backup_path: PathBuf,
        _mode: RestoreMode,
        passphrase: &str,
        expected_sha256: Option<&str>,
    ) -> Result<RestoreResult, PalError> {
        verify_lockdownd_pairing(&self.udid).await?;

        // Verifies SHA-256 (if provided), platform and AEAD authentication; the decrypted
        // buffer is zeroized when it goes out of scope.
        let _verified = read_and_verify_archive(
            &backup_path,
            Platform::Apple.as_str(),
            passphrase,
            expected_sha256,
        )
        .await?;

        Err(PalError::UnsupportedState(
            "Restore to the device is not implemented yet; archive integrity and passphrase were verified"
                .to_string(),
        ))
    }
}
