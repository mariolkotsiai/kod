use async_trait::async_trait;
use omnimobile_toolkit::core::crypto::{decrypt_payload, encrypt_payload};
use omnimobile_toolkit::core::device_pool::{DevicePool, PoolError};
use omnimobile_toolkit::core::pal::{
    BackupMode, BackupResult, BatteryTelemetry, DeviceSlot, DiagnosticsReport,
    DigitizerMatrixResult, MobileDevice, PalError, Platform, RestoreMode, RestoreResult,
    SensorCheckResult, StorageBreakdown, ThermalReading,
};
use std::path::PathBuf;
use std::sync::Arc;

struct MockDevice {
    id: String,
    slot: DeviceSlot,
    platform: Platform,
}

#[async_trait]
impl MobileDevice for MockDevice {
    fn device_id(&self) -> &str {
        &self.id
    }

    fn slot(&self) -> DeviceSlot {
        self.slot
    }

    fn platform(&self) -> Platform {
        self.platform
    }

    async fn get_diagnostics(&self) -> Result<DiagnosticsReport, PalError> {
        Ok(DiagnosticsReport {
            device_id: self.id.clone(),
            slot: self.slot,
            platform: self.platform,
            thermals: vec![ThermalReading {
                zone: "cpu".to_string(),
                celsius: 31.0,
            }],
            battery: BatteryTelemetry {
                health_percent: 96,
                cycle_count: 110,
                temperature_celsius: 29.5,
            },
            storage: StorageBreakdown {
                total_bytes: 1000,
                used_bytes: 400,
                system_bytes: 100,
            },
            sensors: vec![SensorCheckResult {
                sensor_name: "imu".to_string(),
                operational: true,
                detail: "ok".to_string(),
            }],
            digitizer: DigitizerMatrixResult {
                rows: 10,
                cols: 10,
                dead_zones_detected: 0,
                passed: true,
            },
        })
    }

    async fn backup(&self, _mode: BackupMode, _passphrase: &str) -> Result<BackupResult, PalError> {
        Err(PalError::UnsupportedState("mock".to_string()))
    }

    async fn restore(
        &self,
        _backup_path: PathBuf,
        _mode: RestoreMode,
        _passphrase: &str,
    ) -> Result<RestoreResult, PalError> {
        Err(PalError::UnsupportedState("mock".to_string()))
    }
}

#[test]
fn crypto_roundtrip_and_wrong_passphrase_rejected() {
    let secret = b"sensitive-device-forensic-payload";
    let archive = encrypt_payload(secret, "strong-passphrase-123").expect("encrypt should succeed");

    let decrypted = decrypt_payload(&archive, "strong-passphrase-123").expect("decrypt should succeed");
    assert_eq!(decrypted.as_slice(), secret);

    let bad = decrypt_payload(&archive, "wrong-passphrase");
    assert!(bad.is_err());
}

#[tokio::test]
async fn device_pool_enforces_two_slots_max() {
    let pool = DevicePool::new();
    let d0 = Arc::new(MockDevice {
        id: "android-001".to_string(),
        slot: DeviceSlot::Slot0,
        platform: Platform::Android,
    });
    let d1 = Arc::new(MockDevice {
        id: "apple-002".to_string(),
        slot: DeviceSlot::Slot1,
        platform: Platform::Apple,
    });
    let d2 = Arc::new(MockDevice {
        id: "android-003".to_string(),
        slot: DeviceSlot::Slot0,
        platform: Platform::Android,
    });

    let s0 = pool.attach_device(d0).await.expect("slot 0 attach");
    let s1 = pool.attach_device(d1).await.expect("slot 1 attach");
    assert_eq!(s0, DeviceSlot::Slot0);
    assert_eq!(s1, DeviceSlot::Slot1);

    let err = pool.attach_device(d2).await.expect_err("third device must fail");
    assert!(matches!(err, PoolError::PoolFull));
}