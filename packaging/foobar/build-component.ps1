# Build, assemble, stamp, package and VERIFY the foobar2000 component
# (Plan 0102 Phase 2, ADR-0115).
#
# Produces target/dist/light-music-visualizer-v<version>-foobar2000-component.zip,
# whose single top-level folder holds foo_lmv.fb2k-component and
# READ-ME-FIRST.txt.
#
# This script is checked in rather than inlined into the release workflow so
# packaging is reproducible on any Windows box, not CI-only magic - the same
# reasoning packaging/macos/bundle.sh is built on (ADR-0038, Positive). The
# release workflow's foobar job is a thin caller: fetch the SDK, run this,
# upload the zip. Unlike the macOS bundle, this one CAN be exercised locally on
# every attempt (ADR-0115, Positive), so a green local run is worth something.
#
# The verification lives HERE rather than in the workflow, so a local run is
# held to the same bar as CI. Every check below is fatal.
#
#   Usage:  packaging\foobar\build-component.ps1 [-SkipBuild]
#
#   -SkipBuild   Reuse plugin-foobar/build/foo_lmv.dll already on disk. For
#                iterating on the package layout without paying for a
#                lto = "fat" rebuild of the core; never used by CI.
#
# Written for Windows PowerShell 5.1 as well as pwsh 7: a developer box runs the
# former and the GitHub runner the latter, so nothing here uses ternaries,
# null-coalescing, or -AsHashtable.

param([switch]$SkipBuild)

$ErrorActionPreference = "Stop"

$script:here = Split-Path -Parent $MyInvocation.MyCommand.Path
$repo = Split-Path -Parent (Split-Path -Parent $script:here)
. (Join-Path $script:here "lmv-version.ps1")
. (Join-Path $script:here "sdk-pin.ps1")

$pluginDir = Join-Path $repo "plugin-foobar"
$sdkDir = Join-Path $pluginDir "sdk"
$dll = Join-Path $pluginDir "build\foo_lmv.dll"

# The component's filename is contractual, not cosmetic: foo_lmv.cpp declares
# VALIDATE_COMPONENT_FILENAME("foo_lmv.dll") and foobar2000 refuses to load a
# component whose DLL was renamed.
$DllName = "foo_lmv.dll"
$ComponentName = "foo_lmv.fb2k-component"

function Die($message) { Write-Host ""; throw "build-component.ps1: FAILED: $message" }
function Step($message) { Write-Host ""; Write-Host "==> $message" }
function Check($message) { Write-Host "    ok: $message" }

Add-Type -AssemblyName System.IO.Compression | Out-Null
Add-Type -AssemblyName System.IO.Compression.FileSystem | Out-Null

# --- Zip helpers --------------------------------------------------------------
#
# Entry names are written explicitly rather than derived from a directory walk.
# Compress-Archive and .NET Framework's ZipFile.CreateFromDirectory both emit
# BACKSLASH separators on Windows PowerShell 5.1 and forward slashes on pwsh 7 -
# and the zip format specifies forward slashes. A component archive whose entry
# reads `x64\foo_lmv.dll` is one foobar2000 build away from installing nothing,
# and it would pass a check that read the same archive back through the same
# broken API. Naming every entry by hand removes the variable entirely.
function New-ZipWithEntries {
    param(
        [Parameter(Mandatory = $true)][string]$ZipPath,
        # Ordered list of @{ Path = <file on disk>; Entry = <name in archive> }
        [Parameter(Mandatory = $true)][object[]]$Entries
    )

    if (Test-Path $ZipPath) { Remove-Item -Force $ZipPath }
    New-Item -ItemType Directory -Force (Split-Path -Parent $ZipPath) | Out-Null

    $zip = [System.IO.Compression.ZipFile]::Open($ZipPath, "Create")
    try {
        foreach ($item in $Entries) {
            [System.IO.Compression.ZipFileExtensions]::CreateEntryFromFile(
                $zip, $item.Path, $item.Entry,
                [System.IO.Compression.CompressionLevel]::Optimal) | Out-Null
        }
    }
    finally {
        $zip.Dispose()
    }
}

# Read entry names back OUT of the finished archive - what ships is what was
# written, not what we think we staged. Mirrors bundle.sh's bar.
function Get-ZipEntryNames {
    param([Parameter(Mandatory = $true)][string]$ZipPath)

    $zip = [System.IO.Compression.ZipFile]::OpenRead((Resolve-Path $ZipPath))
    try {
        $names = @()
        foreach ($entry in $zip.Entries) { $names += $entry.FullName }
        return $names
    }
    finally {
        $zip.Dispose()
    }
}

