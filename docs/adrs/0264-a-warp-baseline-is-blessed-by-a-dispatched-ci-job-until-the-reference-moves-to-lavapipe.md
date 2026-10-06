# ADR-0264 — A WARP baseline is blessed by a dispatched CI job, by name, until the reference moves to lavapipe

> **Status:** accepted 2026-10-06, Plan 0249
> **Date:** 2026-10-06
> **Related plan(s):** [0249](../plans/done/0249-a-dispatched-ci-job-blesses-the-named-warp-baselines.md),
> [0218](../plans/done/0218-the-reference-machine-becomes-arch.md)
> **Relates to:** [ADR-0242](0242-the-software-reference-rasterizer-is-lavapipe-and-a-warp-claim-is-re-measured.md)
> (the move this bridges to, and which retires it),
> [ADR-0071](0071-a-numeric-test-contract-states-a-property-or-names-its-machine.md) (a baseline names
> its machine), [ADR-0249](0249-a-human-phase-may-be-owed-after-the-merge.md) (the owed phases it
> serves)

## Context

The golden baselines are a measurement taken on DX12 WARP, and `bless_requested` in
`core/tests/common/mod.rs` panics on any other adapter. The only place WARP runs is Windows: the
owner's Windows box, and the `coverage` job in `.github/workflows/ci.yml`, which is also the only job
that actually compares the goldens. Every other arm skips them with a printed reading.

The development machine is now the Arch box (ADR-0241), and every 3D plan in flight moves a golden:
0248 moved four, 0240 moves `attractor_depth`, 0239 moves `swarm` and `swarm_shaped`, and 0237 adds
a space-tree golden written on llvmpipe. Each owes its bless as a `human` phase marked
`Blocks merge: no`, so the plans merge and `main`'s `coverage` job goes red until someone sits at the
Windows box. On 2026-10-06 it had been red since v0.164.0, and the red hides any other regression
that job would catch.

ADR-0242 already decided the real fix: the reference becomes lavapipe, blessed and gated on the Arch
box, and the Windows arm stops comparing. Plan 0218 carries it, and its Phase 3 asks the owner to
judge 44 recaptured baselines one by one. That is days of the owner's attention, not an afternoon, and
the 3D plans keep landing in the meantime.

## Decision

**We will bless WARP baselines from a `workflow_dispatch` job on `windows-latest`, which blesses only
the baselines it is named, reports what it compared, and uploads the result as an artifact for the
owner to judge and commit. It is a bridge, and it is retired when Plan 0218 Phase 2 moves the
reference to lavapipe.**

- **By name, never by failure.** `RLX_BLESS` takes a comma-separated list of baseline names, and
  `RLX_BLESS=1` keeps meaning "all". A name that matches no baseline fails the run, so a typo is not a
  silent no-op. The job's one required input is that list.
- **Compare first, then bless.** The job runs the golden roster once without blessing, records each
  named baseline's mean and max outlier against the committed one, and lists every *other* baseline
  over tolerance as still failing and not blessed. Then it blesses the named set.
- **The artifact is the output, and a person commits it.** The blessed PNGs and the report go up as
  one artifact, and the report is also the job summary. The job has `contents: read` and writes
  nothing to the repository.

## Consequences

### Positive

- An owed WARP bless no longer waits for the Windows box. It costs one dispatch, a download, and a look.
- `main`'s `coverage` job can be green between 3D plans, so it goes back to catching regressions.
- Naming the set keeps ADR-0242's rule that a baseline is judged, not blessed blind: the job cannot
  bless a regression nobody named, and the report shows what the bless replaced.

### Negative

- A second way to bless means a second path to keep honest. The job and the local `RLX_BLESS=1` must
  produce the same file for the same tree, and nothing but this ADR says so.
- `windows-latest` is a moving image. A runner update that changes WARP's output would move every
  baseline at once, and the job would show that only as a report full of drift. The local box is no
  better pinned, but it changes only when its owner updates it.
- It is work that ADR-0242 makes obsolete. Its value is the gap until Plan 0218 Phase 2 lands, and
  that gap is the owner's judging time.

## Alternatives considered

### Alternative A — Keep blessing on the Windows box by hand

Rejected because that is the state that produced the red `main`. Three plans owe a bless at once, and
the box is reachable but not at hand. "Owed after the merge" turns into "red for a week".

### Alternative B — Re-bless whatever fails

The job would compare, then re-bless every baseline over tolerance. Rejected because it blesses a
regression exactly as readily as an intended change, which is what the golden guard exists to tell
apart. Naming the set is the whole safeguard, and it costs the owner one line of typing.

### Alternative C — The job commits the baselines to a branch

Rejected because it gives CI write access to the repository for a task that needs a person's eyes
anyway. Downloading an artifact is one command, and an owner who has to look at the pictures loses
nothing by also copying them.

### Alternative D — Only unblock Plan 0218

Rejected as the *only* path, not as a path: Plan 0218 is re-ordered alongside this ADR. But its Phase
3 is 44 judgements by the owner, and the 3D plans would keep `main` red until all 44 are done.
