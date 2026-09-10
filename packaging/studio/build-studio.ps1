# Build, stage, zip and verify the Windows studio (ADR-0178, ADR-0038).
#
# Produces target/dist/ritmolux-studio-v<version>-windows-x64.zip, whose single
# top-level folder holds the unpacked Electron application with the player at
# resources/player/ritmolux.exe, READ-ME-FIRST.txt and the Spout notice.
#
# Checked in rather than inlined into the workflow, for the reason
# packaging/macos/bundle.sh gives: packaging is reproducible on any Windows
# machine rather than CI-only magic (ADR-0038, Positive). The release workflow's
# studio-windows job is a thin caller - stage the Spout SDK, run this, upload.
#
# Every check below is fatal. What ships is what Compress-Archive wrote, so the
# verification reads the archive back rather than the staging directory.
#
#   Usage:  ./packaging/studio/build-studio.ps1 [-SkipBuild]
#
#   -SkipBuild  Reuse the release player on disk and the installed node_modules.
#               For iterating on the zip's layout without paying for an
#               `lto = "fat"` rebuild and a full npm ci; never used by CI.

param(
    [switch]$SkipBuild
)

$ErrorActionPreference = 'Stop'

$repoRoot = (Resolve-Path "$PSScriptRoot/../..").Path
$studio = Join-Path $repoRoot 'studio'

function Die($message) { throw "build-studio.ps1: FAILED: $message" }
function Step($message) { Write-Host ""; Write-Host "==> $message" }
function Check($message) { Write-Host "    ok: $message" }

# --- Where cargo writes, which is not necessarily $repoRoot/target ------------
#
# Asks cargo rather than assuming, exactly as packaging/macos/bundle.sh does:
# the two coincide under the default layout and diverge under any
# `build.target-dir`.
$metadata = cargo metadata --format-version 1 --no-deps --manifest-path (Join-Path $repoRoot 'Cargo.toml') | ConvertFrom-Json
$targetDir = $metadata.target_directory
if (-not $targetDir) { Die "could not read target_directory from cargo metadata" }

# --- Version: the single source of truth, section-anchored (ADR-0025) --------
#
# [workspace.package] is the ONE place the application version lives, and the
# studio's package.json is not it: cargo-release bumps Cargo.toml alone, so a
# version read from package.json drifts a patch behind on every close. It is
# overridden below via extraMetadata, which is also what the plist assertion in
# the macOS sibling checks.
$cargoToml = Get-Content -Raw (Join-Path $repoRoot 'Cargo.toml')
if ($cargoToml -notmatch '\[workspace\.package\][^\[]*?\bversion\s*=\s*"([^"]+)"') {
    Die "could not parse [workspace.package] version from Cargo.toml"
}
$version = $Matches[1]

$name = "ritmolux-studio-v$version-windows-x64"
$dist = Join-Path $targetDir 'dist'
$stage = Join-Path $dist $name
$zip = Join-Path $dist "$name.zip"
Write-Host "build-studio.ps1: version $version -> $name.zip"

# --- The player --------------------------------------------------------------
#
# --features spout, matching the standalone zip exactly: the studio ships the
# same player a tester would otherwise download, and without the feature
# `ritmolux --stream` fails with a named error instead of existing.
if (-not $SkipBuild) {
    Step "cargo build --release -p standalone --features spout"
    & cargo build --release -p standalone --features spout
    if ($LASTEXITCODE -ne 0) { Die "cargo build failed" }
}

$player = Join-Path $targetDir 'release/ritmolux.exe'
if (-not (Test-Path $player)) { Die "missing $player (drop -SkipBuild?)" }

Step "staging the player for extraResources"
$playerStage = Join-Path $studio 'staging/player'
if (Test-Path $playerStage) { Remove-Item -Recurse -Force $playerStage }
New-Item -ItemType Directory -Force $playerStage | Out-Null
Copy-Item $player $playerStage

# --- The studio --------------------------------------------------------------

