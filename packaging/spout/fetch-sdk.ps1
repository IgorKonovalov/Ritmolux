# Stage the Spout SDK at standalone/spout-sdk/ (Plan 0115 Phase 3, ADR-0125).
#
# The parameter half of the recipe, mirroring packaging/foobar/fetch-sdk.ps1:
# either the SDK is already unpacked at standalone/spout-sdk/, or this puts it
# there. standalone/build.rs consumes whatever is there and does not care which
# route it came by.
#
#   .\fetch-sdk.ps1           # no-op if the SDK is already staged
#   .\fetch-sdk.ps1 -Force    # re-download and re-extract over it
#
# NOTHING NEEDS THIS UNLESS YOU BUILD WITH --features spout. The feature is off
# by default, so an ordinary cargo build, CI and the macOS target never reach
# this script and never need the SDK or a network.
#
# The SDK is third-party and separately licensed, so it is gitignored and never
# committed (ADR-0115 Alternative A, applied again). Its licence is Simplified
# BSD: binary redistribution is permitted provided the notice travels with the
# distribution, so the notice IS committed, at packaging/spout/spout-license.txt,
# and is copied in beside the staged SDK below.
#
# The release archive is PINNED rather than tracked; sdk-pin.ps1 holds the pin
# and the reasoning, and is the one file a bump edits.

param([switch]$Force)

$ErrorActionPreference = "Stop"

$script:root = Split-Path -Parent $MyInvocation.MyCommand.Path
. (Join-Path $script:root "sdk-pin.ps1")
$repo = Split-Path -Parent (Split-Path -Parent $script:root)
$sdkDir = Join-Path $repo "standalone\spout-sdk"
$archive = Join-Path $repo "standalone\Spout-SDK-$RlxSpoutVersion.zip"
# The one file build.rs needs to find. Checked after extraction, so a release
# that reorganises its layout fails here rather than as a confusing C++ error.
$marker = Join-Path $sdkDir "include\SpoutDX\SpoutDX.h"
$libMarker = Join-Path $sdkDir "MD\lib\SpoutDX_static.lib"

function Die($message) { throw "fetch-sdk.ps1: FAILED: $message" }
function Step($message) { Write-Host ""; Write-Host "==> $message" }
function Check($message) { Write-Host "    ok: $message" }

if ((Test-Path $marker) -and (Test-Path $libMarker) -and (-not $Force)) {
    Write-Host "fetch-sdk.ps1: Spout SDK $RlxSpoutVersion already staged at $sdkDir (use -Force to replace)"
    exit 0
}

# --- Download -----------------------------------------------------------------

if ((-not (Test-Path $archive)) -or $Force) {
    Step "download Spout SDK $RlxSpoutVersion"
    Write-Host "    $RlxSpoutUrl"
    # Windows PowerShell 5.1's Invoke-WebRequest negotiates TLS 1.0 by default
    # against a server that requires 1.2, which surfaces as a bare "connection
    # closed" rather than as a protocol error.
    [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
    # The progress bar makes 5.1's Invoke-WebRequest an order of magnitude
    # slower and renders as noise in a CI log.
    $priorProgress = $ProgressPreference
    $ProgressPreference = "SilentlyContinue"
    try {
        Invoke-WebRequest -Uri $RlxSpoutUrl -OutFile $archive -UseBasicParsing
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
# A pinned URL that quietly changes its bytes is an untested binary riding a
# routine release. Fatal, and the archive is removed so the next run cannot
# reuse a rejected download.

Step "verify SHA-256"
$actual = (Get-FileHash -Algorithm SHA256 -Path $archive).Hash
if ($actual -ne $RlxSpoutSha256.ToUpperInvariant()) {
    Remove-Item -Force $archive
    Die @"
Spout SDK checksum mismatch - refusing to build against it.
  expected: $($RlxSpoutSha256.ToUpperInvariant())
  actual:   $actual
  url:      $RlxSpoutUrl
The pinned archive has been removed. A published release that changes its bytes
is the supply-chain case ADR-0115 named, so this is not a checksum to "just
update": establish why it moved first. If the release was legitimately
republished, re-pin deliberately in packaging/spout/sdk-pin.ps1, as its own
commit, and put a frame into a real Syphon Spout In TOP before shipping.
"@
}
Check "SHA-256 $actual"

# --- Extract, flattened -------------------------------------------------------
#
# The archive nests everything under Spout-SDK-binaries/Libs_<version>/, so a
# straight extraction would put the SDK version into every path build.rs reads.
# The Libs_* directory's CONTENTS are lifted to the root of $sdkDir instead,
# which is what makes standalone/build.rs version-independent: a bump edits
# sdk-pin.ps1 and nothing else.

Step "extract to $sdkDir"
$staging = Join-Path $repo "standalone\spout-sdk-staging"
foreach ($dir in @($sdkDir, $staging)) {
    if (Test-Path $dir) { Remove-Item -Recurse -Force $dir }
}
New-Item -ItemType Directory -Force $staging | Out-Null
Expand-Archive -Path $archive -DestinationPath $staging -Force

$libs = Get-ChildItem $staging -Recurse -Directory | Where-Object { $_.Name -like "Libs_*" } | Select-Object -First 1
if (-not $libs) { Die "no Libs_* directory inside $archive - the release layout changed" }
New-Item -ItemType Directory -Force $sdkDir | Out-Null
Move-Item -Path (Join-Path $libs.FullName "*") -Destination $sdkDir -Force
Remove-Item -Recurse -Force $staging

if (-not (Test-Path $marker)) { Die "extraction produced no include\SpoutDX\SpoutDX.h" }
Check "include\SpoutDX\SpoutDX.h present"
if (-not (Test-Path $libMarker)) { Die "extraction produced no MD\lib\SpoutDX_static.lib" }
Check "MD\lib\SpoutDX_static.lib present"

# --- The licence travels with it ----------------------------------------------
#
# The binaries archive carries no licence file of its own, so the committed
# notice is placed beside the SDK it covers - both so a reader of the staged
# tree can see the terms, and so the release job has one path to copy from.

Step "stage the licence notice"
$license = Join-Path $script:root "spout-license.txt"
if (-not (Test-Path $license)) { Die "packaging\spout\spout-license.txt is missing - it is the notice binary redistribution obliges us to ship" }
Copy-Item -Path $license -Destination (Join-Path $sdkDir "spout-license.txt") -Force
Check "spout-license.txt staged"

Write-Host ""
Write-Host "fetch-sdk.ps1: OK -> Spout SDK $RlxSpoutVersion staged at $sdkDir"
Write-Host "               build with: cargo build -p standalone --features spout"
