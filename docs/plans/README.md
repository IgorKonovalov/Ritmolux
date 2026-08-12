# Plans index

The one-minute "what's in flight" view. Read this first each session instead of
re-deriving state from `git log`. Completed plans move to `done/`; their full
close write-ups move to [README-archive.md](README-archive.md).

**Next free number: 0083** (ADRs are a separate sequence — next free there is **0097**.)

## Active roster

Only plans still in `docs/plans/`. A closed plan leaves this table entirely —
`Recently closed` below and `done/` both already record it. Each row carries at
most two sentences of **live constraint**: what a reader needs to decide whether
to pick this plan up. Anything longer belongs in the plan file, which is where
someone who picked it up is reading.

| Plan | Title | Status | Owner | Live constraint |
|------|-------|--------|-------|-----------------|
| [0079](0079-the-attractor-learns-new-figures.md) | The attractor learns new figures: the tuple roster with per-tuple framing, and measured morph paths | **approved 2026-08-11** | dev, human | The largest of the three ([ADR-0093](../adrs/0093-attractor-tuples-are-content-with-per-tuple-framing.md)): per-family curated tuple tables carrying their own projection + seed box (jitter derived per-entry so `reseed` survives — the Plan 0062 coupling), a CPU-quantized `tuple` param, the rho ≈ 100 Lorenz as the walking skeleton, user-curated roster from contact sheets, then morph-path filmstrip sweeps where **zero survivors is a legitimate recorded outcome** (user accepted the research risk by interview). Closes backlog 0055. **Queued after [0075]'s cohort 6, last of the three** ([0076] landed 2026-08-11) — two `human` curation gates inside. |

## Recommended execution sequence

**Rewritten 2026-08-09, at [0046]'s close, because the shape of this roster changed rather than
its order.** The previous sequence was written when [0075] had four open engine prerequisites, and
it opened by naming [0046] "the critical path". **All four have now landed** — [0071], [0064]
and [0067] within a week of each other, and [0046] at this close. [0068] landed the same day and
changes nothing downstream by construction (it shipped a diagnosis and no fix, on purpose).

**So there is no critical path any more, and that is the headline.** What is left is three plans
that no longer gate each other on capability — only on *file contention* and on one deliberate
preference. Two consequences worth stating plainly, because a reader skimming rows would miss both:

- **[0075]'s Phases 4+ are unblocked.** The renaissance was waiting on this and nothing else. Its
  Phases 1-3 were already ungated; the whole plan is now takeable.
- **The [0053]/[0046] concurrency collision is discharged**, not deferred. [0046] landed first,
  [0053] ran second and has now **closed** (2026-08-09) — so the sequence below is two plans, and
  the bind-group-layout guard [0076] was told to wait for is in place.

**No plan here closes in one session** — every remaining one carries a `human` phase, so plan on
bringing the user in rather than picking on that criterion.

| # | Plan | Why here |
|---|------|----------|
| 1 | [0079](0079-the-attractor-learns-new-figures.md) | The last of the three promoted handoff plans, and now the **only** plan left in the roster. Largest, two `human` curation gates inside. |

**Updated 2026-08-12, at [0081](done/0081-the-sky-gets-a-galaxy.md)'s close** — it held slot 1 and
landed the same day it was written, so the sequence is **one plan** and there is no ordering question
left. **The engine side of the sky family is finished**: ground ([0080]), dither ([0082]) and band
([0081]) all shipped inside two days, which means the three standing content items below are now
authored against a surface that will not move under them. What remains of 0081 is its `human` Phase 6
— judge the galaxy against the reference photograph — which gates no plan and is listed under
Standing.

**Updated 2026-08-12, when [0082](done/0082-the-gradient-stops-banding.md) closed** — it held slot 1
and shipped the same day it was written — from
settling [0080](done/0080-the-sky-gets-a-horizon.md) Phase 7 rather than from a new want. That phase
asked whether the ramp bands; it does, measured, and the user's verdict is *"reads as light, but the
banding is visible"* — so the look passes and the quantization is the defect. **0080's own
arithmetic pointed the wrong way** ("two pixels per 8-bit level is the classic Mach-band
configuration" — two px/level is the *safe*, dense case), which is recorded in
[ADR-0096](../adrs/0096-the-display-write-dithers.md) because that sentence would otherwise send the
next reader hunting in the bright end. It went **first** by the user's call, and has landed: the galaxy's
band is a second wide smooth gradient, and building it onto an undithered chain would have confounded
its own `human` verdict with a defect already known about. [0081](done/0081-the-sky-gets-a-galaxy.md) is
now unblocked and born onto a chain that dithers.

**Updated 2026-08-12, when [0081](done/0081-the-sky-gets-a-galaxy.md) was written**, hours after [0080]
closed and from judging that plan's own output in the running app. It goes first by the user's
explicit call at the interview — offered "ship the sky now and plan the band later" versus "build
the band first, then author the world once", they chose the second. So this is deliberately *not*
the smaller-first ordering: the world waits on the capability rather than being authored twice.
The two plans do not contend for files — 0081 is the backdrop pre-pass, 0079 is the attractor's
tuple tables — which is the same non-collision [0080] and [0079] had.

**One consequence worth stating, because it is easy to miss:** the content lane now has **three**
standing items on one family of looks — [0077]'s Phase 5 (Perseids' quiet sky), [0080]'s Phase 7
(the dusk ground), and this plan's world. They are one pass, not three, and walking the family once
is the point.

**Updated 2026-08-12, at [0080](done/0080-the-sky-gets-a-horizon.md)'s close.** It landed — all
six `dev` phases, no blockers and no majors at review — so the sequence is **one plan**, and for
the first time since this section was rewritten there is no ordering question left to answer. Its
Phase 7 (judge the dusk ground against the reference photo) is `human` and standing, not queued;
see Standing below. It gates nothing, and by construction it does not contend with [0079] for files.

**Updated 2026-08-12, at [0078](done/0078-the-ink-learns-to-bite.md)'s close.** The second of the
three landed — both `dev` phases, no blockers at review — so the sequence is one plan. Its Phase 3
(the ink worlds re-judge, content lane) is standing, not queued; see Standing below. It does not
gate [0079].

**Updated 2026-08-12, at [0077](done/0077-the-quiet-sky.md)'s close.** The first of the three
promoted handoff plans landed — all four `dev` phases, no blockers at review — so the sparse
idiom is gateable *before* any more sparse content is authored, which was the whole point of
its position. Its Phase 5 (the quiet sky itself, content lane) is standing, not queued — see
Standing below; it does not gate 0078 or 0079.

**Updated 2026-08-11, at [0075]'s close.** The renaissance is done — the sequence's anchor
("[0075] last by design") is discharged, and what remains is exactly the three promoted
handoff plans in the order the [0076]-close note below already set: **0077, then 0078, then
0079**.

**Updated 2026-08-11, at [0076]'s close.** The 2026-08-11 handoff confirmation ("[0076] remains
the next execution step") is discharged — [0076] landed the same day, all five phases, no
blockers at review. Cohort 6 authors against `presets/README.md`'s new `[layer]` section.
**The promoted handoff items have their plans** (written the same day, user-decided by
interview): [0077](done/0077-the-quiet-sky.md) (sparse idiom + swarm individuation, closes
0009/0068/0085/0088), [0078](done/0078-the-ink-learns-to-bite.md) (ink contrast, closes 0084),
[0079](0079-the-attractor-learns-new-figures.md) (tuple roster + measured morph paths, closes
0055). Execution order after [0075]'s remainder: **0077, then 0078 (small — any free
session), then 0079** (largest, two `human` curation gates).

### The baseline-drift control any pixel-touching plan inherits

Kept here after [0053]'s close because it is not that plan's property — it applies to every plan
that could move a render. **Do not `git diff` the committed baselines.** On this box **eight
baselines drift from their committed bytes under `LMV_BLESS`** (`composite_bloom`, `composite_kaleido`,
`composite_overlap`, `composite_trails`, `line_joint_zigzag`, `lsystem`, `parametric_curve`,
`star_pattern`), so a naive diff convicts eight files the change never touched. Bless every binary
in scope (`--test golden --test composite --test line_joints --test attractor_trails`) and compare
**bless-to-bless**, then `git checkout -- core/tests/golden`.

[0053]'s close used a tighter form of this than the clean-`main` control it was handed, and it is
the one to reuse: bless twice **on the same branch**, differing only by reverting the change under
test. Bless output is deterministic run-to-run (it is bless-vs-*committed* that drifts), so the two
hash sets are directly comparable and everything except the change is held fixed. All of them came
back identical, which is how "the two WARP fixes moved zero pixels" was established rather than argued.

**The suite is 28 baselines, not 20** (repaired 2026-08-12 at [0080](done/0080-the-sky-gets-a-horizon.md)'s
close, which found the paragraph and the plan it had been copied into both saying "20" against a
directory holding 26; 27 after [0080] and 28 after [0081]). The eight drifters are named above by label, so the numerator survives the
correction; only the denominator was wrong. Re-derive the count rather than copying a number
forward — that is what went stale here.

The other recorded collision, "**[0046] should precede [0075] Phase 3**", is **discharged**: that
phase documents [backlog 0063](../design-backlog.md)'s `spin`x`fade` smear ceiling in *frames*, and
[0046] has now made the decay time-based, so the paragraph gets written once against final
semantics. The honest phrasing there is seconds rather than frames — and note the shipped
exponent is `fade^(dt / FALLBACK_DT)`, exactly `1.0` at the capture step, so every number measured
at 60 Hz or at capture `dt` stays correct as written.

