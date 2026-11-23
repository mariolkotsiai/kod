//! Security-boundary tests (Phase 1): subprocess runner, program / serial / UDID allowlists,
//! per-slot locking and the archive input checks that guard the restore command.
//!
//! None of these tests needs a phone or `adb`. The only external process they start is `rustc`
//! (always installed wherever `cargo test` runs) or a tool that is rejected BEFORE it is spawned.

use omnimobile_toolkit::companion::apple_relay::run_imobile_cmd;
use omnimobile_toolkit::core::archive_io::{
    read_and_verify_archive, resolve_restore_path, write_archive_atomic, MAX_ARCHIVE_BYTES,
};
use omnimobile_toolkit::core::crypto::{compute_sha256, encrypt_payload, ArchiveMeta};
use omnimobile_toolkit::core::device_pool::DevicePool;
use omnimobile_toolkit::core::exec::{exec, exec_raw};
use omnimobile_toolkit::core::pal::{DeviceSlot, PalError};
use omnimobile_toolkit::protocols::android::executor::{
    fastboot_has_device, run_adb, run_fastboot, verify_adb_host_trust,
};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::timeout;

// ---------------------------------------------------------------- shared helpers

const PASS: &str = "strong-passphrase-123";
/// Helper tool used for the real-process tests: present on every machine that runs `cargo test`.
const TOOL: &str = "rustc";
/// Generous bound for tools that return immediately (only protects against a hung test run).
const LIMIT: Duration = Duration::from_secs(30);

/// A bad-input rejection must happen BEFORE any process is started: the error may be neither a
/// spawn failure ("Failed to execute") nor a timeout ("timed out").
fn assert_rejected_before_spawn(err: PalError, context: &str) {
    match err {
        PalError::Transport(msg) => assert!(
            !msg.contains("Failed to execute") && !msg.contains("timed out"),
            "{context}: a process was started: {msg}"
        ),
        other => panic!("{context}: unexpected error {other:?}"),
    }
}

/// Writes a real encrypted `.ombackup` into `base`; returns its path and the file's SHA-256.
async fn write_test_archive(base: &Path) -> (PathBuf, String) {
    let meta = ArchiveMeta {
        device_id: "android-001",
        platform: "android",
        mode: "selective",
    };
    let archive = encrypt_payload(b"contacts-export", PASS, &meta).expect("encrypt");
    let serialized = serde_json::to_vec_pretty(&archive).expect("serialize");
    let path = write_archive_atomic(base, "android_x_slot0", &serialized)
        .await
        .expect("write archive");
    (path, compute_sha256(&serialized))
}

