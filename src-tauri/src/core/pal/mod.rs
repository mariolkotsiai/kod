use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BackupMode {
    Selective,
    FullData,
    FullImage,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatteryTelemetry {
    pub health_percent: u8,
    pub cycle_count: u32,
    pub temperature_celsius: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageBreakdown {
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub system_bytes: u64,
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
    pub storage: StorageBreakdown,
    pub sensors: Vec<SensorCheckResult>,
    pub digitizer: DigitizerMatrixResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupResult {
    pub archive_path: PathBuf,
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

#[async_trait]
pub trait MobileDevice: Send + Sync {
    fn device_id(&self) -> &str;
    fn slot(&self) -> DeviceSlot;
    fn platform(&self) -> Platform;

    /// Collects thermals, battery cycles, storage breakdown, sensor checks, and digitizer test.
    async fn get_diagnostics(&self) -> Result<DiagnosticsReport, PalError>;

    /// Executes Selective, FullData, or FullImage backup and writes an encrypted archive.
    async fn backup(&self, mode: BackupMode, passphrase: &str) -> Result<BackupResult, PalError>;

    /// Validates archive SHA-256 pre-flight and restores data/image to the device.
    async fn restore(
        &self,
        backup_path: PathBuf,
        mode: RestoreMode,
        passphrase: &str,
    ) -> Result<RestoreResult, PalError>;
}