[0045]: done/0045-linear-light-and-bloom.md
[0046]: done/0046-transformed-feedback.md
[0052]: done/0052-the-emitter-objects-that-spawn-fall-and-die.md
[0053]: done/0053-the-suite-stops-blessing-what-warp-gets-wrong.md
[0055]: done/0055-the-fold-edge-becomes-a-choice.md
[0062]: done/0062-the-chaos-game-grows-a-fern.md
[0065]: done/0065-the-mandala-interior.md
[0066]: done/0066-the-level-lever.md
[0069]: done/0069-the-instrument-that-sees-a-figure-leave-the-frame.md
[0061]: done/0061-the-build-stops-paying-for-what-it-is-not-building.md
[0064]: done/0064-the-symmetry-stage-and-the-banded-palette.md
[0067]: done/0067-the-curation-route.md
[0068]: done/0068-why-the-downbeat-rarely-locks.md
[0071]: done/0071-light-that-adds-without-covering.md
[0072]: done/0072-the-backdrop-joins-the-palette.md
[0075]: done/0075-the-content-renaissance.md
[0076]: done/0076-the-second-layer.md
[0077]: done/0077-the-quiet-sky.md
[0078]: done/0078-the-ink-learns-to-bite.md
[0079]: 0079-the-attractor-learns-new-figures.md
[0080]: done/0080-the-sky-gets-a-horizon.md
[0081]: done/0081-the-sky-gets-a-galaxy.md
[0082]: done/0082-the-gradient-stops-banding.md
[backlog 0038]: ../design-backlog.md
[backlog 0058]: ../design-backlog.md

### The six plans added 2026-08-04, and why they exist

They came from a **backlog sweep**, not from six separate requests: the user asked for the backlog
to be checked, the stale entries retired, and plans made from whatever was left that nothing else
already covered. What that produced is worth stating, because the shape of it is not obvious from
the rows above.

