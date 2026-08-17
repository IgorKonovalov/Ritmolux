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

A ledger row is a pointer: the number, one line of what it was, and where it went. The body it
points at carries the rest. `scripts/check-index-rows.mjs` holds every row below to 320 bytes
([ADR-0116](adrs/0116-an-index-row-is-a-pointer-and-a-gate-holds-it-to-one.md)); the live entry
bodies further down are content, not an index, and are deliberately outside the region.

A trailing `see NNNN` names another backlog entry rather than a document — look for it under
`## Open entries` below, or in [design-backlog-archive.md](design-backlog-archive.md) if it has
already closed. Written once here so that a cross-reference costs a row four bytes instead of
eighty.

<!-- roster:begin cap=320 -->

| # | Entry | Went to |
|---|-------|---------|
| 0001 | `reaction_diffusion` reaches only 2 of the 5 composite levers | [ADR-0026](adrs/0026-full-composite-coverage-fullscreen-scenes.md) + [Plan 0025](plans/done/0025-full-composite-coverage.md) |
| 0002 | No per-bin spectrum: the grammar sees three bands | [ADR-0036](adrs/0036-preset-reachable-spectrum.md) + [Plan 0034](plans/done/0034-preset-reachable-spectrum.md) |
| 0003 | Fixed internal resolutions (RD 256², post stages 720p) | [ADR-0034](adrs/0034-internal-resolution-follows-the-target.md) + [Plan 0033](plans/done/0033-internal-resolution-and-preset-surface.md) |
| 0004 | `zoom`/`pan_*` smear RD's edge: a toroidal sim behind a clamped sampler | [ADR-0034](adrs/0034-internal-resolution-follows-the-target.md) + [Plan 0033](plans/done/0033-internal-resolution-and-preset-surface.md) Phase 5 |
| 0005 | No bloom / glow / halo stage | [ADR-0046](adrs/0046-linear-light-hdr-composite-bloom-tonemap.md) + [Plan 0045](plans/done/0045-linear-light-and-bloom.md) |
| 0006 | `[smoothing]` is a symmetric one-pole: no attack/release split | [ADR-0035](adrs/0035-asymmetric-attack-release-easing.md) + [Plan 0033](plans/done/0033-internal-resolution-and-preset-surface.md) Phase 2 |
| 0007 | `star_pattern` is a hollow ring, and `variant` cannot be blended | [ADR-0060](adrs/0060-star-pattern-variants-interpolate.md) + [Plan 0054](plans/done/0054-the-line-scenes-catch-up.md). **Closed 2026-08-06** |
| 0008 | `shot` harness gaps that cost the content lane real iterations | [Plan 0033](plans/done/0033-internal-resolution-and-preset-surface.md) Phase 1 + [Plan 0037](plans/done/0037-verifying-easing-transient-probe-and-dynamic-signal.md) Phase 4 |
| 0010 | The fold samples outside its source rectangle and clamps | [ADR-0047](adrs/0047-kaleidoscope-fold-domain-disc-with-falloff.md) + [Plan 0045](plans/done/0045-linear-light-and-bloom.md) |
| 0011 | The fold axis is screen-centred, so `pan_*` and `kaleido_*` fight | [ADR-0047](adrs/0047-kaleidoscope-fold-domain-disc-with-falloff.md) + [Plan 0045](plans/done/0045-linear-light-and-bloom.md) Phase 1 |
| 0012 | `--report`'s `cover` penalises ink presets — **premise was false** | [Plan 0037](plans/done/0037-verifying-easing-transient-probe-and-dynamic-signal.md) Phase 5, as documentation |
| 0013 | No synthetic signal has transients, so easing is unverifiable | [ADR-0039](adrs/0039-verify-easing-with-a-transient-probe-not-a-committed-clip.md) + [Plan 0037](plans/done/0037-verifying-easing-transient-probe-and-dynamic-signal.md) |
| 0014 | The line scenes' cosine `hue` ramp is not a hue wheel | [Plan 0037](plans/done/0037-verifying-easing-transient-probe-and-dynamic-signal.md) Phase 5 — **and the entry's own colour names were wrong** |
| 0015 | The band axis is half linear below the crossover | [ADR-0049](adrs/0049-analysis-v2-dual-resolution-axis-normalized-bands.md) + [Plan 0048](plans/done/0048-analysis-v2-and-the-retune.md). **Closed 2026-08-04** |
| 0016 | The `spectrum` readout has no width control | [Plan 0038](plans/done/0038-line-family-unreachable-levers.md) Phase 2 |
| 0017 | `[spectrum]` has no level curve, and the grammar has no `log` | [ADR-0040](adrs/0040-spectrum-level-curve-applies-before-the-easing.md) + [Plan 0038](plans/done/0038-line-family-unreachable-levers.md) |
| 0018 | `BASELINE_Y` is a constant, so `mirror_reflect` throws the copy up | [Plan 0038](plans/done/0038-line-family-unreachable-levers.md) Phase 2 |
| 0019 | `glow` is unreachable from a preset on all four line scenes | [Plan 0038](plans/done/0038-line-family-unreachable-levers.md) Phase 1 |
| 0020 | The library is gained against stimuli 6-100x hotter than real music | [ADR-0042](adrs/0042-reachability-measured-on-the-expression-tree.md) + [Plan 0041](plans/done/0041-report-two-level-stimuli-and-expression-reachability.md). **Closed 2026-08-04** |
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
| 0033 | Every mark the engine can draw is a round blob or a stroked curve — **silhouette half only** | [ADR-0084](adrs/0084-a-particle-marks-silhouette-is-a-signed-distance-function.md) + [Plan 0070](plans/done/0070-shaped-marks.md). **Closed 2026-08-05**; see 0069 |
| 0034 | Nothing in the engine spawns, throws, ages or individuates an object | [ADR-0057](adrs/0057-emitter-scene-analytic-ballistics-seeded-individuation.md) + [Plan 0052](plans/done/0052-the-emitter-objects-that-spawn-fall-and-die.md) |
| 0035 | `presets/README.md` listed 10 expression variables; the code had 19 | Fixed at [Plan 0048](plans/done/0048-analysis-v2-and-the-retune.md)'s close |
| 0036 | Does the fold stop folding the backdrop, and does that lose a look? | [ADR-0055](adrs/0055-backdrop-leaves-the-post-chain.md) |
| 0037 | The fold covers a disc, and on a field scene that reads worse | [ADR-0061](adrs/0061-kaleidoscope-edge-treatment-is-a-per-preset-choice.md) + [Plan 0055](plans/done/0055-the-fold-edge-becomes-a-choice.md) |
| 0039 | Four bind-group layouts are shared by pipelines live in one frame | [ADR-0058](adrs/0058-bind-group-layout-collisions-carry-evidence.md) + [Plan 0053](plans/done/0053-the-suite-stops-blessing-what-warp-gets-wrong.md) |
| 0040 | Additive light occludes by geometry, so a dim figure over a lit backdrop reads as dark speckle | [ADR-0085](adrs/0085-how-much-a-scene-occludes-the-backdrop-is-one-number.md) + [Plan 0071](plans/done/0071-light-that-adds-without-covering.md). **Closed 2026-08-09**; see 0038 |
| 0041 | The line seam's lit-backdrop guard discriminates on ~5 pixels | [Plan 0053](plans/done/0053-the-suite-stops-blessing-what-warp-gets-wrong.md) |
| 0043 | Every reactivity instrument diffs against **silence** | [ADR-0062](adrs/0062-clamp-occupancy-is-the-saturation-instrument.md) + [Plan 0056](plans/done/0056-clamp-occupancy-and-the-axis-anchor.md) |
| 0044 | The axis rebuild silently re-pointed every sub-crossover `bin()` probe | [ADR-0063](adrs/0063-address-the-spectrum-by-frequency.md) + [Plan 0056](plans/done/0056-clamp-occupancy-and-the-axis-anchor.md) |
| 0045 | `docs/analysis-v2-before-flags.md` asks to be deleted | Done 2026-08-03; three inbound links rewritten, not two |
| 0046 | The retune's gain rule is direction-blind — **retracted, the claim was false** | Retracted the same day, before any preset was edited. Kept in full because the *reason* it was wrong is a trap |
| 0047 | `Rich` triples the attractor's light, so the tier is not look-neutral | [ADR-0064](adrs/0064-a-capture-may-pin-the-rich-tier.md) + [ADR-0065](adrs/0065-the-attractor-deposit-is-normalized-by-particle-count.md) + [Plan 0057](plans/done/0057-the-attractors-compute-path.md) |
| 0048 | The `lorenz` family renders as a dust cloud | [ADR-0068](adrs/0068-the-projection-basis-is-a-per-family-property.md) + [Plan 0059](plans/done/0059-lorenz-finds-its-plane.md) |
| 0049 | The fold's residual rays got a second rejection and a shipped instance | [Plan 0055](plans/done/0055-the-fold-edge-becomes-a-choice.md). **Closed 2026-08-04**; see 0058 |
| 0050 | The attractor reseed scatters into an axis-aligned box | [ADR-0066](adrs/0066-a-reseed-disturbs-the-cloud-rather-than-replacing-it.md) + [Plan 0057](plans/done/0057-the-attractors-compute-path.md) |
| 0051 | `variant` can morph and neither `star_*` preset does | Closed by content: both presets now drive `variant` with a triangle wave (`star_rosette.toml:59`, `star_lantern.toml:77`). **Closed 2026-08-04 during a backlog sweep** |
| 0052 | `Spectrum Ridge` has no tonal structure — **premise was false** | Retired 2026-08-03; the preset was never flat and the statistic convicted the right preset for the wrong reason |
| 0053 | The retune rescaled band gains but not the world-space params | [ADR-0067](adrs/0067-coverage-measures-the-scene-not-the-backdrop.md) + [Plan 0058](plans/done/0058-the-gate-can-see-an-empty-frame.md) |
| 0054 | Pixel coverage cannot see a figure whose *tips* leave the frame | [ADR-0083](adrs/0083-in-frame-geometry-is-measured-at-the-line-renderers-draw-seam.md) + [Plan 0069](plans/done/0069-the-instrument-that-sees-a-figure-leave-the-frame.md). **Closed 2026-08-06**; see 0070 |
| 0055 | The attractor's shape vocabulary is "breathe and bend", and the reference figures ask for more | [ADR-0093](adrs/0093-attractor-tuples-are-content-with-per-tuple-framing.md) + [Plan 0079](plans/done/0079-the-attractor-learns-new-figures.md). **Closed 2026-08-13** |
| 0057 | No scene-local level param, so `exposure` gets used for one and two stages disagree | [ADR-0080](adrs/0080-the-attractor-owns-its-level-and-bloom-thresholds-exposed-light.md) + [Plan 0066](plans/done/0066-the-level-lever.md). **Closed 2026-08-05** |
| 0058 | Thirteen presets bind the fold and eleven had not chosen an edge treatment | [Plan 0055](plans/done/0055-the-fold-edge-becomes-a-choice.md) |