/// A command that runs for ~5 s on the current OS (used to prove the timeout fires early).
#[cfg(not(windows))]
fn long_running_command() -> (&'static str, &'static [&'static str]) {
    ("sleep", &["5"])
}

#[cfg(windows)]
fn long_running_command() -> (&'static str, &'static [&'static str]) {
    // `timeout.exe` refuses redirected input and `cmd /c` is a shell, so use ping as a sleeper.
    ("ping", &["-n", "6", "127.0.0.1"])
}

// ---------------------------------------------------------------- SHA-256 format

#[test]
fn sha256_matches_known_vectors() {
    // The digest the UI sends as `expected_sha256` must be exactly this lowercase hex format.
    assert_eq!(
        compute_sha256(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(
        compute_sha256(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

// ---------------------------------------------------------------- subprocess runner (core/exec.rs)

#[tokio::test]
async fn exec_returns_trimmed_stdout_on_success() {
    let out = exec(TOOL, &["--version"], LIMIT)
        .await
        .expect("rustc --version must succeed");
    assert!(out.starts_with("rustc "), "unexpected output: {out:?}");
    assert_eq!(out, out.trim(), "stdout must come back trimmed");
}

#[tokio::test]
async fn exec_passes_arguments_literally_without_a_shell() {
    // A shell would treat `;` as a command separator and run `echo INJECTED`.
    let payload = "no-such-kind; echo INJECTED";
    let out = exec_raw(TOOL, &["--print", payload], LIMIT)
        .await
        .expect("rustc must start");

    assert!(!out.success, "rustc must reject the unknown print kind");
    assert!(
        out.stderr.contains(payload),
        "the argument must arrive as ONE literal string; stderr was: {}",
        out.stderr
    );
    assert!(
        !out.stdout.contains("INJECTED"),
        "no second command may run; stdout was: {}",
        out.stdout
    );
}

#[tokio::test]
async fn exec_error_does_not_forward_raw_stderr() {
    let marker = "no-such-kind-7f3a";

    // Premise: the tool itself reports the marker on stderr...
    let raw = exec_raw(TOOL, &["--print", marker], LIMIT)
        .await
        .expect("rustc must start");
    assert!(!raw.success);
    assert!(
        raw.stderr.contains(marker),
        "test premise: rustc must echo the bad argument, stderr was: {}",
        raw.stderr
    );

    // ...but exec() must turn the failure into a generic error without any stderr content.
    let err = exec(TOOL, &["--print", marker], LIMIT)
        .await
        .expect_err("a non-zero exit must be an error");
    match err {
        PalError::Transport(msg) => {
            assert!(msg.contains("non-zero"), "unexpected message: {msg}");
            assert!(!msg.contains(marker), "stderr leaked into the error: {msg}");
        }
        other => panic!("expected PalError::Transport, got {other:?}"),
    }
}

#[tokio::test]
async fn exec_reports_a_missing_program_as_transport_error() {
    let err = exec("omnimobile-definitely-not-installed", &[], LIMIT)
        .await
        .expect_err("a missing program must fail");
    assert!(
        matches!(err, PalError::Transport(ref m) if m.contains("Failed to execute")),
        "unexpected error: {err:?}"
    );
}

#[tokio::test]
async fn exec_timeout_returns_promptly_on_every_platform() {
    // The existing `exec_enforces_timeout` test is unix-only; this one also runs on Windows.
    let (program, args) = long_running_command();
    let started = Instant::now();
    let err = exec(program, args, Duration::from_millis(300))
        .await
        .expect_err("must time out");

    assert!(
        matches!(err, PalError::Transport(ref m) if m.contains("timed out")),
        "unexpected error: {err:?}"
    );
    // The command would run ~5 s. Returning well before that proves we did not wait for it.
    assert!(
        started.elapsed() < Duration::from_secs(4),
        "exec waited for the child: {:?}",
        started.elapsed()
    );
}

// ---------------------------------------------------------------- program / serial / UDID allowlists

#[tokio::test]
async fn imobile_runner_rejects_non_allowlisted_programs_before_spawning() {
    let udid = "00008030-001A2C3E0A38802E";
    let programs = [
        "rm",
        "sh",
        "cmd",
        "adb",
        "ideviceinfo.exe",
        "IDEVICEINFO",
        "../ideviceinfo",
        "ideviceinfo; id",
        "",
    ];
    for program in programs {
        let err = run_imobile_cmd(program, udid, &[])
            .await
            .expect_err("a program outside the allowlist must be rejected");
        assert!(
            matches!(err, PalError::Transport(ref m) if m.contains("not allowlisted")),
            "{program:?} -> {err:?}"
        );
    }
}

#[tokio::test]
async fn imobile_runner_validates_the_udid_before_spawning() {
    for bad in ["", "a b", "a;b", "$(id)", "../x", "a\nb"] {
        let err = run_imobile_cmd("ideviceinfo", bad, &[])
            .await
            .expect_err("a malformed UDID must be rejected");
        assert!(
            matches!(err, PalError::Transport(ref m) if m.contains("UDID")),
            "{bad:?} -> {err:?}"
        );
    }
}

#[tokio::test]
async fn adb_helpers_reject_injection_shaped_serials_before_spawning() {
    for bad in ["", "a b", "a;b", "$(id)", "a|b", "../x", "a\nb", "é"] {
        let err = run_adb(bad, &["devices"])
            .await
            .expect_err("run_adb must reject");
        assert_rejected_before_spawn(err, &format!("run_adb({bad:?})"));

        let err = run_fastboot(bad, &["devices"])
            .await
            .expect_err("run_fastboot must reject");
        assert_rejected_before_spawn(err, &format!("run_fastboot({bad:?})"));

        let err = fastboot_has_device(bad)
            .await
            .expect_err("fastboot_has_device must reject");
        assert_rejected_before_spawn(err, &format!("fastboot_has_device({bad:?})"));

        let err = verify_adb_host_trust(bad)
            .await
            .expect_err("verify_adb_host_trust must reject");
        assert_rejected_before_spawn(err, &format!("verify_adb_host_trust({bad:?})"));
    }
}

// ---------------------------------------------------------------- per-slot operation lock

#[tokio::test]
async fn lock_slot_blocks_a_second_operation_on_the_same_slot() {
    let pool = DevicePool::new();
    let guard = pool.lock_slot(DeviceSlot::Slot0).await;

    // While the first operation holds the slot, a second one has to wait.
    let blocked = timeout(
        Duration::from_millis(150),
        pool.lock_slot(DeviceSlot::Slot0),
    )
    .await;
    assert!(
        blocked.is_err(),
        "a second lock on the same slot must not be granted"
    );

    // Releasing the guard lets the next operation in.
    drop(guard);
    let acquired = timeout(Duration::from_secs(5), pool.lock_slot(DeviceSlot::Slot0)).await;
    assert!(
        acquired.is_ok(),
        "the slot must be lockable again after release"
    );
}

#[tokio::test]
async fn lock_slot_does_not_block_the_other_slot() {
    let pool = DevicePool::new();
    let _slot0 = pool.lock_slot(DeviceSlot::Slot0).await;

    let other = timeout(Duration::from_secs(5), pool.lock_slot(DeviceSlot::Slot1)).await;
    assert!(
        other.is_ok(),
        "slot 1 must stay available while slot 0 is busy"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn lock_slot_never_lets_two_operations_overlap_on_one_slot() {
    let pool = Arc::new(DevicePool::new());
    let active = Arc::new(AtomicUsize::new(0));
    let max_seen = Arc::new(AtomicUsize::new(0));

    let mut tasks = Vec::new();
    for _ in 0..8 {
        let (pool, active, max_seen) = (
            Arc::clone(&pool),
            Arc::clone(&active),
            Arc::clone(&max_seen),
        );
        tasks.push(tokio::spawn(async move {
            let _guard = pool.lock_slot(DeviceSlot::Slot0).await;
            let now = active.fetch_add(1, Ordering::SeqCst) + 1;
            max_seen.fetch_max(now, Ordering::SeqCst);
            tokio::time::sleep(Duration::from_millis(10)).await;
            active.fetch_sub(1, Ordering::SeqCst);
        }));
    }
    for task in tasks {
        task.await.expect("task must not panic");
    }

    assert_eq!(
        max_seen.load(Ordering::SeqCst),
        1,
        "operations on one slot must run strictly one at a time"
    );
}

// ---------------------------------------------------------------- archive input checks (restore path)

#[tokio::test]
async fn expected_sha256_is_trimmed_and_case_insensitive() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (path, sha) = write_test_archive(&dir.path().join("backups")).await;

    // The UI may send the digest in uppercase and with surrounding whitespace.
    let decorated = format!("  {}\n", sha.to_ascii_uppercase());
    let ok = read_and_verify_archive(&path, "android", PASS, Some(&decorated))
        .await
        .expect("a decorated but correct digest must verify");
    assert_eq!(ok.sha256_hex, sha);

    // A mismatch is reported (lowercase) BEFORE decryption: the wrong passphrase is never tried.
    let wrong = "AB".repeat(32);
    match read_and_verify_archive(&path, "android", "wrong-passphrase-xyz", Some(&wrong)).await {
        Err(PalError::ChecksumMismatch { expected, actual }) => {
            assert_eq!(expected, wrong.to_ascii_lowercase());
            assert_eq!(actual, sha);
        }
        other => panic!(
            "expected ChecksumMismatch before decryption, got {:?}",
            other.map(|verified| verified.sha256_hex)
        ),
    }
}

#[tokio::test]
async fn oversized_archives_are_rejected_before_being_read() {
    let dir = tempfile::tempdir().expect("tempdir");
    let base = dir.path().join("backups");
    std::fs::create_dir_all(&base).expect("create base dir");

    // One byte over the cap. `set_len` only resizes the file; no data is written or read.
    let big = base.join("big.ombackup");
    let file = std::fs::File::create(&big).expect("create file");
    file.set_len(MAX_ARCHIVE_BYTES + 1).expect("resize file");
    drop(file);

    assert!(matches!(
        resolve_restore_path(&base, &big).await,
        Err(PalError::UnsupportedState(_))
    ));
    assert!(matches!(
        read_and_verify_archive(&big, "android", PASS, None).await,
        Err(PalError::UnsupportedState(_))
    ));
}

#[tokio::test]
async fn restore_path_violations_are_reported_as_unsupported_state() {
    let dir = tempfile::tempdir().expect("tempdir");
    let base = dir.path().join("backups");
    let (inside, _) = write_test_archive(&base).await;

    // Control: a real .ombackup inside the backup directory is accepted.
    assert!(resolve_restore_path(&base, &inside).await.is_ok());

    // 1. A valid-looking file outside the base directory.
    let outside = dir.path().join("outside.ombackup");
    std::fs::write(&outside, b"x").expect("write outside file");
    // 2. `..` traversal that lands on that outside file.
    let traversal = base.join("..").join("outside.ombackup");
    // 3. A sibling directory whose NAME merely starts with the base name (string-prefix bug).
    let evil_dir = dir.path().join("backups_evil");
    std::fs::create_dir_all(&evil_dir).expect("create sibling dir");
    let sibling = evil_dir.join("x.ombackup");
    std::fs::write(&sibling, b"x").expect("write sibling file");
    // 4. Right directory, wrong extension.
    let wrong_ext = base.join("note.txt");
    std::fs::write(&wrong_ext, b"x").expect("write wrong-extension file");

    let cases = [
        ("outside", &outside),
        ("traversal", &traversal),
        ("sibling-prefix", &sibling),
        ("wrong extension", &wrong_ext),
    ];
    for (label, candidate) in cases {
        assert!(
            matches!(
                resolve_restore_path(&base, candidate).await,
                Err(PalError::UnsupportedState(_))
            ),
            "{label} must be rejected as UnsupportedState"
        );
    }

    // A path that does not exist is an I/O (transport) error, not a confinement verdict.
    let missing = base.join("missing.ombackup");
    assert!(matches!(
        resolve_restore_path(&base, &missing).await,
        Err(PalError::Transport(_))
    ));
}

// ---------------------------------------------------------------- unix-only guarantees
// Symlinks need elevated rights on Windows and file modes do not exist there, so these two run on
// Linux/macOS only (they do not run in the current windows-latest CI job).

#[cfg(unix)]
#[tokio::test]
async fn symlinks_cannot_escape_the_backup_directory() {
    let dir = tempfile::tempdir().expect("tempdir");
    let base = dir.path().join("backups");
    std::fs::create_dir_all(&base).expect("create base dir");

    let secret = dir.path().join("secret.ombackup");
    std::fs::write(&secret, b"x").expect("write secret");
    let link = base.join("link.ombackup");
    std::os::unix::fs::symlink(&secret, &link).expect("create symlink");

    assert!(matches!(
        resolve_restore_path(&base, &link).await,
        Err(PalError::UnsupportedState(_))
    ));
}

#[cfg(unix)]
#[tokio::test]
async fn archives_are_owner_only_on_unix() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().expect("tempdir");
    let (path, _) = write_test_archive(&dir.path().join("backups")).await;

    let mode = std::fs::metadata(&path)
        .expect("stat archive")
        .permissions()
        .mode();
    assert_eq!(
        mode & 0o777,
        0o600,
        "backups must be readable by the owner only"
    );
}