# --- PE helpers ---------------------------------------------------------------
#
# The two facts worth asserting about the built DLL are its machine type (an
# x86 DLL in the x64/ folder installs and then fails to load) and its export of
# foobar2000_get_interface (without it foobar2000 rejects the component). Both
# are read straight out of the PE headers rather than shelled out to dumpbin:
# -SkipBuild is allowed to run on a box with no MSVC, and a verification step
# that silently degrades when a tool is missing is not a verification step.
function Get-PeInfo {
    param([Parameter(Mandatory = $true)][string]$Path)

    $bytes = [System.IO.File]::ReadAllBytes($Path)
    if ($bytes.Length -lt 0x40) { Die "$Path is too small to be a PE image" }
    if ($bytes[0] -ne 0x4D -or $bytes[1] -ne 0x5A) { Die "$Path has no MZ signature" }

    $peOffset = [System.BitConverter]::ToInt32($bytes, 0x3C)
    if ($peOffset -le 0 -or ($peOffset + 24) -ge $bytes.Length) {
        Die "$Path has an out-of-range PE header offset ($peOffset)"
    }
    if ([System.BitConverter]::ToUInt32($bytes, $peOffset) -ne 0x00004550) {
        Die "$Path has no PE\0\0 signature at offset $peOffset"
    }

    $machine = [System.BitConverter]::ToUInt16($bytes, $peOffset + 4)
    $sectionCount = [System.BitConverter]::ToUInt16($bytes, $peOffset + 6)
    $optionalSize = [System.BitConverter]::ToUInt16($bytes, $peOffset + 20)
    $optional = $peOffset + 24
    $magic = [System.BitConverter]::ToUInt16($bytes, $optional)

    # The data directory sits at a different offset in PE32 and PE32+, and the
    # export directory is entry 0 in both.
    if ($magic -eq 0x20B) { $dataDir = $optional + 112 }
    elseif ($magic -eq 0x10B) { $dataDir = $optional + 96 }
    else { Die "$Path has an unrecognised optional-header magic 0x$('{0:X}' -f $magic)" }

    $sections = @()
    $sectionBase = $optional + $optionalSize
    for ($i = 0; $i -lt $sectionCount; $i++) {
        $s = $sectionBase + ($i * 40)
        $sections += [pscustomobject]@{
            VirtualSize    = [System.BitConverter]::ToUInt32($bytes, $s + 8)
            VirtualAddress = [System.BitConverter]::ToUInt32($bytes, $s + 12)
            RawSize        = [System.BitConverter]::ToUInt32($bytes, $s + 16)
            RawPointer     = [System.BitConverter]::ToUInt32($bytes, $s + 20)
        }
    }

    # An RVA lands in the section whose virtual range covers it; the file offset
    # is that section's raw pointer plus the distance into it.
    function Convert-RvaToOffset($rva) {
        foreach ($s in $sections) {
            $span = $s.VirtualSize
            if ($span -eq 0) { $span = $s.RawSize }
            if ($rva -ge $s.VirtualAddress -and $rva -lt ($s.VirtualAddress + $span)) {
                return [int]($rva - $s.VirtualAddress + $s.RawPointer)
            }
        }
        return -1
    }

    $exports = @()
    $exportRva = [System.BitConverter]::ToUInt32($bytes, $dataDir)
    if ($exportRva -ne 0) {
        $exportOffset = Convert-RvaToOffset $exportRva
        if ($exportOffset -ge 0) {
            $nameCount = [System.BitConverter]::ToUInt32($bytes, $exportOffset + 24)
            $namesRva = [System.BitConverter]::ToUInt32($bytes, $exportOffset + 32)
            $namesOffset = Convert-RvaToOffset $namesRva
            if ($namesOffset -ge 0) {
                for ($i = 0; $i -lt $nameCount; $i++) {
                    $nameRva = [System.BitConverter]::ToUInt32($bytes, $namesOffset + ($i * 4))
                    $nameOffset = Convert-RvaToOffset $nameRva
                    if ($nameOffset -lt 0) { continue }
                    $end = $nameOffset
                    while ($end -lt $bytes.Length -and $bytes[$end] -ne 0) { $end++ }
                    $exports += [System.Text.Encoding]::ASCII.GetString(
                        $bytes, $nameOffset, $end - $nameOffset)
                }
            }
        }
    }

    return [pscustomobject]@{
        Machine = $machine
        Exports = $exports
        # Latin1 keeps one byte to one char, so an offset in this string is an
        # offset in the file - which is what makes a literal search meaningful.
        Text    = [System.Text.Encoding]::GetEncoding(28591).GetString($bytes)
    }
}

# --- The SDK is a parameter, not a property (ADR-0115) ------------------------

