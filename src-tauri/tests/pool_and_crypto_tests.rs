use async_trait::async_trait;
use omnimobile_toolkit::companion::apple_relay::validate_udid;
use omnimobile_toolkit::core::archive_io::{
    read_and_verify_archive, resolve_restore_path, sanitize_component, write_archive_atomic,
};
use omnimobile_toolkit::core::crypto::{
    decrypt_payload, encrypt_payload, ArchiveMeta, CryptoError,
};
use omnimobile_toolkit::core::device_pool::{DevicePool, PoolError};
use omnimobile_toolkit::core::pal::{
    find_plist_u64, find_u64, find_value, BackupMode, BackupResult, BatteryTelemetry, DeviceSlot,
    DiagnosticsReport, DigitizerMatrixResult, MobileDevice, PalError, Platform, RestoreMode,
    RestoreResult, SensorCheckResult, StorageBreakdown, ThermalReading,
};
use omnimobile_toolkit::protocols::android::{executor::validate_serial, parse_df_kb};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

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
                level_percent: Some(80),
                health_percent: Some(96),
                cycle_count: Some(110),
                temperature_celsius: Some(29.5),
            },
            storage: Some(StorageBreakdown {
                total_bytes: 1000,
                used_bytes: 400,
                system_bytes: Some(100),
            }),
            sensors: vec![SensorCheckResult {
                sensor_name: "imu".to_string(),
                operational: true,
                detail: "ok".to_string(),
            }],
            digitizer: Some(DigitizerMatrixResult {
                rows: 10,
                cols: 10,
                dead_zones_detected: 0,
                passed: true,
            }),
        })
    }

    async fn backup(
        &self,
        _mode: BackupMode,
        _passphrase: &str,
        _out_dir: &Path,
    ) -> Result<BackupResult, PalError> {
        Err(PalError::UnsupportedState("mock".to_string()))
    }

    async fn restore(
        &self,
        _backup_path: PathBuf,
        _mode: RestoreMode,
        _passphrase: &str,
        _expected_sha256: Option<&str>,
    ) -> Result<RestoreResult, PalError> {
        Err(PalError::UnsupportedState("mock".to_string()))
    }
}

fn mock(id: &str, slot: DeviceSlot, platform: Platform) -> Arc<MockDevice> {
    Arc::new(MockDevice {
        id: id.to_string(),
        slot,
        platform,
    })
}

fn meta() -> ArchiveMeta<'static> {
    ArchiveMeta {
        device_id: "android-001",
        platform: "android",
        mode: "selective",
    }
}

const PASS: &str = "strong-passphrase-123";

// ---------------------------------------------------------------- crypto

#[test]
fn crypto_roundtrip_and_wrong_passphrase_rejected() {
    let secret = b"sensitive-device-forensic-payload";
    let archive = encrypt_payload(secret, PASS, &meta()).expect("encrypt should succeed");

    let decrypted = decrypt_payload(&archive, PASS).expect("decrypt should succeed");
    assert_eq!(decrypted.as_slice(), secret);

    let bad = decrypt_payload(&archive, "wrong-passphrase-xyz");
    assert!(matches!(bad, Err(CryptoError::DecryptionAuthFailed)));
}

#[test]
fn crypto_rejects_weak_passphrase() {
    let res = encrypt_payload(b"x", "short", &meta());
    assert!(matches!(res, Err(CryptoError::WeakPassphrase)));
}

#[test]
fn crypto_detects_header_and_ciphertext_tampering() {
    let archive = encrypt_payload(b"payload-bytes", PASS, &meta()).expect("encrypt");

    // Header fields are authenticated as AEAD associated data.
    let mut header_tampered = archive.clone();
    header_tampered.header.device_id = "attacker-device".to_string();
    assert!(matches!(
        decrypt_payload(&header_tampered, PASS),
        Err(CryptoError::DecryptionAuthFailed)
    ));

    // Flipping a ciphertext nibble must fail authentication.
    let mut ct_tampered = archive.clone();
    let mut bytes = ct_tampered.ciphertext_hex.clone().into_bytes();
    bytes[0] = if bytes[0] == b'0' { b'1' } else { b'0' };
    ct_tampered.ciphertext_hex = String::from_utf8(bytes).expect("hex stays ASCII");
    assert!(matches!(
        decrypt_payload(&ct_tampered, PASS),
        Err(CryptoError::DecryptionAuthFailed)
    ));
}

