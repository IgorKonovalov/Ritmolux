# 0202 — The three mechanisms get their gate

> **Status:** in-progress
> **Created:** 2026-09-19
> **Approved:** 2026-09-19 (user)
> **Owner skill(s):** dev, human
> **Related ADRs:** [0113](../adrs/0113-milkdrop-presets-are-translated-ahead-of-time-onto-a-warp-mesh-idiom.md)
> (its third `Outcome` is this plan's brief),
> [0199](../adrs/0199-a-converted-waveform-draws-the-sources-figure-at-the-hosts-scale.md)
> (whose inference Phase 4 tests), [0019](../adrs/0019-eased-parameters.md) (the injected `dt` a
> per-second rate is converted against)
> **Takes:** design-backlog 0108 (its re-census instruction, Phase 6) and 0109 (the evidence its
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

### Phase 4 — The waveform scale is measured per mode
- **Owner skill:** dev
- **What:** ADR-0199 inferred that modes 0-5 share mode 6's `0.158` gap; the mode-0 capture reads
  `0.068`, a ratio of `0.43`. Measure the factor for each of the eight modes against the source's own
  construction and carry a per-mode value rather than one fitted constant.
- **Files touched:** `core/src/render/scenes/warp_mesh/draw.rs`,
  `core/src/render/scenes/warp_mesh/tests.rs`, `docs/milkdrop-conversion.md`
- **Done when:** each mode's factor is measured and recorded in the log with the capture it came
  from; the draw carries the per-mode values; a test asserts each mode's drawn extent against its
  measured factor as a ratio of like quantities (ADR-0074), not as a pixel figure; and the plan's log
  states plainly that ADR-0199's inference was falsified on mode 0, which is what the close records
  as that ADR's `Outcome`.

### Phase 5 — The fourth look gate
- **Owner skill:** human
- **What:** the same seven pairs, the same rig — `foo_vis_milk2` 0.2.0.0 (DX11), both renderers fed
  one track through foobar2000, judged live — re-converted at this plan's tip, with the per-pair
  reading written into the implementation log in the shape Plan 0100 Phase 7, Plan 0109 Phase 5 and
  Plan 0142 Phase 4 each used, so the four are comparable.
- **Files touched:** the implementation log.
- **Done when:** seven per-pair readings are recorded, each naming better / good / fixed / washed /
  wrong-on-structure as the previous three gates did; and the log says which of the three mechanisms
  each remaining bad pair is now attributed to, or that it is unattributed.

### Phase 6 — The corpus census is present-day
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
  Phase 5's verdict will say so. That is the plan working, and it is why Phase 2 carries a
  do-not-run condition rather than an assumption.
- **Phase 3 is the least-scoped phase here.** The reference's echo is a composite stage this engine
  approximates; if reading `d4c843a` shows it is a larger divergence than a binding that does not
  reach the composite, the phase should stop and report rather than grow.
- **Phases 5 and 6 are `human` and both need the rig and the corpus**, which live outside this
  checkout. Under the conductor the plan parks in front of Phase 5; that is expected and correct.
- **A fourth "not better" is a real possibility.** The plan's value does not depend on the verdict
  going the other way: three mechanisms settled and a present-day census are worth having whichever
  way the gate reads, and a fourth no-go with all three attributed is a much stronger statement than
  the first one was.

## What this plan does NOT do

- It does not buy reach. Backlog 0109 wants an ADR and an interview and its trigger is the verdict
  this plan produces; writing that ADR now would be deciding against yesterday's evidence.
- It does not lower HLSL arrays or hunt the blank-render list (backlog 0108's own work). Phase 6
  re-measures both so whoever takes them is working from today's numbers.
- It does not touch the disk-texture exclusion in `milkconv/src/shader/emit.rs`, which stays a named
  rejection class.

## Implementation log

**Lane:** `plan-0202-the-three-mechanisms-get-their-gate` in
`C:\Users\Igor Konovalov\WORK\rlx-plan-0202`

| phase | owner | state | commit |
|---|---|---|---|
| 1 — Settle the rate candidate | dev | done | `7568e511` |
| 2 — Repair what Phase 1 convicted | dev | not run — Phase 1 falsified the candidate | committed with this row |
| 3 — The echo nests | dev | not started | |
| 4 — The waveform scale is measured per mode | dev | not started | |
| 5 — The fourth look gate | human | not started | |
| 6 — The corpus census is present-day | human | not started | |

### Notes

**One red was inherited from the branch point and is not this plan's.**
`suite hygiene::every_shipped_preset_has_a_gallery_card` fails at `a92857eb`, the commit this lane
branched from: *"ships with no card: `["cellular_labyrinth", "cellular_wavefront"]`"*. Both presets
landed in that commit without being added to `CARDS` in `scripts/docs-shots.mjs`, and repairing it
means editing that script and re-rendering committed stills under `docs/images/` — neither file is in
any of this plan's phases, and the render is a content-lane act. Recorded here so the state is not
mistaken for something a phase below did. The baseline it establishes, measured before Phase 1's
commit: `cargo nextest run --workspace -P fast --no-fail-fast` → **1731 run, 1730 passed, 1 failed,
317 skipped**, the one failure being that test. Every phase below is checked against that baseline
rather than against green.

**Phase 1 — the rate probe did not run through `shot --render`, and why.** The conductor session's
allowlist denies `cargo run` and denies running a built binary out of `target/`, so the command the
phase names could not be issued. The probe is the same measurement taken one call earlier in the
same path: `shot --render <clip> --fps N` does nothing but drive
`Renderer::capture_stream(name, frames, 1/N, ...)` and convert the frames it hands back into Y4M, so
the probe calls `capture_stream` at each rate directly. It lives in `core/src/render/milk_wash.rs`
beside the wash bisect, which already owns both washed fixtures and the `edge` statistic. Two
differences from a clip-driven render, both of which make the reading *cleaner* rather than weaker:
the `AnalysisFrame` is held constant for the whole run, so the only thing differing between rates is
`dt` and not also the hop-to-frame mapping; and the run is deterministic, so it is a gated instrument
rather than a one-off.

**Phase 1 — the reading.** *Geiss - Fog Tunnel*
(`core/tests/fixtures/milk_wash_fog_tunnel.toml`), 128x128, `AnalysisFrame::default()` held for the
whole run, 8 s of wall clock at each rate, level meaned over the last 2 s. Machine: dev box, Windows
10 Home x86_64. Tree: this lane at `a92857eb` plus the probe this row commits. Command:

```
node tools/conductor/with-lock.mjs suite -- cargo nextest run -p rlx-core --lib \
  -E 'test(the_converted_ground_level_is_read_across_a_frame_rate_ladder)' --no-capture
```

`A field` is the linear `edge` at the feedback field after the last frame; `E display` is the
display-referred `edge` meaned over the tail, with the tail's half-spread beside it.

```text
   fps  frames       A field     E display  spread     rate x    field x  display x
    15     120    0.04549802    0.23894683   1.70%     0.5000     0.6698     0.8106
    30     240    0.06792919    0.29478151   0.87%     1.0000     1.0000     1.0000
    45     360    0.07412413    0.31072101   1.56%     1.5000     1.0912     1.0541
    60     480    0.07673959    0.31705654   1.72%     2.0000     1.1297     1.0756
    90     720    0.07632069    0.31714568   1.90%     3.0000     1.1235     1.0759
   120     960    0.07156828    0.30849582   2.20%     4.0000     1.0536     1.0465
   165    1320    0.06162050    0.28800330   1.67%     5.5000     0.9071     0.9770
```

**Phase 1 — the finding: the rate candidate is falsified, so Phase 2 does not run.** The two rates
the phase names read `0.29478` and `0.28800` at the display and `0.06793` and `0.06162` at the
field — differences of `0.0068` and `0.0063`, both inside the `0.02` mean channel difference this
repository treats as ordinary drift (`docs/testing.md`, the golden compare's tolerance). The
candidate's own prediction is the `rate x` column: an unconverted per-frame deposit into a field
whose equilibrium gain is `1/(1 - d)` would read `5.5x` higher at 165 fps than at 30. It reads
`0.977x`. Across the ladder's 11x span the level is not monotone in the rate at all — it rises to a
peak somewhere between 60 and 90 fps and falls away on both sides, ending below where it started.
That is a bounded hump, not a rate law.

**Phase 1 — what the phase did not settle.** A residual remains: `+13 %` at the field between 30 and
60 fps, and `-19 %` between 60 and 15. It is real (each row's tail spread is under `2.2 %`) and it is
unattributed. Two per-frame mechanisms in this path cannot be `dt`-converted even in principle and
are where to look first — each frame is one bilinear resample of the whole field through the warp
mesh, and each frame applies the 8-bit quantize floor (ADR-0118) once — but neither was tested here
and neither has the candidate's shape.

**Phase 2 did not run.** Its stated condition — *"this phase does not run if Phase 1 falsified the
candidate"* — is met: there is no deposit path to convert, because no deposit path was convicted. No
code changed and no baseline moved for it. The two washed pairs therefore leave Phase 1 with **no
named mechanism**, which is the outcome the plan's own risk section anticipated, and Phase 5's
verdict has to say so.

### Close triggers

- **`presets/` touched:**
- **Plan header `Closes:`** none — see `**Takes:**`; backlog 0108 and 0109 both stay live.
- **What shipped:**
- **Operator docs touched:**
- **Backlog probes (`node scripts/check-backlog-claims.mjs`):**
- **Full suite:**
- **Outstanding `human` phases:**

## Followups (after this lands)

- The reach decision (backlog 0109) — an interview and an ADR, triggered by Phase 5's verdict.
- ADR-0199 gains an `Outcome` at this plan's close recording Phase 4's per-mode measurement.
