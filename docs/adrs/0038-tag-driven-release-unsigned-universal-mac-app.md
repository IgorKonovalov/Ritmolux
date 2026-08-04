# ADR-0038 — Distribution: a tag-driven GitHub Release carrying an unsigned, ad-hoc-signed universal macOS `.app`; standalone binaries only

> **Status:** accepted (Plan 0036, 2026-08-04)
> **Date:** 2026-07-26
> **Related plan(s):** [0036](../plans/done/0036-macos-and-windows-release-artifacts.md)

## Context

Nothing this repo produces can be handed to another person. `ci.yml` builds a **debug**
`cargo build` on `windows-latest` and `macos-latest` and throws the output away — there is no
`upload-artifact` step, no release job, no packaging directory. Distribution is roadmap item 5
in `docs/plans/README.md` and has never been designed. The immediate forcing request is
concrete: the user wants to send a friend something runnable on a Mac.

The macOS code path is complete but has **never executed on Apple hardware**.
`standalone/src/capture_mac.rs` (380 lines of ScreenCaptureKit) compiles in CI on every push;
Plan 0001 Phase 10 — the on-device validation — was deferred at the user's request and is
recorded as "the one outstanding piece of v1" (`docs/plans/README.md:1208`). So the first Mac
launch is simultaneously a distribution problem and the deferred validation.

Three platform facts shape the decision:

1. **Cross-building from the dev box is not available.** The dev machine is Windows. An
   `x86_64`/`aarch64-apple-darwin` link needs the Apple SDK and `ld64`, and the capture path
   binds Objective-C frameworks through `objc2`. A macOS runner is the only build host.
2. **macOS attributes the Screen Recording grant to the *launching* process.** ScreenCaptureKit
   is the only first-party system-audio tap and it requires that TCC grant. A loose Unix binary
   run from a shell is not an identity TCC can name — the prompt and the grant land on Terminal.
   A `.app` bundle with a `CFBundleIdentifier` is what makes the prompt say *this* app and the
   grant stick to it.
3. **The repository is public** (`IgorKonovalov/light-music-visualizer`). A GitHub **Release
   asset** is an unauthenticated download URL; a workflow **run artifact** is not — it requires
   a signed-in account with repo access. Since the recipient is a friend with neither, the
   delivery vehicle is not a free choice.

Against this, `docs/nfr.md` §8 already fixes the v1 posture: *unsigned, no installer, no code
signing; signing, if ever, is a later plan.* The target Mac's architecture is unknown.

## Decision

We will publish distribution artifacts from a **new, separate `.github/workflows/release.yml`**,
triggered by a pushed `v*` tag (with `workflow_dispatch` for artifact-only dry runs), and
containing three jobs: a macOS build, a Windows build, and a `release` job gated on both
(`needs:`) that runs only for a tag and attaches both zips to a GitHub **prerelease** via the
runner's preinstalled `gh` CLI.

The macOS artifact is a **universal** (`lipo`-fused `aarch64` + `x86_64`) binary wrapped in a
minimal `LightMusicVisualizer.app` — `Info.plist` plus `Contents/MacOS/lmv`, no icon — built by
a checked-in `packaging/macos/bundle.sh`. The plist's version fields are substituted from root
`Cargo.toml` `[workspace.package].version` at package time rather than duplicated, following
[ADR-0025](0025-foobar-component-version-single-sourced.md). The bundle is **ad-hoc code-signed**
(`codesign -s -`): free, requiring no Apple account, and it gives TCC and Gatekeeper a stable
code identity to bind a grant to — while remaining "unsigned" in the NFR §8 sense of no
Developer ID and no notarization. The bundle is zipped with `ditto -c -k --keepParent`, not
`zip`, because `zip` does not round-trip a bundle's symlinks and extended attributes.

Both zips carry the same three things: the executable form, a reference copy of the shipped
`presets/*.toml`, and a short `READ-ME-FIRST.txt`. Tester-facing instructions live **only** in
that zip file — no `docs/` page and no `on-device-validation.md` section, at the user's choice.