### Added by the 2026-08-13 sweep

| # | Entry | Went to |
|---|-------|---------|
| 0009 | The `animation.rs` gate penalizes two legitimate designs | [ADR-0091](adrs/0091-the-animation-gate-scores-motion-against-the-figures-footprint.md) + [Plan 0077](plans/done/0077-the-quiet-sky.md) |
| 0055 | The attractor's shape vocabulary is "breathe and bend" | [ADR-0093](adrs/0093-attractor-tuples-are-content-with-per-tuple-framing.md) + [Plan 0079](plans/done/0079-the-attractor-learns-new-figures.md). **Closed 2026-08-13** |
| 0056 | A user-authored preset lived outside the repo for six weeks | [ADR-0081](adrs/0081-the-content-lane-lands-presets-and-architect-curates-the-set.md) + [Plan 0067](plans/done/0067-the-curation-route.md) (`3732fb4`) |
| 0059 | The backdrop does not colour through the shared palette | [ADR-0086](adrs/0086-the-backdrop-colours-through-the-preset-palette.md) + [Plan 0072](plans/done/0072-the-backdrop-joins-the-palette.md) |
| 0060 | An engine fix leaves its preset-side workarounds standing | [Plan 0067](plans/done/0067-the-curation-route.md) Phase 4 — the close-ceremony workaround grep is installed as step 3b and has run at every close since, reporting its result in the close notes even when it finds nothing |
| 0061 | `perspective` moves the figure far more than it enlarges it | [Plan 0075](plans/done/0075-the-content-renaissance.md) Phase 3, as documentation — the ~0.9x translational law and the ~0.3 practical ceiling. The re-centring option (2) had no demonstrated want and is not carried forward |
| 0062 | `depth_hue` is a lightness cue on a lightness ramp | [Plan 0075](plans/done/0075-the-content-renaissance.md); see 0075 |
| 0063 | `spin`'s usable ceiling is set by `fade`, not by taste | [Plan 0075](plans/done/0075-the-content-renaissance.md) Phase 3, as documentation |
| 0064 | An IFS preset switch shows a hard-edged rectangle of noise | [ADR-0087](adrs/0087-the-ifs-particle-carries-its-age-and-its-last-map.md) + [Plan 0073](plans/done/0073-the-fern-unfurls-and-colours-by-what-made-it.md) — the continuous respawn, so the population is never a uniform box at any instant |
| 0065 | `morph` is a travel knob whose visible rate is steepest near zero | Documentation, `cf977f9`. Struck at the time; archived here |
| 0066 | The IFS figures are STILL, so the drift-rate conventions are wrong for them | Documentation, `cf977f9`. **Its one undischarged half is now done**: `docs/capturing.md`'s gate table states that a passing `anim` is not evidence of a *watchable* preset on a still family |
| 0067 | `depth_fade` is a uniform dimmer on every flat family | [Plan 0075](plans/done/0075-the-content-renaissance.md) Phase 2 — option 2, the true no-op, asserted by **byte equality** against a live Lorenz control so it cannot pass vacuously |
| 0070 | The in-frame geometry fraction cannot gate new content | [Plan 0075](plans/done/0075-the-content-renaissance.md) Phase 2 — the `geom` column, where the over-scale defect is actually introduced. The `sanity.rs`-shaped distribution report stays a candidate second step, deliberately not taken |
| 0072 | `sanity.rs`'s coverage floor forces thin-stroke line scenes into washed-out tuning | [Plan 0075](plans/done/0075-the-content-renaissance.md) |
| 0074 | The age channel has nothing spatial to colour | [ADR-0088](adrs/0088-the-ifs-colours-by-distance-from-its-own-skeleton.md) + [Plan 0074](plans/done/0074-the-figure-colours-by-how-far-it-has-come.md) — route 2 (the channel that IS spatial) plus route 3 (`age_*` retired), so the roster did not grow |
| 0076 | The operator docs describe a fern tuning the shipped fern does not carry | Repaired at [Plan 0074](plans/done/0074-the-figure-colours-by-how-far-it-has-come.md)'s close |
| 0084 | The ink stage has no contrast lever | [ADR-0092](adrs/0092-the-ink-remap-gains-a-contrast-exponent.md) + [Plan 0078](plans/done/0078-the-ink-learns-to-bite.md). **The content half is standing, not open** — the two-header re-judge lives in [`content-brief.md`](content-brief.md) §2 |
| 0085 | `swarm` has no `reseed` | [Plan 0077](plans/done/0077-the-quiet-sky.md). **Closed 2026-08-15**; see 0086 |
| 0088 | `shot --report`'s band columns cannot see reactivity spent on bloom | [Plan 0077](plans/done/0077-the-quiet-sky.md) Phase 4 — the mean columns keep their meaning and a footprint reading lands beside them. Third member of a family the project has now fixed three times (0022, 0028, this) |
| 0091 | There is no static, screen-anchored, oriented gradient | [ADR-0094](adrs/0094-the-backdrop-paints-a-directional-ramp.md) + [Plan 0080](plans/done/0080-the-sky-gets-a-horizon.md) |

### Added by the third batch, 2026-08-13

| # | Entry | Went to |
|---|-------|---------|
| 0077 | The doc-link gate is blind to reference-style links | [Plan 0084](plans/done/0084-two-gates-stop-lying-about-what-they-check.md) |
| 0080 | The reactivity gate renders warm-up frames it throws away | [Plan 0084](plans/done/0084-two-gates-stop-lying-about-what-they-check.md) |
| 0090 | The Mac build's capture verdict is stderr-only | [Plan 0083](plans/done/0083-the-build-says-why-it-hears-nothing.md) |

### Added at Plan 0089's close, 2026-08-15

| # | Entry | Went to |
|---|-------|---------|
| 0078 | `kaleido_tile` is a discrete quantity that is not quantized — **premise was false** | [Plan 0089](plans/done/0089-the-framing-contract-stops-lying.md) |
| 0081 | The house gain rule lives only in preset headers — **first half was false** | [Plan 0089](plans/done/0089-the-framing-contract-stops-lying.md) |
| 0089 | The dragon overruns the frame corner, and `FRAME_FILL = 0.88` promises it cannot | [ADR-0103](adrs/0103-the-ifs-fit-frames-a-figure-that-does-not-turn.md) + [Plan 0089](plans/done/0089-the-framing-contract-stops-lying.md). **Closed 2026-08-15** |

### Added at Plan 0085's close, 2026-08-15

| # | Entry | Went to |
|---|-------|---------|
| 0082 | The quality governor reads `frame_ms_p99`, and a preset switch spikes it to 25 ms — **the premise was false** | [ADR-0099](adrs/0099-the-show-length-horizon-is-a-spot-check-and-it-splits-in-two.md) + [Plan 0085](plans/done/0085-the-show-length-horizon-gets-an-instrument.md). **Closed 2026-08-15** |
| 0086 | No capture path reaches the minutes-long horizon | [ADR-0099](adrs/0099-the-show-length-horizon-is-a-spot-check-and-it-splits-in-two.md) + [Plan 0085](plans/done/0085-the-show-length-horizon-gets-an-instrument.md). **Closed 2026-08-15**; see 0093 |

### Added at Plan 0090's close, 2026-08-15

| # | Entry | Went to |
|---|-------|---------|
| 0068 | A swarm mark has no per-mark variation, and the one scene that could hold a starfield could not reach a slow one | [ADR-0104](adrs/0104-the-emitters-source-is-authorable-geometry.md) + [Plan 0090](plans/done/0090-the-emitters-source-moves.md). **Closed 2026-08-15** |

### Added when Plan 0085's Phase 5 was run, later the same day

Their sibling **0083 was half-discharged at the close above and closed a few hours later**, when
the `human` phase it was waiting on was actually run. Both halves of that are worth keeping: a
half-discharged entry *does* stay live with a dated update naming which half, and this one shows
the other half arriving rather than sitting.

| # | Entry | Went to |
|---|-------|---------|
| 0083 | RSS grew 385 to 663 MB over three minutes of switching, with no no-feedback control | [ADR-0099](adrs/0099-the-show-length-horizon-is-a-spot-check-and-it-splits-in-two.md) + [Plan 0085](plans/done/0085-the-show-length-horizon-gets-an-instrument.md). **Closed 2026-08-15**; see 0082, 0094 |

### Added at Plan 0093's Phase 2 audit, closed at Plan 0099's

Its own `absent: poll` probe went red on delivery, which is the ADR-0108 grammar working rather
than failing. The body records two framings this entry got wrong — the ceiling was not a frame
count and not the RD family's mechanism — and both were found by the fix, not by the diagnosis.