#[test]
fn crypto_validates_format_before_running_the_kdf() {
    let archive = encrypt_payload(b"payload-bytes", PASS, &meta()).expect("encrypt");

    let mut bad_nonce = archive.clone();
    bad_nonce.header.nonce_hex = "00".to_string();
    assert!(matches!(
        decrypt_payload(&bad_nonce, PASS),
        Err(CryptoError::InvalidPayload)
    ));

    let mut bad_version = archive.clone();
    bad_version.header.version = 1;
    assert!(matches!(
        decrypt_payload(&bad_version, PASS),
        Err(CryptoError::UnsupportedVersion(1))
    ));

    // Out-of-bounds KDF cost (memory exhaustion attempt) is rejected up front.
    let mut bad_kdf = archive.clone();
    bad_kdf.header.kdf.m_kib = u32::MAX;
    assert!(matches!(
        decrypt_payload(&bad_kdf, PASS),
        Err(CryptoError::InvalidPayload)
    ));

    let mut bad_hex = archive;
    bad_hex.ciphertext_hex = "zz".to_string();
    assert!(matches!(
        decrypt_payload(&bad_hex, PASS),
        Err(CryptoError::InvalidPayload)
    ));
}

// ---------------------------------------------------------------- device pool

#[tokio::test]
async fn device_pool_enforces_two_slots_max() {
    let pool = DevicePool::new();
    let d0 = mock("android-001", DeviceSlot::Slot0, Platform::Android);
    let d1 = mock("apple-002", DeviceSlot::Slot1, Platform::Apple);
    let d2 = mock("android-003", DeviceSlot::Slot0, Platform::Android);

    let s0 = pool.attach_device(d0).await.expect("slot 0 attach");
    let s1 = pool.attach_device(d1).await.expect("slot 1 attach");
    assert_eq!(s0, DeviceSlot::Slot0);
    assert_eq!(s1, DeviceSlot::Slot1);

    let err = pool
        .attach_device(d2)
        .await
        .expect_err("third device must fail");
    assert!(matches!(err, PoolError::PoolFull));
}

#[tokio::test]
async fn device_pool_honors_declared_slot_and_rejects_duplicates() {
    let pool = DevicePool::new();
    pool.attach_device(mock("android-001", DeviceSlot::Slot1, Platform::Android))
        .await
        .expect("attach into the slot the device was built for");

    let occupied = pool
        .attach_device(mock("android-002", DeviceSlot::Slot1, Platform::Android))
        .await
        .expect_err("slot 1 is taken");
    assert!(matches!(
        occupied,
        PoolError::SlotOccupied(DeviceSlot::Slot1)
    ));

    let duplicate = pool
        .attach_device(mock("android-001", DeviceSlot::Slot0, Platform::Android))
        .await
        .expect_err("same device id twice");
    assert!(matches!(duplicate, PoolError::DuplicateDevice(_)));

    let slots = pool.list_slots().await;
    assert!(!slots[0].occupied);
    assert!(slots[1].occupied);
}

// ---------------------------------------------------------------- validators

#[test]
fn serial_and_udid_validators_reject_injection_shapes() {
    for bad in ["", "a b", "a;b", "a$(id)", "a|b", "../x", "a\nb", "é"] {
        assert!(
            validate_serial(bad).is_err(),
            "serial {bad:?} must be rejected"
        );
    }
    assert!(validate_serial(&"a".repeat(129)).is_err());
    assert!(validate_serial("192.168.1.5:5555").is_ok());
    assert!(validate_serial("R58M1234ABC").is_ok());

    for bad in ["", "a b", "a;b", "a:b", "../x"] {
        assert!(validate_udid(bad).is_err(), "udid {bad:?} must be rejected");
    }
    assert!(validate_udid(&"a".repeat(65)).is_err());
    assert!(validate_udid("00008030-001A2C3E0A38802E").is_ok());
}

