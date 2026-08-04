# Design backlog — captured feedback, not yet promoted

Short, durable notes for design gaps surfaced during work but **not yet** decided into an ADR or
plan. Chiefly the `preset-author → architect` feedback handoff (a look wanting something the
preset grammar or engine can't express), plus any other "worth remembering, not worth acting on
yet" finding.

An entry here is **not** a commitment to build — it is a captured signal so the friction isn't
lost between sessions. Verify every entry against the code before acting on it — these are dated
snapshots, and the surface moves (same rule the lanes apply to their own references).

**The lifecycle, in one line:** raised here → **PROMOTED** (an ADR and/or a plan now exists; the
entry stays in this file, because a design that has not landed is still live) → **CLOSED** (the
plan landed; the entry moves to the archive and leaves a ledger row behind).

## Where the closed entries went

**[`design-backlog-archive.md`](design-backlog-archive.md)**, as of 2026-08-04. This file had
reached 3265 lines and the genuinely open entries were under a fifth of it, so the part anyone
needed to read had become the minority of the document. Nothing was deleted: every closed entry's
body moved across verbatim, and the ledger below indexes them.

**Read the archive rather than the ledger whenever you are about to act on the same surface.** The
bodies are kept for the corrections they carry, not for the outcomes — four entries (0010, 0012,
0014, 0046) had their causal claim *inverted* under verification, and one (0052) was retired
because its premise was false. Those are the most useful pages in the whole record and the ledger
cannot express them.

## Closed entries — the ledger

| # | Entry | Went to |
|---|-------|---------|
| 0001 | `reaction_diffusion` reaches only 2 of the 5 composite levers | [ADR-0026](adrs/0026-full-composite-coverage-fullscreen-scenes.md) + [Plan 0025](plans/done/0025-full-composite-coverage.md) |
| 0002 | No per-bin spectrum: the grammar sees three bands | [ADR-0036](adrs/0036-preset-reachable-spectrum.md) + [Plan 0034](plans/done/0034-preset-reachable-spectrum.md) |
| 0003 | Fixed internal resolutions (RD 256², post stages 720p) | [ADR-0034](adrs/0034-internal-resolution-follows-the-target.md) + [Plan 0033](plans/done/0033-internal-resolution-and-preset-surface.md) |
| 0004 | `zoom`/`pan_*` smear RD's edge: a toroidal sim behind a clamped sampler | [ADR-0034](adrs/0034-internal-resolution-follows-the-target.md) + [Plan 0033](plans/done/0033-internal-resolution-and-preset-surface.md) Phase 5 |
| 0005 | No bloom / glow / halo stage | [ADR-0046](adrs/0046-linear-light-hdr-composite-bloom-tonemap.md) + [Plan 0045](plans/done/0045-linear-light-and-bloom.md) |
| 0006 | `[smoothing]` is a symmetric one-pole: no attack/release split | [ADR-0035](adrs/0035-asymmetric-attack-release-easing.md) + [Plan 0033](plans/done/0033-internal-resolution-and-preset-surface.md) Phase 2 |
| 0007 | `star_pattern` is a hollow ring, and `variant` cannot be blended | Morph half: [ADR-0060](adrs/0060-star-pattern-variants-interpolate.md) + [Plan 0054](plans/done/0054-the-line-scenes-catch-up.md). Interior half: [ADR-0079](adrs/0079-the-mandala-interior-is-rings-of-motifs-inside-star-pattern.md) + [Plan 0065](plans/0065-the-mandala-interior.md) |
| 0008 | `shot` harness gaps that cost the content lane real iterations | [Plan 0033](plans/done/0033-internal-resolution-and-preset-surface.md) Phase 1 + [Plan 0037](plans/done/0037-verifying-easing-transient-probe-and-dynamic-signal.md) Phase 4 |
| 0010 | The fold samples outside its source rectangle and clamps | [ADR-0047](adrs/0047-kaleidoscope-fold-domain-disc-with-falloff.md) + [Plan 0045](plans/done/0045-linear-light-and-bloom.md) |
| 0011 | The fold axis is screen-centred, so `pan_*` and `kaleido_*` fight | [ADR-0047](adrs/0047-kaleidoscope-fold-domain-disc-with-falloff.md) + [Plan 0045](plans/done/0045-linear-light-and-bloom.md) Phase 1 |
| 0012 | `--report`'s `cover` penalises ink presets — **premise was false** | [Plan 0037](plans/done/0037-verifying-easing-transient-probe-and-dynamic-signal.md) Phase 5, as documentation |
| 0013 | No synthetic signal has transients, so easing is unverifiable | [ADR-0039](adrs/0039-verify-easing-with-a-transient-probe-not-a-committed-clip.md) + [Plan 0037](plans/done/0037-verifying-easing-transient-probe-and-dynamic-signal.md) |
| 0014 | The line scenes' cosine `hue` ramp is not a hue wheel | [Plan 0037](plans/done/0037-verifying-easing-transient-probe-and-dynamic-signal.md) Phase 5 — **and the entry's own colour names were wrong** |
| 0015 | The band axis is half linear below the crossover | [ADR-0049](adrs/0049-analysis-v2-dual-resolution-axis-normalized-bands.md) + [Plan 0048](plans/done/0048-analysis-v2-and-the-retune.md) Phase 1 — a second 8192-sample window feeds every band below the crossover. **Closed 2026-08-04 during a backlog sweep; the entry never got its marker** |
| 0016 | The `spectrum` readout has no width control | [Plan 0038](plans/done/0038-line-family-unreachable-levers.md) Phase 2 |
| 0017 | `[spectrum]` has no level curve, and the grammar has no `log` | [ADR-0040](adrs/0040-spectrum-level-curve-applies-before-the-easing.md) + [Plan 0038](plans/done/0038-line-family-unreachable-levers.md) |
| 0018 | `BASELINE_Y` is a constant, so `mirror_reflect` throws the copy up | [Plan 0038](plans/done/0038-line-family-unreachable-levers.md) Phase 2 |
| 0019 | `glow` is unreachable from a preset on all four line scenes | [Plan 0038](plans/done/0038-line-family-unreachable-levers.md) Phase 1 |
| 0020 | The library is gained against stimuli 6-100x hotter than real music | Harness half: [ADR-0042](adrs/0042-reachability-measured-on-the-expression-tree.md) + [Plan 0041](plans/done/0041-report-two-level-stimuli-and-expression-reachability.md). Content half: [Plan 0048](plans/done/0048-analysis-v2-and-the-retune.md) Phase 7's retune (368 gains, 36 thresholds). **Content half closed 2026-08-04 during a backlog sweep** |
| 0022 | `--report`'s reactivity columns are blind to a level `curve` | [ADR-0042](adrs/0042-reachability-measured-on-the-expression-tree.md) + [Plan 0041](plans/done/0041-report-two-level-stimuli-and-expression-reachability.md) |
| 0023 | `LineRenderer` has no line joins, so every vertex leaves a notch | [ADR-0041](adrs/0041-line-joins-are-per-endpoint-on-the-segment-instance.md) + [Plan 0039](plans/done/0039-line-joins.md) |
| 0024 | The star rosette is a closed chain and half its joints are unjoined | [Plan 0040](plans/done/0040-line-joins-finish-the-job.md) Phase 3 |
| 0025 | `swarm` cannot express a flock: no depth, no cohesion | [ADR-0044](adrs/0044-swarm-world-is-a-25d-torus-sized-from-the-target.md) + [Plan 0043](plans/done/0043-swarm-depth-and-domain.md) |
| 0026 | `lsystem` has no per-segment colour | [ADR-0059](adrs/0059-line-scenes-colour-along-their-generator-axis.md) + [Plan 0054](plans/done/0054-the-line-scenes-catch-up.md) |
| 0027 | Two engine behaviours that are correct, non-obvious, undocumented | [Plan 0041](plans/done/0041-report-two-level-stimuli-and-expression-reachability.md), as documentation |
| 0028 | Reachability only reports `select`/`clamp`, so a bare comparison is invisible | [ADR-0043](adrs/0043-reachability-reports-comparison-nodes.md) + [Plan 0042](plans/done/0042-reachability-sees-every-comparison.md) |
| 0029 | The swarm's wrap seam sits on the frame edge, and feedback burns it in | [ADR-0044](adrs/0044-swarm-world-is-a-25d-torus-sized-from-the-target.md) + [Plan 0043](plans/done/0043-swarm-depth-and-domain.md) |
| 0030 | The library binds audio to luminance far more than to geometry | `.claude/skills/preset-author/references/craft.md` §1, which is where the entry asked it to land. **Closed 2026-08-04 during a backlog sweep** |
| 0031 | The Rich tier's 3x particle count makes the reseed transient opaque | [Plan 0057](plans/done/0057-the-attractors-compute-path.md) |
| 0034 | Nothing in the engine spawns, throws, ages or individuates an object | [ADR-0057](adrs/0057-emitter-scene-analytic-ballistics-seeded-individuation.md) + [Plan 0052](plans/done/0052-the-emitter-objects-that-spawn-fall-and-die.md) |
| 0035 | `presets/README.md` listed 10 expression variables; the code had 19 | Fixed at [Plan 0048](plans/done/0048-analysis-v2-and-the-retune.md)'s close |
| 0036 | Does the fold stop folding the backdrop, and does that lose a look? | **Retired unfired 2026-08-04.** [ADR-0055](adrs/0055-backdrop-leaves-the-post-chain.md) shipped 2026-07-31; three full-library content passes have run since and no preset was reported worse. The way back is recorded in the archived body if it ever bites |
| 0037 | The fold covers a disc, and on a field scene that reads worse | [ADR-0061](adrs/0061-kaleidoscope-edge-treatment-is-a-per-preset-choice.md) + [Plan 0055](plans/done/0055-the-fold-edge-becomes-a-choice.md) |
| 0039 | Four bind-group layouts are shared by pipelines live in one frame | [ADR-0058](adrs/0058-bind-group-layout-collisions-carry-evidence.md) + [Plan 0053](plans/0053-the-suite-stops-blessing-what-warp-gets-wrong.md) |
| 0041 | The line seam's lit-backdrop guard discriminates on ~5 pixels | [Plan 0053](plans/0053-the-suite-stops-blessing-what-warp-gets-wrong.md) |
| 0043 | Every reactivity instrument diffs against **silence** | [ADR-0062](adrs/0062-clamp-occupancy-is-the-saturation-instrument.md) + [Plan 0056](plans/done/0056-clamp-occupancy-and-the-axis-anchor.md) |
| 0044 | The axis rebuild silently re-pointed every sub-crossover `bin()` probe | [ADR-0063](adrs/0063-address-the-spectrum-by-frequency.md) + [Plan 0056](plans/done/0056-clamp-occupancy-and-the-axis-anchor.md) |
| 0045 | `docs/analysis-v2-before-flags.md` asks to be deleted | Done 2026-08-03; three inbound links rewritten, not two |
| 0046 | The retune's gain rule is direction-blind — **retracted, the claim was false** | Retracted the same day, before any preset was edited. Kept in full because the *reason* it was wrong is a trap |
| 0047 | `Rich` triples the attractor's light, so the tier is not look-neutral | [ADR-0064](adrs/0064-a-capture-may-pin-the-rich-tier.md) + [ADR-0065](adrs/0065-the-attractor-deposit-is-normalized-by-particle-count.md) + [Plan 0057](plans/done/0057-the-attractors-compute-path.md) |
| 0048 | The `lorenz` family renders as a dust cloud | [ADR-0068](adrs/0068-the-projection-basis-is-a-per-family-property.md) + [Plan 0059](plans/done/0059-lorenz-finds-its-plane.md) |
| 0049 | The fold's residual rays got a second rejection and a shipped instance | Its three facts were carried into [Plan 0055](plans/done/0055-the-fold-edge-becomes-a-choice.md) Phase 2's A/B, which was judged 2026-08-04. **Closed 2026-08-04 during a backlog sweep**; the content rule it asked for is [0058](#0058--thirteen-presets-bind-the-fold-and-eleven-of-them-have-not-chosen-an-edge-treatment-because-until-now-there-was-nothing-to-choose)'s |
| 0050 | The attractor reseed scatters into an axis-aligned box | [ADR-0066](adrs/0066-a-reseed-disturbs-the-cloud-rather-than-replacing-it.md) + [Plan 0057](plans/done/0057-the-attractors-compute-path.md) |
| 0051 | `variant` can morph and neither `star_*` preset does | Closed by content: both presets now drive `variant` with a triangle wave (`star_rosette.toml:59`, `star_lantern.toml:77`). **Closed 2026-08-04 during a backlog sweep** |
| 0052 | `Spectrum Ridge` has no tonal structure — **premise was false** | Retired 2026-08-03; the preset was never flat and the statistic convicted the right preset for the wrong reason |
| 0053 | The retune rescaled band gains but not the world-space params | [ADR-0067](adrs/0067-coverage-measures-the-scene-not-the-backdrop.md) + [Plan 0058](plans/done/0058-the-gate-can-see-an-empty-frame.md) |

## Open entries

## 0009 — the `animation.rs` gate penalizes two legitimate designs (informational)

- **Raised:** 2026-07-26, from `preset-author`, explicitly **not** as an argument that the gate should
  change.
- `core/tests/animation.rs` renders at 96x96 and `ANIM_FLOOR` is a whole-frame diff. Two legitimate
  looks fight it: a **rotationally symmetric** figure (Star Rosette's ring) is nearly invariant under
  rotation, so no amount of spin registers as animation and it must move radially instead; and a
  **thin-stroke** figure nearly vanishes at 96 px, so its motion measures near zero even when it is
  clearly animated at 2048.

Both are real preset-authoring constraints imposed by the **test resolution** rather than by the
look. The failure mode is non-obvious and cost several iterations to diagnose. **Captured so the next
author does not re-diagnose it** — the cheap resolution is a sentence in the authoring docs, not a
change to the gate.

---

## Entry 0021 — from the Plan 0038 / ADR-0040 ruling

Not from the content lane. Raised by an `architect` ruling that had to falsify a claim in order to
answer a `dev` finding, and left a real want with nowhere to live.

---

## 0021 — an "even fall" is not reachable with a one-pole, in any ordering

- **Raised:** 2026-07-28, from
  [ADR-0040's Outcome](adrs/0040-spectrum-level-curve-applies-before-the-easing.md#outcome-2026-07-28-after-plan-0038-phase-3s-measurement).
  ADR-0040 chose the spectrum level curve's position in the pipeline partly to buy "a perceptually
  even fall". Plan 0038 Phase 3 measured it, and the closed form settles it: **no ordering can deliver
  that**, because every `[smoothing]` response in this engine is a one-pole exponential and a power of
  an exponential is an exponential.
- **Verified against code:** yes. `Easing::step` (`core/src/preset/schema.rs:223`) is
  `held + (1 - exp(-dt/tau)) * (raw - held)`, one constant per direction (ADR-0035), no shape.
  `Smoother::smooth` (`core/src/render/mod.rs:317`) is the same arithmetic for bindings, and the
  spectrum scene's per-element easing calls the same method.

**The measurement, for the record.** An exponential spends **30 %** of its settling time covering the
first half of its travel (`ln2 / ln10` = 0.301); a linear ramp spends **56 %**. Both curve orderings
measure 0.301 when measured to settlement. So "even" is a ~1.8x gap from what the engine can currently
produce, in either ordering, at any exponent.

**The want is legitimate and has been asked for twice.** This is the half of
[0006](design-backlog-archive.md#0006--smoothing-is-a-one-pole-low-pass-no-attackrelease-split-no-s-curve) that
[ADR-0035](adrs/0035-asymmetric-attack-release-easing.md) deliberately did not take — 0006's origin
ask was literally "use some qubic bezziere function or something", and the asymmetric one-pole
answered the *symmetry* half of that defect while leaving the *shape* half untouched. A meter that
falls at a constant rate is the classic look this cannot make.

**The cheap shape, if it is wanted:** a **rate-limited (slew) release** rather than a curve —
`held += clamp(raw - held, -rate * dt, +rate * dt)` — which is a third `[smoothing]` form beside
today's scalar and `{ attack, release }`, needs **no** new per-binding state (the slot exists), stays
stateless from the author's side, and is frame-rate-independent for the same reason the one-pole is
(ADR-0019's injected real `dt`). A constant-rate fall is exactly evenness 0.556. The nameable rejected
alternative is a full parametric ease curve, which needs a notion of "a transition in progress" and a
rule for a target that moves mid-ease — the same reason it lost in 0006.

**Not the thing ADR-0035 already rejected.** Its Alternative C was a `slew(x, up, down)` **function in
the grammar**, refused outright because expressions are pure and stateless by hard invariant. The
proposal here is the opposite location: a `[smoothing]`-table *form*, where the state already lives and
where the asymmetric one-pole itself landed. That distinction is the whole reason this is a fresh entry
rather than a re-litigation, and any ADR must say so explicitly or it will read as reopening 0035.

**ADR-worthy** as a short supplement to [ADR-0019](adrs/0019-eased-parameters.md) /
[ADR-0035](adrs/0035-asymmetric-attack-release-easing.md) if acted on. **Not urgent**: nothing shipped
is broken, and unlike most entries here this one is a *new capability* rather than a wall the content
lane has already hit. It wants a preset-author "I want this look and cannot get it" before it wants a
plan — the evidence so far is an architect's arithmetic, not a frustrated author.

---

## Entry 0032 — from the Plan 0049 Phase 5 sample-rate sweep

---

## 0032 — both analysis windows are sized in **samples**, so a third of the band axis loses resolution at 96 kHz

- **Raised:** 2026-07-30, by `dev` implementing Plan 0049 Phase 5 item 3 (the sample-rate coverage
  gap Plan 0048's Mode 4 review named), and confirmed at that plan's close review.
- **Verified against code:** yes — measured, and the measurement is pinned by
  `core/src/dsp/fft.rs::the_axis_holds_at_the_rates_we_do_not_develop_at`.

Every band-layout test was at 48 kHz until Plan 0049. `AudioFormat` accepts 8 kHz-384 kHz, and
WASAPI loopback runs at whatever the device mix format is — 96 kHz is an ordinary setting on a
discrete DAC or an audio interface. The sweep measured:

| rate | crossover band | crossover | bin-starved bands |
|------|----------------|-----------|-------------------|
| 44.1 kHz | 19 | ~223 Hz | 8 |
| 48 kHz | 20 | ~246 Hz | 8 |
| 96 kHz | 27 | ~487 Hz | **21** |

**44.1 kHz found nothing, which is the good outcome** — one band lower, same starved count — and
that is the rate foobar hands the plugin for CD material, so the plugin path is unaffected.

**The mechanism at 96 kHz.** `WINDOW_SIZE` and the long window are both fixed in **samples**, not
seconds, so at twice the rate each spans half the time and resolves half the frequency detail. The
crossover rides `sample_rate / WINDOW_SIZE`, and the region the long window still cannot resolve
grows from 8 bands to 21 — a third of the axis — because the widening cascades through `fill`'s
`prev_hi` chain, each widened band pushing the next one's floor up.

**This is not an ADR-0049 regression.** That ADR's claim is about band **edges in Hz** below the
crossover, and those do not move at any rate. It is physics working as specified: a higher sample
rate buys time resolution and spends frequency resolution. But a third of the axis reading at
one-bin resolution is a real difference in what a preset's `bin()` sees on a 96 kHz device, and it
was invisible before the test existed.

### What a fix would be

Size both windows in **seconds** rather than samples, so the analysis time-span — and therefore the
frequency resolution behind every band — is the same on every device. That has real consequences to
weigh (a rate-dependent FFT size, its cost at 192/384 kHz, whether `HOP_SIZE` follows, and what it
does to the onset envelope's cadence and to `docs/nfr.md`'s window budget), and it re-opens a
decision ADR-0049 made. **So it is ADR territory, not a patch** — which is exactly why Plan 0049
recorded it rather than acting on it.

### Priority

Low and honest about it. Nobody has reported it, the two rates that dominate (44.1 / 48 kHz) are
clean, and the failure is a coarser low end rather than anything broken. Worth taking the day
someone runs the standalone on a 96 kHz interface and says the sub-bass reads mushy — at which
point this entry is the starting measurement rather than a fresh investigation.

---

## Entries 0033-0035 — the 2026-07-30 `preset-author` batch (fifth), from two figurative requests

The user asked for two looks that are **figurative** rather than generative, which is a class this
library has never been asked for before:

1. the Windows Solitaire win-cascade, with **hearts** instead of cards — red fill, black outline,
   falling at a rate set by the BPM, arcing off in different directions and leaving a trail of
   stamped copies;
2. **small seven-, eight- and nine-pointed stars**, white-gold on black, twinkling and flashing on
   bass and beat.

Both were rendered as far as the current surface reaches before being reported. The two requests
are independent in the user's mind and turned out to share **exactly one** root gap, which is why
they are filed together. Neither look shipped; the drafts were discarded on the user's instruction.

**What was already sufficient, and should be said first:** the *audio* half of "falls at a speed
that depends on the BPM, a new one on every beat" needs nothing. `tempo` is BPM, and ADR-0050's
clock — `beat_index`, `time_since_beat`, `beat_in_bar`, `bar_index`, `bar_phase` — supplies both the
per-beat event and the phase to drive an arc from. The gap in both requests is entirely in **what
can be drawn**, not in what can be heard.

---

## 0033 — every mark the engine can draw is a round additive blob or a stroked curve, so no *object* has a shape

- **PROMOTED 2026-08-04 (the silhouette half only) → [ADR-0084](adrs/0084-a-particle-marks-silhouette-is-a-signed-distance-function.md) +
  [Plan 0070](plans/0070-shaped-marks.md)** — a `shape` param selecting a signed-distance function
  in the existing particle fragment shader, on `swarm` and `emitter`, keeping the additive model and
  the quadratic falloff. The user chose the SDF route over a fill-and-stroke path, a glyph atlas and
  author-supplied WGSL. **The fill-and-outline half of this entry stays open and is not promoted** —
  a heart in additive light is a heart-shaped glow, and the red-body-black-edge ask still reopens
  ADR-0018/ADR-0056. Re-file that half as its own entry when Plan 0070 lands, so the two stop being
  confused.
- **Raised:** 2026-07-30, from `preset-author`, by both requests above independently.
- **Verified against code:** yes — `core/src/render/scenes/swarm.rs` (`fs_main`),
  `core/src/render/scenes/particles/mod.rs`, `core/src/render/scenes/lines/*`,
  `core/src/render/ink.rs`.

The engine has **no shape vocabulary for an object**. There are exactly two mark-making models:

- **Particles are one hardcoded round blob.** The swarm's fragment shader is three lines —
  `let d = length(in.local); let falloff = max(0.0, 1.0 - d); let g = falloff * falloff;` — a radial
  falloff with no shape input at all. The attractor's compute points are the same idea. There is no
  glyph atlas, no SDF, no shape parameter, and nothing in `PARAMS` that could carry one.
- **Line scenes stroke a generator's path.** `maurer_rose`, the L-system turtle, the Hankin
  rosette, the spectrum comb. These *can* make a shape, but only one figure, centred, whole-frame,
  and only as a **stroke** — there is no fill.

**The second half is worse than the first: the pipeline is additive, so a dark mark cannot exist.**
Every scene blends additively (`swarm.rs`: *"Additive: overlapping particles bloom brighter"*), which
is a lightening model — black adds zero. A red-filled heart with a black outline is **three** tones
(light ground, red body, black edge) and the only dark-on-light route in the engine is the ink stage,
which is structurally **two**-poled: `mix(paper, ink, luminance)`.

**Measured, not assumed.** I drew the cardioid `r = 1 - sin(theta)` through `parametric_curve`
(`n = 1`, `phase = -pi`, `radial_offset = 1`) — a genuinely recognisable heart, and a useful data
point that the *outline* is reachable today. Running the same figure at `ink_amount = 1` with white
paper and black ink rendered the outline **grey, not black**: a thin anti-aliased stroke averages to
mid luminance, so it lands halfway down the paper→ink ramp. The ink stage cannot produce a crisp dark
contour around a light interior, because the contour is not where the luminance is.

**The star half lands on the same gap.** Small 7/8/9-pointed stars scattered across the frame are not
reachable: the swarm can put ten thousand small marks anywhere, but they are round; `parametric_curve`
with `radial_offset = 1` gives exactly `n` lobes and can flip the count every beat
(`n = "7 + floor(hash(beat_index) * 2.999)"`, which works and is rather nice) — but it is **one large
centred figure**, and `mirror_order` replicates about the origin, so the copies land on each other
rather than scattering. Rendered both; the starfield reads well as a starfield and not at all as
*stars with points*.

**Why this is not a preset problem.** There is no combination of existing params that gets closer.
Whatever the answer is — a shape enum on the particle sprite, an SDF glyph, an author-supplied WGSL
pass (already noted as absent in the skill's own gap list), a fill+stroke draw path outside the
additive model — it is a change to how the engine draws, and the fill/outline half re-opens the
additive-blending decision that everything else in the composite assumes.

**Impact.** First time it has been asked for, but it is not an exotic ask: "a shower of *things*" is
a mainstream visualizer idiom, and the request arrived twice in one session from one user. It is also
the gap that most limits what this lane can offer, because the library is entirely non-figurative and
nothing in the grammar hints that figurative is off the table.

**Not deciding:** whether the engine *should* draw figurative objects at all, or which of the four
routes above is right. Both are architect calls, and the additive-model question is ADR-shaped.

---

## 0038 — mid-tone-dominated presets lost ~8 % luminance to the tonemap knee, and the library has not been retuned

- **Raised:** 2026-07-31, from `architect`, at Plan 0045's Mode 4 review.
- **Verified against code:** yes — measured, not inferred (numbers below).
- **For:** `preset-author`. This is genuinely content-lane work; the engine behaved as designed.
- **ROUTED 2026-08-01 → `preset-author`, as a content pass rather than a plan.** The user's
  call at the Plan 0051 close: this needs no engine change and no ADR, so it goes to the lane
  directly. It pairs naturally with [0040](#0040) — both are retunes of the same shipped set
  against a composite whose behaviour has changed under them.

The user's report was "clifford is really dim". Rendering `attractor_clifford` at an identical
stimulus on `main` and on the Plan 0045 branch (640x360, 90 frames, hardware adapter):

| preset | main | branch | |
|---|---|---|---|
| `attractor_clifford` | mean luma 82.54 | 75.91 | **-8.0 %** |
| `attractor_leviathan` | mean luma 63.98 | 67.70 | **+5.8 %** |

That is not drift. It is the tonemap knee's documented price, to the decimal: `tonemap.rs`'s
`KNEE` docstring says a linear 0.8 mid-tone now presents at 0.733, which is -8.4 %. **The split is
the whole story.** Clifford is a diffuse particle cloud living almost entirely in the mid range, so
it pays the knee and collects none of the headroom above 1.0. Leviathan has genuinely over-range
cores, so it gains. Plan 0045 chose to pay this on mid-tones rather than on highlights, deliberately
and in writing — the consequence is simply that every preset shaped like Clifford now reads dimmer.

**The lever already exists and is one line:** `exposure` (default 1.0) is a linear multiplier ahead
of the tonemap, added by this same plan for exactly this. `exposure = "1.1"` restores Clifford's
level without re-balancing a single element against its own background, which is what raising
per-element `brightness` would force. The population to check is presets with no over-range peak —
the attractor family, the softer `fragment_*`, `swarm_drift`.

**A related record correction, since this is the entry about the luminance model.** This file's own
`0034` section (the "why it works, mechanically" passage under the Supernova table, around line
1561) still says "the frame clips per channel" in the present tense, and reasons from it. That
premise retired with Plan 0045. The *conclusion* stands and is if anything stronger — geometry
still has somewhere to go when luminance does not — but the mechanism is now a roll-off, not a
clip. Per this file's append-only rule the passage is left standing; this paragraph is the
correction.

---

## Entries 0040-0041 — from the Plan 0051 Mode 4 review (2026-08-01)

---

## 0040 — additive light occludes by geometry, so a dim figure over a lit backdrop reads as dark speckle

- **PROMOTED 2026-08-04 → [ADR-0085](adrs/0085-how-much-a-scene-occludes-the-backdrop-is-one-number.md) +
  [Plan 0071](plans/0071-light-that-adds-without-covering.md)** — `occlude`, a bindable scalar at
  the backdrop composite, so the resolve is `scene + bg * (1 - alpha * occlude)`. The entry's
  "per-preset opt-in" shape was taken, as a **continuous** scalar rather than the two-valued enum it
  suggested: the scalar contains both endpoints at identical cost and avoids a fourth quantization
  seam. Default `1.0` is byte-identical to today; whether it stays there is Phase 3's `human` call
  from a rendered sample set over a **lit** backdrop, per this entry's own instruction that the
  question wants samples rather than an argument.
- **Raised:** 2026-08-01, from `architect`, at Plan 0051's Mode 4 review.
- **Verified against code:** yes — rendered. `swarm_storm` over `bg_bright = 0.35` at
  `brightness = 0.02` renders as black specks on the backdrop; at the shipped-value backdrop the
  same run's darkest pixel is (71,13,22) against a backdrop of (138,67,56).
- **For:** `architect` (a look decision, ADR-worthy if taken), informed by `preset-author`.

[ADR-0056](adrs/0056-additive-scenes-emit-premultiplied-alpha.md) made a scene emit alpha equal to
its **coverage**, which fixed the black notches and rims. Its last Negative bullet left one thing
open: coverage-as-alpha means a fragment occludes the backdrop **whatever light it emits**. The
resolve is `c * g + bg * (1 - g)`, so a fragment darkens the backdrop wherever `c < bg`.

At the shipped near-black floors this is unobservable — all sixteen affected presets sit between
`bg_bright` 0.009 and 0.070. It matters because **the fix invites raising them**: the black rim was
the reason the swarm and line families were floored (`lsystem_fern.toml:98-103` records the
symptom, misattributing it to the lifted floor washing out the additive halo — a real effect, but
the rim was contributing and the tradeoff has changed). An author acting on that invitation meets a
new ceiling: `bg_bright` can rise only to the **dimmest emitted luminance in the figure**. Past it,
the depth-parallaxed far particles and the `glow`-dimmed strokes stop fading out and start reading
as dark speckle. `presets/README.md` now states this.

**The open question is whether that is the right model.** An additive look arguably wants *no*
occlusion — light adds, it does not cover — which would mean deriving the seam's alpha from
something other than pure geometric coverage, or giving the scene a bindable choice between the two
semantics. ADR-0056 rejected deriving coverage centrally from luminance (a legitimately dark
covered pixel would go transparent, and it puts scene judgement in a shared pass); a per-scene or
per-preset *opt-in* is a different proposal and is not covered by that rejection. Nothing is broken
today — post-fix is brighter than pre-fix at every pixel — so this is a look decision, not a defect.
Settling it wants rendered samples of the same preset under both semantics at a raised backdrop,
per the concrete-examples workflow, not an argument.

---

## 0042 — the downbeat estimator locks on ~3 % of audible time, so the gated bar variables are almost always fallback

- **PROMOTED 2026-08-04 → [ADR-0082](adrs/0082-the-downbeat-gate-holds-and-the-estimator-is-diagnosed-first.md) +
  [Plan 0068](plans/0068-why-the-downbeat-rarely-locks.md)** — as a **diagnosis**, not a fix. The
  ADR records the one thing this entry insists on: `CONFIDENCE_THRESHOLD` does not move to buy lock
  rate, because adjusting a safety gate using data collected while the gate was closed is circular.
  The plan builds the decomposition the 1 Hz column cannot give (four alignment scores, raw and
  corrected effect size), degrades a known-good pattern along three axes to find which term
  collapses first, and ends with a named cause. The repair is a follow-on plan written against the
  diagnosis.
- **Raised:** 2026-08-02, from `architect`, running [Plan 0048](plans/done/0048-analysis-v2-and-the-retune.md)
  Phase 6 (`human`) with the user.
- **Measured, not impressionistic:** 8.8 minutes through the live app on the `v0.28.1` release
  build, 517 log rows at 1 Hz, **458 with signal**, roughly half beat-driven 4/4 (the Plan 0037
  Phase 4 trap/808 material) and half sparse.

`downbeat_locked` was true in **14 of 458 audible rows — 3.1 %**, which over the beat-driven half
is roughly **6 %**. `downbeat_confidence` sat at **mean 0.030, median 0.000** against
`CONFIDENCE_THRESHOLD = 0.25` (`core/src/dsp/downbeat.rs:55`), clearing the gate in **two of
eighteen** 30-second windows and peaking at **0.516** — twice the gate. So the estimator is
capable of locking and rarely does.

**This is a shortfall, not a defect, and the distinction is load-bearing.**
[ADR-0050](adrs/0050-downbeat-and-phrase-tracking-with-confidence-fallback.md) designed the gate so
that failing to lock degrades to the counters-only option the interview declined — the safe floor,
working exactly as specified. Nothing is broken. What is true is that `beat_in_bar`, `bar_index` and
`bar_phase` were counter-derived for essentially the whole session, so a preset binding them today
is binding the fallback.

**The stopping condition did not fire, and the record should not be read as it passing.** No
confidently-wrong bar line was observed — but with the gate shut 97 % of the time there was little
opportunity for one. The mis-accent question is *untested*, not *answered*.

**Do not read this as an argument for lowering `CONFIDENCE_THRESHOLD`.** That is the one change the
measurement must not be taken to recommend: ADR-0050 exists because a confidently wrong beat 1 is
the failure an author cannot work around, and buying lock rate with the gate inverts the trade the
ADR was written to make. If the gate moves at all it moves *after* the estimator improves, not
instead.

**What a design here would weigh** (ADR-0050 supplement territory, and it wants an interview):

- **Improve the accent model.** The estimator folds accents into a 4/4 hypothesis; whether the
  weakness is the accent feature, the fold, or the confidence measure itself is unknown and is the
  first thing to find out. Nothing here has been diagnosed — only the outcome measured.
- **Re-price the confidence measure without moving the gate.** If confidence is systematically
  under-reading a correct alignment, the fix is the measure, not the threshold, and the gate keeps
  its meaning.
- **Accept it and say so in the authoring docs.** Cheapest, and honest: layer 1 (`beat_index`,
  `time_since_beat`) is unconditional and reliable; layer 2 is decorative until further notice.
  `presets/README.md` currently offers both without distinguishing their availability.

**Blocks nothing, qualifies one thing.** Plan 0048 Phase 7's retune should lean on layer 1 and treat
layer 2 as decorative — recorded in that plan's Phase 6 results. **Do not re-measure by ear**: the
1 Hz `downbeat_locked` column is the instrument, and a targeted pass on known-4/4 material only
would sharpen the 6 % figure the half-and-half split leaves approximate.

---

## Entry 0054 — from the Plan 0058 close (2026-08-03), and the plan's own measurement is the argument

---

## 0054 — pixel coverage cannot see a figure whose *tips* leave the frame, and an in-frame geometry fraction is the successor

- **PROMOTED 2026-08-04 → [ADR-0083](adrs/0083-in-frame-geometry-is-measured-at-the-line-renderers-draw-seam.md) +
  [Plan 0069](plans/0069-the-instrument-that-sees-a-figure-leave-the-frame.md)** — and the "where
  does it live" question this entry names as the real decision was answered by **none of its three
  options**. It is measured inside `LineRenderer::draw`, which already receives every endpoint *and*
  the render target's aspect, so all four line families are covered by one implementation with no
  `Scene` accessor (ADR-0067's stated objection) and no harness re-derivation of generator math. The
  entry's per-family limit stands and is documented rather than papered over.
- **Raised:** 2026-08-03, at [Plan 0058](plans/done/0058-the-gate-can-see-an-empty-frame.md)'s
  close. This is not a fresh idea —
  [ADR-0067](adrs/0067-coverage-measures-the-scene-not-the-backdrop.md) named it in Alternatives,
  rejected it as the *primary* mechanism and kept it explicitly as the supplement. What is new is
  the evidence that it is now wanted, and the evidence is a measurement rather than an argument.
- **Verified against code:** yes — measured through the instrument Plan 0058 Phase 3 built, printed
  by `core/tests/sanity.rs` on every run.
- **Lane:** `architect` → `dev`. Engine/harness work, no preset content.

**What Plan 0058 established, and what it could not.** Phase 1 made `sanity` measure the scene
against black instead of against a sampled corner pixel, which catches the **total** case — a figure
so far out of frame that nothing is drawn. That is real and it is pinned by a frozen fixture. Phase 3
then tried to catch the **partial** case with a stimulus-relative check: capture at two excitations
and assert the louder frame does not draw less picture. It ships as a **report, not a gate**, and
the numbers are why:

```text
 ratio   cov@0.4  cov@1.0  preset
 0.8552   0.2878   0.2461  De Jong          <- lowest legitimate (correct content)
 0.9568   0.3164   0.3027  Leviathan        <- correct content
 1.0514   0.3866   0.4065  Spectrum Corona  <- OVER-SCALED, scale = 5.20
 1.0891   0.5088   0.5541  Spectrum Comb    <- OVER-SCALED, scale = 3.80
    inf   0.0000   0.0000  Spectrum Ridge (pre-repair)  <- 0/0, no denominator
```

**No threshold on this axis convicts anything it was built for.** The two over-scaled presets score
*above* 1.0 — they draw more when loud — because a comb roots every bar on a shared baseline and a
corona roots every spoke at a centre, so clipping the tips costs a rounding error of lit pixels
while the body stays exactly where it was. Meanwhile the only content anywhere near a plausible
threshold is correct: the attractor family's *peak buys structure* idiom, which
[ADR-0062](adrs/0062-clamp-occupancy-is-the-saturation-instrument.md) already records as real. A
gate at `0.80` would sit `0.055` from De Jong while catching none of the three known-defective
configurations.

**So the diagnosis is that the measure is wrong, not the threshold.** Tips are almost no pixels.
Asking a pixel-coverage statistic about a figure that overshoots its frame is asking the wrong
question, and no calibration of it will help.

**What a design here would weigh.**

- **The obvious mechanism, and its reach.** Line and spectrum scenes build a CPU segment list, so
  "what share of the drawn geometry lands inside the render target" is computable without a GPU
  readback for exactly the families most exposed. That is also its limit: `fragment_field`,
  `reaction_diffusion` and the attractor draw no such list, so this cannot be an engine-wide gate —
  it is a per-family instrument, which is a shape this project has not built before.
- **Where it lives.** ADR-0067 declined it partly because it "needs a `Scene`-adjacent accessor",
  and widening the `Scene` trait is ADR-0002 territory. Whether the fraction is computed inside the
  scene, exposed through a diagnostic seam, or derived by the harness from the same generator config
  a preset declares is the real decision, with rejected alternatives.
- **The confirmation half already works and is worth keeping either way.** Repaired, the same ratio
  moved `1.0891 -> 1.7196` (comb) and `1.0514 -> 1.6756` (corona). The check is blind as a
  conviction and sharp as a confirmation — useful to a content pass verifying its own repair even
  though it can never fail the build.
- **Non-vacuity is already available.** `core/tests/sanity.rs` carries `pre_repair_spectrum_ridge`
  as a frozen fixture, and `git show 2efb80e^:presets/spectrum_comb.toml` is the partial case. Any
  instrument proposed here can be tested against both before it is trusted.

---

## 0055 — the attractor's shape vocabulary is "breathe and bend", and the reference figures ask for more

- **Raised:** 2026-08-04, by the user, watching the attractor family in the app after Plan 0059
  Phases 1/1b/2/3 landed. Reference: a Google Images sweep for **`de jong strange attractor`**
  (the query, not the session URL — the URL carries per-session tokens and will not resolve later).
- **Verified against code:** yes — `core/src/render/scenes/particles/mod.rs`, and the four
  coefficient bindings in each of the six `presets/attractor_*.toml`.
- **Not a defect.** Everything below already works as designed. This is a capability question.

**What exists today.** The attractor's shape *is* programmatically drivable: `a`/`b`/`c`/`d` are
named bindable params carrying the family's coefficients, and all six shipped presets already
steer them — slow incommensurate sines for drift plus clamped band terms on top. Plan 0059 added
two more shape levers beside them: `[particles] density` (how many trajectories) and, on the
continuous families, the `prev -> pos` segment that turns a sparse cloud into legible curves.

**Where it stops.** These are **chaotic** maps, and `presets/README.md` already tells authors to
move the coefficients "slowly and by a little" for a concrete reason: a nearby coefficient can have
a *completely different* attractor, so past a small step the figure **cuts** rather than morphs.
The vocabulary is therefore "breathe and bend around one figure", and the reference images the user
is comparing against are a *gallery of different figures* — De Jong at widely separated coefficient
tuples, each its own shape. Nothing in the current surface walks between them without cutting.

**What a design here would weigh.**

- **Is the ask variety-over-time or morph-between-figures?** They are different features. Variety
  could be had by re-seeding to a new coefficient tuple on a section change and accepting the cut
  (cheap, no engine work, possibly hidden by the existing dissolve). A genuine *morph* needs a path
  through coefficient space along which the attractor stays recognisable, which is a real research
  question and may not exist in general.
- **A curated tuple roster is the cheap middle.** The reference galleries are, in effect, a list of
  known-good `(a,b,c,d)` per family. A preset-facing roster — pick tuple N, or walk a named path
  between two — buys most of the visual variety without solving continuous morphing. It also has a
  clear rejected alternative (free coefficient binding, which is what exists and cuts), so it is
  ADR-shaped.
- **Cross-fading two attractors is the other shape**, and it is expensive in the way this engine
  already understands: two particle buffers and two dispatches, against a scene that is already the
  heaviest in the library. ADR-0024's cross-preset transition may make this unnecessary — a
  dissolve between two attractor *presets* is a morph between two figures, at zero new engine cost.
  Worth measuring before building anything.
- **Watch the interaction with `density` and the streak.** Both landed in Plan 0059 and neither has
  had a content pass yet (Phase 4). The reference look the user is pointing at is *sparse curves*,
  which is exactly what those two levers exist to reach — so some of this gap may close in Phase 4
  without any new capability. **Re-check this entry after Phase 4 before designing against it.**

---

## Entry 0056 — from the Plan 0050 close (2026-08-04), found while clearing a stale preset cache

---

## 0056 — a user-authored preset has been living outside the repo for six weeks, and it is a curation candidate the boundary has no route for

- **PROMOTED 2026-08-04 → [ADR-0081](adrs/0081-the-content-lane-lands-presets-and-architect-curates-the-set.md) +
  [Plan 0067](plans/0067-the-curation-route.md)** — both halves the entry asks for. The owed
  ADR-0017 supplement is written and the boundary moves: **`preset-author` lands presets directly,
  `architect` curates the set**, with the curation pass hooked to the close of any plan that touched
  `presets/` (a standing cadence was rejected on this project's own evidence — the version bump).
  The plan strengthens the gate that authorization rests on *before* walking this file through it,
  because all five preset gates synthesize `AnalysisFrame` and none runs the analyzer. Phase 3 is
  the Coral Oracle pass, and declining it is a successful outcome.
- **Raised:** 2026-08-04, by `architect`, checking whether a `%APPDATA%` preset cache was safe to
  delete before clearing it at the user's request.
- **Verified against code:** partly — the two defects below are read off the file against today's
  grammar and are **not** rendered. Render it before acting.
- **Not a defect in the engine.** This is a content-lifecycle gap plus two rot findings in one file.

**What was found.** `%APPDATA%\light-music-visualizer\presets\chthonic_coral_oracle.toml` — "Chthonic
Coral Oracle", a reaction-diffusion preset — has **never been tracked in git**. It survived the cache
clear deliberately; the other 43 files were retired or stale shipped copies, all recoverable from
`c11bbf9` / `de707cb`. It is the only non-shipped preset in the user's library.

**It is not a stray.** This is the preset that raised [backlog 0001](design-backlog-archive.md#0001--reaction_diffusion-reaches-only-2-of-the-5-plan-0018-composite-levers)
on 2026-07-24 — the entry that became [ADR-0026](adrs/0026-full-composite-coverage-fullscreen-scenes.md)
and [Plan 0025](plans/done/0025-full-composite-coverage.md), i.e. the reason `reaction_diffusion`
reaches the composite levers at all. The preset that motivated a whole plan then **never came back
into the repo**, and the levers it asked for landed without the look that asked for them ever being
shipped or re-checked.

**Why it is a candidate.** It composes four things into one coherent idea — Pearson-regime drift on
bass, beat-stamped `inject` growth, trails, and a bass-breathing kaleidoscopic fold — and documents
its own drive map in the header more thoroughly than several shipped presets do. Nothing in the
shipped set does regime drift.

**Two rot findings, both from the file rather than from a render.**

- **`bar` is no longer a variable.** `kaleido_angle = "time * 0.06 + bar * 0.5"` was written against
  the pre-[0048] grammar; since [ADR-0050](adrs/0050-downbeat-and-phrase-tracking-with-confidence-fallback.md) the trio is
  `bar_index` / `bar_phase` / `beat_in_bar`. Expect a load-time unknown-name **warning** and a dead
  term, not a failure. Note also what [0048] Phase 6 measured — the downbeat estimator locks on
  **3.1 %** of audible time — so whatever this term becomes should be built on `beat_index` /
  `time_since_beat` rather than on the bar trio.
- **`kaleido_order` is eased under `[smoothing]` at `tau = 2.0`.** That is the seam
  `presets/README.md` records: an eased param is continuous even where its math needs an integer, so
  a breathing wedge count sweeps through invalid values between them. Whether it reads as jitter or
  as intended breathing is a **render** question, and this entry does not answer it.

**What is actually being asked.** Not "ship this file". Two things:

1. **A `preset-author` pass** to refresh it against the v2 grammar, judge it in motion, and decide
   whether it earns a place — including whether the regime-drift idea is better carried by a new
   preset than by restoring this one.
2. **The route itself, which does not exist.** [ADR-0017](adrs/0017-preset-author-skill-lane.md) put
   curation at "`dev` embeds a curated preset" because embedding meant editing Rust in two coupled
   spots. [ADR-0022](adrs/0022-build-time-preset-embedding.md) removed that premise — `core/build.rs`
   globs `presets/*.toml`, so shipping a preset is committing a `.toml`. The architect skill already
   names this as an **owed ADR-0017 supplement**; this is the first concrete case waiting on it, and
   it is a good one to decide against, because the file is a real candidate with real rot rather
   than a hypothetical.

**Do not fold this into Plan 0059 Phase 4.** That pass is the attractor family, judged against a
figure that just changed shape; this is one reaction-diffusion preset and an unrelated boundary
question. They share only the lane.

---

## Entry 0057 — from the Plan 0059 Phase 4 content pass (2026-08-04)

---

## 0057 — a preset has no scene-local way to set a figure's level, so `exposure` gets used for it and two other stages disagree with that use

- **PROMOTED 2026-08-04 → [ADR-0080](adrs/0080-the-attractor-owns-its-level-and-bloom-thresholds-exposed-light.md) +
  [Plan 0066](plans/0066-the-level-lever.md)** — both halves, at the user's call. The attractor gains
  `brightness`, matching the param `swarm` and `emitter` already carry (it is the **only** particle
  family without one, which is why its two presets reached for `exposure`), and the bloom
  bright-pass thresholds **post-exposure** luminance. **The pixel cost turned out to be nil on the
  golden suite and the arithmetic is why:** no fixture binds `exposure`
  (`grep -l exposure core/tests/fixtures/*.toml` is empty across all 23), so the new factor is
  literal `1.0` and every baseline is byte-identical. The only looks that move are Lorenz and
  Thomas, which Phase 5 retunes because their headers document the retired model.
- **Raised:** 2026-08-04, from `preset-author` (Plan [0059](plans/done/0059-lorenz-finds-its-plane.md)
  Phase 4, `990fedc`). All three findings verified against code, with rendered evidence noted.
- **One entry, not three.** These are one gap seen from three sides: there is no per-scene deposit
  or intensity param, so a figure's level is spent on `exposure` — the one lever that is
  engine-wide, interpolated across a dissolve, and measured *after* the stage that would want to
  discriminate on it.
- **Why it has no history:** `attractor_lorenz` and `attractor_thomas` are the **first two shipped
  presets to bind `exposure` at all.** Nothing had a caller before, so nothing had a complaint.

**1. `density` is exposure-neutral in total light only (ADR-0065), and the docs said otherwise.**
Per texel it is not neutral: the same energy lands on `1/N` of the pixels, so a sparse preset needs
a cut on the order of `trail frames / density`. The shipped values are `exposure = 0.03` on Lorenz at
`density = 0.002` and `0.10` on Thomas at `0.02`, both picked off rendered ladders rather than
derived. `presets/README.md`'s `[particles]` section told authors they could re-aim `density`
*"without re-tuning `size`, `fade` or `exposure`"* — **true of the sum, false of the picture.**
*Corrected at this close* rather than left for the entry's promotion, because it is wrong today and
the next sparse preset would be misled by it.

**2. The ADR-shaped half: should a scene have a local deposit / intensity param?** `exposure` is
engine-wide and **crossfades across a preset dissolve** (`crossfade_from` in
`core/src/render/tonemap.rs`, ADR-0032's seam), so an extreme per-preset value drags the ~1 s blend
through a badly-exposed frame. Both new presets deliberately buy as much of their level as possible
with `size` and `fade` first *because of this* — those are scene-local and blend as pixels. That is
a workaround with a ceiling, and the question it poses is a real tradeoff with alternatives to
reject: a per-scene param, versus normalizing `exposure` per-preset at the crossfade, versus
declaring the current behaviour correct and documenting the workaround as the technique.

**3. `bloom_threshold` is measured in pre-exposure units, so at these values it cannot discriminate
at all.** Chain order is scene → post chain → tonemap, so the bright-pass reads the figure *before*
`exposure` scales it, and `bloom.rs` clamps the threshold at `MAX_THRESHOLD = 8.0`. At
`exposure = 0.03` the whole figure is over any threshold a preset can ask for. **Rendered: threshold
`0.95` against `8.0` on Lorenz are near-indistinguishable.** Lorenz therefore ships it pinned at the
ceiling, with its header saying to read the pair as *capped, not tuned*. **A threshold in
pre-exposure linear units is only meaningful while every preset sits near `exposure = 1.0`** — which
was true until this commit and is now not.

**What this entry is not.** It is not a bug report: nothing renders wrongly, both presets ship the
look they intend, and the workarounds are recorded in their headers. It is the observation that the
workarounds exist because one lever is doing a job it was not shaped for, and that the cost lands on
the *next* author rather than on these two.

---

## Entry 0058 — from Plan 0055 Phase 4 (2026-08-04), the content half of a decision the engine has now made

---

## 0058 — thirteen presets bind the fold and eleven of them have not chosen an edge treatment, because until now there was nothing to choose

- **Raised:** 2026-08-04, at [Plan 0055](plans/done/0055-the-fold-edge-becomes-a-choice.md) Phase 4. Not
  a gap the content lane found — a gap the engine lane *created* on purpose and is handing over.
- **Verified against code:** yes. `grep -l kaleido_order presets/*.toml` returns thirteen files, and
  `grep -l '^kaleido_edge' presets/*.toml` returns two of them, so eleven ride the default. Anchor
  that second grep — `swarm_dense` mentions `kaleido_edge` in a header comment without binding it,
  so an unanchored match reports three.
- **For:** `preset-author`. No engine change, no ADR. The capability exists and is documented; what
  is missing is a per-preset judgement that only looking can supply.

**What changed under them.** [ADR-0061](adrs/0061-kaleidoscope-edge-treatment-is-a-per-preset-choice.md)
made the region outside the fold's inscribed disc a per-preset choice, `kaleido_edge`, with three
treatments: `falloff` (0, the fade ADR-0047 shipped), `tile` (1) and `squash` (2). Plan 0055 Phase
2's live A/B made **`tile` the default**, so every fold-binding preset that says nothing has already
moved from *cropping to a disc* to *filling its frame*. That is a real visual change to eleven
shipped presets, applied by a default rather than by an author, and it is the reason this entry
exists rather than being optional polish.

**Why the scope matters.** At 16:9 the frame's corner sits at 2.04x the disc radius, so **56 % of
the frame** is what the treatment decides. This is not a corner detail on any of the thirteen.

**The eleven that have not been looked at:** `attractor_dejong`, `attractor_lorenz`,
`curve_cathedral`, `fragment_glacier`, `fragment_supernova`, `fragment_warp`, `lsystem_arrowhead`,
`reaction_reef`, `reaction_reliquary`, `swarm_storm` — plus `swarm_dense`, which is a special case
below. All eleven currently ride the `tile` default without anyone having chosen it.

**The two that have been judged, and they are your reference pair.** Plan 0055 Phase 2 put the whole
roster in front of the user in the running app — in motion, over a lit backdrop, at 16:9 and at a
non-16:9 window — on exactly one centred figure and one border-filling field. Both verdicts are
**landed**, so they are shipped examples you can read rather than advice:

| preset | kind | verdict |
|---|---|---|
| `attractor_leviathan` | centred figure | **`tile`**, landed with a zoom raise (see below) |
| `fragment_kaleido` | border-filling field | **`squash`**, landed |

**That the two chose differently is the finding, not a detail.** It is the whole evidence for
`kaleido_edge` existing at all, and it is the first question to ask of each preset below: is this a
figure with space around it, or a field that fills its frame? The pair does not settle the other
eleven — nobody has watched those — but it tells you what the axis is.

**What Leviathan's change tells you about the others.** Adopting a fill treatment there was **two
edits, not one**. Its `zoom` had been pinned at base 0.72 with a header explaining that the pin was
"a fold constraint, not a taste" — the figure was held inside the inscribed disc so it could not
feed the fold's residual rays. A fill treatment removes that constraint entirely, and the preset
only benefits from one if there is content out past `r_max` for it to act on, so the zoom went to
1.80. **Expect the same shape elsewhere:** any preset whose scale, `zoom` or `glow` was tuned against
a disc that crops is now tuned against a premise that no longer holds. Grep the fold-binding headers
for language about the disc, the inscribed radius, or the rays before assuming a file only needs one
line added.

**`swarm_dense` is the odd one and worth doing first.** It pins `kaleido_order = "1"` — the fold off
— and its header documented that as a *mitigation for an engine artifact*: bright bars along the
frame edges, which was design-backlog 0010's clamped-edge smear. That artifact was fixed engine-side
by ADR-0047 a plan ago, so the dodge has been unnecessary since then and the comment was stale twice
over. Phase 4 corrected the comment and **deliberately did not turn the fold back on**, because
nobody has looked at this preset folded since the fix and that is a judgement for this lane. It is a
sparse figure over a dark field, which is the case where the three treatments differ most.

**Pairs with [0038](#0038) and [0040](#0040)**, and the pairing is the argument for doing them
together rather than in sequence: all three are retunes of the same shipped set against a composite
that moved underneath it. 0038 is the tonemap knee's ~8 % luminance loss, 0040 is coverage-as-alpha
making dim figures read as dark speckle over a lit backdrop — and a lit backdrop is exactly the
configuration this entry's treatments are judged in, since under `falloff` the corners *are* the
backdrop and under `tile`/`squash` they stop being it. Judging any one of the three at
`bg_bright = 0` is what produced the confirmation failure ADR-0061's Notes records.

**How to judge it.** In the running app, in motion, over a **lit** backdrop, at 16:9 and at a
window that is clearly not 16:9 — `LMV_PRESET_DIR` pointed at the repo's `presets/` makes an edit
live in about 150 ms, so walking a preset through `kaleido_edge = 0 .. 2` is changing one integer
and watching. The parameter roster and the per-treatment guidance are in
[`presets/README.md`](../presets/README.md#screen-space-kaleidoscope--kaleido_order-kaleido_angle-kaleido_center_x-kaleido_center_y-kaleido_edge).

**Not in scope.** Adding a fourth treatment. The roster is a closed set by ADR-0061; a look that
needs a new edge behaviour is engine work and routes back through `architect`.