| # | Entry | Went to |
|---|-------|---------|
| 0093 | The headless capture path dies past a few thousand frames, so the horizon cannot reach its own length | [Plan 0099](plans/done/0099-the-horizon-reaches-its-own-length.md), no ADR. **Closed 2026-08-16** |

<!-- roster:end -->

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
> **MEASURED AGAIN 2026-08-15**, at [Plan 0091](plans/done/0091-the-figure-fills-the-frame.md) Phase 1,
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

> **ITS TRIGGER FIRED AND RESOLVED NEGATIVELY — 2026-08-16. Do not take this entry off it.**
> The entry says to take the lighting work *"if the Phase 6 look gate says the flat sparkle is the
> disappointing one in the set"*. That gate ran at
> [Plan 0091](plans/done/0091-the-figure-fills-the-frame.md)'s close and the user rejected the star
> silhouettes — but the stated reason was that they looked **"dirty and upscaled"**, which is
> neither silhouette nor shading. It was [0099](design-backlog.md): the probes drew their figures
> through 8 to 32 of the palette's 256 LUT texels, with edge transitions of 1.3 texels, because a
> sharp star's tiny inradius had forced `color_span` down to `0.037`. Re-rendered with
> `palette_steps` bound, the same five silhouettes come back **crisp**.
>
> **So the trigger is answered and the answer is no.** The re-judge ran on a fair probe the same
> day and the verdict was *"objectively good, yes to all"* on all five silhouettes
> ([backlog 0100](design-backlog.md) carries the one soft edge, and it is about edge wobble rather
> than shading). Nothing in that gate says the flat sparkle was the disappointing one. **This entry
> stays filed on its original evidence — two reference images — and the Phase 6 trigger is spent.**
> A future lighting plan needs a fresh want, not this one.

- **Raised:** 2026-08-13, from the second of two user reference batches, alongside
  [Plan 0091](plans/done/0091-the-figure-fills-the-frame.md). Filed separately **at the point of raising**
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
is flat graphic work that [Plan 0091](plans/done/0091-the-figure-fills-the-frame.md) Phase 5 reaches with
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

---

## 0095 — the backdrop ramp makes parallel stripes only, and a converging fan cannot be lit *and* darkened

**Raised by:** `architect`, at [Plan 0091](plans/done/0091-the-figure-fills-the-frame.md)'s close, from
that plan's **Phase 7, which was designed as a cut point and was cut**. **Owner if taken:**
`architect` (it owes its own ADR) then `dev`.

- **Verified 2026-08-16** — the ramp's coordinate is linear along `bg_angle` and the swept
  coordinate is repeat-addressed, so `bg_hue_span > 1` gives repeating **parallel** stripes and
  nothing gives an angular one: `present: bg_angle in: core/src/render/background.rs`
- **Verified 2026-08-16** — the backdrop is outside the chain's input, which is why no blend mode
  and no fold can reach it: `present: backdrop is not in the chain's input in: core/src/render/post.rs`

### The want

The third of the user's Plan 0091 reference images is a collage whose floor is **white with black
stripes converging to a vanishing point**. Everything else in that collage is now reachable: the
flat red figure is `shape_field` (ADR-0105), and the dark-on-light treatment is a `multiply` layer
(ADR-0106). The floor is not.

### What is actually missing, and it is small

**Only the coordinate.** The backdrop's ramp is already swept and already repeat-addressed, so
repeating stripes ship today at no cost. An **angular** coordinate about a movable point turns the
same stripes into a fan. Folding the existing ramp is not available — `post.rs` puts the backdrop
outside the chain the kaleidoscope reads — so this is a backdrop *mode*, not a reuse.

### The constraint Plan 0091 Phase 1 added, and the reason this entry exists rather than a phase

Phase 1 measured what a `multiply` layer does to a **lit backdrop**, and the answer changes what a
backdrop fan can be *for*:

- at the default `occlude = 1` the backdrop is **absent** — held out by coverage, not darkened, and
  the frame is byte-identical to the same preset over a black backdrop;
- at `occlude = 0` the backdrop is added *after* the junction and becomes a **floor**: a multiply
  layer reaching display luma 18.9 over black reaches only **171.3** over a lit sky.

So **a fan drawn on the backdrop and a dark figure drawn over it are mutually exclusive at the
tones the reference has.** The collage's black stripes would have to come from the backdrop's own
palette stops — which the ramp can express — and the figure over them cannot then be darkened into
that floor.

### What a fix would be, and the alternative that Phase 1 promoted

Two routes, and the second is no longer the weaker one:

1. **A backdrop coordinate mode** (`bg_coord = "linear" | "radial"` plus a centre). Cheapest, and it
   is the third decision on that pass after ADR-0094 and ADR-0095 — so it owes an ADR, with every
   default an arithmetic identity and the existing baselines moving zero pixels.
2. **Draw the floor as a scene in the chain.** Plan 0091 listed this as the rejected alternative
   because it costs the preset's one `[layer]` slot (ADR-0090). Phase 1's measurement argues *for*
   it: a floor inside the chain is reachable by every blend mode, and a floor on the backdrop is
   not. The slot cost is real and it is the honest trade to weigh, not a reason to dismiss it.

The collage's floor is also **bounded by a horizon**, which the ramp expresses only through stop
placement — so "a fan" may not be the whole of what the image is doing.

### Priority

**Low.** Nothing is blocked and nothing ships broken. The heart is the part of those references the
user asked for twice and it shipped; the floor was asked for once, as one element of a collage, and
this engine is not a collage tool. It becomes worth taking when someone authors a world that wants
a vanishing point — at which point the two routes above should be judged by rendering, not by
argument, which is what Phase 7's own first done-when said.

---

## 0096 — `shape_field` draws offset contours, and the reference construction everyone reaches for is scaled copies

> **PROMOTED 2026-08-16** — [ADR-0111](adrs/0111-the-shape-field-gains-a-scaled-copy-coordinate.md)
> (proposed) and [Plan 0098](plans/0098-the-figure-nests-properly.md) Phases 2-4. The entry stays
> live until that plan lands, per this file's own lifecycle: a design that has not shipped is
> still live.

**Raised by:** `preset-author`, authoring `presets/shape_pulse.toml` against two user reference
images at [Plan 0091](plans/done/0091-the-figure-fills-the-frame.md) Phase 6 (2026-08-16).
**Owner if taken:** `architect` (it owes an ADR — see the routes below), then `dev`.

- **Verified 2026-08-16** — the scene's scalar is a distance, so its level sets are offsets by
  construction: `present: definition of an offset curve in: core/src/render/scenes/shape_field.rs`
- **Verified 2026-08-16** — the normalization that makes it so:
  `present: at that deepest point in: core/src/render/scenes/marks.rs`

### The finding

[ADR-0105](adrs/0105-the-mark-roster-becomes-a-fullscreen-distance-field.md) chose "a band of the
palette coordinate is a band of constant distance, **which is the definition of an offset curve**",
and the scene delivers exactly that. The user's reference — nested heart contours, red on black —
is **not** an offset family. Its inner rings stay sharply heart-shaped down to a small core, which
only **self-similar scaled copies** do.

The mechanism is geometry and it is not tunable. An inward offset is an erosion, and erosion
**rounds a reflex corner while keeping convex ones sharp**. On the heart that means the bottom
point stays crisp and the top notch fills in — which is precisely the artifact the user asked the
content lane to fix and it could not.

**The cost is a hard coupling, not a difficulty.** The core's inner boundary sits at
`d = ((1/palette_steps) / color_span)^(1/gamma)`, so a notch sharp enough to read needs
`palette_steps * color_span ~ 1` — which leaves **one** band inside the figure. Measured at 9
steps on the heart:

| core sits at | notch rounding | rings inside the figure |
|---|---|---|
| 0.48 | 0.33 | four — what ships |
| 0.68 | 0.20 | two |
| 0.81 | 0.12 | one |
| 0.91 | 0.06 | none: a black heart with a red rim |

The user judged the last one in the running app and **rejected it**; `shape_pulse` keeps the
rounded core. So "many rings inside **and** a small sharp core" is unreachable today, and it is the
reference's whole construction.

### What a fix would be

A second coordinate mode on the scene: a **shape-radius** rather than a distance — for a region
star-shaped about its centre, `r / r_boundary(theta)`, which is `0` at the centre and `1` on the
outline like the distance is, but whose level sets are *scaled copies*. It would decouple ring
count from notch sharpness entirely, and both would then be free parameters.

**It owes an ADR when taken**, and the rejected alternatives are already visible: reusing
`kaleido_radial` (it nests shrinking copies periodic in `log r`, but it nests the **frame** about a
screen point rather than the figure about its own centre, so it cannot follow `pan_*` or a shape);
and doing it in the palette (what `shape_pulse` does today — stripes packed as gradient stops,
which fakes the ring *count* but cannot change what the level sets are shaped like).

### Priority

**Medium.** Nothing ships broken and the first world landed without it. It is the difference
between "the construction resembles the reference" and "the construction *is* the reference", on
the one family this project has now had two batches of user reference images for.

---

## 0097 — a curved or jittered `star` returns a NEGATIVE normalized distance at its own centre, and on `shape_field` that is a hole through the figure

> **PROMOTED 2026-08-16** — [Plan 0098](plans/0098-the-figure-nests-properly.md) Phase 1, placed
> first because it is on the file the rest of that plan extends.

**Raised by:** `preset-author`, building the Phase 6 star probes for
[Plan 0091](plans/done/0091-the-figure-fills-the-frame.md) (2026-08-16).
**Owner if taken:** `dev`.

