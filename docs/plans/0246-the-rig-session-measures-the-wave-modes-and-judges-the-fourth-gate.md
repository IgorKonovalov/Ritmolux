# 0246 — The rig session measures the wave modes and judges the fourth gate

> **Status:** approved (2026-10-01, carried over from Plan 0202's approval of 2026-09-19). **Not
> queued until the owner has the Windows rig**: Phase 1 is an input to Phase 2, so it blocks, and a
> lane opened before it would only park.
> **Created:** 2026-10-01, split out of [Plan 0202](done/0202-the-three-mechanisms-get-their-gate.md)
> **Owner skill(s):** human, dev
> **Related ADRs:** [0113](../adrs/0113-milkdrop-presets-are-translated-ahead-of-time-onto-a-warp-mesh-idiom.md)
> (its third `Outcome` is the brief), [0199](../adrs/0199-a-converted-waveform-draws-the-sources-figure-at-the-hosts-scale.md)
> (whose inference Phase 2 tests), [0249](../adrs/0249-a-human-phase-may-be-owed-after-the-merge.md)
> (why Phases 3 and 4 do not block)
> **Takes:** design-backlog 0108 (its re-census instruction, Phase 4) and 0109 (the evidence its
> go/no-go waits on, Phase 3). **Neither entry is closed by this plan** - both stay live, each with a
> dated bullet naming what this plan recorded.

## TL;DR

These are Plan 0202's Phases 4-7, moved verbatim except for renumbering and the `Blocks merge` lines.
Each needs the Windows rig (`foo_vis_milk2` 0.2.0.0 under foobar2000) or the MilkDrop corpus, so
Plan 0202 closed on its first three phases instead of holding a lane for them.
- **Phase 1 (owner):** captures the eight wave modes on the rig.
- **Phase 2 (`dev`):** fits a per-mode waveform scale against those captures.
- **Phases 3 and 4 (owner):** the fourth look gate and the corpus census. Both are judgements or
  measurements that nothing after them reads, so the conductor merges without them and owes them.

## Context & problem

Plan 0202's Context carries the case in full: ADR-0113's third `Outcome` names three mechanisms, and
the third is a waveform scale confirmed on one mode. The unit-scale `nWaveMode = 0` capture reads
`k ~ 0.068` against the `0.158` fitted on mode 6, a ratio of `0.43`, which does **not** support
ADR-0199's inference that modes 0-5 share mode 6's gap. Plan 0202 settled the other two mechanisms:
its Phase 1 falsified the rate candidate, and its Phase 3 repaired the echo's orientation. What is left needs
the rig.

## Decision

**The order:** keep 0202's order and wording. The one change is that Phases 3 and 4 carry
`Blocks merge: no`, because their outputs are a verdict and a census that no later phase reads.

**How to run it:** the owner does Phase 1 on the Windows box and commits its eight rows into this
plan's log on `main`, then queues the plan. The conductor then starts at Phase 2, because a human
phase whose log row reads `done` is not waited for.

## Implementation phases

### Phase 1 — The eight modes are captured on the rig
- **Owner skill:** human
- **What:** On the rig ADR-0199 names — `foo_vis_milk2` 0.2.0.0 (DX11) under foobar2000 — capture each
  of the eight wave modes at unit `fWaveScale` from a stimulus whose sample value at the draw call is
  known (a full-scale sine, as Plan 0127's mode-6 capture used), so the per-mode host factor Phase 2
  fits has a measurement behind every mode. Modes 6 and 0 already have captures; re-take them in the
  same session so all eight share one host version and one stimulus.
- **Files touched:** the implementation log (one row per mode: host, version, mode, stimulus, date,
  and the peak-to-peak reading), and the capture files wherever the owner keeps the corpus.
- **Done when:** all eight modes have a recorded capture in the log with the five fields above, in
  one session on one host version.

### Phase 2 — The waveform scale is measured per mode
- **Owner skill:** dev
- **What:** ADR-0199 inferred that modes 0-5 share mode 6's `0.158` gap; the mode-0 capture reads
  `0.068`, a ratio of `0.43`. Measure the factor for each of the eight modes against the source's own
  construction and carry a per-mode value rather than one fitted constant.
- **Files touched:** `core/src/render/scenes/warp_mesh/draw.rs`,
  `core/src/render/scenes/warp_mesh/tests.rs`, `docs/milkdrop-conversion.md`
- **Done when:** each mode's factor is measured against its Phase 1 capture and recorded in the
  log with that capture named; the draw carries the per-mode values; a test asserts each mode's drawn extent against its
  measured factor as a ratio of like quantities (ADR-0074), not as a pixel figure; and the plan's log
  states plainly that ADR-0199's inference was falsified on mode 0, which is what the close records
  as that ADR's `Outcome`.

### Phase 3 — The fourth look gate
- **Owner skill:** human
- **Blocks merge:** no
- **What:** the same seven pairs, the same rig — `foo_vis_milk2` 0.2.0.0 (DX11), both renderers fed
  one track through foobar2000, judged live — re-converted at the tip this plan merges, with the per-pair
  reading written into the implementation log in the shape Plan 0100 Phase 7, Plan 0109 Phase 5 and
  Plan 0142 Phase 4 each used, so the four are comparable.
- **Files touched:** the implementation log.
- **Done when:** seven per-pair readings are recorded, each naming better / good / fixed / washed /
  wrong-on-structure as the previous three gates did; and the log says which of the three mechanisms
  each remaining bad pair is now attributed to, or that it is unattributed.

### Phase 4 — The corpus census is present-day
- **Owner skill:** human
- **Blocks merge:** no
- **What:** re-run `milkconv --report` and `milkconv --render` over `WORK/milkdrop-corpus` at the
  tip this plan merges, and record the conversion rate, the non-blank rate and the rejection ranking. Backlog
  0108's numbers (71, 218, 80.1 %) are a 2026-08-16 measurement taken before five plans changed what
  a converted preset renders, and both 0108 and 0109 instruct whoever takes them to re-measure first.
- **Files touched:** `docs/milkdrop-conversion.md` (its measured corpus tables), the implementation
  log.
- **Done when:** the operator doc's tables carry the present-day run with its date and machine
  beside the earlier eras rather than replacing them; the blank-render count is re-counted; and the
  log states whether the rejection ranking still puts disk textures at 88.7 % of failures.

## Risks & open questions

- **A fourth "not better" is a real possibility** (Plan 0202's Risks). Three mechanisms settled and a
  present-day census are worth having whichever way the gate reads.
- **The rig is a Windows box, and this project is developed on Linux.** Phase 1 is the whole reason
  the plan waits. If the rig cannot be had, Phase 2 stays unbuilt and ADR-0199's inference stays
  unverified on modes 1-5 and 7, which is the state of the tree today.

## What this plan does NOT do

- It does not buy reach. Backlog 0109 wants an ADR and an interview, and Phase 3's verdict triggers it.
- It does not lower HLSL arrays or hunt the blank-render list (backlog 0108's own work). Phase 4
  re-measures both.
- It does not reopen Plan 0202's settled mechanisms.

## Implementation log

**Lane:**

| phase | owner | state | commit |
|---|---|---|---|
| 1 — The eight modes are captured on the rig | human | not started | |
| 2 — The waveform scale is measured per mode | dev | not started | |
| 3 — The fourth look gate | human | not started | |
| 4 — The corpus census is present-day | human | not started | |

### Notes

### Close triggers

## Followups (after this lands)

- ADR-0199 gains an `Outcome` at this plan's close recording Phase 2's per-mode measurement.
- The reach decision (backlog 0109): an interview and an ADR, triggered by Phase 3's verdict.
