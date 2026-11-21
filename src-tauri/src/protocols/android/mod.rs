pub mod executor;

use crate::core::archive_io::{read_and_verify_archive, sanitize_component, write_archive_atomic};
use crate::core::crypto::{compute_sha256, encrypt_payload, ArchiveMeta};
use crate::core::pal::{
    find_value, BackupMode, BackupResult, BatteryTelemetry, DeviceSlot, DiagnosticsReport,
    MobileDevice, PalError, Platform, RestoreMode, RestoreResult, StorageBreakdown,
    ThermalReading,
};
use async_trait::async_trait;
use executor::{run_adb, run_adb_timeout, verify_adb_host_trust};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::fs;
use zeroize::Zeroizing;

const COMPANION_PKG: &str = "com.omnimobile.agent";
const INSTALL_TIMEOUT: Duration = Duration::from_secs(120);

/// SHA-256 (lowercase hex) of the release-built companion APK, pinned at build time.
/// `None` = sideloading is refused (secure by default). An `Option`, not an empty string, so the
/// "not configured" state can never be mistaken for a digest (and clippy does not flag it).
pub const EXPECTED_APK_SHA256: Option<&str> = None;

/// Parses `df -k <mount>` output into `(total_kb, used_kb)` (second line, columns 2 and 3).
pub fn parse_df_kb(raw: &str) -> Option<(u64, u64)> {
    let line = raw.lines().filter(|l| !l.trim().is_empty()).nth(1)?;
    let fields: Vec<&str> = line.split_whitespace().collect();
    let total_kb = fields.get(1)?.parse::<u64>().ok()?;
    let used_kb = fields.get(2)?.parse::<u64>().ok()?;
    Some((total_kb, used_kb))
}

pub struct AndroidDevice {
    serial: String,
    slot: DeviceSlot,
    apk_path: Option<PathBuf>,
}

impl AndroidDevice {
    pub fn new(serial: String, slot: DeviceSlot, apk_path: Option<PathBuf>) -> Self {
        Self {
            serial,
            slot,
            apk_path,
        }
    }

    /// Sideloads the companion APK after verifying its SHA-256 against [`EXPECTED_APK_SHA256`].
    /// No `-g`: the stub needs no runtime permissions, so none are granted.
    pub async fn install_companion(&self) -> Result<(), PalError> {
        verify_adb_host_trust(&self.serial).await?;

        let apk = self.apk_path.as_ref().ok_or_else(|| {
            PalError::UnsupportedState("No companion APK configured".to_string())
        })?;
        let expected_sha256 = EXPECTED_APK_SHA256.ok_or_else(|| {
            PalError::UntrustedPeer(
                "No pinned companion APK digest configured; refusing to sideload".to_string(),
            )
        })?;

        // Canonical absolute path: also guarantees the argument cannot start with '-'.
        let apk = fs::canonicalize(apk)
            .await
            .map_err(|e| PalError::Transport(format!("Companion APK path: {e}")))?;
        let bytes = fs::read(&apk)
            .await
            .map_err(|e| PalError::Transport(format!("Companion APK read: {e}")))?;
        if !compute_sha256(&bytes).eq_ignore_ascii_case(expected_sha256) {
            return Err(PalError::UntrustedPeer(
                "Companion APK digest mismatch".to_string(),
            ));
        }

        let apk_str = apk.to_string_lossy().to_string();
        // adb on Windows does not handle the \\?\ verbatim prefix produced by canonicalize.
        let apk_arg = apk_str.strip_prefix(r"\\?\").unwrap_or(&apk_str);
        run_adb_timeout(&self.serial, &["install", "-r", apk_arg], INSTALL_TIMEOUT).await?;
        Ok(())
    }

    /// Removes the companion app (also use this at startup to clean up after a crash).
    pub async fn uninstall_companion(&self) -> Result<(), PalError> {
        let _ = run_adb(&self.serial, &["shell", "am", "force-stop", COMPANION_PKG]).await;
        run_adb(&self.serial, &["uninstall", COMPANION_PKG]).await?;
        Ok(())
    }

    /// Best-effort read of a numeric sysfs node; `None` when unreadable or not numeric.
    async fn read_sysfs_u64(&self, path: &str) -> Option<u64> {
        run_adb(&self.serial, &["shell", "cat", path])
            .await
            .ok()?
            .trim()
            .parse::<u64>()
            .ok()
    }

    async fn read_battery_telemetry(&self) -> Result<BatteryTelemetry, PalError> {
        let raw = run_adb(&self.serial, &["shell", "dumpsys", "battery"]).await?;

        let level_percent = find_value(&raw, "level")
            .and_then(|v| v.parse::<u8>().ok())
            .map(|v| v.min(100));
        let temperature_celsius = find_value(&raw, "temperature")
            .and_then(|v| v.parse::<i32>().ok())
            .map(|tenths| tenths as f32 / 10.0);

        let cycle_count = self
            .read_sysfs_u64("/sys/class/power_supply/battery/cycle_count")
            .await
            .and_then(|v| u32::try_from(v).ok());
        let charge_full = self
            .read_sysfs_u64("/sys/class/power_supply/battery/charge_full")
            .await;
        let charge_design = self
            .read_sysfs_u64("/sys/class/power_supply/battery/charge_full_design")
            .await;
        let health_percent = match (charge_full, charge_design) {
            (Some(full), Some(design)) if design > 0 => {
                u8::try_from((full.saturating_mul(100) / design).min(100)).ok()
            }
            _ => None,
        };

        Ok(BatteryTelemetry {
            level_percent,
            health_percent,
            cycle_count,
            temperature_celsius,
        })
    }

    async fn read_storage(&self) -> Option<StorageBreakdown> {
        let data = run_adb(&self.serial, &["shell", "df", "-k", "/data"])
            .await
            .ok()?;
        let (total_kb, used_kb) = parse_df_kb(&data)?;
        let system_bytes = run_adb(&self.serial, &["shell", "df", "-k", "/system"])
            .await
            .ok()
            .and_then(|raw| parse_df_kb(&raw))
            .map(|(system_total_kb, _)| system_total_kb.saturating_mul(1024));

        Some(StorageBreakdown {
            total_bytes: total_kb.saturating_mul(1024),
            used_bytes: used_kb.saturating_mul(1024),
            system_bytes,
        })
    }
}

#[async_trait]
impl MobileDevice for AndroidDevice {
    fn device_id(&self) -> &str {
        &self.serial
    }

