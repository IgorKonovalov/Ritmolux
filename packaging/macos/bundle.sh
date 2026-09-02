#!/usr/bin/env bash
#
# Build, bundle, ad-hoc sign and zip the macOS standalone (Plan 0036, ADR-0038).
#
# Produces <target-dir>/dist/ritmolux-v<version>-macos-universal.zip,
# whose single top-level folder holds Ritmolux.app (a universal
# arm64 + x86_64 binary), a reference copy of presets/*.toml, and
# READ-ME-FIRST.txt.
#
# This script is checked in rather than inlined into the workflow so packaging
# is reproducible on any Mac, not CI-only magic (ADR-0038, Positive). The
# release workflow's macos job is a thin caller: install the two targets, run
# this, upload the zip.
#
# The verification the plan requires lives HERE rather than in the workflow, so
# a local run is held to the same bar as CI. Every check is fatal.
#
#   Usage:  packaging/macos/bundle.sh [--skip-build]
#
#   --skip-build   Reuse the two <target-dir>/<triple>/release/ritmolux binaries on
#                  disk. For iterating on the bundle layout without paying for
#                  a `lto = "fat"` rebuild twice; never used by CI.
#
# macOS ships bash 3.2, so nothing here uses bash 4 syntax.

set -euo pipefail

APP_NAME="Ritmolux"
BUNDLE_DIR="${APP_NAME}.app"
BIN_NAME="ritmolux"
ARM_TARGET="aarch64-apple-darwin"
INTEL_TARGET="x86_64-apple-darwin"

script_dir="$(cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(cd -- "${script_dir}/../.." && pwd)"

skip_build=0
for arg in "$@"; do
    case "$arg" in
        --skip-build) skip_build=1 ;;
        *) echo "bundle.sh: unknown argument: $arg" >&2; exit 2 ;;
    esac
done

die() { echo "bundle.sh: FAILED: $*" >&2; exit 1; }
step() { echo ""; echo "==> $*"; }
check() { echo "    ok: $*"; }

# --- Where cargo writes, which is not necessarily "${repo_root}/target" -------
#
# The two coincide under the default layout and diverge under any
# `build.target-dir`, so this asks cargo instead of assuming: the answer is right
# under either, which is why plugin-foobar/build.ps1 asks the same question.
# `--no-deps` keeps it to the workspace manifest, which is all target_directory
# needs. This is the one path here that is a RELEASE path.
#
# Parsed with sed rather than jq, because jq is not on a stock macOS and this
# script has to run on a bare Mac. `target_directory` is a top-level string in
# `--format-version 1`, and on a Mac its value carries neither a quote nor a
# backslash, so there is no JSON escape for the pattern to get wrong.
read_target_dir() {
    cargo metadata --format-version 1 --no-deps --manifest-path "${repo_root}/Cargo.toml" |
        sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p'
}

target_dir="$(read_target_dir)"
[ -n "$target_dir" ] || die "could not read target_directory from cargo metadata"

