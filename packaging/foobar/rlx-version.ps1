# The component's version, read from the one place that defines it (ADR-0025).
#
# Dot-sourced by its callers rather than copied again: the same section-anchored
# regex also lives in plugin-foobar/build.ps1, and a version that disagrees with
# itself across copies is exactly what ADR-0025 exists to prevent.
# packaging/windows/stage.ps1 dot-sources this file rather than carrying a copy.
#
# Anchored to [workspace.package]: a naive first-`version =` match would happily
# read a member crate's inherited line or a [profile] key, and nothing
# downstream would catch the wrong string.

function Get-RlxWorkspaceVersion {
    param([Parameter(Mandatory = $true)][string]$RepoRoot)

    $cargoToml = Join-Path $RepoRoot "Cargo.toml"
    if (-not (Test-Path $cargoToml)) {
        throw "no Cargo.toml at $cargoToml - is '$RepoRoot' the repository root?"
    }

    $text = Get-Content -Raw $cargoToml
    if ($text -notmatch '\[workspace\.package\][^\[]*?\bversion\s*=\s*"([^"]+)"') {
        throw "could not parse [workspace.package] version from $cargoToml"
    }
    return $Matches[1]
}
