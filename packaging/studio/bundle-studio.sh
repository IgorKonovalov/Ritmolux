#!/usr/bin/env bash
#
# Build, bundle, ad-hoc sign, zip and verify the macOS studio (ADR-0178, ADR-0038).
#
# Produces <target-dir>/dist/ritmolux-studio-v<version>-macos-universal.zip,
# whose single top-level folder holds Ritmolux Studio.app - a universal
# arm64 + x86_64 Electron application with a universal player inside it at
# Contents/Resources/player/ritmolux - and READ-ME-FIRST.txt.
#
# A SIBLING of packaging/macos/bundle.sh rather than a mode of it, which the
# plan's file list allows: the player's recipe builds a bundle by hand from two
# cargo outputs, while this one hands the bundling to electron-builder and takes
# it back for the signing, the archiving and the verification. What is shared is
# the BAR - every assertion bundle.sh makes about the player's bundle is made
# here about the studio's, plus one about the player inside it.
#
# Checked in rather than inlined into the workflow so packaging is reproducible
# on any Mac, not CI-only magic (ADR-0038, Positive). The release workflow's
# studio-macos job is a thin caller: install the two targets, run this, upload.
#
#   Usage:  packaging/studio/bundle-studio.sh [--skip-build]
#
#   --skip-build   Reuse the two <target-dir>/<triple>/release/ritmolux binaries
#                  on disk and the installed node_modules. For iterating on the
#                  layout without paying for a `lto = "fat"` rebuild twice;
#                  never used by CI.
#
# macOS ships bash 3.2, so nothing here uses bash 4 syntax.

set -euo pipefail

APP_NAME="Ritmolux Studio"
BUNDLE_DIR="${APP_NAME}.app"
BIN_NAME="ritmolux"
ARM_TARGET="aarch64-apple-darwin"
INTEL_TARGET="x86_64-apple-darwin"

script_dir="$(cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(cd -- "${script_dir}/../.." && pwd)"
studio="${repo_root}/studio"

skip_build=0
for arg in "$@"; do
    case "$arg" in
        --skip-build) skip_build=1 ;;
        *) echo "bundle-studio.sh: unknown argument: $arg" >&2; exit 2 ;;
    esac
done

die() { echo "bundle-studio.sh: FAILED: $*" >&2; exit 1; }
step() { echo ""; echo "==> $*"; }
check() { echo "    ok: $*"; }

# --- Where cargo writes, which is not necessarily "${repo_root}/target" -------
#
# Asks cargo rather than assuming, and parses with sed rather than jq because
# jq is not on a stock macOS - the same two reasons packaging/macos/bundle.sh
# gives.
read_target_dir() {
    cargo metadata --format-version 1 --no-deps --manifest-path "${repo_root}/Cargo.toml" |
        sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p'
}

target_dir="$(read_target_dir)"
[ -n "$target_dir" ] || die "could not read target_directory from cargo metadata"

# --- Version: the single source of truth, section-anchored (ADR-0025) ---------
#
# [workspace.package] is the ONE place the application version lives, and
# studio/package.json is not it: cargo-release bumps Cargo.toml alone, so a
# version read from package.json drifts a patch behind on every close. It is
# pushed into the bundle via extraMetadata below and read back out of the built
# plist in the verification, so the two cannot silently disagree.
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

stage_name="ritmolux-studio-v${version}-macos-universal"
out_dir="${target_dir}/dist"
stage="${out_dir}/${stage_name}"
bundle="${stage}/${BUNDLE_DIR}"
zip_path="${out_dir}/${stage_name}.zip"

echo "bundle-studio.sh: version ${version} -> ${stage_name}.zip"

# --- The player, universal ---------------------------------------------------
#
# The studio carries the same universal binary the standalone zip does, built
# the same way: two cargo targets and a lipo. The targets are installed by the
# caller (the workflow runs `rustup target add`, per ADR-0038: putting them in
# rust-toolchain.toml would cost every clone an extra download for a CI-only
# need, NFR section 4).
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

step "lipo -create -> universal ${BIN_NAME} for extraResources"
player_stage="${studio}/staging/player"
rm -rf "$player_stage"
mkdir -p "$player_stage"
lipo -create -output "${player_stage}/${BIN_NAME}" "$arm_bin" "$intel_bin"

# --- The studio --------------------------------------------------------------

if [ "$skip_build" -eq 0 ]; then
    # npm ci rather than npm install: it installs the committed lockfile exactly
    # and fails if package.json and the lockfile disagree, which is the only
    # reason the lockfile is committed at all.
    step "npm ci"
    ( cd "$studio" && npm ci )
fi

step "npm run build (main, preload, renderer)"
( cd "$studio" && npm run build )

# --universal makes electron-builder fetch both Electron architectures and lipo
# them, which is what ADR-0038's universal-app promise means for a shell that is
# mostly prebuilt binary. --dir stops before any archive: the ad-hoc signature
# has to go on before the zip is written, and electron-builder's own zip target
# would write it first.
step "electron-builder --mac --universal --dir"
( cd "$studio" && npx --no-install electron-builder --mac --universal --dir \
    -c.extraMetadata.version="$version" )

