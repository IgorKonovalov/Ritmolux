# Releasing

How the version number moves and what pushing a tag builds. This is a **maintainer's page** — it
describes the process a release goes through, not anything a user or a preset author needs.

The scheme is decided in [ADR-0005](adrs/0005-versioning-and-release-cadence.md); what follows is
the operational summary.

## One version, one command, once per plan

- **Single source of truth:** root `Cargo.toml` `[workspace.package].version`. Every workspace
  member inherits it (`version.workspace = true`). **The studio holds two copies that
  `cargo-release` does not move** — `studio/package.json` `"version"` and
  `EXPECTED_PLAYER_VERSION` in `studio/shared/protocol.ts` — see "The studio's two copies" below.
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

### The studio's two copies follow, then the tag moves

`cargo release` edits `Cargo.toml` and `Cargo.lock` only. Straight after it, bring the studio's
two copies to the same version in their own commit, then move the tag onto that commit so a
build of the tag carries all three:

```sh
# edit "version" in studio/package.json and EXPECTED_PLAYER_VERSION in studio/shared/protocol.ts
(cd studio && npx vitest run shared/version.test.ts)   # holds all three equal
git commit -m "fix(studio): the two version copies follow the workspace to X.Y.Z" -- studio/package.json studio/shared/protocol.ts
git tag -a -f vX.Y.Z -m "chore: Release vX.Y.Z"
node scripts/check-release-tag.mjs     # the tag exists, is annotated, and is on HEAD's history
```

**Move the tag with `-a -f`, never with `git tag -d` and a plain `git tag`.** `-f` moves it; `-a`
keeps it an annotated tag object carrying the message `cargo release` wrote. A plain `git tag`
writes a *lightweight* tag, and the push below never sends one of those
([ADR-0203](adrs/0203-a-release-tag-is-annotated-and-origin-is-what-is-checked.md)).

**`EXPECTED_PLAYER_VERSION` is the copy that ships wrong.** The packaging scripts override
`package.json`'s version at build time, but the constant is compiled into the renderer bundle, so a
stale one ships a studio that refuses the player packaged beside it — a refusal indistinguishable
from a genuinely mismatched bundle. `studio/shared/version.test.ts` fails the studio's suite (and
the pre-push studio step) until both copies follow. Five separate `fix(studio)` sync commits
exist because this step lived nowhere a close would read it.

While in the `0.x` band: a feature-plan is a **minor** bump (`0.1.0 -> 0.2.0`), a fix-only
plan is a **patch** bump (`0.1.0 -> 0.1.1`), and a docs/chore-only plan legitimately gets
**no** bump (choose the level deliberately — this is not a missed step). Reaching `1.0.0` is
a deliberate future act (freezing the C ABI and standalone behavior), never backed into.

## Pushing the tag is what builds the artifacts

`cargo-release` writes the tag; **pushing it is what produces downloadable builds.** The user
pushes — the architect never does.

```sh
git push --follow-tags origin main
```

**`--follow-tags` sends annotated tags only**, and only those pointing at a commit the push carries.
A lightweight tag stays on your machine without a word, which is how eight of the ten tags stranded
on 2026-09-14 got there; two annotated ones were left behind by whatever push was actually run. So
the property is checked rather than the habit: the pre-push hook runs
`node scripts/check-release-tag.mjs`, which refuses the push while the version `main` declares has
no annotated tag on `HEAD`'s history, and CI's `links` job runs it with `--remote` on every push to
`main`, which goes red until `origin` advertises that tag as annotated.

**Push one tag, not a backlog of them.** GitHub does not start workflows for tags pushed in bulk, so
a `git push --tags` carrying more than three of them fires nothing at all — no Release run, no
artifacts, and no error to read. That is how `v0.113.0` came to exist on `origin` with zero Release
runs: the 2026-09-10 history rewrite force-pushed 131 tags in one command. The recovery is to delete
the remote ref and push it again on its own, because re-pushing an unchanged ref emits no event
either:

```sh
git push origin :refs/tags/vX.Y.Z
git push origin vX.Y.Z
```

That fires [`.github/workflows/release.yml`](../.github/workflows/release.yml)
([ADR-0038](adrs/0038-tag-driven-release-unsigned-universal-mac-app.md),
[ADR-0115](adrs/0115-the-foobar-component-is-a-released-artifact-with-a-parameterized-sdk.md)),
which builds the macOS, Windows and Linux standalone
([ADR-0131](adrs/0131-the-linux-standalone-captures-through-pulseaudios-simple-api.md)), the
foobar2000 component and the two studio zips
([ADR-0178](adrs/0178-the-studio-shell-conventions.md)) in parallel and, only if **every one** is
green, publishes a GitHub **prerelease** carrying five zips and one tarball:

```text
ritmolux-v<version>-macos-universal.zip          # universal .app, ad-hoc signed
ritmolux-v<version>-windows-x64.zip              # ritmolux.exe
ritmolux-v<version>-linux-x64.tar.gz             # ritmolux, x86_64, built on ubuntu-latest
ritmolux-v<version>-foobar2000-component.zip     # foo_ritmolux.fb2k-component, x64
ritmolux-studio-v<version>-macos-universal.zip   # universal Studio.app, ad-hoc signed
ritmolux-studio-v<version>-windows-x64.zip       # Ritmolux Studio.exe
```

Each carries a `READ-ME-FIRST.txt`; the standalone archives also carry a reference copy of
`presets/*.toml`. **The two studio zips carry a player of their own**, at `resources/player/`
inside the application, so a tester who takes the studio needs nothing else — that is the whole
reason the studio is a release artifact rather than a checkout-only tool. If any build fails, the
release job is **skipped** and no release exists — there is no half-published state. Re-running
the same tag's workflow replaces the assets rather than failing.

The publish step asserts exactly five zips **and** exactly one tarball, counted per kind, so a job
that silently stopped producing its artifact fails the release instead of shortening it — a count
of zips alone would pass a release that shipped nothing for Linux. Both `gh release` commands
upload the two kinds it counted.

**Every archive's name carries the version from `[workspace.package]`, the studio's included.**
`studio/package.json` has a `version` field of its own and `cargo-release` does not touch it, so
the two packaging scripts override it at build time (`-c.extraMetadata.version`) rather than read
it. The macOS script then reads the version back out of the built `Info.plist` and fails if it
disagrees with `Cargo.toml` — which is what stops the studio from ever shipping a stale version
string.

**A tag push is outward-facing.** Since ADR-0038 this is the last step of every close ceremony
whose tag gets pushed, so every plan close now publishes a public prerelease whether or not that
build was meant for anyone. `--prerelease` while in `0.x` softens the implication; it does not
remove it. If a close should *not* publish, do not push the tag — and expect `main`'s CI to go
red on its next push: the `links` job's release-tag step fails until that version's tag reaches
`origin`, or until the next close bumps the version past it. That red run is the documented cost of
holding a tag back, not a fault to chase.

**Editing anything under `.github/workflows/` needs the `workflow` OAuth scope** on the git
credential. Without it the push is rejected with a scope error that names neither the file nor
the fix:

```sh
gh auth refresh -s workflow
```

To rehearse the builds, run the workflow from the Actions tab (`workflow_dispatch`): it produces
every archive as **run artifacts**. Note that a `workflow_dispatch` is only offered once the
workflow file exists on the default branch.

**A dispatch never publishes, on any ref — a tag included.** The `release` job's condition names
the event as well as the ref, `if: github.event_name == 'push' && startsWith(github.ref,
'refs/tags/v')`, so the rehearsal is safe wherever you launch it and you do not have to pick the
ref carefully to stay out of trouble. Plan 0165 Phase 2 made that true; before it, the condition
read the ref alone, a dispatch on a `v*` tag satisfied it, and **the safest-looking rehearsal was
the one that published** — which is not hypothetical here: run `31955362251` published `v0.70.0`
from a dispatch on that tag.

**The consequence: a dispatch is no longer a way to publish a tag whose push produced no run.**
That recovery is the delete-and-re-push above, and it is now the only one.

The component job can also be rehearsed locally, and unlike the macOS bundle it runs on the
box this project is developed on:

```powershell
.\packaging\foobar\fetch-sdk.ps1          # once; idempotent
.\packaging\foobar\build-component.ps1    # same script, same checks, as CI runs
```

So can the Windows studio, which needs the same Spout SDK because the player it carries is built
with the feature:

```powershell
.\packaging\spout\fetch-sdk.ps1           # once; idempotent
.\packaging\studio\build-studio.ps1       # same script, same checks, as CI runs
```

The Linux tarball rehearses on any Linux box with pkg-config and libpulse's headers installed —
the same script and the same checks as CI's `linux` job:

```sh
bash packaging/linux/stage.sh             # --skip-build reuses target/release/ritmolux
```

It is **not** the shippable tarball when built anywhere newer than the runner: the binary requires
the glibc of the machine that linked it, and CI's `ubuntu-latest` is the floor the release notes
name.

The Windows studio's macOS sibling is `packaging/studio/bundle-studio.sh`, and like
`packaging/macos/bundle.sh` it needs a Mac — the ad-hoc signature, the `lipo` calls and the `plutil` assertions have no Windows
equivalent. Both take `--skip-build` / `-SkipBuild` to reuse the release binaries and the
installed `node_modules`, for iterating on the zip's layout without paying for an `lto = "fat"`
rebuild.