- **Verified 2026-08-16** — the branch that produces it:
  `present: select\(nearest, -nearest in: core/src/render/scenes/marks.rs`

### The finding

`marks.rs` documents the roster's normalization as `0` at the shape's deepest interior point,
exactly `1` on the outline. The `star` arm honours that on its **straight-edge** branch, which
returns `r*cos(f) + r*sin(f)*B` and is therefore `0` at `r = 0`. Its **curved/jittered** branch —
taken whenever `star_curve` or `star_jitter` is non-zero — returns `1 + sd/inradius` with `sd` the
true nearest distance, so at the centre it returns `1 - k/inradius`, where `k` is the valley radius.

**That is always negative, and provably so rather than incidentally.** `inradius` is the
perpendicular from the origin to the edge *line*, and a perpendicular to a chord is never longer
than either endpoint's radius — so `inradius <= k` for every configuration, hence `d(0) <= 0`
always. Measured across the probe set: `-0.23` (valley 0.20, 4 points), `-0.30` (0.12, 4),
`-0.30` (0.45, 6), `-0.75` (0.45, 9), `-0.94` (0.18, 7).

**On `shape_field` it is visible and ugly.** The palette repeat-addresses, so a negative coordinate
wraps to the gradient's far end and punches a hard n-sided dark hole through the middle of the
figure — hexagonal on a six-pointer, nine-sided on a nine-pointer.

**Nothing in the suite can see it.** On the particle path a negative `d` only makes
`max(0, 1 - d)` exceed 1 and the falloff saturates brighter, so no golden baseline moves; and no
shipped preset drives `shape_field` with a star.

### Two consequences the entry did not state, added 2026-08-16 from the content lane

Both surfaced while authoring `presets/shape_facet.toml`, and the first is the one that matters
most because **it defeats this entry's own recommended workaround**.

- **`gamma` becomes unusable, and `color_center` cannot rescue it.** The shader takes
  `select(pow(d, gamma), d, gamma == 1.0)` (`shape_field.rs:202`), and `pow` of a negative base is
  **NaN** — which lands as a hard artifact through the middle of the figure. The offset this entry
  recommends is applied on the *next* line, `coord = shaped * color_span + color_center`
  (`shape_field.rs:203`), so it arrives **after** the exponent and cannot repair a NaN that already
  happened. On any curved or jittered star, `gamma` must therefore be pinned to exactly `1.0` — the
  identity branch is the only escape. **A repair that only fixes the sign leaves this standing**, so
  Plan 0098 Phase 1 is scoped against both.
- **A binding that sweeps through `star_curve = 0` flips branches mid-morph.** The closed-form
  straight branch and the sampled Bezier branch disagree by the polyline's sagitta — ~0.0032,
  measured at Plan 0091 Phase 5 — so a param eased through zero crosses a small discontinuity at
  exactly the point an author is most likely to animate through.

### What a fix would be

The disagreement is in the **reference** the two branches divide by, not in either distance. The
straight branch's `inradius` is the edge-plane perpendicular — an approximation Plan 0091 Phase 2
recorded and deliberately kept, because repairing it would move every shipped `shape = "3"` mark.
The curved branch computes a true distance and then divides it by that same approximate reference.
Either give the curved branch a reference equal to the figure's actual deepest-point distance, or
clamp the normalized result at 0 and record that the interior is not metric there.

**Whoever takes it should read Plan 0091 Phase 2 first** — the byte-identity contract on the
particle path is the constraint that shaped the current arithmetic, and a naive repair breaks it.

### Priority

**Medium.** It is invisible until a preset puts a shaped star on the field scene, and the content
lane already hit it on its first attempt — four probe presets carry a `color_center` offset whose
only purpose is to dodge it.

---

## 0098 — `thickness` below 0.167 is a dead zone on every line scene: all values render identically and nothing says so

> **PROMOTED 2026-08-16** — folded into [Plan 0087](plans/0087-the-line-renderer-draws-a-curve.md)
> as Phase 1b, placed before that plan's Phase 4 stop gate so it cannot be orphaned if the arc work
> is abandoned. The doc half is already discharged.

**Raised by:** `preset-author`, repairing `presets/fragment_vitrail.toml` (2026-08-16).
**Owner if taken:** `dev` for the warning, `architect` for the doc line.

- **Verified 2026-08-16** — the floor that creates the dead zone:
  `present: max\(0\.0005\) in: core/src/render/scenes/lines/parametric.rs`

### The finding

`thickness` maps to an NDC-y half-width as `(thickness * 0.003).max(0.0005)`. The `.max` is a floor,
so **every `thickness` below `0.167` produces the identical half-width** — 0.0005 NDC, about 0.27 px
at 1080p, which rasterizes as a broken dotted line rather than a stroke.

`fragment_vitrail` shipped with `thickness = 0.016` — two orders below the 1.5-3.2 every other line
preset uses — so its Maurer rose rendered as scattered dots and read as gauze over the vault for
its whole shipped life.

**The dead zone is what made it expensive to find.** The content lane re-tuned `0.016` to `0.022`
to `0.038` and the picture did not change *at all*, because all three clamp to the same floor — so
the thickness hypothesis was discarded as disproved, and chord count and sample count were swept
first. The value is in range, the preset loads clean, and nothing warns.

### What a fix would be

A load-time warning when a line scene's `thickness` binding rests below the floor's own threshold,
in ADR-0020's shape (the unknown-param warning already exists and is the precedent). The floor
itself should stay — it is what stops a zero thickness degenerating the quad.

The doc half is already done: `presets/README.md` now states the working range and the dead zone.

### Priority

**Low for the engine, and the doc half is discharged.** One preset was affected and is repaired.
It is filed because the *failure mode* — a parameter range where changing the value does nothing,
with no warning — is the kind that costs a session every time someone meets it.

---

## 0099 — a narrow `color_span` silently spends the palette's resolution, and the figure comes back looking upscaled

**Raised by:** `architect`, from a user look call on the Plan 0091 Phase 6 star probes (2026-08-16).
**Owner if taken:** `dev` for a warning; the authoring half is already documented.

- **Verified 2026-08-16** — the LUT is a fixed 256 texels, so a coordinate range is a resolution
  budget: `present: LUT_SIZE: usize = 256 in: core/src/render/palette.rs`
- **Verified 2026-08-16** — and it is sampled with linear filtering, which is what turns too few
  texels into a soft edge rather than a stepped one:
  `present: fn sample_lut in: core/src/render/palette.rs`

### The finding

The user's verdict on the star probes was that they looked **"dirty and upscaled"**. That was read
first as a silhouette complaint and second as a shading one. It was neither: **the figures were
drawn through 8 to 32 of the palette's 256 texels.**

