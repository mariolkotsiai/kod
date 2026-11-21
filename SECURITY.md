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

## Reporting a Vulnerability
Do **NOT** open public GitHub issues for security vulnerabilities.
1. Use GitHub's **Private Vulnerability Reporting** ("Security" tab -> "Report a vulnerability").
2. Alternatively, email `security@omnimobile-toolkit.org`. Encrypted reports are accepted once a
   PGP key fingerprint is published here (TODO before the first release).
3. Expect an initial acknowledgment within 48 hours and a remediation timeline within 7 days.
