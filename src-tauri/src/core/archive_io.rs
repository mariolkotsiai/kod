//! Crash-safe archive writing, confined restore paths and verified archive reads.

use crate::core::crypto::{compute_sha256, decrypt_payload, EncryptedArchive};
use crate::core::pal::PalError;
use std::path::{Path, PathBuf};
use tokio::{fs, io::AsyncWriteExt};
use zeroize::Zeroizing;

/// Upper bound for an archive that is read fully into memory (restore / verify).
pub const MAX_ARCHIVE_BYTES: u64 = 512 * 1024 * 1024;

fn io<E: std::fmt::Display>(ctx: &'static str) -> impl FnOnce(E) -> PalError {
    move |e| PalError::Transport(format!("{ctx}: {e}"))
}

/// Keeps only `[A-Za-z0-9._-]`; everything else (notably `:` which creates NTFS alternate data
/// streams, and path separators) becomes `_`.
pub fn sanitize_component(input: &str) -> String {
    input
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Writes `bytes` to `<dir>/<stem>_<UTC timestamp>.ombackup` atomically:
/// temp file (owner-only on Unix) -> fsync -> rename. An existing backup is never overwritten.
pub async fn write_archive_atomic(dir: &Path, stem: &str, bytes: &[u8]) -> Result<PathBuf, PalError> {
    fs::create_dir_all(dir).await.map_err(io("create backup dir"))?;

    let ts = chrono::Utc::now().format("%Y%m%dT%H%M%S%3fZ");
    let final_path = dir.join(format!("{stem}_{ts}.ombackup"));
    let tmp_path = dir.join(format!("{stem}_{ts}.ombackup.tmp"));

    if fs::try_exists(&final_path)
        .await
        .map_err(io("stat backup target"))?
    {
        return Err(PalError::Transport(
            "Backup target already exists".to_string(),
        ));
    }

    let mut opts = fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    opts.mode(0o600);
    let mut file = opts.open(&tmp_path).await.map_err(io("create temp archive"))?;

    let write_result = async {
        file.write_all(bytes).await?;
        // Flush to disk before the rename so a crash cannot leave a truncated "final" file.
        file.sync_all().await
    }
    .await;
    drop(file);

    if let Err(e) = write_result {
        let _ = fs::remove_file(&tmp_path).await;
        return Err(PalError::Transport(format!("write temp archive: {e}")));
    }
    if let Err(e) = fs::rename(&tmp_path, &final_path).await {
        let _ = fs::remove_file(&tmp_path).await;
        return Err(PalError::Transport(format!("finalize archive: {e}")));
    }
    Ok(final_path)
}

/// Confines a restore input to `base` (after canonicalization, so `..` and symlinks cannot escape),
/// requires the `.ombackup` extension and enforces [`MAX_ARCHIVE_BYTES`].
pub async fn resolve_restore_path(base: &Path, candidate: &Path) -> Result<PathBuf, PalError> {
    let base = fs::canonicalize(base).await.map_err(io("backup base dir"))?;
    let resolved = fs::canonicalize(candidate)
        .await
        .map_err(io("backup path"))?;

    let ext_ok = resolved.extension().and_then(|e| e.to_str()) == Some("ombackup");
    if !resolved.starts_with(&base) || !ext_ok {
        return Err(PalError::UnsupportedState(
            "Backup path must be an .ombackup file inside the backup directory".to_string(),
        ));
    }

    let len = fs::metadata(&resolved).await.map_err(io("stat backup"))?.len();
    if len > MAX_ARCHIVE_BYTES {
        return Err(PalError::UnsupportedState(
            "Backup file exceeds the maximum supported size".to_string(),
        ));
    }
    Ok(resolved)
}

/// Result of a successful archive verification.
pub struct VerifiedArchive {
    /// SHA-256 of the archive file as stored on disk.
    pub sha256_hex: String,
    /// Decrypted payload; overwritten with zeros when dropped.
    pub plaintext: Zeroizing<Vec<u8>>,
}

/// Reads an archive (bounded), compares its file SHA-256 with `expected_sha256` when given,
/// checks the platform recorded in the header and authenticates + decrypts it.
pub async fn read_and_verify_archive(
    path: &Path,
    expected_platform: &str,
    passphrase: &str,
    expected_sha256: Option<&str>,
) -> Result<VerifiedArchive, PalError> {
    let len = fs::metadata(path).await.map_err(io("stat backup"))?.len();
    if len > MAX_ARCHIVE_BYTES {
        return Err(PalError::UnsupportedState(
            "Backup file exceeds the maximum supported size".to_string(),
        ));
    }

    let raw = fs::read(path).await.map_err(io("read backup"))?;
    let sha256_hex = compute_sha256(&raw);

    if let Some(expected) = expected_sha256 {
        let expected = expected.trim();
        if !expected.eq_ignore_ascii_case(&sha256_hex) {
            return Err(PalError::ChecksumMismatch {
                expected: expected.to_ascii_lowercase(),
                actual: sha256_hex,
            });
        }
    }

    let archive: EncryptedArchive = serde_json::from_slice(&raw)
        .map_err(|e| PalError::Transport(format!("Invalid archive JSON: {e}")))?;

    if archive.header.platform != expected_platform {
        return Err(PalError::UnsupportedState(format!(
            "Archive was created for platform '{}', not '{expected_platform}'",
            archive.header.platform
        )));
    }

    let plaintext = decrypt_payload(&archive, passphrase)?;
    Ok(VerifiedArchive {
        sha256_hex,
        plaintext,
    })
}