- **Seven entries were retired**, none of which carried a marker saying it was dead — 0015 (the
  half-linear band axis, landed with Plan 0048's second analysis window), 0020's content half (Plan
  0048 Phase 7's 368-gain retune), 0030 (landed in the content lane's own `craft.md`), 0036 (retired
  unfired), 0049 (carried into Plan 0055's judged A/B), 0051 (both `star_*` presets now ship triangle
  waves) and 0007's interior half (specified at last by [0065]). The backlog had been accumulating
  answered questions faster than it was closing them.
- **Five entries stay parked deliberately** — 0009 (informational), 0021 (the slew release, awaiting
  an author who wants it), 0032 (96 kHz, awaiting a report), 0038 and 0058 (content-lane retunes,
  routed not planned), and 0055 (attractor variety, which [0062] partly covers and whose own
  re-check condition just landed). **0058 has since closed** by content on 2026-08-04 (`859ec66`),
  so the parked content-lane retune is 0038 alone — worth knowing because two later documents kept
  pairing them.
- **Two of the six plans ship no capability at all.** [0068] ships a diagnosis and explicitly no
  fix; [0069] replaces a measure that was proved not to work. Both are here because the alternative
  — tuning a threshold, or calibrating a statistic that cannot separate the cases — is the move each
  one's ADR exists to refuse.
- **[0066] and [0071] each turned out to move zero pixels**, in both cases by *arithmetic* rather
  than by a chosen default: no golden fixture binds `exposure`, and `occlude` defaults to literal
  `1.0`. Neither was designed for that outcome; both plans check it as a phase failure rather than
  claiming it. **[0066] has since closed and the claim held exactly** — zero baselines modified
  across the whole plan, and it left behind the fixture that makes the premise false going forward
  (`composite_bloom_exposed.toml` is now the suite's only `exposure`-binding fixture), so the same
  reasoning cannot be reused unchecked. **[0071] has now closed and its claim held too, but it was
  the check rather than the arithmetic that established it**: the default at literal `1.0` was the
  argument, and what was run was an `LMV_BLESS` on the change re-encoding all 19 baselines
  hash-identical to an `LMV_BLESS` on clean `main`. That is the form to reuse — a bless-against-a-
  control, not a diff against the committed files, because three baselines on this machine
  (`lsystem`, `parametric_curve`, `star_pattern`) drift from their committed bytes under `LMV_BLESS`
  on clean `main` too, and a naive diff would have convicted the change of moving them.


## Standing (not a plan)

- **Plan [0061] Phase 9 — the one verification still outstanding** (2026-08-08). The plan is
  `done` and every `dev` phase landed. **Phase 8 ran and passed the same day**: the foobar plugin
  builds against the extracted `lmv-core-cabi` and `foo_lmv.dll` loads in foobar2000 v2 and
  renders. That closed the one risk [ADR-0072](../adrs/0072-the-c-abi-ships-from-its-own-crate.md)
  carried into C++ link time — the linked artifact renamed to `lmv_core_c.lib`, and CI has no
  plugin job that would have caught a stale path. What remains needs a CI run rather than this
  machine:
  - **Phase 9 — read a cache-warm CI run** (the *second* after the push; the first is a cold
    build, because a `[profile.dev]` edit and a new workspace member each invalidate `rust-cache`
    wholesale). It re-derives `COVERAGE_FLOOR` — currently **91**, measured on a hardware-GPU box
    where CI has WARP — and checks the one property
    [ADR-0073](../adrs/0073-the-windows-ci-critical-path.md) committed to: **`coverage` is the
    longest job**. If it is not, `check (windows-latest)` is build-dominated, and that is the
    single measurement that flips ADR-0073's Alternative A (merge the two Windows jobs) from
    rejected to worth taking — route it back to `architect` as a supplement rather than editing
    the job.
- **Plan [0071] Phase 5 — the retune `occlude` unblocked** (2026-08-09). The plan is `done` and
  every `dev` phase landed; this phase is `human` and was left undone **on purpose**, not missed.
  It is a `preset-author` pass raising the floors that were floored for the black rim, now that the
  ceiling above them is adjustable. **Run it as one pass with [backlog 0038]** — both are retunes of
  the same shipped set against a composite that moved underneath it, both are judged in motion over
  a lit backdrop, and doing them separately means walking the same presets twice. **The plan's own
  text says "0038 and 0058"; that is one entry out of date** — backlog 0058 closed by content on
  2026-08-04 (`859ec66`, all thirteen fold-binding presets now name a `kaleido_edge`), five days
  before this plan reached Phase 5. The three-way pass is a two-way pass. Two things the close
  measured that it should start from: **no shipped
  preset binds `occlude` today**, and at shipped brightnesses the default's effect is almost
  negligible — the ceiling binds where the figure is *dim*, so the presets worth walking first are
  the ones with a dimming depth cue (`swarm_storm`'s `depth_fade`, `lsystem_fern`'s `glow`-dimmed
  outer stems). `lsystem_fern.toml`'s header now says so in place.
- **Plan [0077](done/0077-the-quiet-sky.md) Phase 5 — the quiet sky ships through the curation
  route** (2026-08-12). The plan is `done` and every `dev` phase landed; this phase is `human`
  (content lane) and outstanding **on purpose**, not missed. The lane authors Perseids' quiet
  twinkling sky — sparse marks, low coverage, slow shimmer on swarm `twinkle` — under the
  fresh-slate rule, landing it through the [0067] route at the author's preferred density, with
  the plan's two riders: the sanity floor is *read, not fought* (re-derive by the floor's own
  recorded rule if it prices the sky out — the backlog-0072 precedent), and if the world binds
  `reseed` or any sustained force it owes **one minutes-horizon soak observation with the
  verdict recorded in the world's header** — that is [backlog 0086](../design-backlog.md)'s
  bounded check; the entry stays parked and its trigger re-arms for the next slow-accumulation
  look. Two stale headers the close-grep found belong to the same pass: `fragment_vitrail.toml`
  still explains its onset-`flash` binding by "the report is bloom-blind" (fixed by Phase 4 —
  the binding may stay for its look, but the reason is gone), and `emitter_perseids.toml`'s
  header records the routed-out quiet sky this phase exists to ship.
- **Plan [0081](done/0081-the-sky-gets-a-galaxy.md) Phase 6 — judge the galaxy against the
  reference** (2026-08-12). The plan is `done` and all five `dev` phases landed; this phase is
  `human` and outstanding **on purpose**, not missed. It asks the three questions no instrument in
  this repo can answer, and the first is the one the whole decision rides on:
  - **Does it read as a galaxy, or as a smudge?** [ADR-0095](../adrs/0095-the-backdrop-paints-a-curved-band.md)
    rejected fbm mottling on the bet that the scattered starfield drawn *in front* supplies the
    texture the smooth band lacks. **If it reads as an airbrushed streak that is a result, not a
    failure** — the answer is Alternative A (fbm), with this observation as its evidence, and it
    gets its own ADR and plan rather than a patch. Nothing in the shipped code forecloses it: the
    envelope is a single multiply, so noise multiplies into it later.
  - **Does the arc's curvature read at a normal field of view**, or does `bg_band_curve` have to be
    pushed so far the ends leave the frame?
  - **Does it band under two overlapping gradients?** This is the *same run* as the banding
    reference frame's second check below — do them together, on the kept frame, at 1920x1080.

  Two things to start from. The look wants `bg_bright = 0` with the band alone, which the widened
  build condition now supports and which no earlier configuration could reach. And the backdrop
  still earns a preset **nothing** at `sanity` or `animation`, so however much of the frame the sky
  fills, the figure carries both floors.
- **The banding reference frame is kept in the repo, and it is owed a re-measure twice**
  (2026-08-12). `core/tests/fixtures/scratch-0082/dusk_ground_banding.toml` — a `scratch-NNNN/`
  in the [0046] arrangement, so nothing includes it, no test names it and `LMV_BLESS` does not
  touch it. It is the dusk ground at `bg_ramp_gamma = 0.4`: **the darkest of the Plan 0080 probes**
  (mean RGB 34.0 / 42.7 / 69.5, against 82.2 and 122.6 for the other two) **and** the worst banding
  case, both for the same reason — a fast-dropping ramp leaves a long dim tail, and a flat tail is
  where one 8-bit level lasts longest. It is committed rather than left in a session directory
  because **a before/after taken on two different pictures would prove nothing**. Re-measure it at
  **1920x1080** (plateau width is in pixels, so the resolution is part of the measurement). The
  first of its two checks is **discharged**: after [0082](done/0082-the-gradient-stops-banding.md)
  the widest mid-range plateau went **58 px -> 20 px** and pixels-per-level **7.5 -> 2.1**, still
  0 % rail-pinned, and the `human` verdict on the held frame was that the grain does not read as
  texture. **Not a hairline, which is what that plan predicted** — what the dither buys on this
  picture is the level count and the collapse of wide plateaus from 17 to 3. What remains is the
  second, and **it is now due rather than pending**: [0081](done/0081-the-sky-gets-a-galaxy.md)
  closed 2026-08-12, so add `bg_band_amount` to the frame and confirm the dither still holds under
  **two overlapping gradients** — which nothing inside 0081 checks, and which is why it is named
  here rather than assumed. It is [0081]'s own Phase 6 third question, so **run it in that same
  sitting**; its README carries the run command and both sets of numbers.
- **Plan [0080](done/0080-the-sky-gets-a-horizon.md) Phase 7 — ANSWERED 2026-08-12, and it
  produced a plan.** All three questions are settled; the phase is discharged and only the content
  half remains (folded into the family pass below).
  - **"Does the fade read as light?" — YES.** The user's verdict on the running app at `v0.54.0`:
    *"reads as light, but the banding is visible."* The ramp does what ADR-0094 was written to do.
  - **"Does the horizon sit where the `[palette]` stops put it?" — settled by construction, not by
    eye.** `a_swept_span_samples_the_palette_at_the_coordinate_its_height_implies` asserts exactly
    that at seven rows to within one 8-bit level, and it is green. The question never needed a
    human.
  - **"Does it band?" — YES, and it is now measured rather than judged.** Run lengths down the
    mid-column of the 1080p renders, where a plateau of one identical 8-bit value *is* the band:
    widest mid-range plateau **58 px at value 11** (`bg_ramp_gamma = 0.4`), **31 px at value 30**
    (`1.0`), **122 px at value 225** (`2.5`), with mean 4.1–7.5 px per level. **0 % of the column is
    rail-pinned** on any channel in any of the three — so a quantized gradient, not a tonemap clip,
    which also **retires the suspicion raised at the close that `bg_bright = 0.85` was reaching the
    tonemap's shoulder. It is not; nothing clips.**
  - **The plan's own banding arithmetic was backwards, and that is the finding worth carrying.** Its
    rider read "roughly **two pixels per 8-bit output level** at 1080p, which is the classic
    Mach-band configuration". Two px/level is the *safe*, dense case; banding lives where a level
    lasts a **long** time, in the flattest part of the curve. Its prose instruction ("the low
    `bg_ramp_gamma` end, where the tail is flattest and the steps widest") named the right place
    while its arithmetic argued for the opposite one. Recorded in
    [ADR-0096](../adrs/0096-the-display-write-dithers.md), because that sentence would otherwise
    send the next reader hunting in the bright end where there is nothing to find.
  - **The plan said "if it bands, a dither is its own decision and its own ADR" — so it has one.**
    [ADR-0096](../adrs/0096-the-display-write-dithers.md) +
    [Plan 0082](done/0082-the-gradient-stops-banding.md), sequenced **first** on the roster by the
    user's call and **closed 2026-08-12**, so [0081](done/0081-the-sky-gets-a-galaxy.md)'s band is born
    onto a chain that already dithers.
  - **What remains is content, not judgement.** The dusk world ships through the [0067] curation
    route, and it is **one pass with [0077]'s standing Phase 5** (Perseids' quiet sky) and
    [0081]'s world — three standing items on one family of looks. Two things to start from: the
    backdrop earns a preset **nothing** at `sanity` or `animation` (both are blind to `bg_*`), so
    the figure carries both floors; and the `occlude` question is still open and pairs with
    [0071]'s standing Phase 5 retune above — but the *tonemap-knee* half of that pairing is now
    measured away.
- **Plan [0078](done/0078-the-ink-learns-to-bite.md) Phase 3 — the ink worlds re-judge on
  `ink_gamma`** (2026-08-12). The plan is `done` and both `dev` phases landed; this phase is
  `human` (content lane) and outstanding **on purpose**, not missed. The lever ships and is
  documented — `presets/README.md`'s ink section carries the three-lever note (`ink_gamma` x
  `ink_amount` x `exposure`) and the measured mean-byte ladder. **The roster is two headers, both
  named by the close's step-3b grep, and both were predicted at [0075]'s close as workarounds that
  would go stale the moment this landed**: `reaction_etching.toml` (its duotone is painted into
  `[palette]` because "the ink remap gives a mid-contrast field no contrast lever of its own" — but
  note the world has since been *inverted to scratchboard*, bright line work on black, so whether
  the palette version still earns its place is a live judgement, not a foregone retune), and
  `swarm_shatter.toml` (its light-ground twin was routed out with "when ink grows a contrast
  control, the light-ground twin becomes authorable" — that condition is now met). The output per
  world is a verdict in its header, judged in motion: retune onto `ink_gamma`, or a recorded "the
  palette version stays on its looks". One rider from the close: `dev`'s eyeball on
  `attractor_ink` found that **which way to take the exponent depends on how dense the drawing
  already is** — a sparse figure's bite likely lives *below* 1, not above. If a world needs a toe
  *and* a shoulder, that is [ADR-0092](../adrs/0092-the-ink-remap-gains-a-contrast-exponent.md)'s
  named negative and the finding routes to the backlog with its measurement rather than growing
  the plan.
- **[On-device validation — low-end Windows iGPU smoke](../on-device-validation.md)** — a
  hardware-gated checklist, **not** a phased plan and **not** in the roster above: it never blocks a
  plan from closing. Holds the low-end / older Windows iGPU checks (fps floor ≥ 60 @ 1080p; footprint
  on a second GPU vendor) the user can only run once that box is in hand. Ticked when run; deleted when
  empty. Currently home to the extracted Plan 0012 Phase 3 (also covers the identical Plan 0003 Phase 3
  iGPU-fps carry-forward).


## Recently closed

One line per plan. **The full close write-ups — review verdicts, the findings
each close recorded, the properties that outlived the plan — moved verbatim to
[README-archive.md](README-archive.md)** (Plan 0061 Phase 7b). Nothing was
deleted; it simply stopped being loaded into every session's context.

- [0081 — The sky gets a galaxy: the backdrop paints a curved band](done/0081-the-sky-gets-a-galaxy.md) — closed 2026-08-12 (all five `dev` phases; **Phase 6 is `human` and deliberately outstanding — see Standing**). Review: **no blockers, no majors, two minors, two nits** (minors, both repaired in the close series: `docs/capturing.md`'s *"One golden baseline is lit, and it is the exception that proves the rule"* was falsified by this plan's own fixture — there are now two, which is the same doc drifting in the same paragraph that Plan 0080's close repaired; and this README's standing baseline-drift control still said "27 baselines" against a directory holding 28. Nits: `backdrop_ramp.rs`'s `0.25`-row bounds on the straight-band spread and the bow's shear are the only two thresholds in the new suite without a stated derivation, and the tighter of them has 1.6x headroom over its measured 0.16 in a statistic the dither now feeds; and the shader's `select` evaluates both arms, so a backdrop with no band pays two extra LUT fetches per pixel where an `if` would give byte-identical output and skip them). **The backdrop paints one soft curved band** ([ADR-0095](../adrs/0095-the-backdrop-paints-a-curved-band.md)): seven params — `bg_band_amount`/`bg_band_angle`/`bg_band_pos`/`bg_band_width`/`bg_band_curve`/`bg_band_hue`/`bg_band_hue_span` — drawn **additively over the ground and under the scene**, which is what unresolved starlight is, so the four-role look (ground, band, stars, figure) becomes authorable without widening [ADR-0090](../adrs/0090-a-preset-composes-two-scene-layers.md)'s one-`[layer]` cap. **Every default is an arithmetic identity and it was proved three times**, once per code phase, by the bless-to-bless control (bless twice on the branch differing only by reverting the change — never a `git diff`, since eight baselines drift from their committed bytes on this box anyway). The identity is **structural rather than arithmetic**: `bg_band_amount = 0` takes a `select` arm, so the pre-band expression is the *untaken* branch and the inertness test asserts `worst == 0` over the whole frame with every other band param bound off its default. **The widened build condition (`bg_bright > 0 || bg_band_amount > 0`) is the plan's one-line change and it has its own test** — a band over an unlit ground, the only thing in the suite that can see it, with the `amount = 0` companion coming back the plain black clear. Three axes now share **one** `axis_pos()` and both palette fetches one `palette_at()`, deliberately so ADR-0037's trap cannot be fixed in one axis and left in another. **Two plan-accuracy findings, both recorded by `dev` rather than absorbed, both in [ADR-0095's Outcome](../adrs/0095-the-backdrop-paints-a-curved-band.md#outcome--2026-08-12-at-plan-0081s-close):** (i) the plan, this README's roster row and the ADR all framed the along-band normalizer as *not* cancelling at the default angle, making Phase 2 the sole possible sighting of ADR-0037 on that axis — **it cancels** (numerator `ndc.x * aspect`, denominator `aspect`), confirmed by re-running the bow measurement with the aspect forced to 1.0 and reproducing every digit; what Phase 2 actually catches is a wrong normalizer *form*, verified to bite (dropping the aspect from the denominator alone shears the arc 1.36 rows and fails the first edge assertion), and the trap itself is one property across all three axes because they read the single aspect the pass is handed. (ii) ADR-0095's table ("its own coordinate in the same `[palette]`", absolute) and the plan's Phase 3 done-when ("leave the band on the ground's own coordinate", an offset) **cannot both hold**; `dev` raised the fork and **the user chose absolute**, on the authoring argument that the ground's coordinate varies along the band's path, so an offset would drag the ramp's sweep into the arc. **Every numeric done-when is a differential, never a magnitude.** The `1/e` half-width goes into the *control* — a flat band at the upper width rail carrying `amount/e` — because the tonemap and the sRGB encode sit between the envelope and the 8-bit write, so an asserted ratio of 0.368 would be a claim about the tonemap's shoulder; the two frames agree to 1 level at the crossings and differ by 21 at the centre. The bow is *located* column by column with a **luma-weighted centroid rather than an argmax**, a direct consequence of [0082] landing first: a gaussian's peak is flat, `pos = 0.5` puts the centre between two pixels, and the dither's ±1 LSB makes argmax report rows 31, 32 and 33 for three columns of one straight band. The fifth `EXTRA_FIXTURES` entry is **appended, never inserted**, so no pre-existing baseline is rendered from different device state, and **its seven values were tuned against the suite's own tolerance rather than for looks** — each reverted to its default in turn and re-measured, all seven clearing `MEAN_TOL = 0.02`, the tightest by 1.1x; two needed the tuning (a wide `bg_band_width` scored 0.0055 on its own revert, and the colour pair fights itself through the repeat addressing). Blessed only after comparing adapters, which the `48 → 80 B` uniform growth owes: WARP `152.200 120.540 086.088` against hardware `152.198 120.550 086.114`, 0.026 of one level, and the other 27 baselines restored to their committed bytes after the un-scoped `LMV_BLESS`. The ADR-0058 enumeration and `shot --report`'s generic binding walk were **confirmed, not edited** — a changed answer would have been the finding. Phase 5's sweep of `.claude/skills/preset-author/references/**` was a **done-when rather than a reviewer's catch**, which is the fix for the identical minor raised at both Plan 0078's and Plan 0080's closes; it landed. **Curation (step 3b):** no preset content landed — only `presets/README.md` and `docs/preset-palettes.md` — so no near-duplicate sweep owed; the workaround grep over all 27 headers finds **nothing** citing a missing band or a painted-in galaxy, which is expected, since nothing could express the shape to work around.
- [0082 — The gradient stops banding: the display write dithers](done/0082-the-gradient-stops-banding.md) — closed 2026-08-12 (four `dev` phases plus a self-repair, and the `human` Phase 5 **answered rather than left standing**). Review: **no blockers, one major, five minors, three nits** — and every finding is a consequence of something the *plan* got wrong, recorded honestly by `dev` rather than absorbed. **The tonemap dithers** ([ADR-0096](../adrs/0096-the-display-write-dithers.md)): ±1 **encoded** LSB of TPDF noise from an integer hash of the pixel coordinates, divided by the sRGB transfer function's local slope because `Rgba8UnormSrgb` means the *hardware* encodes after the shader. One site, always on, not a param, no time term. The dusk ground's dark tail went **7.5 px/level and a 58-px plateau → 2.1 and 20**, wide plateaus 17 → 3, still 0 % rail-pinned; the user's by-eye verdict on the held frame was *"looks fine"* on both halves, which **retires ADR-0096 Alternative F** (the animated dither) as a followup. **The major is the ADR, accepted with a dated [Outcome](../adrs/0096-the-display-write-dithers.md#outcome--2026-08-12-at-plan-0082s-close) that falsifies two of its claims.** (i) Its "three parts, each load-bearing" is **four**: the dither must **fade at the rails**, which the ADR never mentions — at a rail the value is already exactly representable and the write clamps, so half the noise is discarded and what survives is a **DC lift**, and an exactly-black frame came back at mean **0.18/255** over a suite where nearly every fixture runs `bg_bright = 0`. Caught by two *existing* guards (the emitter burst test at lead peak 0.1827 where it asserts empty, and bloom roundness), not a new one; `dither_offset`'s fade is provably inert at and above code value 1, because below the knee the slope is the exact constant 12.92 so `min(l, 1-l) * slope * 255` **is** the encoded byte value. (ii) Its third Positive consequence — the headline argument that an integer hash buys **byte-for-byte** adapter agreement, sharper than the 0.02 drift floor — is **false**. The hash is exact (65 536 float values, zero differing bit patterns), but the **hardware sRGB encode downstream of it** is not: DX12 permits tolerance in float-to-sRGB8 and WARP's approximation departs from the true curve below ~byte 20, so **212 of 2 049 408 re-blessed channels move by 2**. The integer hash still earned its place (Alternative C would have diverged on essentially *every* pixel), but the promised instrument does not exist. **Three of this plan's own numeric done-whens were wrong**, which is the architect-side lesson: "the golden suite goes red here" — it did not, the guard is a **tolerance** guard and a one-level shift reports 0.0007-0.0013 against 0.02, three orders of magnitude inside, so Phase 2 was a deliberate **re-pin** rather than the repair of a red build; "a delta of 2 anywhere is a finding" — bounded-by-one is a **hardware** claim and the shipped assertion reads its bound off `is_software()`; and the TL;DR's "hairlines", which the honest 20 px replaces. **The first deliberate full re-bless in the project's history** landed alone in its own commit, measured **bless-to-bless** (8 of the 27 rewrite against their committed bytes on a clean local bless, so a `git diff` would have charged that drift to the dither). Verified at the close rather than taken on trust: golden 27/27 green, `backdrop_ramp` 6/6 so the `<= 1` tolerances survived, all four byte-equality tests pass including Plan 0075's `depth_fade` no-op against a live Lorenz control — the property that made *static* the right choice — and both dither tests reproduce their recorded numbers exactly. Two scope calls at the Step-1 gate, both right: `mix32` / `unit01` promoted to `gpu::HASH_WGSL` (the shared home [0077]'s close asked a *third particle scene* to build, arriving instead from the display write), and Phase 3's guard placed **in-crate** because an integration test cannot reach a `#[cfg(test)]` field and a public off-switch is exactly what Alternative G rejects. No ADR-0058 entry was owed — the amplitude reuses the `Ctl` uniform's already-zero `.z`, so the layout shape is unchanged. Minors repaired in the close series: `docs/on-device-validation.md` had not learned the dither (three `pow` per pixel on a fullscreen draw, never measured on weak hardware; and the grain verdict was taken on **one display**, where a 6-bit + FRC panel running its own temporal dither is the case it cannot speak to), and the two "before" figures for the dusk probe disagree 2.3x with the scan axis unrecorded — written down as a **stated discrepancy** rather than an invented explanation. Nits: the corrected WARP mechanism's disproof lives in prose only and nothing asserts it; both dither tests request the software adapter so the tight hardware bound never runs automatically (fine — the derived-1/3 mean is the adapter-robust guard); and `core/Cargo.toml:31`'s last surviving mojibake, repaired. **Curation (step 3b):** no preset content landed — only `presets/README.md` — so no near-duplicate sweep owed; the workaround grep over all 27 headers finds **nothing** citing banding or a step-breaking stop, because no shipped preset binds the ramp params yet.
- [0080 — The sky gets a horizon: the backdrop paints a directional ramp](done/0080-the-sky-gets-a-horizon.md) — closed 2026-08-12 (all six `dev` phases; **Phase 7 is `human` and deliberately outstanding — see Standing**). Review: **no blockers, no majors, three minors, one nit** (minors, all repaired in the close series: the `preset-author` lane's own `references/systems.md` and `craft.md` did not know the ramp exists — the Plan 0078 `ink_gamma` minor repeated verbatim, and load-bearing because Phase 7's followup *is* that lane authoring a ramp world; `docs/capturing.md`'s "every golden baseline runs `bg_bright = 0`" was falsified by this plan's own fixture, the suite's **first lit golden baseline**; and this README's standing baseline-drift control still said "8 of 20" against a directory holding 26. Nit: `backdrop_ramp.rs:273`'s `+ 100.0` reversal margin is the one threshold in the new suite without a stated derivation, in a suite otherwise exemplary on [ADR-0071](../adrs/0071-a-numeric-test-contract-states-a-property-or-names-its-machine.md)). **The backdrop paints a directional ramp** ([ADR-0094](../adrs/0094-the-backdrop-paints-a-directional-ramp.md), closes backlog 0091, a user-rejected workaround rather than speculation): five params (`bg_angle`, `bg_hue_span`, `bg_shade`/`bg_shade_end`, `bg_ramp_gamma`) turn one palette *sample* into a *segment* swept along one axis, with the hardcoded `mix(0.72, 1.0, ndc.y)` tilt **retired into** the shade ramp so there is one brightness ramp on the frame rather than two, and horizon placement authored by the `[palette]` stops' own `at` positions — no second placement mechanism. **Every default is an arithmetic identity, and that was proved four times**: all 26 pre-existing baselines came back hash-identical under a bless-to-bless control once per code phase (bless twice on the branch, differing only by reverting the change — never a `git diff`, on this box eight drift from their committed bytes anyway). **ADR-0037's trap was instrumented, not argued.** At `bg_angle = 0` the aspect term *provably* cancels (`d = (0,1)`, denominator `aspect * 0 + 1`), so no default-angle test anywhere could tell a right source from a wrong one; the control runs at π/4 on a 160x100 target **with `trails` active**, because with an empty chain `target.size` **is** `surface` and the control would be theatre — at that size the internal grid quantizes to a square 256x256, so the wrong source is aspect 1.0 exactly against the surface's 1.6. It was verified to **bite**: `composite_into` was temporarily re-pointed at `target.size` and the test failed on its *first* assertion at 20 levels, which is ADR-0037's symptom (turning a stage on changes the shape of the picture) stated directly. The asserted 99-column crossing is derived, not measured — Δndc.x = 2/A = 1.25, times (1 − 1/H) for the edge rows' half-pixel inset, times W/2. The exponent reuses [ADR-0092](../adrs/0092-the-ink-remap-gains-a-contrast-exponent.md)'s shipped `select(pow(s, g), s, g == 1.0)` where the branch is a **correctness requirement** (`pow(x, 1.0)` is `exp2(1.0 * log2(x))`, not bit-exact), and its test asserts *placement* as well as agreement — `e = 0.5` sits at `s = 0.5^(1/g)`, giving rows 15.0/31.5/52.2 over 64, measured 15/15, 32/31, 52/52; agreement alone would have passed with the exponent inert on both channels. The 32 → 48 B uniform growth needed no ADR-0058 entry (the enumeration records **whether** a `min_binding_size` is declared, deliberately not which) and the adapters agreed to **0.044 of one 8-bit level** before blessing. Two `dev` findings recorded rather than absorbed: the plan's "20 baselines" is 26, and `shot --report` needs no per-namespace list because it walks bindings generically (verified with a probe binding four `bg_*` names, one live gate and one dead). **Curation (step 3b):** no preset content landed — only `presets/README.md` — so no near-duplicate sweep owed; the workaround grep over all 27 headers finds **nothing** citing the missing gradient this plan supplies, which is expected: the rejected workaround was never landed.
- [0078 — The ink learns to bite: a contrast exponent on the terminal remap](done/0078-the-ink-learns-to-bite.md) — closed 2026-08-12 (both `dev` phases; **Phase 3 is `human` and deliberately outstanding — see Standing**). Review: **no blockers, no majors, three minors, one nit** (minors: Phase 1 substituted a structural argument for the bless-to-bless control its done-when named — the substitution is the *stronger* instrument and is recorded as such, not repaired; `.claude/skills/preset-author/references/systems.md`'s ink row did not know `ink_gamma` exists, repaired in the close series and load-bearing because Phase 3 is that lane retuning against that table; two shipped preset headers now describe as forced a workaround the engine no longer forces, which is Phase 3's roster rather than a fix. Nit: the mid-band mean's `+ 1.0` byte margin sits below `golden.rs`'s own `0.02`-normalized drift floor for a mean statistic — defensible here because it is a lower bound on 12-33-byte gaps through an *injected* deterministic ramp, not a rendered scene, and the tighter per-level `<= 1.0` tolerance has been measured on WARP). **`ink_gamma` lands** ([ADR-0092](../adrs/0092-the-ink-remap-gains-a-contrast-exponent.md), backlog 0084, hit 3x across two Plan 0075 cohorts): `mix(paper, ink, luma^g)`, endpoints invariant *by arithmetic* rather than by tuning, so the paper never moves at any value. **The default's exact identity had to be built, not inherited** — `pow(x, 1.0)` is `exp2(1.0 * log2(x))` and not bit-exact, so the shader takes an explicit `g == 1.0` branch; without it the zero-baseline sentence would have been false by a rounding step. **And the zero-baseline claim is structural in a stronger sense than the plan argued**: no golden fixture binds `ink_amount` at all and the stage builds its resources only when `active()`, so no committed baseline ever constructs the ink pass — which also covers the one thing a param grep cannot see, the `COPY_DST` flag added to `ink-src` for the endpoint test (the arrangement `tonemap-src` already carried). Endpoint invariance is asserted through the shipped WGSL across `g = 0.25/0.5/1/2/4` **and** over hostile values (0, negative, NaN, ±∞) on the CPU mirror; the test injects a 256-step ramp because no rendered scene reaches key 1 (ADR-0046's shoulder is bounded strictly below it). Two `dev` judgment calls the plan did not specify — the CPU-side crossfade of `gamma` alongside `ink_amount`, and the finite `0.05 .. 20` clamp — are recorded in ADR-0092's [Outcome](../adrs/0092-the-ink-remap-gains-a-contrast-exponent.md#outcome--2026-08-12-at-plan-0078s-close), which falsifies nothing in the ADR. The published mean-byte ladder (`147 / 180 / 209 / 229 / 241`) reproduces independently from pure sRGB-to-linear arithmetic (`147.1 / 180.4 / 208.6 / 228.6 / 241.0`) — a property of the math, not of a rig, which is why it is publishable without naming one. Two plan-accuracy drifts caught and recorded by `dev`: `schema.rs` needed no edit (`GLOBAL_PARAMS` aggregates `ink::PARAMS` by reference — verified, there is no second roster), and two unlisted files were required. **Curation (step 3b):** no preset content landed, so no near-dup sweep owed; the workaround grep names `reaction_etching` and `swarm_shatter`, both carried into the Standing entry.
- [0077 — The quiet sky: the sparse idiom becomes gateable and the swarm individuates](done/0077-the-quiet-sky.md) — closed 2026-08-12 (`dev` scope; **Phase 5 is `human` and deliberately outstanding — see Standing**). Review: **no blockers, no majors, two minors** (both repaired in the close series: `docs/capturing.md` had not learned the report's new footprint block / `reactivity_footprint` JSON key — the operator-doc sweep `dev` correctly left for the close; and the gate's deliberate semantic change — backdrop-only drift no longer counts as animation — lived only in the test until ADR-0091's Outcome recorded it) **and two nits** (`report.rs`'s "the reading never sits below the mean" is a practical-regime claim, not a theorem — sub-`eps` differences on unlit pixels leave the numerator while the denominator shrinks; and the emitter's `unit` hash is now mirrored verbatim into `swarm.rs`, a deliberate, commented duplication that a third particle scene should promote to a shared home). **The sparse idiom becomes gateable**: the animation gate scores `metrics::footprint_diff` — the masked form, chosen over the quotient with the reason recorded on the function — with every constant carrying its derivation (floor at half the shipped minimum; the 139-pixel mask floor capping a one-pixel flicker at 0.0072), the `bg_*` strip re-learning ADR-0067 by measurement (backdrops on, the sparse probe's footprint read 65 % of the frame), the rejected fifth-density Squall draft **passing at 0.1049** where the whole-frame statistic priced it out at 0.0057, the static control failing on a zero numerator, both pinned as a standing non-vacuity test, and the whole-library sweep convicting nothing. **The swarm individuates**: `twinkle`/`size_spread` off the particle's index through the emitter's unit hash — deliberately not `SeededRng`, whose extra stream draw would re-scatter the field — exactly 1.0 at their zero defaults so the goldens pass unblessed, with the shimmer-without-breathing bound derived from the mechanism (`8 * TWINKLE / sqrt(N_visible)`, 16x under the sheet-flash signature). **The swarm gains `reseed`** with ADR-0066's disturbance semantics (±6 % domain-relative kick, never a box respawn), catching a live defect class en route: resetting `prev_reseed` in per-frame `reset_params` turns a held gate into an edge per frame (measured diverging, 105 % coverage gap at 10 s) — the omission is now commented on both scenes. **The report sees bloom** (backlog 0088): the mean columns stay untouched and a footprint reading lands beside them at zero extra GPU cost — the bloom-only fixture reads bass 0.161 against the mean's 0.004, unbound bands stay 0.000 in both readings, and the `flash`-lever house workaround is obsolete. Plan drift recorded honestly by `dev` in the phase commits (no `schema.rs` edit exists to make; the report machinery lives in `report.rs` since Plan 0061). **Curation (step 3b):** no preset content landed, so no near-dup sweep owed; the workaround grep lists two headers for the content lane — `fragment_vitrail`'s "report is bloom-blind" rationale (fixed by Phase 4) and Perseids' routed-out quiet sky (Phase 5's own subject) — named in the Standing entry.
- [0075 — The content renaissance: the library is rebuilt as worlds, by replacement cohorts](done/0075-the-content-renaissance.md) — closed 2026-08-11. Review: **no blockers, no majors, two minors, two nits** (minors: rustfmt drift on two test files the lane touched, repaired in the close series as `6a5a9c6` — the "557/557 green" handoff claim was nextest, which does not check fmt, and the fmt-running pre-push hook never fired because the lane never pushed; the roster row's "the library is 28 worlds" against a measured 25 after cohort 5, moot with the row's deletion here. nits: `standalone/src/shot/report.rs` reaches the extent diagnostic through the deep `lmv_core::render::scenes::lines::renderer` path — a `render`-root re-export would keep the shell at arm's length; Phase 2's "Files touched" named `standalone/examples/shot.rs`, which Plan 0061 Phase 4 had already moved — `dev` caught and recorded the drift in the phase commit). **R6 lands: the library is rebuilt as 27 worlds — the brief's 9 keeps plus 18 authored fresh-slate — through six family cohorts, each landing its worlds through the [0067] route and retiring its named roster in the same series** (45 → 27, ADR-0089's mechanism held: the set was never hollow, the gates never went vacuous, and every cohort was judged live by the user before its retirements committed). Phase 1 ended the sanity floor's selecting-for-the-defect: `metrics::radial_shell_occupancy` (ten annuli over the inscribed disc) rescues a preset under its coverage floor at ≥ 4 occupied shells — the three retired ring mandalas at their honest tunings (frozen byte-for-byte, the backlog's exact pinned numbers 0.2442/0.2505/0.2544) read 10/10/9 shells, the frozen renders-nothing defect reads 0 and still fails, and every constant states its derivation (ADR-0071). Phase 2 made `depth_fade` an exact no-op on flat families — asserted by **byte equality** with a live Lorenz control so the no-op cannot pass vacuously — recorded as ADR-0076's second dated [Outcome](../adrs/0076-the-attractor-keeps-the-depth-it-already-computes.md#outcome-added-at-plan-0075s-close-2026-08-11); and the in-frame geometry fraction joined `shot --report` as the `geom` column, printed exactly where a line seam exists (JSON mirrors the omission). Phase 3 landed the measured depth-lever corrections (the `perspective` orbit and its ~0.3 ceiling, `depth_hue`'s three regimes, the `spin`×`fade` smear ceilings) in `presets/README.md` and `docs/preset-palettes.md`. Cohort 6 shipped the library's first two layered worlds (Vitrail, Sumi) on [0076]'s `[layer]`. Retirement commits froze the test fixtures they orphaned (Star Rosette's ladder source, the honest mandala tunings) rather than leaving dangling `include_str!`s. Engine feedback routed out as designed: backlog 0084–0089 plus re-raises, promoted to [0077](done/0077-the-quiet-sky.md)/[0078](done/0078-the-ink-learns-to-bite.md)/[0079](0079-the-attractor-learns-new-figures.md), nothing absorbed into the plan. Suite 665/665 after the merge with `main`; fmt + clippy clean. **Curation (step 3b), from a fresh `--report` run over the final 27 at this close:** zero near-duplicates below shape 0.08 in all nine families, every gate branch taken under the 110 BPM probe, no clamp saturated, `occ` 0 across the set; the workaround grep finds no header citing an already-fixed defect — three cite approved-but-unbuilt fixes (Perseids → [0077], Shatter's rebuild → [0077], Etching's duotone → [0078]), each already named inside its fixing plan. No curation action owed; the set ships as authored.
- [0076 — The second layer: a preset composes two scenes (R3)](done/0076-the-second-layer.md) — closed 2026-08-11. Review: **no blockers, no majors; one minor** (Phase 2's commit message attributed the memory measurement to WARP when it ran on the hardware adapter — corrected of record inside Phase 4's own commit, nothing left to fix) plus close-time roster/link staleness repaired in the close commit. **R3 lands: a preset composes a second scene through one optional `[layer]` table**, joined `under` (same target, one extra draw, one substance) or `over` (own offscreen, linear-light blend between kaleidoscope and bloom — `add`/`screen`/`multiply`/`overlay` fixed at load, `mix` bindable). **Per-preset scene instances ended the one-instance-per-system roster** (the user's call in ADR-0090), and the Phase 2 discovery is recorded where it was found: a shared `LineRenderer` is *not* shareable between two live line draws (`Queue::write_buffer` applies before the submission's passes), so a layer line scene carries its own. **Layerless presets are byte-identical by construction and by count** — backdrop + scene + tonemap is still exactly 3 draws, and `mix = 0` is byte-identical through both junction positions. The routing junction stays a pure function, unit-enumerated over all eight active-flag combinations without a GPU. Reachability, saturation and the report walk `[layer]` bindings under their own namespace; a dead layer gate flags. Measured on this box, stated as measurements (ADR-0071): attractor+RD ~11.9-12.9 ms/frame at 1080p Floor, layered RD pair +303 MB peak working set (debug/WARP; the release-hardware expectation is the ~33 MB texture arithmetic), layered gate fixture +0.9 ms/frame. Two golden fixtures pin both joins, adapter-compared before blessing (WARP vs hardware mean 0.0002). WARP aliases the same-system pair's identical layouts, so the independence guard runs on hardware and skips-with-notice on software (ADR-0058 posture). The Phase 5 verdicts: pre-bloom `over` reads as intended, all four modes ship under their names, and the fullscreen-`under`-occludes finding became authoring guidance in `presets/README.md`'s new `[layer]` section. No shipped preset declares a layer yet — that is [0075] cohort 6's work, now unblocked. Curation (step 3b): no preset content landed and no engine defect was fixed, so no workaround sweep was owed; verdict "no content change".
- [0053 — The suite stops blessing what WARP gets wrong](done/0053-the-suite-stops-blessing-what-warp-gets-wrong.md) — closed 2026-08-09. Review: **no blockers, two majors, four minors**. **The plan was written to prove the collisions benign and instead found two live mis-renders and fixed them.** `background-bind-layout` collided with the fullscreen scenes' single uniforms and rendered the fragment field a **flat grey** on WARP (`142.712` on all three channels against hardware's `131.010 170.559 141.381`); `blend-bind-layout` collided with `trails-bind-layout` and rendered a dissolve between two `trails`-binding presets wrong (`08.999 38.682 45.794` against `17.572 48.211 50.116`). Both fixed by an explicit `min_binding_size`, each isolated against a no-collision control that agrees to **0.02 of one 8-bit level**. That makes ADR-0058's "nothing is observed to be wrong" and "it fixes nothing that is currently broken" **false**, recorded in its [Outcome](../adrs/0058-bind-group-layout-collisions-carry-evidence.md#outcome--2026-08-09-at-plan-0053s-close). Nine colliding pairs became **four** allowlisted, each carrying a measurement; the shape the test keys on widened from binding *kinds* to kinds + visibility + whether a `min_binding_size` is declared, the first forced by ADR-0058's own boxed note and the second **measured here, twice independently**. Guard confirmed in the reverted direction at the review: dropping either fix back to `gpu::uniform` re-collides its pair and fails. **Zero baselines moved** — a bless-to-bless control differing only by reverting the two fixes came back hash-identical across all 20 PNGs. The line seam's lit-backdrop guard went from **15 channels** of reach to the whole stroke footprint (779 of 28 173 post-fix against 28 178 pre-fix) via a fourth capture at zero emitted light, and the swarm took the same arm. **Two majors left as findings, both consequences of the fix rather than defects in it:** the allowlist's `RIG` names neither adapter, in a repo whose ADR-0074 `Outcome` established five days earlier that this box's WARP 10.0.19041 is the outlier and CI's 10.0.26100 is not — so four `AGREES` entries grant permission on a build never measured; and `core/tests/background_composite.rs` still skips on every software adapter citing the quirk this plan **fixed**, so a check CI has never run may now be liftable (unmeasured — the attractor half is a different layout group). **Phase 3 is `human` and was run by `dev` under the user's explicit authorization at the gate.**
- [0046 — Transformed feedback: the past learns to move](done/0046-transformed-feedback.md) — closed 2026-08-09. Review: **no blockers, no majors, four minors, two nits**. **R2 lands, and with it the last engine gate on [0075]** — both accumulation buffers now resample their past through one shared transform, so a zoom is a tunnel and a rotation a spiral, and the standing `trails` frame-rate defect (a `fade` applied once per *frame*, a third as long at 144 Hz as at 48) is retired en route. **Zero pixels moved without opt-in**: all 20 pre-existing baselines hash-identical to a clean-`main` bless, three added, re-measured at this close after the merge. **[ADR-0037] was caught for the third time here** — `Trails::resolve` had been ignoring its `surface` argument on a premise a rotation falsifies — and the fix carries the negative control this rule has always needed: a spun ring boxes **45x46** at a portrait 100x160 target, against **44x71** with the aspect forced to `1.0`. **One contract narrowed:** `[feedback] blend` reaches the trails stage only, because the attractor deposits additively in one pass and a `max` there would cost a second pipeline — the exact WARP hazard the one-shader warp family avoids. The plan sentence changed, not the code. **Alternative D was overcharged**: including the attractor cost one shared `Transform` and one WGSL snippet concatenated into both shaders, not "a second shader and test surface". Two Phase 5 observations became [backlog 0082](../design-backlog.md) (`frame_ms_p99` spikes to 25.0 ms on preset switches, and the not-yet-built quality governor is specified to read that column) and [backlog 0083](../design-backlog.md) (RSS 385 → 663 MB over three minutes, with no no-feedback control beside it).
- [0068 — Why the downbeat rarely locks](done/0068-why-the-downbeat-rarely-locks.md) — closed 2026-08-09. Review: **no blockers, no majors, two minors, one nit**. **It shipped the diagnosis and no fix, as designed, and that was verified rather than assumed** — `CONFIDENCE_THRESHOLD` is still `0.25`, `BASS_WEIGHT` still `0.7`, the fold and the confidence measure are untouched, and the `effect_size` split is arithmetic-identical on every branch. The probe reads the gate rather than second-guessing it: `terms()` is `&self`, allocation-free, clock-free, and its `to_bits()` equality against the published `BarClock::confidence` is asserted on all six cases. **The named cause is the accent feature's bass weighting**, and the finding that names it is that backbeat rock/pop locks **48x worse** than four-on-the-floor — 0.14 % against 6.79 % over 98 minutes — because a bass accent marks every beat in one and the half-bar in the other, so it hardly ever marks the *bar*. [ADR-0082](../adrs/0082-the-downbeat-gate-holds-and-the-estimator-is-diagnosed-first.md) is **accepted** carrying that dated `Outcome`, including the limit that matters most: the 1 Hz log records **band levels, not per-beat accents**, so this is a ladder match plus a construction argument and *not* a direct measurement of the four alignment scores on real audio. **The repair has no ADR and no plan**, and that is a decision taken at this close rather than an omission: the route is named ([design-backlog 0042](../design-backlog.md), answered in place) but the fork an ADR would decide — a stronger accent feature versus a longer history window — is not yet a real fork, because ADR-0082's `Outcome` exonerates the fold and the confidence measure. It stays a backlog pointer until someone takes it.
- [0067 — The curation route](done/0067-the-curation-route.md) — closed 2026-08-09. Review: **no blockers and no code findings**. The one substantive item was a **factual error in the plan** — its claim that `bar` "stopped being a variable at ADR-0050" is false (`bar` is `VAR_NAMES[5]`, the beat phase in `[0, 1)`; ADR-0050 *added* `bar_phase` alongside it) — struck in the plan and in [ADR-0081](../adrs/0081-the-content-lane-lands-presets-and-architect-curates-the-set.md)'s `Outcome`, which repeated it. **The gate is now worth leaning on**: `reactivity` drives PCM through the real analyzer, with a non-vacuity test that has a positive control, and ADR-0081's gate-strength Negative is discharged. **Phase 1d is a recorded negative result** — the resolution ladder is flat because `frame_diff` scores occupancy and occupancy is scale-invariant, so `ANIM_FLOOR` and `SIZE` did not move and CI pays nothing; [backlog 0009](../design-backlog.md) now needs a coverage-aware statistic, which is the earned question. Two costs the lane refused to absorb went to [backlog 0080](../design-backlog.md) (reactivity 1.8x, ~85 % of it warm-up hops that are rendered and thrown away) and the Coral Oracle's gain exception to [backlog 0081](../design-backlog.md) (the house gain rule is written down nowhere).
- [0064 — The symmetry stage and the banded palette](done/0064-the-symmetry-stage-and-the-banded-palette.md) — closed 2026-08-09. Review: **no blockers and no code findings**; the three items raised were all stale text in the plan itself, corrected at the close (its "fourteen" baselines are **19**, its five LUT sample sites are **seven**, and its "duplicated at five sites" risk is **three** WGSL copies plus four calls into the one Rust function). ADR-0037 verified clean at `kaleidoscope.rs:1116` — the aspect comes from the render target, in the stage that has shipped that bug twice. All 19 pre-existing baselines byte-identical. **[ADR-0077](../adrs/0077-the-symmetry-stage-owns-one-coordinate-map.md) was accepted with an [Outcome](../adrs/0077-the-symmetry-stage-owns-one-coordinate-map.md#outcome--2026-08-09-at-plan-0064s-close): its "the inner rings alias severely" was inferred from a texel ratio and never observed** — six cutoffs, three sources, no visible onset — so `kaleido_inner` ships as styling with a protective side effect, not as the rescue. **First run of close-ceremony step 3b**: no near-duplicate geometry in any of the nine families, and nothing in `presets/` still pays for a fixed defect.
- [0071 — Light that adds without covering (`occlude`)](done/0071-light-that-adds-without-covering.md) — closed 2026-08-09. Review: no blockers, two majors, three minors, two nits. **Phase 5 is `human` and deliberately outstanding — see Standing.** The default stayed `1.0` by the user's look, no preset binds it, and **no golden baseline moved** (measured as a bless-against-a-clean-`main`-bless, all 19 hash-identical). Three ADR-0085 claims were falsified and are in its [Outcome](../adrs/0085-how-much-a-scene-occludes-the-backdrop-is-one-number.md#outcome-2026-08-09-after-plan-0071): there was no "one seam" (six sites, four shaders), the `Scene` trait *did* widen (`set_occlude`, its fourth optional method — it meets [ADR-0030](../adrs/0030-scene-target-size-hot-path-hook.md)'s three conditions, checked at the close), and the families *did* drift (additive scenes with an empty chain are unoccluded whatever they bind). It also emptied [ADR-0058](../adrs/0058-bind-group-layout-collisions-carry-evidence.md)'s `[Texture, Sampler]` group — see [0053]'s row.
- [0072 — The backdrop joins the palette](done/0072-the-backdrop-joins-the-palette.md) — closed 2026-08-09. Review: no blockers, no majors, four minors, three nits. **The last surface outside `[palette]` joined it**: no cosine copy remains in `background.rs`, and `saturation` / `palette_mix` fan out to the sky through one binding. Two of the plan's own claims were falsified and are recorded in [ADR-0086](../adrs/0086-the-backdrop-colours-through-the-preset-palette.md)'s `Outcome` — **the two fixtures it ordered re-blessed pin no pixels**, and its "fifteen" is 18 by its own grep (16 in scope, 3 moved). Zero golden baselines changed.
- [0061 — The build stops paying for what it is not building](done/0061-the-build-stops-paying-for-what-it-is-not-building.md) — closed 2026-08-08. Review: no blockers, two majors, three minors (all five doc drift, all repaired at the close). **Phases 8 and 9 are `human` and carried forward — see Standing.**
- [0074 — The figure colours by how far it has come](done/0074-the-figure-colours-by-how-far-it-has-come.md) — closed 2026-08-08. Review: no blockers, four minor items (three repaired at the close)
- [0073 — The fern unfurls and colours by what made it](done/0073-the-fern-unfurls-and-colours-by-what-made-it.md) — closed 2026-08-06. Review: no blockers, two minor doc items repaired at the close
- [0065 — The mandala interior: `star_pattern` stops being hollow](done/0065-the-mandala-interior.md) — closed 2026-08-06. Review: no blockers
- [0069 — The instrument that sees a figure leave the frame](done/0069-the-instrument-that-sees-a-figure-leave-the-frame.md) — closed 2026-08-06. Review: no blockers, three minor, one nit
- [0070 — Shaped marks](done/0070-shaped-marks.md) — closed 2026-08-05. Review: no blockers, one minor
- [0066 — The level lever](done/0066-the-level-lever.md) — closed 2026-08-05. Review: no blockers, one minor
- [0062 — The chaos game grows a fern](done/0062-the-chaos-game-grows-a-fern.md) — closed 2026-08-05. Review: no blockers, one major, four minor
- [0063 — The attractor keeps its depth](done/0063-the-attractor-keeps-its-depth.md) — closed 2026-08-04. Review: no blockers, one major, two minor
- [0036 — macOS and Windows release artifacts](done/0036-macos-and-windows-release-artifacts.md) — closed 2026-08-04. Review: no blockers, one minor, one nit
- [0055 — The fold edge becomes a choice](done/0055-the-fold-edge-becomes-a-choice.md) — closed 2026-08-04. Review: no blockers
- [0051 — the scene seam emits premultiplied alpha](done/0051-the-scene-seam-emits-premultiplied-alpha.md) — closed 2026-08-01. Review: no blockers, one major (an operator-doc gap, fixed at close), four minor
- [0039 — Line joins: the stroke stops coming apart at every vertex](done/0039-line-joins.md) — closed 2026-07-28. d Mode 4 review (no blockers; one major, three minors, two nits — nothing fixed in the close commit, the major is deliberately…)
- [0023 — Cross-preset visual transitions: MilkDrop-style dissolves between presets](done/0023-cross-preset-transitions.md) — closed 2026-07-26. Review: no blockers, no majors; five minors, one nit
- [0019 — Preset expression grammar v2: branching, math functions, tempo, typo warnings](done/0019-preset-grammar-v2.md) — closed 2026-07-25. Review: no blockers; one major, three minors, one nit
- [0015 — Preset-directory override + live iteration](done/0015-preset-dir-override-and-live-iteration.md) — closed 2026-07-25. Review: no blockers; one major, three minors, two nits
- [0029 — Attractor resize cost + ink-stage followups](done/0029-attractor-resize-cost-and-ink-followups.md) — closed 2026-07-25. Review: no blockers, no majors; two minors, two nits
- [0027 — Attractor ink-on-paper (engine-wide final tone-remap) + crisp trails](done/0027-attractor-ink-and-crisp-trails.md) — closed 2026-07-25. Plan 0029 or ADR-0030 rather than reworked
- [0025 — Full composite coverage: background + view transform for reaction-diffusion and attractor](done/0025-full-composite-coverage.md) — closed 2026-07-24. Review: no blockers, no majors; one minor, one nit
- [0028 — Parametric-curve shape params: radial offset + phase (audio-morphable rose geometry)](done/0028-parametric-curve-shape-params.md) — closed 2026-07-24. Review: no blockers, no majors; one minor, one nit
- [0020 — Shared palette system: gradient LUT, named + custom palettes, bindable color (all four scenes)](done/0020-shared-palette-system.md) — closed 2026-07-24. Review: no blockers, no majors; two minor, two nits
- [0026 — Calmer scene rotation: hold one scene by default, longer dwell, softened drop bias](done/0026-calmer-scene-rotation.md) — closed 2026-07-24. Review: no blockers, no majors; one minor, one nit
- [0018 — Engine-wide visual enrichment: zoom, atmosphere, easing, mirrors](done/0018-engine-wide-visual-enrichment.md) — closed 2026-07-23. Review: no blockers, no majors; three minor, two nits
- [0024 — Single-source the foobar component version + refresh stale plugin descriptions](done/0024-foobar-component-version-single-source.md) — closed 2026-07-23. passed Mode 4 review cold (no blockers, no majors, no minors — one nit)
- [0021 — Decouple preset content from code: build-time embedding + single-source system names](done/0021-decouple-preset-content-from-code.md) — closed 2026-07-23. Review: no blockers, no majors, no minors, no nits — a clean landing
- [0016 — GPU compute-particle scenes: strange attractors](done/0016-gpu-compute-particle-scenes.md) — closed 2026-07-23. Review: no blockers, no majors; two minor, three nits
- [0022 — Decouple the golden drift guard from shipped presets (per-system frozen fixtures)](done/0022-golden-fixtures-decouple-content.md) — closed 2026-07-23. Review: no blockers, no majors; one minor, one nit
- [0014 — Reaction-diffusion feedback scene + frame-rate-independent render clock](done/0014-reaction-diffusion-feedback-scene.md) — closed 2026-07-23. Review: no blockers, no majors; two minor, two nits
- [0009 — Live performance features (standalone)](done/0009-live-performance-features.md) — closed 2026-07-23. Review: no blockers, no majors; two minor deviations, both pre-flagged and reconciled
- [0017 — Green CI: reasoned ttf-parser advisory ignore + adapter-skip for headless GPU tests](done/0017-ci-green-advisory-and-gpu-tests.md) — closed 2026-07-23. Review: no blockers, no majors, no minors; two non-actionable nits
- [0010 — Line-geometry scenes: parametric curves, L-systems, star patterns](done/0010-line-geometry-scenes.md) — closed 2026-07-23. Review: no blockers, no majors; three minor, two nits
- [0013 — Headless scene capture + differential visual QA + golden images + shot CLI](done/0013-headless-scene-capture.md) — closed 2026-07-22. Review: no blockers, no majors; one minor, one nit
- [0008 — In-app preset browse overlay (standalone)](done/0008-preset-browse-overlay.md) — closed 2026-07-22. Review: no blockers, no majors
- [0012 — Measure the driver-memory floor + cull dead scenes](done/0012-memory-floor-measure-and-scene-cull.md) — closed 2026-07-22. Review: no blockers, no majors
- [0011 — Diagnostics harness + quick-win memory/perf trim](done/0011-diagnostics-and-memory-trim.md) — closed 2026-07-22. Review: no blockers, no majors; two nits
- [0007 — Curated preset library: robust loading + seed-on-first-run + C ABI v2](done/0007-curated-preset-library.md) — closed 2026-07-22. Review: no blockers, no majors
- [0004 — foo_lmv as an embeddable Default UI panel](done/0004-foobar-ui-element-panel.md) — closed 2026-07-21. Review: no blockers, no majors
- [0005 — Extract the lock-free ring into a wgpu-free crate for Miri](done/0005-miri-ring-extraction.md) — closed 2026-07-21. Review: no blockers, no majors
- [0003 — Generative scenes + data-driven presets](done/0003-generative-scenes-and-presets.md) — closed 2026-07-21. Review: no blockers
- [0006 — Versioning: single source of truth + cargo-release + surfacing](done/0006-versioning-wiring.md) — closed 2026-07-21. Review: no blockers, no majors
- [0002 — Rust enforcement tooling](done/0002-rust-enforcement-tooling.md) — closed 2026-07-21. Review: no blockers
- [0001 — Core + standalone MVP, then foobar parity](done/0001-core-and-standalone-mvp.md) — closed 2026-07-21. Review: no blockers; C ABI recorded in ADR-0003

