# Design backlog — captured feedback, not yet promoted

Short, durable notes for design gaps surfaced during work but **not yet** decided into an ADR or
plan. Chiefly the `preset-author → architect` feedback handoff (a look wanting something the
preset grammar or engine can't express), plus any other "worth remembering, not worth acting on
yet" finding.

An entry here is **not** a commitment to build — it is a captured signal so the friction isn't
lost between sessions. Verify every entry against the code before acting on it — these are dated
snapshots, and the surface moves (same rule the lanes apply to their own references).

## Every live entry carries a probe, and something re-runs it

That rule above used to live only in this paragraph, and four entries were falsified anyway (0052,
0078, 0081, 0082 — two of them wrong on the day they were written). So since
[ADR-0108](adrs/0108-a-backlog-claim-about-the-repo-carries-an-executable-probe.md) the mechanical
half of a verification moves out of the sentence and into something a script can re-run:
`scripts/check-backlog-claims.mjs`, at the three call sites the doc-link checker already occupies
(`pre-push`, the architect close ceremony, and the CI `links` job, which is the un-bypassable one).

**Write a dated bullet with the claim's reduction inside an inline-code span**, in one of three
forms. `<path>` is a file or a directory from the repo root; `<regex>` is JavaScript regex source,
so a literal dot needs escaping. **This is enforced, not advised** — since Plan 0094 a live `##`
entry with no dated bullet beneath it reds the gate at its own line, because "every live entry
carries a probe" was the half of the rule a checker built out of the bullets it finds could not see:

```markdown
- **Verified 2026-08-15** — the governor does not exist: `absent: sustained_miss in: core/src`
- **Verified 2026-08-15** — the rule is documented, and here: `present: G = C / 0\.85 in: presets/README.md`
- **Verified 2026-08-15** — `unprobeable: this is a judgement about rendered output, not a claim
  about repo contents`
```

(Those three are examples and the first is deliberately false today, which is why the checker skips
fenced blocks — a document describing the grammar is not making a claim.)

Three things to know before writing one:

- **Green means the stated reduction still holds, never that the entry is true.** Entry 0081's
  stamp was dated, recent and accurate, and verified the half of the entry that survived rather
  than the half in its own title. A probe only checks the reduction its author chose, and whether
  that reduction covers the claim is something a reader has to see — which is why the probe sits
  beside the claim rather than in a manifest.
- **Prefer the narrowest path that carries the claim** (`core/src/render/tier.rs`, not `core/src`).
  A narrow path keeps the staleness advisory quiet, and it is a better probe for the same reason.
- **`absent:` on a common word is a probe that can never fail**, and it reads as verification while
  checking nothing. If a claim has no honest reduction, say so with `unprobeable: <why>` — the
  checker accepts it, prints every one of them in its advisory, and that visible, countable roster
  is what keeps the opt-out from becoming a blanket.

**Staleness is an advisory and never a failure.** After the pass/fail line the checker asks
`git log` when each probed path was last touched and names the entries whose subject has moved
since anyone read them. It cannot tell a commit that invalidates a claim from one that does not, so
making it a gate would mean firing constantly at a broad path or saying nothing at a narrow one —
it is a report instead, and it never changes the exit code. It also **withholds itself rather than
guessing**: on a shallow clone — which is what the CI `links` job checks out — `git log -1` returns
the tip commit for every path, so the block prints a notice in place of a roster it did not measure.
The pre-push hook and the close ceremony run on a full checkout and get the real reading.

**The archive is out of scope and stays out.** An archived entry is a closed record whose value is
the correction it carries; re-probing it would be checking history against the present.

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

**First measurement, 2026-08-15:** it held. [Plan 0089](plans/done/0089-the-framing-contract-stops-lying.md)'s
close ran step 3c off its own `**Closes:**` header and archived all three entries in the close
commit — the first time a body moved at the close that discharged it rather than at a sweep weeks
later. One close is not a trend; the value of recording it is that the next sweep can tell a
working rule from a lucky one.

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
| 0085 | `swarm` has no `reseed` | [Plan 0077](plans/done/0077-the-quiet-sky.md) Phase 3 — ADR-0066 disturbance semantics, never a box respawn. The minutes-horizon caveat it insisted on is honoured, and the entry it kept alive — [0086](design-backlog-archive.md#0086--no-capture-path-reaches-the-minutes-long-horizon-so-a-slow-accumulation-failure-is-invisible-to-every-instrument) — **closed 2026-08-15** with `shot --horizon`, which ran on this world first and found the collapse repaired |
| 0088 | `shot --report`'s band columns cannot see reactivity spent on bloom | [Plan 0077](plans/done/0077-the-quiet-sky.md) Phase 4 — the mean columns keep their meaning and a footprint reading lands beside them. Third member of a family the project has now fixed three times (0022, 0028, this) |
| 0091 | There is no static, screen-anchored, oriented gradient | [ADR-0094](adrs/0094-the-backdrop-paints-a-directional-ramp.md) + [Plan 0080](plans/done/0080-the-sky-gets-a-horizon.md) |

### Added by the third batch, 2026-08-13

| # | Entry | Went to |
|---|-------|---------|
| 0077 | The doc-link gate is blind to reference-style links | [Plan 0084](plans/done/0084-two-gates-stop-lying-about-what-they-check.md) Phases 1-2. Two new break classes beside the inline one, and **the narrowing that makes them usable was measured rather than assumed** — a shortcut use is reported only when some file in the tree defines that label, without which the repo yields 31 findings of which 24 are prose brackets. It proved itself at its own close, naming all four links the `git mv` into `done/` broke. **One thing left undone:** the fixture tree the phase described was run ad-hoc and never committed, so the script's optional `root` argument has no caller in the repo and the bite check is unrepeatable |
| 0080 | The reactivity gate renders warm-up frames it throws away | [Plan 0084](plans/done/0084-two-gates-stop-lying-about-what-they-check.md) Phases 3-4. `capture_audio_after_warmup` advances the analyzer without rasterizing; **136.3 s -> 100.2 s over 36 presets** on this box's DX12 software adapter. **The entry's premise was half wrong and the correction outlives the speedup** — the warm-up renders were also the *scene* warm-up, so 35 of 36 per-band vectors moved. Read any reactivity figure recorded before 2026-08-13 as a different measurement, not as drift. **What has no instrument:** GPU-integrated scene state now meets the measured window colder, documented in three places and asserted in none |
| 0090 | The Mac build's capture verdict is stderr-only | [Plan 0083](plans/done/0083-the-build-says-why-it-hears-nothing.md). The verdict is a value reaching both artifacts a remote tester can send — a `capture` column on every `diagnostics.log` row and an `audio` line under F3 — with a column rather than a startup line, because the log rotates and a line written once is what rotation deletes. **The capability is what closed; the tester's own answer is that plan's `human` Phase 5**, standing in the plans README, and it is recorded here when it arrives |

### Added at Plan 0089's close, 2026-08-15

| # | Entry | Went to |
|---|-------|---------|
| 0078 | `kaleido_tile` is a discrete quantity that is not quantized — **premise was false** | [Plan 0089](plans/done/0089-the-framing-contract-stops-lying.md) Phase 2, as documentation. The param is deliberately **not** rounded and `fold_tile`'s doc comment said so five phases before the entry was filed; what survived was the doc gap the entry named precisely, and `presets/README.md` now carries both facts — that `kaleido_tile` is the one param on the symmetry stage an author may ease between values, and that a fractional count leaves the border cell **cut off at the frame edge**. Judging whether that clipped edge *reads* badly stays unasked, because nobody has rendered it |
| 0081 | The house gain rule lives only in preset headers — **first half was false** | [Plan 0089](plans/done/0089-the-framing-contract-stops-lying.md) Phase 3, as documentation. The rule itself had been in `presets/README.md` since 2026-08-03; the **exception class** was the survivor and is now named — *a param whose cap is a failure state rather than a maximum*, treated by pulling the range in at **both** ends rather than gaining to reach the cap, worked through Gray-Scott `feed`/`kill`. **The plan's shipped instance was wrong and `dev` corrected it at implementation:** it named `chthonic_coral_oracle.toml`, retired with cohort three on 2026-08-10 (`d92dcb2`), three days before the plan was written — the coral is right as *provenance* and wrong as a file, so the three shipped `reaction_*` presets carry the instance and `reaction_etching`'s `feed` line is quoted as the treatment |
| 0089 | The dragon overruns the frame corner, and `FRAME_FILL = 0.88` promises it cannot | [ADR-0103](adrs/0103-the-ifs-fit-frames-a-figure-that-does-not-turn.md) + [Plan 0089](plans/done/0089-the-framing-contract-stops-lying.md) Phase 1. **Closed 2026-08-15, and it was never one figure's bug.** The fit measures an axis-aligned box and `project` rotates it at `spin`'s default, so only a figure at or under `sqrt(1/FRAME_FILL² − 1)` stays inside at every angle — measured over the roster, **only the fern complies** (`a = 0.4851`), and sierpinski/tree/dragon/spiral overrun by 34/41/58/79 %. The contract was restated to what it guarantees, pinned as a property test asserted against the shipped `fit_scale`, with **zero pixels moved**. The guarantee itself is **deferred with a trigger, not bought** — the entry's own second candidate (the preset's `zoom` reaching 1.04) did not fire, and all three 2-D IFS presets' bindings peaking above 1.0 is now a content-lane sitting |

### Added at Plan 0085's close, 2026-08-15

| # | Entry | Went to |
|---|-------|---------|
| 0082 | The quality governor reads `frame_ms_p99`, and a preset switch spikes it to 25 ms — **the premise was false** | [ADR-0099](adrs/0099-the-show-length-horizon-is-a-spot-check-and-it-splits-in-two.md) + [Plan 0085](plans/done/0085-the-show-length-horizon-gets-an-instrument.md) Phases 3-4. **Closed 2026-08-15**, and the qualification landed in all three places a governor design starts from, with the three candidate responses named and deliberately not chosen. **But this entry described a governor that does not exist.** R0 was **not** unbuilt — [Plan 0044](plans/done/0044-quality-tiers.md) / [ADR-0045](adrs/0045-quality-tiers-floor-and-rich.md) shipped it on 2026-07-30, ten days *before* this entry was raised — and the shipped `sustained_miss` **never reads p99**: it needs 75 % of at least 180 raw frame times past `budget * 1.25`, which a switch's handful of slow frames in a 240-sample ring cannot approach. So the built governor already landed this entry's *second* candidate response, independently, as a miss fraction. The hazard is real and lives in the **prose**; a revisit starting from the old description would build it. Same failure mode as 0078 and 0081 — a claim about the repo, rotting the way claims about the repo do |
| 0086 | No capture path reaches the minutes-long horizon | [ADR-0099](adrs/0099-the-show-length-horizon-is-a-spot-check-and-it-splits-in-two.md) + [Plan 0085](plans/done/0085-the-show-length-horizon-gets-an-instrument.md) Phases 1-2. **Closed 2026-08-15** as the shape the entry specified rather than the gate it refused: `shot --horizon <minutes>`, run by the lane, verdict in the world's header, both determinism properties asserted on rendered pixels and a static control reading `delta 0.0000` as the non-vacuity half. **The named subject came back clean** — `swarm_shatter` wanders 0.197-0.384 across ten minutes with no trend, its collapse repaired by [Plan 0077](plans/done/0077-the-quiet-sky.md)'s `reseed` — and the instrument convicted a world nobody suspected instead: `attractor_ink`, coverage 0.199 -> 0.002 with the **silhouette intact and the density gone**, recorded in that header and deliberately unrepaired. **One bound the entry could not anticipate:** the headless path dies at 3,601 frames on both RD worlds, so those two rows are 0.5 minutes and their `monotone 1.00` is settling, not drift — filed as [0093](#0093--the-headless-capture-path-dies-past-a-few-thousand-frames-so-the-horizon-cannot-reach-its-own-headline-length) |

### Added at Plan 0090's close, 2026-08-15

| # | Entry | Went to |
|---|-------|---------|
| 0068 | A swarm mark has no per-mark variation, and the one scene that could hold a starfield could not reach a slow one | [ADR-0104](adrs/0104-the-emitters-source-is-authorable-geometry.md) + [Plan 0090](plans/done/0090-the-emitters-source-moves.md) Phases 1-4, closing the **second** option; option 1 had landed at [Plan 0077](plans/done/0077-the-quiet-sky.md) Phase 2. **Closed 2026-08-15, both halves.** The source is now two authorable scalars plus the two params that answer what moving it costs (`spawn_fade`, `prewarm`), every default an exact arithmetic identity — the golden suite passes against the **committed** baselines, not merely re-blessed. **This entry named one warm-up and there were two**, which the plan found while grounding the gate argument and then measured: the *travel* warm-up is geometry and the *population* one is the spawn rate. Slow draft, `prewarm = 0` against `prewarm = 1` — `sanity` `0.0074` / 0 of 10 radial shells (**convicted blank**) against `0.1470` / 10 of 10; `animation` `0.0629` against `0.1702`; `reactivity` `0.0002` against `0.0195`. **The measurement corrected the plan's own guess**: the animation gate was never the wall (it passes the sparse draft cold), `sanity` was, and no floor moved either way. The world itself is Plan 0090's `human` Phase 5 and stands under Standing — content work on a delivered surface, not an undischarged half. **One lesson banked**: this entry's `present: SOURCE_Y: f32 = -1\.12` probe was written to go red on delivery and did not, because `DEFAULT_SOURCE_Y` still contains the substring — anchor a probe on the line, not on a bare identifier |

### Added when Plan 0085's Phase 5 was run, later the same day

Their sibling **0083 was half-discharged at the close above and closed a few hours later**, when
the `human` phase it was waiting on was actually run. Both halves of that are worth keeping: a
half-discharged entry *does* stay live with a dated update naming which half, and this one shows
the other half arriving rather than sitting.

| # | Entry | Went to |
|---|-------|---------|
| 0083 | RSS grew 385 to 663 MB over three minutes of switching, with no no-feedback control | [ADR-0099](adrs/0099-the-show-length-horizon-is-a-spot-check-and-it-splits-in-two.md) + [Plan 0085](plans/done/0085-the-show-length-horizon-gets-an-instrument.md) Phases 3 and 5. **Closed 2026-08-15, bounded direction.** Three runs at a fixed 20 s dwell: feedback (62 switches, 1196 s) **382.6 -> 367.2 MB**, no-feedback control (62 switches, 1196 s) **379.9 -> 380.1 MB**, and feedback with no switching (1797 s) **379.7 -> 328.0 MB**. **Nothing grew and the long run fell 52 MB.** The control is what makes it readable — run 1 oscillates across ~30 MB while run 2 sits inside 0.4 MB, so feedback churn is real, **per-switch, and recovered every switch**. Caveats bound it rather than undermine it: no audio, windowed never fullscreen, different presets, 165 Hz — a lighter load than the original, and the fullscreen reconfigure that dominated the original observation never happened. **The runs also falsified a claim in the archived [0082](design-backlog-archive.md#0082--the-quality-governor-reads-frame_ms_p99-and-a-preset-switch-spikes-p99-to-25-ms-while-nothing-is-dropped)**, filed as [0094](#0094--the-frame_ms_p99-tail-is-not-switch-correlated-so-the-steady-state-column-does-not-remove-it) |

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
- **Verified 2026-08-15** — the mechanism citation still resolves, and the rate-limited release
  this entry proposes as the cheap shape is still unbuilt:
  `present: Easing in: core/src/preset/schema.rs`, `absent: slew in: core/src`. The evenness
  arithmetic itself is not a repo claim and is not reduced here.

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
- **Verified 2026-08-15** — both windows are still sized in samples, and the sweep that measured
  the consequence is still pinned: `present: WINDOW_SIZE: usize = 2048 in: core/src/dsp/mod.rs`,
  `present: LOW_WINDOW_SIZE: usize = 8192 in: core/src/dsp/mod.rs`,
  `present: the_axis_holds_at_the_rates_we_do_not_develop_at in: core/src/dsp/fft.rs`. A literal
  sample count is the claim, so the day either becomes a duration this goes red — which is the
  fix this entry asks for and therefore the re-read it wants.

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
- **Verified 2026-08-15** — the one binding this entry names is still the only one a retune has
  to start from: `present: ^exposure in: presets/lsystem_vellum.toml`. The count around it does
  not reduce — `unprobeable: exactly one shipped preset binds exposure is a claim about how many
  files match, and the grammar deliberately has no count verb (ADR-0108, Notes)`. The document this
  entry corrects still carries the sentence it corrects: `present: tonemap-knee in: docs/plans/README.md`
  — which goes red when that paragraph is next rewritten, and that is the moment to re-read whether
  the correction is still owed.
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

- **PROMOTED A THIRD TIME 2026-08-15 → [ADR-0109](adrs/0109-the-beat-clock-counts-onsets-not-beats.md) +
  [Plan 0095](plans/0095-the-downbeat-fold-gets-a-musical-beat.md)**, and **this entry's stated cause
  is corrected here rather than left standing.** Plan 0086 ran its measurement and closed at Phase 2:
  the cue was never changed, so the repair this entry has now been promoted for three times is still
  unbuilt and the entry stays **live**. What the measurement found is upstream of the cue. `beat_index`
  counts **onset-detector events, not musical beats** — 1.73x / 1.35-2.10x / 1.76x detections per beat
  on three genres, wandering across 1x, 2x and 4x inside a single track, against a synthesized control
  that reads exactly 1.00 — so the 4/4 fold is indexed by a unit that is not a beat and a bar-locked
  accent precesses across all four alignments. The bass-weighted accent named below (Plan 0068 Phase 3,
  and repeated in the 2026-08-15 verification bullet) is therefore **at best secondary**: it is a real
  property of the feature, but it is not what holds the publish rate down, and a second accent band
  would not have moved it. The measured rates in this entry's body remain accurate as *outcomes*; the
  mechanism sentence attached to them does not.
- **PROMOTED AGAIN 2026-08-13 → [ADR-0097](adrs/0097-the-downbeat-cue-is-chosen-against-per-beat-evidence.md) +
  [Plan 0086](plans/done/0086-the-downbeat-finds-a-cue-that-is-not-the-kick.md)** — the **repair**, which
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
- **Verified 2026-08-15** — the gate has not moved, and the accent is still bass-weighted, which
  is the cause Plan 0068 Phase 3 named: `present: CONFIDENCE_THRESHOLD: f32 = 0\.25 in: core/src/dsp/downbeat.rs`,
  `present: BASS_WEIGHT: f32 = 0\.7 in: core/src/dsp/downbeat.rs`. Both are written to go red when
  [Plan 0086](plans/done/0086-the-downbeat-finds-a-cue-that-is-not-the-kick.md) changes the cue, which
  is exactly when this entry's measured rates stop describing the engine. The authoring-doc
  qualification this entry called for is also still in place, checked rather than assumed:
  `present: 70 % bass band in: presets/README.md`.

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

## Entry 0069 — from Plan 0070's close (2026-08-05)

Its sibling
**[0068](design-backlog-archive.md#0068--a-swarm-mark-has-no-per-mark-variation-so-the-only-scene-that-can-hold-a-starfield-cannot-make-one-twinkle)
closed 2026-08-15** at [Plan 0090](plans/done/0090-the-emitters-source-moves.md)'s close — both
options delivered — and its body is in the archive.

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
> [ADR-0106](adrs/0106-two-tone-graphics-come-from-a-multiply-layer.md).
>
> **MEASURED AGAIN 2026-08-15**, at [Plan 0091](plans/0091-the-figure-fills-the-frame.md) Phase 1,
> which settled the path ADR-0106 left open and falsified one of its own consequences in the
> process. Both runs are `core/tests/layer.rs`, so the numbers are re-derivable rather than
> recalled:
>
> - **Multiply does NOT reach the backdrop, and the answer is negative in the clean way.** The
>   backdrop sits outside the chain's input (`post.rs:33`) and is composited *underneath* the
>   junction, so no blend mode can operate on it. At the default `occlude = 1` the frame is
>   **byte-identical** over a lit backdrop and over a black one — the backdrop is not darkened, it
>   is **absent**, held out by coverage. With it visible (`occlude = 0`) it is added after the
>   blend and floors the frame: the same multiply layer reaching **18.9** over black reaches only
>   **171.3** over a lit sky, where the sky alone reads 196.9. **Consequence for authoring: a light
>   ground must come from the CHAIN, not from `bg_*`.**
> - **A particle layer CAN darken, and ADR-0106's Negative consequence saying otherwise is
>   wrong.** Measured: a frozen swarm at `brightness = 0` in a `multiply` slot takes a light chain
>   from luma **174.1** to **0.9** — *darker* than the field route's 18.9. The mechanism the ADR
>   states ("a particle's alpha *is* its brightness") is not what the code does: `swarm.rs` emits
>   `vec4(color * g, g)` where `g` is the mark's **geometric** falloff, independent of its colour,
>   and `layer_blend.rs:138` un-premultiplies (`straight = b.rgb / max(b.a, 1e-4)`) before the mode
>   runs. So a black particle has full coverage and a zero operand — the darkening condition met,
>   not failed. **The real difference between the two routes is footprint, not capability**: a field
>   darkens every pixel, a particle darkens only inside its marks.
>
> **What survives, and it is why this entry is corrected rather than archived:** multiply darkens in
> proportion to *coverage*, and **nothing in this engine still decides what is in front of what**. A
> shaped object that occludes another figure is unbuilt.

- **Raised:** 2026-08-05, at [Plan 0070](plans/done/0070-shaped-marks.md)'s close. **Re-filed from
  [0033](design-backlog-archive.md), at that entry's own instruction** — 0033 carried two asks, Plan
  0070 answered one of them, and leaving the other inside a closed entry is how the two get confused
  again.
- **Verified by measurement:** yes, and the measurement is the point. The cardioid
  `r = 1 - sin(theta)` drawn through `parametric_curve` at `ink_amount = 1` on white paper renders
  its outline **grey**, not black: a thin anti-aliased stroke averages to mid luminance and lands
  halfway down the ink ramp.
- **Verified 2026-08-15** — the correction box above, not the title: the darkening blend and the
  fullscreen coverage it needs both exist:
  `present: multiply in: core/src/render/layer_blend.rs`,
  `present: occlude in: core/src/render/scenes/fragment_field.rs`. The half this entry stays live
  for does not reduce — `unprobeable: nothing in this engine decides what is in front of what is
  the absence of a whole mechanism rather than of a symbol, and every narrow spelling of it (depth,
  sort, order) is a common word in this tree, so any probe on it could never fail and would read as
  verification while checking nothing`

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
- **Verified 2026-08-15** — the standing note that [Plan 0087](plans/0087-the-line-renderer-draws-a-curve.md)
  Phase 6 replaces is still in the file: `present: Nothing here fakes one in: core/src/render/scenes/lines/star.rs`.
  The promotion bullet cites it at `star.rs:599` and it now sits at line 606, which is why the probe
  is on the sentence rather than the line.

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
- **Verified 2026-08-15** — both mechanisms are still exactly as this entry's 2026-08-13 resolution
  describes them. The join extends each flagged endpoint by the half-width
  (`present: JOINED_A in: core/src/render/scenes/lines/renderer.rs`), and the per-motif vertex count
  is still a fixed constant with no authorable resolution
  (`present: fn vertex_count in: core/src/render/scenes/lines/star.rs`,
  `present: SMOOTH_SAMPLES in: core/src/render/scenes/lines/star.rs`).
  [Plan 0087](plans/0087-the-line-renderer-draws-a-curve.md) is written to falsify the second.

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
- **Verified 2026-08-15** — item 2's engine fact is unchanged, and it is the only half of this
  entry still open: one sampler, repeating on the palette coordinate:
  `present: address_mode_u: wgpu::AddressMode::Repeat in: core/src/render/palette.rs`.
  [ADR-0102](adrs/0102-a-palette-coordinates-edge-is-a-per-preset-choice.md) is proposed and has no
  plan, so this is expected to hold until a look asks for the clamp.
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

## 0079 — an accumulating figure rendered with `trails = 0` is not a sparse source, it is a blank one, and a whole third of a decision grid was unreadable because of it

**Raised by:** `architect`, reading [Plan 0064](plans/done/0064-the-symmetry-stage-and-the-banded-palette.md)
Phase 3's sample set at Phase 4. **Owner if taken:** whoever next builds a capture grid — this is a
methodology note, not a code change.

- **Verified 2026-08-15** — `unprobeable: this is a capture-hygiene rule for whoever next builds a
  sample grid, and its own What a fix would be section is nothing in code, so it makes no claim
  about this repository's contents at all`

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

---

## Entries 0084-0089 — from the Plan 0075 cohorts 1-5 handoff (2026-08-11)

The renaissance's first five cohorts (28 worlds, cohort 5 judged live 2026-08-11) handed back
one assembled feedback note. Three of its items are **re-raises** and are recorded as dated
updates inside [0009](#0009--the-animationrs-gate-penalizes-two-legitimate-designs-informational),
[0055](#0055--the-attractors-shape-vocabulary-is-breathe-and-bend-and-the-reference-figures-ask-for-more)
and [0068](design-backlog-archive.md#0068--a-swarm-mark-has-no-per-mark-variation-so-the-only-scene-that-can-hold-a-starfield-cannot-make-one-twinkle)
rather than as new entries; the two doc drifts it carried went to
[Plan 0075](plans/done/0075-the-content-renaissance.md) Phase 6's sweep list, not here. Each entry
below carries a **handoff verdict** — promote or park — per that plan's Decision (promotion on
demonstrated want, each through its own ADR/plan, never absorbed into 0075). Promoted items
queue **behind Plan 0076 and cohort 6**; none of them gates the collage. All measurements below
are the lane's renders, reported at the handoff — not independently re-verified here; the
file's standing verify-before-acting rule applies.

---

## 0087 — `reaction_diffusion` has no glow of its own, and the engine bloom's threshold sits above where its output lives

**Raised by:** `preset-author`, Plan 0075 cohort 3 (the Verdigris/Mitosis register).
**Owner if taken:** `architect` then `dev`, if a second want arrives.

- **Verified 2026-08-15** — the absence is still an absence: the RD scene has no glow or threshold
  of its own, so the engine-wide `bloom_*` is still the only instrument pointed at it:
  `absent: bloom in: core/src/render/scenes/reaction_diffusion.rs`. That is the whole of this
  entry's repo claim; whether a second want has arrived is not a fact about the tree.

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

## 0092 — every figure this engine draws is unlit, and two reference images ask for a shaded one

- **Raised:** 2026-08-13, from the second of two user reference batches, alongside
  [Plan 0091](plans/0091-the-figure-fills-the-frame.md). Filed separately **at the point of raising**
  rather than absorbed into that plan, because it is a lighting decision and the plan is a silhouette
  one, and bundling them would have made a shading register arrive as a side effect of a shape phase.
- **Verified by measurement:** no, and it does not need one — the claim is an absence. Nothing in
  `core/src/render/` computes a surface normal or evaluates a light. Every family is emissive: the
  particle scenes add glow, the field scenes map a scalar through a LUT, the line renderer strokes,
  and the terminal stages remap what those produced. Brightness in this engine is *authored colour*,
  never *illumination*.
- **Verified 2026-08-15** — nothing shades: `absent: matcap in: core/src`. Deliberately narrow
  rather than probing for a normal or a light — `normalize` is everywhere in this tree, so a probe
  spelled that way could never fail, and a probe that cannot fail reads as verification while
  checking nothing (ADR-0108, Negative). `matcap` is the name this entry's own proposed fix
  carries, so the probe goes red exactly when the entry is delivered.

Two of the six star references are chrome — a four-pointed sparkle with concave edges, rendered as
polished metal with specular highlights, a horizon reflection and self-shadowing. They read as
**objects with a surface**, and the engine has no vocabulary for that at all. The rest of the batch
is flat graphic work that [Plan 0091](plans/0091-the-figure-fills-the-frame.md) Phase 5 reaches with
three parameters; these two are a different question wearing the same silhouette.

### Why it is worth an entry rather than a comment

**The prerequisite is about to exist, and that is the whole reason to file this now.** ADR-0105 puts
a signed distance field on screen, and the gradient of a distance field *is* a surface normal —
`normalize(vec3(dFdx(d), dFdy(d), k))` for a 2.5D bevel, or the analytic gradient where an arm has
one. So the expensive precondition for shading (knowing which way a surface faces) arrives as a free
by-product of a plan already written for other reasons. A matcap or a small analytic environment then
turns that normal into the chrome look, and neither needs a light rig, a depth buffer, or a second
pass.

**It would also be the engine's first non-emissive register**, which is why it is an ADR-worthy
decision and not a parameter. Every consequence downstream assumes emission: ADR-0056's alpha *is*
the falloff, bloom's bright-pass reads emitted light, ADR-0046's linear-light ordering is built for
additive accumulation, and [ADR-0106](adrs/0106-two-tone-graphics-come-from-a-multiply-layer.md) has
just established that darkening requires a `multiply` layer. A shaded object has dark regions that
are *shape information* rather than absent light, and nothing in that chain currently distinguishes
the two.

### What a fix would be

A `shade` amount on the shape field, off by default, deriving a normal from the distance gradient
and reading a small built-in matcap. Off is an exact identity, so nothing that ships moves. The open
questions are which matcaps ship (a closed roster, on ADR-0084's precedent), whether the bevel
profile is authorable or fixed, and — the load-bearing one — whether a shaded figure should bloom,
since its highlight is the brightest thing on screen and is *not* an emitter.

### Priority

**Low, and gated on Plan 0091.** There is no route to this before the distance field exists, and one
user batch that mixed it with five flat references is a want expressed once rather than a demonstrated
gap. Take it if the Phase 6 look gate says the flat sparkle is the disappointing one in the set — that
verdict is the trigger, and it is scheduled.

---

---

## 0093 — the headless capture path dies past a few thousand frames, so the horizon cannot reach its own headline length

**Raised by:** `architect`, at [Plan 0085](plans/done/0085-the-show-length-horizon-gets-an-instrument.md)'s
close, from that plan's Phase 2 findings. **Owner if taken:** `dev` — but read the mechanism below
first, because the candidate cause is one line and the entry may be cheaper than it looks.

- **Verified 2026-08-15** — the candidate mechanism is still exactly as stated below, and still
  unfixed: `present: fn step_offscreen in: core/src/render/capture_api.rs`,
  `absent: poll in: core/src/render/capture_api.rs`. The second probe is the whole hypothesis in one
  line — the day anyone polls per frame in that file it goes red, which is the right moment to
  re-read this entry whether or not the fix worked.
- **Verified 2026-08-15, AND IT CONVICTED THIS ENTRY — reported by `dev` at Plan 0093 Phase 2,
  corrected in place by `architect` at that plan's close.** The finding said *"Both shipped
  reaction-diffusion worlds (`reaction_mitosis`, `reaction_verdigris`)"*. **There are three:**
  `present: system = "reaction_diffusion" in: presets/reaction_etching.toml`. `reaction_etching`
  landed in `6ebec33` on **2026-08-10**, five days before this entry was written, so this is a
  **birth defect** — the class
  [ADR-0108](adrs/0108-a-backlog-claim-about-the-repo-carries-an-executable-probe.md) exists for,
  found by the first pass that read the entry against the tree, by the instrument that pass was
  building. It is corrected rather than closed because the finding it distorts is untouched: the
  capture path still dies at 3,601 frames and the candidate mechanism is still one line. What the
  correction costs the entry is its **scope argument**, and that half is now stated as the open
  question it always was — see *Why it is worth an entry* below.

### The finding

**Two of the three shipped reaction-diffusion worlds** (`reaction_mitosis`, `reaction_verdigris`)
fail at **3,601 frames** with `Buffer with 'lmv-capture-readback' label is invalid`, after the
process's resident set climbs to **~2.9 GB**. The third, `reaction_etching`, **was never run** —
it was shipped before this entry was written and was simply missed.  Measured on the Windows development box, hardware adapter, debug build,
at 96x96 — the capture size is not the lever, since 2.9 GB is four orders above what 3,600 frames of
96x96 RGBA would be.

**It is pre-existing and not a defect in Plan 0085's new sampling primitive.** The shipped
`Renderer::capture_preset` fails identically at the same frame count on the same preset, which was
run as the control *before* this was called a finding. It is only visible now because nothing in
this repo had ever driven a world for thousands of frames: the four synthesized gates capture 30,
the reactivity gate a few hundred.

### Why it is worth an entry

**It bounds the instrument Plan 0085 just shipped.** `shot --horizon 10` is documented as ten
simulated minutes — 36,001 renders — and on two shipped worlds it cannot get past 3,601. Those two
rows in the plan's Phase 2 table are therefore a **0.5-minute** horizon, and their `monotone 1.00`
is a world still settling into its pattern rather than drifting: a horizon shorter than a world's
own warm-up reads settling *as* drift, which is precisely the misreading the instrument exists to
prevent.

**How wide the ceiling is, is open — and the entry originally overstated how well that was known.**
Every world in the *measured* set other than these two cleared 36,001 renders, which is why this
reads as a family ceiling rather than a general one. But the measured set was not the roster: it
omitted `reaction_etching`, the third RD world, so the family evidence is two of three and nobody
has run the member that would confirm it. Two readings survive that and the entry does not choose
between them — a mechanism ceiling specific to RD's heavy per-frame ping-pong, or a *cost* ceiling
that any sufficiently expensive world reaches and RD reaches first. **Running `reaction_etching`
separates them, and it is the cheapest thing anyone can do here** — it costs one `shot --horizon`
and it decides which of the two the fix has to answer.

It is also **adjacent to, and not the same as,**
[0083](design-backlog-archive.md#0083--rss-grew-385-to-663-mb-over-three-minutes-of-preset-switching-and-there-is-no-no-feedback-control-to-compare-it-against):
that is the *live app's* resident set under preset switching, this is a *headless offscreen loop*
that never rebuilds a surface. If they share a cause it would be worth knowing, and nothing
currently says they do.

### What a fix would be — with a candidate mechanism, stated as unverified

`core/src/render/capture_api.rs:481` — **`step_offscreen` creates a command encoder, submits it, and
never polls.** `capture::read_back` holds the only `device.poll` anywhere in the capture path, so
between two sampled frames wgpu has no opportunity to release the transient resources each
submission retains. At a 60-second interval that is **3,600 consecutive unpolled submits**. All
three capture entry points (`capture_preset`, `capture_preset_over`, `capture_preset_at`) funnel
through `step_offscreen`, which is exactly why the control failed the same way — and it would
explain why an RD world, whose per-frame ping-pong is the heaviest in the engine, hits it first.

**This is a hypothesis and nobody has run it.** The check is cheap: poll once per frame in
`step_offscreen` and re-run `shot --horizon 10` on `reaction_mitosis` — and on `reaction_etching`,
which has never been run at all and is what decides whether the ceiling is the family's or the
cost's. If the RSS trace flattens,
the fix is one line and the entry closes with a measurement; if it does not, the diagnosis is wrong
and the real one starts from a GPU memory report rather than from this paragraph. **Do not close
this by lowering a documented horizon** — the instrument's stated length is what makes a recorded
header verdict comparable across worlds.

### Priority

**Medium.** Nothing ships broken — this is a QA path, not a runtime one, and the live app polls
every frame through its own present. But it silently truncates the only instrument this project has
for show-length behaviour, on the family that most needs it, and the truncation reads as a *result*
(`monotone 1.00`) rather than as an error unless someone reads the run's stderr.

---

## 0094 — the `frame_ms_p99` tail is not switch-correlated, so the steady-state column does not remove it

**Raised by:** `architect`, at [Plan 0085](plans/done/0085-the-show-length-horizon-gets-an-instrument.md)
Phase 5, from the three paired runs that closed
[0083](design-backlog-archive.md#0083--rss-grew-385-to-663-mb-over-three-minutes-of-preset-switching-and-there-is-no-no-feedback-control-to-compare-it-against).
**Owner if taken:** `architect`, and only if the governor is revisited.

- **Verified 2026-08-15** — the governor reads the raw series at a miss fraction, not `p99`:
  `present: MISS_FRACTION in: core/src/render/tier.rs`. Written in
  [ADR-0108](adrs/0108-a-backlog-claim-about-the-repo-carries-an-executable-probe.md)'s grammar
  while [Plan 0093](plans/done/0093-the-backlog-stops-asserting-things-about-a-repo-it-has-not-read.md)
  is still in flight, because this entry's whole reason to exist is that its *predecessor's*
  repo-claim rotted. The probe covers the load-bearing half: if `sustained_miss` is ever rewritten
  to read `p99`, that constant is what goes with it, and this entry should go red rather than keep
  telling a reader the tail is unremovable by an exclusion nobody is applying.

**This is a new entry rather than an edit to
[0082](design-backlog-archive.md#0082--the-quality-governor-reads-frame_ms_p99-and-a-preset-switch-spikes-p99-to-25-ms-while-nothing-is-dropped)**,
which is closed and archived. It cites it; it does not amend it. 0082 already carries two
corrections — the governor was built, and it never reads p99. This is a **third**, and it is about
the half of 0082 that survived both.

### The finding

0082 states, as the observation the whole entry rests on: *"The spikes coincide with preset switches
and the fullscreen toggle — GPU resource rebuilds, not steady-state cost."* Plan 0085 Phase 5's
**run 3** was 1,797 s of feedback presets with auto-rotate **off** — three surface reconfigures at
startup and nothing after. It reached:

| | value |
|---|---|
| `frame_ms_p99` max | **23.960 ms** at t = 355 s |
| second peak | 18.681 ms at t = 1761 s |
| switches after startup | **0** |
| rows where `frame_ms_p99_steady` diverged from raw | **0 of 359** |
| frames dropped (`diagnostics.log`, whole window) | **0 of 200,667** |

The steady column never diverged because there was no switch anywhere near those spikes — the
exclusion window had nothing to exclude. **A ~24 ms p99 excursion happened in a session with no
preset switching at all.**

Both mechanisms are real and they are different sizes. In **run 1** (62 switches) the exclusion
fired on **58 of 239** rows and separated them cleanly — ~11.9 ms raw against ~6.5-7.0 ms steady —
so switches *do* elevate p99 and the new column *does* remove that. But the largest excursions in
both runs (17.7 ms in run 1, 24.0 ms in run 3) landed on rows the exclusion did not touch.

### Why it is worth an entry

**0082's first candidate response — "exclude the frames following a preset switch or a surface
reconfigure from the governor's window" — would not have solved the problem 0082 raised.** The
spikes it was written about survive the exclusion. Anyone revisiting the governor from that entry
would implement the exclusion, watch the tail persist, and have to rediscover this.

It is **not** currently a defect: nothing demotes on p99 (the shipped `sustained_miss` reads the raw
series at a 75 %-of-180 miss fraction, which no excursion of this size approaches), zero frames
dropped across 200,667, and fps never left the 146-165 band against a 60 fps floor. The value is
entirely in *not* designing against a false model later.

### What a fix would be

Nothing, until the governor is revisited. What is missing is a **cause** — the tail is unexplained,
and three candidates are untested: OS/driver scheduling on a 165 Hz vsync, a genuinely expensive
frame in one of the four feedback presets, or another process on a developer box. The cheap
discriminator is a run with the per-frame series retained rather than a p99 summary, which
`--soak`'s coarse tick cannot give — `diagnostics.log`'s 1 Hz rows are the nearest existing
instrument and were not read for this.

**Whoever takes this should re-measure rather than trust the table above.** These runs were
windowed, with no audio, on one box — and **the fullscreen toggle, which is the one event 0082
named that this run could not reproduce, remains the strongest untested candidate for the original
25.037 ms.**

### Priority

**Low.** It blocks nothing, nothing ships broken, and it becomes load-bearing only when someone
opens the governor. It is filed because that is exactly the moment the correction would otherwise
be missing.
