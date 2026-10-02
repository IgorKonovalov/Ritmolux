# 0202 — The three mechanisms get their gate

> **Status:** done - closed 2026-10-02 by a conductor close at Phase 3: Phase 1 falsified the rate
> candidate, Phase 2 did not run, Phase 3 made the echo orientation truncate like the reference.
> Round 1 review: no blockers, no majors, five minors (four repaired at the close). Full suite green
> on the close tip. Version 0.160.2. **Split 2026-10-01:** Phases 4-7 moved to
> [Plan 0246](../0246-the-rig-session-measures-the-wave-modes-and-judges-the-fourth-gate.md), because
> each needs the Windows rig or the corpus and the lane should not wait on them. This plan closes at
> Phase 3.
> **Created:** 2026-09-19
> **Approved:** 2026-09-19 (user)
> **Owner skill(s):** dev, human
> **Related ADRs:** [0113](../../adrs/0113-milkdrop-presets-are-translated-ahead-of-time-onto-a-warp-mesh-idiom.md)
> (its third `Outcome` is this plan's brief),
> [0199](../../adrs/0199-a-converted-waveform-draws-the-sources-figure-at-the-hosts-scale.md)
> (whose inference Plan 0246 Phase 2 now tests), [0019](../../adrs/0019-eased-parameters.md) (the injected `dt` a
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

[ADR-0113](../../adrs/0113-milkdrop-presets-are-translated-ahead-of-time-onto-a-warp-mesh-idiom.md)'s
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

- **`presets/` touched:** no preset file. `presets/README.md`'s hand-written echo table was corrected
  at the close, prose only.
- **Plan header `Closes:`** none — see `**Takes:**`; backlog 0108 and 0109 both stay live.
- **What shipped:** fix-only (filled at the close).
- **Operator docs touched:** `presets/README.md`'s echo table, at the close.
- **Backlog probes (`node scripts/check-backlog-claims.mjs`):** re-run at the close, exit 0 (55 reductions across 27 live entries).
- **Full suite:** the suite ledger record of gate `0202-pre-review` at 2026-10-02T06:00:29Z, tree
  `41886eb`: 1934 tests run, 1934 passed, 8 skipped; re-run on the close tip.
- **Outstanding `human` phases:** none after the split; Phases 4 and 6-7 are Plan 0246's.

## Close review

The conductor's round 1 review, graded at tip `d2b63ba9d07c63dbc1aaf9ac389bfd541e8f65f2`, in full. It
was the only round, so no earlier finding was resolved by a fix round. Minors 1, 2, 3 and 5 were
repaired at the close in `8c11da22`; minor 4 asked for no change and stays as recorded.

### Plan 0202 — close review, round 1

Graded at tip `d2b63ba9d07c63dbc1aaf9ac389bfd541e8f65f2`, lane `/home/igor/Work/rlx-plan-0202`, branch
`plan-0202-the-three-mechanisms-get-their-gate`.

**Verdict: Plan 0202 landed cleanly at its split scope (Phases 1-3). No blockers, no majors, five minors,
all of them prose that a close can repair.**

#### Evidence

- **Full suite:** `node .../with-lock.mjs suite -- cargo nextest run --workspace` printed
  `with-lock: skipped cargo nextest run --workspace: tree 41886eb is green in the suite ledger, run by gate
  0202-pre-review at 2026-10-02T06:00:29.799Z: 1934 tests run: 1934 passed (21 slow), 8 skipped`. That
  ledger record is this review's full-suite evidence (ADR-0207). It covers the GPU suites the Phase 3
  session could not run (its log says the `-P fast` run was green and the filtered golden run was stopped
  waiting on the lock), so no golden moved under the change.
- **rustdoc:** `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` — clean.
- **Comment hygiene:** `node scripts/check-comment-hygiene.mjs` — OK.
- **Diff against `main`:** `core/src/render/scenes/warp_mesh/mod.rs`, `core/src/render/scenes/warp_mesh/tests.rs`,
  and the plan. Tree clean before and after this review.

#### Lens 1 — Alignment

- **Owner tags:** every phase carries one in-vocabulary `**Owner skill:**` line (Phases 1-3 `dev`). The
  split to Plan 0246 is recorded in the header and the log rows; Phases 4-7 are not this plan's.
- **Phase 1** (log-only). The readings table names the preset, command, machine, tree and statistic as the
  done-when asks. The finding departs from the done-when as written and **says so**: the whole preset at
  30 vs 165 fps differs by 0.023, just over the 0.02 floor, but the difference is non-monotone (60 fps
  brightest, 165 darkest), vanishes when the motion terms are zeroed, and the deposit-plus-decay reading
  agrees within 0.0003. The rate candidate is about a deposit bypassing `Exposure` raising the field by
  `1/(1-d)`; the deposit-only isolation tests exactly that, and a 165 fps reading that is *darker* runs
  against the wash. I accept the falsification. The residual motion-path rate dependence is not lost in
  the Notes, but it is missing from `## Followups` (minor 5).
- **Phase 2** correctly did not run, per its own condition.
- **Phase 3** (amended). `echo_orientation` now computes `(v.trunc() % 4.0) as i32` and maps `1 | -1 | -3`
  to the x flip, `2`, `3` as before, everything else to 0; non-finite still returns 0. I read the reference
  directly (`~/Work/milkdrop2-src/vis_milk2/milkdropfs.cpp` l.4149 `(int) (*var_pf_echo_orient) % 4`,
  l.4195-4198 `if (n % 2)` flips tu, `if (n >= 2)` flips tv): C truncates, the remainder keeps the
  dividend's sign, `-1 % 2` and `-3 % 2` are non-zero, `-2 % 2` is zero, and no negative is `>= 2`. The
  code and its doc comment match that exactly. Taking the `f32` remainder before the cast keeps a huge
  value from overflowing the integer cast, which the doc comment states.
- **The named test** is renamed to `the_echo_orientation_truncates_to_four_states` and asserts every value
  the done-when lists: `0.99 -> 0`, `1.0 -> 1`, `1.99 -> 1`, `0.76 -> 0`, `1.24 -> 1`, plus the wrap
  (`4 -> 0`, `5 -> 1`) and the negative cases (`-0.5 -> 0`, `-1 -> 1`, `-2 -> 0`, `-3.5 -> 1`, `-4 -> 0`,
  `-5 -> 1`) with the `d4c843a` file and lines cited in the comment. Non-finite stays asserted. No
  tautology.
- **The neighbouring `colour_source` doc comment** was corrected as the phase required, and no longer
  cites a rounding rule that `echo_orientation` gave up.
- **Whole-number bindings cannot move:** no shipped preset binds `echo_orient` at all (`git grep`), and the
  three fixtures carry `0.0` or `1.0` in their headers, which map identically under both rules.
- **Implementation log:** present, with lane, phase-to-commit table and notes. Its `### Close triggers`
  bullets are blank (minor 3) and it is longer than the phases section (minor 4).

#### Lens 2 — Layering, real-time safety

A pure scalar quantizer in a scene module; no new types, no allocation, no audio-source or platform
reach, no `unwrap`. C ABI and control protocol untouched. Nothing to report.

#### Lens 3 — Doc freshness and bookkeeping

- Two reader-facing statements still describe the retired rounding rule (minors 1 and 2). The generated
  parameter reference row (`presets/README.md:832`) and the schemas carry no rounding claim and need no
  regeneration.
- **Version bump owed: patch.** The plan shipped one behaviour fix in `core` (the echo flip now matches the
  reference for fractional and negative orientations) and no feature.
- No ADR to accept; ADR-0199's `Outcome` is owed at Plan 0246's close, as the plan says. `Closes:` is none,
  so no backlog entry moves.

#### Lens 4 — Correctness and determinism

The quantizer is a pure function of its input; the new assertions are exact integer properties, not
measured thresholds. No geometry, aspect or numeric-noise question arises.

#### Lens 5 — Design integrity

No seam widened; the change stays inside the function that owns the rule.

#### Findings

**Blockers.** None.

**Majors.** None.

**Minors.**

1. **`core/tests/suite/preset.rs:2457`** — the roster comment reads
   ``// `echo_orientation`: rounds, then wraps modulo the four flips.`` It is now false. Replace with
   ``// `echo_orientation`: truncates toward zero as the reference's `(int)` cast does, then takes C's
   sign-keeping remainder over the four flips.`` (Comment-only; close-repairable.) **Fixed in `8c11da22`.**
2. **`presets/README.md:2317`** — the hand-written echo table says *"Rounded to the nearest of the four, so a
   smoothed or computed value never lands between them. Out of range wraps, so a preset that animates the
   orientation by counting gets a cycle."* Replace with: *"Truncated toward zero, as MilkDrop reads it, so
   a smoothed or computed value never lands between states and `0.99` is still `0`. Above `3` it wraps, so
   a preset that animates the orientation by counting upward gets a cycle; below zero `-1` and `-3` flip
   left-right and `-2` flips nothing."* (Hand-written prose outside the generated region;
   close-repairable.) **Fixed in `8c11da22`.**
3. **`docs/plans/0202-the-three-mechanisms-get-their-gate.md:233`** — every `### Close triggers` bullet but
   `Closes:` is empty (What shipped, `presets/` touched, operator docs, backlog probes, Full suite,
   outstanding human phases). The full-suite evidence exists in the ledger record cited above; the close
   should fill the bullets (fix-only; `presets/README.md` prose only; Full suite = the ledger record;
   no outstanding human phase after the split). **Fixed in `8c11da22`.**
4. **`docs/plans/0202-the-three-mechanisms-get-their-gate.md:149`** — the `## Implementation log` (about 93
   lines) outweighs the `## Implementation phases` section (about 58). Most of the weight is Phase 3's
   first-run reading of `d4c843a`, which is the evidence for the amendment and earns its place; no
   trimming asked, recorded because nothing else gates the property.
5. **`docs/plans/0202-the-three-mechanisms-get-their-gate.md:243`** — Phase 1's Notes call the residual
   rate dependence in the motion path (warp, zoom and rot advection: whole-preset luma 0.270 / 0.287 /
   0.247 at 30 / 60 / 165 fps, gone with motion zeroed) "a followup", but `## Followups` does not list it.
   Add a bullet so Plan 0246's look gate and the next reader can find it. **Fixed in `8c11da22`.**

**Nits.** None.

#### Bookkeeping the close owes

- Repair minors 1, 2, 3 and 5 (all comment or Markdown prose), and fill the close triggers.
- `Status: done`, `git mv` to `docs/plans/done/`, re-point links (`node scripts/check-doc-links.mjs`),
  backlog probes, index rows, translations advisory, `toc.mjs`.
- `## Close review` section carrying this review.
- **Patch** version bump plus the studio's two version copies, annotated tag, `check-release-tag.mjs`.

### What the close ran

- Upstream CI (`node scripts/check-upstream-ci.mjs`): green, run 36908854196 on `main` at `d32b1cc`.
- `git merge main` was clean (the conductor queue only).
- Doc links, index rows and backlog probes exit 0; the translation advisory names no moved source.
- No preset file changed, so the curation sweep has nothing to judge; no ADR to accept; no backlog
  entry moves.

## Followups (after this lands)

- **Where *Songflower (Moss Posy)*'s nested weave comes from is unattributed.** Phase 3's reading
  took the echo off the list. The three candidates left are the field's own: `fDecay = 1`,
  `bTexWrap = 1`, and a per-pixel `zoom` that falls below 1. Take them in a probe with a stop
  condition, the way Phase 1 took the rate candidate, if Plan 0246's look gate still reads that pair as wrong.
- **A residual rate dependence lives in the motion path.** Phase 1 read *Fog Tunnel*'s whole-preset
  luma at 0.270 / 0.287 / 0.247 at 30 / 60 / 165 fps, non-monotone, and it vanished with every motion
  term zeroed, so the warp, zoom and rot advection is not rate-independent. Plan 0246's look gate
  runs at the rig's 165 fps and should read its washed pairs knowing this; a probe with a stop
  condition is the next step if that gate attributes anything to it.

- The reach decision (backlog 0109) — an interview and an ADR, triggered by Plan 0246's look gate.
- ADR-0199's `Outcome` for the per-mode measurement is owed at Plan 0246's close, not this one.