## Roadmap (agreed 2026-07-21, revised same day for the live-show use case; numbers assigned when drafted)

> **2026-07-30: a second, strategic roadmap now exists —
> [docs/roadmap-visual-richness.md](../roadmap-visual-richness.md)** — from the user-requested
> "why is everything dull" architecture review. It diagnoses the five capability caps (single
> quality tier, decay-only feedback, 8-bit additive composite, one-scene/fixed-chain
> composition, starved grammar) and orders the themes R0-R6 that answer them. Item 3's
> remaining half below (quality tiers + governor) is that roadmap's R0. New visual-capability
> plans should cite it.

Execution order after Plan 0001, per the NFR interviews ([docs/nfr.md](../nfr.md)):

1. **Preset / scripting engine** — layered presets per
   [ADR-0002](../adrs/0002-layered-preset-architecture.md): TOML data + expression language
   driving built-in systems (feedback/warp, boids, walkers/growth, 3D scene), with an
   optional budgeted Rhai script for staged per-track arcs (NFR §10). Replaces "scenes are
   Rust code" — Plan 0001's Scene trait becomes the rendering vocabulary presets drive, so
   keep it thin. **Delivered by [Plan 0003](done/0003-generative-scenes-and-presets.md)** (layers
   1-2: fragment-field + swarm systems, data + expression presets); Rhai (layer 3), blending, and
   compute-scale particles remain follow-ups tracked in 0003.
