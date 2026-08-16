# ADR-0115 — The foobar2000 component is a released artifact, and the SDK is a build parameter

> **Status:** accepted (2026-08-16, user approval)
> **Date:** 2026-08-16
> **Related plan(s):** [0102](../plans/done/0102-the-component-ships.md)

## Context

Half of this project's architecture exists to serve the foobar2000 plugin. [ADR-0001](0001-rust-core-wgpu-cabi-foobar-shim.md)
chose a Rust core behind a C ABI **because** a C++ shim had to link it; there is a versioned
twelve-function ABI ([`docs/specs/0001-c-abi.md`](../specs/0001-c-abi.md)), a conformance suite, a
coverage gate on `lmv-core-cabi` ([ADR-0072](0072-the-c-abi-ships-from-its-own-crate.md)), and a
component version single-sourced from the workspace version ([ADR-0025](0025-foobar-component-version-single-sourced.md)).

**None of it reaches a user.** [NFR §8](../nfr.md#8-distribution-v1) records the reason plainly:
"Standalone only — CI does not ship a `.fb2k-component`. The foobar2000 SDK is third-party,
separately licensed and `.gitignore`'d, so no runner can build the shim; it stays a local
`plugin-foobar/build.ps1` artifact." That sentence corrected an earlier promise, honestly, and
then nothing moved for sixty-odd plans.

The cost of that is larger than it looks. The foobar2000 component repository is a real
distribution channel with a captive enthusiast audience, and it is **nearly empty for this
category**: `foo_vis_milk2` (MilkDrop 2 ported to DX11) is the only serious visual component,
`Shpeck` bridges twenty-year-old Winamp plugins, and the rest are spectrum readouts. Meanwhile the
standalone app — the half that *does* ship — is unsigned, warned about by SmartScreen, and
competing against projectM's twenty-year content library. **The plugin is the weaker product in
capability and the far stronger position in distribution**, and it is the one that is not shipped.

The blocker is genuinely a licensing and infrastructure question rather than a technical one.
`build.ps1` already works: it builds `lmv-core-cabi`, builds three SDK projects with MSBuild, links
`foo_lmv.dll`, and can install it into a profile. What is missing is a route from that to an
artifact attached to a `v*` tag, on a build host that does not have the SDK sitting in a gitignored
directory.

## Decision

We will make the foobar2000 component a **released artifact, versioned with the application and
attached to the same `v*` tag**, produced by a **documented reproducible recipe** in
`packaging/foobar/` — and we will treat **how the SDK reaches the build host as a parameter of that
recipe**, not as a property of it.

The recipe takes an SDK either **fetched at build time** from foobar2000's published download, or
**pre-staged** at `plugin-foobar/sdk/`. Both paths produce the same `.fb2k-component` (a zip with
the component layout) from the same script; the difference is only where the input came from. A
`human` phase reads the SDK licence and decides which path the **published** build uses:

- if fetching is permitted, the release workflow gains a Windows job and the component ships
  automatically alongside the two existing zips;
- if it is not, the recipe runs locally, `docs/releasing.md` gains the step, and the artifact is
  attached by hand.

**The decision this ADR makes is that the component ships and that the recipe is one script either
way.** It deliberately does not make the licence call, because that is a reading of someone else's
terms and it is not the architect's to guess. What it forecloses is the outcome where the answer to
an unread licence keeps a finished component unreleased for another sixty plans.

## Consequences

### Positive

- **The project reaches an audience that already exists**, in a category with one incumbent, instead
  of competing for attention as an unsigned standalone download.
- **The whole C ABI investment starts paying.** Twelve versioned functions, a spec, a conformance
  suite and a second coverage gate currently serve an artifact nobody can install.
- **The component's version is already correct** — ADR-0025 single-sources it from the workspace
  version, so a released component cannot disagree with the app it was built beside.
- **The recipe is testable locally either way.** Unlike the macOS bundle
  ([ADR-0038](0038-tag-driven-release-unsigned-universal-mac-app.md)), which can only be exercised
  on a host the project does not own, this one runs on the dev box on every attempt.

### Negative

- **A fetched SDK is a supply-chain input.** If the release workflow downloads the SDK, the build
  depends on a third-party URL staying up and serving the same bytes; a pinned version and a
  checksum are the mitigation, and they are Phase 2's work rather than an afterthought.
- **The component is unsigned, like everything else here.** foobar2000 will load it, but Windows
  SmartScreen posture on the download is unchanged. Signing remains out of scope
  ([NFR §8](../nfr.md#8-distribution-v1)).
- **A third artifact means a third `READ-ME-FIRST`** and a third way for the release to be half
  done. The recipe either produces all three or the tag is incomplete.
- **The component's own quality is unmeasured by CI.** No runner can load foobar2000, so
  installation and `visualisation_stream` behaviour stay in
  [`docs/on-device-validation.md`](../on-device-validation.md) — a real gap, named rather than
  solved, exactly as the macOS path is.

## Alternatives considered

### Alternative A — vendor the SDK in the repository

Commit `plugin-foobar/sdk/` and let any runner build. **Rejected on licensing.** The SDK is
third-party and separately licensed; Plan 0001 deliberately gitignored it and documented how to
re-obtain it. Committing it would reverse that call on somebody else's terms, and a licence
question resolved by ignoring it is not resolved.

### Alternative B — a self-hosted Windows runner

Register the dev box as a GitHub Actions runner so the SDK can simply live on disk. **Rejected
because it makes a personal machine into release infrastructure.** Releases would then be possible
only when one computer is on, and a self-hosted runner on a workstation is the standard way repository
secrets end up somewhere they should not be. The cost is permanent; the problem it solves is a
one-line licence question.

### Alternative C — leave it as a local artifact

The status quo: `build.ps1` produces a DLL, the developer installs it, nobody else has it. **Rejected
because this is the thing being fixed.** It is also the outcome that happens by default if this ADR
is never written, which is why the decision is stated as "it ships" rather than as "we should
consider shipping it".

### Alternative D — put the component inside the Windows standalone zip

One download for both. **Rejected because they install differently** — a `.fb2k-component` is
dragged into foobar2000's preferences, an `.exe` is run — and bundling them makes both sets of
instructions longer and each audience read half a document that is not for them.

## Notes

- The component layout is a zip with a fixed structure; `packaging/macos/bundle.sh` is the model for
  the recipe — **build, assemble, stamp the version, package, and verify**, so the packaging step is
  reproducible on a developer's machine rather than being CI-only magic
  ([ADR-0038](0038-tag-driven-release-unsigned-universal-mac-app.md)).
- The SDK release currently in use is **2025-03-07** (`plugin-foobar/README.md`). Whichever path
  Phase 1 selects, the version is pinned rather than "latest": a component that silently rebuilt
  against a newer SDK would be an untested change riding a version bump that says nothing about it.

## Outcome (2026-08-16, [Plan 0102](../plans/done/0102-the-component-ships.md))

**The route this ADR left open resolved to the fetch branch, and the Decision above stands
unchanged.** Recorded here rather than edited into the body, per the append-only rule.

- **The licence question, answered.** Phase 1's `human` reading: the foobar2000 SDK licence is
  BSD-style. It permits redistribution in binary form and puts a notice obligation only on
  redistributed **source**. We ship the linked DLL, no SDK source, and nothing implying endorsement,
  so `.github/workflows/release.yml` gained a `foobar` job that fetches the pinned SDK. Alternative A
  still stands: the SDK is fetched, never committed.
- **The pin is stronger than the ADR assumed.** The fetched archive is byte-identical to the copy
  this project has built against since Plan 0001 (SHA-256 in `packaging/foobar/sdk-pin.ps1`,
  recomputed independently at the close), and foobar2000.org keeps every SDK release at a stable
  `/downloads/SDK-<date>.7z` URL back to 2011-03-11. The Negative section's supply-chain risk is a
  bytes-changed risk, not a URL-rot risk.
- **What the Negative section did not name, and should have.** The component was already carrying
  two filed pre-existing defects when it became a public artifact — a Default UI panel that attaches
  its surface at 1x1 and looks broken until a track change ([backlog 0102](../design-backlog.md)),
  and a context menu that shadows foobar2000's layout-edit menu so the panel cannot be removed by
  the documented route ([backlog 0103](../design-backlog.md)). Both are `Medium`, both pre-date this
  ADR, and both are what a stranger meets first. The shipped `READ-ME-FIRST.txt` now names them;
  neither is fixed. This is the risk a distribution decision owes its reader and this ADR did not
  carry it.
- **One assertion is weaker on the pre-staged route than on the fetch route**, which cuts against
  this ADR's claim that both paths are equivalent inputs to one recipe:
  `build-component.ps1` stamps the pinned SDK version into the shipped `READ-ME-FIRST.txt` while
  asserting nothing about what is actually staged. Filed as
  [backlog 0105](../design-backlog.md); the fix is one grep against the `sdk-readme.html` the SDK
  itself ships.