# --- Version: the single source of truth, section-anchored (ADR-0025) ---------
#
# Anchored to [workspace.package] exactly as plugin-foobar/build.ps1:57 is: a
# naive first-`version =` match would happily read a member crate's inherited
# line or a [profile] key and put a wrong string in the plist, where nothing
# downstream would catch it.
read_workspace_version() {
    awk '
        /^\[workspace\.package\]/ { insec = 1; next }
        /^\[/                     { insec = 0 }
        insec && match($0, /^[ \t]*version[ \t]*=[ \t]*"[^"]+"/) {
            line = substr($0, RSTART, RLENGTH)
            sub(/^[^"]*"/, "", line)
            sub(/"$/, "", line)
            print line
            exit
        }
    ' "${repo_root}/Cargo.toml"
}

version="$(read_workspace_version)"
[ -n "$version" ] || die "could not parse [workspace.package] version from Cargo.toml"

stage_name="ritmolux-v${version}-macos-universal"
out_dir="${target_dir}/dist"
stage="${out_dir}/${stage_name}"
bundle="${stage}/${BUNDLE_DIR}"
zip_path="${out_dir}/${stage_name}.zip"

echo "bundle.sh: version ${version} -> ${stage_name}.zip"

# --- Build both architectures ------------------------------------------------
#
# The targets are installed by the caller (the workflow runs `rustup target
# add`, per Plan 0036 Phase 1: putting them in rust-toolchain.toml would cost
# every clone an extra download for a CI-only need, NFR section 4). Failing here
# with the exact command beats failing inside cargo with a vaguer one.
if [ "$skip_build" -eq 0 ]; then
    installed="$(rustup target list --installed)"
    for target in "$ARM_TARGET" "$INTEL_TARGET"; do
        echo "$installed" | grep -qx "$target" \
            || die "rust target $target is not installed; run: rustup target add $target"
    done

    for target in "$ARM_TARGET" "$INTEL_TARGET"; do
        step "cargo build --release -p standalone --target ${target}"
        ( cd "$repo_root" && cargo build --release -p standalone --target "$target" )
    done
fi

arm_bin="${target_dir}/${ARM_TARGET}/release/${BIN_NAME}"
intel_bin="${target_dir}/${INTEL_TARGET}/release/${BIN_NAME}"
[ -f "$arm_bin" ] || die "missing $arm_bin (drop --skip-build?)"
[ -f "$intel_bin" ] || die "missing $intel_bin (drop --skip-build?)"

# --- Stage the bundle --------------------------------------------------------

step "staging ${stage_name}"
rm -rf "$stage" "$zip_path"
mkdir -p "${bundle}/Contents/MacOS"

step "lipo -create -> universal ${BIN_NAME}"
lipo -create -output "${bundle}/Contents/MacOS/${BIN_NAME}" "$arm_bin" "$intel_bin"

step "Info.plist (version substituted from [workspace.package])"
# The version is a dotted numeric string from Cargo.toml, so it carries no sed
# metacharacter and needs no escaping beyond the delimiter choice.
sed "s|@VERSION@|${version}|g" "${script_dir}/Info.plist.in" \
    > "${bundle}/Contents/Info.plist"

step "codesign --sign - (ad-hoc)"
# Ad-hoc: no Apple account, but it gives TCC a stable code identity to bind the
# Screen Recording grant to (ADR-0038). The identity is derived from the binary,
# so it changes on every rebuild and the tester re-grants each time - that cost
# is accepted and the zip README says so.
codesign --force --sign - "$bundle"

step "staging presets and READ-ME-FIRST.txt"
mkdir -p "${stage}/presets"
# *.toml only: presets/README.md is the authoring reference, not preset content,
# and the zip is for a tester rather than an author. The app does not read these
# either way - core/build.rs embeds the set at compile time (ADR-0022) and the
# shell seeds an editable copy on first run.
cp "${repo_root}"/presets/*.toml "${stage}/presets/"
cp "${script_dir}/READ-ME-FIRST.md" "${stage}/READ-ME-FIRST.txt"

step "ditto -c -k --sequesterRsrc --keepParent"
# ditto, not zip: `zip` does not round-trip a bundle's symlinks and extended
# attributes, which is how a bundle arrives subtly broken on the other end.
ditto -c -k --sequesterRsrc --keepParent "$stage" "$zip_path"

# --- Verify ------------------------------------------------------------------
#
# Plan 0036 Phase 1's done-when, asserted by the script rather than eyeballed.
# Any failure below is a failed package: the zip exists but must not ship.

step "verify"

archs="$(lipo -archs "${bundle}/Contents/MacOS/${BIN_NAME}")"
case " $archs " in
    *" arm64 "*) ;;
    *) die "universal binary is missing arm64 (lipo -archs: $archs)" ;;
esac
case " $archs " in
    *" x86_64 "*) ;;
    *) die "universal binary is missing x86_64 (lipo -archs: $archs)" ;;
esac
check "lipo -archs: $archs"

plutil -lint "${bundle}/Contents/Info.plist" >/dev/null \
    || die "plutil -lint rejected Info.plist"
check "plutil -lint"

codesign --verify --strict "$bundle" \
    || die "codesign --verify --strict rejected the bundle"
check "codesign --verify --strict"

plist_version="$(plutil -extract CFBundleShortVersionString raw -o - \
    "${bundle}/Contents/Info.plist")"
[ "$plist_version" = "$version" ] \
    || die "plist CFBundleShortVersionString is '$plist_version', Cargo.toml says '$version'"
check "plist version matches [workspace.package]: $plist_version"

# Zip contents, read back out of the archive rather than off the staging dir -
# what ships is what ditto wrote, not what we think we staged.
entries="$(unzip -Z1 "$zip_path")"
for required in "${stage_name}/${BUNDLE_DIR}/" \
                "${stage_name}/READ-ME-FIRST.txt" \
                "${stage_name}/presets/"; do
    echo "$entries" | grep -qF "$required" \
        || die "zip is missing top-level entry: $required"
done
check "zip top level holds ${BUNDLE_DIR}, READ-ME-FIRST.txt, presets/"

repo_toml_count="$(ls -1 "${repo_root}"/presets/*.toml | wc -l | tr -d ' ')"
zip_toml_count="$(echo "$entries" | grep -c "^${stage_name}/presets/.*\.toml$" || true)"
[ "$zip_toml_count" -eq "$repo_toml_count" ] \
    || die "zip carries ${zip_toml_count} presets, repo has ${repo_toml_count}"
check "presets/: ${zip_toml_count} .toml files, matching the repo"

# No .md anywhere: presets/README.md is the one that would slip in, and the
# tester's instructions ship as .txt so a double-click opens them.
if echo "$entries" | grep -q '\.md$'; then
    die "zip contains a .md file: $(echo "$entries" | grep '\.md$' | tr '\n' ' ')"
fi
check "no .md in the zip"

echo ""
echo "bundle.sh: OK -> ${zip_path}"