`shape_field` normalizes its distance by each shape's inradius, and a sharp star's inradius is tiny
(`0.093` against the heart's `0.637`), so the frame corner reaches `d = 26.8`. Keeping one palette
sweep on frame therefore caps `color_span` near `0.037` — and the figure's whole interior, `d` in
`0..1`, then occupies **9.6 of 256 texels**, with the silhouette edge transition spanning **1.31**.
A 1.3-texel transition stretched across half a screen, sampled with linear filtering, is exactly an
upscaled gradient, and that is what was on the screen.

| probe | figure occupies | edge transition |
|---|---|---|
| `p3a sharp7` | 8.6 texels | 1.31 |
| `p3d sparkle4 deep` | 8.4 | 1.25 |
| `p3c sparkle4` | 14.8 | 2.23 |
| `p3b bang9` | 25.0 | 3.76 |
| `p3e hand6` | 32.3 | 4.86 |

**Re-rendered with `palette_steps` bound, the same five silhouettes come back crisp** — because the
band quantizer snaps the coordinate to a band *centre* before the LUT read, so every pixel samples
one exact texel and no edge is ever interpolated. The silhouettes were exact all along; the probe
was spending its resolution on nothing.

### Why it is worth an entry

**Nothing warns, and the failure presents as a different defect.** A preset author sees a soft,
crawling figure and reasonably concludes the *shape* is wrong — which is precisely what happened
here, and it nearly routed a lighting plan
([0092](design-backlog.md)) off a misread. The trap is silent, the value is in range, and the
symptom names the wrong subsystem.

It is also **the third member of a family this project keeps meeting**: a parameter range where the
engine quietly stops honouring the value — `thickness` below `0.167`
([0098](design-backlog.md)), `color_span` not being portable between shapes (documented in
`presets/README.md`), and now a `color_span` narrow enough to starve the gradient.

### What a fix would be

Cheapest and probably right: a **load-time warning** when a `shape_field` preset's `color_span`
puts the figure's own `0..1` interior below some small number of LUT texels — the ADR-0020 warning
surface again. It cannot be exact, because how much of the coordinate the *figure* occupies depends
on the shape's inradius and the framing, but the scene knows both.

Two things that are **not** the fix. Enlarging `LUT_SIZE` moves the threshold without removing the
trap and costs every scene memory. And nearest-filtering the LUT would harden the edge while
turning every smooth gradient in the library into bands.

**The authoring workaround is real and already documented:** bind `palette_steps`, which removes
the interpolation entirely.

### Priority

**Low-Medium.** One probe set was affected and no shipped preset is — `shape_pulse` binds
`palette_steps` and is unaffected by construction. It earns its place because of what it cost: a
user look verdict was misattributed twice, and the entry that nearly absorbed the blame
([0092](design-backlog.md)) is a composite-scale piece of work.

---

## 0100 — "hand-drawn" is edge wobble, not spike-length variation, and the star arm has no lever for it

**Raised by:** `architect`, from Plan 0091 Phase 6's star look gate (2026-08-16).
**Owner if taken:** `architect` then `dev`. **Low priority and the user said so in the same breath
as passing it** — this entry exists because the *reason* it fell slightly short is specific and
worth not rediscovering.

- **Verified 2026-08-16** — the only per-spike variation the arm has is a tip-radius scale, drawn
  from an index hash: `present: rt = 1\.0 \+ jitter in: core/src/render/scenes/marks.rs`
- **Verified 2026-08-16** — and there is no seed or phase input to re-scatter it:
  `absent: star_seed in: core/src/render/scenes/marks.rs`

### The finding

Plan 0091 Phase 5 shipped three `star` shape params against a batch of six reference images. Phase 6
judged them, and the verdict was **yes on all five silhouettes**, with one soft exception: the
hand-drawn six-pointer *"maybe"* reads as hand-drawn, *"but it's fine also"*.

That soft edge has a specific cause. `star_jitter` varies each spike's **tip radius** — one scalar
per spike, drawn from `mark_spike_hash01(index)`. A hand-drawn figure does not vary that way: its
spikes are roughly the right length and its **edges wander** — the line between tip and valley is
not straight or cleanly bowed, it wobbles along its length. That is **displacement along the edge**,
a different quantity from the one the arm exposes, and `star_curve` cannot supply it either because
it bows the whole edge as one smooth quadratic.

Two smaller consequences of the same shape, both real and neither urgent:

- **There is no lever to re-scatter the jitter while keeping its amount.** The pattern is a pure
  function of the spike index, so a preset that wants *this much* irregularity in a *different*
  arrangement cannot ask for it. That is the price of the determinism rule the arm correctly follows
  (a `sin`-based hash would differ between GPUs), but a `star_seed` input would keep determinism and
  restore the choice.
- **A jittered star's exterior is the roster's least accurate field** — up to 0.54 out, because the
  angular fold measures against a point's own spike when a longer neighbour may be nearer
  (Plan 0091 Phase 5 measured it). Edge displacement would make that worse, so anything built here
  should be judged against that number rather than assumed free.

### A record defect, worth one line

[Plan 0091](plans/done/0091-the-figure-fills-the-frame.md) line 124 says this item was **"separately
filed rather than absorbed"**. It was not — no entry existed until this one, five days later, and
the claim was never checked. It is the same class
[ADR-0108](adrs/0108-a-backlog-claim-about-the-repo-carries-an-executable-probe.md) exists for, one
level up: a plan asserting something about the backlog rather than about the code.

### What a fix would be

Not obvious, and that is why this is filed rather than planned. A displacement term needs a
coordinate along the edge and a noise source that is cheap in a fragment shader and stable across
GPUs — the same constraints that made the tip jitter an integer hash. It is plausibly a
`star_wobble` amplitude plus a frequency, evaluated on the folded edge parameter the arm already
computes.

### Priority

**Low.** The look gate passed the silhouette. Take it when someone wants a *deliberately* rough
figure rather than a slightly irregular one, and read the exterior-accuracy number first.

---

## 0101 — the mark roster cannot morph between silhouettes, and two other rosters in this engine already do

**Raised by:** `preset-author`, from the user's "can we morph the shape with music" at
`presets/shape_facet.toml`'s authoring (2026-08-16). **Owner if taken:** `architect` — it owes an
ADR, and it is a real design question rather than a missing knob.

- **Verified 2026-08-16** — the selector is rounded, so a fractional index selects no arm:
  `present: clamp\(MIN_SHAPE, MAX_SHAPE\)\.round\(\) in: core/src/render/scenes/marks.rs`
- **Verified 2026-08-16** — the same file states the roster is a list of identities rather than a
  quantity: `present: values are \*identities\* rather than a quantity in: core/src/render/scenes/marks.rs`

### The finding

**Within an arm, morphing already works and needs nothing** — `star_valley`, `star_curve` and
`star_jitter` are clamp-only, so a binding drives them continuously and the silhouette genuinely
deforms with the music. That was demonstrated and it is the answer to the question as asked.

**Between arms it is a cut.** `mark_shape` rounds, deliberately: ADR-0084's roster is five
identities and a fractional index selects no arm at all. So a preset can switch `star` to `heart`
on a beat but cannot travel between them, and an eased binding steps rather than morphs.

**What makes this worth an entry rather than a shrug is that this engine has twice decided the
opposite for other rosters.** [ADR-0060](adrs/0060-star-pattern-variants-interpolate.md) makes the
star-pattern variants **interpolate**; [ADR-0075](adrs/0075-ifs-family-morphs-in-singular-value-space.md)
morphs the IFS family in **singular-value space** — a considered answer to exactly the question
"these are discrete entries, so how do you travel between them". So "roster entries interpolate" is
an established idea here, and the newest roster is the one without it.

### Why it is a design question and not a knob

A naive lerp between two signed-distance functions is **not** a shape in any principled sense — the
result is a field whose zero set is some blend that neither arm describes, and whose interior may
not even be connected. It is nonetheless a well-known technique that often looks good, which is
precisely the kind of trade an ADR exists to record rather than to have someone discover in a
preset.

Two constraints any answer has to meet:

- **The particle path shares this chunk.** `swarm` and `emitter` read the same `mark_distance`, so a
  morph capability arrives there too, and its default must be an exact identity or every shipped
  `shape`-bearing preset moves (ADR-0105's shared-chunk consequence).
- **It collides with [ADR-0111](adrs/0111-the-shape-field-gains-a-scaled-copy-coordinate.md).** That
  decision adds a second per-arm scalar (`r_boundary`), so a morph would have to blend *both* the
  distance and the boundary radius, or be defined only for the distance coordinate. Whoever takes
  this should read 0111 first — and if both are wanted, they are probably one plan rather than two.

### Priority

**Low, and it is genuinely optional.** The question that prompted it is already answered by the
three continuous star params. This is filed so that if someone later wants a figure that travels
from a heart to a star across a phrase, the precedents and the constraints are in one place instead
of being re-derived.

## 0102 — a foobar panel attaches its surface at 1x1 and only a stream-format change ever revives it

Found at [Plan 0097](plans/done/0097-the-track-announces-itself.md)'s Phase 6, on the reporter's own
foobar2000 v2.25.10 — **not** by this plan's changes, and it reproduces on a build from the commit
before it.

A Default UI panel is created **0x0** and sized by the layout afterwards. `VizSession::claim` runs
on `WM_CREATE`, so `ensure_handle` attaches the wgpu surface at the `1x1` fallback and sets
`needs_reattach = (w == 0 || ht == 0)`. The shim expects the first real `WM_SIZE` to call
`reattach_at_current_size()` — and in practice that does not reliably happen. The panel then
**renders without presenting**: `lmv_get_metrics` reports a healthy `draw_calls` (30-31 observed)
against a black panel, and `gpu_bytes` reports the *config* size rather than the surface's, so it
looks correctly sized while nothing appears.

What actually revives it is unrelated: `claim` starts the handle at a default `48000/2`, so the
first audio chunk of ordinary 44.1 kHz material triggers `ensure_handle(44100, 2)`, which destroys
and recreates the handle **at the owner's now-real client size**. That is why the panel can sit
black and then come alive on a track boundary, with no user action — the reporter's words were
*"gut feeling that it started working after the next track came by itself"*.

- **Verified 2026-08-16** — the degenerate-attach flag exists and is set from the client rect:
  `present: needs_reattach = \(w == 0 \|\| ht == 0\) in: plugin-foobar/foo_lmv.cpp`

### Why it is filed rather than fixed

[Plan 0097](plans/done/0097-the-track-announces-itself.md) fixed a *different* pre-existing defect in
the same file under an explicitly approved scope expansion (the render timer, `1016777`). A second
patch to the same window/ownership path in the same session would have been a third guess layered on
two — this one wants a design pass over surface lifetime, not another edge case handled.

### What a fix would have to decide

Whether the panel should **defer the attach** until it has a real client rect (claim without a
surface, attach on first non-degenerate `WM_SIZE`), or whether `needs_reattach` should be re-checked
from the same watchdog `1016777` added, which already re-derives visibility from the window every
500 ms and is the obvious place to also notice a surface that never became real.

### Reproduced independently 2026-08-16, on a second machine, with a worse symptom

At [Plan 0102](plans/done/0102-the-component-ships.md)'s Phase 5 — the released component installed
into a foobar2000 v2.25.10 profile on the dev box. **The revival mechanism this entry names was
confirmed exactly**, and the symptom it describes was not the one observed.

What was not seen: the panel was never black. It came up rendering a correct, well-formed attractor
at full panel size, which is why nothing about it looked wrong.

What was seen instead, from `plugin-diagnostics.log`: **6.5 fps, `frame_ms_avg` 135 -> 154 ms**,
`frames_total` advancing exactly 7 per second-sample — **from the first sample of the session**, not
degrading into it. One thread pegged `Running` at 52 of the process's 69 CPU-seconds over 66 s of
uptime. `Responding` stayed `True`, so nothing was deadlocked; foobar2000's own status bar simply
froze at `0:00 / 3:21` under playing audio and the playlist view showed no rows, because the host
paints on the thread the renderer was consuming. **The user's report was "the plugin and interface
is completely stale", and the interface half is the part this entry does not predict.**

The recovery was this entry's own path, arrived at accidentally a second time: adding an album to
the **playing** playlist put 44.1 kHz material through `ensure_handle`, and frame cost went to
**17.6 ms at 56-58 fps** — `8.7x`, with `draw_calls` (30-31), `gpu_bytes` (2024640, byte-identical)
and the preset all unchanged across the transition. That invariance is what rules out the obvious
alternative: a cost that large which vanishes on an unrelated event is not the preset, the quality
tier, or the GPU being busy.

- **Verified 2026-08-16** — the field an operator would reach for cannot arbitrate this, exactly as
  this entry already says: `gpu_bytes` was identical in the 6.5 fps and 57 fps stretches:
  `present: gpu_bytes in: plugin-foobar/foo_lmv.cpp`

**What this adds to the diagnosis.** A surface attached at a size that does not match the window
does not only fail to present — it can present *expensively*, which looks like nothing being wrong
at all. A fix that only restores the black case would leave this one standing, so the deferred-attach
option in the section above is the safer of the two: re-checking `needs_reattach` from the watchdog
repairs a surface that never became real, but this session's surface **did** become real enough to
draw a correct picture.

**The window is "panel creation until playback actually starts", and that is what makes the severity
so variable.** Follow-up on the same box, once the playlist had content: a **brief** slow patch at
the start of the first track on a fresh foobar2000, then correct for the rest of the session. That
is the same defect with a short window, and it reconciles the two observations — the two-minute
episode above was a session where **playback had not started at all** (title bar carried a track
from the previous run, status bar sat at `0:00`, playlist empty), so no chunk had yet reached
`ensure_handle` and nothing was scheduled to fix it. Press play early and the window is a moment;
browse the library first and it lasts until you do.

**That is the wrong way round for a new user.** Someone who has just installed a visualizer
component opens it and *looks* at it before playing anything — which is precisely the path that
holds the bad state open. The severity is inversely proportional to how quickly the user does the
one thing that hides the bug.

**What is not established.** Whether the slow present is the same degenerate attach or a second,
adjacent defect in the same lifetime; nothing here measured the surface's actual configured size,
because no instrument in this repo reports it. That gap is the first thing a fix should close.
Nor was playback state at the start of the long episode captured directly — it is inferred from a
status bar that the same defect was starving, so treat it as the reading that fits both runs rather
than as an observation.

### Priority

**Was Medium, raised to High 2026-08-16.** The original grounds were that it is the first thing a
new plugin user sees and looks exactly like a broken component. The reproduction above is worse than
that on two counts: the component now **ships** ([Plan 0102](plans/done/0102-the-component-ships.md),
`v0.70.0`), so a stranger meets this rather than a developer; and the failure is not confined to our
panel — it makes **foobar2000 itself** feel dead, with no visible cause and nothing in the console.
Compounding it, [0103](design-backlog.md) means the user cannot remove the panel by the documented
route to escape. Whoever picks this up should read the two together.

## 0103 — the plugin's context menu shadows foobar's, so the panel cannot be removed from a layout

Found the same session, the same way, and also pre-existing.

`wnd_proc`'s `WM_CONTEXTMENU` case shows the component's own menu (`Next scene` / `Diagnostics
overlay`) whenever this window owns the session, and **never consults foobar's layout-edit state**.
In Default UI's layout editing mode the host expects a panel's right-click to surface *its* menu
(Cut / Copy / Replace / Remove); ours wins instead, so the panel cannot be removed by the documented
route. The reporter hit exactly this: *"in layout setup I'm not able to remove visualizer — its menu
overrides setup menu"*. The workaround is Preferences → Display → Default User Interface's layout
tree, which is not discoverable from the panel.

- **Verified 2026-08-16** — nothing in the shim asks the host whether layout editing is on:
  `absent: is_edit_mode_enabled in: plugin-foobar/foo_lmv.cpp`

### What a fix would have to decide

`ui_element_instance_callback` exposes the edit-mode query; the panel path can consult it and fall
through to `DefWindowProc` when editing is on. The **pop-out** host has no such callback and no
layout to edit, so the two hosts stop sharing one `WM_CONTEXTMENU` branch — which is the design
question, since sharing `wnd_proc` between both host kinds is deliberate in this file.

### Priority

**Medium-low.** One workaround exists and works, but it is undiscoverable, and "I cannot remove your
component from my layout" is a bad first impression.

## 0104 — `check-index-rows.mjs` has no assertion that it can convict, so a dead detector reads exactly like a clean tree

Found at Plan 0105's close (2026-08-16), reviewing the gate that plan built.

Of the three checkers in `scripts/`, this is the only one whose fixture asserts **exit 0**.
`scripts/fixtures/README.md` argues the inversion is correct — *"a byte cap is trivially red on any
tree with a fat row in it, so the interesting assertion is the reverse"* — and that was true of the
tree it was written against, which still held 136 over-cap rows. **Phases 2-4 of the same plan made
it false.** Nothing in the repo now contains a row the gate would reject, so nothing anywhere
exercises its ability to reject one.

The consequence is that the checker's row detection is unasserted. Demonstrated at the close by
copying the script and replacing `TABLE_ROW` and `BULLET` with regexes that match nothing: the
fixture reports `3 regions, 0 rows, 0 over cap` and **exits 0**, and so does the repo run. Every one
of the three call sites — pre-push, the CI `links` job, the architect close ceremony — goes green.
The per-file region and row counts the script prints are the documented mitigation, and they are
*printed*, not asserted; nothing compares them to an expected number.

This is the same non-vacuity class the other two gates were repaired for.
[Plan 0084](plans/done/0084-two-gates-stop-lying-about-what-they-check.md) found the link checker
covering one of markdown's two link forms, and
[Plan 0094](plans/done/0094-the-two-doc-gates-check-what-they-claim-to.md) found a directory-name
skip swallowing a real tree and a whole half of ADR-0108's rule invisible to a bullet-driven check.
Both now ship fixtures that expect **exit 1 with an exact break count**, and
`check-backlog-claims.mjs` additionally carries a `--self-test` whose non-vacuity assertion is
pinned to the real repository rather than to the fixture.

- **Verified 2026-08-16** — the script has no self-test:
  `absent: self-test in: scripts/check-index-rows.mjs`
- **Verified 2026-08-16** — its fixture is documented as the one that passes rather than fails:
  `present: Expect \*\*exit 0\*\* in: scripts/fixtures/README.md`
- **Verified 2026-08-16** — the sibling gate has the mechanism this one lacks:
  `present: --self-test in: scripts/check-backlog-claims.mjs`

### What a fix would be

Two shapes, and they are not exclusive. A **second fixture root** — `scripts/fixtures/index-rows-red/`
with one over-cap row inside a marked region, run as its own root and expected to exit 1 with
exactly one break — keeps the existing green fixture's four negative assertions intact rather than
flipping them. Or a **`--self-test`** on the model `check-backlog-claims.mjs` already carries,
asserting the green fixture's own counts (3 regions, 4 rows) so a detector that finds nothing fails
loudly. The `--self-test` is the cheaper of the two and covers the demonstrated mutation; the red
fixture additionally covers the reporting path, which nothing currently runs either.

[ADR-0116](adrs/0116-an-index-row-is-a-pointer-and-a-gate-holds-it-to-one.md) names the *marker*
hole in its Negative section and pins it as deliberate behavior in the fixture. It does not name
this one, and its dated `Outcome` now says so.

### Priority

**Medium.** Nothing is broken today — the gate was verified working by hand at Plan 0105's close,
and the fix is small. What makes it worth an entry is the failure mode: a silent one, in a gate
whose entire argument ([ADR-0033](adrs/0033-testing-strategy-coverage-ratchet-and-pre-push-gate.md))
is that a rule nothing re-runs is a rule nobody follows. A check that re-runs and cannot fail is the
same rule wearing a green tick.

## 0105 — the component's READ-ME-FIRST states the SDK it was built against, and on the pre-staged route nothing checks that claim

Found at [Plan 0102](plans/done/0102-the-component-ships.md)'s close (2026-08-16), reviewing the
recipe that plan built.

`packaging/foobar/build-component.ps1` substitutes `@SDK_VERSION@` into the shipped
`READ-ME-FIRST.txt` from `packaging/foobar/sdk-pin.ps1`'s `$LmvSdkVersion` — the **pin**, which is
what the recipe intends to have been built against. What it verifies about the SDK actually on disk
is one existence test, `plugin-foobar/sdk/foobar2000/SDK/foobar2000.h`. The two are the same fact
only on the fetch route, where `fetch-sdk.ps1` downloaded the pinned archive and checked its
SHA-256.

[ADR-0115](adrs/0115-the-foobar-component-is-a-released-artifact-with-a-parameterized-sdk.md) makes
the **pre-staged** route first-class — "how the SDK reaches the build host is a parameter of the
recipe rather than a property of it" — and `plugin-foobar/README.md` documents unpacking it by hand.
On that route a developer with an older SDK unpacked at `plugin-foobar/sdk/` produces a component
whose reader-facing document asserts a build against 2025-03-07, with every one of the recipe's
seven fatal checks green. Nothing downstream can tell, because the SDK version is not in the DLL.

The fix is cheap and the recipe is one grep short of it: the SDK archive ships `sdk-readme.html`
carrying `<h1>foobar2000 SDK, version 2025-03-07</h1>`, so the staged tree states its own version
and `build-component.ps1` can fail when it disagrees with the pin instead of asserting over it. That
also closes the smaller half — the script's own `ok: SDK <version> staged` line prints the pin, not
what is staged.

- **Verified 2026-08-16** — the recipe never reads the SDK's own version marker:
  `absent: sdk-readme in: packaging/foobar/build-component.ps1`
- **Verified 2026-08-16** — but it does stamp a version claim into what ships:
  `present: @SDK_VERSION@ in: packaging/foobar/build-component.ps1`
- **Verified 2026-08-16** — the only thing asserted about the staged tree is that a header exists:
  `present: foobar2000.h in: packaging/foobar/build-component.ps1`

### Priority

**Low.** CI takes the fetch route, so nothing published today can carry the wrong claim, and the
window is a developer who hand-staged a different SDK and then shipped that build. It is filed
because the recipe's whole argument is that a local run is held to CI's bar
([ADR-0038](adrs/0038-tag-driven-release-unsigned-universal-mac-app.md)'s model, applied by
ADR-0115) — and this is the one assertion where the local route is held to a looser one.

---

## 0106 — converted MilkDrop presets wash out or invert: the float feedback field never truncates

- **PROMOTED 2026-08-17 → [ADR-0118](adrs/0118-the-milkdrop-feedback-field-quantizes-in-the-encoded-domain.md) +
  [Plan 0108](plans/0108-the-milkdrop-import-gets-its-tone-back.md) Phases 1-2.** The entry's own
  design call — what "8-bit truncation" means in linear light — is answered with the arithmetic it
  asked for: one 8-bit sRGB step is `3.03e-4` in linear, so the literal `1/255` floor the entry warns
  against truncates everything below sRGB level ~13, **13x too aggressive**. The decision quantizes
  in the encoded domain instead, per bundle and driven by a uniform. The entry stays **live** until
  that plan lands, and its re-test trigger is that plan's Phase 2.

**Raised by:** `architect`, from Plan 0100 Phase 7's side-by-side judgment (2026-08-16).
**Owner if taken:** `architect` (the emulation is a design call) then `dev`. **The dominant
fidelity defect of the MilkDrop import, and the one gating the plan's central claim** — the HDR
"better or merely different" verdict came back *merely different* and cannot be fairly re-judged
until this is fixed.

- **Verified 2026-08-16** — the warp pass scales the previous frame by a decay term and writes the
  product back with no low-end floor: `present: decay in: core/src/render/scenes/warp_mesh/mod.rs`
- `unprobeable:` the defect itself is a rendered divergence against an external reference
  (`foo_vis_milk2` 0.2.0.0, DX11) and lives in no greppable line; the evidence is the seven
  side-by-side pairs recorded in Plan 0100's Phase 7 section.

### The finding

MilkDrop's feedback target is 8-bit: `decay` times a dim pixel **truncates to zero**, and that
quantization is what keeps a classic preset's background black and its trails finite. This engine's
field is `Rgba16Float` — nothing truncates, every dim residual survives and accumulates. Judged
over seven presets side by side (Plan 0100 Phase 7, 2026-08-16), one mechanism with four
presentations: pastel wash (*Songflower*, *Cosmic Dust 2*), white-hot glow (*Contortion*), runaway
to the clamp with per-channel fringing (*chasers 19 Portal*), and full tonal **inversion** — *Fog
Tunnel*, a dark preset rendered on a white plateau. The control ran the other way: *Blur Mix 3*,
whose blur chain actively darkens, kept its blacks and looked genuinely good — which is what
scopes the defect to the feedback path rather than the shader translation.

### What a fix would be

An opt-in (per-`[milk]`-bundle, not engine-wide) floor in the warp epilogue emulating the
reference's quantization — Butterchurn and projectM both carry an equivalent, so the shape is
known. The design call is what "8-bit truncation" means in linear light: the reference truncates
in its gamma-encoded target, so a literal `1/255` floor in linear is wrong at both ends. Measure
against the same seven pairs.

### Priority

**High within the import lane.** Plan 0100's Phase 7 verdict is provisionally negative on the
plan's own motivating claim; this entry is the re-test trigger.

---

## 0107 — the MilkDrop draw layer misplaces figures, and two warp-path defects mirror or unfold the frame

> **THIS ENTRY'S OWN LEADING SUSPECT IS FALSIFIED — 2026-08-17. Do not start where item 2 says to.**
> It says to *"check `s_fw`'s address mode against the reference's toroidal wrap first"*.
> `core/src/render/scenes/warp_mesh/shader.rs:463` already builds `s_fw` with
> `wgpu::AddressMode::Repeat`, and Repeat produces a **shifted** copy — not the **reflected** one the
> entry's own fingerprint paragraph describes. The address mode cannot be the cause of a reflection.
>
> The replacement hypothesis, with arithmetic, is
> [Plan 0108](plans/0108-the-milkdrop-import-gets-its-tone-back.md) Phase 3: the polar pair is built
> with a y-negation (`emit.rs:1418`, `vec2<f32>(2.0, -2.0)`), so a preset reconstructing `uv` from
> `ang` recovers `uv_orig.y` reflected about 0.5 — a mirror about the horizontal midline with its
> seam on the fixed line. The negation is provably correct for the EEL per-vertex program; whether
> it is correct in the **pixel** shader is the open question, and it is answerable in one render.

- **PROMOTED 2026-08-17 → [Plan 0108](plans/0108-the-milkdrop-import-gets-its-tone-back.md)
  Phases 3-5, no ADR** — three unknown-cause hunts, one phase each, **each carrying a stop
  condition** (a phase that cannot reproduce its defect commits the fixture, records what it ruled
  out, and the plan continues). That shape was the user's call over committing to fix all three,
  which this entry's falsified suspect shows would have been dishonest.

**Raised by:** `architect`, from Plan 0100 Phase 7's side-by-side judgment (2026-08-16).
**Owner if taken:** `dev` (mechanism hunts), `architect` if a fix needs a design call.

- **Verified 2026-08-16** — the wave placement code under suspicion is the warp-mesh draw layer:
  `present: wave_mystery in: core/src/render/scenes/warp_mesh/draw.rs`
- `unprobeable:` the three defects are rendered divergences against an external reference; the
  evidence is the pair record in Plan 0100's Phase 7 section.

### The finding

Three distinct defects, each seen in at least one pair against `foo_vis_milk2`:

1. **Waveform placement/orientation.** *Blur Mix 3* draws one steep diagonal stroke where the
   reference draws horizontal full-width traces — suspect the `wave_mystery` angle mapping or the
   mode geometry. *Cauldron painterly 5*'s centrepiece spiro scribble is entirely absent.
   *Cosmic Dust 2*'s beaded `wave_usedots` trails never appear. (The mono engine drawing one trace
   where stereo draws two is known and accepted — Plan 0100 Phase 4; these are beyond it.)
2. **A horizontal reflection seam in warp sampling.** Content mirrors across a horizontal line
   with a bright ragged boundary: *Contortion*'s split sphere, *Cauldron*'s flipped top band,
   *Cosmic Dust 2*'s full-width false horizon. The fingerprint (reflected copy, not shifted)
   points at the wrap path's sampler address mode or a v-axis flip — check `s_fw`'s address mode
   against the reference's toroidal wrap first, not the `ang` discontinuity first suspected.
3. **`chasers 19 Portal`'s mirror symmetry never takes effect** despite a clean conversion — the
   uv-fold that makes the portal is inert. One targeted reproduction wanted.

### Priority

**Medium-high within the import lane.** Item 2 is likely one sampler descriptor; item 1 decides
whether waveform-led presets (the whole *Blur Mix* / *Fog Tunnel* family) read as authored.

---

## 0108 — the conversion tail: HLSL arrays (~71 files) and 218 MD2 presets that convert but render blank

**Raised by:** `dev` (Plan 0100 Phase 6 log, "followup noticed, not acted on"), filed by
`architect` at the close (2026-08-16). **Owner if taken:** `dev`.

- **Verified 2026-08-16** — arrays are a named rejection class, not a silent drop:
  `present: array declaration in: milkconv/src/shader/parse.rs`
- `unprobeable:` the counts (71, 218, 80.1 %) are a measurement of one corpus run (2026-08-16,
  dev box), reproducible with `milkconv --report`/`--render` over `WORK/milkdrop-corpus`, not a
  property of this tree; both eras' tables are in `docs/capturing.md`.

### The finding

After Phase 6, the corpus converts at 80.1 % (8 289 of 10 347) and renders non-blank at 77.9 %.
The residual worth work, in order: **218 MD2 presets convert but render blank** — with their
shaders now running, these are fidelity findings (most plausibly warp shaders that supply no light
of their own whose source was a refused disk texture), and backlog 0106/0107's fixes should be
re-measured against them before any new mechanism is hunted. **~71 files use HLSL arrays**, today
a named `unsupported` rejection; a bounded-size array lowering in the frontend would recover them.
The `emitter-invalid` class (naga refusing our own emission) ended Phase 6 at zero and should stay
there.

### Priority

**Low until 0106/0107 land** — the blank list is contaminated by both, so counting it again first
is wasted; re-run `--render` after they land and re-rank.

---

## 0109 — disk textures are 88.7 % of every MilkDrop conversion failure, and the exclusion's trigger condition is already met

**Raised by:** `architect`, at Plan 0108's planning sweep (2026-08-17), reading
[ADR-0113](adrs/0113-milkdrop-presets-are-translated-ahead-of-time-onto-a-warp-mesh-idiom.md)'s
Outcome and [Plan 0100](plans/done/0100-the-engine-speaks-milkdrop.md)'s followup list against
`docs/capturing.md`'s measured corpus tables. **Owner if taken:** `architect` (it reopens a scoped
exclusion and is ADR territory) then `dev`.

