# Build, measure, stage, zip and VERIFY the Windows standalone (ADR-0038,
# ADR-0231).
#
# Produces target/dist/ritmolux-v<version>-windows-x64.zip, whose single
# top-level folder holds ritmolux.exe, a reference copy of presets/*.toml,
# READ-ME-FIRST.txt and spout-license.txt.
#
# Checked in rather than inlined into the release workflow so packaging is
# reproducible on any Windows box, not CI-only magic - the same shape as
# packaging/macos/bundle.sh and packaging/linux/stage.sh. The release
# workflow's windows job is a thin caller: stage the Spout SDK, run this,
# upload the zip.
#
# The verification lives HERE rather than in the workflow, so a local run is
# held to the same bar as CI. Every check below is fatal except the size
# measurement, which prints and at most warns (ADR-0231, on ADR-0159's terms).
#
#   Usage:  packaging\windows\stage.ps1 [-SkipBuild] [-WarnBytes <n>]
#
#   -SkipBuild   Reuse <target-dir>\release\ritmolux.exe already on disk. For
#                iterating on the zip's layout without paying for a
#                lto = "fat" rebuild; never used by CI.
#   -WarnBytes   Lower the size warning's threshold. Its default sits well
#                above what the exe measures today, so this is how that branch
#                is exercised without waiting for the artifact to grow into it.
#
# Written for Windows PowerShell 5.1 as well as pwsh 7: a developer box runs the
# former and the GitHub runner the latter, so nothing here uses ternaries,
# null-coalescing, or -AsHashtable.

# `param` has to be the first statement, so the two size constants it would
# read for a default are below it and -WarnBytes 0 means "use the default".
param([switch]$SkipBuild, [long]$WarnBytes = 0)

$ErrorActionPreference = "Stop"

# NFR section 4: the exe's soft cap, and 90% of it. A size is a MEASUREMENT
# (ADR-0071): printed on every build, warned on above the threshold, and NEVER
# fatal - the checks below are properties of a correct artifact, and a release
# must not fail on a byte count. The same two figures sit in
# packaging/macos/bundle.sh, which measures the same executable's other build.
$ExeCapBytes = 16777216
$ExeWarnBytes = 15099494
if ($WarnBytes -le 0) { $WarnBytes = $ExeWarnBytes }

$script:here = Split-Path -Parent $MyInvocation.MyCommand.Path
$repo = Split-Path -Parent (Split-Path -Parent $script:here)
. (Join-Path $repo "packaging\foobar\rlx-version.ps1")

$ExeName = "ritmolux.exe"

function Die($message) { Write-Host ""; throw "stage.ps1: FAILED: $message" }
function Step($message) { Write-Host ""; Write-Host "==> $message" }
function Check($message) { Write-Host "    ok: $message" }

Add-Type -AssemblyName System.IO.Compression | Out-Null
Add-Type -AssemblyName System.IO.Compression.FileSystem | Out-Null

# --- Zip helpers --------------------------------------------------------------
#
# Entry names are written explicitly rather than derived from a directory walk,
# for the reason packaging/foobar/build-component.ps1 gives: Compress-Archive
# emits BACKSLASH separators on Windows PowerShell 5.1 and forward slashes on
# pwsh 7, and the zip format specifies forward slashes. Naming every entry by
# hand removes the variable, and lets the verification below assert the exact
# entry names rather than normalize them first.
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

# --- Where cargo writes, which is not necessarily "$repo\target" --------------
#
# Asked of cargo rather than assumed, as bundle.sh and plugin-foobar/build.ps1
# do: the answer is right under the default layout and under any
# `build.target-dir`.
$metadata = & cargo metadata --format-version 1 --no-deps --manifest-path (Join-Path $repo "Cargo.toml") | ConvertFrom-Json
if ($LASTEXITCODE -ne 0 -and $null -ne $LASTEXITCODE) { Die "cargo metadata exited $LASTEXITCODE" }
$targetDir = $metadata.target_directory
if (-not $targetDir) { Die "could not read target_directory from cargo metadata" }
$exe = Join-Path $targetDir "release\$ExeName"

# --- Version: the single source of truth, section-anchored (ADR-0025) ---------

$version = Get-RlxWorkspaceVersion -RepoRoot $repo
$stageName = "ritmolux-v$version-windows-x64"
$outDir = Join-Path $targetDir "dist"
$stage = Join-Path $outDir $stageName
$zipPath = Join-Path $outDir "$stageName.zip"

Write-Host "stage.ps1: version $version -> $stageName.zip"

# --- Build --------------------------------------------------------------------
#
# --features spout is what makes `ritmolux --stream` exist in the shipped
# binary; without it the mode fails with a named error saying so (ADR-0125).
# The SDK it links is staged by packaging\spout\fetch-sdk.ps1, which the
# release workflow runs before this script.
$BuildCommand = "cargo build --release -p standalone --features spout"
if (-not $SkipBuild) {
    Step $BuildCommand
    & cargo build --release -p standalone --features spout
    if ($LASTEXITCODE -ne 0) { Die "cargo build exited $LASTEXITCODE" }
}

if (-not (Test-Path $exe)) { Die "missing $exe (drop -SkipBuild?)" }

