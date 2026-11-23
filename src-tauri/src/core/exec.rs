//! Hardened subprocess execution: no shell, hard timeout, child killed when the future is dropped.

use crate::core::pal::PalError;
use std::time::Duration;
use tokio::{process::Command, time::timeout};

/// Default upper bound for a single adb / libimobiledevice invocation.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Captured result of a finished subprocess (stdout/stderr already trimmed).
#[derive(Debug, Clone)]
pub struct ExecOutput {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

/// Runs `program args...` (no shell) and returns its captured output, even for non-zero exit codes.
///
/// * `timeout`: a hung tool (e.g. `fastboot` waiting for a device) can no longer block forever.
/// * `kill_on_drop`: when the timeout drops the future, the child process is killed (no orphans).
pub async fn exec_raw(
    program: &str,
    args: &[&str],
    limit: Duration,
) -> Result<ExecOutput, PalError> {
    let mut cmd = Command::new(program);
    cmd.args(args).kill_on_drop(true);

    let out = timeout(limit, cmd.output())
        .await
        .map_err(|_| PalError::Transport(format!("{program} timed out after {limit:?}")))?
        .map_err(|e| PalError::Transport(format!("Failed to execute {program}: {e}")))?;

    Ok(ExecOutput {
        success: out.status.success(),
        stdout: String::from_utf8_lossy(&out.stdout).trim().to_string(),
        stderr: String::from_utf8_lossy(&out.stderr).trim().to_string(),
    })
}

/// Like [`exec_raw`] but maps a non-zero exit status to an error and returns stdout on success.
/// Raw stderr is logged at debug level only; it is never forwarded to the UI.
pub async fn exec(program: &str, args: &[&str], limit: Duration) -> Result<String, PalError> {
    let out = exec_raw(program, args, limit).await?;
    if !out.success {
        tracing::debug!(program, stderr = %out.stderr, "subprocess exited with failure");
        return Err(PalError::Transport(format!(
            "{program} exited with a non-zero status"
        )));
    }
    Ok(out.stdout)
}
