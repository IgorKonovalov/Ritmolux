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

### The second sweep — 2026-08-13, and it found the same drift again

**Twenty more entries moved**, taking this file from 2387 lines back to about a thousand. Almost all
of them **already carried their own `CLOSED` / `DELIVERED` marker** and had simply never been moved
— the plan that discharged them landed, the marker was written, and the body stayed in the open
section. That is exactly the accumulation the 2026-08-04 sweep found and wrote a lifecycle to
prevent, recurring inside ten days, so the lifecycle above is not self-enforcing and the archive
step wants doing **at the close that discharges the entry**, not at the next sweep.

**Two entries did not close — they were falsified**, and both are worth reading as method rather
than as outcome, because in both cases the entry was written against a surface that already
contained its answer:

- **0078** (`kaleido_tile` is not quantized) — `core/src/render/kaleidoscope.rs:458` carries an
  explicit *"Deliberately **not** rounded"* doc comment with its reasoning, and that comment landed
  at **Plan 0064 Phase 1** (`e648a02`), five phases before the entry was filed at Phase 6 of the
  same plan.
- **0081** (the house gain rule is written down nowhere) — `presets/README.md:203` has carried
  `G = C / 0.85` and `C / 0.60` since **2026-08-03** (`fc698cd`), six days before the entry claimed
  the rule did not exist.

Both are corrected in place below rather than moved, because a live entry that is *wrong* is more
dangerous than one that is merely closed: the first sends someone to do work that is already done.
The standing rule at the top of this file — verify against code before acting — is aimed at the
symptom half of an entry. These two say it applies to the *absence* half too: "nothing does X" and
"nothing documents X" are claims about the repo, and they rot the same way.

### The third batch — same day, and it is the lifecycle failing one more time to prove the point

**Three more entries moved on 2026-08-13, hours after the sweep above** — 0077 and 0080 (discharged
by [Plan 0084](plans/done/0084-two-gates-stop-lying-about-what-they-check.md), closed that day) and
0090 (discharged by [Plan 0083](plans/done/0083-the-build-says-why-it-hears-nothing.md), also closed
that day). All three had their `CLOSED` marker written by the close that earned it and **all three
bodies stayed in the open section anyway**, which is the *exact* failure the paragraph above had just
diagnosed and prescribed against. Two closes ran between the prescription and this batch and neither
performed the step.

So the honest reading was that "archive at the close that discharges the entry" had been a rule with
no carrier: it lived in this file, and the close ceremony that would execute it lived in
`.claude/skills/architect/SKILL.md`, which did not mention this file's archive at all. **Fixed at this
batch** — that ceremony now carries the step as **3c**, triggered off the plan header's
`**Closes:** design-backlog NNNN` line, and it says explicitly that writing the `CLOSED` marker is
only half of it. Whether a rule with a carrier actually holds is the thing the next sweep measures.

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
| 0055 | The attractor's shape vocabulary is "breathe and bend", and the reference figures ask for more | [ADR-0093](adrs/0093-attractor-tuples-are-content-with-per-tuple-framing.md) + [Plan 0079](plans/done/0079-the-attractor-learns-new-figures.md). **Closed 2026-08-13, both halves.** Variety: a curated per-family tuple roster (13/13/13/12) whose entries carry their own **measured framing**, so the rho ≈ 100 Lorenz cohort 5 called unreachable now renders centred and in frame. Morph: the entry's "may not exist in general" research question **has an answer along a single-coefficient axis** — four measured paths ship out of twenty swept, four refused by measurement (a mid-walk tuple can collapse to a fixed point, which has no scale to render at). Cross-fading two instances stays rejected and unneeded |
| 0057 | No scene-local level param, so `exposure` gets used for one and two stages disagree | [ADR-0080](adrs/0080-the-attractor-owns-its-level-and-bloom-thresholds-exposed-light.md) + [Plan 0066](plans/done/0066-the-level-lever.md). **Closed 2026-08-05.** Both halves landed; the retune found a consequence the ADR had not — the background pre-pass is upstream of the tonemap, so moving a number from `exposure` to `brightness` multiplies the sky by `1/old_exposure` (33x on Lorenz). Recorded as the ADR's `Outcome` |
| 0058 | Thirteen presets bind the fold and eleven had not chosen an edge treatment | Closed by content 2026-08-04, `859ec66` — all thirteen now name a `kaleido_edge`, the verdicts spread across all three treatments. **The entry named `attractor_dejong`, which binds no `kaleido_*` param; the thirteenth is `attractor_clifford`** — inherited from [Plan 0055](plans/done/0055-the-fold-edge-becomes-a-choice.md)'s own scope bullet, corrected in both |

### Added by the 2026-08-13 sweep

