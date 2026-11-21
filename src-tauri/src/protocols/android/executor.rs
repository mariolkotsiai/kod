use crate::core::exec::{exec, DEFAULT_TIMEOUT};
use crate::core::pal::PalError;
use std::time::Duration;

const FASTBOOT_TIMEOUT: Duration = Duration::from_secs(15);
const ADB_DEVICES_TIMEOUT: Duration = Duration::from_secs(10);

/// Validates serial numbers so only safe alphanumeric, dash, underscore, dot, or colon chars pass.
/// (`:` is needed for network adb serials like `192.168.1.5:5555`; never put a serial into a file
/// name without `archive_io::sanitize_component`.)
pub fn validate_serial(serial: &str) -> Result<&str, PalError> {
    if serial.is_empty() || serial.len() > 128 {
        return Err(PalError::Transport("Invalid or empty device serial".to_string()));
    }
    let valid = serial
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ':'));
    if !valid {
        return Err(PalError::Transport(
            "Serial contains disallowed characters".to_string(),
        ));
    }
    Ok(serial)
}

/// Executes `adb -s <serial> <args...>` without a shell, with the default timeout.
pub async fn run_adb(serial: &str, args: &[&str]) -> Result<String, PalError> {
    run_adb_timeout(serial, args, DEFAULT_TIMEOUT).await
}

/// Executes `adb -s <serial> <args...>` without a shell and a caller-chosen timeout.
pub async fn run_adb_timeout(
    serial: &str,
    args: &[&str],
    limit: Duration,
) -> Result<String, PalError> {
    let clean_serial = validate_serial(serial)?;

    // Discrete arguments only (no shell on the host). Note: `adb shell <args>` is re-parsed by the
    // device shell, so never pass user-controlled strings after "shell".
    let mut full: Vec<&str> = Vec::with_capacity(args.len() + 2);
    full.push("-s");
    full.push(clean_serial);
    full.extend_from_slice(args);

    exec("adb", &full, limit).await
}

/// Executes `fastboot -s <serial> <args...>` without a shell, with a hard timeout.
pub async fn run_fastboot(serial: &str, args: &[&str]) -> Result<String, PalError> {
    let clean_serial = validate_serial(serial)?;

    let mut full: Vec<&str> = Vec::with_capacity(args.len() + 2);
    full.push("-s");
    full.push(clean_serial);
    full.extend_from_slice(args);

    exec("fastboot", &full, FASTBOOT_TIMEOUT).await
}

/// True when `fastboot devices` lists the serial (i.e. the device is in bootloader/fastboot mode).
/// Check this before `run_fastboot`, otherwise fastboot waits for a device that never shows up.
pub async fn fastboot_has_device(serial: &str) -> Result<bool, PalError> {
    let clean_serial = validate_serial(serial)?;
    let out = exec("fastboot", &["devices"], FASTBOOT_TIMEOUT).await?;
    Ok(out
        .lines()
        .any(|line| line.split_whitespace().next() == Some(clean_serial)))
}

/// Verifies that the target Android device has accepted the RSA host key (`device` state).
pub async fn verify_adb_host_trust(serial: &str) -> Result<(), PalError> {
    let clean_serial = validate_serial(serial)?;
    let stdout = exec("adb", &["devices"], ADB_DEVICES_TIMEOUT).await?;

    // No `.skip(1)`: the header and daemon-start lines never have the serial as first column.
    for line in stdout.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 && parts[0] == clean_serial {
            return match parts[1] {
                "device" => Ok(()),
                "unauthorized" => Err(PalError::UntrustedPeer(
                    "ADB RSA host key is unauthorized on device".to_string(),
                )),
                other => Err(PalError::UntrustedPeer(format!(
                    "Unexpected ADB device state: {other}"
                ))),
            };
        }
    }

    Err(PalError::Transport(format!(
        "Device serial '{clean_serial}' not found in ADB list"
    )))
}