    fn slot(&self) -> DeviceSlot {
        self.slot
    }

    fn platform(&self) -> Platform {
        Platform::Android
    }

    async fn get_diagnostics(&self) -> Result<DiagnosticsReport, PalError> {
        verify_adb_host_trust(&self.serial).await?;

        let battery = self.read_battery_telemetry().await?;
        let storage = self.read_storage().await;
        let thermals = match battery.temperature_celsius {
            Some(celsius) => vec![ThermalReading {
                zone: "battery".to_string(),
                celsius,
            }],
            None => Vec::new(),
        };

        // Sensors and the digitizer matrix are not probed yet: report "not measured"
        // instead of fabricated PASS results.
        Ok(DiagnosticsReport {
            device_id: self.serial.clone(),
            slot: self.slot,
            platform: Platform::Android,
            thermals,
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
        verify_adb_host_trust(&self.serial).await?;

        let mode_label = mode.as_str();
        let raw_payload = Zeroizing::new(match mode {
            BackupMode::Selective => run_adb(
                &self.serial,
                &[
                    "shell",
                    "content",
                    "query",
                    "--uri",
                    "content://com.android.contacts/data/phones",
                ],
            )
            .await?
            .into_bytes(),
            BackupMode::FullData | BackupMode::FullImage => {
                return Err(PalError::UnsupportedState(format!(
                    "Backup mode '{mode_label}' is not implemented for Android"
                )));
            }
        });

        let meta = ArchiveMeta {
            device_id: &self.serial,
            platform: Platform::Android.as_str(),
            mode: mode_label,
        };
        let archive = encrypt_payload(&raw_payload, passphrase, &meta)?;
        let serialized = serde_json::to_vec_pretty(&archive)
            .map_err(|e| PalError::Transport(format!("Archive serialization failed: {e}")))?;

        let stem = format!(
            "android_{}_slot{}",
            sanitize_component(&self.serial),
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
        verify_adb_host_trust(&self.serial).await?;

        // Verifies SHA-256 (if provided), platform and AEAD authentication; the decrypted
        // buffer is zeroized when it goes out of scope.
        let _verified = read_and_verify_archive(
            &backup_path,
            Platform::Android.as_str(),
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
