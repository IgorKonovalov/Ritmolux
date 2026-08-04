# Releasing

How the application version moves. The scheme is decided in
[ADR-0005](adrs/0005-versioning-and-release-cadence.md); this note is the operational
summary.

## One version, one command, once per plan

- **Single source of truth:** root `Cargo.toml` `[workspace.package].version`. Both crates
  inherit it (`version.workspace = true`); nothing else holds an app-version string.
- **Bump authority:** [`cargo-release`](https://github.com/crate-ci/cargo-release), a dev
  tool installed with `cargo install cargo-release` (not a workspace dependency). Config is
  in `release.toml`.
- **Cadence & owner:** one bump per shipped plan, run by the **architect in the close
  ceremony** — after the plan flips to `done` and its docs land. Not per phase commit.
- **No push:** `cargo-release` stages the version edit and writes the `vX.Y.Z` tag but does
  not push; the user pushes (project no-auto-push rule).

## Commands

```sh
# Preview — cargo-release is dry-run by default, so this changes nothing:
cargo release <patch|minor> --no-push

# Do it (bumps the workspace version, commits, tags vX.Y.Z, no push):
cargo release <patch|minor> --no-push --no-publish --no-confirm --execute
```

`--execute` is what makes it real; without it cargo-release only reports what it
would do. `push = false` / `publish = false` are already pinned in `release.toml`
— the explicit flags on the command line are belt-and-braces.

While in the `0.x` band: a feature-plan is a **minor** bump (`0.1.0 -> 0.2.0`), a fix-only
plan is a **patch** bump (`0.1.0 -> 0.1.1`), and a docs/chore-only plan legitimately gets
**no** bump (choose the level deliberately — this is not a missed step). Reaching `1.0.0` is
a deliberate future act (freezing the C ABI and standalone behavior), never backed into.

## Pushing the tag is what builds the artifacts

`cargo-release` writes the tag; **pushing it is what produces downloadable builds.** The user
pushes — the architect never does.

```sh
git push --follow-tags
```

That fires [`.github/workflows/release.yml`](../.github/workflows/release.yml)
([ADR-0038](adrs/0038-tag-driven-release-unsigned-universal-mac-app.md)), which builds the
macOS and Windows standalone in parallel and, only if **both** are green, publishes a GitHub
**prerelease** carrying two zips:

```text
light-music-visualizer-v<version>-macos-universal.zip    # universal .app, ad-hoc signed
light-music-visualizer-v<version>-windows-x64.zip        # lmv.exe
```

Each also carries a reference copy of `presets/*.toml` and a `READ-ME-FIRST.txt`. If either
build fails, the release job is **skipped** and no release exists — there is no half-published
state. Re-running the same tag's workflow replaces the assets rather than failing.

**A tag push is outward-facing.** Since ADR-0038 this is the last step of every close ceremony
whose tag gets pushed, so every plan close now publishes a public prerelease whether or not that
build was meant for anyone. `--prerelease` while in `0.x` softens the implication; it does not
remove it. If a close should *not* publish, do not push the tag.

**Editing anything under `.github/workflows/` needs the `workflow` OAuth scope** on the git
credential. Without it the push is rejected with a scope error that names neither the file nor
the fix:

```sh
gh auth refresh -s workflow
```

To rehearse the builds without publishing anything, run the workflow from the Actions tab
(`workflow_dispatch`): it produces both zips as **run artifacts** and creates no release. Note
that a `workflow_dispatch` is only offered once the workflow file exists on the default branch.

## What this does NOT touch

- **The C ABI version** (`LMV_ABI_VERSION`, `core/src/ffi.rs`) is a **separate axis**
  (ADR-0003). It moves only when the `extern "C"` surface changes shape — never on an app
  bump, and an ABI bump never implies an app bump.
- **Dependency versions** (exact `=` pins, cargo-deny) are unrelated.
- **The foobar plugin's build.** `cargo-release` does not run it — but since
  [ADR-0025](adrs/0025-foobar-component-version-single-sourced.md) the component version is
  no longer independent: `plugin-foobar/build.ps1` reads `[workspace.package].version` out of
  root `Cargo.toml` and generates `build/foo_lmv_version.h`, which `DECLARE_COMPONENT_VERSION`
  consumes. So a bump here reaches foobar's Components list **on the plugin's next build**,
  with no second string to edit. (This revises ADR-0005's original "independent plugin
  version" note.)

## Where the version surfaces

- The standalone window title (`env!("CARGO_PKG_VERSION")`, resolves to the workspace
  version).
- The `vX.Y.Z` git tag and both release-zip names (NFR section 8).
- The macOS bundle's `CFBundleShortVersionString` / `CFBundleVersion`, substituted into
  `packaging/macos/Info.plist.in` at package time. `bundle.sh` asserts the plist and
  `[workspace.package]` agree, so a drift fails the build rather than shipping.
- The foobar component's `DECLARE_COMPONENT_VERSION`, via the generated
  `build/foo_lmv_version.h` (ADR-0025).

All four read the one string in root `Cargo.toml` — none is edited by hand.
