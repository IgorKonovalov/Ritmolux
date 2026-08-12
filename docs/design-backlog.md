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
| 0007 | `star_pattern` is a hollow ring, and `variant` cannot be blended | **Closed in full 2026-08-06.** Morph half: [ADR-0060](adrs/0060-star-pattern-variants-interpolate.md) + [Plan 0054](plans/done/0054-the-line-scenes-catch-up.md). Interior half: [ADR-0079](adrs/0079-the-mandala-interior-is-rings-of-motifs-inside-star-pattern.md) + [Plan 0065](plans/done/0065-the-mandala-interior.md) — `[generator] rings`, three ring levers, three mandala presets |
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
| 0033 | Every mark the engine can draw is a round blob or a stroked curve — **silhouette half only** | [ADR-0084](adrs/0084-a-particle-marks-silhouette-is-a-signed-distance-function.md) + [Plan 0070](plans/done/0070-shaped-marks.md). **Closed 2026-08-05.** `shape`/`points` on `swarm` and `emitter`; `swarm_starfield` ships. **The fill-and-outline half is NOT closed** — re-filed as [0069](#0069--there-is-no-way-to-draw-a-two-tone-object-a-fill-with-a-contrasting-outline-because-the-composite-is-additive) at that close, as this entry asked, so the two stop being confused |
| 0034 | Nothing in the engine spawns, throws, ages or individuates an object | [ADR-0057](adrs/0057-emitter-scene-analytic-ballistics-seeded-individuation.md) + [Plan 0052](plans/done/0052-the-emitter-objects-that-spawn-fall-and-die.md) |
| 0035 | `presets/README.md` listed 10 expression variables; the code had 19 | Fixed at [Plan 0048](plans/done/0048-analysis-v2-and-the-retune.md)'s close |
| 0036 | Does the fold stop folding the backdrop, and does that lose a look? | **Retired unfired 2026-08-04.** [ADR-0055](adrs/0055-backdrop-leaves-the-post-chain.md) shipped 2026-07-31; three full-library content passes have run since and no preset was reported worse. The way back is recorded in the archived body if it ever bites |
| 0037 | The fold covers a disc, and on a field scene that reads worse | [ADR-0061](adrs/0061-kaleidoscope-edge-treatment-is-a-per-preset-choice.md) + [Plan 0055](plans/done/0055-the-fold-edge-becomes-a-choice.md) |
| 0039 | Four bind-group layouts are shared by pipelines live in one frame | [ADR-0058](adrs/0058-bind-group-layout-collisions-carry-evidence.md) + [Plan 0053](plans/done/0053-the-suite-stops-blessing-what-warp-gets-wrong.md) |
| 0040 | Additive light occludes by geometry, so a dim figure over a lit backdrop reads as dark speckle | [ADR-0085](adrs/0085-how-much-a-scene-occludes-the-backdrop-is-one-number.md) + [Plan 0071](plans/done/0071-light-that-adds-without-covering.md). **Closed 2026-08-09.** `occlude` shipped as a bindable scalar at the backdrop composite and **the default stayed at `1.0`** — decided by the user in the running app over a lit backdrop at 0.35 and 0.60, not by the argument. So the answer to "is coverage the right model" was *yes, keep it, and make the exception reachable*. **The retune it invites is NOT closed** — Plan 0071 Phase 5, still outstanding, grouped with [0038](#0038--mid-tone-dominated-presets-lost-8--luminance-to-the-tonemap-knee-and-the-library-has-not-been-retuned) |
| 0041 | The line seam's lit-backdrop guard discriminates on ~5 pixels | [Plan 0053](plans/done/0053-the-suite-stops-blessing-what-warp-gets-wrong.md) |
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
| 0054 | Pixel coverage cannot see a figure whose *tips* leave the frame | [ADR-0083](adrs/0083-in-frame-geometry-is-measured-at-the-line-renderers-draw-seam.md) + [Plan 0069](plans/done/0069-the-instrument-that-sees-a-figure-leave-the-frame.md). **Closed 2026-08-06.** The successor measures in-frame segment length inside `LineRenderer::draw` and convicts both frozen defects (`0.4975` / `0.7788` separation, against the `0.055` pixel coverage had). **But it has no separating absolute threshold over the shipped library either** — `Rose Zoom` and `Rose Overflow` bracket the over-scaled comb and both are correct content, so it shipped as a **paired** instrument, not the gate this entry asked for. What that leaves open is [0070](#0070--the-in-frame-geometry-fraction-cannot-gate-new-content-and-the-number-it-computes-for-every-line-preset-is-not-in-the-authors-report) |
| 0057 | No scene-local level param, so `exposure` gets used for one and two stages disagree | [ADR-0080](adrs/0080-the-attractor-owns-its-level-and-bloom-thresholds-exposed-light.md) + [Plan 0066](plans/done/0066-the-level-lever.md). **Closed 2026-08-05.** Both halves landed; the retune found a consequence the ADR had not — the background pre-pass is upstream of the tonemap, so moving a number from `exposure` to `brightness` multiplies the sky by `1/old_exposure` (33x on Lorenz). Recorded as the ADR's `Outcome` |
| 0058 | Thirteen presets bind the fold and eleven had not chosen an edge treatment | Closed by content 2026-08-04, `859ec66` — all thirteen now name a `kaleido_edge`, the verdicts spread across all three treatments. **The entry named `attractor_dejong`, which binds no `kaleido_*` param; the thirteenth is `attractor_clifford`** — inherited from [Plan 0055](plans/done/0055-the-fold-edge-becomes-a-choice.md)'s own scope bullet, corrected in both |

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

### Update 2026-08-04 — the gate has now rejected a preset, and there is a number

- **Raised by:** `preset-author`, landing `emitter_squall` (`f6c56dc`). The entry above was filed
  explicitly *not* as an argument that the gate should change. This update does not make that
  argument either, but it does retire the phrase "informational": the gate is now shaping shipped
  content rather than only measuring it.
- **ROUTED 2026-08-04 → [Plan 0067](plans/done/0067-the-curation-route.md) Phase 1d**, as a bounded
  measurement rather than a redesign. The user's call, from the two options put to them: raise the
  gate's resolution and re-baseline the floor if the measurement supports it, rather than design a
  coverage-aware successor statistic.
- **MEASURED 2026-08-09, and the answer is negative — which sharpens this entry rather than closing
  it.** Phase 1d rendered both cases plus a static control at 96 / 192 / 384. **The ladder is flat.**
  `frame_diff` scores **occupancy**, and occupancy is scale-invariant, so no resolution separates
  *sparse but moving* from *static*. `ANIM_FLOOR` and `SIZE` did not move; the ladder ships
  `#[ignore]`d as a recorded measurement, at zero CI cost. The route the user chose is therefore
  **closed off**, and what remains is the alternative that was explicitly not taken: this gate needs
  a **coverage-aware statistic**, not a bigger render. That question is now *earned* rather than
  speculative, and it is the live form of this entry.

**The measurement, from the preset's own header.** Squall's second draft was the shipped geometry at
**a fifth of the density** — "far better looking, individual parabolas, lots of dark" — and it
failed: `anim` **0.005** against `ANIM_FLOOR = 0.01`, with three of four reactivity bands under
0.02. The shipped density is the lowest that clears both with margin, measured rather than guessed:
`anim` **0.018**, bands 0.025 / 0.018 / 0.032 / 0.023. So the gate did not reject a broken preset —
it rejected the better-looking one of two, and the author raised density by 5x to pass.

**One of the two cases is provably not a resolution problem, and the plan phase says so.** A figure
invariant under rotation by `2*pi/k` produces an **identical image** under that rotation, so its
whole-frame difference is zero at *every* resolution — Star Rosette's spinning ring cannot be
rescued by rendering it larger. The thin-stroke / sparse case is the one where resolution plausibly
helps, and even there it is not obvious: a mark smaller than a pixel at 96x96 is lost or aliased
rather than area-averaged, so whether the statistic separates a sparse-but-moving frame from a
static one is an empirical question. **That is why the phase measures a ladder before it moves a
constant.** The non-vacuity probe is free: the rejected draft is the shipped `emitter_squall` with
`spawn_rate` cut to a fifth.

**What this does not become.** An argument that `ANIM_FLOOR` should be lowered. A floor that a
genuinely static preset can clear is worth nothing, and the shipped Squall sits at 1.8x the current
floor, so the headroom is not large enough to give away blind.

### Update 2026-08-11 — the Plan 0075 cohorts add a second casualty, and this one was routed out rather than tuned up

- **Raised by:** `preset-author`, Plan 0075 cohort 4. A QUIET twinkling starfield — sparse marks,
  low coverage, slow shimmer (the "Perseids' quiet sky" look) — was **routed out of the cohort
  rather than shipped**, because the coverage/animation floors at 96x96 make a legitimately
  sparse, slow look ungateable. (The other half of that casualty — a swarm mark has no per-mark
  variation to twinkle — is
  [0068](#0068--a-swarm-mark-has-no-per-mark-variation-so-the-only-scene-that-can-hold-a-starfield-cannot-make-one-twinkle)'s,
  re-raised the same day.)
- The earned question above now has **two named casualties of two different severities**:
  `emitter_squall` shipped at 5x the density its author preferred (the gate shaped it), and
  Perseids did not ship at all (the gate priced the look out). The first was bent; the second
  was lost.
- **Handoff verdict (2026-08-11): promote** — the coverage-aware statistic, jointly with
  [0068](#0068--a-swarm-mark-has-no-per-mark-variation-so-the-only-scene-that-can-hold-a-starfield-cannot-make-one-twinkle)
  as the sparse-idiom pair: one look class, two walls, and fixing either without the other
  leaves the look unreachable.
- **PROMOTED same day → [ADR-0091](adrs/0091-the-animation-gate-scores-motion-against-the-figures-footprint.md) +
  [Plan 0077](plans/done/0077-the-quiet-sky.md) Phase 1** (queued after Plan 0076 + cohort 6).
  The rotational-symmetry case stays out of scope by arithmetic and remains this entry's
  documented authoring constraint — the promotion covers the sparse half only.
- **DELIVERED 2026-08-12 (Plan 0077 Phase 1, `698b734`).** The gate scores
  `metrics::footprint_diff` — motion over the union of lit pixels, backdrops stripped — and
  the entry's own casualty is the standing proof: the rejected fifth-density Squall draft
  **passes at 0.1049** (the whole-frame statistic read it 0.0057), the static control keeps
  failing on a zero numerator, both pinned as a non-vacuity test, and the whole-library
  re-sweep convicted nothing. The statistic half of this entry is closed;
  [ADR-0091's Outcome](adrs/0091-the-animation-gate-scores-motion-against-the-figures-footprint.md#outcome--2026-08-12-at-plan-0077s-close)
  carries the details. The rotational-symmetry case stays what it always was — a documented
  authoring constraint no image-domain statistic can lift.

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

## 0038 — mid-tone-dominated presets lost ~8 % luminance to the tonemap knee, and the library has not been retuned

- **Raised:** 2026-07-31, from `architect`, at Plan 0045's Mode 4 review.
- **Verified against code:** yes — measured, not inferred (numbers below).
- **For:** `preset-author`. This is genuinely content-lane work; the engine behaved as designed.
- **ROUTED 2026-08-01 → `preset-author`, as a content pass rather than a plan.** The user's
  call at the Plan 0051 close: this needs no engine change and no ADR, so it goes to the lane
  directly. It pairs naturally with [0040](design-backlog-archive.md#0040--additive-light-occludes-by-geometry-so-a-dim-figure-over-a-lit-backdrop-reads-as-dark-speckle) (**closed 2026-08-09**; its retune half is Plan 0071 Phase 5, which this should run with) — both are retunes of the same shipped set
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

## 0042 — the downbeat estimator locks on ~3 % of audible time, so the gated bar variables are almost always fallback

- **PROMOTED 2026-08-04 → [ADR-0082](adrs/0082-the-downbeat-gate-holds-and-the-estimator-is-diagnosed-first.md) +
  [Plan 0068](plans/done/0068-why-the-downbeat-rarely-locks.md)** — as a **diagnosis**, not a fix. The
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

**ANSWERED 2026-08-09 by Plan 0068 Phase 3 (the targeted pass this entry asked for).** 98 minutes of
unambiguous 4/4 through the live app on `v0.48.0`, 5900 audible rows: **352 locked — 6.0 %**. Split by
genre it is **6.79 %** on four-on-the-floor techno (5173 rows) and **0.14 %** on backbeat rock/pop
(727 rows, one single locked row, peak confidence 0.2664 against the 0.25 gate). Two things this
settles:

- **The ~6 % estimate was right, and it is a ceiling rather than a floor.** Restricting to clear 4/4
  does not rescue the rate — the material was never the problem.
- **Backbeat material is 48x *worse* than four-on-the-floor**, which is the opposite of the intuition
  and is what names the cause. The accent is 70 % bass band (`BASS_WEIGHT`); the kick marks every
  beat in four-on-the-floor and the half-bar in a backbeat, so it hardly ever marks the *bar*. The
  named cause is the accent feature, not the fold and not the confidence measure — full reasoning,
  the ladder placement and the limits are in
  [ADR-0082](adrs/0082-the-downbeat-gate-holds-and-the-estimator-is-diagnosed-first.md)'s `Outcome`.

The authoring-doc qualification this entry called for is done (`presets/README.md`, `docs/presets.md`,
both now stating the measured rate). **The repair — a downbeat cue that is not bass energy — is not
written and has no plan**; it is the open work this entry now points at. The mis-accent question
ADR-0050 guards is *still* untested, for the same reason as before: the gate was shut ~94 % of the
time here too.

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

### Update 2026-08-11 — cohort 5 answers the re-check, and the concrete mechanism is per-tuple framing

Plan 0075's own Risks asked for this entry to be re-checked before the brief, because the
Plan 0059 Phase 4 levers might have closed the gap. Cohort 5 is the answer: the levers deliver
the breathe-and-bend vocabulary as designed, and the remaining wall is not coefficient freedom —
it is **framing**. A Lorenz at rho ≈ 100 (the torus-knot regime) was considered for the cohort
and is unreachable: `AttractorFamily::projection()` / `seed_box()` are **per-family constants
sized to the canonical tuple**, so a wild tuple renders off-centre and out of frame with no
preset-side recovery (`pan` cannot span it). Cohort 5 shipped unheld IFS figures instead.

This sharpens the entry's own "curated tuple roster is the cheap middle": the roster needs
**per-tuple projection and seed boxes**, not just a list of coefficients — a tuple is only
reachable with its framing carried beside it.

**Handoff verdict (2026-08-11): promote.** The entry already names the rejected alternative
(free coefficient binding, which exists and cuts), the mechanism is now concrete, and a cohort
demonstrably shipped around the gap. ADR-shaped and earned.

**PROMOTED same day → [ADR-0093](adrs/0093-attractor-tuples-are-content-with-per-tuple-framing.md) +
[Plan 0079](plans/0079-the-attractor-learns-new-figures.md)** — the roster **plus** measured
morph paths, the stronger form, user-decided by interview (zero surviving paths is a recorded
outcome, not a failure). Queued after Plan 0076 + cohort 6, last of the three handoff plans.

---

## Entry 0056 — from the Plan 0050 close (2026-08-04), found while clearing a stale preset cache

---

## 0056 — a user-authored preset has been living outside the repo for six weeks, and it is a curation candidate the boundary has no route for

- **PROMOTED 2026-08-04 → [ADR-0081](adrs/0081-the-content-lane-lands-presets-and-architect-curates-the-set.md) +
  [Plan 0067](plans/done/0067-the-curation-route.md)** — both halves the entry asks for. The owed
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

## Entries 0059-0060 — from the `preset-author` handoff of 2026-08-04, after Squall and the fold-edge pass

Five findings were handed over; three were record corrections (the `attractor_dejong` misnaming, the
`distinctness` family list, the [0009](#0009--the-animationrs-gate-penalizes-two-legitimate-designs-informational)
update above) and are resolved where they belong. These two are design.

---

## 0059 — the backdrop is the one surface left that does not colour through the shared palette, and nothing says so

- **CLOSED 2026-08-09 — [Plan 0072](plans/done/0072-the-backdrop-joins-the-palette.md) landed.** The
  backdrop samples the preset's baked LUT, `background.rs` carries no cosine copy, and
  `saturation` / `palette_mix` reach the sky through one binding. `docs/preset-palettes.md` has its
  own backdrop section and the roster row; `presets/README.md`'s sentence is true rather than
  corrected. Three of the sixteen in-scope presets were re-tuned. **Everything below is the entry as
  raised** and describes the pre-plan engine — read it as history, not as behaviour.
- **PROMOTED 2026-08-04 → [ADR-0086](adrs/0086-the-backdrop-colours-through-the-preset-palette.md) +
  [Plan 0072](plans/done/0072-the-backdrop-joins-the-palette.md)** — same day it was raised, because the
  entry's own verification turned a documentation gap into a coherence gap. The doc half is not
  waiting for the plan: `docs/preset-palettes.md` gains the backdrop now, describing today's
  behaviour, and `presets/README.md`'s sentence is corrected.
- **Raised:** 2026-08-04, from `preset-author` — as "`bg_hue` is undocumented anywhere", with a
  measured six-row swatch table put in the preset's own header for want of a home.
- **Verified against code:** yes, and the finding got larger under verification.

**The reported half is true.** `docs/preset-palettes.md` — the document that owns the colour surface
and carries a twenty-row swatch table for the line scenes' ramp — does not contain the string
`bg_hue`. `presets/README.md` has one sentence.

**That sentence is wrong, which is the larger half.** It reads: *"`bg_hue` offsets into the shared
cosine palette."* There is no sharing. `core/src/render/background.rs:70` carries its **own copy** of
the iq cosine inline in its WGSL — `d = vec3(0.10, 0.42, 0.62)` — and the pass binds one uniform and
no LUT texture at all (`PARAMS` is `bg_hue`, `bg_bright`, `bg_vignette`; the bind group is
`@group(0) @binding(0) var<uniform>` and nothing else). So:

- **`[palette]` does not reach the backdrop.** An `ember` preset draws an ember figure over a
  *spectrum-cosine* backdrop. `attractor_clifford`'s crimson→ember→white-hot custom gradient does
  not tint its own sky.
- **Neither do `saturation` or `palette_mix`.** The A/B crossfade moves the scene and leaves the
  backdrop where it was.
- **The author's measured table is the ramp that is already documented.** `d = (0.10, 0.42, 0.62)`
  is byte-identical to the built-in `spectrum` gradient (`palette.rs:109`) and to the line scenes'
  hardcoded default (`lines/mod.rs:160`), so the six points measured for `bg_hue` land on the
  existing twenty-row table (0.30 blue = `#57ABF8` cornflower, 0.45 teal = `#2BECFA` aqua, 0.85
  amber = `#FCB118` amber). The right doc fix is therefore **one sentence pointing at that table**
  plus the exclusion above — not a second table that can drift from it, which is how backlog 0014
  got its colour names wrong.

**Why it is the last one.** ADR-0021 unified the fragment field and swarm; Plan 0020 Phase 5 pulled
reaction-diffusion onto `spectrum` and re-blessed its baseline; the attractor followed; Plan 0054 /
ADR-0059 brought the four line families in. The backdrop was named in ADR-0021's *Context* — "ADR-0018
adds a *background* colour (`bg_hue`), not a *scene* palette" — as a reason it was a different
feature, and it has never been revisited since every other surface converged.

**Scope, measured before the ADR was written.** 26 of 37 shipped presets bind `bg_bright > 0`. Eleven
declare no `[palette]`, so their gradient already *is* `spectrum` and they cannot move. Fifteen
declare one and would re-tint. Every one of those fifteen sits at `bg_bright <= 0.039` including its
audio term, so this is a dim wash rather than a repaint — but `bg_hue` values were picked as
positions in the cosine, and the same number means a different colour in a custom gradient, so it is
a re-tune and not a no-op. Plan 0072 owns that pass.

---

## 0060 — an engine fix leaves its preset-side workarounds standing, and only a header comment remembers them

- **Raised:** 2026-08-04, from `preset-author`, at the fold-edge content pass — as a *pattern*
  rather than a defect, with two instances and a specific ask.
- **ROUTED 2026-08-04 → [Plan 0067](plans/done/0067-the-curation-route.md) Phase 4**, which is already
  installing a close-ceremony duty hooked to "this plan touched `presets/`". This is a second
  trigger on the same step: "this plan fixed something a preset could have been framed around".
- **Verified against code:** yes — both instances are readable in the shipped files today.

**The pattern.** An engine defect gets worked around in **preset framing** — a zoom pinned, a fold
switched off, a density cut. The workaround is recorded honestly, in the preset's header comment,
because that is the only place it can be recorded. Then the engine defect is fixed by a later plan,
and the workaround stays: it is not a bug, nothing fails, no gate fires, and the file still renders.
The only artefact that knows the pin exists is a comment nobody has a reason to open.

**Instance 1 — `attractor_leviathan`.** `zoom` pinned at base 0.72, with a header saying the pin was
"a fold constraint, not a taste": the figure was held inside the fold's inscribed disc so it could not
feed the falloff's residual rays. ADR-0061 made the edge a per-preset choice; the pin lifted to 1.80.
**Two edits, not one** — the second only findable from the comment.

**Instance 2 — `attractor_clifford`.** The identical shape, found second and only because Leviathan
had taught the lane to look: framing cut from 1.10 base / 1.42 peak to 0.66 / 0.94 purely to keep the
ribbon's tips off the frame edge, with the note ending *"the general fix is a per-preset edge
treatment — Plan 0055, approved, not built."* It was built a plan later and the framing stayed cut,
so the user's report was that the preset was too small to show any difference between the treatments
— which was true, and was the pin talking.

**A third, already recorded, which is what makes it a pattern rather than a coincidence.**
`swarm_dense` pinned `kaleido_order = 1` (the fold off) to dodge backlog 0010's clamped-edge smear.
ADR-0047 fixed that smear; the pin outlived it by a plan, and its comment was *stale twice over* by
the time anyone read it. Three instances, three engine fixes, three files that kept paying.

**The ask, and it is cheap.** When a plan fixes an engine defect, grep `presets/` for headers citing
it before the plan closes. The workarounds are **greppable by construction** — this project's preset
headers name the ADR, the plan, or the backlog entry they are dodging (`design-backlog 0010`,
`Plan 0055, approved, not built`, `ADR-0065`), which is exactly what makes the sweep a one-line
search rather than a re-read of the library.

**What it is not.** A request that engine plans re-tune presets. The sweep's output is a *list* — the
judgement is content work and stays in the content lane. And the pins were right when they were
written: none of the three is an error, which is the point. A correct workaround for a defect that no
longer exists is invisible to every instrument this project has, because nothing is wrong.

---

## Entries 0061-0063 — from the Plan 0063 Phase 5 content pass (2026-08-04)

The pass that judged ADR-0076's four new attractor levers in motion and re-tuned `attractor_lorenz`
and `attractor_thomas` against them. The plan asked five questions; two of the answers needed no
follow-up and are now in the two presets' headers, and three findings are design signal.

**The two that needed nothing.** *Does perspective alone resolve the rotation, or does it need the
haze?* — **perspective alone, decisively.** Rendered as a five-way at fixed audio: with `depth_fade`
and `depth_hue` both on and `perspective` at 0, the butterfly is exactly as flat as it ever was —
both wings the same size, a symmetric bowtie, the haze reading as an uneven exposure rather than as
distance. Perspective alone reads as a solid at an angle immediately. The haze reinforces and cannot
substitute, which is worth knowing because it is the cue that *sounds* like the depth cue. And *at
what `density` does the no-occlusion limit bite?* — measured on the ladder 0.002 / 0.01 / 0.05 / 0.2
/ 1.0 with all three cues on: **reads, reads, markedly weaker, gone, gone.** The usable ceiling for a
preset that wants depth is about `0.01`; ADR-0044's warning is correct and the number is an order of
magnitude below the full budget. Both shipped 3-D presets (0.002 and 0.02) sit at or inside it.

---

## 0061 — `perspective` moves the figure far more than it enlarges it, so the documented way to recover the framing does not work

- **PROMOTED 2026-08-09 → [Plan 0075](plans/done/0075-the-content-renaissance.md) Phase 3** (with
  [ADR-0089](adrs/0089-the-library-renews-by-replacement-cohorts.md)'s renaissance) — the
  documentation option, which this entry's own Priority section says is cheap enough to do
  without a plan; it rides the renaissance's pre-flight because the rebuild multiplies the cost
  of every doc that points authors the wrong way. ADR-0076's Outcome section is added by
  `architect` at that plan's close.
- **Raised:** 2026-08-04, from `preset-author`, during the Plan 0063 Phase 5 re-tune.
- **Verified against code and by measurement:** yes, and the finding is larger than the documented
  behaviour it corrects.

[ADR-0076](adrs/0076-the-attractor-keeps-the-depth-it-already-computes.md) and `presets/README.md`
both describe one framing consequence of `perspective`: the magnification is applied before the view
transform, so pushing it up makes the figure larger as well as deeper, and *"recovering the framing
is a `zoom` edit"*.

**That is true, and it is the small half.** The dominant effect is not that the figure grows — it is
that the figure **moves**, by an amount that varies with the spin phase. The near side is magnified,
so the projected centroid shifts toward whichever side is currently near; as the figure turns, that
shift **orbits**. Measured peak-to-peak over four spin phases on a bare Lorenz at fixed audio,
600 px square:

| `perspective` | centre-x swing | widest span |
|---|---|---|
| 0.00 | 0.04 NDC | 522 px |
| 0.15 | 0.11 NDC | 525 px |
| 0.25 | 0.20 NDC | 529 px |
| 0.40 | 0.37 NDC | 542 px |
| 0.60 | 0.55 NDC | 555 px |

The swing runs about **0.9 x `perspective`** in NDC. The size growth across that entire sweep is
**6 %**. So the translation is roughly an order of magnitude more consequential for composition than
the enlargement the docs name — and **a `zoom` edit cannot recover it**, because a zoom is a static
scale and this is a phase-varying translation. All a zoom can do is shrink the figure until the orbit
fits inside the frame, which is what both re-tuned presets now pay: `attractor_lorenz` went from a
1.32 base to 1.16 and `attractor_thomas` from 1.14 to 1.02, and both lost real presence for it
(Lorenz's `sanity` coverage now reads 0.2747 against a 0.18 floor — still passing, with less room
than before).

**The practical consequence is a ceiling nobody documented.** The clamp sits at `0.8` and the
projection is optically fine there — it is a true perspective divide, straight lines stay straight,
and the butterfly at 0.8 reads as a strong wide angle rather than as a fisheye. But past about
**0.3** the figure visibly slides around the frame instead of turning in place, which is a worse
artifact than the flatness the parameter was bought to fix. The usable range is roughly the bottom
third of the legal one, and nothing says so.

### What a fix would be

Three options, in increasing cost, and this entry does not pick one:

1. **Documentation only.** Correct the ADR's and README's framing note to say the effect is chiefly
   translational, quote the ~0.9x law, and give the ~0.3 practical ceiling. Cheapest, and it stops
   the next author rediscovering this with a zoom ladder.
2. **Re-centre the projection on the figure's projected centroid.** Removes the orbit and keeps the
   depth. This is **not** ADR-0076's already-noted followup — normalizing `m` by its value at
   `d_n = 0` divides every magnification by `m(0) = 1` and so changes nothing at all about the orbit;
   it only addresses the size growth. Re-centring is a different, larger change, and it needs a
   decision about *what* to centre on: the particle centroid is a per-frame readback, which the frame
   loop must not do, and a fixed per-family offset would be another table.
3. **Accept and expose it** — a lever letting a preset trade orbit for off-centre framing.

### Priority

**Medium, and the documentation half is cheap enough to do without a plan.** Nothing is broken — the
levers work and both presets ship using them — but the guidance that exists points an author at a fix
that cannot work, and the parameter's legal range is three times its usable one.

---

## 0062 — `depth_hue` is a *lightness* cue on a lightness ramp, it wraps at the ends, and it is structurally dead under `ink_amount`

- **PROMOTED 2026-08-09 → [Plan 0075](plans/done/0075-the-content-renaissance.md) Phase 3** — the
  documentation fix (the three regimes, the `2 * min(hue_center, 1 - hue_center)` wrap bound,
  the duotone deadness), in `presets/README.md` and `docs/preset-palettes.md`. The optional
  wrap clamp stays undecided, per this entry's own reasoning that the repeat is the LUT's
  documented behaviour everywhere else.
- **Raised:** 2026-08-04, from `preset-author`, during the Plan 0063 Phase 5 re-tune.
- **Verified by measurement:** yes, all three parts.

ADR-0076 introduces `depth_hue` as the cue that makes distance read as **distance** rather than as
dimness: *"a hue shift is what makes it read as distance — real atmospheric perspective moves colour
as well as contrast."* That reasoning is sound and the parameter delivers it — **conditionally**, and
all three conditions are invisible from the roster.

**1. It needs a palette that travels in hue.** `depth_hue` shifts the per-particle *palette
coordinate*, so what it does is whatever moving along that preset's ramp does. Rendered side by side
at `perspective = 0.5`: against `attractor_lorenz`'s shipped night-blue -> teal -> mint ->
solar-white ramp, a `depth_hue` of 0.4 reads as the near material getting **brighter** — a second
contrast lever pointing the same way as `depth_fade`, not an independent cue. Against a
constant-lightness hue-travel ramp (blue -> cyan -> gold -> orange -> rose) the identical figure at
the identical value puts a clear **cool cyan on the far wing and warm gold on the near one**, which
is the atmospheric reading the ADR describes. Both shipped 3-D presets use lightness ramps, because
that is what an additive glow scene is normally tuned for — so the parameter is at its weakest on
exactly the two presets it shipped for.

**2. It wraps, and the wrap can make far material look near.** The offset is `+/- depth_hue/2` on a
coordinate the LUT sampler **repeats**. `attractor_lorenz`'s `hue_center` runs as low as 0.13, so a
`depth_hue` much above 0.26 sends the far end negative, wraps it to the top of the ramp, and lands
the far material on the same bright mint as the near — the cue inverts into a collision. At
`depth_hue = 1.0` with `hue_center = 0.20` both ends of the depth range sample the *same* coordinate
(0.70, and -0.30 which wraps to 0.70).

**3. It is dead under `ink_amount = 1`.** The terminal remap keys on luminance and discards hue, so a
depth *tint* is exactly the cue an ink preset cannot show. This is the same trap the same file
already records for `saturation` (`attractor_thomas`'s `a` binding carries that story). Not quite
inert — that preset uses the `mono` palette, so shifting the coordinate moves lightness and the remap
does see it — but measured at `depth_hue = 0.4` it moves 42 % of pixels by a **mean of 2/255 and a
max of 42**, a rounding error against what `depth_fade` does deliberately on the same frame. It is
left unbound there.

### What a fix would be

Documentation, chiefly — this is a parameter whose behaviour is entirely legitimate and entirely
undiscoverable. `presets/README.md`'s new depth section and
[`docs/preset-palettes.md`](preset-palettes.md) should say: `depth_hue` reads as a hue cue only on a
ramp with hue travel at roughly constant lightness; on a dark->light ramp it duplicates `depth_fade`;
keep it under `2 * min(hue_center, 1 - hue_center)` or it wraps; and it is inert under a duotone
remap, like `saturation`. A clamp on the wrap would also be defensible, but the repeat is the LUT's
documented behaviour everywhere else in the colour surface, so making this one parameter special is a
decision rather than a fix.

### Priority

**Low-medium.** Nothing misbehaves; an author simply cannot find out which of three regimes they are
in without rendering a ladder, which is what this pass did.

---

## 0063 — `spin`'s usable ceiling is set by `fade`, not by taste, and the pair is undocumented

- **PROMOTED 2026-08-09 → [Plan 0075](plans/done/0075-the-content-renaissance.md) Phase 3** — the
  documentation fix: `spin` and `fade` are one look, the two measured ceilings, and the stale
  "2-4 is where it becomes legible" advice goes.
- **Raised:** 2026-08-04, from `preset-author`, during the Plan 0063 Phase 5 re-tune.
- **Verified by measurement:** yes.

`presets/README.md` documents `spin` as a rate multiplier on 0.18 rad/s and suggests **2-4** as
"where the rotation starts being legible". On the two presets it exists for, 2-4 is at or past the
point where the figure stops being legible at all — and the bound is not the spin, it is the
**trail**.

A frame of trail drawn while the figure turns is a frame of **rotational smear**. The accumulation
stops being a trace of the trajectory and becomes a set of concentric arcs, which destroys precisely
the volume `perspective` was added to buy. The ceiling is therefore a function of `fade`: with
`fade = 0.932` (~15 frames) the rendered ladder 1 / 2 / 3 / 5 / 8 goes **crisp, crisp, softening,
smeared, scribble**, so the usable peak is about **1.9**. `attractor_thomas` runs `fade = 0.955`
(~22 frames) and its ceiling is correspondingly lower, about **1.3**. The arithmetic agrees: holding
the smear under ~5 degrees needs `rate < 0.087 / (frames / 60)` rad/s.

**The related half — a wide `spin` binding reads as instability, not as drive.** A `1 + bass * 5`
range through `--signal dynamic:110` swings the figure through most of a revolution between
transients and the depth reading never settles; it reads as tumbling. The *integration* ADR-0076
specified is working and is what makes any of this usable — the figure accelerates rather than
snapping to a new angle, which was the failure mode the ADR predicted for the multiply form — but the
integration fixes the discontinuity, not the range. Both re-tuned presets use a narrow modulation
(Lorenz 1.0-1.8, Thomas 1.0-1.3).

### What a fix would be

Documentation. `presets/README.md`'s depth section should say that `spin` and `fade` are one look:
the smear mechanism, the two measured ceilings, and the rule that the ceiling falls as `fade` rises.
The current "2-4 is where it becomes legible" should go — it is right for a trail-free scene and
wrong for every attractor preset that ships.

### Priority

**Low**, and it is one paragraph. But the advice currently in the README points the wrong way, which
is the same shape as 0061: guidance that actively costs an author a ladder.

## Entries 0064-0066 — from the Plan 0062 Phase 7 content pass (2026-08-04)

Three findings from judging the new IFS family against real audio. **0064 is the only one that is
an engine defect**; the other two are documentation gaps, and both matter more than "documentation"
usually does here because the current absence points an author the *opposite* way from what the
family wants.

## 0064 — an IFS preset switch shows a hard-edged rectangle of noise for two thirds of a second, which is the artifact ADR-0066 already removed once

- **PROMOTED 2026-08-05 → [ADR-0087](adrs/0087-the-ifs-particle-carries-its-age-and-its-last-map.md) +
  [Plan 0073](plans/done/0073-the-fern-unfurls-and-colours-by-what-made-it.md)** — and the route taken is
  the *second* of the two this entry proposed, folded into a change that was happening anyway. The
  entry's cheaper interim (seed at the figure's own fixed points) is Plan 0073 **Phase 2**; its
  Phase 3 then goes further and makes the respawn **continuous**, so the population is never a
  uniform box at any moment and the rectangle has no instant in which to form. The plan also catches
  something this entry did not: `jitter_extent` is *derived* from `seed_box`, so collapsing that
  spread — the obvious reading of "fix the seed box" — would have made `reseed` silently inert
  across the whole family. What changes is what `seed()` writes, not the box.
- **Raised:** 2026-08-04, from `preset-author`, during the Plan 0062 Phase 7 content pass.
- **Verified by measurement:** yes — captures at 2 / 6 / 12 / 24 / 40 / 90 frames.

Plan 0062 predicted this and called it a *haze*: "the initial fill scatters particles over the
figure's bounding box, so they converge onto it over ~23 steps — 0.39 s at the fixed step, with the
trail carrying the haze roughly a second at `fade = 0.94`". The timing is right and the description
is not. It is not a haze — it is a **legible, hard-edged, axis-aligned rectangle**, which is a
materially worse thing to show.

Rendered at `fade = 0.88`, `density = 1.0`:

| frame | what is on screen |
|---|---|
| 2-6 | a solid noise slab the shape of the seed box; the figure is a faint ghost inside it |
| 12-24 | the fern has resolved, and a **hard rectangular edge** frames it, brighter than the interior |
| 40 | the rectangle is down to a faint corner shadow |
| 90 | clean |

**Why this matters more than the plan's framing suggests.** It is the *same artifact class*
ADR-0066 was written to remove. That decision's own words are that re-uploading the seed array
"*replaced* the cloud with a uniform box rather than scattering it… a uniform fill of an
axis-aligned box" — which is what a `reseed` visibly was, and why it was changed. The rectangle is
now back: not on a beat, but on **every preset switch into an IFS**.

And the timing is the worst available. Presets dissolve into each other over ~1 s (ADR-0024), so the
**entire dissolve happens while the rectangle is on screen**. What a viewer sees a switch reveal is
a rectangle of noise resolving into a plant, rather than a plant.

**Not authorable around.** Nothing on the preset surface reaches the seed box. `fade` shortens how
long the trail *carries* it but not the slab itself; `density` thins it without removing the edge.

### What a fix would be

The successor plan's **staggered respawn** — the one Plan 0062's "What this plan does NOT do"
already names — is the fix, and it removes this as a side effect of doing something else.

A cheaper interim, if the successor plan is far off: seed every particle **at the figure's own fixed
point** instead of across its bounding box. ADR-0075's own Notes already establish that any
contractive map's fixed point `(I - M)^-1 t` lies *on* the attractor, so an orbit started there is
on-figure at step 0 and there is no convergence transient to hide at all. That is a change to what
calls `IfsFigure::seed_box`, not to the parameterization — and the ADR wrote the property down
precisely because it is a consequence of the parameterization rather than of the successor plan.

### Priority

**Medium**, and higher than the plan assumed. It is not a subtle quality issue: it is a visible
rectangle on every entry into the family, and the family now ships two presets.

### Architect note (2026-08-05, Plan 0062 close review)

**Accepted, and the read is right on both counts** — it is the ADR-0066 artifact class, and the
dissolve timing is the part that makes it matter rather than the duration. Two corrections to the
interim before anyone implements it, and one sequencing call.

**The interim must change `seed()`, not `seed_box()`, and the reason is a coupling the note does
not name.** `AttractorFamily::jitter_extent` (`particles/mod.rs:334`) is *derived* from
`seed_box()` — it is `JITTER_FRACTION` of that spread, which is how one constant serves a map
bounded in `[-2, 2]` and a flow spanning `±26`. So returning a fixed point from `seed_box` makes the
kick zero and **`reseed` goes inert on the whole IFS family**, silently, in a family whose
`reseed` response ADR-0075 lists as one of its free wins. The note is right that the change belongs
at the call site; it should say *why*, or the next person takes the shorter route and breaks the
lever instead.

**Seed at four fixed points, not one.** Putting every particle on a single fixed point trades a
rectangle for 50 000 particles on one pixel for frame 0 — a different artifact, not no artifact.
Same cost and same call site: seed particle `i` at the fixed point of map `i mod MAPS`, **restricted
to the maps with `p > 0`**. All of them lie on the attractor (`A = ⋃ fᵢ(A)` with `A` closed, so each
`fᵢ`'s fixed point is the limit of `fᵢⁿ(x)` for `x ∈ A` and therefore in `A`), they are already
spread across the figure, and the fill is on-figure at step 0 with nothing to converge. The `p > 0`
restriction matters because a padded slot's fixed point is only on the attractor when the pad
duplicates a drawn map — true of all five curated tables today, and exactly the kind of thing that
stops being true when a sixth figure is added.

**Sequencing: do not spend a plan on this.** The successor plan (unfurl + per-map colour) removes it
as a side effect of the staggered respawn, and this is two lines inside that plan's first phase. The
successor is not written yet, so the interim stays live as an option — if the successor slips more
than a session or two, fold the four-fixed-points change into whatever next touches
`particles/mod.rs`. What is *not* acceptable is closing this by widening `seed_box`'s box or by
shortening the trail: neither reaches the edge, and both were tried on the reseed path before
ADR-0066 replaced the mechanism.

## ~~0065 — `morph` is a travel knob whose visible rate is steepest near zero, and nothing says so~~

> **Discharged 2026-08-05** (Plan 0062 close). The fix was documentation and landed in the same
> commit that raised it, `cf977f9`: `presets/README.md`'s IFS section now carries the measured
> table, "`morph` is a TRAVEL knob, not a little-life knob", and the spiral-as-poor-target finding.
> Recorded in [ADR-0075](adrs/0075-ifs-family-morphs-in-singular-value-space.md)'s Outcome. **One
> thing is still open and is a review minor, not a backlog entry:** the param table's own `morph`
> row still reads "every value between is a real figure" ~40 lines above the correction, which is
> what an author scanning the table reads.

- **RESOLVED 2026-08-05, by documentation** — `presets/README.md`'s IFS section now carries the
  measured table, the travel-not-life sentence and the spiral-is-a-poor-target note (`cf977f9`). The
  `morph` row of the param table, which still read "every value between is a real figure" some
  thirty lines above its own correction, now points down to it (`Plan 0073` Phase 5 caught that
  inconsistency; it is done, so that item of its doc sweep is already satisfied). Kept here rather
  than archived because the *shape* of the finding — a parameter whose visible rate is front-loaded
  and whose description implies otherwise — is the reusable part.
- **Raised:** 2026-08-04, from `preset-author`, during the Plan 0062 Phase 7 content pass.
- **Verified by measurement:** yes.

`presets/README.md` documents `morph` as "position from `family` to `morph_to`… every value between
is a real figure". That is true, and it reads as an invitation to use a small `morph` for a little
life. Measured on fern → dragon at a neutral frame, the lit width of the figure as a fraction of the
frame:

| `morph` | 0.00 | 0.05 | 0.10 | 0.15 |
|---|---|---|---|---|
| lit width | 0.248 | 0.448 | 0.572 | 0.584 |

**By 0.05 the fern is half again as wide and already reads as a curl rather than as a plant.** The
cause is in the tables and is general: the dragon's two maps are `0.707 * R(45 deg)` and
`R(135 deg)` against the fern's near-zero rotations, and a few degrees of per-map rotation
**compounds through the recursion**. The rate of visible change is dominated by the *angle*
difference between the two tables and is therefore front-loaded — nothing like the linear read the
parameter's description invites. A cross that stays recognisably the named figure would have to live
under about `0.03`, which is not a lever.

The practical consequence: `morph` is for presets that **travel**, and a preset that wants to stay
one figure should not bind it at all and should use the four levers, which is exactly what they are
for. Both shipped IFS presets are now built that way — `attractor_fern` binds no `morph` at all, and
`attractor_dissolve` uses the full range.

**A second, independent finding on the same surface: the spiral is a poor morph TARGET.** Five
candidate pairs were swept end to end as filmstrips. Anything ending at the spiral thins into ragged
streaks with half the frame empty, because the spiral's dominant map contracts at only `0.93` — the
intermediate spends nearly every sample on a map that barely contracts, so the orbit spreads instead
of settling. `fern -> spiral` is the design's own showcase pair (ADR-0075) and came **last of the
five**. The best by a distance is `sierpinski -> fern` — and the figure ADR-0075 doubted would earn a
preset is exactly why it works: the Sierpinski's rigidity is what makes the dissolve legible.

### What a fix would be

Documentation, in `presets/README.md`'s IFS section: the measured table above, the sentence that
`morph` is travel rather than life, and a note that the spiral is a fine figure and a poor target.
No code change — the surface is behaving as designed and being described in a way that points
authors the wrong way.

### Priority

**Medium.** It is a paragraph, and without it the natural first thing an author tries — a small
`morph` for some life — produces a figure that is no longer the one they named.

## ~~0066 — the IFS figures are STILL, so the library's conventions about drift rates are wrong for them~~

> **Discharged 2026-08-05** (Plan 0062 close). Documentation, landed in the same commit that raised
> it, `cf977f9`: `presets/README.md`'s IFS section now says the figure is static so the levers carry
> all the motion and want ~30 s periods, and that `spin`'s default of a full revolution every ~35 s
> is wrong for a figure with an intrinsic "up". **The one part not discharged is the last sentence of
> its fix** — nothing yet notes, where the animation gate is described, that a passing `anim` is not
> evidence of a watchable preset on a still family. That is a real gap in `docs/capturing.md` and is
> the sort of thing that belongs in whatever plan next touches the gate, not in a plan of its own.

- **RESOLVED 2026-08-05, by documentation** — `presets/README.md`'s IFS section now says the figure
  is static so the levers carry all the motion and want ~30 s periods, and that `spin`'s default is
  wrong for a figure with an orientation (`cf977f9`). The optional half — a note wherever the
  animation gate is described, that a passing `anim` is not evidence of a *watchable* preset on this
  family — is **not** done and is the part still worth doing.
- **Raised:** 2026-08-04, from `preset-author`, during the Plan 0062 Phase 7 content pass.
- **Verified by measurement:** yes — `shot --report`.

Every attractor preset in the library drives its slow evolution from `time` sines with periods of
roughly **200-400 s**, and that is correct for a strange attractor: the attractor is *already*
churning, and the drift only stops it repeating. An IFS at fixed levers is a **photograph**. Nothing
in the figure moves on its own, so everything a viewer sees moving is a lever moving.

Copying the library's periods therefore produces a preset that is technically alive and visually
static. Measured `anim`, against the 0.01 gate floor:

| preset | inherited ~200 s periods | ~30 s periods |
|---|---|---|
| `attractor_fern` | 0.018 | 0.033 |
| `attractor_dissolve` | 0.016 | 0.025 |

Both **cleared the gate** at the slow periods. That is the part worth recording: the animation gate
does not catch this, because its floor is set for "is this frozen" and a still figure under a
slowly-panning view clears it comfortably.

**A related trap on the same family, and it cost a render to find.** `spin` **defaults to 1**, which
is a full revolution every ~35 s. On a chaotic cloud that is the shipped look, and leaving `spin`
unbound is correct. On a figure with an intrinsic "up" — the Sierpinski is an equilateral triangle,
the fern is a plant — the default means the figure spends half of every cycle **upside down**, which
reads as the frame being crooked rather than as the figure turning. An IFS preset almost always
wants `spin` bound to a rock instead. It had also been supplying most of `attractor_dissolve`'s
`anim` until it was bound, which is how the slow-period problem above stayed hidden.

### What a fix would be

Documentation in `presets/README.md`'s IFS section: one paragraph saying the figure is static so the
levers carry all the motion and want ~30 s periods, and that `spin`'s default is wrong for a figure
with an orientation. Optionally a line wherever the animation gate is described, noting that a
passing `anim` is not evidence of a *watchable* preset on this family.

### Priority

**Low-medium.** Purely advisory, but it is the difference between the family's first two presets
looking alive and looking like wallpaper, and neither the gates nor `--report`'s `anim` column
flags it.

## 0067 — `depth_fade` is a uniform dimmer on every flat family, where the other two depth cues are exact no-ops

- **PROMOTED 2026-08-09 → [Plan 0075](plans/done/0075-the-content-renaissance.md) Phase 2** — option
  2 (the true no-op: multiply the fade term by has-depth), restoring ADR-0076's stated
  invariant. Option 3 (redefine `depth01` so flat reads as nearest) stays rejected-for-now for
  the reason this entry gives: it changes what `dn = 0` means for the 3-D families too.
- **Raised:** 2026-08-05, from `preset-author`, while looking for depth on the IFS family.
- **Verified by measurement:** yes — per-parameter capture diffs on `attractor_dissolve`.

ADR-0076's design is that a 2-D family has an inverse depth extent of **exactly `0.0`**, so every
depth cue collapses to the identity "with no shader branch, no division and no way to reach a
`NaN`". Two of the three do. The third does not.

Isolated on `attractor_dissolve` (an IFS, so `dn ≡ 0`), each parameter set alone against an
otherwise identical capture:

| parameter | value | pixels differing | max channel delta |
|---|---|---|---|
| `perspective` | 0.7 | **0** of 921 600 | 0 |
| `depth_hue` | 0.6 | **0** of 921 600 | 0 |
| `depth_fade` | 0.9 | **184 989** (20.1 %) | **97** |

The arithmetic is straightforward once seen. `haze(dn) = 1 - depth_fade * (1 - depth01(dn))` and
`depth01(dn) = (dn + 1) * 0.5`, so at `dn = 0` the multiplier is `1 - depth_fade/2` — **not 1**. A
flat figure at `depth_fade = 0.9` is uniformly dimmed by 45 %, with no gradient, because "mid depth"
is the only depth it has.

It is arithmetically consistent — `dn = 0` genuinely is the middle of the depth range, and the middle
of a fade is half of it. It is not what the ADR's own summary claims, and it is not what
`presets/README.md` claimed either until this entry (that text has been corrected).

**Why it matters more than a doc slip.** It is a trap in both directions. An author binding
`depth_fade` on a 2-D preset expecting nothing gets a 45 % brightness cut that looks like the preset
being mysteriously dark; an author who *notices* the dimming may start using `depth_fade` as a
brightness trim, which works, is undocumented, and would break the moment anyone gave the IFS family
a third dimension. `exposure` is the parameter that means "dimmer" and says so.

**Note this is not IFS-specific** — `de_jong` and `clifford` have had it since Plan 0063 / ADR-0076
landed. Neither shipped preset binds `depth_fade`, which is why it went unnoticed.

### What a fix would be

Three options, in ascending cost.

1. **Document it** — done, in `presets/README.md`'s depth section. Cheapest, and leaves the
   asymmetry live.
2. **Make it a true no-op**, by multiplying the fade term by whether the family has depth at all:
   `1 - depth_fade * (1 - depth01(dn)) * has_depth`, where `has_depth` is `0` when the inverse extent
   is `0`. That restores the ADR's stated property — all three cues identical on a flat family — for
   one extra multiply and no branch, in the same style as the existing zero-extent trick. It would
   move any baseline of a preset that binds `depth_fade` on a flat family; none does.
3. **Redefine `depth01` so a flat family reads as nearest** rather than mid. Cleaner conceptually
   (a figure with no depth is not "half far away") but it changes the meaning of `dn = 0` for the 3-D
   families too, so it is the one that needs thought rather than a patch.

### Priority

**Low-medium.** Nothing renders wrong today and no shipped preset touches it. But it is a stated
invariant that is false, and the ADR leans on the "no branch, no division" phrasing as evidence the
design is clean — which is true for two thirds of it.

---

## Entries 0068-0069 — from Plan 0070's close (2026-08-05)

---

## 0068 — a swarm mark has no per-mark variation, so the only scene that can hold a starfield cannot make one twinkle

- **Raised:** 2026-08-05, from `preset-author`, during [Plan 0070](plans/done/0070-shaped-marks.md)
  Phase 6 — the starfield the whole plan was built for.
- **Verified by measurement:** yes. The emitter draft was rendered and gated before being discarded;
  the numbers below are from that run, not from reading the code.

**The scene with the right individuation cannot hold the look, and the scene that can hold it has no
individuation.** `emitter` carries `twinkle`, whose *rate and phase both* come off each object's own
seed, so a field shimmers while the frame's total light sits still — which is exactly what a sky of
stars does and exactly what a whole-field brightness term cannot fake. It also carries
`size_spread`. It is unusable for a starfield anyway, and the reason is geometry rather than taste:
its source line is fixed at `y = -1.12` and cannot be moved, so a star must travel 2.12 units to
cross the frame. A drift slow enough to read as a sky (~0.85 units/s) needs ~2.5 s to fill it, and
**every behavioral gate in the suite captures 30 frames at 1/60 s, which is 0.5 s** — so the gate
sees an empty sky. Measured: the emitter draft reported cover `0.013` and `0.000` on all four bands.
Speeding it to the ~4.3 units/s the geometry demands is a rising shower rather than a starfield, and
the twinkle stops reading at that speed regardless.

`swarm` has the opposite profile. Its population is fixed and present from frame one, so it has no
warm-up at all and the gates see it immediately — which is why `swarm_starfield` ships on it. What
it has no way to express is **per-mark** anything: a `brightness` or `size` binding moves the entire
field together, so the shipped preset's shimmer is a slow global breath plus a beat flash, and the
per-star life the look actually wants is simply absent.

### What would close it

Either half would; they are independent and the first is much smaller.

1. **Per-mark variation on the swarm** — a `twinkle` and a `size_spread` in the emitter's shape,
   driven off the existing per-particle seed. The swarm already draws a seeded per-particle `size`
   factor and a depth scale, so the machinery to individuate is there; nothing exposes a *bound*
   parameter through it.
2. **A movable or point source on the emitter** — the fixed line is recorded separately in
   [0060](#0060--an-engine-fix-leaves-its-preset-side-workarounds-standing-and-only-a-header-comment-remembers-them)'s
   neighbourhood as "no positionable source". Closing that would make the emitter reachable for
   slow-drift looks, and this entry is a second, independent reason to want it.

**Not the answer:** raising the gate's capture length. The gates are 0.5 s by design and a preset
that needs 2.5 s of warm-up to look like anything is also a preset that looks like nothing for the
first 2.5 s of a live show.

### Priority

**Medium.** One preset ships today with a documented compromise rather than a defect, so nothing is
broken. But the emitter's `twinkle` is the single most-cited example of per-object life in the whole
parameter surface, and the scene that most wants it cannot reach it.

### Update 2026-08-11 — a second, independent want, and this time it was the whole look

- **Raised by:** `preset-author`, Plan 0075 cohort 4: the quiet twinkling starfield (the
  Perseids look). Sparse marks, low coverage, slow shimmer — the shimmer half is exactly this
  entry's option 1 (per-mark variation on the swarm), and the look was **routed out of the
  cohort rather than shipped**. The gate half of the same casualty is recorded in
  [0009](#0009--the-animationrs-gate-penalizes-two-legitimate-designs-informational)'s update
  of the same date.
- **Handoff verdict (2026-08-11): promote** — option 1, jointly with 0009's coverage-aware
  statistic as the sparse-idiom pair (fixing either wall alone leaves the look unreachable),
  and with [0085](#0085--swarm-has-no-reseed-so-a-flow-field-pile-up-has-no-recovery-lever)
  (swarm `reseed`) riding the same plan.
- **PROMOTED same day → [Plan 0077](plans/done/0077-the-quiet-sky.md) Phase 2** (option 1; the
  gate half is that plan's Phase 1 via
  [ADR-0091](adrs/0091-the-animation-gate-scores-motion-against-the-figures-footprint.md)).
  Option 2 — the emitter's movable source — stays open in this entry, unpromoted.
- **DELIVERED 2026-08-12 (Plan 0077 Phase 2, `fae16e6`).** The swarm carries `twinkle` and
  `size_spread` with the emitter's names and semantics — **rate and phase both off the seed**,
  so the field shimmers while the whole-frame mean sits still, the exact property this entry
  measured the emitter for. Both default 0 and the goldens pass unblessed (byte-identity by
  arithmetic, not by bless). The quiet sky itself is Plan 0077 Phase 5, standing in the plans
  README. **Option 2 remains open here.**

---

## 0069 — there is no way to draw a two-tone object (a fill with a contrasting outline), because the composite is additive

- **Raised:** 2026-08-05, at [Plan 0070](plans/done/0070-shaped-marks.md)'s close. **Re-filed from
  [0033](design-backlog-archive.md), at that entry's own instruction** — 0033 carried two asks, Plan
  0070 answered one of them, and leaving the other inside a closed entry is how the two get confused
  again.
- **Verified by measurement:** yes, and the measurement is the point. The cardioid
  `r = 1 - sin(theta)` drawn through `parametric_curve` at `ink_amount = 1` on white paper renders
  its outline **grey**, not black: a thin anti-aliased stroke averages to mid luminance and lands
  halfway down the ink ramp.

The original ask was a Solitaire-style cascade of **hearts — red fill, black outline**. Plan 0070
delivered the silhouette: `shape = heart` on `swarm`/`emitter` draws a heart-shaped *glow*,
brightest in its middle, fading to nothing at its boundary. That is as far as an additive pipeline
reaches. **Black adds zero**, so a dark edge cannot exist inside the composite, and the only
dark-on-light route in the engine is the ink stage, which is structurally two-poled
(`mix(paper, ink, luminance)`) and therefore cannot hold three tones either.

### Why this is its own question and not a follow-on

It reopens [ADR-0018](adrs/0018-engine-wide-scene-compositing.md)'s composite and
[ADR-0056](adrs/0056-additive-scenes-emit-premultiplied-alpha.md)'s alpha model, and it needs an
ordering or sorting story the additive pipeline has never required — a filled object *occludes*, and
nothing in this engine has ever had to decide what is in front. That is why
[ADR-0084](adrs/0084-a-particle-marks-silhouette-is-a-signed-distance-function.md) rejected it as a
bundled decision (Alternative B) rather than on its merits. It also sits adjacent to
[0040](design-backlog-archive.md#0040--additive-light-occludes-by-geometry-so-a-dim-figure-over-a-lit-backdrop-reads-as-dark-speckle)
and its plan, which is the *other* place the additive model's occlusion behaviour is being
questioned — anyone taking this should read that first.

### Priority

**Low, and deliberately so.** The user has asked for it once, in a form Plan 0070 partially answered,
and the cost is a composite redesign. It is here so the ask survives, not because it is next.

---

## Entry 0070 — from the Plan 0069 close (2026-08-06), and it is what that plan turned out not to buy

---

## 0070 — the in-frame geometry fraction cannot gate new content, and the number it computes for every line preset is not in the author's report

- **PROMOTED 2026-08-09 → [Plan 0075](plans/done/0075-the-content-renaissance.md) Phase 2** — the
  cheap half this entry names: the fraction becomes a `shot --report` column, where the
  over-scale defect is actually introduced. The distribution report in `sanity.rs`'s shape
  stays a candidate second step, not taken.
- **Raised:** 2026-08-06, at [Plan 0069](plans/done/0069-the-instrument-that-sees-a-figure-leave-the-frame.md)'s
  Mode 4 review. Successor to archived [0054](design-backlog-archive.md#0054--pixel-coverage-cannot-see-a-figure-whose-tips-leave-the-frame-and-an-in-frame-geometry-fraction-is-the-successor),
  which asked for a **gate** and got a **paired instrument**.
- **Verified by measurement:** yes — the numbers below are the printed report from
  `cargo nextest run -p lmv-core --test geometry_extent --no-capture`, reproduced on this box at the
  close.
- **Not a defect.** Everything Plan 0069 built works exactly as
  [ADR-0083](adrs/0083-in-frame-geometry-is-measured-at-the-line-renderers-draw-seam.md) designed it.
  This is the gap that remains after it.

**What landed, and the shape of what it left.** `LineRenderer::draw` now measures the share of drawn
segment length inside the render target, covering the four line families with one implementation.
Against the two frozen defects it is decisive: repairing them moves the measure `0.4975` (comb) and
`0.7788` (corona), where pixel coverage had `0.055` and scored both defects *above* the legitimate
content. That half is closed and closed well.

**But the instrument convicts only in pairs, and a new preset has no pair.** The gate compares a
configuration against a frozen *repair* of itself, matched by name. That is the right question for a
content pass verifying its own fix, and it is the only question available — no absolute threshold
orders the library, because `Rose Zoom` (`0.3492`) and `Rose Overflow` (`0.3659`) **bracket** the
over-scaled comb (`0.3563`) and both are working exactly as authored. A length fraction cannot tell
"deliberately inside the figure" from "accidentally outside the frame"; they are the same picture.

So the defect class Plan 0069 was built for **still ships undetected on new content**. Author a
`spectrum` preset tomorrow with the same over-scale that shipped in `2efb80e^` and nothing fails:
`sanity.rs` sweeps the library against per-system coverage floors, but `geometry_extent.rs` asserts
only on the two frozen pairs. The plan's own Followup — "if Phase 3 convicts a shipped preset, file
it" — could not fire for the same structural reason, and should be read as answered rather than
pending.

### What would close it

**Put the number where the author already looks, rather than inventing a threshold ADR-0083 twice
proves does not exist.** `shot --report` (`standalone/examples/shot.rs:28`) is the content lane's own
metrics table and does not carry this column. The fraction is computed on the CPU from segment
endpoints and an aspect, with no rasterizer in the loop — so it is cheap, machine-independent, and
already produced on every capture of a line-family preset. Surfacing it there turns "no gate" from a
hole into a number a human can read while tuning `scale`, which is the point at which the defect is
actually introduced.

A distribution *report* in `sanity.rs`'s shape (`report_coverage_distribution`, lowest-first, printed
not asserted) is the natural second step and is explicitly an ADR-0071 report rather than a gate.

**Not the answer:** an absolute floor over the library. It fails two shipped presets on day one, and
it is the identical mistake this whole line of work exists to stop making.

### Priority

**Medium-low.** Nothing is broken and no preset ships defective today — both known defects were
already repaired before the instrument existed. But the plan that built the instrument was scoped as
"the gate that convicts an over-scaled figure", and what is deployed cannot do that for content
nobody has already fixed. The cheap half (a `--report` column) is small enough that leaving it undone
is mostly a matter of nobody having filed it.

---

## Entries 0071-0073 — from the Plan 0065 Phase 3 roster decision (2026-08-06)

Three findings raised when the user judged the Phase 2 sample set and the shipped `star_mandala`
preset in the running app. **0071 is a decision the user actually took** and needs promoting rather
than re-deciding; 0072 and 0073 are the two defects behind the verdict on the preset, which was
"maximally lame — all lines are half transparent, line connections are visible, there is no curve
lines".

**These three were raised as `0070`-`0072` on the `plan-0065-mandala-interior` lane and renumbered
here at its merge**, because `main` had independently minted a `0070` the same day at Plan 0069's
close. Commit messages from that lane (`3c0e56a`, `a35485a`) and the plan's own Phase 3 verdict
still cite the old numbers; the mapping is `0070`→`0071`, `0071`→`0072`, `0072`→`0073`.

## 0071 — the scalloped boundary was chosen as a real curve primitive, and the engine has none

- **Raised:** 2026-08-06, at Plan 0065 Phase 3. **This is a user decision, not an open question.**
- **Verified by measurement:** n/a — it is a look decision, taken from the `bound A touch` /
  `bound B curve` A/B in the Phase 2 sample set.

[ADR-0079](adrs/0079-the-mandala-interior-is-rings-of-motifs-inside-star-pattern.md)'s Notes left
open whether the reference image's scalloped outer boundary is "a motif ring whose members touch, or
a separate boundary curve". Phase 2 rendered it both ways and **the user chose the curve** — and
chose it in the strong form, as a real primitive rather than as side B's approximation.

Side B in the sample set is **not** a boundary curve. The engine has no such primitive and Phase 1
did not add one; side B is 40 `arc` motifs scaled 1.12x so their members overlap and the scallops
merge into something that *reads* continuous. The user was shown that distinction explicitly and
picked the primitive anyway.

### What a fix would be

A new roster member or a new `[generator]` key on `star_pattern` — a closed scalloped curve whose
lobe count and depth are parameters, sampled as one continuous outline rather than as N placed
copies. Architect (ADR) then dev.

### Priority

**Blocked out of Plan 0065 by construction** — the plan's Phase 5 ships presets from the closed
roster, and this is engine work by the user's own choice. It does not gate Phases 4-5: the three
chosen compositions (four rings, six rings, rings in weave) carry no boundary ring.

## 0072 — `sanity.rs`'s coverage floor forces dense thin-stroke line scenes into washed-out tuning, and it is measuring the halo

- **PROMOTED 2026-08-09 → [Plan 0075](plans/done/0075-the-content-renaissance.md) Phase 1** — first
  phase of the content renaissance, because a gate that selects for the rejected look must not
  examine a whole new library. The mechanism (per-family thin-stroke floor vs structural
  occupancy) is chosen at implementation from this entry's two candidates; the non-vacuity
  probes are the ones pinned below (the retired mandalas' honest tunings, and the
  renders-nothing case the current floor does catch).
- **Raised:** 2026-08-06, at Plan 0065 Phase 3, from the user's verdict on the shipped preset.
- **Verified by measurement:** yes, three ways, all inside Plan 0065's own lane.

`star_mandala`'s first draft ran `thickness = 1.15` / `glow = 0.85` / `trails = 0.26` — the author's
judgement that forty times as many segments want a lighter hand. It **fails** `core/tests/sanity.rs`,
which captures at 96x96 and enforces a `star_pattern` coverage floor of **0.34**: it measured
**0.2541**. A second pass at 2.05 / 1.20 / 0.30 read **0.3053**, still short. What ships reads
**0.4346** at 2.45 / 1.55 / 0.40 — and the user's first reaction to it in the running app was that
every line is half transparent.

**The gate cannot see the figure it is gating.** Three measurements say so:

- The mandala's coverage (0.4346) sits **below** bare `star_rosette`'s (0.7995) while drawing ~46x
  the geometry.
- In a controlled A/B at `star_rosette`'s own shipped tuning, the bare rosette and the four-ring
  mandala score **identically (0.403)** — 46x the segments, same number.
- A structural measure separates them cleanly: **9 of 10 radial shells occupied against 1**.

At 96x96 a hairline over a 46-fold ornament aliases to nothing, so `coverage` on this scene is a
measure of the **halo and the trail**, not of the figure. The only lever that moves it is inflating
glow and tail — which is exactly the look the user rejected. **The gate is selecting for the defect.**

### What Plan 0065 Phase 5 measured when it refused the trade (2026-08-06)

The phase was told not to buy coverage with `glow` and `trails`, and it did not. Three mandala
presets ship tuned by eye at 1280x720 with **`glow` at the engine's 1.0 and no `trails` binding at
all**, and all three **fail** this floor:

```text
star_pattern  floor 0.34  lowest 0.2442 (Star Mandala) - factor 0.72
  0.2442 Star Mandala  |  0.2505 Mandala Six  |  0.2544 Mandala Weave  |  0.6908 Star Lantern  |  0.7995 Star Rosette
```

Three things that sharpen the entry rather than repeating it:

- **`thickness` alone cannot reach the floor at a look anyone would ship.** Swept on the four-ring
  preset with `glow = 1.0` and no trail: `2.2 -> 0.198`, `3.1 -> 0.244`, `4.6 -> 0.247`,
  `6.5 -> 0.311`, `9.0 -> 0.366`. The floor is first cleared at a base thickness of about **9**,
  which is a **29-px-wide stroke at 1080p**; the figure has already started closing into a blot by
  4.6. The lever the entry hoped for is not enough on its own.
- **Density still does nothing, now measured within one tuning family.** `Mandala Six` draws 1 684
  segments against `Star Mandala`'s 1 092 — **54 % more geometry** — for a coverage difference of
  0.2442 vs 0.2505, i.e. **2.6 %**. That is the entry's central claim reproduced without changing
  anything else.
- **The three failures are the honest tuning, and the pictures are the evidence.** Compare
  `presets/star_mandala.toml` before and after this phase in `git log` — the pre-Phase-5 file carries
  `glow = 1.55` / `trails = 0.40` and its own comment block explaining the escalation, which is the
  tuning the user rejected in the running app.

**So the gate is red on `star_pattern` until this entry is taken.** That was accepted deliberately
rather than papered over; the alternative was to ship the look the user had already refused.

### Update 2026-08-06 — the floor moved to `0.12` and back to `0.34` the same day, and this entry survives both

Plan 0065 Phase 7 re-derived the floor to `0.12` at its close, which is exactly what
`coverage_floor`'s own rule prescribes when a family minimum moves. Hours later the user rejected all
three presets on sight for a reason that has nothing to do with this entry — the motifs are sampled
polylines and the vertices show ([0073](#0073--motif-outlines-show-their-vertices-and-a-sampled-polyline-does-not-read-as-a-curve))
— so they were retired and the floor reverted to `0.34`.

**Nothing about that answers this entry.** The three measurements above were taken on real content
and stand on their own: at 96x96 the bare rosette and the 46x-denser mandala score *identically*,
54 % more geometry moves coverage 2.6 %, and `thickness` alone first clears the floor at a 29-px
stroke. The next dense thin-stroke line preset meets the same wall. What changed is only that the
library currently ships no preset sitting against it, so the pressure is off the schedule rather than
off the problem — and a reader finding `0.34` in the code should not conclude the episode never
happened.

### Why this is not the same entry as 0054

[0054](design-backlog-archive.md#0054--pixel-coverage-cannot-see-a-figure-whose-tips-leave-the-frame-and-an-in-frame-geometry-fraction-is-the-successor)
and its successor [Plan 0069](plans/done/0069-the-instrument-that-sees-a-figure-leave-the-frame.md)
are about a figure leaving the frame; the in-frame geometry fraction that plan shipped does **not**
catch this one, because a mandala is entirely inside the frame and scores a clean 1.0. This is the
opposite failure: the figure is present, correct and dense, and the instrument reads it as sparse.
Both entries are the same root cause — a pixel statistic standing in for a structural one — and
anyone taking either should read the other.

### What a fix would be

Not a threshold change. Either a per-family floor that acknowledges what thin-stroke line scenes
render at 96x96, or a structural occupancy measure (the radial-shell count above is one candidate and
it already exists as a one-off in the lane). Whatever replaces it must be checked against the case
where it currently succeeds, which is catching a scene that renders **nothing**.

### Priority

**Medium-high, and it will recur on the next mandala.** Every future preset on this scene meets the
same floor and gets pushed toward the same washed-out tuning. It is the one item here that actively
degrades shipped content rather than merely failing to help.

## 0073 — motif outlines show their vertices, and a sampled polyline does not read as a curve

- **Raised:** 2026-08-06, at Plan 0065 Phase 3, from the user's verdict in the running app: "line
  connections are visible, there is no curve lines".
- **Verified by measurement:** **no** — user judgement in the running app, corroborated by the
  Phase 2 sheets (the joints are clearest on `bound A touch`, where neighbouring `arc` members meet).
  **The mechanism below is a hypothesis and has not been measured.**

Every motif in the closed roster is a parametric outline **sampled to straight segments** and drawn
as instanced quads through the shared `LineRenderer`. Two consequences the user saw:

- **Joints are brighter than the strokes they join.** Where two quads meet at a vertex they overlap
  and sum additively, so a vertex reads as a bead. [Plan 0040](plans/done/0040-line-joins-finish-the-job.md)'s
  close found the same shape on a mirrored line preset — the quietest part of the readout rendering
  as its brightest — and Plan 0065's own Risks section predicted it for concentric strokes before the
  plan was built. **Verify against what Plan 0040 actually landed before assuming joins are absent;**
  the visible artifact may be additive overlap on top of working joins rather than missing joins.
- **`circle`, `petal` and `arc` are polygons.** At the sample sheets' stroke and scale they read as
  smooth; in the shipped preset, at motif `scale` 0.13-0.46 with an inflated glow, they do not.
  Segment count per motif is fixed and is not an authorable parameter.

### Update 2026-08-06, same day — this was re-judged after the retune and it is NOT 0072 wearing a mask

The paragraph below hoped the faceting was mostly inflated strokes. It is not. The three presets
were retuned to solid strokes at `glow = 1.0` with no trails, rendered, and the user's verdict on
the result was **"we don't have curves, anything curved is based on several lines, and it's easy to
see them — lines look upscaled and half baked"**. A crop of `Mandala Weave` confirms it directly:
the `circle` motifs are visibly polygons, the strokes carry stair-stepped edges, and every vertex is
a bright bead.

**So this is a ceiling on the approach, not a defect in it.** A parametric outline sampled to
straight segments cannot read as a drawn curve at ornament scale, and no tuning available to the
content lane changes that. **All three ring-mandala presets were retired** (`star_mandala`,
`star_mandala_six`, `star_weave`) and the `star_pattern` coverage floor reverted to `0.34` with them.

**The mandala look then shipped as `presets/reaction_gilt.toml`** (itself retired at Plan 0075
cohort 3; the register now lives in `fragment_mandala`, and the file survives in git history), by a
different mechanism entirely: a Gray-Scott field's **analytic iso-contours** — curves evaluated per
pixel in the shader, with no geometry and therefore no vertex at any resolution — folded into a
10-to-18-wedge rosette by `kaleido_order`, on `kaleido_edge = 0` so it reads as an object on black.
The symmetry is a composite-stage property rather than a placement rule, which is the user's own
proposal ("mandalas really should be done differently — with kaleidoscope") and it is measurably
better: it passes every gate, reacts on all four bands, and is not a near-duplicate of `Reef` or
`Reliquary`.

**What this does NOT close.** The `rings` capability itself is shipped, tested and documented, and
`star_pattern` is no longer hollow — [ADR-0079](adrs/0079-the-mandala-interior-is-rings-of-motifs-inside-star-pattern.md)
stands. What is now known is that placed outline geometry is the wrong mechanism *for this look*;
whether it is right for some other look is untested, and the roster has no shipped user of it.

### What a fix would be

Unchanged in substance, and the candidates should still not be bundled: a curve-aware stroke in the
line renderer, or an authorable sample resolution per motif. The third candidate — "wait and see
what 0072's retune does to the faceting" — is **answered and closed**: the retune happened and the
faceting survived it.

### Priority

**Low, and it changed shape rather than urgency.** It was deferred by the user as "maybe we can
improve upon in the future"; it is lower now, because the look that made it urgent no longer routes
through this code. It becomes urgent again the moment anyone wants a *line* scene to draw something
that reads as a curve — which is every future user of `star_pattern`'s motif roster, and arguably
`parametric_curve` too.

---

## ~~Entry 0074 — from Plan 0073 Phase 6 (2026-08-06), the content pass on the two IFS colour channels~~ — BUILT

> **Closed 2026-08-08 by [Plan 0074](plans/done/0074-the-figure-colours-by-how-far-it-has-come.md) /
> [ADR-0088](adrs/0088-the-ifs-colours-by-distance-from-its-own-skeleton.md).** Route 2 was taken —
> the channel that IS spatial — and route 3 shipped with it: `root_tint` / `root_hue` measure
> distance to the nearest drawn fixed point directly, and `age_tint` / `age_hue` are retired, so the
> roster did not grow. Route 1 (`emergence`) shipped too, but on ADR-0087's independent
> `fade`-interaction merits rather than as the age channel's rescue.
>
> The plan gated on a rendered sample set after one implementation phase rather than at the end,
> because this entry's whole lesson was that the *previous* channel's reasoning sounded right and
> was not. It read, on all five figures. The gate also found something this entry did not predict:
> the palette coordinate is a **fixed budget**, so `attractor_fern` needs `map_tint` cut `0.46 -> 0.22`
> before `root_tint` improves on stock — stacked at full strength the plant washes out.
>
> Kept below unstruck as the record of the measurement that motivated it.

## ~~0074 — the age channel has nothing spatial to colour, because the emergence ramp hides the only steps where age correlates with position~~

- **Raised:** 2026-08-06, at [Plan 0073](plans/done/0073-the-fern-unfurls-and-colours-by-what-made-it.md)
  Phase 6 — the `preset-author` pass that phase exists to run. It is the phase's own
  "any channel or route that could not be made to read is written up here rather than quietly left
  bound to nothing".
- **Verified by measurement:** yes — rendered, and rendered in the configuration most favourable to
  the channel.
- **Half of the plan landed and landed well.** `map_tint` / `map_hue` work, are now bound in both
  shipped IFS presets, and survive the morph. This entry is about the other half only.

**What was built.** [ADR-0087](adrs/0087-the-ifs-particle-carries-its-age-and-its-last-map.md) gave
every particle two channels — which map last moved it, and how many steps since it last respawned —
each reaching the picture by a palette coordinate and a hue rotation. The ADR's stated reading of the
age channel is that "age is distance-from-the-fixed-points in disguise": a young particle has been
iterated only a few times, so it sits near one of the four points the figure contracts toward, and an
old one has spread across the whole figure.

**What it actually renders as: per-particle speckle with no gradient anywhere.** Measured on a bare
fern, `fade = 0` so no trail could average anything, `size = 0.9`, a three-stop ramp chosen for
maximum contrast, and `age_tint = 0.75` — the most favourable configuration the preset surface can
build. The figure comes out a uniform tint carrying fine multi-coloured noise. The same probe with
`map_tint = 0.75` instead, everything else identical, separates the fern into four legible regions:
cream body, teal and cyan fronds, a thin stem. Swept across `morph` at 0 / 0.25 / 0.5 / 0.75 / 1.0 on
`sierpinski -> fern` with `age_tint = 0.75` and a long `fade = 0.912`, no position shows a gradient.

**Why, and it is structural rather than a tuning miss.** The ADR's reading is true only for a
particle's first handful of steps. The family's probability-weighted per-step contraction is `0.742`,
so after about ten iterations a particle's position is decorrelated from its age — it can be
anywhere on the figure. And the first **eight** steps are exactly what
[`EMERGENCE_STEPS`](../core/src/render/scenes/particles/mod.rs) deliberately makes invisible, because
those are the steps where a particle sits on one of four points and the trail field would integrate
a thousand of them per frame into four bright dots.

So the two constants ADR-0087 treats as independent look knobs are in **direct opposition**:

- the emergence ramp exists to hide the four restart points;
- the restart points are the only place age correlates with position;
- therefore the ramp hides precisely the material the age channel exists to colour.

Lengthening the lifetime does not help — the problem is spatial, not temporal. The 180-step lifetime
was judged fine in the running app on 2026-08-06 ("looks ok really"), and the churn reads as life
rather than as twinkle, so the plan's *other* two open questions are answered and only this one is not.

**What the channel is still good for, and why that is not enough to bind it.** Because each
particle's colour cycles through the ramp over its own life, `age_tint` under a trail is a slow
per-particle shimmer. But `hue_spread` already gives per-particle colour variation from the fixed
seed, on the same coordinate, and reads as *grain* rather than as noise because it does not cycle.
Neither shipped preset binds `age_tint` or `age_hue`, and neither should until this is resolved.

### What would close it

Three candidates, and they are genuinely different decisions rather than a menu:

1. **Make the ramp length authorable, or shorten it, and accept some hot spots.** The cheapest, and
   the one the plan half-anticipated by calling both constants "the lever". It trades the artifact
   the ramp was built to remove against the gradient the age channel was built to show. Somebody has
   to look at that trade in motion; no capture decides it.
2. **Colour by a channel that IS spatial.** What the ADR wanted to show — distance from the fixed
   points — is computable directly and does not decay: the shader already has the four points on its
   step uniform. A `depth`-like "how far from the nearest restart point" channel would produce the
   gradient the age channel was reaching for, permanently, and would not fight the ramp. This is the
   honest successor and it is a new plan.
3. **Retire `age_tint` / `age_hue`.** Two params on the longest roster in the library that no
   shipped preset can use is a cost, and ADR-0087's Consequences already flagged "four new params on
   a scene that just took five" as a knowing risk. The two spare words in `Particle` stay spent
   either way — `age` still drives the emergence ramp, which is load-bearing.

**Not the answer:** tuning the lifetime, or binding the channel harder. Both were tried at the
extremes and the picture does not change in kind.

### Priority

**Medium.** Nothing is broken and nothing ships defective — the params default to the identity and
no preset binds them. But half of an accepted ADR is currently unusable content surface, and the
roster documents four channels where an author will find two that work. Whichever of the three
routes is taken, `presets/README.md` needs to stop presenting the four as peers.

---

## Entries 0075-0076 — from the Plan 0074 Phase 6 content pass (2026-08-08), binding the root channel

## 0075 — `root_tint` earned no binding on either shipped IFS preset, and `root_hue` earned both

- **Raised:** 2026-08-08, at [Plan 0074](plans/done/0074-the-figure-colours-by-how-far-it-has-come.md)
  Phase 6 — the `preset-author` pass, filed under that phase's own "any route that could not be made
  to read is written up here rather than quietly left bound to nothing".
- **Verified by measurement:** yes — both routes rendered against each other on both presets, at a
  quiet frame and a typical one, and on `attractor_dissolve` at three points across its morph.
- **Nothing here argues the channel was a mistake.** `root` reads, exactly as the Phase 2 gate
  found. This entry is about *which of its two routes* a real preset can afford, and the answer was
  the same on both looks for two **different** reasons — which is what makes it a property of the
  tint route rather than a fact about one palette.

**What shipped.** `attractor_fern` binds `root_hue = 0.21 + sin(time * 0.047) * 0.05` with
`map_tint` **left at its full `0.46`**; `attractor_dissolve` binds
`root_hue = 0.17 + sin(time * 0.1200 + 0.35) * 0.05`, in phase with its `palette_mix` so the crystal
end is nearly pure and the living end carries the depth. Neither binds `root_tint`.

**The fern: the coordinate was already spent, and the hue route made the budget question moot.**
Phase 2's gate had found a tuning that beats stock — `map_tint` `0.46 -> 0.22` with
`root_tint = 0.85`, the budget *split* rather than stacked — and it recorded that it had judged that
split before `root_hue` existed. Rendered against each other now: the split reads *flatter* than
stock at rest, because cutting `map_tint` in half is exactly the part-separation the fern's Plan
0073 pass paid `hue_spread` for, and the anchored `root_tint` returns a wash rather than a
separation. `root_hue` at full `map_tint` keeps both — the body cools to jade while the frond
origins stay warm, and nothing was given up. **The hue route is not the fallback the gate assumed;
on this preset it is the answer.**

**The dissolve: a different reason, and the general one.** Its palette coordinate is not
contested — the problem is that its mineral end already runs its densest crossings into near-white.
`root_tint` is **anchored**, so it only ever pushes *up* the ramp: at `0.75` it whitens exactly the
regions that were already brightest and the frost structure flattens. The hue route does not touch
the coordinate, so it buys the same depth with none of the headroom.

Stated generally, and this is the part worth keeping: **an anchored coordinate term spends the
ramp's bright end by construction.** So `root_tint` is structurally disadvantaged on any preset
whose palette *ends* bright — which, under the additive composite this library authors for, is most
of them. `map_tint` does not have this problem because it is centred and spends both directions.

**Negative `root_tint` is expressible, is the obvious escape, and has an unflagged edge.** Nothing
stops a preset writing `root_tint = -0.30`, which ramps the figure *down* the palette — spending the
dark end, which is empty. It reads (subtly) on the fern and costs no headroom. But at `-0.55` a
**bright cream speckle appears mid-figure**, in the region that should be darkest: the coordinate
crosses zero and the LUT sampler *repeats*, wrapping the darkest points to the ramp's brightest
stop. Arithmetic confirms it — the fern's coordinate floor is `hue_center` at its sine trough
(`0.20`) minus `hue_spread/2`, so a `root01` of `0.46` goes negative at about `root_tint = -0.38`.
The centred params can find the same edge, but they are far less likely to: they push half as far in
either direction, where an anchored term walks monotonically toward one edge and will find it if
driven. **`presets/README.md` documents neither the negative direction nor the wrap.**

### What a fix would be

Nothing engine-side is *required* — the two-route design already provides the escape, and it worked.
Three things are cheap and would save the next author the same session:

1. ~~**Say in `presets/README.md` that `root_tint` may be negative, and what happens at the edge.**~~
   **Done 2026-08-08 at Plan 0074's close**, with the anchored-term-spends-the-bright-end property, the
   negative escape, the wrap, and the fern's `-0.38` arithmetic. **That is the authoring half of this
   entry closed; what remains open is item 2, which is the engine question.**
2. **Consider clamping the palette coordinate rather than repeating it** — or documenting the repeat
   as deliberate. A wrap that turns the darkest region of a figure into its brightest speckle is a
   surprising default for a *coordinate*, whatever it is for a texture sampler. This is an engine
   question and an ADR-sized one; it touches every scene that samples the LUT, not just the IFS.
3. **Nothing about the anchoring.** It is correct — [ADR-0088](adrs/0088-the-ifs-colours-by-distance-from-its-own-skeleton.md)'s
   *Anchoring* section reasoned it from the measured distribution and this pass agrees with it. The
   consequence above is a cost of a right decision, not evidence against it.

### Priority

**Low.** No shipped content is wrong and no author is blocked — the route that works is bound in
both presets and documented. It is a documentation gap with one genuine engine question behind it.

## ~~0076 — the operator docs describe a fern tuning that the shipped fern does not carry~~ — REPAIRED

> **Closed 2026-08-08 at [Plan 0074](plans/done/0074-the-figure-colours-by-how-far-it-has-come.md)'s
> Mode 4 close**, in exactly the shape this entry proposed: the measurement and its attribution to the
> gate are kept in both files, and each is finished with what actually shipped, joined to the
> `*_hue`-is-the-escape paragraph already sitting below it.

- **Raised:** 2026-08-08, same pass as [0075](#0075--root_tint-earned-no-binding-on-either-shipped-ifs-preset-and-root_hue-earned-both). **This is drift, not a design gap** — filed here because
  it was found by the content lane and the fix is the architect's.
- **Verified:** yes, by reading both files against the shipped `.toml`.

Plan 0074 Phase 5 wrote the Phase 2 gate's fern finding into two operator docs as the worked example
of the palette-coordinate budget rule:

- `presets/README.md` — "Plan 0074 then found the same fern needs `map_tint` cut from `0.46` to
  `0.22` before `root_tint` improves the picture."
- `docs/preset-palettes.md` — the same numbers, as the budget rule's worked example.

Both sentences are **true as measurements** and were correct when written. But Phase 6 then did the
comparison the gate could not — against `root_hue`, which did not exist yet — and shipped the fern
with `map_tint` at its **full `0.46`** and no `root_tint` at all. So a reader who opens
`attractor_fern.toml` expecting the documented `0.22` finds `0.46`, and the worked example now
illustrates a tuning nothing in the repo carries.

**The rule itself survives intact** — the coordinate *is* a fixed budget, and the fern is still the
evidence for it. What changed is the conclusion drawn from it: the fern did not pay the budget, it
took the escape. That is arguably the *better* worked example, since it ends by showing what `*_hue`
is for.

### What a fix would be

Two sentences in each file at the plan's close: keep the measurement, attribute it to the gate, and
finish it with what shipped instead. `docs/preset-palettes.md`'s "**`*_hue` is the escape**"
paragraph is already immediately below the worked example in both files, so the correction is a
join, not a rewrite.

### Priority

**Medium** — it is small, it is in the two files an author reads *first* for exactly this decision,
and it is the kind of drift that reads as authoritative until someone diffs it against the preset.

## Entry 0077 — from the Plan 0061 close (2026-08-08), and it is a gate's blind spot rather than a preset gap

## 0077 — the doc-link gate is blind to reference-style links, so 85 of them rendered as bracket noise behind a green check

**Raised by:** `architect`, at Plan 0061's close ceremony. **Owner if taken:** `dev` — it is a
change to `scripts/check-doc-links.mjs`, which gates CI and the pre-push hook.

### The finding

Markdown has two link forms. `scripts/check-doc-links.mjs` validates one of them: its regex is
`/\]\((?!https?:|mailto:|#)([^)#\s]+)/g`, which matches `[text](target)` and nothing else. The
reference form — `[0044]` in the prose, resolved by a `[0044]: plans/done/0044-….md` definition
elsewhere in the file — is invisible to it, in **both** directions:

- a **use with no definition** renders as the literal characters `[0044]`, and the checker is silent;
- a **definition with a broken target** is never resolved at all, so it can rot freely.

Measured at Plan 0061's close: **85 undefined uses across 11 files**, every one of them behind a
green `links` job and a green pre-push hook.

### Why it accumulated, which is the part worth keeping

**62 of the 85 were created by Plan 0061's own Phase 7b**, and the phase did nothing wrong. It moved
~2,700 lines of link-dense prose from `docs/plans/README.md` into `README-archive.md` verbatim; the
reference *definitions* those lines depend on sit in a block at the bottom of `README.md` and stayed
behind. The archive shipped with **zero** definitions. The phase's done-when said, correctly, *"Run
it, do not inspect"* — and it ran clean.

This is the same shape as the 74 broken inline links Plan 0060 found: a rot that degrades only in a
browser, accumulates one close at a time, and has no natural moment of discovery. Plan 0060 built
the gate for the first form. This is the second form of the identical failure.

### What a fix would be

Collect each file's `^[label]: target` definitions and its `[label]` uses (excluding inline `[…](…)`
and fenced/inline code, which the checker already strips), then report two new classes beside the
existing one: a use with no definition, and a definition whose relative target does not resolve. The
existing per-line `file:line -> target` output shape carries both without a new format. Worth a
deliberately-broken-label check that it goes red naming the label, for the same reason Phase 2c owed
one: a link checker that silently passes is worse than none.

### Priority

**Medium.** The 85 uses were repaired at the close and all 68 definitions in the repo currently
resolve, so nothing is broken today — it is *unguarded*, and it re-accumulates on exactly the
close-ceremony `git mv` that Plan 0060 already proved nobody catches by eye.

---

## Entries 0078-0079 — from Plan 0064's Phase 4 and Phase 6 (2026-08-09), the symmetry stage

## 0078 — `kaleido_tile` is a discrete quantity that is not quantized, so it is the one term of the composed map an author cannot bind

**Raised by:** `preset-author`, at [Plan 0064](plans/done/0064-the-symmetry-stage-and-the-banded-palette.md)
Phase 6. **Owner if taken:** `dev` — it is a CPU-side quantization in
`core/src/render/kaleidoscope.rs`, beside the one `kaleido_spiral` already has.

### The finding

The composed map ships three discrete quantities. Two of them are quantized to integers CPU-side
before the uniform is packed, for the reason this project has already written down twice — an eased
parameter is continuous even when its math needs integers, so the smoothing sweeps it through values
that are not merely wrong but meaningless. `kaleido_spiral` is quantized because a fractional winding
number draws a visible seam. `palette_steps` is quantized because a fractional band count is not a
band count.

**`kaleido_tile` is the same kind of quantity and did not get the same treatment.** It is cells
across the frame, with alternate cells mirrored. A fractional value splits a cell at the frame edge,
so the mirroring no longer meets and the wallpaper stops being seamless — which is the entire
property that makes the tile read as a pattern rather than a grid of stamps.

The practical consequence is narrow and complete: **a preset can only ever bind `kaleido_tile` to a
constant.** `fragment_tiled.toml` does exactly that and says so in its header. Every other term of
the composed map is audio-bindable; this one is decoration you set once.

### Why it is worth an entry rather than a comment

Phase 4 decided the tile **ships** — it was the term the plan named as most likely to drop, and the
rendered grid earned it a place. So this is now an author-facing param whose most natural binding
(a fold count that responds to the music, exactly as `kaleido_order` does) is unavailable, and
nothing warns: the param *is* known, so ADR-0020's unknown-parameter warning cannot catch it, and a
swept value renders a plausible-looking broken picture rather than failing.

### What a fix would be

Quantize `tile` where `spiral` is quantized, and say so in `presets/README.md` alongside the existing
note for the other two. The open question a fix has to answer is what a *transition* between two
integer tile counts should look like — `kaleido_order` has the same problem and solves it with a long
`[smoothing]` constant so the change is rare rather than smooth, which works because the fold count
changes the motif in place. A tile count changes the layout, so the same trick may read as a jump.

### Priority

**Low-medium.** Nothing is broken and the constant binding is honest. It is the gap between "the
stage has five new terms" and "the stage has four new terms you can drive with audio", which is the
difference an author meets on their first attempt.

## 0079 — an accumulating figure rendered with `trails = 0` is not a sparse source, it is a blank one, and a whole third of a decision grid was unreadable because of it

**Raised by:** `architect`, reading [Plan 0064](plans/done/0064-the-symmetry-stage-and-the-banded-palette.md)
Phase 3's sample set at Phase 4. **Owner if taken:** whoever next builds a capture grid — this is a
methodology note, not a code change.

### The finding

Phase 3 rendered its grid with `trails = 0` in every cell, on sound reasoning that is written into
the set's own index: a trail averages frames, and each cell is a judgement about **one frame's**
coordinate map. That is correct for `fragment_field` and for `star_pattern`.

It is wrong for `attractor_lorenz`, and the six attractor sheets came out **near-black**. An
attractor *is* its accumulation — the figure exists only as the deposit of many frames — so removing
the trail does not make it sparse, it removes the picture. Four of the twelve cells in each attractor
sheet have no legible content at all.

### Why it matters beyond one grid

The attractor was in the set **for a specific reason** the plan states: a coordinate map behaves
completely differently on a texture that fills the frame and on one that is mostly empty. So the
sparse-source question is exactly the one those sheets were rendered to answer, and they are the only
cells that could have answered it. Phase 6 then asks the same question again in its own words — does
the mandala hold up on a sparse source, or only on a full-frame field — and had to be answered live
because the grid could not.

The general form: **a capture-hygiene rule that is right for most scenes can be wrong for one
family**, and the failure is silent, because a near-black cell looks like a preset that renders dark
rather than like a broken measurement.

### What a fix would be

Nothing in code. When a grid spans scene families, set the accumulation **per source** rather than
globally — trails off for scenes whose figure is present in one frame, trails at the preset's own
value for scenes whose figure *is* the accumulation — and state the difference in the index, so a
reader knows the cells are not directly comparable and why.

### Priority

**Low**, and it is a note for the next person building a grid rather than work to schedule. It cost
this plan one unanswered question, which Phase 6 absorbed.

[0048]: plans/done/0048-analysis-v2-and-the-retune.md

## 0080 — the reactivity gate pays 1.8x to render frames it throws away, because warm-up and measurement share one capture path

**Raised by:** `architect`, at [Plan 0067](plans/done/0067-the-curation-route.md)'s close.
**Owner if taken:** `dev` — `core/tests/reactivity.rs` and whatever `Renderer::capture_audio` needs
to grow to express "advance without rasterizing".

### The finding

Plan 0067 Phase 1 moved the reactivity gate onto real PCM through the real analyzer, which was the
right call and is the only reason a green suite now says anything about audio at all. It cost
**86 s → 167 s** over 41 presets, measured interleaved on one machine, and the lane declined to
absorb the number silently.

**About 85 % of that growth is warm-up.** Each capture runs `WARMUP_HOPS + SIGNAL_HOPS` hops, and
the warm-up exists so the analyzer's window is full before anything is measured — an FFT and an
onset envelope need history. But `capture_audio` **renders every hop**, warm-up included, and those
frames are discarded. The gate is paying for a full rasterization pass per warm-up hop to obtain a
DSP state that needs no pixels.

### Why it is worth an entry rather than a comment

The cheap fix was already tried and correctly rejected: `SIGNAL_HOPS = 16` cut the cost but dropped
`emitter_squall` to 10 % headroom, which is a gate that fails on someone else's machine rather than
a gate that is faster. So the *measured* budget is not negotiable downward — which leaves the
discarded work as the only slack, and makes this the one place the cost can come from.

It also compounds. This is the pattern Plan 0067 explicitly nominates as the one to copy if another
gate ever needs to answer an audio question, and each copy would inherit the same waste.

### What a fix would be

Split "advance the analyzer" from "capture a frame" in the capture path, so warm-up hops push
samples and step the DSP without a render pass, and only the measured window rasterizes. The
property to preserve is the one the plan already leans on: determinism. The analysis must remain a
pure function of its window, so a warm-up that skips rendering has to produce byte-identical
analyzer state to one that does not — which is testable directly, and is the assertion that would
make the change safe.

### Priority

**Medium.** Nothing is wrong; CI is simply slower than it needs to be on a job this project already
pays for more than once per push. Worth taking the next time anyone is in that file.

## 0081 — the house gain rule lives only in preset headers, so the exception the Coral Oracle found has nothing to be an exception to

**Raised by:** `architect`, at [Plan 0067](plans/done/0067-the-curation-route.md)'s close, from the
`chthonic_coral_oracle.toml` commit. **Owner if taken:** `architect` (the rule is a doc), with
`preset-author` review.

### The finding

This project applies a consistent convention when gaining a band term into a clamped range: derive
the gain from the cap so a typical passage reaches about half of it and a peak reaches it —
`cap / 0.85` for `bass` and `mid`, `cap / 0.60` for `treb` and `onset`. It is applied across the
library and it is the reason a clamp is meaningful rather than decorative.

**It is written down nowhere.** It is not in `presets/README.md`, not in `docs/presets.md`. It
propagates by being copied from one preset header to the next, which is how the Coral Oracle's
author found it and re-derived every gain from it.

And then that preset found a case where the rule is **actively wrong**. For a Gray-Scott regime the
cap is a **death state**: feed high and kill low is the filled regime, where the gaps close and the
picture becomes a flat wash with no contour left to draw. Gains derived by the rule (feed cap 0.059,
kill floor 0.0595) put the field there on a sustained loud passage and rendered as flat mustard. So
`feed` and `kill` are deliberately gentle, with caps pulled inside the labyrinth regime at **both**
ends — a small reactive span on purpose, a drift between two living states rather than a sweep to
the edge of the parameter space.

### Why it is worth an entry rather than a comment

The exception is already recorded, in the one file it applies to. The gap is structural: a
convention that exists only as folklore cannot carry an exception, because the next author meets
the *rule* (copied from a neighbouring preset) without ever meeting the *reason* it has limits.
Both halves are load-bearing for the `preset-author` lane, which keeps no catalogue of its own and
reads `presets/README.md` as the authority.

### What a fix would be

One short section in `presets/README.md`: the rule, the two constants and why they differ, and the
exception class the Oracle names — **a param whose cap is a failure state rather than a maximum**
gets its range pulled in at both ends instead. Gray-Scott `feed`/`kill` is the worked example; it is
unlikely to be the only member of that class, and naming the class is what makes the entry useful
beyond one preset.

### Priority

**Medium.** Cheap, and it converts a piece of folklore plus a one-file exception into a rule an
author can apply and know the limits of.

---

## 0082 — the quality governor reads `frame_ms_p99`, and a preset switch spikes p99 to 25 ms while nothing is dropped

**Raised by:** `architect`, at [Plan 0046](plans/done/0046-transformed-feedback.md)'s close, from
that plan's Phase 5 measurement. **Owner if taken:** `dev`, after an `architect` call on what the
governor should do about it.

### The finding

Three minutes of the standalone at Rich tier, 1080p, two feedback presets with switching and a
fullscreen toggle — 158 audible 1 Hz rows:

| | value |
|---|---|
| fps median / min | 165.0 / **114.3** |
| rows below the NFR §1 60 fps floor | **0 of 158** |
| `frame_ms_avg` median / max | 6.061 / **8.749** ms |
| `frame_ms_p99` median / max | 6.866 / **25.037** ms (p95 of that column: 18.1) |
| frames dropped | **0 of 28 698** |

So the frame budget holds with roughly 2.7x headroom and the transform costs nothing measurable.
**But `frame_ms_p99` exceeds the 16.67 ms budget while `frame_ms_avg` never passes 8.7 ms and no
frame is dropped.** The spikes coincide with preset switches and the fullscreen toggle — GPU
resource rebuilds, not steady-state cost.

### Why it is worth an entry

**The adaptive-quality governor is specified to read p99** (roadmap item 3 / R0, not yet built). A
demotion decision reading this column would see 25 ms during a preset switch and demote a preset
that is in fact running at 165 fps — and demotion changes what the audience sees, on the one event
that is already visually disruptive. The measurement is not a defect; the *instrument the governor
will read* is what needs the qualification, and the cheapest time to know is before the governor
exists.

### What a fix would be

One of three, and choosing is the design question: exclude the frames following a preset switch or
a surface reconfigure from the governor's window; make the governor require N consecutive bad
windows rather than one; or give it a separate steady-state statistic and leave p99 as the
diagnostic it is today. Whatever is chosen, this measurement is the test case.

### Priority

**Medium — but it must be read before R0's governor is designed**, not after.

---

## 0083 — RSS grew 385 to 663 MB over three minutes of preset switching, and there is no no-feedback control to compare it against

**Raised by:** `architect`, at [Plan 0046](plans/done/0046-transformed-feedback.md)'s close, from
that plan's Phase 5 measurement. **Owner if taken:** `dev` (a measurement first, not a fix).

### The finding

Over the same three-minute Phase 5 run, resident set grew **385 MB to 663 MB** (max 663), against
the ~327 MB driver-dominated floor [ADR-0010](adrs/0010-accept-gpu-driver-memory-floor.md)
established and the NFR §12 working-set target.

**This is not yet evidence of a leak, and it is important not to record it as one.** Three minutes
is short, the run switched presets repeatedly (each switch builds a side's GPU resources), and this
plan adds two accumulation buffers, so *some* growth is expected. What makes it worth keeping is
the other half: **it was never measured against a no-feedback control**, so nothing separates
"expected cost of what landed" from "growth that does not stop".

### Why it is worth an entry

R6 ([Plan 0075](plans/done/0075-the-content-renaissance.md)) will ship feedback presets, and the live-show
use case runs for hours — the exact regime three minutes cannot speak to. A number with no control
beside it is the kind of observation that gets quoted later as either a clean bill of health or a
known leak, depending on who is quoting it, and it supports neither.

### What a fix would be

The measurement, not a change: two runs of equal length and equal switching, one on feedback
presets and one on a no-feedback control, reading the same RSS column — and one longer run
(tens of minutes, no switching) to separate per-switch cost from monotone growth. Only if the
control run also climbs is there something to fix.

### Priority

**Medium.** Cheap to run, and it is owed *before* R6 ships long-running feedback content.

---

## Entries 0084-0089 — from the Plan 0075 cohorts 1-5 handoff (2026-08-11)

The renaissance's first five cohorts (28 worlds, cohort 5 judged live 2026-08-11) handed back
one assembled feedback note. Three of its items are **re-raises** and are recorded as dated
updates inside [0009](#0009--the-animationrs-gate-penalizes-two-legitimate-designs-informational),
[0055](#0055--the-attractors-shape-vocabulary-is-breathe-and-bend-and-the-reference-figures-ask-for-more)
and [0068](#0068--a-swarm-mark-has-no-per-mark-variation-so-the-only-scene-that-can-hold-a-starfield-cannot-make-one-twinkle)
rather than as new entries; the two doc drifts it carried went to
[Plan 0075](plans/done/0075-the-content-renaissance.md) Phase 6's sweep list, not here. Each entry
below carries a **handoff verdict** — promote or park — per that plan's Decision (promotion on
demonstrated want, each through its own ADR/plan, never absorbed into 0075). Promoted items
queue **behind Plan 0076 and cohort 6**; none of them gates the collage. All measurements below
are the lane's renders, reported at the handoff — not independently re-verified here; the
file's standing verify-before-acting rule applies.

## 0084 — the ink stage has no contrast lever, and three worlds in two cohorts paid for it

- ~~**PROMOTED 2026-08-11 → [ADR-0092](adrs/0092-the-ink-remap-gains-a-contrast-exponent.md) +
  [Plan 0078](plans/done/0078-the-ink-learns-to-bite.md)** — same day as filed; the response
  exponent, endpoints invariant so the paper never moves. Queued after Plan 0076 + cohort 6.~~
- **CLOSED 2026-08-12 at [Plan 0078](plans/done/0078-the-ink-learns-to-bite.md)'s close.**
  `ink_gamma` ships and `presets/README.md` carries the three-lever note. **The content half is
  standing, not closed**: that plan's Phase 3 (`human`) is the ink worlds re-judging onto the
  lever, carried in the [plans README](plans/README.md)'s Standing section with its two-header
  roster (Etching, Shatter). If a world turns out to need a toe *and* a shoulder, that is
  ADR-0092's named negative — file it **with the measurement** as a new entry rather than
  reopening this one.

**Raised by:** `preset-author`, three separate times across Plan 0075 cohorts 3 and 4.
**Owner if taken:** `architect` (a small ADR — where the response shape lives) then `dev`.

### The finding

The want, each time: a duotone whose dark pole bites harder **without moving the paper**. The
reach, each time: a contrast/gamma control on the terminal ink remap (`ink_*` / `paper_*`). The
remap keys on luminance with a fixed response, so the surface has exactly two workarounds and
both give something up: author the duotone into `[palette]` (the Etching world did this — it
works, and it spends the palette on what the remap should be doing), or juggle
`brightness`/`fade`, which trades away structure to buy contrast.

### Handoff verdict (2026-08-11): promote

Three measurements across two cohorts, and every ink-mode world pays it — the clearest
demonstrated want in the handoff. The obvious shape is a response exponent on the remap; the
nameable rejected alternative is palette-side authoring, which Etching proves possible and
which costs the palette. Where the lever lives and what it does to the existing `ink_*` roster
is the ADR.

## 0085 — `swarm` has no `reseed`, so a flow-field pile-up has no recovery lever

- **PROMOTED 2026-08-11 → [Plan 0077](plans/done/0077-the-quiet-sky.md) Phase 3** — same day as
  filed, riding the swarm-individuation plan exactly as the verdict below proposed; ADR-0066
  disturbance semantics, and the horizon caveat is carried by that plan's Phase 5.
- **DELIVERED 2026-08-12 (Plan 0077 Phase 3, `3bfc7c8`).** The swarm's `reseed` is the
  attractor's percussive accent with ADR-0066's semantics — a seeded ±6 % domain-relative kick
  on a rising edge past 0.5, never a box respawn — measured dispersing (frame-diff 0.153
  against control) and re-gathering (coverage gap 0.30 % three seconds on). The caveat this
  entry insisted on is honoured, not waved through: the *minutes-horizon* rescue is explicitly
  unclaimed by any test, and the one-off soak observation is Phase 5's rider, standing in the
  plans README with 0086's trigger intact.

**Raised by:** `preset-author`, Plan 0075 cohort 4 (the Shatter world). **Owner if taken:**
`dev`, inside whatever plan takes
[0068](#0068--a-swarm-mark-has-no-per-mark-variation-so-the-only-scene-that-can-hold-a-starfield-cannot-make-one-twinkle)'s
per-mark variation — same scene, same plan.

### The finding

Three live collapses before Shatter was rebuilt at engine-default dynamics: a swarm under
minutes of sustained flow-field force piles onto the field's attractors and stays there. The
author's reach was `reseed` — the attractor family has exactly that lever, since
[ADR-0066](adrs/0066-a-reseed-disturbs-the-cloud-rather-than-replacing-it.md), for exactly this
class of reason (disturb the cloud rather than let it degenerate). The swarm has no equivalent,
so the only recovery was rebuilding the world's dynamics from scratch.

Note what the suite said throughout the three collapses: **green**. The failure develops over
minutes and no capture path reaches that horizon — that half is
[0086](#0086--no-capture-path-reaches-the-minutes-long-horizon-so-a-slow-accumulation-failure-is-invisible-to-every-instrument),
and anyone taking this entry must read it first: a swarm `reseed` shipped without it is a lever
whose need the suite cannot demonstrate and whose effect it cannot verify.

### Handoff verdict (2026-08-11): promote, riding 0068's plan

One demonstrated want with three live failures behind it, and the marginal cost inside a
swarm-individuation plan is small. Not worth a plan of its own.

## 0086 — no capture path reaches the minutes-long horizon, so a slow-accumulation failure is invisible to every instrument

**Raised by:** `preset-author`, Plan 0075 cohort 4, watching Shatter collapse live three times
while the suite stayed green. **Owner if taken:** `architect` first — this is a methodology
question before it is code.

### The finding

`shot` strips and all five behavioral gates live in the first seconds of a preset's life (the
synthetic gates capture 30 frames at 1/60 s; even the PCM-driven reactivity gate measures a few
seconds of hops). Shatter's collapse developed over **minutes** of sustained force. Nothing
went red and nothing was flagged — the suite is structurally blind to any failure whose
timescale is the show rather than the capture. The class is real and now has a named member:
any slow-divergence look (accumulating forces, feedback with net gain, populations that
migrate) can ship green and die on stage.

Adjacent, not identical:
[0083](#0083--rss-grew-385-to-663-mb-over-three-minutes-of-preset-switching-and-there-is-no-no-feedback-control-to-compare-it-against)
wants a long-horizon RSS measurement for the same regime (hours-long live shows); the two would
share a soak recipe.

### What a fix would be

**Not a gate.** A minutes-long capture per preset is not a price this suite can pay — see
[0080](#0080--the-reactivity-gate-pays-18x-to-render-frames-it-throws-away-because-warm-up-and-measurement-share-one-capture-path)
for what the *seconds* already cost. The honest shape is a documented soak-style spot-check: a
`shot` mode or recipe that renders N minutes at capture cadence and reports drift statistics
(population spread, deposit concentration), run by the lane on worlds whose mechanism has an
accumulation axis, with the verdict recorded in the preset header the way the fold-edge
verdicts were.

### Handoff verdict (2026-08-11): park, with a trigger

Park until someone ships a look with a slow-accumulation axis — and that trigger explicitly
includes acting on
[0085](#0085--swarm-has-no-reseed-so-a-flow-field-pile-up-has-no-recovery-lever), which must
not ship its lever verified only at the horizon the suite can see.

**Trigger status (2026-08-11, same day):** [Plan 0077](plans/done/0077-the-quiet-sky.md) acts on
0085 and its Phase 5 carries the bounded check this entry's "What a fix would be" prescribes —
one minutes-horizon observation, verdict recorded in the world's header. The entry itself
stays **parked**: no instrument is built, and the trigger re-arms for the next
slow-accumulation look. (2026-08-12: the plan's `dev` scope closed; the bounded check travels
with its Phase 5, standing in the plans README — this entry's status is unchanged.)

## 0087 — `reaction_diffusion` has no glow of its own, and the engine bloom's threshold sits above where its output lives

**Raised by:** `preset-author`, Plan 0075 cohort 3 (the Verdigris/Mitosis register).
**Owner if taken:** `architect` then `dev`, if a second want arrives.

### The finding

The want: a glow accent on the RD field. Engine `bloom_*` acts on the composited frame, and its
threshold sits where the RD field's mapped output rarely reaches — driving the field bright
enough to cross it blows out the pattern first. So one cohort's RD looks were tuned around the
absence.

The route already exists as precedent:
[ADR-0080](adrs/0080-the-attractor-owns-its-level-and-bloom-thresholds-exposed-light.md) gave
the attractor **its own** level and bloom thresholds when the engine-wide ones proved the wrong
instrument for one scene's dynamic range. RD asking for the same shape is not a new argument —
it is the same argument on a second scene.

### Handoff verdict (2026-08-11): park

One cohort's demonstrated want. The ADR-0080 shape is the named route when the second arrives.

## 0088 — `shot --report`'s band columns cannot see reactivity spent on bloom

- **PROMOTED 2026-08-11 → [Plan 0077](plans/done/0077-the-quiet-sky.md) Phase 4** — same day as
  filed, riding the sparse-idiom plan as its small instrument phase; no ADR, per the verdict
  below.
- **DELIVERED 2026-08-12 (Plan 0077 Phase 4, `b1ca4e9`).** The mean columns stay untouched —
  every historical `--report` number keeps meaning what it said — and a **footprint reading**
  lands beside them (`metrics::footprint_diff` over the same capture pairs; text gets its own
  labeled block, JSON gains `reactivity_footprint`). The bloom-only fixture reads bass 0.161
  where the mean reads 0.004, unbound bands stay 0.000 in both readings, and the end-to-end
  claim is pinned in `shot_cli.rs`. The family is now three for three. **The house workaround
  is obsolete** — `fragment_vitrail.toml`'s header still cites it and is on the content lane's
  list (Plan 0077's Standing entry).

**Raised by:** `preset-author`, Plan 0075 cohort 4, as an instrument note. **Owner if taken:**
`dev` — `standalone/examples/shot.rs` and the report's band statistic.

### The finding

A preset whose reactivity is spent on `bloom_amount` reads **~0.000** in the report's band
columns — dead, in the very instrument the lane verifies with. Mechanism as the lane reports
it: sRGB→linear palette peaks times glow sit just under the report's threshold. House
workaround now in use: give onset a structural lever (e.g. `flash`) alongside bloom, so the
report has something to see.

This is the third member of a family the project has fixed twice:
[0022](design-backlog-archive.md) (the report blind to a level `curve`) and
[0028](design-backlog-archive.md) (reachability blind to bare comparisons) were the same
shape — the author's instrument silently under-reporting a legitimate binding class — and both
were promoted and closed.

### Handoff verdict (2026-08-11): promote, small

An instrument that misreports the library it verifies earns its fix, and the family precedent
is two for two. A single-phase plan, or ride the next plan that touches `shot --report`
(Plan 0075 Phase 2 is already adding a column there). Likely no ADR — unless the fix wants a
threshold, in which case ADR-0071's rule applies to wherever the number comes from.

## 0089 — the dragon overruns the frame corner at the default view, and `FRAME_FILL = 0.88` promises it cannot

**Raised by:** `preset-author`, Plan 0075 cohort 5, at 1280x720 — the reference aspect, not an
odd target. **Owner if taken:** `dev` — the suspicion to check first is `FitLut` against the
fallback `frame()` in the IFS fit path.

### The finding

`FRAME_FILL = 0.88` documents that the fitted figure stays inside the frame with margin. The
dragon (two maps at exactly 0.7071, space-filling) overruns the frame **corner** at the default
view; the shipped world works around it with `zoom = 0.92`. Small — but it contradicts a
documented invariant, and this project does not leave a falsified stated property standing.
The first question is which of the two fit sources mis-measures the dragon's extent: the
`FitLut` or the fallback `frame()`.

### Handoff verdict (2026-08-11): park, take opportunistically

Nothing ships broken — the workaround is one line and honest, in the world's own header. Take
it the next time anyone is in the IFS fit path, and do not close it by re-documenting the
invariant away without knowing which source mis-fits.

---

## 0090 — the Mac build's capture verdict is stderr-only, so a Finder-launched tester cannot tell us why it hears nothing

**Raised by:** `architect`, 2026-08-11, from the first external Mac tester report — "app
works, but does not react to music; permissions were granted". **Owner if taken:** `dev` —
`standalone/src/main.rs`'s capture startup, `diaglog.rs`, optionally `overlay.rs`.

### The finding

When `capture_mac::start()` fails, the app deliberately degrades: it prints one line to
**stderr** and renders without audio (`main.rs:1023` — same shape on Windows at `:996`).
That degradation is correct; what is wrong is that the reason exists **only on stderr**,
which a Finder launch discards. The two artifacts a remote tester can actually send carry no
capture verdict at all:

- **`diagnostics.log`** — the file `READ-ME-FIRST.md` step 6 asks testers to send — has the
  band columns (`bass`/`mid`/`treb`/`onset`), so it can show *whether* audio reached the
  analyzer (all ~0.000 under playing music = capture-side), but records nothing about
  whether capture started or which `CaptureError` it died with.
- **The F3 overlay** has no capture field (grep confirms: no capture/audio-state string in
  `overlay.rs`).

So the README's step 3 escape hatch — relaunch from Terminal to see the message — is the
*only* route to the reason, and it is the highest-friction ask in the whole tester loop.
The loop stalls exactly where this report stalled: "permissions were granted" is the
tester's entire observable, and it cannot distinguish the real candidates (not restarted
after granting; a **stale TCC grant** — each ad-hoc-signed build is a different app to
macOS, so the Privacy toggle can show an older build's entry as enabled while the new
binary is denied, the pile-up the README's update note already names; macOS below 13; a
Sequoia periodic re-approval lapse).

### What a fix would be

Cheap, and both halves off the audio thread. At startup, write one line into
`diagnostics.log`: capture started (path + negotiated format) or the `CaptureError`'s
`Display` — startup code, before the 1 Hz cadence. Optionally a capture field in the F3
overlay (`audio: SCK 48k stereo` / `audio: NONE — <reason>`) so a tester can read the
verdict off a screenshot. The sacred-callback rule is untouched — nothing here is on the
capture thread.

### Priority

**Medium-high while external Mac testing is active** — every remote round-trip that starts
with "it doesn't react" pays a Terminal-relaunch cycle this one log line would eliminate.
Drops to low once a capture-device picker (the live-performance plan's Mac half) surfaces
the state in-app anyway.

### Update 2026-08-11 — the tester's log arrived, and it is this entry demonstrated

The `diagnostics.log` came back: **1,249 rows spanning ~6.5 days and 12 app restarts, and
every row has all four band columns at exactly 0.0000.** The renderer is healthy throughout
(steady 60.0 fps, `frame_ms_avg` ~16.7, `gpu_bytes` constant at the 1080p float target; the
scattered 0.1-fps rows coincide with multi-minute timestamp gaps — sleep/background
throttling, not crashes). So the log *proves* capture never delivered one sample on any
launch — which rules out "forgot to restart after granting" (twelve restarts), quiet
music/gain (a live tap shows a noise floor eventually), and anything render-side — **and it
cannot say why**, which is this entry's exact claim. Thirteen launches produced thirteen
stderr lines naming the reason, and every one was discarded by Finder. The surviving
suspects (stale/mismatched TCC grant from the ad-hoc per-build identity, macOS below 13, an
SCK start error) are distinguishable only by the Terminal relaunch this entry exists to make
unnecessary.

---

## 0091 — there is no static, screen-anchored, oriented gradient, so a horizon cannot be drawn

- **PROMOTED 2026-08-12 → [ADR-0094](adrs/0094-the-backdrop-paints-a-directional-ramp.md) +
  [Plan 0080](plans/done/0080-the-sky-gets-a-horizon.md)** — the same day it was raised, because the
  entry arrived with the workaround already rejected by the user and the mechanism already
  concrete. The backdrop pre-pass gains one ramp axis and paints a *segment* of the preset's
  palette along it; the shape was chosen by user interview over a `gradient` scene and over an
  orientation lever on `fragment_field`.
- **Raised:** 2026-08-12, from `preset-author`, in a content session riding
  [Plan 0077](plans/done/0077-the-quiet-sky.md)'s new swarm params.
- **Verified against code:** yes, all three walls, at the lines cited below.
- **Not a defect.** Everything named here works as designed. This is a missing primitive.

**The look that failed.** A photoreal dusk — a bright orange horizon band whose light fades
smoothly upward through deep blue to near-black over roughly the lower third of the frame (the
user supplied a reference photo of exactly this), with a music-driven starfield above it. The best
approximation the shipped engine can express renders a **hard-edged static band with a thin glow
rim**, and the user's verdict on it is **"unacceptable"** — so this is a demonstrated want with a
rejected workaround, not speculation.

**Three walls, each structural rather than a tuning miss.**

1. **`fragment_field` cannot be it.** Its field is `0.5 + 0.5 * sin(p.x + p.y + t * 0.5)`
   (`core/src/render/scenes/fragment_field.rs:142`) — both axes weighted equally by construction,
   so its bands are diagonal and there is no orientation lever — **and** `t` enters the field phase
   directly, so even `warp = 0` cannot hold a frame still. There is no time-scale or freeze param
   on the scene.
2. **The backdrop pre-pass cannot be it.** It takes **one** palette sample at `bg_hue` and
   multiplies it by a *fixed* tilt, `mix(0.72, 1.0, 0.5 + 0.5 * ndc.y)`
   (`core/src/render/background.rs:124`), plus a radial vignette. The tilt is hardcoded, is 28 % of
   brightness rather than a colour ramp, and points **up** — brighter at the top, the wrong way
   round for a horizon.
3. **A line-scene fake cannot finish it.** A `spectrum` slab at `scale = 0` gives a static
   screen-anchored band — this worked, and is what the demo ships — but `glow` is a local stroke
   halo and the bloom pyramid's spatial radius is bounded (`bloom_radius = 3.0` measured, barely
   visible). **No stage downstream can turn a hard edge into a quarter-frame fade.**

**The layer budget is the second half, and it is why the ground cannot simply be another scene.**
This class of look needs three roles — ground gradient, reactive figure, star layer — and
[ADR-0090](adrs/0090-a-preset-composes-two-scene-layers.md) caps composition at a main scene plus
one `[layer]`. The dusk look already spends its layer on the star swarm.

**What made the smallest option also the strongest.** The demo's own `[palette]` already runs
`#060b24 → #1b2a5e → #c74b1d → #ff7a1f → #ffd06e` — near-black through deep blue to hot amber. That
*is* the dusk ramp, already authored, already baked into the LUT the backdrop has sampled since
[ADR-0086](adrs/0086-the-backdrop-colours-through-the-preset-palette.md). Sweeping the coordinate
along a screen axis makes the smooth fade fall out of the palette's own stops, and each stop's `at`
becomes the horizon's vertical placement — so no second colour language and no second placement
mechanism is needed.

**Evidence.** `quiet_sky_demo.toml` and `sun_v8..v10.png` in the content session's scratchpad; the
reference photo is the user's. The `v10` render is the finding in one image: a razor edge with no
upward light at all.

**One thing the promotion does not fix, recorded here because it is the same look's other wall.**
The backdrop is **invisible to every behavioral gate** — coverage measures the scene
([ADR-0067](adrs/0067-coverage-measures-the-scene-not-the-backdrop.md)) and the animation gate
strips `bg_*` ([ADR-0091](adrs/0091-the-animation-gate-scores-motion-against-the-figures-footprint.md)'s
Outcome). That is correct, and it is what stops a more capable backdrop being used to game the
gates, but it means a dusk world's whole gate burden falls on its figure and star layer even
though the frame looks full. If that prices the look out, the rule is the one
[0072](#0072--sanityrss-coverage-floor-forces-dense-thin-stroke-line-scenes-into-washed-out-tuning-and-it-is-measuring-the-halo)
established: **read the floor and re-derive by its own recorded rule, do not lower it to fit.**
