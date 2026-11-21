#Requires -Version 5.1
# `param` must be the first statement of a script; it cannot come after Set-StrictMode.
param(
    [Parameter(Mandatory = $true)]
    [string]$ArtifactPath
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

if (-not (Test-Path -LiteralPath $ArtifactPath)) {
    Write-Error "Artifact not found: $ArtifactPath"
    exit 1
}

# --- Windows Authenticode signing -------------------------------------------------------------
# Fails closed: the script never reports an unsigned artifact as signed.
# WIN_CERT_THUMBPRINT = SHA-1 thumbprint of a code-signing certificate that is installed in the
# runner's certificate store (HSM / cloud signing / imported on the runner). No PFX password is
# ever passed on the command line (it would be visible in the process list).
if (-not $env:WIN_CERT_THUMBPRINT) {
    Write-Error "WIN_CERT_THUMBPRINT is not set: refusing to treat '$ArtifactPath' as signed."
    exit 1
}

& signtool.exe sign /sha1 $env:WIN_CERT_THUMBPRINT /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 $ArtifactPath
if ($LASTEXITCODE -ne 0) {
    Write-Error "signtool sign failed with exit code $LASTEXITCODE"
    exit 1
}

& signtool.exe verify /pa $ArtifactPath
if ($LASTEXITCODE -ne 0) {
    Write-Error "signtool verify failed with exit code $LASTEXITCODE"
    exit 1
}

Write-Output "Signed and verified: $ArtifactPath"

# --- macOS Developer ID & notarization --------------------------------------------------------
# Not handled here. Run it on a macOS runner (Phase 2 packaging), for example:
#   codesign --force --options runtime --sign "Developer ID Application: <TEAM_NAME> (<TEAM_ID>)" <artifact>
#   xcrun notarytool submit <artifact> --apple-id $APPLE_ID --team-id $APPLE_TEAM_ID --password $APPLE_APP_PASSWORD --wait
