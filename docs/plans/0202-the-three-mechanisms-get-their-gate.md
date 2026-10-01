# 0202 — The three mechanisms get their gate

> **Status:** in-progress. **Split 2026-10-01:** Phases 4-7 moved to
> [Plan 0246](0246-the-rig-session-measures-the-wave-modes-and-judges-the-fourth-gate.md), because
> each needs the Windows rig or the corpus and the lane should not wait on them. This plan closes at
> Phase 3.
> **Created:** 2026-09-19
> **Approved:** 2026-09-19 (user)
> **Owner skill(s):** dev, human
> **Related ADRs:** [0113](../adrs/0113-milkdrop-presets-are-translated-ahead-of-time-onto-a-warp-mesh-idiom.md)
> (its third `Outcome` is this plan's brief),
> [0199](../adrs/0199-a-converted-waveform-draws-the-sources-figure-at-the-hosts-scale.md)
> (whose inference Plan 0246 Phase 2 now tests), [0019](../adrs/0019-eased-parameters.md) (the injected `dt` a
> per-second rate is converted against)
> **Takes:** nothing since the split. Design-backlog 0108 and 0109 went to Plan 0246 with the
> phases that serve them, and neither entry is closed by this plan.

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

### Phase 3 — The echo's orientation truncates like the reference
- **Owner skill:** dev
- **Amended 2026-09-26 (owner), after the phase stopped at its own stop condition.** Its first
  premise, that the reference's echo nests the previous frame, is **falsified by `d4c843a`**. The
  reading is in the implementation log's Notes. *Songflower* has no comp shader, so the reference
  takes `ShowToUser_NoShaders`, whose echo composites the **current** frame with one zoomed and
  flipped copy and never feeds back. At the preset's `fVideoEchoAlpha = 1.0` the reference cannot
  draw a nested weave either, and our present pass already does the same `mix` from the same
  per-frame outputs. What the reading did find is **one divergence**, and this phase repairs that and
  nothing else. The weave stays **unattributed**. Plan 0246's look gate says so for its pair, and its three
  remaining candidates are a followup below, not work here.
- **What:** `echo_orientation` in `core/src/render/scenes/warp_mesh/mod.rs` **rounds** the bound
  value, and the reference takes `(int)v % 4`, which **truncates**. *Songflower*'s
  `echo_orient = 1 + 16*pfdy_r` sweeps about 0.76-1.24, so the reference flips x only while the value
  is at or above 1, and this engine flips it the whole time. Make the quantizer truncate toward zero
  as C's `(int)` cast does. For a negative value, do what the reference does with `(int)v % 4`'s
  negative remainder: read it in `d4c843a`'s use of the orientation, not from C semantics alone, and
  state the answer in the doc comment. Keep the function total on non-finite input. Correct the doc
  comment at the neighbouring quantizer that cites `echo_orientation`'s rounding as its reason
  (`mod.rs`, the comment beginning *"Rounded here for `echo_orientation`'s reason"*), so it does not
  claim a rule this phase removes.
- **Files touched:** `core/src/render/scenes/warp_mesh/mod.rs`,
  `core/src/render/scenes/warp_mesh/tests.rs`, any golden baseline the change moves.
- **Done when:** `the_echo_orientation_quantizes_to_four_states` (renamed to match if it no longer
  describes the rule) asserts truncation at the boundary the reference draws: `0.99` reads 0, `1.0`
  and `1.99` read 1, and the *Songflower* sweep's two ends (`0.76` and `1.24`) read 0 and 1. It also
  asserts the negative case as the reference resolves it, with the `d4c843a` file and line it was read
  from cited in the test's comment. Every golden stays green, or each baseline the change moved is
  re-blessed with the move named in the log. A hand-written preset binding a whole-number
  `echo_orient` (`warp_cauldron`'s `"1"`) cannot move.

## Risks & open questions

- **Phase 1 may falsify its own candidate**, and then two washed pairs have no named mechanism and
  Plan 0246's look gate will say so. That is the plan working, and it is why Phase 2 carries a
  do-not-run condition rather than an assumption.
- **Phase 3 is the least-scoped phase here.** The reference's echo is a composite stage this engine
  approximates; if reading `d4c843a` shows it is a larger divergence than a binding that does not
  reach the composite, the phase should stop and report rather than grow.
- **The rig and corpus phases left with the split.** Phases 4-7 need the Windows rig or the corpus,
  which live outside this checkout, and are Plan 0246's now.
- **A fourth "not better" is a real possibility.** The plan's value does not depend on the verdict
  going the other way: three mechanisms settled and a present-day census are worth having whichever
  way the gate reads, and a fourth no-go with all three attributed is a much stronger statement than
  the first one was.

## What this plan does NOT do

- It does not buy reach. Backlog 0109 wants an ADR and an interview and its trigger is the verdict
  this plan produces; writing that ADR now would be deciding against yesterday's evidence.
- It does not lower HLSL arrays or hunt the blank-render list (backlog 0108's own work). Plan 0246's census
  re-measures both so whoever takes them is working from today's numbers.
- It does not touch the disk-texture exclusion in `milkconv/src/shader/emit.rs`, which stays a named
  rejection class.

## Implementation log

**Lane:** branch `plan-0202-the-three-mechanisms-get-their-gate`, worktree
`/home/igor/Work/rlx-plan-0202` (conductor run)

| phase | owner | state | commit |
|---|---|---|---|
| 1 — Settle the rate candidate | dev | done | 7027a97c |
| 2 — Repair what Phase 1 convicted | dev | done - not run: Phase 1 falsified the candidate | 09be6b65 |
| 3 — The echo's orientation truncates like the reference | dev | done (first run parked at ece8b14c, phase amended) | committed with this row |
| 4 — The eight modes are captured on the rig | human | not run - moved to Plan 0246 on 2026-10-01 | |
| 5 — The waveform scale is measured per mode | dev | not run - moved to Plan 0246 on 2026-10-01 | |
| 6 — The fourth look gate | human | not run - moved to Plan 0246 on 2026-10-01 | |
| 7 — The corpus census is present-day | human | not run - moved to Plan 0246 on 2026-10-01 | |

### Notes

- **Phase 1 readings.** Preset `core/tests/fixtures/milk_wash_fog_tunnel.toml` (*Geiss - Fog
  Tunnel*). Command: `shot --preset-file core/tests/fixtures/milk_wash_fog_tunnel.toml --render
  <silent 48 kHz mono WAV, 30 s> --fps <30|60|165> --size 128x96` (release build of the `shot`
  example, tier floor), its Y4M stdout read by a scratch Node script that spawned the binary,
  because the conductor allowlist refuses a shell redirect. Statistic: full-range luma `Y/255` averaged
  over the whole frame, sampled on one time-matched 10 Hz grid over `t` in 5-30 s at every rate.
  Machine: the Arch Linux dev box, hardware adapter, cargo 1.97.1. Tree: `7f8c3607`.

  | subject | 30 fps | 60 fps | 165 fps |
  |---|---|---|---|
  | Fog Tunnel as converted | 0.27006 | 0.28744 | 0.24706 |
  | the same, every motion term zeroed | 0.02045 | 0.02063 | 0.02033 |

  "Every motion term zeroed" is a scratch copy of the fixture with `zoom = 1`, `rot`, `dy` and
  `warp` at 0, and the four sine amplitudes on `rot`, `cx`, `cy` and `warp` at 0. That leaves
  only the deposit (the waveform, through `Exposure`) and the decay.
- **Phase 1 finding: the rate candidate is falsified.** The deposit and decay equilibrium agrees
  across 30, 60 and 165 fps to within 0.0003, which is far inside the 0.02 drift floor, so no
  deposit path bypasses `Exposure` on this preset. **This departs from the done-when as written.**
  The whole preset read at 30 and 165 fps differs by 0.023, just over the floor. That difference is
  non-monotone in rate: 60 fps is the brightest and 165 fps the darkest. It vanishes when the
  motion terms are zeroed. It is therefore not a deposit path, and it runs the wrong way for a wash
  at the rig's 165 fps. The candidate was judged on the deposit-only reading. The residual
  rate-dependence in the motion path (the warp, zoom and rot advection) is a followup and was not
  pursued.
- **Phase 2 did not run**, per its own condition.
- **Phase 3 stopped at its stop condition**, the "least-scoped phase" risk above. Reading `d4c843a`
  (local clone `~/Work/milkdrop2-src`) contradicts the phase's premise; no code was written.
  - *Songflower (Moss Posy)*
    (`milkdrop-corpus/milkdrop-original/Milkdrop-Original/Aderrasi - Songflower (Moss Posy).milk`)
    has no `PSVERSION` lines, so it has no comp shader. `CPlugin::RenderFrame`
    (`milkdropfs.cpp` l.1160-1166) therefore takes `ShowToUser_NoShaders`.
  - That path (l.4147-4233) reads the per-frame `echo_zoom`, `echo_alpha` and `echo_orient`. It
    draws `m_lpVS[1]`, the current frame's warp plus its waves, at `1 - alpha`. It then adds a copy
    zoomed about the centre by `1/echo_zoom` and flipped by orientation, at `alpha`. The result goes
    to the back buffer only.
  - The swap at l.1216-1219 hands the next frame `m_lpVS[1]` as it was before the composite. **The
    echo never feeds back and does not composite the previous frame.**
  - At *Songflower*'s header `fVideoEchoAlpha = 1.0`, the displayed frame is the one zoomed and
    flipped copy alone. The reference's own echo cannot draw a nested weave for this preset.
  - This engine's present pass (`shaders.rs`, `PRESENT_SHADER`) does the same `mix` about the same
    centre, from the same per-frame outputs (`mod.rs` l.1087-1089 into `encode.rs` l.258-266). So
    the binding does reach the composite.
  - One divergence was found, and it cannot produce nesting. The reference takes the orientation as
    `(int)v % 4`, a truncation; `echo_orientation` rounds. *Songflower*'s `echo_orient = 1 +
    16*pfdy_r` sweeps about 0.76-1.24, so the reference flips x only while it is at or above 1, and
    this engine always flips.
  - For comparison, the shader path (`GenCompPShaderText`, `plugin.cpp` l.9594-9637) bakes the
    header's echo values into the comp shader as literals at load. There a per-frame echo binding
    is not read at all.
  - Where the weave comes from is unattributed. It is not the echo, so it is outside this phase.
    The candidates left are the field's own: `fDecay = 1`, `bTexWrap = 1`, and a per-pixel `zoom`
    that falls below 1.
- **Phase 3 (amended), the negative case.** Read at `d4c843a` `vis_milk2/milkdropfs.cpp` l.4149
  (`(int) v % 4`) and l.4195-4198 (x flips on `n % 2`, y on `n >= 2`): -1 and -3 flip x only,
  -2 flips nothing, and no negative flips y. The earlier rule read -1 as 3 (both flips).
- **Phase 3 goldens: the GPU suites the fast profile skips were not run.** The per-phase
  `-P fast` run was green (1775 passed, 86 skipped). A filtered run of the warp-mesh and
  `milk_wash` tests waited on the suite lock, held by another lane's full workspace run, until the
  session timeout, and was stopped before it started. No preset or fixture binds an `echo_orient`
  whose reading changes: every whole non-negative value maps as before, the three fixtures carry
  0 or 1 in their headers with no per-frame assignment, and `warp_cauldron` binds `"1"`. No
  baseline was re-blessed.
- **Stale prose outside the phase's files, not edited:** `core/tests/suite/preset.rs` l.2444
  (a comment saying `echo_orientation` "rounds") and `presets/README.md` l.2277 (the hand-written
  echo table: "Rounded to the nearest of the four").

### Close triggers

- **`presets/` touched:**
- **Plan header `Closes:`** none — see `**Takes:**`; backlog 0108 and 0109 both stay live.
- **What shipped:**
- **Operator docs touched:**
- **Backlog probes (`node scripts/check-backlog-claims.mjs`):**
- **Full suite:**
- **Outstanding `human` phases:**

## Followups (after this lands)

- **Where *Songflower (Moss Posy)*'s nested weave comes from is unattributed.** Phase 3's reading
  took the echo off the list. The three candidates left are the field's own: `fDecay = 1`,
  `bTexWrap = 1`, and a per-pixel `zoom` that falls below 1. Take them in a probe with a stop
  condition, the way Phase 1 took the rate candidate, if Plan 0246's look gate still reads that pair as wrong.

- The reach decision (backlog 0109) — an interview and an ADR, triggered by Plan 0246's look gate.
- ADR-0199's `Outcome` for the per-mode measurement is owed at Plan 0246's close, not this one.
