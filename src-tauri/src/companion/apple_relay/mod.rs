use crate::core::exec::{exec, exec_raw, DEFAULT_TIMEOUT};
use crate::core::pal::PalError;

/// Only these libimobiledevice tools may be launched through [`run_imobile_cmd`].
const ALLOWED_PROGRAMS: [&str; 4] = [
    "ideviceinfo",
    "idevicediagnostics",
    "idevicebackup2",
    "idevicepair",
];

/// Validates Apple UDID format (alphanumeric and dash only) to prevent argument/path abuse.
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

/// Runs an allowlisted libimobiledevice CLI tool with discrete arguments (no shell) and a timeout.
pub async fn run_imobile_cmd(program: &str, udid: &str, args: &[&str]) -> Result<String, PalError> {
    if !ALLOWED_PROGRAMS.contains(&program) {
        return Err(PalError::Transport(format!(
            "Program '{program}' is not allowlisted"
        )));
    }
    let clean_udid = validate_udid(udid)?;

    let mut full: Vec<&str> = Vec::with_capacity(args.len() + 2);
    full.push("-u");
    full.push(clean_udid);
    full.extend_from_slice(args);

    exec(program, &full, DEFAULT_TIMEOUT).await
}

/// Verifies lockdownd pairing record & SSL certificate trust prior to transferring data.
pub async fn verify_lockdownd_pairing(udid: &str) -> Result<(), PalError> {
    let clean_udid = validate_udid(udid)?;

    // exec_raw: a timeout / missing binary stays a Transport error; only a real
    // "validate" failure is reported as an untrusted peer.
    let out = exec_raw(
        "idevicepair",
        &["-u", clean_udid, "validate"],
        DEFAULT_TIMEOUT,
    )
    .await?;

    if !out.success {
        tracing::debug!(stderr = %out.stderr, "idevicepair validate failed");
        return Err(PalError::UntrustedPeer(format!(
            "lockdownd pairing/SSL validation failed for UDID {clean_udid}"
        )));
    }

    Ok(())
}
