use crate::core::pal::PalError;
use tokio::process::Command;

/// Validates serial numbers so only safe alphanumeric, dash, underscore, dot, or colon chars pass.
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

/// Executes `adb -s <serial> <args...>` directly without invoking a shell interpreter.
pub async fn run_adb(serial: &str, args: &[&str]) -> Result<String, PalError> {
    let clean_serial = validate_serial(serial)?;

    // Strictly pass discrete arguments via Command::args to prevent command injection.
    let mut cmd = Command::new("adb");
    cmd.arg("-s").arg(clean_serial).args(args);

    let output = cmd
        .output()
        .await
        .map_err(|e| PalError::Transport(format!("Failed to execute adb: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(PalError::Transport(format!("adb command failed: {stderr}")));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Executes `fastboot -s <serial> <args...>` directly without shell interpolation.
pub async fn run_fastboot(serial: &str, args: &[&str]) -> Result<String, PalError> {
    let clean_serial = validate_serial(serial)?;

    let mut cmd = Command::new("fastboot");
    cmd.arg("-s").arg(clean_serial).args(args);

    let output = cmd
        .output()
        .await
        .map_err(|e| PalError::Transport(format!("Failed to execute fastboot: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(PalError::Transport(format!(
            "fastboot command failed: {stderr}"
        )));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Verifies that the target Android device has accepted the RSA host key (`device` state).
pub async fn verify_adb_host_trust(serial: &str) -> Result<(), PalError> {
    let clean_serial = validate_serial(serial)?;
    let output = Command::new("adb")
        .arg("devices")
        .output()
        .await
        .map_err(|e| PalError::Transport(format!("Failed to query adb devices: {e}")))?;

    if !output.status.success() {
        return Err(PalError::Transport("adb devices check failed".to_string()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines().skip(1) {
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