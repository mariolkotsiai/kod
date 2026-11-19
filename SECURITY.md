# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 1.x.x   | :white_check_mark: |
| < 1.0   | :x:                |

## Threat Model & Cryptographic Guarantees
- **Data-at-Rest:** All backup archives are encrypted using AES-256-GCM with keys derived via Argon2id.
- **Memory Hygiene:** Sensitive buffers, pairing certificates, and derived keys implement `ZeroizeOnDrop`.
- **Execution Isolation:** External binaries (`adb`, `fastboot`, `idevicebackup2`) are invoked strictly via discrete argument arrays without shell interpolation.

## Reporting a Vulnerability
Do **NOT** open public GitHub issues for security vulnerabilities.
1. Use GitHub's **Private Vulnerability Reporting** ("Security" tab -> "Report a vulnerability").
2. Alternatively, encrypt your report using our PGP key and send it to `security@omnimobile-toolkit.org`.
3. Expect an initial acknowledgment within 48 hours and a remediation timeline within 7 days.