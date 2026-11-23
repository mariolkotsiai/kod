# Security Policy

## Supported Versions

| Version        | Supported                  |
| -------------- | -------------------------- |
| 0.1.x          | :white_check_mark: best effort (pre-release) |
| < 0.1          | :x:                        |

## Threat Model & Guarantees (what the code does today)
- **Data-at-rest:** Backup archives (format v2) use AES-256-GCM with a key derived via Argon2id
  (64 MiB, 3 iterations, 1 lane, random 16-byte salt per archive). The complete archive header
  (version, KDF parameters, salt, nonce, device, platform, mode, timestamp) is authenticated as
  AEAD associated data. Archives are written atomically (temp file, fsync, rename), never overwrite
  an existing backup, and are owner-only (`0600`) on Unix.
- **Memory hygiene:** Derived keys, decrypted payloads and backup plaintext buffers are overwritten
  on drop (`ZeroizeOnDrop` / `Zeroizing`). Limitation: passphrases travel from the webview to the
  backend as JavaScript strings, which cannot be reliably wiped from webview memory.
- **Execution isolation:** External tools (`adb`, `fastboot`, `ideviceinfo`, `idevicediagnostics`,
  `idevicebackup2`, `idevicepair`) are started with discrete argument arrays (no host shell),
  from an allowlist, with validated device identifiers and a hard timeout. Note that
  `adb shell <args>` is re-parsed by the shell on the device, so user-controlled strings must never
  be passed after `shell`.
- **IPC surface:** Strict Content-Security-Policy, and only the explicitly listed app commands are
  callable from the webview (`build.rs` app manifest + `capabilities/default.json`).
- **Restore input:** Restore paths are canonicalized and confined to the application's backup
  directory, must have the `.ombackup` extension and are size-capped.

## Known Limitations
- Restore to the device and the `FullData` / `FullImage` backup modes are not implemented yet;
  they return an explicit error instead of reporting success.
- `adb` / `fastboot` / libimobiledevice tools are resolved from `PATH` until they are bundled as
  pinned, hash-verified sidecars.
- The companion APK is only sideloaded when its SHA-256 matches `EXPECTED_APK_SHA256`
  (empty by default, i.e. sideloading is refused).

## Known Dependency Advisories
Status as of 2026-11-22 (`cargo audit` 0.22.2, RustSec database of 1560 advisories): **0 vulnerabilities**
and 6 informational warnings. CI runs `cargo audit` and reports them without failing the build; they are
listed here instead of being silently ignored.

| Crate | Advisory | Kind | Pulled in by | Status |
| --- | --- | --- | --- | --- |
| `chrono` 0.4.45 | RUSTSEC-2026-0543 | unmaintained | direct dependency | Used in two places (backup file name timestamp, archive header `created_at`). The advisory suggests `jiff` or `time`. Candidate for replacement. |
| `lru` 0.12.5 | RUSTSEC-2026-0002 | unsound (`IterMut::next` / `next_back`) | `ratatui` 0.27 (TUI only) | Fixed in `lru` >= 0.16.3. Whether ratatui calls `IterMut` is not verified. Depends on a ratatui release that uses a patched `lru`. |
| `lru` 0.12.5 | RUSTSEC-2026-0253 | unsound (`LruCache::pop` panic safety) | `ratatui` 0.27 (TUI only) | Fixed in `lru` >= 0.18.2. Exploitation needs unwinding panics, `catch_unwind` and keys with a panicking `Drop`. Depends on a ratatui release that uses a patched `lru`. |
| `paste` 1.0.15 | RUSTSEC-2024-0436 | unmaintained | `ratatui` 0.27 | Proc-macro: runs at compile time only. |
| `glib` 0.18.5 | RUSTSEC-2024-0429 | unsound (`VariantStrIter`) | Tauri's GTK3 backend (`gtk`, `webkit2gtk`, `wry`, `tao`) | Fixed in `glib` >= 0.20.0, which Tauri 2.12.7 does not use. Not under this project's control. |
| `proc-macro-error` 1.0.4 | RUSTSEC-2024-0370 | unmaintained | `glib-macros`, `gtk3-macros` (Tauri's GTK3 backend) | Proc-macro: runs at compile time only. Not under this project's control. |

The GTK3 stack is Tauri's Linux backend.

## Reporting a Vulnerability
Do **NOT** open public GitHub issues for security vulnerabilities.
1. Use GitHub's **Private Vulnerability Reporting** ("Security" tab -> "Report a vulnerability").
2. Expect an initial acknowledgment within 48 hours and a remediation timeline within 7 days.