2. **Live performance features** — line-in/audio-interface capture, scene triggers
   (auto-rotate + hotkey/MIDI + experimental track-change detection), fullscreen on a
   chosen display/projector, 4-hour soak stability (NFR §10).
   **Delivered by [Plan 0009](done/0009-live-performance-features.md)** (standalone borderless-
   fullscreen on a chosen display, line-in capture selection, drop-biased scene director +
   hotkeys, spectral track-change novelty nudge on the native `Frame`, `--soak` instrumentation;
   C ABI frozen). **MIDI triggers and the ≥4-hour projector-rig soak run remain** — MIDI is its
   own ADR-backed follow-up; the soak run is a `human` on-device carry-forward.
3. **Adaptive quality + runtime-memory trim** — quality tiers + frame-time governor for the
   60 fps iGPU floor (NFR §1), plus cutting the standalone's ~200 MB working set (NFR §12).
   The memory trim's primary lever — compiling wgpu with only the per-OS backend feature
   (DX12/Metal), dropping the dead Vulkan/GL paths — is a cheap, low-risk win that can
   front-run the full tier system. Both validated on the older iGPU test PC (footprint stated
   before/after; the backend trim must not regress the §1 floor).
   **Front-run by [Plan 0011](done/0011-diagnostics-and-memory-trim.md)** (diagnostics harness +
   the cheap NFR §12 levers, all-three-frontend, C ABI v3 / [ADR-0008](../adrs/0008-c-abi-v3-diagnostics.md)):
   it builds the before/after measuring stick and lands the wgpu-backend + swapchain trims. The
   **adaptive-quality tiers + frame-time governor remain** for a later plan — 0011 explicitly
   does not do them.