# --- Measure: the exe's length, against NFR section 4's cap (ADR-0231) --------
#
# The cheapest fact about the artifact, and the only one NFR section 4
# constrains. Printed in bytes, the unit NFR section 4 writes its series in, so
# extending that series is a copy out of a build log rather than a measurement.
# The build is named beside the number because a size is a property of a
# build, not of the tree (ADR-0071): it moves with the toolchain, the profile
# and the feature set, and a figure with none of those attached cannot be
# compared with the next one.
Step "measure $ExeName against NFR section 4"
$exeBytes = (Get-Item $exe).Length
$toolchain = (& rustc --version) -join " "
# Invariant culture, not the operator's: a locale that renders `-f '{0:0.0}'`
# as `65,4` breaks a figure meant to be copied into a document that writes
# decimal points.
$capPercent = [string]::Format(
    [System.Globalization.CultureInfo]::InvariantCulture,
    "{0:0.0}", (100.0 * $exeBytes / $ExeCapBytes))
Write-Host "    $ExeName is $exeBytes B ($capPercent % of the $ExeCapBytes B cap)"
Write-Host "    build: $BuildCommand, v$version, $toolchain, x86_64-pc-windows-msvc"
if ($exeBytes -gt $WarnBytes) {
    # A warning, never a Die. The cap is soft, and a release blocked on a byte
    # count is one where someone edits the constant under time pressure at a
    # tag - which is worse than no gate, because it also destroys the record.
    #
    # The cap is named, the threshold is not described as a fraction of it: with
    # -WarnBytes the two are unrelated, and a message asserting 90% would be
    # false in exactly the run that exercises this branch.
    Write-Warning ("$ExeName is $exeBytes B, past the $WarnBytes B warning " +
        "threshold. NFR section 4's cap is $ExeCapBytes B. This is not a " +
        "release blocker. Record the figure in the exe's size series in " +
        "docs/nfr.md section 4 and say what moved it.")
} else {
    Check "under the $WarnBytes B warning threshold"
}

# --- Assemble -----------------------------------------------------------------

Step "staging $stageName"
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
if (Test-Path $zipPath) { Remove-Item -Force $zipPath }
New-Item -ItemType Directory -Force (Join-Path $stage "presets") | Out-Null

Copy-Item $exe (Join-Path $stage $ExeName)
# *.toml only - presets/README.md is the authoring reference, not preset
# content, and the zip is for a tester rather than an author. The app does not
# read these either way - core/build.rs embeds the set at compile time
# (ADR-0022) and the shell seeds an editable copy on first run.
$presetFiles = @(Get-ChildItem (Join-Path $repo "presets") -Filter *.toml -File | Sort-Object Name)
foreach ($toml in $presetFiles) {
    Copy-Item $toml.FullName (Join-Path $stage "presets\$($toml.Name)")
}
# .txt so a double-click opens it, matching the other archives.
Copy-Item (Join-Path $script:here "READ-ME-FIRST.md") (Join-Path $stage "READ-ME-FIRST.txt")
# Spout is Simplified BSD and is STATICALLY linked into ritmolux.exe, so clause
# 2 binds this archive: a binary redistribution must reproduce the notice in
# the materials shipped with it. No SpoutDX.dll travels (build.rs links
# SpoutDX_static.lib) - the notice does.
Copy-Item (Join-Path $repo "packaging\spout\spout-license.txt") (Join-Path $stage "spout-license.txt")

Step "package $stageName.zip"
$entries = @(
    @{ Path = (Join-Path $stage $ExeName); Entry = "$stageName/$ExeName" },
    @{ Path = (Join-Path $stage "READ-ME-FIRST.txt"); Entry = "$stageName/READ-ME-FIRST.txt" },
    @{ Path = (Join-Path $stage "spout-license.txt"); Entry = "$stageName/spout-license.txt" }
)
foreach ($toml in $presetFiles) {
    $entries += @{ Path = (Join-Path $stage "presets\$($toml.Name)"); Entry = "$stageName/presets/$($toml.Name)" }
}
New-ZipWithEntries -ZipPath $zipPath -Entries $entries

# --- Verify -------------------------------------------------------------------
#
# Read back out of the archive rather than off the staging directory - what
# ships is what was written. Any failure below is a failed package: the zip
# exists but must not ship.

Step "verify"

$zipEntries = @(Get-ZipEntryNames -ZipPath $zipPath)

# 1. The three named files, under the single top-level folder.
foreach ($required in "$stageName/$ExeName", "$stageName/READ-ME-FIRST.txt", "$stageName/spout-license.txt") {
    if ($zipEntries -notcontains $required) {
        Die "zip is missing $required (holds: $($zipEntries -join ', '))"
    }
}
Check "zip top level holds $ExeName, READ-ME-FIRST.txt and spout-license.txt"

# 2. Every entry sits under that folder - a stray sibling would unpack beside
#    it, and the single-top-level-folder layout is what every archive here
#    promises.
$outside = @($zipEntries | Where-Object { -not $_.StartsWith("$stageName/") })
if ($outside.Count -ne 0) {
    Die "zip has entries outside $stageName/: $($outside -join ', ')"
}
Check "single top-level folder: $stageName/"

# 3. The preset copy is the whole repo library, no more and no fewer.
$repoCount = $presetFiles.Count
$zipCount = @($zipEntries | Where-Object { $_ -like "$stageName/presets/*.toml" }).Count
if ($zipCount -ne $repoCount) {
    Die "zip carries $zipCount presets, repo has $repoCount"
}
Check "presets/: $zipCount .toml files, matching the repo"

# 4. No .md anywhere: presets/README.md is the one that would slip in, and the
#    tester's instructions ship as .txt so a double-click opens them.
$md = @($zipEntries | Where-Object { $_ -like "*.md" })
if ($md.Count -ne 0) { Die "zip contains a .md file: $($md -join ' ')" }
Check "no .md in the zip"

Write-Host ""
Write-Host "stage.ps1: OK -> $zipPath"