Push-Location $studio
try {
    if (-not $SkipBuild) {
        # npm ci rather than npm install: it installs the committed lockfile
        # exactly and fails if package.json and the lockfile disagree, which is
        # the only reason the lockfile is committed at all.
        Step "npm ci"
        & npm ci
        if ($LASTEXITCODE -ne 0) { Die "npm ci failed" }
    }

    Step "npm run build (main, preload, renderer)"
    & npm run build
    if ($LASTEXITCODE -ne 0) { Die "npm run build failed" }

    # --dir: the unpacked application only. The archive is assembled below, so
    # all five zips in a release share one naming and one layout.
    Step "electron-builder --win --dir"
    # The override is QUOTED as one argument on purpose: unquoted, PowerShell
    # splits `-c.extraMetadata.version=x` at the first dot and electron-builder
    # reads the remainder as a config FILE path, failing with ENOENT.
    & npx --no-install electron-builder --win --dir "-c.extraMetadata.version=$version"
    if ($LASTEXITCODE -ne 0) { Die "electron-builder failed" }
}
finally {
    Pop-Location
}

$unpacked = Join-Path $studio 'release/win-unpacked'
if (-not (Test-Path $unpacked)) { Die "electron-builder produced no $unpacked" }

# --- Stage and zip -----------------------------------------------------------

Step "staging $name"
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
if (Test-Path $zip) { Remove-Item -Force $zip }
New-Item -ItemType Directory -Force $stage | Out-Null
Copy-Item "$unpacked/*" $stage -Recurse

Copy-Item (Join-Path $PSScriptRoot 'READ-ME-FIRST.md') (Join-Path $stage 'READ-ME-FIRST.txt')
# Spout is Simplified BSD and is STATICALLY linked into the ritmolux.exe this
# zip carries, so clause 2 binds this archive exactly as it binds the standalone
# one: a binary redistribution must reproduce the notice in the materials
# shipped with it.
Copy-Item (Join-Path $repoRoot 'packaging/spout/spout-license.txt') (Join-Path $stage 'spout-license.txt')

# -Path without a wildcard keeps the staging folder as the archive's single
# top-level entry, which is the layout every zip in this release uses.
Step "Compress-Archive"
Compress-Archive -Path $stage -DestinationPath $zip -Force

# --- Verify ------------------------------------------------------------------

Step "verify"

# Needed on Windows PowerShell 5.1, already present on the pwsh 7 the runner
# uses - so a failure here is not one.
try { Add-Type -AssemblyName System.IO.Compression.FileSystem -ErrorAction Stop } catch { }
$archive = [IO.Compression.ZipFile]::OpenRead($zip)
try { $entries = $archive.Entries | ForEach-Object { $_.FullName } }
finally { $archive.Dispose() }
# Compress-Archive writes backslash separators on Windows PowerShell 5.1 and
# forward slashes on pwsh 7. Normalize rather than depend on which one the host
# ships - a check that passes only on the machine we happened to try it on is
# not a check.
$entries = $entries | ForEach-Object { $_.Replace('\', '/') }

# resources/player/ritmolux.exe is the load-bearing one and no unit test can
# assert it: it is the path resolve.ts's FIRST candidate points at, so a tester
# who unzips and double-clicks configures nothing exactly when this entry is
# here. resolve.ts's own tests prove the order, never the layout.
foreach ($required in "$name/Ritmolux Studio.exe",
                      "$name/resources/player/ritmolux.exe",
                      "$name/resources/app.asar",
                      "$name/READ-ME-FIRST.txt",
                      "$name/spout-license.txt") {
    if ($entries -notcontains $required) { Die "zip is missing $required" }
}
Check "zip holds the studio, the bundled player at resources/player/, READ-ME-FIRST.txt and the Spout notice"

$md = $entries | Where-Object { $_ -like "*.md" }
if ($md) { Die "zip contains a .md file: $($md -join ' ')" }
Check "no .md in the zip"

# Printed, never enforced. NFR section 4's studio row is a soft cap like the two
# beside it, and a release must not fail over a size.
$bytes = (Get-Item $zip).Length
Check "zip is $bytes B"

Write-Host ""
Write-Host "build-studio.ps1: OK -> $zip"