### A tag that did not reach origin

The gate reads only the version at the tip, so two bumps before one push can still strand the
older tag. List every local `v*` tag `origin` lacks — a listing, never a failure, and it names no
`batch/*` scratch ref:

```sh
node scripts/check-release-tag.mjs --stranded     # one line per tag: name, annotated or lightweight
```

Repair each one on the commit it already names, then push it **by name**, never with `--tags`:

```sh
git tag -a -f vX.Y.Z 'vX.Y.Z^{commit}' -m "chore: Release vX.Y.Z"   # only if it printed lightweight
git push origin vX.Y.Z
```

One tag per push publishes one release. Several in one push can fire nothing at all (see "Push
one tag" above), so if a whole backlog of them should reach `origin` *without* releases, disable
`release.yml` and `ci.yml` for that one push rather than rely on that suppression.

### While you are here: read the component's size

`foo_ritmolux.dll` carries a soft cap of its own — **12,582,912 B (12 MiB)**
([`docs/nfr.md`](nfr.md) §4, ADR-0159) — and it grew +910,848 B between Plan 0097 and Plan 0141
without anyone watching, none of it attributed as it landed. **You no longer have to measure it.**
The recipe above reads its own output's length and prints it beside the cap:

```
    foo_ritmolux.dll is 9789952 B (77.8 % of the 12582912 B cap)
```

Past **11,324,620 B** — 90 % of the cap — that step emits a warning instead of a check mark. It
warns and never dies: a release blocked on a byte count is one where someone edits the constant
under time pressure at a tag, which is worse than no gate because it also destroys the record.

**What is still yours is the row.** If the printed figure has moved more than **~100 KB** since the
last one, add a dated row to the size series in
[`docs/specs/0001-c-abi.md`](specs/0001-c-abi.md) and say what moved it — that table is the only
record of the trend, and a trend is what the cap is actually about. The reminder sits here rather
than only in that spec because the trigger it replaces — "re-measure when a dependency is added" —
was conditioned on an event that never happened, and the growth arrived anyway.

## What this does NOT touch

- **The C ABI version** (`RLX_ABI_VERSION`, `core-cabi/src/lib.rs`) is a **separate axis**
  (ADR-0003). It moves only when the `extern "C"` surface changes shape — never on an app
  bump, and an ABI bump never implies an app bump.
- **Dependency versions** (exact `=` pins, cargo-deny) are unrelated.
- **The foobar plugin's build.** `cargo-release` does not run it — but since
  [ADR-0025](adrs/0025-foobar-component-version-single-sourced.md) the component version is
  no longer independent: `plugin-foobar/build.ps1` reads `[workspace.package].version` out of
  root `Cargo.toml` and generates `build/foo_ritmolux_version.h`, which `DECLARE_COMPONENT_VERSION`
  consumes. So a bump here reaches foobar's Components list **on the plugin's next build**,
  with no second string to edit. (This revises ADR-0005's original "independent plugin
  version" note.) Since Plan 0102 that next build is the **tag push**, not a developer running
  `build.ps1` — and the packaging recipe fails the release if the DLL does not carry the
  workspace version, so the two cannot drift silently.
- **The pinned foobar2000 SDK** (`packaging/foobar/sdk-pin.ps1`) is a **separate axis** and
  never moves on an app bump. Moving it is its own commit, and it owes the on-device check in
  [`on-device-validation.md`](on-device-validation.md) — nothing in CI can load foobar2000.

## Where the version surfaces

- The standalone window title (`env!("CARGO_PKG_VERSION")`, resolves to the workspace
  version).
- The `vX.Y.Z` git tag and every release-zip name (NFR section 8).
- The macOS bundle's `CFBundleShortVersionString` / `CFBundleVersion`, substituted into
  `packaging/macos/Info.plist.in` at package time. `bundle.sh` asserts the plist and
  `[workspace.package]` agree, so a drift fails the build rather than shipping.
- The foobar component's `DECLARE_COMPONENT_VERSION`, via the generated
  `build/foo_ritmolux_version.h` (ADR-0025). `build-component.ps1` reads the version back out of the
  linked DLL and asserts it matches `[workspace.package]`, so — as with the macOS plist — a
  drift fails the build rather than shipping.

- The studio's `hello` check: `EXPECTED_PLAYER_VERSION` in `studio/shared/protocol.ts`, and
  `studio/package.json`'s `version`. **These two are edited by hand** at every bump (above) and
  held to `[workspace.package]` by `studio/shared/version.test.ts`.

Every other surface reads the one string in root `Cargo.toml` and is never edited by hand.
