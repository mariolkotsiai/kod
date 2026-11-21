use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeviceSlot {
    Slot0 = 0,
    Slot1 = 1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Platform {
    Android,
    Apple,
}

impl Platform {
    /// Stable identifier stored in archive headers.
    pub fn as_str(&self) -> &'static str {
        match self {
            Platform::Android => "android",
            Platform::Apple => "apple",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BackupMode {
    Selective,
    FullData,
    FullImage,
}

impl BackupMode {
    /// Stable identifier stored in archive headers.
    pub fn as_str(&self) -> &'static str {
        match self {
            BackupMode::Selective => "selective",
            BackupMode::FullData => "full_data",
            BackupMode::FullImage => "full_image",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RestoreMode {
    Selective,
    FullData,
    FullImage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThermalReading {
    pub zone: String,
    pub celsius: f32,
}

/// Every field is optional: `None` means "not measured on this device", never a made-up default.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatteryTelemetry {
    /// Current charge level (what `dumpsys battery` / lockdownd report as "level").
    pub level_percent: Option<u8>,
    /// Capacity relative to design capacity, only when the device exposes both values.
    pub health_percent: Option<u8>,
    pub cycle_count: Option<u32>,
    pub temperature_celsius: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageBreakdown {
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub system_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorCheckResult {
    pub sensor_name: String,
    pub operational: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DigitizerMatrixResult {
    pub rows: u16,
    pub cols: u16,
    pub dead_zones_detected: u16,
    pub passed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticsReport {
    pub device_id: String,
    pub slot: DeviceSlot,
    pub platform: Platform,
    pub thermals: Vec<ThermalReading>,
    pub battery: BatteryTelemetry,
    /// `None` = not measured.
    pub storage: Option<StorageBreakdown>,
    /// Only sensors that were genuinely probed.
    pub sensors: Vec<SensorCheckResult>,
    /// `None` = not measured (requires the companion app handshake, not implemented yet).
    pub digitizer: Option<DigitizerMatrixResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupResult {
    pub archive_path: PathBuf,
    /// SHA-256 of the archive file on disk.
    pub sha256_hex: String,
    pub size_bytes: u64,
    pub encrypted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreResult {
    pub verified_sha256: String,
    pub restored_items: u64,
}

#[derive(Debug, Error)]
pub enum PalError {
    #[error("Transport or USB I/O failure: {0}")]
    Transport(String),
    #[error("Peer trust / certificate verification failed: {0}")]
    UntrustedPeer(String),
    #[error("Checksum mismatch during pre-flight validation: expected {expected}, got {actual}")]
    ChecksumMismatch { expected: String, actual: String },
    #[error("Unsupported operation for current device state: {0}")]
    UnsupportedState(String),
    #[error("Cryptographic operation failed: {0}")]
    Crypto(#[from] crate::core::crypto::CryptoError),
}

/// Returns the trimmed value of the first `key: value` line in `raw` (leading whitespace ignored).
pub fn find_value<'a>(raw: &'a str, key: &str) -> Option<&'a str> {
    raw.lines().find_map(|line| {
        let rest = line.trim().strip_prefix(key)?;
        rest.strip_prefix(':').map(str::trim)
    })
}

/// [`find_value`] parsed as `u64`.
pub fn find_u64(raw: &str, key: &str) -> Option<u64> {
    find_value(raw, key)?.parse::<u64>().ok()
}

/// Reads `<key>NAME</key><integer>N</integer>` from XML plist output (what `idevicediagnostics`
/// prints). `None` when the key is absent or the value that follows it is not an `<integer>`.
pub fn find_plist_u64(raw: &str, key: &str) -> Option<u64> {
    let marker = format!("<key>{key}</key>");
    let after_key = &raw[raw.find(&marker)? + marker.len()..];
    // The value element must directly follow the key (only whitespace in between).
    let value = after_key.trim_start().strip_prefix("<integer>")?;
    value[..value.find("</integer>")?].trim().parse::<u64>().ok()
}

#[async_trait]
pub trait MobileDevice: Send + Sync {
    fn device_id(&self) -> &str;
    fn slot(&self) -> DeviceSlot;
    fn platform(&self) -> Platform;

    /// Collects diagnostics. Values that cannot be measured are reported as `None`/empty.
    async fn get_diagnostics(&self) -> Result<DiagnosticsReport, PalError>;

    /// Creates an encrypted archive inside `out_dir` (never overwrites an existing archive).
    /// Modes that are not implemented for the platform return `PalError::UnsupportedState`.
    async fn backup(
        &self,
        mode: BackupMode,
        passphrase: &str,
        out_dir: &Path,
    ) -> Result<BackupResult, PalError>;

    /// Verifies the archive (optional expected SHA-256, platform, AEAD authentication) and restores
    /// it. `backup_path` must already be confined with `archive_io::resolve_restore_path`.
    async fn restore(
        &self,
        backup_path: PathBuf,
        mode: RestoreMode,
        passphrase: &str,
        expected_sha256: Option<&str>,
    ) -> Result<RestoreResult, PalError>;
}