The scope is the **standalone frontend on both platforms**. The `.fb2k-component` is excluded:
the foobar2000 SDK is third-party, separately licensed, and `.gitignore`'d
(`plugin-foobar/sdk/`, `git ls-files` returns 0 files), so no runner can build the shim. It
remains a local `plugin-foobar/build.ps1` artifact.

We rejected cross-compiling from Windows (no Apple SDK or linker), a bare binary with no bundle
(TCC names Terminal, not us), a notarized DMG (a paid Apple account and a notary round trip,
against NFR §8), architecture-specific Mac builds (the target Mac is unknown), extending
`ci.yml` (mixes a gate with an artifact producer and slows every push), and run-artifacts
without a Release (a public Release asset is downloadable without an account; an artifact is
not).

## Consequences

### Positive

- A friend, or anyone, gets a plain download URL — no GitHub account, no toolchain, no Xcode.
- The universal binary removes an unknown: it runs on Apple Silicon and Intel, so a failed
  launch is never "wrong architecture".
- The `.app` makes the permission prompt legible ("Light Music Visualizer" wants to record the
  screen), which is the difference between a tester granting the permission and abandoning.
- The release becomes a by-product of the close ceremony that already exists: `cargo-release`
  writes the `vX.Y.Z` tag (`docs/releasing.md`), the user pushes it, artifacts appear. No second
  ritual to remember, and it lands squarely on roadmap item 5's standalone half.
- The bundle script is checked in, so packaging is reproducible locally on any Mac, not
  CI-only magic.

### Negative

- **An ad-hoc signature's identity is derived from the binary itself, so it changes on every
  rebuild.** TCC binds the Screen Recording grant to that identity: the tester must re-grant the
  permission for every new build we send them, and stale entries accumulate in System Settings.
  A Developer ID signature is what would make a grant survive updates — we are explicitly not
  buying that yet.
- Gatekeeper still quarantines a zip downloaded from a browser. Ad-hoc signing does not change
  that: the tester must strip the quarantine attribute or use right-click → Open. This is a real
  step a non-technical recipient can fail at, and the zip README carries the whole weight of
  explaining it.
- The universal binary roughly **doubles** the macOS download (the Windows release `lmv.exe`
  measured ~7.6 MB), which sits against NFR §4's smallness bias, and it doubles the Mac build
  time on top of `lto = "fat"` and `codegen-units = 1`.
- Every future plan close whose tag gets pushed now publishes a public prerelease, whether or
  not that build was meant for anyone. Marking releases `--prerelease` while in 0.x softens the
  implication but does not remove it.
- Two workflow files must now stay in step with `rust-toolchain.toml` and the crate layout.
  Pushing changes under `.github/workflows/` also requires the `workflow` OAuth scope on the
  user's git credential.
- Anyone downloading the Mac build is running an **unsigned, unnotarized, never-before-executed-
  on-macOS** binary. That is acceptable for one friend testing on purpose; it is not a posture to
  advertise more widely.

### Neutral

- `presets/*.toml` ships in the zip purely as a readable reference copy. It is not what the app
  loads: `core/build.rs` embeds the set at compile time ([ADR-0022](0022-build-time-preset-embedding.md))
  and the shell seeds an editable copy under `~/Library/Application Support/light-music-visualizer/`
  write-if-absent (`standalone/src/lib.rs:87`).
- The Release is a prerelease while the app is 0.x; promoting to a full release is a deliberate
  future act, like reaching 1.0.0 itself.

## Alternatives considered

### Alternative A — Cross-compile the Mac build from the Windows dev box

Would remove CI from the loop entirely. Rejected on availability, not preference: an
`*-apple-darwin` target needs the Apple SDK headers and `ld64`, which are not licensed for
redistribution to a Windows host, and `standalone`'s capture path links the ScreenCaptureKit,
CoreMedia and Foundation frameworks through `objc2`. A macOS runner is not a convenience here,
it is the only host.

### Alternative B — Ship the bare `lmv` binary, no bundle

Cheapest possible artifact: one `cargo build --release`, one file. Rejected because it breaks
the permission the app needs to do anything interesting. TCC identifies the *responsible*
process, so a binary launched from Terminal produces a prompt naming Terminal, grants the
permission to Terminal, and gives the tester no app to find in System Settings → Privacy →
Screen Recording. The bundle costs one plist and one `mkdir` tree; the permission is
load-bearing.