#[test]
fn parsing_helpers_handle_real_world_shapes() {
    let battery = "Current Battery Service state:\n  level: 87\n  temperature: 290\n";
    assert_eq!(find_value(battery, "level"), Some("87"));
    assert_eq!(find_u64(battery, "temperature"), Some(290));
    assert_eq!(find_value(battery, "levelX"), None);

    let df = "Filesystem     1K-blocks     Used Available Use% Mounted on\n/dev/block/dm-9 112000000 56000000  56000000  50% /data\n";
    assert_eq!(parse_df_kb(df), Some((112_000_000, 56_000_000)));
    assert_eq!(parse_df_kb(""), None);

    // idevicediagnostics prints XML plist.
    let plist = "<dict>\n\t<key>CycleCount</key>\n\t<integer>412</integer>\n\t<key>Temperature</key>\n\t<real>30.5</real>\n</dict>";
    assert_eq!(find_plist_u64(plist, "CycleCount"), Some(412));
    assert_eq!(find_plist_u64(plist, "Temperature"), None); // <real>, not <integer>
    assert_eq!(find_plist_u64(plist, "Missing"), None);
}

// ---------------------------------------------------------------- archive I/O

#[test]
fn sanitize_component_strips_ads_and_separators() {
    assert_eq!(sanitize_component("192.168.1.5:5555"), "192.168.1.5_5555");
    assert_eq!(sanitize_component("a/b\\c"), "a_b_c");
    assert_eq!(sanitize_component("R58M-1_2.x"), "R58M-1_2.x");
}

#[tokio::test]
async fn archive_io_is_atomic_confined_and_verified() {
    let dir = tempfile::tempdir().expect("tempdir");
    let base = dir.path().join("backups");

    // A real archive written through the atomic writer.
    let archive = encrypt_payload(b"contacts-export", PASS, &meta()).expect("encrypt");
    let serialized = serde_json::to_vec_pretty(&archive).expect("serialize");
    let p1 = write_archive_atomic(&base, "android_x_slot0", &serialized)
        .await
        .expect("first write");
    tokio::time::sleep(Duration::from_millis(5)).await;
    let p2 = write_archive_atomic(&base, "android_x_slot0", &serialized)
        .await
        .expect("second write");
    assert_ne!(p1, p2, "a second backup must never overwrite the first");
    assert!(
        std::fs::read_dir(&base).expect("read_dir").all(|e| e
            .expect("entry")
            .path()
            .extension()
            .and_then(|x| x.to_str())
            == Some("ombackup")),
        "no temp files may remain"
    );

    // Confinement checks.
    let outside = dir.path().join("outside.ombackup");
    std::fs::write(&outside, b"x").expect("write outside");
    let wrong_ext = base.join("note.txt");
    std::fs::write(&wrong_ext, b"x").expect("write wrong ext");

    assert!(resolve_restore_path(&base, &p1).await.is_ok());
    assert!(resolve_restore_path(&base, &outside).await.is_err());
    assert!(
        resolve_restore_path(&base, &base.join("../outside.ombackup"))
            .await
            .is_err()
    );
    assert!(resolve_restore_path(&base, &wrong_ext).await.is_err());

    // Verified read: checksum, platform and authentication.
    let good_sha = omnimobile_toolkit::core::crypto::compute_sha256(&serialized);
    let verified = read_and_verify_archive(&p1, "android", PASS, Some(&good_sha))
        .await
        .expect("verify");
    assert_eq!(verified.plaintext.as_slice(), b"contacts-export");
    assert_eq!(verified.sha256_hex, good_sha);

    let bad_sha = "0".repeat(64);
    assert!(matches!(
        read_and_verify_archive(&p1, "android", PASS, Some(&bad_sha)).await,
        Err(PalError::ChecksumMismatch { .. })
    ));
    assert!(matches!(
        read_and_verify_archive(&p1, "apple", PASS, None).await,
        Err(PalError::UnsupportedState(_))
    ));
    assert!(matches!(
        read_and_verify_archive(&p1, "android", "wrong-passphrase-xyz", None).await,
        Err(PalError::Crypto(_))
    ));

    // Malformed JSON is a transport-level error, not a panic.
    let garbage = base.join("garbage.ombackup");
    std::fs::write(&garbage, b"{not json").expect("write garbage");
    assert!(read_and_verify_archive(&garbage, "android", PASS, None)
        .await
        .is_err());
}

// ---------------------------------------------------------------- subprocess runner

#[cfg(unix)]
#[tokio::test]
async fn exec_enforces_timeout() {
    use omnimobile_toolkit::core::exec::exec;

    let err = exec("sleep", &["5"], Duration::from_millis(200))
        .await
        .expect_err("must time out");
    assert!(matches!(err, PalError::Transport(ref m) if m.contains("timed out")));
}
