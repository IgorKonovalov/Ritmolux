# 0202 — The three mechanisms get their gate

> **Status:** approved
> **Created:** 2026-09-19
> **Approved:** 2026-09-19 (user)
> **Owner skill(s):** dev, human
> **Related ADRs:** [0113](../adrs/0113-milkdrop-presets-are-translated-ahead-of-time-onto-a-warp-mesh-idiom.md)
> (its third `Outcome` is this plan's brief),
> [0199](../adrs/0199-a-converted-waveform-draws-the-sources-figure-at-the-hosts-scale.md)
> (whose inference Phase 5 tests), [0019](../adrs/0019-eased-parameters.md) (the injected `dt` a
> per-second rate is converted against)
> **Takes:** design-backlog 0108 (its re-census instruction, Phase 7) and 0109 (the evidence its
> go/no-go waits on). **Neither entry is closed by this plan** — both stay live, each with a dated
> bullet naming what this plan recorded.

## TL;DR

The MilkDrop import's motivating claim — that the same preset should look better here — has come
back "not better" three times, and the third verdict is the first one that is not a single unknown:
it names three separate mechanisms, one per remaining bad pair. This plan settles each of the three
and then runs a fourth look gate on the same seven pairs, which is the evidence the reach work
(backlog 0109, 1 826 files) has been waiting on for three verdicts. The first visible behaviour is a
washed preset's ground level measured at two frame rates, which settles the candidate that a session
could not settle by eye.

## Context & problem

[ADR-0113](../adrs/0113-milkdrop-presets-are-translated-ahead-of-time-onto-a-warp-mesh-idiom.md)'s
third `Outcome` (2026-09-18, at Plan 0142's look gate) reads: of seven pairs against
`foo_vis_milk2` 0.2.0.0, **one better, two good, one fixed, two still washed at the ground, one
wrong on structure**. What changed is not the verdict but its shape — after five plans the honest
statement is *demonstrated on one pair, unfalsified on three, outstanding on three*, and each of the
three outstanding now names its own mechanism:

- **A rate candidate, unsettled.** Ours ran at 164-165 fps against the rig's frame cap. Plan 0142's
  log says the deposit is per frame and unconverted, so a rate mismatch would raise the field by
  `1/(1 - d)` — the shape of both remaining washed pairs. It also names the test: render one washed
  preset through `shot --render` at `--fps 30` and at `--fps 165` and compare the ground level.
  **The claim is not obviously true of this tree**: `Exposure` in
  `core/src/render/scenes/warp_mesh/draw.rs` exists precisely to convert a per-frame deposit through
  `dt * NOMINAL_FPS`, and it clamps at four nominal frames. So the probe may convict a path that
  bypasses it, or it may falsify the candidate. Either is a result.
- **An echo that is bound and does not nest.** *Songflower (Moss Posy)* sets `fDecay = 1.000`, so no
  wash repair can reach it; its `fVideoEchoAlpha`, `echo_zoom` and `echo_orient` are driven from its
  per-frame code and the reference's nested weave is absent from ours.
- **A waveform scale confirmed on one mode.** The unit-scale `nWaveMode = 0` capture reads
  `k ~ 0.068` against the `0.158` fitted on mode 6 — a ratio of `0.43`, which does **not** support
  ADR-0199's inference that modes 0-5 share mode 6's gap.

Both conversion-rate entries ride on this verdict. Backlog 0109 — disk textures, 88.7 % of every
conversion failure, 25x the reach of 0108 — states its own precondition as *"the fidelity work has
settled whether converted presets are worth having more of"*, and has now been unbought three times
by a verdict rather than left un-picked-up. A fourth gate with a route is what that precondition
needs; buying reach today would be deciding against yesterday's evidence.

## Decision

Take the three named mechanisms in the order their evidence is cheapest to get: the rate candidate
first, because it is a probe rather than a repair and it may falsify itself; then the echo, then the
per-mode waveform scale. Then re-run the look gate on the same seven pairs against the same rig, so
the fourth verdict is comparable to the three before it. **The reach decision is not in this plan**:
backlog 0109 asks for an ADR and an interview, and its trigger is this gate's verdict.

## Implementation phases

### Phase 1 — Settle the rate candidate
- **Owner skill:** dev
- **What:** run the probe Plan 0142's log names — one washed converted preset (`milk_wash_blur_mix_3`
  or `milk_wash_fog_tunnel`) through `shot --render` at `--fps 30` and at `--fps 165`, comparing the
  steady-state ground level — and read the result against `Exposure`'s existing conversion.
- **Files touched:** none necessarily; the readings go in the implementation log.
- **Done when:** the two ground levels are recorded with the preset, the command, the machine and the
  tree, and the log states one of two findings in words: **either** the levels agree within the
  project's declared drift floor, in which case the rate candidate is **falsified** and Phase 2 does
  not run, **or** they differ, in which case the phase names the deposit path that does not go
  through `Exposure` and what it deposits. Naming the path is the deliverable — repairing it is
  Phase 2.

### Phase 2 — Repair what Phase 1 convicted
- **Owner skill:** dev
- **What:** convert the deposit path Phase 1 named through the same `dt * NOMINAL_FPS` basis every
  other MilkDrop rate uses (ADR-0019), so a converted preset's ground level is the same at 30 and at
  165 fps. **This phase does not run if Phase 1 falsified the candidate** — say so in the log and
  move on.
- **Files touched:** `core/src/render/scenes/warp_mesh/draw.rs` or whichever path Phase 1 named,
  `core/src/render/scenes/warp_mesh/tests.rs`, any `milk_wash_*` golden baseline the repair moves
- **Done when:** the Phase 1 probe re-run shows the two ground levels agreeing; a test asserts the
  rate-independence as a **property** (the same preset at two frame rates reaches the same steady
  state) rather than as a frozen level; and every baseline the repair moved is re-blessed with the
  move named in the log.

### Phase 3 — The echo nests
- **Owner skill:** dev
- **What:** make `fVideoEchoAlpha`, `echo_zoom` and `echo_orient` produce the reference's nesting —
  the previous frame composited at the bound scale and orientation, at the bound alpha, as
  `xeiraex/milkdrop2` `d4c843a` composites it. *Songflower (Moss Posy)* is the case that reads wrong
  and the one to judge against.
- **Files touched:** `core/src/render/scenes/warp_mesh/` (the composite path and its shaders),
  `core/src/milk/outputs.rs` if the binding does not reach the composite, the scene's tests
- **Done when:** a converted *Songflower* draws a nested weave rather than the bare grid; a test
  asserts the echo composite reads its three bound parameters, each moved independently and each
  changing the frame in the direction the source's own arithmetic says; and no preset without an
  echo binding changes at all, shown by the golden set staying green.

### Phase 4 — The eight modes are captured on the rig
- **Owner skill:** human
- **What:** On the rig ADR-0199 names — `foo_vis_milk2` 0.2.0.0 (DX11) under foobar2000 — capture each
  of the eight wave modes at unit `fWaveScale` from a stimulus whose sample value at the draw call is
  known (a full-scale sine, as Plan 0127's mode-6 capture used), so the per-mode host factor Phase 5
  fits has a measurement behind every mode. Modes 6 and 0 already have captures; re-take them in the
  same session so all eight share one host version and one stimulus.
- **Files touched:** the implementation log (one row per mode: host, version, mode, stimulus, date,
  and the peak-to-peak reading), and the capture files wherever the owner keeps the corpus.
- **Done when:** all eight modes have a recorded capture in the log with the five fields above, in
  one session on one host version.

### Phase 5 — The waveform scale is measured per mode
- **Owner skill:** dev
- **What:** ADR-0199 inferred that modes 0-5 share mode 6's `0.158` gap; the mode-0 capture reads
  `0.068`, a ratio of `0.43`. Measure the factor for each of the eight modes against the source's own
  construction and carry a per-mode value rather than one fitted constant.
- **Files touched:** `core/src/render/scenes/warp_mesh/draw.rs`,
  `core/src/render/scenes/warp_mesh/tests.rs`, `docs/milkdrop-conversion.md`
- **Done when:** each mode's factor is measured against its Phase 4 capture and recorded in the
  log with that capture named; the draw carries the per-mode values; a test asserts each mode's drawn extent against its
  measured factor as a ratio of like quantities (ADR-0074), not as a pixel figure; and the plan's log
  states plainly that ADR-0199's inference was falsified on mode 0, which is what the close records
  as that ADR's `Outcome`.

### Phase 6 — The fourth look gate
- **Owner skill:** human
- **What:** the same seven pairs, the same rig — `foo_vis_milk2` 0.2.0.0 (DX11), both renderers fed
  one track through foobar2000, judged live — re-converted at this plan's tip, with the per-pair
  reading written into the implementation log in the shape Plan 0100 Phase 7, Plan 0109 Phase 5 and
  Plan 0142 Phase 4 each used, so the four are comparable.
- **Files touched:** the implementation log.
- **Done when:** seven per-pair readings are recorded, each naming better / good / fixed / washed /
  wrong-on-structure as the previous three gates did; and the log says which of the three mechanisms
  each remaining bad pair is now attributed to, or that it is unattributed.

### Phase 7 — The corpus census is present-day
- **Owner skill:** human
- **What:** re-run `milkconv --report` and `milkconv --render` over `WORK/milkdrop-corpus` at this
  plan's tip, and record the conversion rate, the non-blank rate and the rejection ranking. Backlog
  0108's numbers (71, 218, 80.1 %) are a 2026-08-16 measurement taken before five plans changed what
  a converted preset renders, and both 0108 and 0109 instruct whoever takes them to re-measure first.
- **Files touched:** `docs/milkdrop-conversion.md` (its measured corpus tables), the implementation
  log.
- **Done when:** the operator doc's tables carry the present-day run with its date and machine
  beside the earlier eras rather than replacing them; the blank-render count is re-counted; and the
  log states whether the rejection ranking still puts disk textures at 88.7 % of failures.

## Risks & open questions

- **Phase 1 may falsify its own candidate**, and then two washed pairs have no named mechanism and
  Phase 6's verdict will say so. That is the plan working, and it is why Phase 2 carries a
  do-not-run condition rather than an assumption.
- **Phase 3 is the least-scoped phase here.** The reference's echo is a composite stage this engine
  approximates; if reading `d4c843a` shows it is a larger divergence than a binding that does not
  reach the composite, the phase should stop and report rather than grow.
- **Phases 4, 6 and 7 are `human` and all need the rig or the corpus**, which live outside this
  checkout. Under the conductor the plan parks in front of Phase 4, after Phases 1-3, and again
  in front of Phase 6; that is expected and correct.
- **A fourth "not better" is a real possibility.** The plan's value does not depend on the verdict
  going the other way: three mechanisms settled and a present-day census are worth having whichever
  way the gate reads, and a fourth no-go with all three attributed is a much stronger statement than
  the first one was.

## What this plan does NOT do

- It does not buy reach. Backlog 0109 wants an ADR and an interview and its trigger is the verdict
  this plan produces; writing that ADR now would be deciding against yesterday's evidence.
- It does not lower HLSL arrays or hunt the blank-render list (backlog 0108's own work). Phase 7
  re-measures both so whoever takes them is working from today's numbers.
- It does not touch the disk-texture exclusion in `milkconv/src/shader/emit.rs`, which stays a named
  rejection class.

## Implementation log

**Lane:** _(to be filled by `dev`)_

| phase | owner | state | commit |
|---|---|---|---|
| 1 — Settle the rate candidate | dev | not started | |
| 2 — Repair what Phase 1 convicted | dev | not started | |
| 3 — The echo nests | dev | not started | |
| 4 — The eight modes are captured on the rig | human | not started | |
| 5 — The waveform scale is measured per mode | dev | not started | |
| 6 — The fourth look gate | human | not started | |
| 7 — The corpus census is present-day | human | not started | |

### Notes

### Close triggers

- **`presets/` touched:**
- **Plan header `Closes:`** none — see `**Takes:**`; backlog 0108 and 0109 both stay live.
- **What shipped:**
- **Operator docs touched:**
- **Backlog probes (`node scripts/check-backlog-claims.mjs`):**
- **Full suite:**
- **Outstanding `human` phases:**

## Followups (after this lands)

- The reach decision (backlog 0109) — an interview and an ADR, triggered by Phase 6's verdict.
- ADR-0199 gains an `Outcome` at this plan's close recording Phase 5's per-mode measurement.
