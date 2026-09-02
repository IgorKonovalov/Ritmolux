# The component's version, read from the one place that defines it (ADR-0025).
#
# Dot-sourced by fetch-sdk.ps1's siblings rather than copied a fourth time: the
# same section-anchored regex already lives in plugin-foobar/build.ps1 and in
# .github/workflows/release.yml's windows job, and a version that disagrees with
# itself across three copies is exactly what ADR-0025 exists to prevent.
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