### Alternative C — A signed and notarized `.dmg`

The right answer for a public v1: no quarantine dance, a stable identity so grants survive
updates, and a familiar drag-to-Applications install. Rejected for now on cost and cadence — it
needs a paid Apple Developer account, a notary submission per build, and secrets in CI, and NFR
§8 explicitly defers signing to "a later plan + human task". Nothing in this decision blocks it:
the same `bundle.sh` gains signing and stapling steps when there is a certificate to use.

### Alternative D — Architecture-specific Mac builds

One `cargo build` instead of two, and a ~7 MB download instead of ~15 MB. Rejected because it
requires knowing the recipient's hardware before building, and getting it wrong produces the
single most confusing failure mode available — a binary macOS refuses to launch at all, or one
that silently runs under Rosetta 2 with different performance characteristics than we would be
measuring. Universal buys certainty for one extra compile.

### Alternative E — Extend `ci.yml` with a packaging job

Fewer files, no new workflow. Rejected because `ci.yml` is a **gate** — its jobs answer "is this
push sound?" and every one of them is expected to run on every push. A release-profile,
two-architecture, `lto = "fat"` build is minutes of work that answers a different question, and
a red packaging job would read as a broken gate. Separate trigger, separate file, separate
question.

### Alternative F — Upload run artifacts only, no Release

Simpler: `actions/upload-artifact` and stop. Rejected on the actual delivery requirement — a run
artifact is only downloadable by an authenticated user with access to the repository, so the
friend cannot fetch it and the user becomes a manual relay for every iteration. The repository
being public makes a Release asset a bare URL. Run artifacts are still produced, for the
`workflow_dispatch` dry-run path where no tag exists.

## Notes

- The degraded path already exists and is the likeliest first-launch outcome: if
  ScreenCaptureKit cannot start, `standalone/src/main.rs:677` prints "ScreenCaptureKit capture
  unavailable (…); rendering without audio" and the app renders silence-driven visuals rather
  than exiting. A permission failure therefore looks like "it opened but nothing reacts", not a
  crash — the zip README must pre-frame that, since a `.app` launched from Finder sends stderr
  to the unified log where a tester will never look.
- First-execution-on-macOS surface beyond capture: Metal adapter selection through wgpu, and
  glyphon's `FontSystem::new()` (`core/src/render/text.rs:79`) loading the system font set for
  the F3 overlay.

## Outcome (2026-08-04, at Plan 0036's close)

The decision stands unchanged and no code contradicts it. Two of the recorded consequences met
measurement for the first time, and **one of them is wrong**.

- **"It doubles the Mac build time" — falsified, and in the useful direction.** On the first dry
  run (`30944179623`, both caches cold), the `macos` job took **5m22s** building *two*
  architectures with `lto = "fat"` and `codegen-units = 1`, while the single-architecture
  `windows` job took **6m22s**. The universal build is not the slow one; it was the faster of the
  two. The plan's matching risk ("the Mac job will be slow — accept the wall-clock") priced a cost
  that did not arrive. Apple-silicon runners are the likeliest reason, but that is inference, not
  measurement — what is measured is one run on the `macos-latest` and `windows-latest` images of
  this date, and it should not be restated as a property of universal builds in general.
- **"Roughly doubles the macOS download" — confirmed, on the artifact rather than the binary.**
  The published zips are **6.33 MiB** (macOS universal) against **3.62 MiB** (Windows x64), a
  factor of 1.75. Note these are *zips carrying the presets and the README*, not the bare
  executables the "~7.6 MB" figure in Negative refers to; the two are not the same quantity, so
  the ratio agreeing is weaker evidence than it looks.
- **The build path is verified; the publish path is not.** Every check in
  `packaging/macos/bundle.sh` ran against real Apple tooling and passed. The `release` job did
  **not** run — a `workflow_dispatch` is gated out of it by design — so `gh release create`, the
  two-asset guard and the `--clobber` re-run path remain unexecuted. The first pushed tag is their
  first test.
- **The stale citation in Notes:** the degraded-path `eprintln!` is now
  `standalone/src/main.rs:998`, not `:677`. Left in place per the append-only rule; corrected here.