- **Verified 2026-08-17** — the exclusion is a named rejection class, deliberately, and says so in
  its own message: `present: fn disk_texture in: milkconv/src/shader/emit.rs`,
  `present: deliberately out of scope in: milkconv/src/shader/emit.rs`
- **Verified 2026-08-17** — the corpus tables this entry re-reads are in the operator doc, both
  eras: `present: WHY A FILE DID NOT CONVERT, ranked in: docs/capturing.md`
- `unprobeable:` the counts (1 217 / 609 / 2 058 / 88.7 %) are a measurement of one corpus run
  (2026-08-16, dev box, `WORK/milkdrop-corpus`), reproducible with `milkconv --report`, not a
  property of this tree

### The finding

**This entry claims nothing new about the mechanism. It claims the ranking was already decided and
nobody carried it.** Plan 0100's followup list says *"MilkDrop's `textures/` support, if Phase 5's
failure ranking says it is a large class."* Phase 5 ran, Phase 6 ran after it, and the ranking is
in `docs/capturing.md`:

| Rejection reason | Files | Share of corpus |
|---|---|---|
| warp shader `disk-texture` | 1 217 | 11.8 % |
| comp shader `disk-texture` | 609 | 5.9 % |
| **every other cause combined** | **232** | **2.2 %** |

