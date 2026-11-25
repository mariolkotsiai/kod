# Kernel Overview & Diagnostics - KOD

**Back up your phone and move your data from one phone to another, over a USB cable, on your own computer.**
No cloud account, no app store install, no upload anywhere. Works with Android and iPhone, and with
two phones connected at the same time.

"Kernel Overview" means reading information from your phone (system version, storage, battery,
hardware). KOD never modifies the kernel, the bootloader or the system partitions of your phone.
See [What it will never do](#what-it-will-never-do).

> [!WARNING]
> **Pre-release (0.1.x). Not ready for use with real data.**
> The security foundation (encrypted archives, safe tool execution, CI checks) is in place, but phones
> cannot be connected from the app yet. Do not rely on this software for backups until a release
> says otherwise. See [What works today](#what-works-today).

## Why

Moving to a new phone or keeping a copy of your data usually means a cloud account (Google Drive,
iCloud, OneDrive), an app from the Play Store or App Store, a subscription, or a slow Wi-Fi transfer.
KOD does it locally:

- **Local-first.** Your data goes from the phone to your computer (or to the second phone) and nowhere
  else. There is no KOD server.
- **Nothing to install from an app store.** The phone only needs the settings that Android and Apple
  already provide for computer access.
- **Encrypted by default.** Backup archives are encrypted with a passphrase only you know
  (AES-256-GCM, key derived with Argon2id).
- **Two phones at once.** Connect an old and a new phone and transfer directly, including between
  Android and iPhone.
- **Open source** (Apache-2.0), with a public [security policy](SECURITY.md).

## Planned features

| Feature | Android | iPhone |
|---|---|---|
| Photos and videos | Planned | Planned (see [Things to know](#things-to-know)) |
| Contacts | Planned | Planned |
| Messages (SMS) | Planned, view/export only | Planned, view/export only |
| Phone-to-phone transfer (Android and iPhone, any direction) | Planned | Planned |
| Full device backup | Not possible without root (app data is protected by Android) | Planned (standard local iPhone backup) |
| Battery health, storage, device info | Planned | Planned |
| Hardware tests (sensors, touch screen) | Planned | Not planned (would need an Apple-signed app on the phone) |
| Restore to the phone | Planned | Planned |

Both a desktop window and a keyboard-driven terminal mode (`--tui`) are planned.

## What it will never do

- Root, jailbreak, or unlock the bootloader of your phone.
- Bypass a screen lock, Google account protection (FRP), Activation Lock, or any other protection.
- Access a phone that you have not unlocked and approved yourself.
- Upload your data anywhere or require an account.
- Dump raw disk or partition images.

You may only use KOD on devices you own or are authorized to manage.

## What works today

| Area | Status |
|---|---|
| Encrypted backup archive format (`.ombackup`, AES-256-GCM, Argon2id) | Implemented and tested |
| Safe execution of `adb` / libimobiledevice tools (no shell, allowlist, timeouts) | Implemented and tested |
| Connecting a phone from the app | **Not yet** |
| Backup, transfer, restore with a real phone | **Not yet** |
| Windows installer (not code-signed yet) | Built automatically for each release, see [Installing KOD](#installing-kod) |

## Requirements

Download every tool below **only from the official source listed**. Never use mirrors, "driver packs"
or re-uploads.

### Your computer

- **Windows 10 or 11, 64-bit.** The minimum Windows build will be confirmed with the first installer.
- **Microsoft Edge WebView2 Runtime.** Already included in Windows 11. If it is missing, the installer
  downloads it from Microsoft, so the first installation needs an internet connection.

### For Android phones

1. On the phone, enable **Developer options** and **USB debugging**
   ([official guide](https://developer.android.com/studio/debug/dev-options)).
2. Connect the cable and accept the **"Allow USB debugging?"** prompt on the phone.
3. **Android SDK Platform-Tools** (`adb`) from Google:
   https://developer.android.com/tools/releases/platform-tools
   Whether KOD will ship `adb` itself is still being decided. Until then, install it yourself and
   add it to your `PATH`.
4. A **USB driver**, only if Windows does not recognize the phone:
   https://developer.android.com/studio/run/oem-usb (links to each manufacturer's official driver).

You can turn USB debugging off again when you are done.

### For iPhones

1. **Apple Devices** app from the Microsoft Store: open the Microsoft Store, search for "Apple Devices",
   select **Get** ([Apple's guide](https://support.apple.com/guide/devices-windows/mchl5ded2763/windows)).
   Apple's software provides the USB connection to the iPhone on Windows, and KOD cannot ship it.
   (Exactly which Apple component is needed will be confirmed during device testing.)
2. Unlock the iPhone, connect the cable and tap **"Trust This Computer"**.
3. **libimobiledevice tools:** there are no official Windows builds yet. How KOD will provide
   them is still being decided. Until then, iPhone support on Windows is not available.

## Installing KOD

1. Open this repository's [Releases page](https://github.com/mariolkotsiai/kod/releases). Download
   KOD only from there.
2. From the release, download the installer `KOD_<version>_x64-setup.exe` and the `SHA256SUMS` file
   into the same folder.
3. Check the installer before running it. Open PowerShell in that folder and run the commands below,
   with the file name you downloaded:

   ```powershell
   $name = "KOD_0.1.0_x64-setup.exe"
   # The line for this file in SHA256SUMS starts with its SHA-256 hash
   $expected = ((Get-Content .\SHA256SUMS) -match [regex]::Escape($name)) -split '\s+' | Select-Object -First 1
   # True = the file matches the hash published with the release
   (Get-FileHash ".\$name" -Algorithm SHA256).Hash -eq $expected
   ```

   If it prints `False`, do not run the file. Delete it and download it again from the Releases page.
4. Run the installer. It is **not code-signed yet**, so Windows SmartScreen may show
   **"Windows protected your PC"**. Select **More info**, check that **App** shows the file you
   verified and **Publisher** shows *Unknown publisher*, then select **Run anyway**.
5. Choose whether to install KOD for your user account only or for all users of the computer.
   Installing for all users needs administrator rights.

To uninstall, open **Settings > Apps > Installed apps** (Windows 10: **Apps & features**), find
**Kernel Overview and Diagnostics - KOD** and select **Uninstall**. The uninstaller can also delete
the app data.

Each release also includes `sbom-cyclonedx.json`: a CycloneDX list (SBOM) of the Rust components the
app is built from.

## Things to know

- **Your backup passphrase cannot be recovered.** Without it, the backup cannot be opened by anyone,
  including you.
- **Older iPhones.** Apple's own Windows backup requires iOS 9 or later
  ([Apple](https://support.apple.com/en-us/108967)). KOD is not planned or tested for older versions.
- **iPhone photos and iCloud.** If **iCloud Photos** uses **Optimize iPhone Storage**, the full-size
  originals are in iCloud, not on the phone. A cable backup only contains what is on the phone.
- **Android app data.** Android protects each app's private data. Without root (which this project
  will not use), only shared storage, contacts, messages and similar data can be backed up.
- **Messages** can be exported and viewed, but writing them back to a phone is limited by Android and
  iOS and may not be supported.
- **Two phones** can be connected at the same time, each in its own slot. A third phone is refused.

## Building from source

Requirements: Node.js 24, Rust stable (via rustup), Windows.

```powershell
npm ci
npm run typecheck
npm run build
cd src-tauri
cargo test --locked
```

## Security and privacy

- Report vulnerabilities privately as described in [SECURITY.md](SECURITY.md). Do not open public issues.
- No data leaves your computer. There is no account and no server.
- See [DISCLAIMER.md](DISCLAIMER.md). The software is provided as-is, without warranty.

## Prior art and acknowledgements

- [libimobiledevice](https://libimobiledevice.org/): open protocols for talking to iPhones.
- [scrcpy](https://github.com/Genymobile/scrcpy): running helper code on Android over `adb` without
  installing an app.
- [Open Android Backup](https://github.com/mrrfv/open-android-backup): `adb`-based Android backup.
  It is GPL-3.0, so only ideas are reused, no code.

## License

[Apache License 2.0](LICENSE).

Android is a trademark of Google LLC. Apple, iPhone and iCloud are trademarks of Apple Inc.
This project is not affiliated with or endorsed by Google, Apple or Microsoft.