4. **Remaining v1 UX** — always-on-top / mini mode, settings persistence (NFR §11;
   fullscreen/multi-monitor land earlier with live features).
5. **Packaging & release** — GitHub release zip: unsigned standalone exe +
   `.fb2k-component` (NFR §8).

Later, unordered: better tempo tracking, preset sharing/library, signed installer.


## Conventions

- **Numbering:** sequential, zero-padded 4 digits. Take the next free number above, then
  bump it here in the same session.
- **Phases:** ordered, each one commit, each tagged `**Owner skill:**` with one value from the
  vocabulary `dev` (all code) or `human` (a task only the user can do). The `dev` skill reads
  this tag at the start of each phase; a missing tag is a Mode 4 review blocker. An optional
  `**Area:**` note (`core` / `standalone` / `plugin`) orients the reader but is not the tag.
- **Skills:** `architect` designs and owns `docs/`; `dev` implements all code. `architect`
  writes and closes plans; `dev` flips `draft → in-progress` at "go" and nothing else in the file.
- **Lifecycle:** `draft` → `approved` (user/architect validated it; ready for `dev`) →
  `in-progress` → `done` (then `git mv` to `done/` and drop from this roster). Review
  happens at plan end, in a fresh `/architect` session — not by the session that wrote
  the code.