if (-not (Test-Path (Join-Path $sdkDir "foobar2000\SDK\foobar2000.h"))) {
    Die @"
foobar2000 SDK not staged at $sdkDir.
Run packaging\foobar\fetch-sdk.ps1 to download the pinned release, or unpack it
there by hand (see plugin-foobar/README.md). The SDK is third-party and
separately licensed, so it is gitignored and never committed.
"@
}

# The staged tree states its own version, so compare rather than assert over it.
# The pin and what is on disk are the same fact only on the fetch route, where
# fetch-sdk.ps1 checked a SHA-256; the pre-staged route is first-class
# (ADR-0115) and unpacks by hand whatever the developer has. Without this, a
# hand-staged older SDK produces a component whose READ-ME-FIRST.txt asserts a
# build against the pin - a claim nothing downstream can check, because the SDK
# version is not in the DLL.
#
# The marker is a line inside sdk-readme.html's <h1>, and the tag spans lines,
# so this matches the text and not the element. Captured to end-of-line rather
# than as a date: the version format is foobar2000's to change, and a pattern
# that stops matching would fail this open.
$sdkReadme = Join-Path $sdkDir "sdk-readme.html"
if (-not (Test-Path $sdkReadme)) {
    Die @"
staged SDK has no sdk-readme.html at $sdkReadme.
That file carries the SDK's own version marker, which is what this recipe
compares against packaging\foobar\sdk-pin.ps1. Re-stage the SDK from the
official archive rather than a partial copy of one.
"@
}
$sdkMatch = [regex]::Match(
    (Get-Content -Raw $sdkReadme), "foobar2000 SDK,\s*version\s*([^\r\n<]+)")
if (-not $sdkMatch.Success) {
    Die @"
no version marker in $sdkReadme.
Expected a line reading `"foobar2000 SDK, version <version>`". Re-stage the SDK
from the official archive.
"@
}
$stagedSdkVersion = $sdkMatch.Groups[1].Value.Trim()
if ($stagedSdkVersion -ne $LmvSdkVersion) {
    Die @"
the staged SDK is not the pinned one.
  staged (plugin-foobar\sdk\sdk-readme.html): $stagedSdkVersion
  pinned (packaging\foobar\sdk-pin.ps1):      $LmvSdkVersion
READ-ME-FIRST.txt states the pin, so this build would ship a version claim that
never touched it. Run packaging\foobar\fetch-sdk.ps1 -Force to stage the pinned
release, or bump the pin if the move is deliberate - sdk-pin.ps1 says what a
bump requires.
"@
}
Check "SDK $stagedSdkVersion staged at plugin-foobar\sdk (matches the pin)"

# --- Build --------------------------------------------------------------------

if (-not $SkipBuild) {
    Step "plugin-foobar\build.ps1"
    # Called rather than reimplemented: the MSVC location, the SDK project
    # builds, the link line and the version header are that script's job and
    # duplicating them here is how the two drift.
    & (Join-Path $pluginDir "build.ps1")
    if ($LASTEXITCODE -ne 0 -and $null -ne $LASTEXITCODE) {
        Die "plugin-foobar\build.ps1 exited $LASTEXITCODE"
    }
}

if (-not (Test-Path $dll)) { Die "missing $dll (drop -SkipBuild?)" }

# --- Stamp: the version, from the one place that defines it (ADR-0025) --------

$version = Get-LmvWorkspaceVersion -RepoRoot $repo
$stageName = "light-music-visualizer-v$version-foobar2000-component"
$outDir = Join-Path $repo "target\dist"
$stage = Join-Path $outDir $stageName
$zipPath = Join-Path $outDir "$stageName.zip"
$componentPath = Join-Path $stage $ComponentName
$readmePath = Join-Path $stage "READ-ME-FIRST.txt"

Write-Host ""
Write-Host "build-component.ps1: version $version, SDK $LmvSdkVersion -> $stageName.zip"

# --- Assemble -----------------------------------------------------------------

Step "staging $stageName"
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
if (Test-Path $zipPath) { Remove-Item -Force $zipPath }
New-Item -ItemType Directory -Force $stage | Out-Null

# The component archive holds the x64 payload and NOTHING else. foobar2000
# extracts a component's whole archive and then overwrites the root with the
# contents of the subfolder matching its architecture, so every extra file
# lands in the user's components folder - a documented cause of broken x64
# installs. The reader-facing text therefore travels in the wrapper zip, not in
# here.
Step "assemble $ComponentName (x64/$DllName, and nothing else)"
New-ZipWithEntries -ZipPath $componentPath -Entries @(
    @{ Path = $dll; Entry = "x64/$DllName" }
)

# The root of the archive is deliberately empty: nothing here builds a 32-bit
# shim, so x86 foobar2000 has no payload to find and will report the component
# as unsupported rather than crashing. READ-ME-FIRST.txt says so.

