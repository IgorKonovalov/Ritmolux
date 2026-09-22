#!/usr/bin/env bash
#
# Build, stage, archive and verify the Linux standalone (ADR-0131).
#
# Produces <target-dir>/dist/ritmolux-v<version>-linux-x64.tar.gz, whose single
# top-level folder holds the ritmolux binary, a reference copy of
# presets/*.toml, and READ-ME-FIRST.txt.
#
# Checked in rather than inlined into the workflow so packaging is reproducible
# on any Linux box, not CI-only magic - the same shape as packaging/macos/
# bundle.sh. The release workflow's linux job is a thin caller: install the
# build packages, run this, upload the tarball.
#
# The verification lives HERE rather than in the workflow, so a local run is
# held to the same bar as CI. Every check is fatal.
#
#   Usage:  packaging/linux/stage.sh [--skip-build]
#
#   --skip-build   Reuse <target-dir>/release/ritmolux on disk. For iterating on
#                  the layout without paying for a `lto = "fat"` rebuild; never
#                  used by CI.
#
# Building needs pkg-config and libpulse's headers (libpulse-dev on Ubuntu,
# libpulse on Arch), plus the xkbcommon and wayland headers winit links.
#
# The glibc the binary requires is the build machine's. CI builds on the
# ubuntu-24.04 runner, so that image's glibc is the floor the release notes
# name; a tarball built on a newer distribution will not start on an older one.

set -euo pipefail

BIN_NAME="ritmolux"

script_dir="$(cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(cd -- "${script_dir}/../.." && pwd)"

skip_build=0
for arg in "$@"; do
    case "$arg" in
        --skip-build) skip_build=1 ;;
        *) echo "stage.sh: unknown argument: $arg" >&2; exit 2 ;;
    esac
done

die() { echo "stage.sh: FAILED: $*" >&2; exit 1; }
step() { echo ""; echo "==> $*"; }
check() { echo "    ok: $*"; }

# --- Where cargo writes, which is not necessarily "${repo_root}/target" -------
#
# Asked of cargo rather than assumed, as bundle.sh does: the answer is right
# under the default layout and under any `build.target-dir`. `target_directory`
# is a top-level string in `--format-version 1`; a path holding a double quote
# or a backslash would defeat the sed pattern, and die below rather than
# stage into the wrong place.
read_target_dir() {
    cargo metadata --format-version 1 --no-deps --manifest-path "${repo_root}/Cargo.toml" |
        sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p'
}

target_dir="$(read_target_dir)"
[ -n "$target_dir" ] || die "could not read target_directory from cargo metadata"

# --- Version: the single source of truth, section-anchored (ADR-0025) ---------
#
# Anchored to [workspace.package]: a naive first-`version =` match would read a
# member crate's inherited line or a [profile] key and name the archive wrong.
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

stage_name="ritmolux-v${version}-linux-x64"
out_dir="${target_dir}/dist"
stage="${out_dir}/${stage_name}"
archive="${out_dir}/${stage_name}.tar.gz"

echo "stage.sh: version ${version} -> ${stage_name}.tar.gz"

# --- Build -------------------------------------------------------------------
#
# No `spout` feature: Spout is a Windows texture-sharing SDK, so `--stream` has
# no sender to publish through on this platform.
if [ "$skip_build" -eq 0 ]; then
    step "cargo build --release -p standalone"
    ( cd "$repo_root" && cargo build --release -p standalone )
fi

bin="${target_dir}/release/${BIN_NAME}"
[ -f "$bin" ] || die "missing $bin (drop --skip-build?)"

# --- Stage -------------------------------------------------------------------

step "staging ${stage_name}"
rm -rf "$stage" "$archive"
mkdir -p "${stage}/presets"
install -m 0755 "$bin" "${stage}/${BIN_NAME}"
# *.toml only: presets/README.md is the authoring reference, not preset content,
# and the archive is for a tester rather than an author. The app does not read
# these either way - core/build.rs embeds the set at compile time (ADR-0022) and
# the shell seeds an editable copy on first run.
cp "${repo_root}"/presets/*.toml "${stage}/presets/"
cp "${script_dir}/READ-ME-FIRST.md" "${stage}/READ-ME-FIRST.txt"

step "tar -czf (single top-level folder)"
# -C to the parent and the folder's own name as the only operand: that is what
# makes the folder the archive's single top-level entry.
tar -czf "$archive" -C "$out_dir" "$stage_name"

# --- Verify ------------------------------------------------------------------
#
# Read back out of the archive rather than off the staging directory - what
# ships is what tar wrote. Any failure below is a failed package: the archive
# exists but must not ship. The Windows job's checks, minus Spout's licence,
# which has no counterpart here.

step "verify"

entries="$(tar -tzf "$archive" | sed 's|^\./||')"

# Here-strings, not pipes, for the reason bundle.sh gives: `grep -q` exits at the
# first match, and under `set -o pipefail` the upstream writer then dies with
# EPIPE - a present entry reads as missing, and an absent one can read as fine.
for required in "${stage_name}/${BIN_NAME}" "${stage_name}/READ-ME-FIRST.txt"; do
    grep -qxF -- "$required" <<<"$entries" \
        || die "archive is missing top-level entry: $required"
done
check "archive top level holds ${BIN_NAME} and READ-ME-FIRST.txt"

if grep -qv "^${stage_name}/" <<<"$entries"; then
    die "archive has an entry outside ${stage_name}/: $(grep -v "^${stage_name}/" <<<"$entries" | head -3 | tr '\n' ' ')"
fi
check "single top-level folder: ${stage_name}/"

# The mode bit travels in the tar header; a binary that unpacks without it is
# one more step the tester has to be told about.
mode="$(tar -tvzf "$archive" "${stage_name}/${BIN_NAME}" | awk '{ print $1 }')"
case "$mode" in
    -rwx*) ;;
    *) die "${BIN_NAME} is not executable in the archive (mode ${mode})" ;;
esac
check "${BIN_NAME} is executable in the archive (${mode})"

repo_toml_count="$(ls -1 "${repo_root}"/presets/*.toml | wc -l | tr -d ' ')"
archive_toml_count="$(grep -c "^${stage_name}/presets/.*\.toml$" <<<"$entries" || true)"
[ "$archive_toml_count" -eq "$repo_toml_count" ] \
    || die "archive carries ${archive_toml_count} presets, repo has ${repo_toml_count}"
check "presets/: ${archive_toml_count} .toml files, matching the repo"

# No .md anywhere: presets/README.md is the one that would slip in. Fails closed:
# the here-string keeps an EPIPE from turning a hit into a miss.
if grep -q '\.md$' <<<"$entries"; then
    die "archive contains a .md file: $(grep '\.md$' <<<"$entries" | tr '\n' ' ')"
fi
check "no .md in the archive"

echo ""
echo "stage.sh: OK -> ${archive}"