| # | Entry | Went to |
|---|-------|---------|
| 0009 | The `animation.rs` gate penalizes two legitimate designs | [ADR-0091](adrs/0091-the-animation-gate-scores-motion-against-the-figures-footprint.md) + [Plan 0077](plans/done/0077-the-quiet-sky.md) Phase 1. **The sparse half is delivered** — the gate scores `metrics::footprint_diff`, and the entry's own casualty is the proof (the rejected fifth-density Squall draft passes at 0.1049 where the whole-frame statistic read 0.0057). **The rotational-symmetry half was never fixable and is now documented rather than open**: a figure invariant under rotation by `2*pi/k` renders an *identical* image under it, so its frame difference is zero at every resolution. `docs/capturing.md`'s gate table carries both limits as of this sweep |
| 0055 | The attractor's shape vocabulary is "breathe and bend" | [ADR-0093](adrs/0093-attractor-tuples-are-content-with-per-tuple-framing.md) + [Plan 0079](plans/done/0079-the-attractor-learns-new-figures.md). **Closed 2026-08-13, both halves** — a curated per-family tuple roster carrying *measured framing*, and four measured morph paths out of twenty swept (four refused by measurement: a mid-walk tuple can collapse to a fixed point, which has no scale to render at) |
| 0056 | A user-authored preset lived outside the repo for six weeks | [ADR-0081](adrs/0081-the-content-lane-lands-presets-and-architect-curates-the-set.md) + [Plan 0067](plans/done/0067-the-curation-route.md). **Both halves closed.** The route exists — the content lane lands presets directly and `architect` curates the set at plan-close cadence — and the file itself came home (`3732fb4`, Phase 3) and was later retired with cohort three (`d92dcb2`), which is the route working rather than failing |
| 0059 | The backdrop does not colour through the shared palette | [ADR-0086](adrs/0086-the-backdrop-colours-through-the-preset-palette.md) + [Plan 0072](plans/done/0072-the-backdrop-joins-the-palette.md) |
| 0060 | An engine fix leaves its preset-side workarounds standing | [Plan 0067](plans/done/0067-the-curation-route.md) Phase 4 — the close-ceremony workaround grep is installed as step 3b and has run at every close since, reporting its result in the close notes even when it finds nothing |
| 0061 | `perspective` moves the figure far more than it enlarges it | [Plan 0075](plans/done/0075-the-content-renaissance.md) Phase 3, as documentation — the ~0.9x translational law and the ~0.3 practical ceiling. The re-centring option (2) had no demonstrated want and is not carried forward |
| 0062 | `depth_hue` is a lightness cue on a lightness ramp | [Plan 0075](plans/done/0075-the-content-renaissance.md) Phase 3, as documentation (the three regimes, the `2 * min(hue_center, 1 - hue_center)` wrap bound, the duotone deadness). **The wrap-versus-clamp question survives as [0075](#0075--root_tint-earned-no-binding-on-either-shipped-ifs-preset-and-root_hue-earned-both)'s item 2**, which is where the engine half of it belongs |
| 0063 | `spin`'s usable ceiling is set by `fade`, not by taste | [Plan 0075](plans/done/0075-the-content-renaissance.md) Phase 3, as documentation |
| 0064 | An IFS preset switch shows a hard-edged rectangle of noise | [ADR-0087](adrs/0087-the-ifs-particle-carries-its-age-and-its-last-map.md) + [Plan 0073](plans/done/0073-the-fern-unfurls-and-colours-by-what-made-it.md) — the continuous respawn, so the population is never a uniform box at any instant |
| 0065 | `morph` is a travel knob whose visible rate is steepest near zero | Documentation, `cf977f9`. Struck at the time; archived here |
| 0066 | The IFS figures are STILL, so the drift-rate conventions are wrong for them | Documentation, `cf977f9`. **Its one undischarged half is now done**: `docs/capturing.md`'s gate table states that a passing `anim` is not evidence of a *watchable* preset on a still family |
| 0067 | `depth_fade` is a uniform dimmer on every flat family | [Plan 0075](plans/done/0075-the-content-renaissance.md) Phase 2 — option 2, the true no-op, asserted by **byte equality** against a live Lorenz control so it cannot pass vacuously |
| 0070 | The in-frame geometry fraction cannot gate new content | [Plan 0075](plans/done/0075-the-content-renaissance.md) Phase 2 — the `geom` column, where the over-scale defect is actually introduced. The `sanity.rs`-shaped distribution report stays a candidate second step, deliberately not taken |
| 0072 | `sanity.rs`'s coverage floor forces thin-stroke line scenes into washed-out tuning | [Plan 0075](plans/done/0075-the-content-renaissance.md) Phase 1 — `metrics::radial_shell_occupancy` rescues a preset under its coverage floor at ≥ 4 occupied shells; the three retired mandalas at their honest tunings read 10/10/9, and the frozen renders-nothing defect reads 0 and still fails |
| 0074 | The age channel has nothing spatial to colour | [ADR-0088](adrs/0088-the-ifs-colours-by-distance-from-its-own-skeleton.md) + [Plan 0074](plans/done/0074-the-figure-colours-by-how-far-it-has-come.md) — route 2 (the channel that IS spatial) plus route 3 (`age_*` retired), so the roster did not grow |
| 0076 | The operator docs describe a fern tuning the shipped fern does not carry | Repaired at [Plan 0074](plans/done/0074-the-figure-colours-by-how-far-it-has-come.md)'s close |
| 0084 | The ink stage has no contrast lever | [ADR-0092](adrs/0092-the-ink-remap-gains-a-contrast-exponent.md) + [Plan 0078](plans/done/0078-the-ink-learns-to-bite.md). **The content half is standing, not open** — the two-header re-judge lives in [`content-brief.md`](content-brief.md) §2 |
| 0085 | `swarm` has no `reseed` | [Plan 0077](plans/done/0077-the-quiet-sky.md) Phase 3 — ADR-0066 disturbance semantics, never a box respawn. The minutes-horizon caveat it insisted on is honoured and stays live as [0086](#0086--no-capture-path-reaches-the-minutes-long-horizon-so-a-slow-accumulation-failure-is-invisible-to-every-instrument) |
| 0088 | `shot --report`'s band columns cannot see reactivity spent on bloom | [Plan 0077](plans/done/0077-the-quiet-sky.md) Phase 4 — the mean columns keep their meaning and a footprint reading lands beside them. Third member of a family the project has now fixed three times (0022, 0028, this) |
| 0091 | There is no static, screen-anchored, oriented gradient | [ADR-0094](adrs/0094-the-backdrop-paints-a-directional-ramp.md) + [Plan 0080](plans/done/0080-the-sky-gets-a-horizon.md) |

### Added by the third batch, 2026-08-13

| # | Entry | Went to |
|---|-------|---------|
| 0077 | The doc-link gate is blind to reference-style links | [Plan 0084](plans/done/0084-two-gates-stop-lying-about-what-they-check.md) Phases 1-2. Two new break classes beside the inline one, and **the narrowing that makes them usable was measured rather than assumed** — a shortcut use is reported only when some file in the tree defines that label, without which the repo yields 31 findings of which 24 are prose brackets. It proved itself at its own close, naming all four links the `git mv` into `done/` broke. **One thing left undone:** the fixture tree the phase described was run ad-hoc and never committed, so the script's optional `root` argument has no caller in the repo and the bite check is unrepeatable |
| 0080 | The reactivity gate renders warm-up frames it throws away | [Plan 0084](plans/done/0084-two-gates-stop-lying-about-what-they-check.md) Phases 3-4. `capture_audio_after_warmup` advances the analyzer without rasterizing; **136.3 s -> 100.2 s over 36 presets** on this box's DX12 software adapter. **The entry's premise was half wrong and the correction outlives the speedup** — the warm-up renders were also the *scene* warm-up, so 35 of 36 per-band vectors moved. Read any reactivity figure recorded before 2026-08-13 as a different measurement, not as drift. **What has no instrument:** GPU-integrated scene state now meets the measured window colder, documented in three places and asserted in none |
| 0090 | The Mac build's capture verdict is stderr-only | [Plan 0083](plans/done/0083-the-build-says-why-it-hears-nothing.md). The verdict is a value reaching both artifacts a remote tester can send — a `capture` column on every `diagnostics.log` row and an `audio` line under F3 — with a column rather than a startup line, because the log rotates and a line written once is what rotation deletes. **The capability is what closed; the tester's own answer is that plan's `human` Phase 5**, standing in the plans README, and it is recorded here when it arrives |

## Open entries


---

## Entry 0021 — from the Plan 0038 / ADR-0040 ruling

Not from the content lane. Raised by an `architect` ruling that had to falsify a claim in order to
answer a `dev` finding, and left a real want with nowhere to live.

---

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

---

## Entry 0032 — from the Plan 0049 Phase 5 sample-rate sweep

---

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

---

## 0038 — mid-tone-dominated presets lost ~8 % luminance to the tonemap knee, and the library has not been retuned

- **STILL OPEN 2026-08-13, and a document that says otherwise is wrong.**
  [`docs/plans/README.md`](plans/README.md)'s Plan 0080 Phase 7 write-up states that *"the
  **tonemap-knee** half of that pairing is now measured away."* **It is not.** What Plan 0080 Phase 7
  retired is a *different* suspicion raised at its own close — that `bg_bright = 0.85` was reaching
  the tonemap's shoulder on the **backdrop ramp** — settled by finding 0 % of the scanned column
  rail-pinned on any channel in any of the three probes. That measurement is about a backdrop
  gradient. **This entry is about mid-tone figure luminance on attractor presets**, measured as
  `attractor_clifford` 82.54 → 75.91 mean luma, and no backdrop measurement speaks to it. The two
  were conflated because both mention the tonemap.
- **Re-verified 2026-08-13:** exactly **one** shipped preset binds `exposure`
  (`lsystem_vellum.toml:60`), so the lever this entry names as the one-line fix is still essentially
  unused across the library.
- **ROUTED, and now scheduled:** it is §4 of [`content-brief.md`](content-brief.md), paired with Plan
  0071's standing `occlude` retune as one pass over the shipped set. That brief also records the
  other correction this entry's routing carries — the plan text says to run it "with 0038 and 0058",
  but **0058 closed by content on 2026-08-04**, five days before Plan 0071 reached Phase 5, so the
  three-way pass is a two-way pass.
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

---

## 0042 — the downbeat estimator locks on ~3 % of audible time, so the gated bar variables are almost always fallback

- **PROMOTED AGAIN 2026-08-13 → [ADR-0097](adrs/0097-the-downbeat-cue-is-chosen-against-per-beat-evidence.md) +
  [Plan 0086](plans/0086-the-downbeat-finds-a-cue-that-is-not-the-kick.md)** — the **repair**, which
  the 2026-08-09 answer below explicitly left unwritten. The plan does not open by building the cue:
  ADR-0082's own `Outcome` records that the 1 Hz log carries **band levels, not per-beat accents**,
  so "the accent feature is the cause" is a ladder match plus a construction argument. Phase 1
  therefore builds the per-beat `DownbeatTerms` capture — the decomposition Plan 0068 wrote and
  nothing outside its tests has ever called — and the cue is chosen at a `human` gate from a ranked
  shortlist, because at least three failures fit the same evidence (a narrow accent, a 2-periodic
  degeneracy where alignments 0 and 2 tie, or a thin history window) and they want different
  repairs. `CONFIDENCE_THRESHOLD` still does not move.
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

---

## Entries 0068-0069 — from Plan 0070's close (2026-08-05)

---

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
  README.

### Option 2 — PROMOTED 2026-08-13, by interview

- **→ [ADR-0104](adrs/0104-the-emitters-source-is-authorable-geometry.md) +
  [Plan 0090](plans/0090-the-emitters-source-moves.md).** Four scalars, every default an exact
  arithmetic identity: `source_y` (which **may sit inside the frame** — the interview took that
  trade deliberately, because clamping it below is the decision that keeps the emitter unusable for
  any slow look), `source_width` (**fractional**, so `aspect * 1.0` is bit-for-bit today's value; `0`
  is a point source, which falls out rather than being its own concept), `spawn_fade`, and `prewarm`.
- **The internals were already the right shape**, which is why this is smaller than the entry implies:
  `source_half_width` is a real field on `Spawn` (`emitter.rs:357`) assigned `self.aspect`
  unconditionally, and the spawn site already multiplies a unit draw by it. One constant and one
  assignment.
- **This entry named ONE warm-up and there are TWO, which is the finding the promotion adds.** Moving
  the source into the frame removes the *travel* warm-up this entry measured (2.12 units against a
  0.5 s capture) and leaves the *population* warm-up untouched: the pool starts empty and fills at
  `spawn_rate`, so Perseids' own numbers put ~100 of ~560 objects on screen at 0.5 s — **about
  18 %** — wherever `source_y` is. `prewarm` is Plan 0090 Phase 3 and is the phase that actually
  makes a slow world gateable; it is also the plan's designed cut point, being beyond what the
  interview covered.
- **The entry's refusal is honoured**: no gate's capture length, floor or statistic moves. The
  warm-up is attacked instead of the instrument.
- **CORRECTION — this entry's own claim that the Perseids look was "routed out of the cohort rather
  than shipped" is stale.** `presets/emitter_perseids.toml` exists on `system = "emitter"` at
  `launch_speed = 2.6`: it shipped as exactly the fast-shower compromise this entry predicted would
  be the only reachable form. What did not ship is the quiet version, which is Plan 0090 Phase 5.
  The demonstrated want is therefore stronger than the entry records, not weaker.

---

---

## 0069 — there is no way to draw a two-tone object (a fill with a contrasting outline), because the composite is additive

> **CORRECTED 2026-08-13 — the title and the mechanism below are FALSE for field scenes, and have
> been since 2026-08-11. This entry stays live for its other half only; do not act on the paragraph
> that follows without reading this box first.**
>
> The claim "black adds zero, so a dark edge cannot exist inside the composite" was written
> **2026-08-05**. The layer system landed **2026-08-11** ([ADR-0090](adrs/0090-a-preset-composes-two-scene-layers.md)),
> six days later, and three closes ran in between without anyone revisiting this. `layer_blend.rs`
> gives `multiply`, which **strictly darkens**, and `fragment_field.rs:168` shows a fullscreen field
> emitting alpha = `occlude` — **1 by default on every pixel, including black ones** — which is the
> coverage a darkening blend needs.
>
> **Measured 2026-08-13**, one preset, `blend` the only variable, 640x360 on this box's hardware
> adapter: `multiply` reaches **min luma 18.5** with **61.9 %** of pixels below 64; the `add`
> control **cannot get below 181.6** anywhere, with **0.0 %** below 64. Three tones coexist in the
> multiply frame. Full derivation and costs in
> [ADR-0106](adrs/0106-two-tone-graphics-come-from-a-multiply-layer.md); the route gets written down
> in [Plan 0091](plans/0091-the-figure-fills-the-frame.md) Phase 1, which also measures the one path
> still open — whether multiply reaches the **backdrop**, which sits outside the chain's input
> (`post.rs:33`) and is therefore a separate question.
>
> **What survives, and it is why this entry is corrected rather than archived:** multiply darkens in
> proportion to *coverage*, and **nothing in this engine still decides what is in front of what**. A
> shaped object that occludes another figure is unbuilt. The mechanism below is also still exactly
> right for **particle** scenes — a particle's alpha *is* its falloff, so a black sprite has no
> coverage and cannot darken anything. That asymmetry between the two routes to one shape is now a
> documented authoring trap rather than an engine limit.

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

---

## 0071 — the scalloped boundary was chosen as a real curve primitive, and the engine has none

- **PROMOTED 2026-08-13 → [ADR-0098](adrs/0098-the-line-renderer-draws-arcs-as-per-pixel-distance-fields.md) +
  [Plan 0087](plans/0087-the-line-renderer-draws-a-curve.md) Phase 6**, jointly with
  [0073](#0073--motif-outlines-show-their-vertices-and-a-sampled-polyline-does-not-read-as-a-curve).
  The user's decision has had no route for a week, and the reason it now has one is that it stops
  being its own feature: **a closed scalloped outline is a chain of circular arcs**, so on the arc
  primitive ADR-0098 adds it is a roster entry rather than a new mechanism. `star.rs:599`'s standing
  note — *"the engine does not have [one]. Nothing here fakes one"* — is what that phase replaces.

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

---

## 0073 — motif outlines show their vertices, and a sampled polyline does not read as a curve

- **PROMOTED 2026-08-13 → [ADR-0098](adrs/0098-the-line-renderer-draws-arcs-as-per-pixel-distance-fields.md) +
  [Plan 0087](plans/0087-the-line-renderer-draws-a-curve.md)**, with
  [0071](#0071--the-scalloped-boundary-was-chosen-as-a-real-curve-primitive-and-the-engine-has-none)
  riding it. The route is a **circular-arc instance whose stroke is a per-pixel signed distance** —
  no vertices at any resolution — with non-circular outlines expressed as a **G1-continuous biarc
  chain**, because a polyline shows its joints for being only C0 and tangent continuity is what makes
  the same handful of pieces read as a drawn curve.
- **THIS ENTRY'S OWN HEDGE IS RESOLVED, and it resolves against the entry's first reading.** It asked
  someone to *"verify against what Plan 0040 actually landed before assuming joins are absent"*.
  Verified 2026-08-13: **the joins are present and working exactly as
  [ADR-0041](adrs/0041-line-joins-are-per-endpoint-on-the-segment-instance.md) specifies.** Each
  joined endpoint extends its quad backward or forward by the half-width
  (`core/src/render/scenes/lines/renderer.rs:129`), so adjacent quads *deliberately* overlap by half
  a stroke — and the composite is additive, so that overlap **sums**. The bead is the join mechanism
  working, not failing, which is why no amount of join work would have removed it and why raising the
  sample count makes it worse per unit length. Faceting is the separate defect
  (`star.rs:665` fixes vertex count per motif with no authorable resolution), and the two are not
  both reachable from one lever.
- **What the promotion does not promise:** the bead is **reduced, not removed**. A `circle` goes from
  `SMOOTH_SAMPLES` joints to zero; a rose keeps as many as it has lobes, and those still overlap
  additively. If that still reads badly, the remaining route is
  [0069](#0069--there-is-no-way-to-draw-a-two-tone-object-a-fill-with-a-contrasting-outline-because-the-composite-is-additive)'s
  composite question — and Plan 0087 Phase 4 is a `human` gate placed *before* the expensive half
  precisely so that answer arrives cheaply.

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

---

## Entries 0075-0076 — from the Plan 0074 Phase 6 content pass (2026-08-08), binding the root channel

---

## 0075 — `root_tint` earned no binding on either shipped IFS preset, and `root_hue` earned both

- **ITEM 2 DECIDED 2026-08-13 → [ADR-0102](adrs/0102-a-palette-coordinates-edge-is-a-per-preset-choice.md),
  proposed, and deliberately with no plan.** The entry asked whether to clamp the palette coordinate
  rather than repeat it. The ADR's finding is that **no per-param answer exists**: there is exactly one
  coordinate, it is a *sum* of contributors of two kinds — angles, where wrapping is correct, and
  distances, where it is a discontinuity at the quantity's own floor — and a sum cannot carry two
  addressing behaviours. So the edge becomes a **per-preset `[palette]` choice**, wrap by default
  (zero pixels move), implemented as a shader `select` rather than a second sampler, clamped to the
  **texel-centre range** because linear filtering blends texel 255 into texel 0 at exactly 0 and 1.
  **Verified against code 2026-08-13:** one sampler, `AddressMode::Repeat` on `u`
  (`core/src/render/palette.rs:332`), and its docstring gives the cyclic justification.
- **No plan, by the user's call** — the want is real, no shipped content is wrong, and both presets
  bind the route that works. This entry stays live because the design has not landed, which is the
  lifecycle working rather than a miss. **Item 1 remains closed** (the authoring half landed at Plan
  0074's close) and **item 3 is not a defect**.

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

---

## Entries 0078-0079 — from Plan 0064's Phase 4 and Phase 6 (2026-08-09), the symmetry stage

---

## ~~0078 — `kaleido_tile` is a discrete quantity that is not quantized, so it is the one term of the composed map an author cannot bind~~ — FALSIFIED

> **CORRECTED 2026-08-13, at the backlog sweep. The premise is false, and it was false when the
> entry was written.** `kaleido_tile` is not quantized **on purpose**, and the reasoning is a doc
> comment on the function itself (`core/src/render/kaleidoscope.rs:458`):
>
> > *Deliberately **not** rounded, unlike the fold order and the winding number. Those two are
> > integral because a fractional value is undefined or torn; a fractional cell count is neither.
> > `abs(fract(x·n/2)·2 − 1)` at `n = 2.5` is a perfectly continuous mirrored grid whose last cell is
> > cut off at the frame edge, so a smoothed `kaleido_tile` can ease between cell counts instead of
> > snapping — the one param on this stage where that is true.*
>
> That comment landed in `e648a02`, **Plan 0064 Phase 1** — five phases before this entry was filed
> at Phase 6 of the same plan. So the entry's central claim, that `tile` is "the same kind of
> quantity" as `kaleido_spiral` and `palette_steps`, is answered in place: those two are rounded
> because a fractional value is *meaningless*, and a fractional cell count is not.
>
> **What survives is smaller and is a look question, not a correctness one.** A fractional count
> genuinely does cut the last cell at the frame edge, so the wallpaper is seamless *within* the
> frame and clipped at its border. Whether that reads as a broken tiling or as an ordinary crop is
> a **render** judgement nobody has made — the entry asserts the first without having looked. If a
> content pass binds `kaleido_tile` to audio and the clipped edge reads badly, **that** is a fresh
> entry with a rendered pair behind it, and it would be arguing for a *clamp at the edge*, not for
> rounding.
>
> **The one thing worth acting on is a doc gap, and it is one line.** `fragment_tiled.toml` binds
> `kaleido_tile = "2"` as a constant and `presets/README.md` says nothing about whether it may be
> driven — so an author meets neither the capability nor its edge behaviour.
>
> **PROMOTED 2026-08-13 → [Plan 0089](plans/0089-the-framing-contract-stops-lying.md) Phase 2**, and
> the promotion is the point: "folded into the next plan that touches the symmetry stage's docs" was
> written here and **no such plan was ever written**, so the item sat with a named home and no carrier.
> The same was true of [0081](#0081--the-house-gain-rule-lives-only-in-preset-headers-the-first-half-is-falsified-the-exception-class-survives)'s
> survivor, which rides the same plan. Judging whether the clipped last cell *reads* badly is
> explicitly **not** in that phase's scope — nobody has looked, and a render is what would decide it.
>
> **The method note, which is why this is corrected in place rather than archived:** the entry is a
> claim about what the repo does *not* do, and it was checked against the param roster rather than
> against the function. "Nothing does X" rots exactly like "X is broken" does. Everything below is
> the entry as raised.

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

---

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

---

## 0081 — the house gain rule lives only in preset headers *(the first half is FALSIFIED; the exception class survives)*

> **CORRECTED 2026-08-13, at the backlog sweep.** The entry's central claim — *"It is written down
> nowhere. It is not in `presets/README.md`, not in `docs/presets.md`"* — **is false, and was false
> when the entry was written.** `presets/README.md:203` carries it, inside the "A GAIN can be wrong
> the same way" bullet:
>
> > *The rule that came out of the retune: pick `G = C / 0.85` for `bass`/`mid` and `C / 0.60` for
> > `treb`/`onset`, which puts a typical passage near half the cap and a peak at it.*
>
> It landed in `fc698cd` on **2026-08-03** — six days before this entry was raised on 2026-08-09. So
> the rule is not folklore; it is documented, in the file the `preset-author` lane treats as the
> authority, in the paragraph an author reads when composing a clamped band term.
>
> **The second half of the entry stands, unchanged and unbuilt.** The *exception class* is nowhere:
> a param whose cap is a **failure state** rather than a maximum wants its range pulled in at both
> ends instead of gained to reach the cap. Gray-Scott `feed`/`kill` is the worked example — gains
> derived by the house rule put the field in the filled regime, where the gaps close and the picture
> becomes a flat wash with no contour left to draw, and the Coral Oracle's author found this by
> rendering it as flat mustard. Naming the **class** is what makes it useful beyond one preset, and
> it is unlikely to be the only member.
>
> **So this is now a one-paragraph doc item, not a rule-plus-exception item**, and it is half the
> size the entry claims.
>
> **PROMOTED 2026-08-13 → [Plan 0089](plans/0089-the-framing-contract-stops-lying.md) Phase 3.** "It
> goes into the next plan that touches `presets/README.md`'s reactivity section" was written here and
> no such plan followed, which is the same no-carrier failure as
> [0078](#0078--kaleido_tile-is-a-discrete-quantity-that-is-not-quantized-so-it-is-the-one-term-of-the-composed-map-an-author-cannot-bind)'s
> survivor; the two ride one plan. **Re-verified against code 2026-08-13** — neither
> `presets/README.md` nor `docs/presets.md` contains any "failure state" or "death state" language, so
> the exception class is still nowhere.
>
> Same method note as [0078](#0078--kaleido_tile-is-a-discrete-quantity-that-is-not-quantized-so-it-is-the-one-term-of-the-composed-map-an-author-cannot-bind): both entries assert an *absence*, and neither absence was
> checked. Everything below is the entry as raised.

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

---

## 0082 — the quality governor reads `frame_ms_p99`, and a preset switch spikes p99 to 25 ms while nothing is dropped

- **PROMOTED 2026-08-13 → [ADR-0099](adrs/0099-the-show-length-horizon-is-a-spot-check-and-it-splits-in-two.md) +
  [Plan 0085](plans/0085-the-show-length-horizon-gets-an-instrument.md) Phases 3-4**, jointly with
  [0083](#0083--rss-grew-385-to-663-mb-over-three-minutes-of-preset-switching-and-there-is-no-no-feedback-control-to-compare-it-against)
  and [0086](#0086--no-capture-path-reaches-the-minutes-long-horizon-so-a-slow-accumulation-failure-is-invisible-to-every-instrument):
  three entries about the show-length timescale nothing in this repo measures. **The plan does not
  choose among this entry's three candidate responses** — that stays R0's decision, exactly as
  written below. What it does is add a steady-state frame-time column beside `p99` and record the
  qualification where R0's designer will meet it before they meet the column, which is this entry's
  own "the cheapest time to know is before the governor exists".

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

---

## 0083 — RSS grew 385 to 663 MB over three minutes of preset switching, and there is no no-feedback control to compare it against

- **PROMOTED 2026-08-13 → [ADR-0099](adrs/0099-the-show-length-horizon-is-a-spot-check-and-it-splits-in-two.md) +
  [Plan 0085](plans/0085-the-show-length-horizon-gets-an-instrument.md) Phase 5** — the measurement
  this entry asks for, not a fix, and it stays a `human` phase because it needs the live app on a
  real machine for a real duration. One thing the plan adds that this entry did not ask for and
  needs: **`--soak` has no notion of a preset switch**, which is the axis the whole question turns
  on, so a `switches` column lands first (Phase 3) and the three runs read against it. **Either
  answer closes this entry** — the entry's complaint is the missing control, not the number.

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

---

## 0086 — no capture path reaches the minutes-long horizon, so a slow-accumulation failure is invisible to every instrument

- **PROMOTED 2026-08-13 → [ADR-0099](adrs/0099-the-show-length-horizon-is-a-spot-check-and-it-splits-in-two.md) +
  [Plan 0085](plans/0085-the-show-length-horizon-gets-an-instrument.md) Phases 1-2** — and promoted
  **as the shape this entry specified**, not as the gate it refused: a headless `shot` mode rendering
  N simulated minutes at capture cadence and reporting drift statistics, run by the lane on worlds
  with an accumulation axis, verdict in the world's header. The ADR adds one thing the entry did not
  separate: **this half is deterministic and reproducible headless** (forces integrate against
  injected `dt`), while [0083](#0083--rss-grew-385-to-663-mb-over-three-minutes-of-preset-switching-and-there-is-no-no-feedback-control-to-compare-it-against)'s
  RSS growth comes from live GPU resource churn no headless loop reproduces — so they share a
  motivation and **not** an instrument, and one harness for both would be blind to two of the three.
  The 2026-08-11 park-with-a-trigger below is discharged by the trigger having fired.

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
[0080](design-backlog-archive.md#0080--the-reactivity-gate-pays-18x-to-render-frames-it-throws-away-because-warm-up-and-measurement-share-one-capture-path)
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

---

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

---

## 0089 — the dragon overruns the frame corner at the default view, and `FRAME_FILL = 0.88` promises it cannot

- **PROMOTED 2026-08-13 → [ADR-0103](adrs/0103-the-ifs-fit-frames-a-figure-that-does-not-turn.md) +
  [Plan 0089](plans/0089-the-framing-contract-stops-lying.md) Phase 1.** The correction below named
  **rotation** as the leading candidate; it is now **derived**, and the derivation makes this entry
  much larger than the dragon. `fit_scale` fits the axis-aligned half-extents; a centred AABB rotated
  by `θ` reaches `sqrt(hx²+hy²)`, so with `a = hx/hy` the guarantee holds at every angle only if
  `a <= sqrt(1/FRAME_FILL² − 1) = 0.5397` (vertical-binding) and is **unsatisfiable** horizontal-binding
  at any aspect >= 1. **A square figure overruns by 24.4 %**; only a figure 1.85x taller than wide is
  safe. The fern (`a ~ 0.48`) is the sole compliant shipped figure **and the one the fit was built
  on**, which is why nobody saw it — while **all three** 2D-IFS presets independently bind `spin` down
  *and* set base `zoom` below 1 (0.92 / 0.96 / 0.96), so the library has been paying in triplicate with
  only one header naming why.
- **The entry's own second candidate stays live as the plan's redirect.** If Phase 1's non-vacuity
  check finds the dragon *inside* the bound, rotation is not the mechanism and the finding is the
  preset's own `zoom` reaching `1.04` at a bass peak. The plan says so rather than tuning the test
  until the expected figure fails.
- **What the promotion does not do:** it does not buy the guarantee. The routes that would — fitting
  the rotation-invariant radius, or a per-figure measured fill — are **priced and deferred with a
  trigger** in ADR-0103, because both re-frame all three shipped worlds on top of compensating `zoom`
  values they already carry. What lands is a contract that is true, pinned as a property test, moving
  zero pixels.

> **CORRECTION 2026-08-13, at the backlog sweep — the entry's own first suspect is largely
> exonerated, and it would have sent `dev` hunting in the wrong file.** The entry says *"the
> suspicion to check first is `FitLut` against the fallback `frame()`."* But
> `core/src/render/scenes/particles/ifs/tests.rs:1064`
> (`the_fit_leaves_margin_for_what_it_under_measures`) **already asserts the property this entry
> says is falsified**, for every shipped figure and three morph points on two pairs: it compares the
> fit's sampled half-extent against a 200 000-iteration long run and requires the true figure to
> fill under `0.97` of the frame, with a non-vacuity check that the under-measure is real. That test
> is green, and it covers the dragon.
>
> **So the suspect should be what the fit does not model, and the leading candidate is rotation.**
> The fit measures an **axis-aligned** bounding box; the view transform then **rotates** the figure.
> An axis-aligned box that rotates sweeps its corners outside — which is a *corner* overrun
> specifically, matching the entry's own wording, and it would be invisible to a per-axis fill test
> by construction. `presets/attractor_dragon.toml:117` binds `spin = "sin(time * 0.24) * 0.26"`, so
> the shipped world does rotate. Its `zoom` line directly below is the `0.92` workaround.
>
> A second candidate worth eliminating in the same sitting: the same file's `zoom` reaches
> `0.92 + bar * 0.04 + clamp(bass * 0.094, 0, 0.080)` — **1.04 at a bass peak**, past 1.0 — so part
> of what was seen may be the preset zooming in rather than the fit mis-measuring.
>
> **This does not close the entry** — a stated invariant is still falsified and this project does not
> leave one standing. It redirects it, and it keeps the entry's own good instruction: do not close
> this by re-documenting the invariant away without knowing which mechanism breaks it.

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