Step "stamp READ-ME-FIRST.txt"
$readmeSource = Join-Path $script:here "READ-ME-FIRST.md"
if (-not (Test-Path $readmeSource)) { Die "missing $readmeSource" }
$readmeText = Get-Content -Raw $readmeSource
$readmeText = $readmeText.Replace("@VERSION@", $version)
$readmeText = $readmeText.Replace("@SDK_VERSION@", $LmvSdkVersion)
# .txt so a double-click opens it, matching the two existing zips. Written
# without a BOM: Notepad is fine either way, but a BOM is the kind of thing that
# shows up as a stray glyph in whatever the recipient actually opens it with.
[System.IO.File]::WriteAllText(
    $readmePath, $readmeText, (New-Object System.Text.UTF8Encoding($false)))

Step "package $stageName.zip"
New-ZipWithEntries -ZipPath $zipPath -Entries @(
    @{ Path = $componentPath; Entry = "$stageName/$ComponentName" },
    @{ Path = $readmePath; Entry = "$stageName/READ-ME-FIRST.txt" }
)

# --- Verify -------------------------------------------------------------------
#
# Plan 0102 Phase 2's done-when, asserted rather than eyeballed. Any failure
# below is a failed package: the zip exists but must not ship.

Step "verify"

# 1. The component archive's exact contents. Equality, not containment: an
#    extra entry is the failure mode being guarded against.
$componentEntries = @(Get-ZipEntryNames -ZipPath $componentPath)
$expected = @("x64/$DllName")
if ($componentEntries.Count -ne 1 -or $componentEntries[0] -ne $expected[0]) {
    Die "$ComponentName should hold exactly '$($expected[0])', holds: $($componentEntries -join ', ')"
}
Check "$ComponentName holds exactly x64/$DllName"

# 2. The payload is genuinely x64. An x86 DLL under x64/ installs and then
#    fails to load, with nothing in the package to say why.
$pe = Get-PeInfo -Path $dll
if ($pe.Machine -ne 0x8664) {
    Die "$DllName is machine 0x$('{0:X4}' -f $pe.Machine), expected 0x8664 (x64)"
}
Check "$DllName is an x64 PE image"

# 3. The component entry point. foobar2000 loads a component by resolving this
#    export; a DLL without it is rejected at startup.
if ($pe.Exports -notcontains "foobar2000_get_interface") {
    Die "$DllName does not export foobar2000_get_interface (exports: $($pe.Exports -join ', '))"
}
Check "$DllName exports foobar2000_get_interface"

# 4. The declared version. DECLARE_COMPONENT_VERSION stores it as a literal in
#    the image, so the version foobar2000 will show in its component list is
#    readable here - which is the only way to check that the build really did
#    substitute it rather than fall through to foo_lmv.cpp's #ifndef default.
if (-not $pe.Text.Contains("Light Music Visualizer")) {
    Die "$DllName does not carry its DECLARE_COMPONENT_VERSION name string"
}
if ($pe.Text.Contains("0.0.0-dev")) {
    Die @"
$DllName carries the "0.0.0-dev" fallback from foo_lmv.cpp's #ifndef, so the
version was NOT substituted: build/foo_lmv_version.h was missing or not on the
include path. The component would ship claiming a version it is not.
"@
}
if (-not $pe.Text.Contains($version)) {
    Die "$DllName does not carry the workspace version '$version'"
}
Check "declared version is $version, matching [workspace.package]"

# 5. The wrapper zip's exact contents.
$zipEntries = @(Get-ZipEntryNames -ZipPath $zipPath)
foreach ($required in "$stageName/$ComponentName", "$stageName/READ-ME-FIRST.txt") {
    if ($zipEntries -notcontains $required) {
        Die "zip is missing $required (holds: $($zipEntries -join ', '))"
    }
}
if ($zipEntries.Count -ne 2) {
    Die "zip should hold exactly 2 entries, holds $($zipEntries.Count): $($zipEntries -join ', ')"
}
Check "zip top level holds $ComponentName and READ-ME-FIRST.txt"

# 6. No .md, matching the two existing zips: the recipient gets .txt so a
#    double-click opens it.
$md = $zipEntries | Where-Object { $_ -like "*.md" }
if ($md) { Die "zip contains a .md file: $($md -join ' ')" }
Check "no .md in the zip"

# 7. Every placeholder substituted. A READ-ME-FIRST still reading @VERSION@ is
#    the sort of thing that ships unnoticed because nothing else reads it.
$shipped = Get-Content -Raw $readmePath
if ($shipped -match '@[A-Z_]+@') {
    Die "READ-ME-FIRST.txt has an unsubstituted placeholder: $($Matches[0])"
}
Check "READ-ME-FIRST.txt has no unsubstituted placeholders"

Write-Host ""
Write-Host "build-component.ps1: OK -> $zipPath"
