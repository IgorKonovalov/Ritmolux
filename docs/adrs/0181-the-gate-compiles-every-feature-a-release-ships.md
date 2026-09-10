# ADR-0181 — The per-push gate compiles every feature a release ships

> **Status:** accepted 2026-09-10
> **Date:** 2026-09-10
> **Related plan(s):** [0165-the-release-path-stops-being-the-first-compile](../plans/done/0165-the-release-path-stops-being-the-first-compile.md)

## Context

`standalone` carries one off-by-default feature, `spout`, which compiles the Spout video-out and
the C++ shim behind it ([ADR-0125](0125-the-live-video-out-is-a-spout-sender-fed-by-a-frame-tap.md)). The
shipped Windows binary is built **with** it: `release.yml`'s `windows` job stages the pinned SDK
and runs `cargo build --release -p standalone --features spout`, because that flag is what makes
`ritmolux --stream` exist in the artifact a tester downloads.

Nothing compiles that feature before a `v*` tag is pushed. `ci.yml` contains no occurrence of the
string `spout`; neither does `.githooks/pre-push`. So the release workflow's `windows` job is the
**first** compile of the code path — and it is also the job that publishes, which means the first
compile and the public artifact are the same event.

That has already cost a release. On 2026-09-09 `v0.112.0` was tagged and pushed; run `34397644084`
came back `foobar` green, `macos` green, `windows` **failed**:

```text
error[E0425]: cannot find function `start_capture` in the crate root
   --> standalone\src\stream.rs:348:26
```

The `release` job is gated on `needs: [macos, windows, foobar]`, so it was skipped, and **no
GitHub Release for `v0.112.0` was ever created**. The defect was repaired seven commits later by
`e9b27a2` during the Plan 0158 merge — incidentally, as part of unrelated work, by a refactor that
moved the function into `capture_start` and updated the gated call site to
`crate::capture_start::start_capture`. No gate caught it and nobody went looking; the repair and
the break simply passed each other.

The reason a default build cannot see this class is structural: code behind
`#[cfg(feature = "spout")]` is **not type-checked when the feature is off**. Every `cargo build`,
every `clippy --all-targets`, every `nextest` run in this repository is therefore blind to it. The
specific failure — an item moving modules while a gated call site keeps naming the old path — is
the most ordinary refactor there is, and it is invisible by construction.

The tension this decision has to resolve is with
[ADR-0038](0038-tag-driven-release-unsigned-universal-mac-app.md), whose Alternative E rejected
extending `ci.yml` with a packaging job, on the grounds that `ci.yml` is a **gate** — every job in
it answers *"is this push sound?"* and is expected to run every time — while a release-profile,
two-architecture, `lto = "fat"` build is minutes of work answering a different question, where a
red job would read as a broken gate.

## Decision

We will add a `spout` job to `ci.yml`: on `windows-latest`, stage the pinned Spout SDK, then run
`cargo check -p standalone --features spout`. A **compile check in the dev profile**, not a
packaging step.

ADR-0038's boundary holds, because the two jobs answer different questions. What `release.yml`
owns is the release-profile, two-architecture, publishable build — and that stays there. What a
gate owns is *does this source compile*, and a feature the shipped binary enables is source the
gate has to cover. A `#[cfg]` block nothing compiles is not covered by a gate that claims to
compile the workspace; it is a hole in the claim.

## Consequences

### Positive
- The `E0425` class is caught at push instead of at publish. Any refactor that moves an item out
  from under a gated call site fails on the push that wrote it, next to the commit that caused it.
- Wall clock is effectively unchanged. `check (windows-latest)` took **19m41s** on run
  `34448163324`; a separate job runs concurrently with it, so the critical path does not move.
- The job's name states what it guards. A reader scanning a red run learns which surface broke
  without opening a log.

### Negative
- **The gate gains a network fetch on every push.** `packaging/spout/fetch-sdk.ps1` downloads a
  third-party SDK, so a push can now go red for a reason outside this repository — a CDN blip
  reads as a broken gate, which is precisely the failure mode ADR-0038 Alternative E was
  protecting `ci.yml` from. This is the price, and it is the reason this is an ADR rather than a
  comment. The mitigation available is a cache keyed on the pinned hash; Plan 0165 Phase 1 decides
  whether to take it on first evidence rather than speculatively.
