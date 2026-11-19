#Requires -Version 5.1
Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

param(
    [Parameter(Mandatory = $true)]
    [string]$ArtifactPath
)

if (-not (Test-Path -LiteralPath $ArtifactPath)) {
    Write-Error "Artifact not found: $ArtifactPath"
    exit 1
}

# --- Windows Authenticode Signing Stub ---
# Configure $env:WIN_CERT_PFX_PATH and $env:WIN_CERT_PASSWORD in CI secrets before enabling.
# & "signtool.exe" sign /f $env:WIN_CERT_PFX_PATH /p $env:WIN_CERT_PASSWORD /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 $ArtifactPath
Write-Output "[STUB] Windows Authenticode signing hook ready for: $ArtifactPath"

# --- macOS Apple Developer ID & Notarization Stub (run on macOS runner) ---
# Configure APPLE_ID, APPLE_TEAM_ID, and APPLE_APP_PASSWORD via environment secrets.
# codesign --force --options runtime --sign "Developer ID Application: <TEAM_NAME> (<APPLE_TEAM_ID>)" "$ArtifactPath"
# xcrun notarytool submit "$ArtifactPath" --apple-id "$env:APPLE_ID" --team-id "$env:APPLE_TEAM_ID" --password "$env:APPLE_APP_PASSWORD" --wait
Write-Output "[STUB] macOS Apple Developer ID & notarytool hook ready for: $ArtifactPath"