unpacked="${studio}/release/mac-universal/${BUNDLE_DIR}"
[ -d "$unpacked" ] || die "electron-builder produced no ${unpacked}"

# --- Stage ------------------------------------------------------------------

step "staging ${stage_name}"
rm -rf "$stage" "$zip_path"
mkdir -p "$stage"
# ditto, not cp -R: it round-trips a bundle's symlinks and extended attributes,
# which an Electron framework bundle is full of.
ditto "$unpacked" "$bundle"

cp "${script_dir}/READ-ME-FIRST.md" "${stage}/READ-ME-FIRST.txt"

step "codesign --force --deep --sign - (ad-hoc)"
# Ad-hoc: no Apple account, but it gives TCC a stable code identity to bind the
# Screen Recording grant to (ADR-0038). --deep because an Electron bundle nests
# the framework, the helpers and the player, and an unsigned nested binary makes
# the outer signature invalid. The identity is derived from the contents, so it
# changes on every rebuild and the tester re-grants each time - that cost is
# accepted and the zip README says so.
codesign --force --deep --sign - "$bundle"

step "ditto -c -k --sequesterRsrc --keepParent"
ditto -c -k --sequesterRsrc --keepParent "$stage" "$zip_path"

# --- Verify ------------------------------------------------------------------
#
# The bar packaging/macos/bundle.sh sets, applied to the studio's bundle, plus
# the two properties only this artifact has: a universal PLAYER inside it, and
# a bundled path that matches what resolve.ts looks at first. Any failure below
# is a failed package: the zip exists but must not ship.

step "verify"

studio_bin="${bundle}/Contents/MacOS/${APP_NAME}"
[ -f "$studio_bin" ] || die "missing ${studio_bin}"
studio_archs="$(lipo -archs "$studio_bin")"
case " $studio_archs " in *" arm64 "*) ;; *) die "studio binary is missing arm64 (lipo -archs: $studio_archs)" ;; esac
case " $studio_archs " in *" x86_64 "*) ;; *) die "studio binary is missing x86_64 (lipo -archs: $studio_archs)" ;; esac
check "studio lipo -archs: $studio_archs"

# The bundled player, at the path studio/electron/player/resolve.ts's FIRST
# candidate points at. No unit test can assert this: resolve.ts's own tests
# prove the ORDER, and only the archive proves the layout, so a tester
# configuring nothing is exactly this check.
bundled_player="${bundle}/Contents/Resources/player/${BIN_NAME}"
[ -f "$bundled_player" ] || die "missing the bundled player at ${bundled_player}"
player_archs="$(lipo -archs "$bundled_player")"
case " $player_archs " in *" arm64 "*) ;; *) die "bundled player is missing arm64 (lipo -archs: $player_archs)" ;; esac
case " $player_archs " in *" x86_64 "*) ;; *) die "bundled player is missing x86_64 (lipo -archs: $player_archs)" ;; esac
check "bundled player at Resources/player/: lipo -archs: $player_archs"

plutil -lint "${bundle}/Contents/Info.plist" >/dev/null \
    || die "plutil -lint rejected Info.plist"
check "plutil -lint"

codesign --verify --strict --deep "$bundle" \
    || die "codesign --verify --strict --deep rejected the bundle"
check "codesign --verify --strict --deep"

plist_version="$(plutil -extract CFBundleShortVersionString raw -o - \
    "${bundle}/Contents/Info.plist")"
[ "$plist_version" = "$version" ] \
    || die "plist CFBundleShortVersionString is '$plist_version', Cargo.toml says '$version'"
check "plist version matches [workspace.package]: $plist_version"

# Zip contents, read back out of the archive rather than off the staging dir -
# what ships is what ditto wrote, not what we think we staged.
entries="$(unzip -Z1 "$zip_path")"
for required in "${stage_name}/${BUNDLE_DIR}/" \
                "${stage_name}/${BUNDLE_DIR}/Contents/Resources/player/${BIN_NAME}" \
                "${stage_name}/READ-ME-FIRST.txt"; do
    echo "$entries" | grep -qF "$required" \
        || die "zip is missing entry: $required"
done
check "zip holds ${BUNDLE_DIR}, its bundled player and READ-ME-FIRST.txt"

# No .md anywhere: the tester's instructions ship as .txt so a double-click
# opens them.
if echo "$entries" | grep -q '\.md$'; then
    die "zip contains a .md file: $(echo "$entries" | grep '\.md$' | tr '\n' ' ')"
fi
check "no .md in the zip"

# Printed, never enforced. NFR section 4's studio row is a soft cap like the two
# beside it, and a release must not fail over a size.
bytes="$(wc -c < "$zip_path" | tr -d ' ')"
check "zip is ${bytes} B"

echo ""
echo "bundle-studio.sh: OK -> ${zip_path}"