- **A `cargo check` is weaker than the build it stands in for.** It type-checks; it does not link,
  and it does not run at `lto = "fat"`. A link error or an LTO-only failure still surfaces first at
  the tag push. This buys the defect class that actually bit us, not the whole class of
  release-only failures, and the ADR should not be read as claiming otherwise.
- **Another `windows-latest` runner on every push**, which is billed minutes for a path that
  changes rarely.

### Neutral
- The job is Windows-only, because Spout is a DirectX facility. There is no macOS or Linux arm to
  add, so the matrix does not grow.

## Alternatives considered

### Alternative A — One more step inside the existing `check (windows-latest)` job
Fewest moving parts: the SDK fetch and a `cargo check` appended to a job that already runs on
Windows. Rejected on two counts. It serializes the network fetch into the **longest job in the
gate** (19m41s), so the critical path grows by exactly what the fetch and shim compile cost; and
it buries the thing being guarded inside a job named `check`, where a failure names the wrong
subject.

### Alternative B — Leave CI alone and document a pre-tag manual build
Write into `docs/releasing.md` that the releaser builds `--features spout` locally before pushing
a tag. Rejected because this is, in effect, what we already had, and it failed in the only way
that matters: the step was not written down, it did not happen before `v0.112.0`, and the
consequence is a release that **silently does not exist**. The `release` job is skipped by its
`needs:`, so there is no red release and no announcement — just an absent artifact nobody is
looking for. A manual step whose omission is invisible is not a control.

### Alternative C — Run the real thing: `cargo build --release -p standalone --features spout`
Catches link and LTO failures too, which the chosen check does not. Rejected as ADR-0038
Alternative E verbatim: an `lto = "fat"` release build is minutes of gate time answering the
packaging question. The dev-profile check buys the observed defect class at a fraction of the cost,
and the tag push remains the full-fidelity build by design.

## Outcome — 2026-09-10, at Plan 0165's close review

Three findings from reviewing the implementation. The first is the decision working; the other two
are facts the Context above understates, recorded here rather than edited into the body so that what
was known when the decision was taken stays readable.

**The gate and the real build agree, which had never been tested.** The `spout` job went green on CI
run `34467912506` in **3m30s**, against `check (windows-latest)`'s 25m on the same run — so the
"wall clock does not move" claim in Positive holds with room. Release run `34468008655` then built
the same feature at release profile and `lto = "fat"`, and its `windows` job — the one that killed
both `v0.108.0` and `v0.112.0` — succeeded and packaged. That is the first occasion the dev-profile
check and the full build have both run on this feature, and the Negative section's accepted gap
(a check does not link) went untested rather than unfound.

**The `E0425` cost two releases, not one.** `v0.108.0` failed on 2026-09-05 with the identical
`cannot find function start_capture in the crate root` — run `33992293177`, `windows` red at the
`Build, stage, zip and verify` step, `macos` and `foobar` green, `release` skipped. That is four
days and four tags before `v0.112.0`. So the defect survived **two** release attempts before an
unrelated refactor repaired it, and the Context's "worked example" framing reads as a single
incident. This strengthens the decision rather than qualifying it: the window between a gated call
site breaking and anyone noticing was weeks, not days.

**The sibling finding's hazard was already realized.** ADR-0181's Notes and
[backlog 0194](../design-backlog-archive.md) describe
a `workflow_dispatch` on a tag ref as a latent publish path. It is not latent — run `31955362251`
**published `v0.70.0`** from a dispatch on that tag, `release` job green in 7 s. It appears to have
been used deliberately, to recover a tag whose own push produced no Release run.

That second fact carries a consequence for Plan 0165 Phase 2, which is worth stating because the
plan does not: **narrowing the condition to the push event removes that recovery.** After Phase 2
the only way to publish a tag whose push fired no run is the delete-and-re-push in
`docs/releasing.md`. The narrowing is still right — a rehearsal that can publish is not a
rehearsal — but it is a net removal of one path, not a pure tightening, and the recovery it leaves
standing is now load-bearing.

## Notes

- The reduction behind the "nothing compiles it" claim is backlog entry 0193's probe:
  `absent: spout in: .github/workflows/ci.yml`. It goes red on the commit that discharges it,
  which is the intended signal.
- The failing release run is `34397644084`; the green post-repair rehearsal, dispatched on `main`
  at `037a0a4`, is `34450413424` — `windows`, `macos` and `foobar` all succeeded and `release` was
  skipped.
