# 0102 — The component ships

> **Status:** draft
> **Created:** 2026-08-16
> **Owner skill(s):** dev, human
> **Related ADRs:** [0115](../adrs/0115-the-foobar-component-is-a-released-artifact-with-a-parameterized-sdk.md) (the foobar2000 component is a released artifact, and the SDK is a build parameter)

## TL;DR

The foobar2000 component becomes a **released artifact**, versioned with the app and attached to
the same `v*` tag, built by one reproducible recipe in `packaging/foobar/`. A `human` phase reads
the SDK licence and decides whether the release workflow fetches the SDK or whether the recipe runs
locally; **both paths use the same script**, so the licence answer selects a route rather than
changing the design. This is the smallest real distribution win available to this project and it
has been one unread licence away for sixty plans.

## Context & problem

Half this architecture exists for the plugin. [ADR-0001](../adrs/0001-rust-core-wgpu-cabi-foobar-shim.md)
chose a Rust core behind a C ABI *because* a C++ shim had to link it. There is a versioned
twelve-function ABI with a living spec, a conformance suite, a second coverage gate on
`lmv-core-cabi`, and a component version single-sourced from the workspace version
([ADR-0025](../adrs/0025-foobar-component-version-single-sourced.md)). `plugin-foobar/build.ps1`
works today: it builds the C ABI staticlib, builds three SDK projects with MSBuild, links
`foo_lmv.dll` and can install it into a profile.

