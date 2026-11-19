use crate::core::pal::PalError;
use tokio::process::Command;

/// Validates Apple UDID format (hex/UUID-style characters only) to prevent argument/path abuse.
pub fn validate_udid(udid: &str) -> Result<&str, PalError> {
    if udid.is_empty() || udid.len() > 64 {
        return Err(PalError::Transport("Invalid or empty Apple UDID".to_string()));
    }
    let valid = udid
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-');
    if !valid {
        return Err(PalError::Transport(
            "Apple UDID contains disallowed characters".to_string(),
        ));
    }
    Ok(udid)
}

/// Runs a libimobiledevice CLI tool with discrete arguments (no shell interpolation).
pub async fn run_imobile_cmd(program: &str, udid: &str, args: &[&str]) -> Result<String, PalError> {
    let clean_udid = validate_udid(udid)?;

    let mut cmd = Command::new(program);
    cmd.arg("-u").arg(clean_udid).args(args);

    let output = cmd
        .output()
        .await
        .map_err(|e| PalError::Transport(format!("Failed executing {program}: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(PalError::Transport(format!("{program} failed: {stderr}")));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Verifies lockdownd pairing record & SSL certificate trust prior to transferring data.
pub async fn verify_lockdownd_pairing(udid: &str) -> Result<(), PalError> {
    let clean_udid = validate_udid(udid)?;

    let output = Command::new("idevicepair")
        .arg("-u")
        .arg(clean_udid)
        .arg("validate")
        .output()
        .await
        .map_err(|e| PalError::Transport(format!("Failed running idevicepair validate: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(PalError::UntrustedPeer(format!(
            "lockdownd pairing/SSL validation failed for UDID {clean_udid}: {stderr}"
        )));
    }

    Ok(())
}