Total conversion failures are `10 347 - 8 289 = 2 058`. Disk textures are **1 826 of them —
88.7 %**. Every other named cause in the whole corpus — HLSL arrays, computed conditions, parse
failures, unknown names, the EEL program classes — sums to 232 files.

So the conditional in Plan 0100's followup is **satisfied**, and by a margin that is not close.
The 19 % figure ADR-0113's Outcome prices the exclusion at was the *census grep*; the converter
sees a slightly wider class (it also flags `sampler_pc`) and measured **21.8 %** of the corpus
reading a disk texture.

### Why this is an entry rather than a line in Plan 0108

Plan 0108 is fidelity work on presets that already convert. This is the **conversion rate**, and it
is a different question with a different owner: shipping or sourcing texture files reopens
[ADR-0113](adrs/0113-milkdrop-presets-are-translated-ahead-of-time-onto-a-warp-mesh-idiom.md)'s
scope and collides directly with the provenance question Plan 0100 Phase 8 deferred (*decide
later*, nothing third-party in the repository or a release). **Those two are the same decision seen
from two sides** — a texture is third-party content exactly as a preset is — which is why this
wants an ADR and an interview rather than a phase.

It also sits *above* [0108](#0108--the-conversion-tail-hlsl-arrays-71-files-and-218-md2-presets-that-convert-but-render-blank)
in value by its own arithmetic: that entry's HLSL-array lowering recovers ~71 files, and this
recovers ~1 826. Both are conversion-rate work; only one of them is 25x the other.

### What a fix would be, and the shape is genuinely open

At least four routes, and they differ on the question this project has already deferred once rather
than on mechanism:

1. **Ship nothing, load from the user's own `textures/` directory** — the same shape as
   `LMV_PRESET_DIR` today. No provenance question at all, because nothing third-party enters the
   repository; the user who has the preset pack already has its textures. Cheapest, and the most
   consistent with Phase 8's standing answer.
2. **Substitute procedurally.** The six built-in noise textures already exist and 51 % of the corpus
   samples one. A missing disk texture could resolve to a procedural stand-in rather than a
   rejection — the preset renders *something its author did not draw*, which Phase 6 explicitly
   moved away from, so this trades fidelity for conversion rate and needs a judged look call.
3. **Ship a small curated texture set.** Highest fidelity, and it walks straight into the licensing
   question Phase 8 deferred.
4. **Keep the exclusion and stop calling it a corner.** Legitimate, and it is the null option that
   should be named: the cost is stated, the 8 289 that convert are the product, and the entry closes
   as a decision rather than as work.

### Priority

**Medium, and it is the largest single lever on the import's reach.** It blocks nothing — the
import ships and works on four fifths of the corpus. Take it when the fidelity work
([Plan 0108](plans/0108-the-milkdrop-import-gets-its-tone-back.md)) has settled whether converted
presets are worth having more of, which is the honest ordering: reach is only worth buying after
quality is judged. **Do not take it before Plan 0108's Phase 2**, whose verdict on whether these
presets read as better or merely different is exactly the evidence for how much reach is worth.

---

## 0110 — an attractor's sample budget ignores the render target, so a 1080p render reads as an upscale

- **Raised:** 2026-08-17, from [Plan 0101](plans/done/0101-the-engine-renders-a-music-video.md) Phase 5.
  The first real music video this engine produced — `attractor_leviathan` over a 4:41 track at
  1920x1080/60, `--tier rich` — came back with the verdict *"it just looks like Leviathan
  upscaled"*, and that is exactly what it is.
- **Verified 2026-08-17** — the count is a plain tier constant with no render-target term:
  `present: attractor_particles: 150_000 in: core/src/render/tier.rs`,
  `present: attractor_particles: 50_000 in: core/src/render/tier.rs`. Whether the result *looks*
  upscaled is a judgement about pixels: `unprobeable: the grain is judged by eye; only the
  constants and the grid's surface sizing are claims about the repo`.

**The arithmetic.** `TierConfig::attractor_particles` is 50,000 at `Floor` and 150,000 at `Rich`,
fixed. The trail grid *is* surface-sized (Plan 0027), so the deposit spreads over whatever the
target holds: at 1920x1080 that is 150,000 particles into 2.07 M texels, ~0.07 per pixel per
frame; in a 640x360-class window it is ~0.65, nine times denser. Going to 1080p multiplies pixels
4x while `Rich` multiplies particles 3x, so **density falls as resolution rises** and the figure
gets grainier rather than finer.

**The engine already says this in the type's own docs** and stops one step short of the
consequence: `attractor_particles` is *"a sample count and not a brightness"* whose deposit is
divided by the count (ADR-0065), so *"raising it buys a smoother figure rather than a brighter
one"*. Correct — and the count never learned that the number of pixels it is smoothing over is
not fixed.

**Why it surfaces now.** Every capture path in this repo renders small, and the live app runs at a
window size where the density is fine. Plan 0101 is the first path that renders at 1080p and asks
the result to stand on its own as a *file*, where the whole point is that it is not bounded by a
display. Offline is also where the fix is affordable — there is no 60 Hz deadline, so the sample
budget can be raised with no governor to answer to.

**Shapes, none decided.** (a) Scale the count with the target's pixel count against a reference
resolution, capped — the smallest change, and it makes `Rich` mean the same *density* everywhere
rather than the same *number*. (b) A preset-authorable density param, which puts the look in the
content lane where the rest of the attractor's look already lives. (c) Accumulate more than one
integration step per frame offline, trading render wall-clock for grain — available only to
`shot --render`, and the one that costs no live budget at all. (a) and (c) compose.

**ADR-worthy** if taken: it changes what a tier *means* (a count becomes a density), which is
exactly the class [ADR-0065](adrs/0065-the-attractor-deposit-is-normalized-by-particle-count.md) and the
`tier.rs` module header already argue about. **Priority: high for the video path, low for the
app** — nothing shipped is broken and the live tiers are validated where they are. It gates
whether a rendered file is publishable, which is the question Plan 0101 exists to make askable and
[Plan 0103](plans/0103-the-project-gets-an-audience.md) depends on the answer to.

---

## 0111 — `shot --render` spawns the encoder and builds a GPU device before it validates `--preset`

- **Raised:** 2026-08-17, hit twice while running
  [Plan 0101](plans/done/0101-the-engine-renders-a-music-video.md) Phase 5.
- **Verified 2026-08-17** — `run()` never scans the roster for membership; its only roster read is
  the one-entry `[only]` arm: `absent: presets\.iter in: standalone/src/shot/render.rs`.

**Reproduced.** `--preset attractor_leviathan` — the *filename*, where the roster key is the
preset's `name` field, `"Leviathan"` — exits 1 with `no preset named 'attractor_leviathan' in the
roster` and leaves a **262-byte MP4 at `--out`**, because `ffmpeg` had already been spawned,
consumed the muxed WAV, written a valid audio-only container and exited 0. At the 20 s trial size
the leftover was 26 KB. The name is only checked deep inside `Renderer::capture_stream`'s
`reset_for_capture`, after `Encoder::spawn` *and* after a GPU device is built.

**Why it matters beyond tidiness.** ADR-0114's rule for this path is that a missing encoder is a
named error and *never* a silent fallback, because a quietly-substituted encoder makes an exported
file untrustworthy. This is that hazard one step over: nothing was substituted, but the artifact
left at the destination is a real, playable MP4 that a glance cannot tell from a short render. Two
wasted costs ride along — a child process and a GPU device — on an error knowable from the
arguments alone.

**The fix is small**: check `name` against `presets` in `render::run()` before `Encoder::spawn`.
The roster is already in hand there — the `(None, [only])` arm reads it two lines up — and the
error can name the roster's keys, which would also have caught the filename-vs-`name` confusion
that produced this. **No ADR needed. Priority: low-medium**, a good first-phase item on any
follow-up that touches `--render`.

---

## 0112 — the one canonical `ffmpeg` invocation is archival-grade and has no size lever

- **Raised:** 2026-08-17, from [Plan 0101](plans/done/0101-the-engine-renders-a-music-video.md) Phase
  5's real render.
- **Verified 2026-08-17** — the quality setting is a literal in the generated command and no flag
  reaches it: `present: -crf in: standalone/src/shot/render.rs`,
  `absent: --crf in: standalone/examples/shot.rs`.

**Measured.** `attractor_leviathan` at 1920x1080/60, `--tier rich`, a 4:41 track: **3.73 GB,
106 Mbit/s**. On a 30 s slice, the shipped `-crf 18` is 119 Mbit/s, `-crf 23` is 60, `-crf 28` is
27. The engine's grainiest families are pathological for x264 — an attractor is a per-pixel
stochastic spray, so every frame is full-frame noise that changes completely — and entry 0110 is
why that grain is there at 1080p at all. For scale, ~12 Mbit/s is a typical 1080p60 upload
recommendation, so the shipped default is about **9x** it.

**Nothing is missing from the capability** — omit `--ffmpeg`, redirect stdout and run your own
encoder, which `capturing.md` documents. What is missing is a lever on the *convenience* path,
whose whole stated justification is that there is exactly one command line to fix rather than a
wiki of incantations; today adjusting it means editing `ffmpeg_args`.

**Shapes:** a `--crf <n>` passthrough (smallest, and the tests already pin the load-bearing
arguments so the colour tags cannot be lost); or one line in `docs/capturing.md` naming the
raw-stream path as the size-control route, which costs nothing and may be the whole answer.
**No ADR needed. Priority: low** — and it may be **discharged by 0110** rather than on its own,
since most of those bits are encoding shot noise that should not have been in the picture.
