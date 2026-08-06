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
| 0033 | Every mark the engine can draw is a round blob or a stroked curve — **silhouette half only** | [ADR-0084](adrs/0084-a-particle-marks-silhouette-is-a-signed-distance-function.md) + [Plan 0070](plans/done/0070-shaped-marks.md). **Closed 2026-08-05.** `shape`/`points` on `swarm` and `emitter`; `swarm_starfield` ships. **The fill-and-outline half is NOT closed** — re-filed as [0069](#0069--there-is-no-way-to-draw-a-two-tone-object-a-fill-with-a-contrasting-outline-because-the-composite-is-additive) at that close, as this entry asked, so the two stop being confused |
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
- **ROUTED 2026-08-04 → [Plan 0067](plans/0067-the-curation-route.md) Phase 1d**, as a bounded
  measurement rather than a redesign. The user's call, from the two options put to them: raise the
  gate's resolution and re-baseline the floor if the measurement supports it, rather than design a
  coverage-aware successor statistic.

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

## Entries 0059-0060 — from the `preset-author` handoff of 2026-08-04, after Squall and the fold-edge pass

Five findings were handed over; three were record corrections (the `attractor_dejong` misnaming, the
`distinctness` family list, the [0009](#0009--the-animationrs-gate-penalizes-two-legitimate-designs-informational)
update above) and are resolved where they belong. These two are design.

---

## 0059 — the backdrop is the one surface left that does not colour through the shared palette, and nothing says so

- **PROMOTED 2026-08-04 → [ADR-0086](adrs/0086-the-backdrop-colours-through-the-preset-palette.md) +
  [Plan 0072](plans/0072-the-backdrop-joins-the-palette.md)** — same day it was raised, because the
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
- **ROUTED 2026-08-04 → [Plan 0067](plans/0067-the-curation-route.md) Phase 4**, which is already
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
  [Plan 0073](plans/0073-the-fern-unfurls-and-colours-by-what-made-it.md)** — and the route taken is
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
[0040](#0040--additive-light-occludes-by-geometry-so-a-dim-figure-over-a-lit-backdrop-reads-as-dark-speckle)
and its plan, which is the *other* place the additive model's occlusion behaviour is being
questioned — anyone taking this should read that first.

### Priority

**Low, and deliberately so.** The user has asked for it once, in a form Plan 0070 partially answered,
and the cost is a composite redesign. It is here so the ask survives, not because it is next.
