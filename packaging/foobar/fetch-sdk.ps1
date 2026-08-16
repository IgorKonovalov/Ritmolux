# Stage the foobar2000 SDK at plugin-foobar/sdk/ (Plan 0102 Phase 2, ADR-0115).
#
# This is the *parameter* half of the recipe, not the recipe. ADR-0115 makes how
# the SDK reaches the build host a parameter of build-component.ps1 rather than
# a property of it: either it is already unpacked at plugin-foobar/sdk/, or this
# script puts it there. Both routes hand the same tree to the same build.
#
#   .\fetch-sdk.ps1           # no-op if the SDK is already staged
#   .\fetch-sdk.ps1 -Force    # re-download and re-extract over it
#
# The SDK is third-party and separately licensed, so it is gitignored and never
# committed (ADR-0115 Alternative A). Its licence is BSD-style: redistribution
# in binary form is permitted, and only *source* redistribution carries a notice
# obligation - we ship neither SDK source nor the author's name as an
# endorsement. See plugin-foobar/sdk/sdk-license.txt once this has run.
#
# The SDK release is PINNED rather than tracked; sdk-pin.ps1 holds the pin and
# the reasoning, and is the one file a bump edits.

param([switch]$Force)

$ErrorActionPreference = "Stop"

$script:root = Split-Path -Parent $MyInvocation.MyCommand.Path
. (Join-Path $script:root "sdk-pin.ps1")
$SdkVersion = $LmvSdkVersion
$SdkUrl = $LmvSdkUrl
$SdkSha256 = $LmvSdkSha256
$repo = Split-Path -Parent (Split-Path -Parent $script:root)
$pluginDir = Join-Path $repo "plugin-foobar"
$sdkDir = Join-Path $pluginDir "sdk"
$archive = Join-Path $pluginDir "SDK-$SdkVersion.7z"
$marker = Join-Path $sdkDir "foobar2000\SDK\foobar2000.h"

function Die($message) { throw "fetch-sdk.ps1: FAILED: $message" }
function Step($message) { Write-Host ""; Write-Host "==> $message" }
function Check($message) { Write-Host "    ok: $message" }

# 7-Zip: preinstalled and on PATH on GitHub's windows runners, in Program Files
# on a typical developer box. Nothing in .NET or Windows' bundled tar reads 7z,
# so this is a real prerequisite rather than a convenience.
function Find-SevenZip {
    $cmd = Get-Command 7z.exe -ErrorAction SilentlyContinue
    if ($cmd) { return $cmd.Source }
    foreach ($candidate in @(
            (Join-Path $env:ProgramFiles "7-Zip\7z.exe"),
            (Join-Path ${env:ProgramFiles(x86)} "7-Zip\7z.exe"))) {
        if (Test-Path $candidate) { return $candidate }
    }
    Die "7-Zip not found. Install it (https://www.7-zip.org) or put 7z.exe on PATH."
}

if ((Test-Path $marker) -and (-not $Force)) {
    Write-Host "fetch-sdk.ps1: SDK $SdkVersion already staged at $sdkDir (use -Force to replace)"
    exit 0
}

# --- Download -----------------------------------------------------------------

if ((-not (Test-Path $archive)) -or $Force) {
    Step "download SDK $SdkVersion"
    Write-Host "    $SdkUrl"
    # Invoke-WebRequest on Windows PowerShell 5.1 negotiates TLS 1.0 by default
    # against a server that requires 1.2, which surfaces as a bare "connection
    # closed" rather than as a protocol error.
    [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
    # The progress bar makes 5.1's Invoke-WebRequest an order of magnitude
    # slower and renders as noise in a CI log.
    $priorProgress = $ProgressPreference
    $ProgressPreference = "SilentlyContinue"
    try {
        Invoke-WebRequest -Uri $SdkUrl -OutFile $archive -UseBasicParsing
    }
    finally {
        $ProgressPreference = $priorProgress
    }
}
else {
    Write-Host "fetch-sdk.ps1: reusing $archive (use -Force to re-download)"
}

# --- Verify the bytes before trusting them ------------------------------------
#
# A pinned URL that quietly changes its bytes is an untested component riding a
# routine release (ADR-0115 Negative). Fatal, and the archive is removed so the
# next run cannot reuse a rejected download.

Step "verify SHA-256"
$actual = (Get-FileHash -Algorithm SHA256 -Path $archive).Hash
if ($actual -ne $SdkSha256.ToUpperInvariant()) {
    Remove-Item -Force $archive
    Die @"
SDK checksum mismatch - refusing to build against it.
  expected: $($SdkSha256.ToUpperInvariant())
  actual:   $actual
  url:      $SdkUrl
The pinned archive has been removed. A published release that changes its bytes
is exactly the supply-chain case ADR-0115 named, so this is not a checksum to
"just update": establish why it moved first. If foobar2000.org legitimately
republished this release, re-pin deliberately in packaging/foobar/sdk-pin.ps1,
as its own commit, then re-run Plan 0102 Phase 5's on-device check.
"@
}
Check "SHA-256 $actual"

# --- Extract ------------------------------------------------------------------

Step "extract to $sdkDir"
$sevenZip = Find-SevenZip
if (Test-Path $sdkDir) { Remove-Item -Recurse -Force $sdkDir }
New-Item -ItemType Directory -Force $sdkDir | Out-Null
& $sevenZip x "-o$sdkDir" $archive -y | Out-Null
if ($LASTEXITCODE -ne 0) { Die "7-Zip exited $LASTEXITCODE extracting $archive" }

if (-not (Test-Path $marker)) { Die "extraction produced no $marker" }
Check "foobar2000/SDK/foobar2000.h present"

$license = Join-Path $sdkDir "sdk-license.txt"
if (-not (Test-Path $license)) { Die "extraction produced no sdk-license.txt" }
Check "sdk-license.txt present"

Write-Host ""
Write-Host "fetch-sdk.ps1: OK -> SDK $SdkVersion staged at $sdkDir"