**No user can get it.** [NFR §8](../nfr.md#8-distribution-v1) says so plainly — the SDK is
third-party, separately licensed and gitignored, so no runner can build the shim, and the component
"stays a local `plugin-foobar/build.ps1` artifact". That sentence honestly corrected an earlier
promise and then nothing moved.

The opportunity cost is the part worth stating. The foobar2000 component repository is a real
channel with a captive audience and **almost no competition in this category** — `foo_vis_milk2` is
the only serious visual component, `Shpeck` bridges twenty-year-old Winamp plugins, and everything
else is a spectrum readout. Meanwhile the half that *does* ship is an unsigned standalone `.exe`
that SmartScreen warns about, competing against a twenty-year content library. The plugin is the
weaker product in capability and much the stronger position in distribution, and it is the one
nobody can install.

## Decision

Per [ADR-0115](../adrs/0115-the-foobar-component-is-a-released-artifact-with-a-parameterized-sdk.md):
the component ships, versioned with the app, from one recipe whose **SDK source is a parameter**
(fetched at build time, or pre-staged locally). We rejected **vendoring the SDK** (it is separately
licensed and was deliberately gitignored — a licence question resolved by ignoring it is not
resolved), a **self-hosted runner** (a personal workstation becomes release infrastructure, and a
permanent cost to answer a one-line licence question), **leaving it local** (the status quo being
fixed), and **bundling it into the Windows standalone zip** (different install mechanism, and both
sets of instructions get worse).

## Architecture diagram

```mermaid
flowchart TB
    subgraph inputs["SDK acquisition — the parameter"]
        fetch["fetched at build time<br/>pinned version + checksum"]
        staged["pre-staged at plugin-foobar/sdk/"]
    end

    fetch --> recipe
    staged --> recipe

    subgraph recipe["packaging/foobar/ — one script, both routes"]
        build["build.ps1<br/>lmv-core-cabi → foo_lmv.dll"]
        build --> stamp["stamp version from<br/>[workspace.package] (ADR-0025)"]
        stamp --> pack["assemble .fb2k-component"]
        pack --> verify["verify: layout, version, exports"]
    end

    verify --> rel["v* tag release<br/>beside the two existing zips"]
```

## Implementation phases

### Phase 1 — read the licence

- **Owner skill:** human
- **What:** Read `plugin-foobar/sdk/sdk-license.txt` and the terms on the foobar2000 SDK download
  page, and answer one question: **may an automated build fetch and use the SDK, and may the
  resulting component be redistributed?**
- **Done when:** the user states the answer, and it selects Phase 3's route. If the answer is
  ambiguous, the ambiguous case is the manual route — this plan does not gamble on a reading of
  someone else's terms.

### Phase 2 — the recipe

- **Owner skill:** dev
- **What:** `packaging/foobar/` produces a versioned `.fb2k-component` from a pre-staged SDK,
  reproducibly, on the dev box.
- **Files touched:** `packaging/foobar/` (new), `plugin-foobar/build.ps1` (factor out the packaging
  step), `plugin-foobar/README.md`.
- **Notes for the implementer:** `packaging/macos/bundle.sh` is the model — **build, assemble,
  stamp, package, and verify**, so packaging runs the same on a developer's machine as in CI rather
  than being CI-only magic ([ADR-0038](../adrs/0038-tag-driven-release-unsigned-universal-mac-app.md)).
  Pin the SDK release (**2025-03-07** today) rather than tracking latest: a component that silently
  rebuilt against a newer SDK is an untested change riding a version bump that says nothing about
  it.
- **Done when:** one command produces a `.fb2k-component` whose declared version equals the
  workspace version, and whose verify step fails loudly if the layout is wrong or the version was
  not substituted.

### Phase 3 — the route Phase 1 chose

- **Owner skill:** dev
- **What:** If fetching is permitted, `.github/workflows/release.yml` gains a Windows job that
  fetches the pinned SDK against a checksum, runs the Phase 2 recipe, and attaches the component to
  the tag. If it is not, `docs/releasing.md` gains the manual step and the recipe's output is
  attached by hand.
- **Files touched:** `.github/workflows/release.yml` **or** `docs/releasing.md`; `docs/nfr.md` §8
  either way.
- **Notes for the implementer:** editing `.github/workflows/*` needs a git credential carrying the
  `workflow` OAuth scope, or the push is rejected — `gh auth refresh -s workflow` is the fix.
- **Done when:** a `v*` tag produces three artifacts rather than two (or two plus a documented
  manual step), and **[NFR §8](../nfr.md#8-distribution-v1)'s "CI does not ship a
  `.fb2k-component`" paragraph is corrected** — it is the authority on what a tag ships and it will
  be wrong the moment this lands.

### Phase 4 — what a recipient reads

- **Owner skill:** dev
- **What:** A `READ-ME-FIRST` for the component in the shape the two existing zips already carry:
  what it is, how to install it into foobar2000, which foobar2000 versions it targets, and that it
  is unsigned.
- **Files touched:** `packaging/foobar/READ-ME-FIRST.md`, `README.md`, `plugin-foobar/README.md`.
- **Done when:** a reader who has never seen this project can install the component from the zip
  without opening the repository.

### Phase 5 — it actually installs

- **Owner skill:** human
- **What:** Install the released artifact into a clean foobar2000 profile — not the dev profile
  `build.ps1 -Install` writes to — and play something.
- **Done when:** the component loads, reports the right version in foobar2000's component list,
  renders from `visualisation_stream`, and survives a track change and a preset switch. Record the
  result in [`docs/on-device-validation.md`](../on-device-validation.md), which is where everything
  CI cannot run lives.

## Risks & open questions

- **Phase 1 may say no**, and the plan still completes — it lands on the manual route, which is
  strictly better than today. The plan is written so that answer is a branch rather than a failure.
- **A fetched SDK is a supply-chain input.** Pin the version and check the hash; a URL that changes
  its bytes is otherwise an untested component riding a routine release.
- **The component is unsigned**, like everything else here. Out of scope
  ([NFR §8](../nfr.md#8-distribution-v1)).
- **Nothing in CI can test it.** No runner loads foobar2000, so Phase 5 is the only functional
  check and it is manual, exactly as the macOS path is.
- **Contention: none.** `packaging/`, `plugin-foobar/` and `docs/releasing.md` are touched by
  nothing on the roster. This runs in parallel with any other lane.

## What this plan does NOT do

- **No signing, no notarization, no installer.**
- **No submission to the foobar2000 component repository** — that is a separate, human, external
  process and it belongs with [Plan 0103](0103-the-project-gets-an-audience.md).
- **No macOS plugin.** foobar2000's SDK is Windows-centric per ADR-0001, unchanged.
- **No C ABI change.**

## Followups (after this lands)

- Submit to the foobar2000 component repository ([Plan 0103](0103-the-project-gets-an-audience.md)).
- Revisit whether the component's preset directory should be shared with the standalone app's, once
  real users have both installed.
