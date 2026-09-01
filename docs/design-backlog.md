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

**Closed 2026-08-17** at [Plan 0108](plans/done/0108-the-milkdrop-import-gets-its-tone-back.md)'s
close. Both were promoted, both landed, and the look gate that closed them **falsified a central
claim in each** — read the archived bodies rather than these rows if you are picking the work up.
The residual defects are live entries 0113-0116, not reopenings of these.

| # | Entry | Went to |
|---|-------|---------|
| 0106 | Converted MilkDrop presets wash out or invert: the float feedback field never truncates | [ADR-0118](adrs/0118-the-milkdrop-feedback-field-quantizes-in-the-encoded-domain.md) + [Plan 0108](plans/done/0108-the-milkdrop-import-gets-its-tone-back.md). **Closed 2026-08-17** |
| 0107 | The MilkDrop draw layer misplaces figures, and two warp-path defects mirror or unfold the frame | [Plan 0108](plans/done/0108-the-milkdrop-import-gets-its-tone-back.md) Phases 3-5. Two fixed, two re-attributed (both attributions here were wrong), one seam still open. **Closed 2026-08-17** |
| 0111 | `shot --render` spawns the encoder and builds a GPU device before it validates `--preset` | [Plan 0139](plans/done/0139-the-render-path-validates-before-it-spends.md) Phase 1. **Closed 2026-09-01** |
| 0112 | The one canonical `ffmpeg` invocation is archival-grade and has no size lever | [Plan 0139](plans/done/0139-the-render-path-validates-before-it-spends.md) Phase 2, as `--crf`; the default stays 18. **Closed 2026-09-01** |
| 0114 | A negative scale is clamped away, so MilkDrop’s standard mirror idiom collapses | [Plan 0109](plans/done/0109-the-milkdrop-import-gets-its-geometry-back.md) Phase 1. Both halves were engine-side. **Closed 2026-08-19** |
| 0115 | There is no video-echo stage, and one preset in seven is unrecognisable without it | [ADR-0119](adrs/0119-the-video-echo-blends-toward-its-copy-rather-than-adding-it.md) + [Plan 0109](plans/done/0109-the-milkdrop-import-gets-its-geometry-back.md) Phases 3 and 7. **Closed 2026-08-19** |
| 0116 | The mode 6/7 waveform rotates a full turn every two minutes, and the reference’s does not | [Plan 0109](plans/done/0109-the-milkdrop-import-gets-its-geometry-back.md) Phase 2. **Closed 2026-08-19** |
| 0121 | A bundle that never names `decay` reads MilkDrop’s per-frame default as a per-second one | [Plan 0111](plans/done/0111-the-milkdrop-import-stops-washing-out.md) Phase 1. Its own “it moves goldens” prediction was wrong. **Closed 2026-08-19** |

**Closed 2026-08-25** at [Plan 0087](plans/done/0087-the-line-renderer-draws-a-curve.md)'s **mid-plan**
review — the plan itself is still open (phases 5-7 unbuilt), but Phase 1b was placed before its stop
gate precisely so this entry could not be orphaned by that outcome, and it discharges independently.

| # | Entry | Went to |
|---|-------|---------|
| 0098 | `thickness` below 0.167 is a dead zone on every line scene, and nothing says so | [Plan 0087](plans/done/0087-the-line-renderer-draws-a-curve.md) Phase 1b. The warning landed, the floor stays. Its own probe went red on delivery: the constant moved. **Closed 2026-08-25** |
| 0071 | The scalloped boundary was chosen as a real curve primitive, and the engine has none | [ADR-0098](adrs/0098-the-line-renderer-draws-arcs-as-per-pixel-distance-fields.md) + [Plan 0087](plans/done/0087-the-line-renderer-draws-a-curve.md) Phase 6, as roster member `scallop`. **Closed 2026-08-27** |
| 0073 | Motif outlines show their vertices, and a sampled polyline does not read as a curve | [ADR-0098](adrs/0098-the-line-renderer-draws-arcs-as-per-pixel-distance-fields.md) + [Plan 0087](plans/done/0087-the-line-renderer-draws-a-curve.md) Phases 3 and 5. The straight-line half is now 0134. **Closed 2026-08-27** |
| 0131 | `shot --report` truncates preset names to 14 characters, and the library now has its first collision | [Plan 0121](plans/done/0121-a-rate-an-ink-edge-and-a-motion-reading.md) Phase 2. Elided in the middle, column unwidened. **Closed 2026-08-27** |
| 0137 | `fragment_field` has three hardcoded animation rates and no parameter behind any of them | [ADR-0132](adrs/0132-a-rate-parameter-integrates-a-phase.md) + [Plan 0121](plans/done/0121-a-rate-an-ink-edge-and-a-motion-reading.md) Phase 3; see 0141. **Closed 2026-08-27** |
| 0138 | `palette_contour` keys on the band grid and never reads the LUT | [ADR-0133](adrs/0133-the-band-contour-fires-where-the-ink-changes.md) + [Plan 0121](plans/done/0121-a-rate-an-ink-edge-and-a-motion-reading.md) Phase 5; see 0140. **Closed 2026-08-27** |
| 0139 | Nothing in the harness measures motion rate or the silent-to-driven difference | [ADR-0134](adrs/0134-motion-is-two-readings-and-anchoring-is-why-neither-can-be-a-threshold.md) + [Plan 0121](plans/done/0121-a-rate-an-ink-edge-and-a-motion-reading.md) Phase 1. **Closed 2026-08-27** |
| 0141 | ADR-0132's rule is engine-wide and its enumeration was not: three rates still multiplied the clock | [ADR-0135](adrs/0135-every-scene-rate-integrates-through-one-shared-phase.md) + [Plan 0122](plans/done/0122-every-rate-integrates.md); see 0149, 0150. **Closed 2026-08-28** |
| 0129 | The doc-link gate walks `.md` only, so the same links in `.rs` comments rot unwatched | [ADR-0127](adrs/0127-a-comment-carries-the-mechanism-and-the-decision-record-stays-in-docs.md) + [Plan 0118](plans/done/0118-the-comments-stop-narrating-the-plans-that-wrote-them.md). **Closed 2026-08-27** |
| 0096 | `shape_field` draws offset contours, and the reference construction everyone reaches for is scaled copies | [ADR-0111](adrs/0111-the-shape-field-gains-a-scaled-copy-coordinate.md) + [Plan 0098](plans/done/0098-the-figure-nests-properly.md) Phases 2-4, as `coord_mode`. **Closed 2026-08-27** |
| 0097 | A curved or jittered `star` returns a NEGATIVE normalized distance at its own centre | [Plan 0098](plans/done/0098-the-figure-nests-properly.md) Phase 1. The reference was repaired, not the result clamped. **Closed 2026-08-27** |

| 0145 | The `animation` gate thresholds silent motion only, so a world alive only on the music fails it | [ADR-0136](adrs/0136-the-animation-gate-asks-its-question-in-both-readings.md) + [Plan 0123](plans/done/0123-a-gate-a-latch-and-an-ink.md) Phases 1-2; see 0152. **Closed 2026-08-28** |
| 0147 | No latch: a gate cannot be armed on time and fired on the music (2nd instance) | [ADR-0137](adrs/0137-a-latch-is-render-layer-state-and-its-name-resolves-to-a-slot-at-load.md) + [Plan 0123](plans/done/0123-a-gate-a-latch-and-an-ink.md) Phases 3-6, as `[latch]`. **Closed 2026-08-28** |
| 0148 | A hard-ink palette cannot reach any additive scene, confining limited ink to 4 of 12 systems | [ADR-0138](adrs/0138-limited-ink-is-a-supported-palette-class-defined-at-the-draw-seam.md) + [Plan 0123](plans/done/0123-a-gate-a-latch-and-an-ink.md) Phases 7-9, as `stroke_blend`; see 0153. **Closed 2026-08-28** |
| 0122 | A mode-6 or -7 wave trace is normalized to the frame's height, so it covers `1/aspect` of its width | [Plan 0127](plans/done/0127-the-picture-stops-depending-on-the-volume-slider.md) Phase 2. **Closed 2026-08-28** |
| 0123 | The waveform is the one un-normalized output, so the volume slider changes the picture | [ADR-0139](adrs/0139-the-waveform-is-levelled-at-the-analyzer-and-publishes-its-gain.md) + [Plan 0127](plans/done/0127-the-picture-stops-depending-on-the-volume-slider.md) Phase 1; see 0120. **Closed 2026-08-28** |
| 0155 | The input-recovery settle window is counted in frames, so its guarantee differs on every display | [Plan 0135](plans/done/0135-the-show-night-surfaces-stop-lying.md) Phase 4, as `INPUT_RECOVERY_SETTLE_SECS`. **Closed 2026-08-30** |
| 0156 | After a give-up, a loss inside the settle window rewrites no surface, so the verdict reads `live` while nothing delivers | [Plan 0135](plans/done/0135-the-show-night-surfaces-stop-lying.md) Phase 3, as `RecoveryPolicy::on_restart`. **Closed 2026-08-30** |
| 0159 | An unrecognized flag is silently ignored and there is no `--help` | [ADR-0148](adrs/0148-the-cli-refuses-an-argument-no-scanner-claimed.md) + [Plan 0135](plans/done/0135-the-show-night-surfaces-stop-lying.md) Phases 1-2; see 0167. **Closed 2026-08-30** |
| 0167 | Six flags do nothing without `--stream`, and the roster built to end silently-ignored flags does not say so | [ADR-0155](adrs/0155-the-window-takes-the-adapter-and-the-preset-the-operator-names.md) + [Plan 0144](plans/done/0144-the-flags-mean-what-they-say.md) Phases 1-3; see 0159. **Closed 2026-08-31** |
| 0168 | The broken-literal defect is a class, and the guard Plan 0124 shipped is a six-item list | [Plan 0144](plans/done/0144-the-flags-mean-what-they-say.md) Phase 4, as a repo-wide scan in `check-comment-hygiene.mjs`; the unrejoined form is still unseen, see 0173. **Closed 2026-08-31** |
| 0169 | `cargo doc` emits intra-doc-link warnings and nothing in the project runs `cargo doc` | [Plan 0144](plans/done/0144-the-flags-mean-what-they-say.md) Phase 6: 71 cleared, then `RUSTDOCFLAGS=-D warnings` added to CI. **Closed 2026-08-31** |
| 0117 | The preset menu dispatches a snapshot index across a modal wait, and "nothing can reload" is not sound | [Plan 0141](plans/done/0141-the-plugin-seams-stop-drifting.md) Phase 1, via `select_preset_named`. **Closed 2026-09-01** |
| 0118 | `foo_lmv.dll` grew ~400 KB and the spec still advertised the old headroom | [Plan 0141](plans/done/0141-the-plugin-seams-stop-drifting.md) Phases 2-3: a dated series, 98.4 % of it Plan 0100; the later window is open, see 0178. **Closed 2026-09-01** |
| 0105 | READ-ME-FIRST states an SDK version that nothing checks on the pre-staged route | [Plan 0141](plans/done/0141-the-plugin-seams-stop-drifting.md) Phase 4: the recipe reads the staged tree's own marker and dies on disagreement. **Closed 2026-09-01** |
| 0130 | `boundary_density` scales with the capture resolution, and neither it nor its floors named the 96x96 | [Plan 0137](plans/done/0137-the-metrics-measure-light.md) Phase 4, as documentation — the statistic is ~`1/L` and stays so. **Closed 2026-09-01** |
| 0132 | The metrics module has no level statistic, and every statistic it has reads gamma-encoded code values | [ADR-0150](adrs/0150-the-level-question-is-asked-in-linear-light.md) + [Plan 0137](plans/done/0137-the-metrics-measure-light.md) Phases 1-3. **Closed 2026-09-01** |
| 0151 | The driven floor's sharpest non-vacuity probe is printed and never asserted | [Plan 0137](plans/done/0137-the-metrics-measure-light.md) Phase 5; both halves now asserted, mutation-checked. **Closed 2026-09-01** |
| 0152 | A disjunctive gate made *the shipped library's minimum* ambiguous in both floors' derivations | [Plan 0137](plans/done/0137-the-metrics-measure-light.md) Phase 6; re-measured over 81 presets, no constant moved. **Closed 2026-09-01** |
| 0135 | `parametric_curve` commits ~6.5 MB at Rich for buffers every shipped preset leaves empty | [Plan 0149](plans/0149-the-line-corners-stop-being-blunt.md) Phase 4, reserved lazily rather than at load — 5,999,992 B not committed, and `nfr.md` 12 corrected. **Closed 2026-09-01** |
| 0144 | The repaired `star` interior is exact only when the spikes are equal, and three places state it unconditionally | [Plan 0149](plans/0149-the-line-corners-stop-being-blunt.md) Phase 5. All four repairs landed; the prose was qualified, the divisor kept. **Closed 2026-09-01** |
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
- **Verified 2026-08-29** at the Plan 0104 close — **the "lever is unused" half of this entry is
  falsified, and the retune it asks for is not.** The reduction that now stands for this entry is
  that its own measured subject is still untouched:
  `absent: ^exposure in: presets/attractor_clifford.toml` — red the day someone retunes the
  preset whose −8.0 % opened this entry, which is exactly when it should be re-read. This entry said twice (2026-08-13, 2026-08-15)
  that exactly **one** shipped preset binds `exposure` (`lsystem_vellum.toml:60`). **Sixteen do**,
  and fifteen of them landed in [Plan 0104](plans/done/0104-the-library-stops-being-lopsided.md):
  its Phase 2 found that a branching or line figure has too little area for a level term to
  register on the stroke, and moved the level response to a whole-frame stage — `exposure` or
  `bg_bright` — on cohort after cohort. So `exposure` is now a routine authoring lever rather
  than an unused one, and any argument here resting on its rarity is void.
- **What survives that correction is the whole of the ask.** None of the fifteen new binders is in
  the population this entry names — *the attractor family, the softer `fragment_*`, `swarm_drift`*
  — which is the set of presets with no over-range peak, and not one of them was touched by
  Plan 0104. The measured −8.0 % on `attractor_clifford` is unaddressed. **The entry stays live.**
- **Why no gate caught this, which is the reusable part.** The claim is carried as
  `unprobeable: ... the grammar deliberately has no count verb (ADR-0108, Notes)`, so
  `scripts/check-backlog-claims.mjs` reported green across every run of the plan that falsified it.
  This is the case the close ceremony prints the `unprobeable:` roster for: the roster is the set of
  claims nothing checks, and a claim in it decays silently until a human reads it against the tree.
- **ROUTED, and now scheduled:** it is §4 of [`content-brief.md`](content-brief.md), paired with Plan
  0071's standing `occlude` retune as one pass over the shipped set. That brief also records the
  other correction this entry's routing carries — the plan text says to run it "with 0038 and 0058",
  but **0058 closed by content on 2026-08-04**, five days before Plan 0071 reached Phase 5, so the
  three-way pass is a two-way pass.
- **Raised:** 2026-07-31, from `architect`, at Plan 0045's Mode 4 review.
- **Verified against code:** yes — measured, not inferred (numbers below).
- **Verified 2026-08-15, and its headline claim is superseded above — sixteen presets bind
  `exposure`, not one.** What that dated check still establishes stands: the original binding is
  present — `present: ^exposure in: presets/lsystem_vellum.toml`. The count around it never
  reduced, and that is why the falsification went unseen for a whole plan:
  `unprobeable: exactly one shipped preset binds exposure is a claim about how many files match,
  and the grammar deliberately has no count verb (ADR-0108, Notes)`. The document this
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

- **The instrument half is now built, 2026-08-25 — the entry stays live and its headline number is
  untouched.** [Plan 0117](plans/done/0117-the-downbeat-log-sees-the-counter-it-folds-over.md)
  appended `fold_beat` and `grid_bar_phase` to `--downbeat-log`, so the sentence closing the bullet
  below — *"an instrument that logs `beat_in_bar` / `bar_index` ... is the cheapest next step and is
  not yet filed"* — is discharged. **Nothing was measured with it.** The two readings that bullet
  says are inseparable are still inseparable, because no capture carries the columns; what changed
  is that spending a capture is now a `human` call rather than an unbuilt prerequisite.
- **Verified 2026-08-25** — the columns exist: `present: fold_beat in: standalone/src/downbeatlog.rs`
- **HALF-DISCHARGED 2026-08-25, and the entry stays live** — [Plan 0095](plans/done/0095-the-downbeat-fold-gets-a-musical-beat.md)
  closed, and it built the repair the 2026-08-15 bullet said was still unbuilt: the fold now buckets
  over a tempo-driven bar grid (`core/src/dsp/grid.rs`) instead of over `beat_index`. **The cause
  that bullet corrected is fixed; this entry's headline number is not.** Measured against a
  reconstruction of the pre-0095 fold on the same captures, the share of hops over the gate moved
  `0.00 → 2.36 %` on rock/pop, `0.79 → 3.67 %` on hip-hop and `4.16 → 0.42 %` on techno — so the
  trio is still counter-derived the overwhelming majority of the time, which is exactly what this
  entry's title says. What changed is *why*: it is no longer a fold indexed by a unit that is not a
  beat. Two things now bound the remaining shortfall, and either could be the successor. **The
  accent feature** is still 70 % bass band (ADR-0082's `Outcome`), and **the gate is now binding
  where it never was** — rock/pop's corrected effect size reached a p90 of 0.2060 against
  `CONFIDENCE_THRESHOLD = 0.25`, i.e. the distribution moved up *under* the gate rather than through
  it, and ADR-0082's reason for that threshold was argued when the estimator had no signal at all.
  That is not licence to lower it; it is the first evidence a decision about it could be made
  against. **Nothing in this repo can separate the two**, because no log column carries the grid —
  `--downbeat-log` predates it. An instrument that logs `beat_in_bar` / `bar_index` beside the
  existing accent decomposition is the cheapest next step and is not yet filed.
- **PROMOTED A THIRD TIME 2026-08-15 → [ADR-0109](adrs/0109-the-beat-clock-counts-onsets-not-beats.md) +
  [Plan 0095](plans/done/0095-the-downbeat-fold-gets-a-musical-beat.md)**, and **this entry's stated cause
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

### Updated 2026-08-26 at [Plan 0113](plans/done/0113-the-engine-paints-a-canvas.md)'s close — half the ask now exists, and this entry stays live for the other half

**Occlusion exists inside one scene.** `shape_collage` (twelfth system,
[ADR-0123](adrs/0123-a-flat-graphic-scene-paints-its-own-paper-and-composites-opaque-elements-in-one-pass.md))
paints flat opaque elements on its own paper in painter order, so a black bar genuinely sits in
front of a red one and a fill with a contrasting outline is drawable today — two elements, the
smaller later in the array. `the_later_element_wins_the_overlap` renders the pair in **both** array
orders and asserts the overlap takes the later element's colour each time, which is the mechanism
rather than an example of it: `present: SystemKind::ShapeCollage in: core/src/preset/schema.rs`

**And it cost no composite change**, which is the half of this entry's own pricing that was wrong.
This entry has sat at **Low** since 2026-08-05 because it was priced as a composite redesign.
ADR-0123 found that price mistaken for the in-scene case: a fullscreen scene emitting `alpha = 1`
already holds the backdrop out (measured, Plan 0091 Phase 1), and the tonemap is the identity below
`KNEE`, so the capability landed as a **scene** with the composite untouched.

**What is still missing is the cross-scene half, and it is the harder one.** A collage element and a
`swarm` particle still have no ordering relationship, because nothing in the engine decides what is
in front of what across scenes — which is this entry's title claim and remains true. ADR-0090's two
layers compose by blend, not by depth, and ADR-0018 / ADR-0031 rejected a render graph twice. So the
entry stays **live**: the ask survives, the in-scene route is now a shipped answer for anyone who
only needs one world at a time, and the remaining work is engine-wide depth rather than a fill model.

### Priority

**Low, and deliberately so.** The user has asked for it once, in a form Plan 0070 partially answered
and Plan 0113 half-answered, and the cost of the remaining half is still a composite redesign. It is
here so the ask survives, not because it is next.

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

**0071 and 0073 are CLOSED** (2026-08-27, Plan 0087) and their bodies are in
[`design-backlog-archive.md`](design-backlog-archive.md); 0072 stays live above.

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

### Update 2026-08-30 — a show-length run exists now, and it neither discharges nor contradicts this

The 2026-08-29 live set ran **8h08m / 3,505,083 frames with zero dropped**, at 120.0 fps flat, on
the show notebook with real audio and a real rotation. That is **17x** run 3's 200,667 frames and
the first data this project has at show length rather than in minutes, so it is worth naming here:
**the tail never became a drop.** The entry's "not currently a defect" reading survives a horizon
two orders of magnitude past the one it was written on.

**It does not close the entry, because the one column that would is missing.** The summary that
survives the night records fps, frames, dropped, `rss_bytes`, `gpu_bytes`, handles and threads —
and **not** `frame_ms_p99`. No soak or `diagnostics.log` from the set is findable on this machine
as of 2026-08-30. So the cheap discriminator "What a fix would be" asks for — a run with the
per-frame series retained — was within reach on the night and was not captured. The practical
lesson is for the *next* show rather than for the governor: a set that runs this long is the
cheapest instrument this project will ever get for the p99 question, and it costs one `--soak`
path on the command line.

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
- **Seen on-device 2026-08-24**, post-0107, at [Plan 0107](plans/done/0107-the-foobar-menu-picks-a-preset.md)
  Phase 5: layout-edit right-click still surfaces the component's menu — now Preset ▸ and four
  items — wholly in place of Cut / Copy / Replace / Remove. The entry's evidence was a code probe
  plus one reporter's account; this adds a second machine and the current menu.

### Updated 2026-08-18 — the shadowing menu is now four items and a submenu

[Plan 0107](plans/done/0107-the-foobar-menu-picks-a-preset.md) rebuilt this same handler without
touching the edit-mode question, so the description above is stale on one detail: the menu is no
longer `Next scene` / `Diagnostics overlay` but a **Preset** submenu listing the whole roster, plus
Next scene, Reload presets, Open presets folder and the overlay toggle. The claim is unchanged and
the probe still holds — the entry is if anything **stronger**, since more of the panel's right-click
is now unreachable in layout-edit mode, and the two plans were deliberately run in sequence on the
understanding that whichever landed second would restructure the other's menu.

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
([Plan 0108](plans/done/0108-the-milkdrop-import-gets-its-tone-back.md)) has settled whether converted
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

--

## 0113 — the converted feedback field equilibrates far brighter than the reference's, and nothing knows why

**Raised by:** `architect`, from [Plan 0108](plans/done/0108-the-milkdrop-import-gets-its-tone-back.md)'s
look gate (2026-08-17). **Owner if taken:** `dev`. **The dominant fidelity defect of the MilkDrop
import**, and the successor to [archived 0106](design-backlog-archive.md), whose diagnosis this
entry replaces rather than repeats.

- **Verified 2026-08-17** — the warp pass still has no mechanism bounding the field's equilibrium
  level, only a per-frame decay and a ceiling clamp:
  `present: decay in: core/src/render/scenes/warp_mesh/mod.rs`
- `unprobeable:` the defect is a rendered divergence against an external reference
  (`foo_vis_milk2` 0.2.0.0, DX11) and lives in no greppable line; the evidence is the seven
  side-by-side pairs recorded in Plan 0108's look-gate section.

### The finding

On five of Plan 0108's seven pairs the background equilibrates far brighter than the reference's and
takes the picture with it. *Fog Tunnel*'s tunnel is a **skeleton of discrete concentric rings** in
the reference and a **solid tube** here — the gaps between rings have filled in. *Contortion*'s
near-black ground is saturated magenta here. *Cosmic Dust 2* is magenta where the reference is green.

**This is not [ADR-0118](adrs/0118-the-milkdrop-feedback-field-quantizes-in-the-encoded-domain.md)'s
truncation defect, and that is the point of filing separately.** The quantizer floors pixels below
one encoded step — linear `3.03e-4`. These backgrounds sit three orders of magnitude above it, and
the same pairs at `quantize_steps` of 255, 0 and -255 are nearly indistinguishable. Archived 0106
claimed one mechanism with four presentations; at most one of the four was truncation.

**Two hypotheses are already dead. Do not re-run them.**

1. **Frame-rate accumulation.** The deposit is `dt`-scaled at
   `core/src/render/scenes/warp_mesh/mod.rs` (`self.deposit * dt`) and the draw layer carries an
   `Exposure(dt * NOMINAL_FPS)`, so a 60 Hz box does not deposit twice a 30 Hz box's light per second.
2. **Non-additive waves being drawn additively.** `bAdditiveWaves` does not separate the washed
   presets from the clean one — measured across all seven, 2026-08-17: *Blur Mix 3*, the good
   control, is `0`; *Contortion*, badly washed, is `1`.

### What a fix would be

Unknown, which is why this is an entry and not an ADR. **Instrument the field, not the composite** —
every observation so far is of the final picture, where `gamma`, `brightness` and the present remaps
all sit downstream and any of them could be the whole story. Read the `Rgba16Float` field's own
equilibrium level frame by frame and compare it against what the reference's 8-bit field must hold
given the same `decay` and deposit.

### Priority

**High.** It is the dominant defect of the import and it survived two plans because nobody could see
it. [Plan 0109](plans/done/0109-the-milkdrop-import-gets-its-geometry-back.md) Phase 4 takes it, with a
stop condition.

### Update 2026-08-19 — half discharged by [Plan 0109](plans/done/0109-the-milkdrop-import-gets-its-geometry-back.md) Phase 4, and its leading hypothesis corrected

**The entry stays live. What Phase 4 delivered is the instrument, not the fix** — which is the branch
that plan wrote for it. The field is now readable: `PingPongField` carries `COPY_SRC` and a test-only
`read_texture`, and `warp_mesh/tests.rs` drives the scene directly, copies the field after every frame
and decodes it with `read_back_linear`, which does not clamp at 1.

**What was ruled out, by measurement rather than by argument.**

- **The field is not where the wash lives on the built-in warp path.** With the quantizer on, the field
  converges by frame 120 and stays there, and the background of a zooming tunnel sits at `1e-6` linear
  and does not move over 300 frames. With it off, both integrate without bound. ADR-0118's mechanism is
  now *observed* rather than inferred from seven captures.
- **The decay multiply's domain is not it** — the third hypothesis, after frame-rate accumulation and
  `bAdditiveWaves`. This engine multiplies in linear light where the reference multiplies its 8-bit
  target in the encoded domain, which predicts our trails outliving the reference's by about `2.4x`.
  Measured, they do not: the quantizer's truncation absorbs most of the domain error. Reproducible —
  `the_decay_domain_is_not_the_wash` derives both predictions from the decay the frames actually ran
  with and asserts the measured fade is nearer the reference's arithmetic than the pure-linear one.

**The hypothesis Phase 4 named as live is wrong about this evidence, and that is the important half of
this update.** Phase 4 proposed that a converted warp shader applies `decay` only when the preset's own
HLSL names it — 6 909 of 8 162 corpus files with a warp shader do not — and that this "predicts the look
gate's own pattern, including why *Blur Mix 3* was the clean control." **The census says the inverse.**
Of Plan 0108's seven pairs:

| preset | warp shader | 0108 verdict |
|---|---|---|
| Contortion, Songflower, chasers 19 Portal, Cosmic Dust 2, Fog Tunnel | **none** (MilkDrop 1.x) | **all five washed** |
| Blur Mix 3 | yes | clean control |
| Cauldron painterly 5 | yes | better |

The five that wash carry no `warp_` or `comp_` block at all; the only two that do are the control and
the one good verdict. The shader-`decay` gap may still be a real corpus-wide defect worth its own
entry, but **it cannot be this wash.** The washed five run the built-in path, which Phase 4 measured as
clean at the field.

**So the remaining direction is downstream of the field** — `gamma`, `brightness`, the four composite
remaps, the post chain and the tonemap — which is the half Phase 4 was told to rule out and instead
only ruled the *field* in as clean. One value worth a look on the way in: *Cosmic Dust 2* sets
`fGammaAdj = 1.9`, the highest of the seven, and this engine applies gamma to linear light where
MilkDrop applied it to 8-bit display-referred pixels. Worked through by hand that points *darker*
rather than brighter, and *Contortion* washes at `fGammaAdj = 1.0`, so it is a question rather than a
lead.

**Two symptoms this entry inherited are retracted.** Both came from Plan 0108's gate and both were
convicted at Plan 0109's:

- **"hue magenta where the reference is green" (*Cosmic Dust 2*) was never evidence.** The preset drives
  `wave_r`/`wave_g`/`wave_b` from three independent LFOs on `time` at incommensurate frequencies with
  ~4-7 s periods, so the hue cycles the whole circle continuously. Green and magenta are the same preset
  a few seconds apart, and two renderers started at different moments are simply out of phase. A hue
  comparison at one instant measures nothing.
- **"a black ray artifact on all four frame edges" (*Contortion*) is probably a presentation of the
  wash, not a second defect.** Motion vectors are off in that preset (`nMotionVectorsX/Y = 0`,
  `mv_a = 0`), so the hatching is not the grid. What it does set is `ob_size = ib_size = 0.01` at full
  alpha in **pure black** — a thin opaque black border drawn every frame and dragged inward by the warp,
  stroke on stroke. Black-on-black is invisible; black-on-yellow-green is glaring. Unverified against
  the reference, hence "probably".

### Update 2026-08-19 — [Plan 0111](plans/done/0111-the-milkdrop-import-stops-washing-out.md) Phase 2 bisected the chain and the wash is **not downstream**. The field was never measured on a real bundle.

**The entry stays live, and its direction reverses.** The update above concluded *"the remaining
direction is downstream of the field"*. Phase 2 measured that chain and it is not there.

One statistic (`edge`, the mean over the outermost ring) at every seam, both subjects, one run,
128x128, 300 frames, quantizer at its default 255:

```text
  seam        fog tunnel    blur mix 3    ratio     (hardware)
  A field     0.29798886    0.01990991    14.967
  B present   0.52298039    0.08744538     5.981
  E display   0.74454564    0.25118530     2.964

  seam        fog tunnel    blur mix 3    ratio     (DX12 WARP)
  A field     0.29793853    0.01192657    24.981
  B present   0.52290142    0.05914328     8.841
  E display   0.74456638    0.24515122     3.037
```

**The plan's five seams are three**, measured rather than assumed: every post stage reports `active`
only above zero and neither converted preset binds `bloom`, `trails` or any kaleidoscope param, so
`PostChain::begin` hands the scene the tonemap's own input texture; and `bg_bright` defaults to `0`
and neither binds it, so the backdrop contributes nothing. Seams B, C and D are one texture.

**No seam departs upward, and the separation is already maximal at the field** — the ratio is
`14.967` at A and lower at every seam after it, on both adapters. The **present pass** demonstrably
compresses it (`14.967 -> 5.981`, linear against linear). So no downstream stage creates the wash,
and Plan 0111 Phase 3 did not run.

**Corrected at Plan 0111's close, 2026-08-20, and the correction matters to whoever reads the E
column.** The first draft of this update read `15 -> 6 -> 3` as one monotone sequence and credited
the tonemap with the second fall. **Seam E is not in the same domain as A and B.** A and B are
linear-light means; E is a mean of sRGB-encoded code values (`HEADLESS_FORMAT = Rgba8UnormSrgb`,
bytes over 255), and a ratio of encoded values is not the same kind of quantity as a ratio of linear
ones ([ADR-0074](adrs/0074-a-ratio-against-an-in-run-control-is-not-automatically-portable.md)).
Encoding seam B's own two means gives `0.750` and `0.327` — ratio `2.294`, **below** the observed
`2.964` — so the transfer function alone over-explains the whole B-to-E fall with no stage involved.
And the tonemap is *exactly* the identity below `KNEE = 0.6`, which both backgrounds sit under; the
empirical bound agrees, leaving under 1 % of *Fog Tunnel*'s reading for tonemap and Jensen together.
**The tonemap is a no-op for these two subjects.** It is therefore neither ruled in nor ruled out by
this bisect for a subject whose background clears the knee — for these two, the knee rules it out,
not the measurement.

**Why the field looked clean and is not.** Plan 0109 Phase 4's probe drives
`MilkBundle::from_assembly(None, None, None)` — an **empty** bundle with synthetic params — so its
`1e-6` background is a statement about a stand-in, not about any preset. Driven with *Fog Tunnel*'s own
bundle the field background reads `0.298`: five orders of magnitude higher, and almost exactly the
*"three orders of magnitude above the quantizer's `3.03e-4` floor"* this entry predicted from the look
gate before anyone could measure it. **The field is where the separation already exists**, and the
built-in warp path is back in scope.

**Two caveats, neither of which moves the conclusion.** `edge` reads background only for a figure that
does not fill the frame, and our *Fog Tunnel* draws the solid tube that is the defect — so the absolute
`14.967` is soft while the monotonic trend, being one statistic at every seam, is not. And *Blur Mix 3*
alone diverges `1.67x` between adapters at the field (it is the one subject with a blur chain), so any
threshold here would be adapter-dependent; the probe asserts none.

- **Verified 2026-08-19** — the field is still read back by the probe this update is built on:
  `present: fn feedback_field in: core/src/render/scenes/warp_mesh/mod.rs`

---

## 0119 — `ang`'s branch cut on the +x axis seams every per-vertex program that is continuous in it

**Raised by:** `architect`, from [Plan 0109](plans/done/0109-the-milkdrop-import-gets-its-geometry-back.md)'s
Phase 5 look gate (2026-08-19). **Owner if taken:** `dev`.

- **Verified 2026-08-19** — the wrap is unconditional and has no continuity treatment:
  `present: ang \+= std::f32::consts::TAU in: core/src/render/scenes/warp_mesh/mod.rs`

### The finding

`vertex_position` computes `ang = atan2(py, px)` and lifts the negative half by `TAU`, so `ang` is
`0..tau` **with a discontinuity along the +x axis**. Any per-vertex program whose output varies
continuously with `ang` therefore jumps across that ray, and the jump is a visible seam running from
the frame centre to the right edge.

**This supersedes Plan 0108's reading of the same symptom.** That plan attributed the seam to a sign
in the emitted warp epilogue's polar pair (`milkconv/src/shader/emit.rs`) and built a reproduction
fixture for it, and [Plan 0109](plans/done/0109-the-milkdrop-import-gets-its-geometry-back.md)
Phase 5 asked whether the seam survived the mirror fixes. It did — **on *Songflower (Moss Posy)* and
*chasers 19 Portal*, both MilkDrop 1.x presets carrying no `warp_` or `comp_` block at all.** `emit.rs`
never runs on them. Whatever is true of the emitted epilogue, it is not what produces this seam.

The two look different only because of what surrounds them: on *Songflower* the cut runs centre to
right edge, which is the branch cut plainly; on *chasers 19 Portal* it reads full-width because that
preset's own fold mirrors it.

### What a fix would be

Unknown, and it is a design call rather than a patch — which is why this is an entry. The reference
has the same branch cut in `atan2`, so MilkDrop presets are *authored* against a discontinuity at +x
and simply avoid it or hide it; the question is whether this engine's `ang` lands the cut in the same
place and with the same handedness as the reference's, and it is measurable against a converted
fixture rather than arguable. Do not "fix" it by smoothing the wrap — that would break every preset
that uses the cut deliberately.

### Priority

**Medium.** It is one of the two remaining named geometry defects of the import (the other being
[0113](#0113--the-converted-feedback-field-equilibrates-far-brighter-than-the-references-and-nothing-knows-why)'s
wash), it shows on real content rather than only on a fixture, and the diagnosis is now specific
enough to act on.

### Update 2026-08-20 — [Plan 0111](plans/done/0111-the-milkdrop-import-stops-washing-out.md) Phase 4 pinned **our** half and could not reach the reference's. The entry stays live, **half discharged**.

**What landed.** `ang_cuts_on_plus_x_and_turns_counter_clockwise_on_screen` pins this engine's
construction from `vertex_position`'s arithmetic rather than from a picture: the cut is on **+x** and
is a genuine discontinuity of nearly a full turn (asserted against the two neighbouring vertices),
`ang` is continuous elsewhere along the swept column, and it increases **counter-clockwise as seen on
screen** because y is flipped before the `atan2`. `milkconv/tests/warp_geometry.rs` already holds the
emitted WGSL epilogue and the draw layer to the same y-down/y-up asymmetry, so the three agree by
test. Neither fact can now move silently.

**What did not, and this entry's own "What a fix would be" was wrong about it.** That section says
the question "is measurable against a converted fixture rather than arguable". **It is not.** A
converted fixture measures *this engine*, which is the half already pinned; the missing half is the
reference's handedness, and no `.milk` file records the convention it was authored against. The phase
required the comparison be derived from the source format's convention or the reference
implementation with the source named, and never from a picture — none of those exist in this
environment. A corpus-wide search for a preset stating a rotation direction returns two files, both
building their own Kardan rotation from `q` variables rather than reading the per-vertex `ang`.

So the seam is **not** corrected and is **not** recorded as authored-against either; that second
claim needs the missing half and asserting it would be exactly the prose error
[ADR-0071](adrs/0071-a-numeric-test-contract-states-a-property-or-names-its-machine.md) catches.

**What would settle it, most direct first:** MilkDrop 2's `milkdropfs.cpp` mesh setup, where the sign
of the `y` handed to `atan2f` is one line; the authoring documentation that shipped with MilkDrop; or
one reference capture of a deliberately handedness-revealing preset, which is a look-gate artifact
and not a test. **This is the same procurement wall 0120 and 0122
stopped on**, and one source clears all three — which is the argument for treating it as one
procurement question rather than three engineering ones.

- **Verified 2026-08-20** — the wrap is still unconditional and still has no continuity treatment:
  `present: ang \+= std::f32::consts::TAU in: core/src/render/scenes/warp_mesh/mod.rs`

---

## 0120 — the converted waveform figure renders larger than the reference's, and `wave_scale` is applied raw

**Raised by:** `architect`, from [Plan 0109](plans/done/0109-the-milkdrop-import-gets-its-geometry-back.md)'s
Phase 5 look gate (2026-08-19). **Owner if taken:** `dev`.

- **Verified 2026-08-19** — the authored scale is a bare multiply on the trace samples, with no
  normalization constant and no comment on units:
  `present: held \* scale in: core/src/render/scenes/warp_mesh/draw.rs`

### The finding

Two of the seven judged pairs reported an oversized waveform figure, independently and unprompted:
*Blur Mix 3* ("a bit upscaled", `nWaveMode = 6`, `fWaveScale = 3.266`) and *Cauldron painterly 5*
("wave became very large", `nWaveMode = 5`, `fWaveScale = 1.139`). `draw.rs` applies the value as
`*slot = held * scale`, straight from `fWaveScale`, on a trace already normalized to `-1..1`.
MilkDrop's `fWaveScale` is not a bare multiplier on such a trace — the reference normalizes by the
sample range — so a missing constant is a plausible single home.

**It is a candidate and not a diagnosis, deliberately.** The complaint does not scale with the
authored value: the preset with the *larger* `fWaveScale` read as *less* wrong, at a different
`nWaveMode`. So there may be two defects here rather than one, and a third observation is recorded
without a mechanism at all: *Blur Mix 3*'s **crisp trace spans roughly the middle 57 % of the frame**
while its blurred halo reaches the edges, where Plan 0108 described the reference as drawing
"horizontal full-width traces". Amplitude and extent are different quantities and may have different
causes.

### What a fix would be

Derive the reference's constant from MilkDrop's own waveform construction rather than by matching a
picture, then decide whether the x-extent is the same defect or a second one. Both are checkable
against `milkconv/tests/draw_layer.rs`, which already builds the figure as geometry and can assert a
span without a capture.

### Priority

**Medium.** It touches the whole waveform-led family — the *Blur Mix* / *Fog Tunnel* / *Cauldron*
presets — but unlike the wash it makes a preset look mis-tuned rather than unrecognisable.

### Update 2026-08-19 — [Plan 0111](plans/done/0111-the-milkdrop-import-stops-washing-out.md) Phase 5 **split this entry in two**. The x-extent is a separate defect and is now [0122](design-backlog-archive.md#0122--a-mode-6-or-7-wave-trace-is-normalized-to-the-frames-height-so-it-covers-1aspect-of-its-width); the amplitude constant **stays live and undecided**.

**The amplitude half stopped, on the branch the phase was given for it.** The reference's
normalization is not derivable from any source available in this environment: there is no MilkDrop
source and no authoring documentation beside the corpus, and a `.milk` preset does not record the
convention it was authored against. Nothing was changed, and matching a picture was refused — it
would produce a number right for one preset at one wave mode and wrong for the corpus.

**What the corpus does settle, and it narrows the question usefully.** Across the 552 presets in
`milkdrop-original` that set `fWaveScale`:

```text
  n=552  min=0.0000  p10=0.0100  p25=0.2920  median=0.9724  p75=1.5540  p90=3.2350  max=100.0000
```

**The median is 0.9724 — unity.** So `fWaveScale` is authored as a multiplier *about 1*, and what is
missing is a single **base amplitude** (what a unit-scale wave should occupy), not a per-preset or
mode-dependent correction. That also weakens this entry's own "factor of 279 across the seven pinned
presets, so it cannot be a bare multiplier" argument: `p10 = 0.01` means a tenth of the corpus
authors near-zero scales deliberately, and a near-flat trace is a *visible flat line* rather than an
invisible one, so the spread is consistent with a bare multiplier over a correct base.

Any candidate constant must keep both ends of that distribution usable. Whoever takes this needs one
of: MilkDrop 2's waveform draw, its authoring documentation, or a reference capture of a preset built
to be amplitude-revealing.


### Update 2026-08-28 — [Plan 0127](plans/done/0127-the-picture-stops-depending-on-the-volume-slider.md) took the measurement this entry stopped for, and it **falsifies the entry's title**. Phase 4 was written to apply a base amplitude constant and was **skipped** on the reading; nothing in `draw.rs` changed, so this stays live.

**The reference capture happened.** Phase 3 built two purpose-authored `.milk` presets
(`nWaveMode = 6`, `fWaveScale = 1.0`, `fWaveSmoothing = 0`, `fWaveParam = 0`, warp/zoom/rot/echo
neutral, a thin opaque white line on black) and drove both `foo_vis_milk2` 0.2.0.0 and this engine
from one 60 s 48 kHz full-scale 200 Hz sine, verified 0.0 dBFS peak. Two readings, each with the
screenshot it came from, in `WORK/lmv-0127-gate/` outside the repo:

| | reference (`foo_vis_milk2`) | ours |
|---|---|---|
| peak-to-peak at `fWaveScale = 1`, full-scale input | **0.316** frame heights | **0.3019** frame heights |
| x-extent at 16:9 | **1.000** of frame width | 1.000 (after Plan 0127 Phase 2) |

**So we render 4.7 % SMALLER than the reference at unit scale, not larger.** The ratio is a single
number — ours is linear to within 0.9 % across a 4x sweep of `wave_scale` (0.3037 / 0.3019 / 0.3009
at 0.5 / 1.0 / 2.0) — so `draw.rs`'s mode-6/7 factor of `0.15` implies MilkDrop's is ~`0.157`.
**The oversized figure two Plan 0109 judges reported was [0123](design-backlog-archive.md#0123--the-waveform-is-the-one-un-normalized-analysis-output-so-the-os-volume-slider-changes-the-picture--and-the-two-frontends-disagree)**,
the volume dependence, which is now closed — not a missing base amplitude. This entry's own reading
of that interaction ("an un-normalized trace times an un-normalized `wave_scale` is hypersensitive")
was right; the half it attributed to `wave_scale` was not there.

**Why the constant was not applied anyway.** Phase 4's second done-when required both ends of the
corpus distribution to stay usable. Measured at 1920x1080 on the geometry the draw layer builds:
`p10 = 0.01` draws 5 px (0.0046 H), a visible flat line; `p90 = 3.235` draws 0.9722 H — inside the
frame with 15 px of margin; `p90` scaled by 1.047 draws exactly 1.0000 H, rows 0..1079, **clipped at
the frame edge**. Applying the constant fails the criterion the phase existed to satisfy, for 4.7 %.

**What is left of this entry, and it is much less than its title claims.** The base amplitude is
measured and it is very nearly right; whether the last 4.7 % is worth a change that re-blesses the
converted goldens and pushes the top decile of the corpus to the frame edge is a judgement nobody
has needed to make. **Priority drops to low.** Whoever picks it up starts from `1.047` and does not
re-derive it.

- **Verified 2026-08-19** — the scale is still a bare multiply with no normalization constant:
  `present: \*slot = held \* scale in: core/src/render/scenes/warp_mesh/draw.rs`
- **Verified 2026-08-28** — still true after Plan 0127, which measured the constant and did not
  apply it: `present: \*slot = held \* scale in: core/src/render/scenes/warp_mesh/draw.rs`

---

## 0124 — ADR-0113's motivating claim has read "provisionally negative" since 2026-08-16, and two look gates have run without re-taking it

**Raised by:** a loose-ends sweep (2026-08-24), reading an orphaned draft plan — *0108 — The field
learns to forget*, written 2026-08-16 outside the repo, never committed and since superseded by
Plans 0108, 0109 and 0111. Five of its six phases have landed elsewhere; this is the one that has
not. **Owner if taken:** `architect` — an ADR Outcome is architect work.

- **Verified 2026-08-24** — the ADR carries no Outcome beyond the two it was closed with:
  `absent: Outcome \(2026-08-(?!16) in: docs/adrs/0113-milkdrop-presets-are-translated-ahead-of-time-onto-a-warp-mesh-idiom.md`

### The finding

[ADR-0113](adrs/0113-milkdrop-presets-are-translated-ahead-of-time-onto-a-warp-mesh-idiom.md)'s
Context argues **"the same preset should look better here"** — linear-light HDR against the
reference's 8-bit additive. Its second Outcome, dated 2026-08-16 at Plan 0100's close, records the
user's verdict as **merely different, not better**, attributes it to one defect (backlog 0106, the
field that never truncates), and then commits in as many words: *"The HDR question is re-judged
after 0106 lands."*

**0106 landed 2026-08-17.** Two look gates have run since — Plan 0108's, and
[Plan 0109](plans/done/0109-the-milkdrop-import-gets-its-geometry-back.md) Phase 5 on 2026-08-19,
same seven pairs, same rig, `foo_vis_milk2` 0.2.0.0 — and 0109's gate reads three of the seven as
**fixed**, including the portal and *Blur Mix 3*'s traces. Neither gate produced a third Outcome.
So the ADR still tells a reader its founding claim is provisionally negative, on evidence two
plans and four ADRs old.

### Why this is an entry rather than nothing

The per-pair verdicts are recorded — in the plans that ran them. But a plan doc is where a phase's
result lives, and an ADR is where a **decision's** motivating claim lives; this ADR is the one that
asked to be revisited, and nothing carries that ask. The 2026-08-16 wording is also the load-bearing
half: "merely different, not better" is the sentence that would make someone question the whole
ahead-of-time translation approach, and it is now the least current thing in the file.

### What a fix would have to decide

Whether the verdict can be re-taken **from the record** — 0109 Phase 5's table is on file and
per-pair — or needs a fresh gate. Three pairs still read *washed* there, and backlog 0113 (the wash)
is **live**, having survived Plan 0111's bisect and reversed back to the field. So a re-take today
may honestly still read *merely different, with the wash dominating*. **That is a perfectly good
third Outcome** — dated, naming 0113 as the remaining blocker, and saying the claim is not yet
answerable rather than leaving 2026-08-16's silence to stand for it.

### Priority

**Low.** Nothing renders wrong because of this. The cost is a reader — including a future
`architect` session — trusting a stale verdict about whether this project's whole MilkDrop
translation strategy is worth it.

---

## 0125 — every diffused frame is an upscale: both profiles diffuse well below the stream's own resolution

**Raised by:** the user, at Plan 0106's Phase 6 human gate (2026-08-25), on a full-track render of
`star_rosewindow` — *"it would obviously be great if resolution would be higher"*. **Owner if
taken:** `architect` — it reopens a clause ADR-0121 recorded as deliberately rejected, so it is an
ADR question before it is a code one.

- **Verified 2026-08-25** — the shipping `quality` profile diffuses at a 589,824 px budget, which is
  28 % of a 1920x1080 frame, so every output pixel is resampled up:
  `present: "size": "589824" in: tools/sd-filter/sd_filter.py`

### The finding

The clip that drew the verdict was rendered at **`fast`** — a 262,144 px budget, **680x384** at
16:9 — and resampled to 1920x1080. `quality` is 1024x576, **2.25x the pixels**, and *has never been
rendered on a real track*. So an unknown and possibly large share of this complaint is a profile
choice rather than a wall, and **the cheap first move is a side-by-side still at both budgets**, not
a design.

What is genuinely walled, and why this is not simply "raise the budget":

- **SD1.5 duplicates or mirrors content above roughly 768²** — its native-resolution artifact, named
  in Plan 0106 Phase 1's traps. Raising the budget does not scale smoothly into it.
- **SDXL plus ControlNet is ~7.5 GB against an 8 GB card**, and the spike already peaks at 5.68 GB
  with two ControlNets loaded. Offloading fixes the memory and ruins the throughput over thousands
  of frames, which Phase 1 also measured.
- **Cost scales with pixels.** Phase 2b measured 2.721 s/frame at 589,824 px against roughly a third
  of that at 262,144. A 4-minute track at `quality` already measures ~5.9 h *before* the 1.406x
  scope correction Plan 0106 Phase 7d applies to that figure.

**The tension worth surfacing before anyone designs.**
[ADR-0121](adrs/0121-the-diffusion-filter-is-an-offline-stage-with-profiles-and-it-interpolates-its-own-stride.md)'s
Alternative C is *diffuse at a smaller budget and upscale*, measured as the cheaper route and
**rejected by this same user in the design interview**, on the ground that generated detail is worth
its price against inferred detail. This verdict does not obviously overturn that — the ask is for
*more* detail, and an upscaler infers rather than generates — but it does mean the rejection was
made before anyone had watched five minutes of output. A tiled or multi-pass approach that
*generates* at higher resolution is the option neither the ADR nor the plan has costed.

## 0126 — a render is one prompt, one seed and one preset from first frame to last, so nothing varies across a track

**Raised by:** the user, at Plan 0106's Phase 6 human gate (2026-08-25), on a 5:15 render —
*"...and with more variety"*. **Owner if taken:** `architect` — it is squarely inside Plan 0106's
stated non-scope, so taking it is a scope decision, not an implementation one.

- **Verified 2026-08-25** — one seed is fixed for the entire render, by construction and on purpose:
  `present: manual_seed\(cfg in: tools/sd-filter/sd_filter.py`

### The finding

This is **not a defect** — it is Plan 0106's *What this plan does NOT do*, in as many words:
*"No timeline, cuts, or prompt automation across a track. One prompt per render, matching 0101's one
preset per render."* The fixed seed is load-bearing for the thing the gate approved: Phase 1 records
that a per-frame seed *"guarantees boiling whatever else is tuned"*. So variety cannot be bought by
simply unfixing it, and that is the first thing a designer would reach for.

The plan already carries the shape of the answer in its own Followups — **audio-conditioned
diffusion**, *"denoise from the onset envelope, prompt blend on bar boundaries"* — which was filed as
a nice-to-have and is now a user ask with a watched render behind it. Levers worth costing, roughly
in increasing order of what they disturb:

- **Prompt interpolation on bar or section boundaries**, which the analyzer can already supply. The
  smallest change that produces real variation, and it keeps one seed and one preset.
- **Preset changes across a track** — 0101 renders one preset per render, so this is a `shot`
  question before it is a filter question.
- **Denoise strength driven by the onset envelope**, which is the one lever that **reopens Plan
  0106's no-audio-conditioning decision**: it carries real audio data across a seam the plan
  deliberately kept image-only. Phase 2 named exactly this as the repair if the music stopped
  reading — it did not, so this would be taken for variety rather than for reactivity, which is a
  different justification and wants its own ADR.

**Do not fold this into a resolution plan.** It shares a verdict with backlog 0125 and nothing else:
one is a pixel budget against a VRAM wall, the other is a timeline the pipeline does not have.

## 0127 — the figure gate walks the working tree, so a gitignored local note can redden a push

**Raised by:** `architect`, at Plan 0106's close (2026-08-25), when the gate the same plan shipped
convicted a **gitignored** provenance file under `renders/` that had never been and could never be
committed. **Owner if taken:** `dev` — it is a scan-set question in one script, not a design one.

- **Verified 2026-08-25** — the walk is filesystem-based and consults no ignore rules:
  `present: readdirSync in: scripts/check-filter-figures.mjs`

### The finding

`scripts/check-filter-figures.mjs` enumerates candidate markdown by walking directories, skipping a
hardcoded set (`node_modules`, `target`, `.git`, and the seeded-fixture tree). It never asks git what
is tracked, so **any local file naming `sd-filter` and carrying a figure fails the pre-push hook** —
scratch notes, a pasted measurement, a downloaded README. The author's fix is to edit or delete a
file the repository will never see.

**CI is unaffected and that is why this is small.** The `links` job checks out the tracked tree, so
gitignored files do not exist there; only the local hook can fire on this. That also means the gate's
*enforcement* is sound — nothing wrong can reach `main` through this hole — and what is wrong is the
**local ergonomics**, which is a weaker complaint than it first looks.

**The counter-argument is real and should be costed before this is taken.** A gitignored copy of a
cost figure still misleads whoever reads it, and this repository's whole reason for the gate is that
*the copy that broke it was the one outside the list anyone was checking*
([ADR-0122](adrs/0122-a-sidecar-tool-documents-itself-in-one-place.md)). Restricting the scan to
`git ls-files` buys ergonomics and gives up exactly that reach. A middle option — scan the working
tree, but report an untracked hit as a **warning that does not set the exit code**, in the shape
`check-backlog-claims.mjs`'s advisory block already uses — keeps both and is probably the answer.

**Not urgent.** One close hit it, once, and editing the offending file took a minute.

## 0128 — `tonal_flatness` convicts a flat-graphic composition, and no reference tone and no structural statistic repairs it

**Raised by:** `preset-author`, routed 2026-08-25, when a deliberately flat black-and-white preset
(`presets/fragment_tiledmono.toml`, `fragment_field` + `palette_steps = 20`) was rejected by
`every_preset_draws_a_real_shape` at `tonal_flatness = 0.9494` against a `0.90` ceiling.
**Owner if taken:** `architect` first — this is a question about what the sanity lens *means*, not a
threshold to retune.

- **Verified 2026-08-25** — the ceiling that convicts: `present: MAX_TONAL_FLATNESS: f32 = 0\.90 in: core/tests/sanity.rs`
- **Verified 2026-08-25** — every statistic keys off departure from the black reference:
  `present: fn is_lit in: core/src/render/metrics.rs`
- **Verified 2026-08-26** — the derived ground that half-discharged this entry:
  `present: pub fn modal_ground in: core/src/render/metrics.rs`
- **Verified 2026-08-26** — **the titular claim is discharged.** A structural statistic does repair
  it, and the preset ships: `present: pub fn boundary_density in: core/src/render/metrics.rs`
- **Verified 2026-08-26** — the conjunction it asked for is live and per system kind:
  `present: fn boundary_floor in: core/tests/sanity.rs`
- **Verified 2026-08-26** — the surviving half, which is the absence of an instrument rather than a
  fact about the tree: `unprobeable: whether Sumi, Whorl, Supernova and Neon Tunnel are compositions
  or fills is a question no statistic in this repo asks, so there is nothing to match on`
- **Verified 2026-08-26** — **the motivating family landed.** `shape_collage` merged with Plan 0113,
  so "all eleven current systems" in The finding below is now twelve, and the light-ground case that
  section calls *"about to stop holding"* has stopped holding. Its `coverage_floor` is derived from
  the family's own distribution (0.13, half `On White`'s 0.2677) rather than resting on the
  `1.0000` this entry named degenerate, and the emptying canvas is convicted on the real family
  instead of the synthetic stand-in: `present: SystemKind::ShapeCollage in: core/src/preset/schema.rs`

### Half-discharged again 2026-08-26 by [Plan 0119](plans/done/0119-the-flatness-gate-gets-its-second-term.md) — the title is discharged, the residue is not

**Read this before the section below it, which this one narrows.** The heading of this entry —
*"no reference tone and no structural statistic repairs it"* — is now **false**, and it was false in
its second half from the day it was written.
[ADR-0130](adrs/0130-the-structural-term-is-boundary-density-and-conditioning-the-population-is-what-made-it-work.md)
ships `metrics::boundary_density` as the flatness gate's second term, `fragment_tiledmono` is in the
embedded set, and nothing in the preset moved to get there.

**Discharged — the flat-graphic conviction.** `every_preset_draws_a_real_shape` convicts only a frame
that is over `MAX_TONAL_FLATNESS` **and** under `boundary_floor(system)`. What made a "failed"
candidate work was ADR-0129's conditioning correction, not a new statistic: a conjunction's second
term is judged only over the frames the first term admits to it, and conditioned that way the
population has two members. `boundary` was in the section below marked **no**; its reading never
moved.

**Not discharged — the full-coverage residue.** `Sumi`, `Whorl`, `Supernova` and `Neon Tunnel` still
read honest `coverage` near 1.0 with nothing asking whether they are compositions or fills. That is
this entry's last live half. The `#[ignore]`d `tile@N` columns in `core/tests/sanity.rs` are the
instrument ADR-0129 argued that question needs, and losing the flatness contest does not disturb that
argument. **Owner if taken:** `architect`.

**And a live hazard the discharge created**, recorded here because it lands on the same content:
22 of the 43 shipped presets read under their family's boundary floor and pass on term one alone, so
converting one to a two-ink print flips it from passing to convicted. See ADR-0130's landmine
Negative and [Plan 0119](plans/done/0119-the-flatness-gate-gets-its-second-term.md)'s mono-cohort
table for the per-preset numbers.

### Half-discharged 2026-08-26 by [Plan 0116](plans/done/0116-the-sanity-lens-finds-the-ground.md), and one causal claim below is falsified

**Read this before the body.** The entry stays live because only one of its two failure modes was
repaired, and the diagnosis it gives for the other one is wrong.

**Discharged — "a light ground is worse".** `sanity` no longer measures against a constant. `is_lit`
takes a reference derived per capture ([ADR-0126](adrs/0126-the-sanity-lens-measures-departure-from-the-frames-own-ground.md),
`metrics::modal_ground`: the mean RGB of the frame's most populous luminance band), so the three
statistics this entry called degenerate carry information again for the eight presets that have a
ground — `Tiled Rosette` `1.0000` → `0.1645`, `Ink on Paper` → `0.2167`, `Vellum` → `0.3704`. The
re-basing reached 17 of 41 presets and moved **no verdict**, at either excitation. The emptying
canvas this entry called time-critical for Plan 0113 is now convicted at the quiet excitation, on a
synthetic fixture that also pins the old lens reading the same frame as `coverage 1.0000`.

**Not discharged, and the entry's own explanation of it is falsified — "dark ink on black".** This
body attributes `fragment_tiledmono`'s conviction to its black ink "not being counted", so that the
look is "measured on its white alone". With the ground correctly at the paper the ink **is** counted
and the paper is excluded, and the preset reads `0.9413` — marginally *worse* than the `0.9346` it
read against black. All three candidate estimators found the paper at `(245,245,245)` and all three
still convicted it. The mechanism is symmetric: a duotone has two large populations and `is_lit`
removes whichever one is the ground, so the other holds ~94 % of what remains either way. **This is a
property of `tonal_flatness`, not of the reference it measures from**, which is what
[ADR-0128](adrs/0128-a-tonally-flat-picture-is-a-blot-only-if-it-is-also-structureless.md) was raised
to take.

**And ADR-0128's mechanism did not survive measurement either.** Plan 0116 Phase 8 tabled boundary
length, connected components and Sobel density over the lit mask against the frozen `Blown Out` blot
and the held preset. None separates them with the library outside the gap, and every one produces —
under this repo's own threshold ceremony — a constant that stops convicting the blot by an order of
magnitude. The reason is a collision with the repair above: under a derived ground a saturated blot is
its own modal band, so its lit mask is the mass's **fringe**, and a fringe is a thin ring that every
structural statistic scores as structured. `fragment_tiledmono` stays in `presets/pending/` with
**nothing scheduled**, and the four groundless luminous fields (`Sumi`, `Whorl`, `Supernova`,
`Neon Tunnel`) stay composition-or-fill unanswered.

**So what is still open** is narrower and harder than what this entry was raised as: a gate that can
tell a deliberately flat graphic composition from a saturated blot, when the two are not separable by
tone *or* by any structural statistic measured so far over the lit mask. Start from the fringe
mechanism, not from ADR-0128's roster. **Owner if taken:** `architect`.

### The finding

`is_lit(px, bg, eps)` is true when any of the first three channels differs from `bg` by more than
`EPS = 10`, and `sanity` passes `BLACK` as `bg`. **All four of the lens's statistics are built on
it** — `coverage` is lit/total, `quadrant_spread` counts lit pixels per quadrant,
`radial_shell_occupancy` counts shells containing lit pixels, and `tonal_flatness` buckets lit pixels
by luminance. The lens therefore encodes an unstated precondition: **the scene draws light onto a
black ground, and black means nothing was drawn.** That holds for all eleven current systems, which
are additive and luminous. It is about to stop holding.

Two distinct failure modes, and they are not the same bug:

**Dark ink on black.** When a scene draws black as a *colour*, that ink is not counted. A
black-and-white look is then measured on its white alone and reads near `1.0` by construction,
however much structure the frame holds. Measured at the gate's exact capture (96x96, `FRAMES = 30`,
all bands at `LOUD = 1.0`): white 94.94 % of lit, red 0.24 %, flatness `0.9494`. The same preset at
frame 120 reads `0.8303`, because the gate samples during the preset's own `[smoothing]` warm-up —
worth knowing separately, since it means the statistic is read before the picture has settled. At
1280x720 the frame is 46.7 % white, 45.9 % black, 7.0 % red, and pixels belonging to none of the
three inks total 0.2-0.3 %, i.e. edge antialiasing. There is no gradient and no blown-out region;
this is the *opposite* of the additive-ceiling blot the docstring describes, caught by the same net.

**A light ground is worse, and this is the part that reaches Plan 0113.** If the scene paints its own
pale paper, every pixel is lit. `coverage` goes to ~1.0, `quadrant_spread` to 4, and
`radial_shell_occupancy` to every shell — **three of the four statistics stop carrying information at
all**, passing any floor trivially. The fourth becomes the paper's share of the frame, so
`tonal_flatness` now convicts a composition *for being sparse* — which for the suprematist target is
the goal rather than the defect.

### Why it is time-critical

[Plan 0113](plans/done/0113-the-engine-paints-a-canvas.md) is **approved** and adds `shape_collage`,
described in its own TL;DR as "the engine's first **graphic** world rather than a luminous one: no
glow, no bloom, hard edges, solid colour", on "its own off-white paper" (`f(1.0) = 0.800`). It
reaches flat colour through the same tonemap property this preset uses — identity below `KNEE = 0.6`.
The plan does not mention `tonal_flatness`, `sanity`, or any metric question anywhere; its Phase 1
done-when renders a preset, and Phase 8 ships a set. Both meet this lens.

**Scope this honestly:** the routed note claimed every flat-graphic preset trips the gate, and that
is overstated. A canvas at 85 % paper reads `0.85` and passes; one at 92 % fails. The reliable claims
are the weaker and the stronger one — that the *pass* is uninformative because three statistics have
gone degenerate, and that the *failure*, when it comes, arrives on the sparsest and most correct
compositions.

### What is not the answer

`KNOWN_FLAT` is documented as a defect list that must stay empty ("if one ever goes over, that is a
defect to route, not an entry to re-add"), so an exemption is not the escape hatch — this entry is
that routing.

Lifting the ink from `#000000` to `#010101` puts the black at luma ~22 after the preset's glow, over
`EPS`, so it counts as lit and flatness falls to ~0.5. It passes today and is visually
indistinguishable. **Rejected:** it defeats the gate by tickling a threshold rather than correcting
the model, and it leaves the trap armed for the scene that will actually need it. The user declined
altering content to satisfy a gate.

Raising the second lit tone until the statistic moves was measured and costs the look: splitting the
red into two palette runs cleared the gate at `0.497` and took red to 46.6 % of lit pixels, turning
every large dark mass red.

### Candidate directions, none decided

- **The scene declares its ground, and the statistics measure departure from that** rather than from
  a hardcoded `BLACK`. Most faithful to what the lens is asking; touches every call site and the
  capture surface.
- **A structural rescue in the shape of the one Plan 0075 added for thin strokes** — shell occupancy
  earned its place there precisely because it could not be bought with glow. The analogue here is a
  statistic that sees composition rather than tone.
- **A per-system precondition**: graphic systems opt out of the tonal lens and into a different one.
  Cheapest; risks becoming the exemption list this project already refuses.

### Status of the motivating content

`presets/fragment_tiledmono.toml` is finished and user-approved, sitting **untracked** in the working
tree — not committed, because `core/build.rs` globs `presets/` and landing it would take CI red.
Note that the same glob picks up untracked files, so a local `cargo nextest run -p lmv-core` fails on
it today until this is settled or the file is parked elsewhere.

### Measured against the real scene, 2026-08-25 — and `dev` has already hit the other half

Plan 0113 turned out to be **in flight**, not pending: `plan-0113-shape-collage` carries three
commits (the painter, the cost instrument, the tier caps). That makes the collision above
measurable rather than predicted, and it is worse than predicted in one direction and better in
another.

**`dev` reached the same finding independently, from the coverage side.** The branch adds a
`coverage_floor` arm for the new system whose own comment reads: *"a `shape_collage` canvas paints
its own paper across every pixel (ADR-0123), so its lit fraction is 1.0 by construction whatever the
elements do, and the statistic this floor is made of cannot distinguish a good canvas from an empty
one."* It then leans on the tonal statistic as the rescue: *"The question this family actually needs
asked is tonal, not areal — a canvas that drew no elements is a flat sheet of paper, which
`MAX_TONAL_FLATNESS` sees and coverage does not."*

**Measured on the branch's committed golden** (`core/tests/golden/shape_collage.png`, 128x128):

    coverage        1.0000   exactly, confirming the comment above
    tonal_flatness  0.7577   passes, 0.14 under the 0.90 ceiling
    paper share     75.77 % of the canvas (bucket 14); elements hold ~24 %

So today's sample canvas passes, and the black-and-white false positive is **not** yet reproduced on
this family. The problem is what the plan builds next.

**The rescue `dev` is relying on is measured only where it cannot fire.** `sanity` captures at
`LOUD`, where Phase 6's `density` lever puts the canvas at its *fullest* — the state with the most
elements and therefore the lowest flatness. The second, quieter capture (Plan 0058) buys exactly one
gate, `MODERATE_MIN_COVERAGE`, and that is the areal statistic which is degenerate at `1.0` for this
family. `quadrant_spread` and `radial_shell_occupancy` are degenerate for the same reason.

The consequence is precise: **Phase 6 explicitly builds an emptying canvas** — *"`density` gates what
fraction of the generated list is live, with elements fading in and out by age"* — and the state it
builds is measured by nothing. A canvas that empties correctly on a quiet passage and a canvas that
is broken and draws no elements are the same picture, a flat sheet of paper, and every statistic in
the lens either cannot see it or is not read at the excitation where it happens.

That is the sharp form of this entry, and it is a stronger claim than the one it was routed with:

- the **false positive** (a correct black-and-white or sparse composition convicted) is real but
  bounded — it needs ~90 % single-tone, which today's canvas is not;
- the **false negative** is unbounded and already designed-in — for this family the lens has one
  live statistic, read at the one excitation where the defect it is meant to catch cannot appear.

### What this does not settle

Whether the answer is a ground-relative `is_lit`, a structural statistic in the shape of Plan 0075's
shell-occupancy rescue, reading the tonal statistic at the quiet excitation too, or a per-system
lens, is a real decision with real alternatives and belongs in an ADR. It should land **before Plan
0113 Phase 6**, which is where the emptying canvas arrives. Phases 3-5 are unaffected.

## 0133 — `docs-shots.mjs` cannot run at all, so the operator-doc image sweep has been dead since 2026-08-15

**Raised by:** `architect`, at [Plan 0114](plans/done/0114-the-line-stroke-reads-as-a-drawn-line.md)'s
close, attempting the operator-doc image refresh that close ceremony owes.
**Owner if taken:** `dev` — three manifest entries plus a re-run, unless someone first decides what
`warp_mesh` should be a picture *of*, which is a content question.

- **Verified 2026-08-26** — the guard that aborts the run, and its message:
  `present: the gallery does not match SystemKind::from_name in: scripts/docs-shots.mjs`
- **Verified 2026-08-26** — the three systems the manifest has no gallery entry for; each of these
  reds the moment its entry is added, which is the intended re-read trigger:
  `absent: gallery/shape_field\.png in: scripts/docs-shots.mjs`
- **Verified 2026-08-26** — `absent: gallery/warp_mesh\.png in: scripts/docs-shots.mjs`
- **Verified 2026-08-26** — `absent: gallery/shape_collage\.png in: scripts/docs-shots.mjs`
- **Verified 2026-08-26** — all three are real systems the cross-check reads out of:
  `present: "shape_collage" => SystemKind:: in: core/src/preset/schema.rs`

### The finding

`scripts/docs-shots.mjs` is the committed instrument
[ADR-0100](adrs/0100-documentation-images-are-committed-headless-renders.md) makes every committed
documentation image depend on, and its own header states the contract in as many words: *"an image
set nobody can re-shoot without remembering a command line is an image set that goes stale"*, and
*"freshness is a human duty at a named cadence (the close-ceremony operator-doc sweep), not a
check."*

**It throws before rendering anything.** Its manifest holds nine gallery entries; `SystemKind`
now has twelve. The script cross-checks the two and turns a mismatch into a hard error, so
`shape_field`, `warp_mesh` and `shape_collage` having no gallery image takes down the *whole*
run — including the eight images that are nothing to do with them.

**The guard is not the bug; the cadence is.** The cross-check is doing exactly what its comment
says it is for — *"a hardcoded list of nine names would let a tenth system ship with no gallery
picture"* — and it caught precisely that. What failed is that the only thing which executes it is a
human running it at a close, and ADR-0100 deliberately keeps it out of CI (renders are not
byte-reproducible; a CI diff would be permanently red). So the guard fired into an empty room:
`shape_field` entered `SystemKind` on **2026-08-15** (`78d1671`, Plan 0091 Phase 3), `warp_mesh` on
2026-08-16 and `shape_collage` on 2026-08-25, and no close in the eleven days since could have
re-shot an image even if it had tried.

**What it is currently hiding.** Plan 0114 moved the line stroke's default `softness` and retuned
six line presets, so `parametric_curve.png`, `lsystem.png`, `star_pattern.png` and `spectrum.png`
all show a stroke the engine no longer draws. That is four of the nine gallery images stale, with
no way to refresh them and nothing that reports it.

### What a fix looks like

Three manifest entries and one run. The only real decision is what each new picture should be of —
`shape_field` and `shape_collage` have obvious shipped worlds (`shape_pulse`, `collage_suprematist`),
while `warp_mesh` ships none, so it needs a fixture bundle or a converted `.milk` named in the
manifest's provenance line like every other entry.

**The separable question worth deciding once**, because this will recur on the thirteenth system:
whether "every system has a gallery image" should be asserted somewhere that runs *without*
rendering. The cross-check is pure text — it reads the manifest and `schema.rs` and needs no GPU —
so it could live in `core/tests/` or in the `links` CI job and fail on the commit that ships a
scene, instead of silently disabling the sweep until someone tries to use it. That is a different
claim from "the images are current", which is the one ADR-0100 correctly refuses to gate.

## 0134 — a joined corner is blunt, and the stroke that hid it is gone

`renderer.rs` extends a joined end by exactly `width` along its own direction; a corner of interior
angle `theta` needs `width / sin(theta / 2)`. A `diamond`'s 61.9-degree vertex needs 26.3 px of a
13.5 px half-width and gets 15, so the point is truncated to a flat bevel, and the two quads sum on
the inner side to 1.38x the stroke.
[ADR-0041](adrs/0041-line-joins-are-per-endpoint-on-the-segment-instance.md) accepted this because
the quadratic falloff blurred it;
[Plan 0114](plans/done/0114-the-line-stroke-reads-as-a-drawn-line.md) took `DEFAULT_SOFTNESS` to
`0.25` and there is no blur left.

- **Raised:** 2026-08-27, at [Plan 0087](plans/done/0087-the-line-renderer-draws-a-curve.md) Phase 7,
  in the running app. Verdict: *"how straight lines are connected, its clearly visible and doesn't
  look solid"*. It is the other half of the sitting that closed 0073 — that entry bought the curved
  motifs, this is what is left on the straight ones.
- **Verified by measurement 2026-08-27** — a single `diamond` filling a 1000x1000 frame at
  `thickness = 9`: the profile through the acute vertex is 26 px of flat 185 and then zero, with no
  taper at all, and the corner patch reads 1.38x the stroke's own value. Both halves are the same
  constant.
- **Verified 2026-09-01** — the extension is still one unmitred `width`, and this entry stays
  live. Reduced on 2026-08-27 to the presence of the vertex shader's `select(0.0, width, ...)`
  expression;
  [Plan 0149](plans/0149-the-line-corners-stop-being-blunt.md) Phase 1 then replaced the join
  bitfield with a per-endpoint `f32` length, deleting that expression. **What it verified is
  still true**: every producer passes a flat `width` at a joined end, the geometry is
  byte-identical, and the corner is exactly as blunt as it was measured. The mechanism moved and
  the defect did not, so the reduction is re-pointed rather than the entry closed. What
  discharges this is Phase 2, and the constant it introduces is what the reduction now watches
  for:
  `absent: MITER_LIMIT in: core/src/render/scenes/lines/renderer.rs`

**This revises ADR-0041**, whose disc-join alternative closes with *"worth revisiting only if the
blunt corners above turn out to matter"*. They matter. It reaches all four line families and moves
every line golden, and the shader cannot compute `theta` on its own: a segment does not know its
neighbour's direction, which is the whole point of *per-endpoint on the segment instance*. Three
designs are open and they differ in instance-buffer cost — a per-endpoint miter length carried on
the instance, a round join drawn in the fragment, or ADR-0041's own disc-per-vertex.

**One unrelated nit to sweep while in these files**, recorded here because it is too small for its
own entry and the close write-up is not loaded per session: in
`core/src/render/scenes/lines/renderer/tests.rs` the doc block deriving the arc comparison's two
tolerances from `golden.rs` runs into `SOFT_PROFILE`'s own block with no blank line between them,
so both attach to `SOFT_PROFILE` and `ARC_MEAN_TOL` / `ARC_OUTLIER_TOL` are left undocumented.
The numbers are right — `0.02` and `48` match `golden.rs` exactly — but their derivation
now documents the wrong constant, which is the ADR-0071 failure one level down.

### Priority

**Medium.** It is a named user complaint on shipped presets, and the two roster members it disfigures
(`diamond`, `chevron`) are exactly the two Plan 0087 did *not* convert to arcs — so the defect is now
concentrated rather than diffuse.
- **PROMOTED 2026-09-01 -> [ADR-0158](adrs/0158-a-joined-end-carries-its-own-miter-length.md) + [Plan 0149](plans/0149-the-line-corners-stop-being-blunt.md) Phases 1-2.** The ADR takes the first of the three
  designs this entry lists - a per-endpoint miter length on the instance - and supersedes ADR-0041's
  geometry half on the ground this entry names: Plan 0114 removed the blur that decision rested on.
  The plan's Phase 5 also takes the `tests.rs` doc-block nit recorded at the end of this entry.

---

## 0140 — the band contour can only ever be an anti-aliased grey, so on a hard-banded palette it is the one thing that puts shading into a two-ink print

[ADR-0133](adrs/0133-the-band-contour-fires-where-the-ink-changes.md) fixed *which* edges
`band_contour` draws at. It did not touch *what* it draws, and the remaining half is the one a
limited-ink look runs into: the return is `1.0 - amount * (1.0 - smoothstep(0.0, w, d))`, a scalar
**multiplied into the colour**, ramped over one `fwidth`. So the line is always (a) a darkening
toward black rather than an ink, and (b) soft. On a palette quantized by `palette_steps` every band
edge is already hard — `band_coord` snaps to band centres and there is no anti-aliasing anywhere in
the frame — so the contour is the only source of intermediate values in the picture.

- **Raised:** 2026-08-27, from Plan 0121 Phase 6, turning `palette_contour` on in
  `shape_contourmono` now that ADR-0133 made it usable there. It **is** on and it **is** an
  improvement — this is the residual, not a regression.
- **Measured**, `shape_contourmono` at 640x360, fully driven, counting exact frame colours:

  | `palette_contour` | distinct colours in frame | pure red | the line |
  |---|---|---|---|
  | `0` | **9** | 6.00 % | none |
  | `0.25` | 80 | 4.79 % | invisible |
  | `0.5` | 179 | 4.73 % | barely visible |
  | `1.0` | 684 | 4.69 % | a key — shipped |

- **The cost does not scale with `amount`, which is the surprising part and the reason a low value
  is not a compromise.** The SET of pixels the contour touches is fixed by the geometry; `amount`
  only sets how dark they go. So `0.25` already costs 20 % of the red's pure share and the full
  jump from 9 colours to 80, and buys a line nobody can see. Only at `1.0` does about a sixth of
  the touched pixels reach true black — an ink core with the ramp either side of it — which is why
  the shipped value is the maximum rather than the usual `0.2`-`0.5`.
- **What a fix looks like.** Either would do, and neither is obviously right:
  - a **hard** contour — replace the `smoothstep` with a step at a width in band units, so on an
    already-quantized palette the line is one more flat ink;
  - a contour **colour** rather than a multiply toward black, so the key can be the palette's own
    dark ink (or the red) instead of whatever `col * k` lands on.

  The second subsumes the first only if it also gets a hardness; the first is much the smaller
  change. Both are new parameters on a surface ADR-0133 deliberately kept parameterless, so the
  question is whether one look justifies that — today it is one, `shape_contourmono`.
- **Not urgent, and the reason is honest:** the soft grey sits *aligned with a hard edge*, so it
  reads as an edge rather than as shading. That is exactly the distinction the plateau case failed
  and is why the parameter is usable at all now. The complaint is that the frame stops being
  literally two-ink at the pixel level, not that it looks wrong.
- **Verified 2026-08-27** — the contour is still a soft scalar darken with no colour of its own:
  `present: 1\.0 - clamp\(amount, 0\.0, 1\.0\) \* \(1\.0 - smoothstep\(0\.0, w, d\)\) in: core/src/render/palette.rs`

### Priority

**Low.** One preset wants it, the workaround (full strength, ink core, soft edges) is shipped and
looks good, and the fix costs a parameter on a surface that was deliberately kept free of one.
Revisit if a second limited-ink world lands on a contoured scene.

## 0142 — a same-system dissolve runs `Scene::update` twice in one frame, so every stateful scene advances at 2x for its duration

`scene_for_mut` resolves a scene by `SystemKind`, so when a dissolve crosses two presets of the
**same** system the outgoing side and the live side get the *same* `&mut Box<dyn Scene>`.
`evaluate_preset` then runs twice against it in one frame, and everything `Scene::update` mutates
advances twice.

- **Raised:** 2026-08-27, at [Plan 0121](plans/done/0121-a-rate-an-ink-edge-and-a-motion-reading.md)'s
  close review, and re-raised the same day while planning
  [0122](plans/done/0122-every-rate-integrates.md) — which moves three more rates into the affected class.
  **Owner if taken:** `dev`, but the design question is `architect`'s: see below.
- **Verified 2026-08-27** — the scene is resolved by system, so both sides of a same-system dissolve
  get one instance:
  `present: fn scene_for_mut\(scenes: &mut SceneRoster, system: SystemKind\) in: core/src/render/mod.rs`
- **Verified 2026-08-28** — and `update` carries per-frame state that is not idempotent. Re-pointed
  at [Plan 0122](plans/done/0122-every-rate-integrates.md)'s close: the accumulator survives
  unchanged, `advance_spin` does not — it collapsed into the shared `scenes::Phase` and this probe
  was anchored on the deleted function's name rather than on the claim:
  `present: self\.spin_time\.step\(self\.spin, self\.dt\); in: core/src/render/scenes/particles/mod.rs`
- **Verified 2026-08-27** — the third claim, that no test can observe this, does not reduce:
  `unprobeable: no test reaches the dual-live render path is a negative about the whole suite, not a
  match count. The mechanism is a cfg(test) escape hatch on the transition's fidelity governor - a
  headless capture has no frame-time clock, so dual_live_eligible always answers Freeze and the path
  is reachable only through Transition::set_mode. The pointer is that function's own doc comment in
  core/src/render/transition.rs`

### The finding

It is **pre-existing and it predates the rates.** `particles::spin_time` has integrated in `update`
since ADR-0076, and `emitter`'s `self.field.step(time, &cfg)` steps a particle field there too. Both
double-advance today on a same-system dissolve, and the library has twelve `fragment_field` worlds
and seventeen attractors — so crossing two presets of one system is the *common* case, not the
exotic one.

`set_time` was idempotent, which is why nothing noticed: for years the only thing a scene took from
the renderer per frame was a value, and setting it twice is setting it once. `update` growing state
changed that quietly, one scene at a time.

**Size:** the affected phase runs at 2x for the dissolve's duration and is permanently offset
afterward. For a rotation or a noise coordinate a constant offset is invisible, so the visible part
is the transient — a scene that visibly quickens for the length of a crossfade and then returns.

- **What a fix looks like**, and the choice is a real one:
  - **Make the integration idempotent per frame** — a frame counter or a dirty flag on the scene, so
    the second `update` in a frame is a no-op. Smallest change, and it leaves the double
    *evaluation* (two presets' expressions, two smoothers) doing its work correctly, which it is.
  - **Give each side its own scene instance**, the way `side.layer` already does for layer scenes
    (Plan 0076 Phase 2). Structurally right — the two sides genuinely are two presets — and much more
    expensive: a second instance of every stateful scene, allocated at dissolve start, which is
    exactly the mid-run GPU allocation ADR-0030 and the WARP trails note both warn about.
  - **Do nothing, and document it** — the offset is invisible and the transient is short.
- **The instrument problem is the real blocker, and it is why this is filed rather than planned.**
  Whatever the fix, nothing in the suite can currently *observe* the bug, so nothing can observe the
  repair either. A plan would have to start by making the dual-live path reachable from a test —
  which is its own design question, since the governor's `Freeze` answer under capture is deliberate
  and correct.

### Priority

**Low-Medium.** Invisible in the steady state and short-lived in the transient, on a path no test can
reach — but it is now the shared failure mode of six rates rather than two, and every future
`Scene::update` that grows state joins it silently. Revisit when a dissolve visibly misbehaves, or
when someone needs the dual-live path testable for another reason.

---

## 0143 — 20 of the 24 `design-backlog.md#NNNN` anchors in the repo point at bodies that now live in the archive

**Raised by:** `architect`, at [Plan 0118](plans/done/0118-the-comments-stop-narrating-the-plans-that-wrote-them.md)'s
close, 2026-08-27, while repairing the two that plan's own implementation log had noticed.
**Owner if taken:** `dev` — the repointing is mechanical; what is *not* mechanical is the scope
call in the last paragraph, which is an `architect` decision to make first.

- **Verified 2026-08-27** — the link gate says in its own header that it tests the file and not the
  fragment: `present: fragments, not external URLs in: scripts/check-doc-links.mjs`
- **Verified 2026-08-27** — an archived body still addressed by a live anchor:
  `present: ^## 0072 in: docs/design-backlog-archive.md`

### The finding

The close ceremony archives a discharged backlog entry by moving its body verbatim to
`design-backlog-archive.md` and leaving a ledger row behind. Anchors aimed at the moved body —
`(../design-backlog.md#0072--sanityrss-coverage-floor-forces-...)` — keep resolving to a **file that
exists**, so `scripts/check-doc-links.mjs` reports them clean: it validates paths and never
fragments. The link lands at the top of the live backlog instead of at the entry, silently.

Measured across every `.md` in the repo: **24 distinct entry numbers are addressed by anchor, and 20
of them are archived** — `0009 0020 0022 0027 0033 0040 0055 0056 0057 0058 0059 0060 0061 0062
0063 0067 0070 0084 0085 0088` — across roughly 20 files, mostly ADRs and closed plans. Two more
(`0072`, twice in [Plan 0075](plans/done/0075-the-content-renaissance.md)) were repaired at Plan
0118's close and are the reason this was measured at all.

This is the same silent-rot family ADR-0127 retired one level down: a reference whose *form* cannot
be checked, decaying on a routine ceremony step, visible only to a human who follows it. Step 3c of
the architect skill already names it — *"the one class of break here that no gate will catch for
you"* — which is why the accumulation is evidence that naming it was not enough.

### The scope call, which is the actual question

Most of the 20 sites are **append-only** documents: accepted ADRs and closed plans. Repointing them
edits a historical record to keep a convenience link working, which cuts against the append-only
rule. Three defensible answers, and one should be chosen before any editing:

1. Repoint everything, treating a link target as mechanical rather than as content.
2. Repoint only live documents and leave the historical ones, accepting that an old ADR's anchor
   lands at the top of the backlog.
3. Drop the anchor half repo-wide — `[backlog 0072](../design-backlog.md)` plus the bare number,
   which is exactly ADR-0127's answer to the same problem in `.rs`, and would make the whole class
   uncheckable-but-unbreakable.

Option 3 also makes the gate question moot; options 1 and 2 leave a class that wants a fragment
check in `check-doc-links.mjs` to stay repaired.

---

## 0146 — `warp_mesh` colours its light at deposit time, so the palette cannot band the accumulated field

The palette coordinate in `warp_mesh` is the **deposit angle**: the deposit shader computes
`coord = hue + color_center + color_span * (ang / 2pi)`, bands it through `band_coord`, samples both
LUTs and writes premultiplied colour into the field. The field then decays and warps
already-coloured pixels, and the present pass is a scale plus MilkDrop's four composite remaps with
no palette lookup in it. There is no path from an accumulated field **level** to a palette
coordinate, so `palette_steps` quantizes the light going in rather than the structure coming out.

- **Raised:** 2026-08-27, by the content lane, wanting a mono world whose hard ink bands are the
  feedback field's own decay contours — an op-art ladder marching outward. `palette_steps` and
  `palette_contour` are both in `PARAMS` and documented live on this system, which is what made the
  look look reachable.
- **Measured:** a 20-band plateau palette with `deposit_arms` and decay renders a smeared coloured
  blob with **no bands at all** — the plateaus are destroyed by the very feedback that was supposed
  to reveal them.
- **Impact:** one of two mechanisms that closed a whole world (see 0148 for the other). `warp_mesh`
  is effectively unavailable to any hard-ink or posterized look.
- **Verified 2026-08-27** — the palette coordinate is the deposit's own angle, computed in the
  deposit pass:
  `present: let coord = dp\.c\.y \+ dp\.c\.z \* \(ang / 6\.2831853\); in: core/src/render/scenes/warp_mesh/mod.rs`
- **Verified 2026-08-31** — and the banding constants ride the deposit uniform, not the present one.
  Plan 0125 moved the field into the shared colour block, so the spelling changed and the claim did
  not: the call sits in the same `DepositUniform` write it always did.
  `present: palette::band_steps\(self\.colour\.steps\), in: core/src/render/scenes/warp_mesh/mod.rs`
- **Verified 2026-08-27** — `unprobeable: that the present pass performs no palette lookup is an
  absence inside one function of a file whose other function does perform one, so no file-scoped
  reduction separates them; read PRESENT_SRC's body.`

### The shape of the question

`shape_field` already does the thing this wants — [ADR-0105](adrs/0105-the-mark-roster-becomes-a-fullscreen-distance-field.md)
calls it the one scene whose palette coordinate is a *distance*, which is what makes `palette_steps`
draw concentric contours. The `warp_mesh` analogue would be a palette coordinate taken from the
field's own level at present time. That is a genuine second colour path on a scene that already has
one, and the two would have to compose or exclude — which is the design question, and it is not
small enough to fold into someone else's plan.

### Priority

**Medium.** It closes a system to a palette class rather than breaking anything that ships, and the
cohort has four systems that do work. It rises if `warp_mesh` is wanted for a limited-ink world
specifically, because nothing else in the engine makes a decay contour.

---

## 0149 — three bindable rates multiply a per-element `age` instead of integrating, and the guard ADR-0135 shipped cannot see any of them

[ADR-0132](adrs/0132-a-rate-parameter-integrates-a-phase.md) decides that **every bindable rate
parameter in this engine integrates a phase**. [ADR-0135](adrs/0135-every-scene-rate-integrates-through-one-shared-phase.md)
and [Plan 0122](plans/done/0122-every-rate-integrates.md) delivered that for the six rates measured
against `self.time`, and added a `hygiene.rs` guard that fails the build on
`self.<field> * self.time`. **Three more rates multiply a per-element `age` instead**, which is the
same defect against a different clock — and the guard matches the shared clock by name, so it passes
all three.

- **Raised:** 2026-08-27, at Plan 0122's Mode 4 close review, by grepping for the *mechanism*
  (`* age`) rather than for the spelling the guard knows. **Owner if taken:** `architect` first — the
  repair shape is a real design question (see below) — then `dev`, then `preset-author` for the three
  affected worlds.
- **Verified 2026-08-27** — the collage rotation multiplies the element's age:
  `present: p\.spin \* spin \* age in: core/src/render/scenes/shape_collage.rs`
- **Verified 2026-08-27** — and so does its translation:
  `present: p\.vel\[0\] \* drift \* age in: core/src/render/scenes/shape_collage.rs`
- **Verified 2026-08-27** — the emitter's sprite rotation, the latent third:
  `present: base \+ rate \* age in: core/src/render/scenes/emitter.rs`
- **Verified 2026-08-27** — and shipped content binds one of them to a band:
  `present: clamp\(mid \* 0\.59, 0, 0\.5\) in: presets/collage_suprematist.toml`
- **Verified 2026-08-27** — the operator doc still describes the defective form as the safe one:
  `present: Integrated against real elapsed time in: presets/README.md`

### The finding

`shape_collage::apply_time` computes an element's placement from its age:

    center:    p.spec.center + p.vel * drift * age      (shape_collage.rs:1320-1321)
    angle_deg: p.spec.angle_deg + (p.spin * spin * age) (shape_collage.rs:1324)

`drift` and `spin` are both in that scene's `PARAMS` roster, so both are bindable, and a binding that
*moves* retroactively rescales every second of the element's life. `emitter::sprite_angle` has the
same shape (`base + rate * age`, `emitter.rs:776`) with `spin` bound only to constants today —
exactly the status `parametric_curve`'s `spin` and `warp_mesh`'s `deposit_spin` had when ADR-0132
corrected them anyway rather than leave counterexamples.

**Three shipped presets bind the collage pair to audio**, which is more content than the `swarm` pair
Plan 0122 existed to fix:

    collage_onwhite.toml:108-109      drift bass swing 0.4   spin mid swing 0.35   [smoothing] 0.6
    collage_suprematist.toml:116-117  drift bass swing 0.6   spin mid swing 0.5    [smoothing] 0.6
    collage_mono.toml:43-44           drift bass swing 0.60  spin mid swing 0.30   [smoothing] 0.60

**Size, computed rather than measured.** A one-pole at `tau = 0.6` closes 2.74 % of its gap per 60 Hz
frame, so `collage_suprematist`'s `spin` moves 0.0137 in a frame across its 0.5 swing. With
`SPIN_SPEED = 0.07` the angle jumps `0.07 · 0.0137 · age` — at `age = 30 s` that is 0.029 rad in one
frame against a nominal `0.07 · 0.5 / 60 = 0.00058`, about **49x**; `drift` is ~35x by the same
route. **Milder than `swarm`'s 210x and bounded differently**: `age` resets on each `recompose`,
where `swarm`'s `time` never resets, so in normal playback this stays a jitter rather than a
teleport. The exception is the case with no onsets — `recompose` is gated on `hash(beat_index)`, so
in a quiet passage it never fires, `age` grows unbounded, and the first bass hit after it lands the
full accumulated swing.

**`presets/README.md:1536` documents the defect as the safe form**, which is how three presets came
to bind it: *"Integrated against real elapsed time, so the canvas moves identically at any refresh
rate."* True about frame-rate independence, false about ADR-0132 — the rate scales the accumulation
rather than being integrated into it — and the `pump_*` row three lines below says "drive the depth
from the music". That row is load-bearing for the `preset-author` lane and should be corrected
whether or not the engine repair is taken.

- **What a fix looks like**, and it is **not** `scenes::Phase`:
  - `Phase` is one accumulator per scene. These need **one per element**, advanced with the element
    and reset when it is born or the canvas recomposes — a different shape, and the reason Plan 0122
    scoped them out rather than folding them in.
  - The cheap alternative is to bake the rate at spawn the way `emitter` already bakes `v0`, so a
    moving binding affects new elements only. That changes what the parameter *means* (it stops
    steering the live canvas), so it is a design call, not a refactor.
  - The emitter's `spin` may be a third case again: sprites are short-lived, so `age` is small and
    the defect may be unobservable. Worth measuring before spending anything on it.
- **The guard question is the durable half.** ADR-0135's guard makes one spelling impossible and says
  nothing about the rule; this entry is the proof. Whether it should reach `* age` is genuinely open
  — `emitter.rs:375-376`'s `v0 * age` ballistics are legitimate (the velocity is baked at spawn), so
  a naive widening false-positives on correct code.

### Priority

**Medium.** Three shipped presets carry it against `swarm`'s two, and the doc actively teaches the
defective form — but the magnitude is 4-6x smaller than the defect Plan 0122 fixed and `age`'s reset
bounds it in ordinary playback. The half worth doing immediately and cheaply is the
`presets/README.md` correction, which costs one sentence and stops the content lane from writing more
of these.

---

## 0150 — `Phase::step` accepts any `dt`, so the guard against a poisoned accumulator is four copies in the callers and the attractor has none

[ADR-0135](adrs/0135-every-scene-rate-integrates-through-one-shared-phase.md) put every bindable
rate behind one `scenes::Phase`, whose whole reason to exist is that `+= rate · dt` becomes the only
way an accumulator moves. It does not constrain what `dt` may be. A single non-finite frame writes
`NaN` into a `Phase` **permanently** — the type has no other mutator, so nothing can ever clear it —
and the defence against that lives, byte-identical, in four separate `Scene::advance` impls. The one
scene holding a `Phase` that does *not* carry it is `attractor`.

- **Raised:** 2026-08-28, at Plan 0122's close, from the Mode 4 review's second `minor`. **Owner if
  taken:** `architect` first — where the guard belongs is a real design call, see below — then `dev`.
- **Verified 2026-08-28** — the shared type takes `dt` on trust:
  `present: pub\(crate\) fn step\(&mut self, rate: f32, dt: f32\) in: core/src/render/scenes/mod.rs`
- **Verified 2026-08-28** — and the attractor stores the frame's `dt` raw beside a `Phase`. The
  probe discriminates: this spelling matches only the two unguarded scenes and none of the four
  guarded ones, so it goes red on the repair rather than on decay:
  `present: self\.dt = dt; in: core/src/render/scenes/particles/mod.rs`
- **Verified 2026-08-28** — while four callers each re-derive the same three lines:
  `present: dt\.is_finite\(\) && dt > 0\.0 in: core/src/render/scenes/swarm.rs`

### The finding

`Phase::step` is `self.0 += rate * dt` with no precondition (`scenes/mod.rs:87`). Four scenes sanitize
`dt` on the way in, with the same expression each time and four separately-written comments giving
the same reason:

    fragment_field.rs:446    dt.is_finite() && dt > 0.0 else FALLBACK_DT
    lines/parametric.rs:330  dt.is_finite() && dt > 0.0 else FALLBACK_DT
    swarm.rs:724             dt.is_finite() && dt > 0.0 else FALLBACK_DT
    warp_mesh/mod.rs:1712    dt.is_finite() && dt > 0.0 else FALLBACK_DT

`particles/mod.rs:1339` writes `self.dt = dt` unguarded, and `update` then runs
`self.spin_time.step(self.spin, self.dt)`. `spin_time` is a `Phase`; a `NaN` or negative `dt` from
the shell — a suspended window, a clock that jumps backwards, a `dt` computed across a device loss —
lands in it and stays. Every attractor world's display rotation is dead for the rest of the process.
`FixedStep::advance` on the line above **self-heals** (`accumulator.min(step)` returns `step` when
one operand is `NaN`), which is exactly why the omission reads as safe on inspection.

**This is the shape ADR-0135 was written against, one level down.** That ADR's own Context calls four
copies of the same three lines *"a rule enforced by a list of sites"* and rejects Alternative A on the
grounds that the duplication is how the defect returns. Four copies of the `dt` guard is that
sentence again, and the site with no copy is the one it predicts.

- **What a fix looks like**, and the choice is not obvious:
  - **Sanitize inside `Phase::step`.** One line, kills all four copies, and the invariant sits on the
    type that exists to hold invariants. But `self.dt` has readers that are **not** `Phase` —
    `swarm`'s damping `powf`, `warp_mesh`'s `pow` — so the four callers would still want their own
    guard and the duplication survives with a narrower job.
  - **Sanitize at the trait seam**, in `draw_frame` before `Scene::advance` is called, so no scene
    ever sees a bad `dt`. Fixes every reader at once and deletes all four copies. Widens what the
    renderer promises about the argument, which is an ADR-0002 question rather than an edit.
  - **A `Dt` newtype** that cannot be constructed non-finite, taken by `advance` and by `step`.
    Strongest, and the largest diff.
- **What it is not:** copying the fourth guard into `particles/mod.rs`. That closes the one live hole
  and leaves five copies, which is the option ADR-0135 already rejected once by name.

### Priority

**Low-medium.** Nothing observed — the shells feed real elapsed time and no capture path produces a
bad `dt`, which is why it survived a plan whose whole subject was these accumulators. It is filed at
this size because the cost of the repair only grows: `Phase` is now the engine's one rate mechanism,
so every rate added after this inherits whichever answer is not chosen.

## 0153 — the palette consumes its stops as linear light, and `preset-palettes.md` presents the resulting shift as unavoidable when below the tonemap knee the author can correct it exactly

> **Filed 2026-08-28** at the Plan 0123 Mode 4 review, from the `preset-author` finding in that
> plan's implementation log (Phase 9), which routes the entry here because the content lane does not
> edit this file. See [ADR-0138](adrs/0138-limited-ink-is-a-supported-palette-class-defined-at-the-draw-seam.md)'s
> Outcome and archived 0148.

`LUT_TEXTURE_FORMAT` is `Rgba8Unorm` and `core/src/render/palette.rs` records that the entries are
used as colour directly, *"no perceptual/gamma management; that is deferred, ADR-0021 Alt E"*. So a
stop written as ordinary sRGB hex is consumed as **linear** light and the display encode lifts it.
Measured at Plan 0123 Phase 9:

| stop written | renders as |
|---|---|
| `#c81423` | `#dd4c64` — green channel nearly quadrupled, the ink arrives coral |
| `#930204` (the sRGB-to-linear value of `#c81423`) | `#c81622` — within 2/255 of the colour named |

This is also why `collage_mono`'s `#b00808` arrives as `#d63131`; Phase 8's measurement recorded that
shift without naming its cause.

**Two separable halves, and the second is worth having whatever the first decides.**

1. **Is the deferral still right?** ADR-0021 Alt E deferred perceptual/gamma management deliberately,
   and reversing it re-bases every shipped palette in the library — the same class of change
   ADR-0126 and Plan 0116 had to absorb. That is a real design question and is **not** being asserted
   here as a defect.
2. **The page is wrong either way.** `docs/preset-palettes.md`'s Remaps section tells a limited-ink
   author that *"a limited-ink frame's plateaus almost never carry the palette's literal RGB, and
   that is fine."* Below the tonemap knee at linear `0.6` the curve is exactly the identity, so the
   shift there is **not** unavoidable — it is exactly correctable by pre-converting the stop, as the
   measurement above shows. The page as written tells an author to give up on something they can
   fix, in the one section aimed at authors who care about exact inks.

**Impact:** the second half misleads precisely the audience the limited-ink class was added for, and
it shipped in the same plan that added the class. The first half is a standing question about a
deferral, not a bug.

**What a fix looks like:** for the second half, one paragraph in the Remaps section giving the
mechanism and the recipe (pre-convert the stop; valid below the knee; above it the channels scale
together and the plateau survives anyway). For the first, an ADR that either re-affirms ADR-0021
Alt E with this cost written down or supersedes it with a migration for the shipped palettes.

- **Verified 2026-08-28** — the deferral is still recorded in the code: `present: no perceptual/gamma management in: core/src/render/palette.rs`
- **Verified 2026-08-28** — and the page still tells the author it is unavoidable: `present: plateaus almost never carry in: docs/preset-palettes.md`

## 0154 — a swap spawns a thread that creates a COM object, and one activation in 22 failed with `REGDB_E_CLASSNOTREG` where the retry budget cannot tell that from a dead device

> **Filed 2026-08-28** at the Plan 0130 Mode 4 review, from that plan's own Phase 5 log — an
> observation the plan reported honestly, claimed no mechanism for, and had nowhere to leave.

Before Plan 0130 `capture_win::start` ran **once per process**. It now runs on every input swap and
on every recovery attempt, and each run spawns a thread that does `CoInitializeEx(MULTITHREADED)`,
`CoCreateInstance(MMDeviceEnumerator)`, `CoUninitialize`, and exits. Under menu-speed churn on the
development box, **one swap in 22 failed** at that `CoCreateInstance` with
`REGDB_E_CLASSNOTREG (0x80040154)`.

**What is and is not claimed.** The shell degraded exactly as designed — the reason printed, the
verdict read `failed WASAPI … Class not registered`, rendering continued, and the next swap came
back live. No mechanism is claimed: the apartment-churn reading is untested, one run on one box is
not evidence for a cause, and 1-in-22 is a single sample, not a rate.

**Why it is worth an entry rather than a note.** `poll_input_lost` reopens up to
`INPUT_RECOVERY_ATTEMPTS` times on **consecutive frames** — the fastest thread-spawn churn the design
can produce, and faster than the churn that drew the error. The budget exists to bound COM
activations against a device that is not coming back; it cannot distinguish an activation that
failed *for its own reasons* from one that failed *because the endpoint is gone*, so a real loss
whose reopens drew this error would spend all three attempts on the wrong failure and write a
`lost …` verdict about a device that was fine. That is the one path where this turns a transient
into a wrong answer, and it is also the path
[`docs/on-device-validation.md`](on-device-validation.md)'s unplug item says has never run.

**Impact:** low frequency, narrow blast radius, and invisible while swaps stay operator-paced. It
matters only where the design already churns fastest, which is the recovery path.

**What a fix looks like** — three shapes, cheapest first, and picking between them wants the unplug
evidence rather than more reasoning:

1. **Retry the activation once, in place**, before charging the attempt to the recovery budget. A
   class-registration failure is not a statement about the endpoint, so it should not spend a
   budget that is counting statements about the endpoint.
2. **Keep one long-lived enumerator on the render thread** rather than creating one per stream
   start. `MMDeviceEnumerator` is a Both-model object and `ComScope` already handles the STA the
   render thread lives in, so `endpoints()` has the pattern; `setup_stream` creates its own because
   it runs on the capture thread.
3. **Separate the two failure classes in the verdict**, so `failed … Class not registered` and
   `lost …` never read as the same conclusion about the device.

- **Verified 2026-08-28** — a fresh COM object is still created per stream start, on the capture
  thread: `present: CoCreateInstance in: standalone/src/capture_win.rs`
- **Verified 2026-08-28** — and nothing anywhere distinguishes this failure class from a dead
  endpoint: `absent: REGDB in: standalone/src`
- **Verified 2026-08-28** — the budget that would be spent on it is still the only bound:
  `present: INPUT_RECOVERY_ATTEMPTS in: standalone/src/main.rs`

**Update 2026-08-30, at Plan 0135's close — still live, still unevidenced.** That plan gathered the
three fixes whose shape was settled and left this one deliberately unfixed: its Phase 5 was a
`human` unplug gate whose whole deliverable was the evidence this entry asks for, and **it did not
run**, for the same reason Plan 0130's Phase 5 did not — there is no removable audio interface on
the box. Nothing about the three candidate shapes has changed and none is preferred; the entry is
carried, not stalled.

**Two things Plan 0135 did change under it**, both worth knowing before anyone re-reads the reasoning
above. The reopen budget is unchanged — still `INPUT_RECOVERY_ATTEMPTS = 3` spent on **consecutive
frames**, so the *fastest churn the design can produce* is still what this entry says it is, and it
is still refresh-rate-dependent even though the *settle* window beside it is now in seconds
(`INPUT_RECOVERY_SETTLE_SECS`, backlog 0155, archived). And an operator-initiated restart now resets
the policy (backlog 0156, archived), so the swap that drew the original 1-in-22 observation returns
a full budget where it used to inherit a spent one — which makes a repeat observation *more*
likely to be visible, not less.

**Where the evidence is now owed from:** the unplug checkbox in
[`docs/on-device-validation.md`](on-device-validation.md), which carries Plan 0135's three extra
questions (**(d)** does `REGDB_E_CLASSNOTREG` appear during a *real* loss, **(e)** how many attempts
a real unplug consumes, **(f)** does the verdict name the right cause) alongside Plan 0130's
original three, and a Standing bullet in [the plans index](plans/README.md). Run against v0.95.0 or
later, or the policy under test is not the repaired one.
- **PARTLY PROMOTED 2026-09-01 -> [Plan 0147](plans/0147-what-the-show-costs-and-what-its-numbers-mean.md) Phase 2**, which takes **only the third shape** - the verdict
  stops reading the same about an activation and about a device. **The mechanism halves stay filed and
  this entry stays live**, because choosing between retry-in-place and a long-lived enumerator wants the
  unplug evidence, and the box still has no removable interface.

## 0157 - the fixed telemetry set omits the bar grid the engine already computes, so a consumer reconstructs a worse one by hand

> **Filed 2026-08-29** from the live lighting rig, after the 2026-08-29 set.

ADR-0144 chose a fixed OSC vocabulary and listed it as *"the four normalized levels, the raw levels,
onset, the beat counter, beat phase, tempo, and the preset name."* The beat counter it means is
`beat_index`, which counts **onset detections** at 1.35x-2.10x per musical beat (ADR-0109). Nothing
in the published set is a musical beat.

**But `AnalysisFrame` carries one.** Plan 0095 built `BarGrid`, and the frame exposes `beat_in_bar`,
`bar_index`, `bar_phase`, `downbeat_confidence` and `downbeat_locked` - a tempo-locked grid with a
lock flag. `core/src/dsp/downbeat.rs` says so in as many words: the fold is driven *"by the grid's
beat count, which is driven by the tempo estimate; `beat_index` counts transients."* The telemetry
set publishes the transient counter and withholds the grid.

**What that cost downstream, measured.** The lighting bridge needed one white flash per musical
beat. Given only `beat_index` it fired **3.6 times a second**, which read as frantic rather than
dramatic. The workaround was to gate a `beat_index` increase against a locally folded tempo and
suppress anything inside 85 % of a beat period - **a beat detector rebuilt in the consumer, from
strictly less information than the engine already had**, with no access to the lock flag that would
have said whether the grid was even tracking.

**Why this is the interesting class.** It is not a defect in the analyzer; the signal exists and is
correct. It is that a *fixed* vocabulary chose its members before there was a consumer, and the
first real consumer wanted a member that was not on the list. ADR-0144 named exactly this risk -
*"the fixed OSC set may prove too narrow"* - and asked for the evidence an operator would produce.
This is that evidence, arriving from the first set played.

**What a fix looks like:** publish the grid. `bar_index`, `beat_in_bar`, `bar_phase` and
`downbeat_locked` under the existing `/lmv/v1` prefix, which is additive by construction - the
prefix is versioned precisely so a later signal does not break a console mapping. The lock flag
matters as much as the count: a consumer that cannot tell a locked grid from a warming one has to
guess, which is what the 85 % window was.

- **Verified 2026-08-29** - the grid the set omits is still computed and still public: `present: pub bar_index in: core/src/dsp/mod.rs`
- **Verified 2026-08-29** - and the frame still carries the lock flag a consumer would need: `present: pub downbeat_locked in: core/src/dsp/mod.rs`
- **Verified 2026-08-29** - and the published set still omits every one of them, which is the claim itself: `absent: bar_index|beat_in_bar|bar_phase|downbeat_locked in: standalone/src/osc.rs`. **This bullet was an `unprobeable:` opt-out until Plan 0132 closed**, on the reasoning that the sink lived on an unmerged lane and no probe against it resolved on `main`. That lane merged, so the opt-out expired and the reduction is a real one: it goes red the day the grid is published, which is the day this entry is discharged.

## 0158 - the tempo octave is unsettled by design, so every consumer folds it, and the rig observed the fold running the opposite way from the documented bias

> **Filed 2026-08-29** from the live lighting rig, after the 2026-08-29 set.

`core/src/dsp/tempo.rs` searches lags between `MIN_BPM = 60.0` and `MAX_BPM = 200.0` and states
plainly that it **does not settle the octave and is not trying to** (Plan 0095). That is a defensible
position for an engine: a preset binding a rate to `tempo` mostly wants a period, not a musical
claim.

**It stops being free the moment something derives timing from it.** The lighting bridge derives
every timing from the tempo - one structural climb per eight beats - so an unfolded estimate made
the whole rig climb at twice the asked rate. The remedy was three lines, halving above 140 and
doubling below 70, and **every future consumer of `/lmv/v1/tempo` will write those same three
lines**, each choosing its own window, none of them recorded anywhere.

**The direction of the error is the part worth flagging, because it does not match the record.**
`tempo.rs` documents the ambiguity as **one-sided**, with `the_octave_ambiguity_is_one_sided` in
`core/tests/tempo_probe.rs` printing that *the slower reading dragged the 140, 165 and 200 BPM rungs
down an octave* - the estimator reading **low**. On the rig it read **high**: 200.9 BPM reported for
material plainly at half that. One of three things is true and nothing here decides which - the
one-sidedness does not hold on real music, the rig's material sits where the bias inverts, or the
`200.9` reading was the estimator still warming and was never a settled lock. Plan 0132's own log
records a warming artefact of exactly this shape being retracted once already (59.84 BPM at 13 s
becoming 127.84 BPM at 45 s on the same signal), which makes the third possibility live.

**Note the probe that would have caught it prints rather than asserts.** That is deliberate and
correct - it is a measurement, not a property, and ADR-0071 is why it is not an assertion. It does
mean nothing fails when the behaviour changes.

**What a fix looks like:** publish a folded tempo beside the raw one rather than replacing it -
presets and the OSC contract are bound to `bpm` as it is. The fold window is the design question and
it is not obviously 70-140. **Before any of that, establish which of the three explanations is
true**, because if the high reading was a warming artefact then the fold is solving a problem that
does not exist and the real answer is a lock flag on the tempo, which 0157 shows already exists for
the grid.

- **Verified 2026-08-29** - the search range is still 60-200 with no octave resolution: `present: const MAX_BPM: f32 = 200.0 in: core/src/dsp/tempo.rs`
- **Verified 2026-08-29** - and the estimator still declines to settle the octave: `present: does not settle the octave in: core/src/dsp/tempo.rs`

## 0160 - the test suite re-creates a `target/` inside the worktree that no redirect reaches, and its own comment says it cannot

> **Filed 2026-08-29** at Plan 0129's close, from that plan's Phase 4, which found it and left it as
> found per the phase's own instruction.

[ADR-0141](adrs/0141-one-artifact-store-serves-every-lane.md) points every lane at one shared
artifact store through a machine-local `build.target-dir`. Plan 0129 Phase 3's done-when says the
lanes write there and *"none of them re-creates its own `target/`"*, and that holds for **every
build** and fails on a **test run**.

`standalone/tests/shot_cli.rs` has two path helpers and only one of them is redirect-safe:

- `shot_exe()` walks up from `std::env::current_exe()` looking for an `examples/` sibling, so it
  follows the store wherever it goes. It is correct, and it carries the comment that makes the file
  look already audited.
- `scratch()` builds `repo_root().join("target").join("shot-cli-tests")`. `repo_root()` is derived
  from `CARGO_MANIFEST_DIR`, not from where cargo writes, so this reaches into the **worktree**
  regardless of the redirect.

Its doc comment states the invariant it breaks: *"under `target/` so it never escapes the build
tree."* Under the store, `target/` is not the build tree, so the scratch output escapes it in
exactly the direction the comment promises it will not.

**Measured 2026-08-29** on the close checkout, after the suite had run under the store: the
repository held a `target/` containing `shot-cli-tests/` and nothing else, ~4 MB.

**Why nothing surfaces it.** `**/target` is gitignored, so `git status` stays clean; the directory
is small, so no disk pressure appears; and the tests pass either way, because the path is created
before use. It is visible only by looking.

**Impact.** Low today - a stray gitignored directory per worktree that `git worktree remove` still
deletes. It matters because it is the one place a redirect assumption is written into a **test**
rather than a script, and because the comment above it actively argues the opposite, which is how
the next reader gets it wrong.

**What a fix looks like:** `env!("CARGO_TARGET_TMPDIR")`, which cargo sets to a per-test-binary
directory inside the real target dir and which exists for exactly this - or `cargo metadata`'s
`target_directory`, the answer `plugin-foobar/build.ps1` already uses. Either way the doc comment
has to be re-stated, since it is the part that is wrong independently of the path.

> **Updated 2026-08-30, at Plan 0134's close.** The redirect this entry is written against is gone -
> [ADR-0147](adrs/0147-the-shared-artifact-store-is-revoked-and-the-linker-stays.md) revoked
> ADR-0141's shared store, so `target/` **is** the build tree again and `scratch()`'s doc comment is
> accidentally true. **The entry stays live and the fix is unchanged**: the code still derives a
> cargo *output* path from `CARGO_MANIFEST_DIR` rather than from where cargo writes, which is wrong
> independently of any redirect and is exactly what a returning redirect would break. What changes
> is the impact - from a live stray directory to a latent one. Read ADR-0141 above as history.

- **Verified 2026-08-29** - the scratch path still starts at the repo root: `present: let dir = repo_root\(\) in: standalone/tests/shot_cli.rs`
- **Verified 2026-08-29** - and still joins its way to a dir inside it: `present: \.join\("shot-cli-tests"\) in: standalone/tests/shot_cli.rs`
- **Verified 2026-08-29** - the sibling that is correct is still correct, so the file is genuinely split: `present: current_exe in: standalone/tests/shot_cli.rs`

## 0161 - three committed scripts still resolve cargo output under `<repo>/target`, which the artifact-store docs assert nothing does

> **Filed 2026-08-29** at Plan 0129's close, by grepping the class its Phase 5 fixed one instance of.

[ADR-0141](adrs/0141-one-artifact-store-serves-every-lane.md)'s Negative section names **one**
committed script the redirect breaks, `plugin-foobar/build.ps1`, and Plan 0129 Phase 5 fixed it
properly - it now reads `target_directory` from `cargo metadata`, correct under both layouts, with
both branches exercised. The plan's Phase 7 documentation then generalized that into a rule, and
`.claude/skills/dev/references/project-context.md` states it flatly: *"Never hardcode
`<repo>/target` in a script or a test."*

**Three committed scripts still do**, and they resolve cargo *output*, so a redirect points them at
a path that does not exist:

- `packaging/macos/bundle.sh` - `${repo_root}/target/${triple}/release/lmv` for both Apple targets
  before the `lipo`. Inert today because the config is Windows-only and no Mac has opted in; it is
  the one on a **release** path, so it is the one that matters if that changes.
- `renders/plan-0106-p6/run.sh` and `renders/plan-0106-p7/run.sh` - `SHOT=target/release/examples/shot.exe`.
  Broken on the development machine right now. They are archived one-off render scripts from a closed
  plan, so nothing runs them on a schedule.

**Not in this class, checked:** `packaging/foobar/build-component.ps1` reads its DLL from
`plugin-foobar/build/` and only *writes* to `$repo\target\dist`, which is where CI's
`release.yml` expects to find the zips - correct, and it should stay. `.github/workflows/release.yml`
uses `target/` throughout and is right to: CI never has the config. The `scripts/*.mjs` default
out-dirs under `target/` are outputs into a gitignored directory, the same untidiness as 0160
rather than breakage.

**Impact.** Low and latent. Nothing here fails a gate, nothing fails CI, and the only currently
broken pair is archived. The reason to record it is that the documentation now asserts a property
the tree does not have, and a doc that overstates its own sweep is what makes the next person stop
looking.

**What a fix looks like:** `bundle.sh` asks `cargo metadata --format-version 1 --no-deps` for
`target_directory` the way `build.ps1` does - four lines and a `jq` or a `python -c`. The two
`renders/` scripts are historical and the honest options are the same one-line fix or a line in
`renders/README.md` saying they assume the pre-ADR-0141 layout.

> **Updated 2026-08-30, at Plan 0134's close.**
> [ADR-0147](adrs/0147-the-shared-artifact-store-is-revoked-and-the-linker-stays.md) revoked
> ADR-0141's shared store, so all three scripts resolve correctly again - **by accident, not by
> repair**. The entry stays live: the rule they violate is unchanged, the redirect could return, and
> `cargo metadata` is the right question under either layout. One line to add to the roster above -
> `plugin-foobar/build.ps1:32` still cites ADR-0141 as the live reason for asking `cargo metadata`;
> its behaviour is correct and only the citation is stale. Read ADR-0141 above as history.

- **Verified 2026-08-29** - the macOS bundler still builds its binary paths from the repo root: `present: repo_root\}/target/ in: packaging/macos/bundle.sh`
- **Verified 2026-08-29** - the two render scripts still assume the old layout: `unprobeable: renders/ is gitignored, so both scripts exist on the authoring machine and in no checkout - probing them passes here and breaks every fresh clone`
- **Verified 2026-08-29** - the script the plan actually fixed no longer does: `absent: Join-Path \$repo "target in: plugin-foobar/build.ps1`

## 0162 - the claim gate resolves a probe path against the working tree, so a path inside `.gitignore` verifies locally and can only ever fail on CI

> **Filed 2026-08-29** from a red CI `links` job whose one broken probe was entry 0161's own,
> written hours earlier by the close that filed it and green on the machine that wrote it.

`runProbe` resolves `<path>` with `existsSync` against the working tree and asks nothing further. A
path that exists but is **not in the repo** is indistinguishable from one that is, so a probe
pointing into gitignored territory - `renders/`, `target/`, `spike/` - passes at every call site
that runs on the authoring machine and fails only at the one that does not.

Two of the three call sites are on that machine: the `pre-push` hook and the architect close
ceremony both read a full working tree, ignored files included. The third is the CI `links` job,
which [ADR-0108](adrs/0108-a-backlog-claim-about-the-repo-carries-an-executable-probe.md) names the
un-bypassable one, and it checks out a clone that by definition contains no ignored file. So the
gate is green wherever the fix would still be cheap and red only once the push has happened.

**The instance.** Entry 0161's second bullet probed `renders/plan-0106-p6/run.sh`. `renders/` is
ignored in full, so that script exists on the authoring machine and in no checkout anywhere. It
went green at the close that filed it and green at pre-push, then broke the `links` job on the
first push that carried it - which was a push of 12 accumulated commits, so the breakage surfaced
against an unrelated fix rather than against the close that caused it. Repaired in place by taking
the `unprobeable:` opt-out, which is the honest reduction here: nothing in a checkout can see it.

**Impact.** Low severity, and it recurs by construction rather than by accident. The cost is not
the one broken entry - it is that the local gate whose entire purpose is to pre-empt a red CI
reports OK, so the author learns from a runner, after pushing, that a claim they verified does not
verify. The lag is unbounded: an unpushed close can sit for days before a push exposes it.

**What a fix looks like:** one git question beside the `existsSync`. `git ls-files -- <path>`
returns nothing for a path the repo does not track and handles directories, which probe paths often
are, so a non-empty result is the whole test; `git check-ignore -q` answers it from the other side.
Either lets the probe report *"probe path is not tracked"* rather than today's *"does not exist"*,
which is the message a CI reader currently gets for a file sitting in front of them in their own
tree. One invocation covering the whole probe set keeps it to a single process, the way the
staleness advisory already batches its `git log`.

- **Verified 2026-08-29** - the probe path is resolved against the filesystem and nothing else: `present: if \(!existsSync\(pathAbs\)\) in: scripts/check-backlog-claims.mjs`
- **Verified 2026-08-29** - nothing asks git whether that path is ignored: `absent: check-ignore in: scripts/check-backlog-claims.mjs`
- **Verified 2026-08-29** - nor whether it is tracked: `absent: ls-files in: scripts/check-backlog-claims.mjs`
- **Verified 2026-08-29** - `renders/` is ignored in full, so nothing beneath it reaches a checkout: `present: ^renders/$ in: .gitignore`
- **Verified 2026-08-29** - the local call site that goes green regardless: `present: check-backlog-claims in: .githooks/pre-push`

## 0163 - `level/bass` reads exactly 1.0 on every local peak by construction, so the lighting consumer that read it as a dimmer value saw pinned dynamics, and the recorded diagnosis blamed an input gain that cannot move it

> **Filed 2026-08-30** from the 2026-08-29 live set, the first full show driven end to end by this
> app. Filed with its own originating diagnosis **falsified** - see "What the night concluded, and
> why it is wrong" below. **Owner if taken:** `architect`, and it is largely a documentation ask.

### The observation

Over the 8h08m set the `bass` term reached `1.0000` repeatedly, and the room read as flat: the
band factor in the bridge's `level = (glow + depth * band) * master` sat at its ceiling, so `depth`
stopped modulating and every fixture ran at `glow + depth` until the release curve pulled it down.
The operator reached for master and the mic gain, neither of which is the lever.

### The mechanism, and why the gain is not the lever

`bass` is levelled by `PeakNormalizer` (ADR-0049): instant attack, a 2.5 s exponential release, and
the reading is `(clean / p).clamp(0.0, 1.0)` against **its own** running peak. So `bass == 1.0` does
not mean "loud" and does not mean "clipped". It means **this hop is the loudest bass since the peak
last released** - which on four-on-the-floor material is *every kick*, by design, at any input
level. At 120 BPM the peak is re-adopted every 0.5 s against a 2.5 s release, so the term spends
most of its life in the top of its range and touches its ceiling once per kick.

**The reading is scale-invariant.** `raw / peak` is unchanged when the input is halved, because both
terms halve. Gain portability is the entire purpose of ADR-0049 - `> 0.5` is meant to mean the same
thing on every track at every gain setting - so turning the mic down, or up, moves this value by
exactly nothing. That property is a feature everywhere else in the project and is the specific trap
for a lighting consumer, which wants a level and is handed a normalized excitation.

### What the night concluded, and why it is wrong

The finding was recorded as *"the mic level was hot enough to saturate the band term"*, with the
implied repair being input gain. The code above falsifies that: no input gain reaches this number.
The entry is filed with the correction attached rather than the symptom alone, because the wrong
diagnosis is the expensive part - it sends the next operator to a control that provably cannot help,
during a show, which is exactly when there is no time to discover that.

### What is actually missing

**Nothing is broken in the analyzer, and the information the consumer needed was already on the
wire.** `/lmv/v1/raw/bass` is published beside `/lmv/v1/level/bass` and is the absolute twin;
`README.md` documents both. What no surface anywhere states is the property above - that `level/*`
is *designed* to touch 1.0 regularly on periodic material - and the word "level" invites precisely
the reading that failed here. A consumer picking a term off the telemetry table has no way to learn
this short of reading `core/src/dsp/gain.rs`.

There is also no live surface for it. Clamp occupancy (ADR-0062) is the project's saturation
instrument, and it is **capture-time only**: `core/tests/saturation.rs` and the `occ` column of
`shot --report`, both walking `clamp()` nodes in an embedded preset. It answers a different
question - "is this authored expression a constant?" - and nothing equivalent runs while the app is
performing. This half is smaller than it looks now that the mechanism is understood, because the
number an operator would want on screen is not occupancy but headroom, and `raw/*` is already it.

### Impact

Low, and confined to consumers that read a band term as a magnitude. It reached the room only
because a lighting look is the first consumer this project has had that multiplies a band term into
a physical output with no further shaping. **[Plan 0133](plans/0133-the-engine-drives-the-lights.md)
brings that look in-house in the same expression grammar and will meet this on its first evening**,
which is the reason to have it written down before that plan is built rather than after.

### What a fix looks like

The cheap and probably sufficient shape is prose: one sentence in `README.md`'s telemetry table
saying what `level/*` is normalized against and that it reaches 1.0 on every local peak, and the
same note wherever Plan 0133's look grammar names its bindable terms. Beyond that there is a real
design question that this entry does **not** answer - whether a lighting consumer wants a
differently-shaped term (a slower level, a headroom reading, or `raw/*` scaled by an operator
control) - and that is an ADR if it is ever wanted, not a patch.

- **Verified 2026-08-30** - the band scalar is a ratio against its own running peak, hence invariant under input gain: `present: \(clean / p\)\.clamp\(0\.0, 1\.0\) in: core/src/dsp/gain.rs`
- **Verified 2026-08-30** - the release is seconds-scale, so the ceiling is re-touched every kick rather than once a set: `present: RELEASE_TAU_SECS: f32 = 2\.5 in: core/src/dsp/gain.rs`
- **Verified 2026-08-30** - `bass` is levelled by that normalizer at the published frame boundary: `present: bass_gain\.normalize in: core/src/dsp/mod.rs`
- **Verified 2026-08-30** - the absolute twin the consumer needed is already published: `present: "/lmv/v1/raw/bass" in: standalone/src/osc.rs`
- **Verified 2026-08-30** - and already documented, which is why this entry is about the missing property rather than a missing address: `present: /lmv/v1/raw/bass in: README.md`
- `unprobeable:` that no surface states `level/*` reaches 1.0 by design is a negative about prose across `README.md`, `docs/` and the OSC table, not a match countable in one file
- **PROMOTED 2026-09-01 -> [Plan 0147](plans/0147-what-the-show-costs-and-what-its-numbers-mean.md) Phase 1**, as the documentation ask this entry says it is. The
  phase lands first in that plan because [Plan 0133](plans/0133-the-engine-drives-the-lights.md) is approved and meets this on its first evening.

## 0164 - the operator console halves the output's frame rate, and two comments say it cannot

> **Filed 2026-08-30** at Plan 0131's close, out of that plan's own Phase 6, which names
> "it costs the output frames" as a valid outcome and routes it here rather than tuning it away.

Two 95 s release runs differing only by `--console`, both hands-off, `[console] enabled` reset to
false first so the closed arm was genuinely closed:

| | closed | open |
|---|---|---|
| mean fps over 18 samples | **61.7** | **33.1** |
| `frame_ms_p99_steady` | **18.6 ms** | **47.3 ms** |
| frames over the same 90 s | 5,549 | 2,976 |

**+29 ms per frame is far more than a full-frame copy plus a 900x640 blit accounts for**, and
landing within 3 % of exactly half is the shape of two presents serialising rather than of copy
cost. The console's present mode was confirmed as `Mailbox` from the diagnostic note it writes on
open, so the **non-blocking arm was taken** and the halving happened with it, not in the `Fifo`
fallback. That is what convicts ADR-0143's stated cadence property: an independent encoder, submit
and present is not an independent frame loop.

**Two levers are visible in the diff and neither has been tried.** The console swapchain is
configured with `desired_maximum_frame_latency = 1`, so its `get_current_texture` waits for its own
previous present to retire - one vblank - while the output's own present waits for another; two
vblanks per frame is exactly the halving. And `present_console` runs synchronously in the display
loop on every frame, with no decimation, which is the explicit remedy Plan 0131 Phase 6 asks for a
verdict on.

**Two comments state the property the measurement denies**, which is the half of this entry that is
a defect rather than a design question. `standalone/src/main.rs` says the console present is placed
after the show's "never before it and never inside it: the console is a monitor and must not delay
the frame it reports on" - being after this frame's present does not stop it delaying the next one.
`core/src/render/aux_target.rs` says "a console that stalls or drops a frame cannot alter what the
show displays". What actually holds is narrower and worth saying instead: the show's **pixels** are
unaffected, asserted byte-exactly; its **cadence** is not, measured at ~2x.

**What bounds the reading.** It is the **integrated** GPU (see 0165), so the absolute cost is not the
discrete GPU's; and both surfaces were on **one display at one refresh rate**, which is precisely the
configuration Plan 0131 Phase 6 says cannot separate the two pacing sources. **The measurement says
the cost is real and large; it does not say the present mode is the mechanism.** The cross-refresh
two-display run that would name it is still owed and is on the checklist.

### What a fix looks like

Raise the console's frame latency, or decimate its present to every Nth output frame, or both, and
re-measure on the same two arms - the instrument already exists and costs 3 minutes. If neither
lever moves it, the mechanism is elsewhere and the next thing to try is presenting the console off
the display thread, which is a real design change and an ADR. Whichever lands, the two comments
above are corrected to the property that survives.

- **Verified 2026-08-30** - the console swapchain still asks for a single in-flight image: `present: desired_maximum_frame_latency = 1 in: core/src/render/aux_target.rs`
- **Verified 2026-08-30** - the console still presents synchronously in the display loop, undecimated: `present: self\.present_console\(\) in: standalone/src/main.rs`
- **Verified 2026-08-30** - the comment that denies the cost is still there: `present: must not delay the frame it reports on in: standalone/src/main.rs`
- **Verified 2026-08-30** - and so is its twin in the core: `present: cannot alter what the show displays in: core/src/render/aux_target.rs`
- **Verified 2026-08-30** - the non-blocking arm the design rests on is the one that ran: `present: AuxPresentMode::NonBlocking\("Mailbox"\) in: core/src/render/aux_target.rs`
- **PROMOTED 2026-09-01 -> [Plan 0147](plans/0147-what-the-show-costs-and-what-its-numbers-mean.md) Phases 3-5.** Both levers become reachable, a hands-off window measures
  four arms, and whichever verdict arrives sets the defaults. **The two false comments are corrected
  either way** - the plan is explicit that a fix is conditional and the claim repair is not.

## 0165 - the windowed app cannot ask for the discrete GPU, so every windowed frame-time figure this project has quoted is an integrated-GPU figure

> **Filed 2026-08-30** at Plan 0131's close. Found by Phase 6, which requires a frame-rate figure to
> name the GPU that produced it (ADR-0071) - and the window had no equivalent of the stream mode's
> startup print, so the phase could not have been satisfied without adding one.

The windowed path builds its adapter with `request_adapter` against a `compatible_surface` and a
**default power preference**, which on a hybrid laptop hands back the power-saving GPU.
`RenderContext::new` takes no adapter choice at all: [ADR-0146](adrs/0146-one-name-selects-the-gpu-and-each-side-matches-its-own-roster.md)
gave `--gpu` to `--stream` and to the Spout sender, and there is no windowed equivalent.

Measured on the dev box, which has both an RTX 3080 and integrated Radeon graphics:

```
# renderer adapter: AMD Radeon(TM) Graphics (Dx12, IntegratedGpu), driver 30.0.13002.1001
```

**Every windowed frame-time number this project has ever recorded on this machine is therefore an
iGPU number** - the NFR 1 checks, the soak runs, the tier calibration readings, Plan 0131's own
console measurement - and until this note was added nothing said so. The numbers are not wrong; they
are attributed to a machine rather than to the adapter inside it, which is exactly what ADR-0071
exists to stop.

**This is also why Plan 0131's dual-GPU degrade path has still never been exercised.** `open_console`
treats an `attach_aux` failure as non-fatal, logs it and leaves the show untouched, and that branch
is unreachable on a single-adapter run - which every run here is, because the window cannot be put on
the other GPU.

### What a fix looks like

Extend `--gpu` to the windowed path: `RenderContext::new` takes the same `AdapterChoice` the stream
mode already resolves, matched against the same roster, with the same
[ADR-0146](adrs/0146-one-name-selects-the-gpu-and-each-side-matches-its-own-roster.md) name rule.
That is a small change with one real question attached - whether a windowed surface can be created
on an adapter that does not drive the display it is on, which is the same dual-GPU question
0131 Phase 6 is still owed - so it wants measuring before it is promised. The startup note that names
the running adapter has already landed and is what makes any of this attributable.

> **Updated 2026-08-31, at Plan 0144's close. Half discharged: the lever landed, the measurement did
> not.** [ADR-0155](adrs/0155-the-window-takes-the-adapter-and-the-preset-the-operator-names.md) gave
> `--gpu` to the windowed path exactly as the fix above describes, and it was observed working —
> `lmv --gpu 1` put this box's window on `NVIDIA GeForce RTX 3080 Laptop GPU (Dx12, DiscreteGpu)`
> with the startup line reading `(pinned by --gpu)`. The dual-GPU question this entry said *"wants
> measuring before it is promised"* is answered for the surface-creation half: a named adapter that
> cannot present is refused by name rather than silently swapped.
>
> **What is left is this entry's actual title.** No windowed frame-time figure has been re-taken on
> the discrete adapter, so every published one is still an iGPU figure — and that is now true *by
> choice* rather than by impossibility, because Plan 0144 deliberately left the unflagged request at
> `AdapterChoice::Default` so no existing number would move underneath a CLI change. The remaining
> work is a measurement pass producing a **new row** beside the iGPU numbers, not a correction to
> them. The first probe below is rewritten because Phase 2 falsified its reduction, not its claim.

- **Re-written 2026-08-31** - the constructor now takes the choice, so the old reduction is dead; what stands is that the window still *asks* for the default when unflagged: `present: None => AdapterChoice::Default in: standalone/src/gpu.rs`
- **Verified 2026-08-30** - and the code's own doc says what the default yields on a hybrid box: `present: the power-saving GPU for a console process in: core/src/render/context.rs`
- **Verified 2026-08-31** - the two unflagged arms are held apart, which is what keeps the published figures comparable: `present: fn the_window_and_the_stream_disagree_when_unflagged in: standalone/src/gpu.rs`
- **Verified 2026-08-30** - the startup note that makes a figure attributable exists: `present: renderer adapter in: standalone/src/main.rs`
- **Verified 2026-08-30** - the console's degrade branch is still built and still unreachable here: `present: console surface unavailable on this adapter in: standalone/src/main.rs`
- **PARTLY PROMOTED 2026-09-01 -> [Plan 0147](plans/0147-what-the-show-costs-and-what-its-numbers-mean.md) Phase 6**, which takes the measurement half: a new windowed
  frame-time row naming the discrete adapter, beside the iGPU figures rather than replacing them. The
  phase also records whether the console's dual-GPU degrade path became reachable; **if it stays
  unexercised this entry keeps that half and stays live.**

## 0166 - the index-row gate measures a row's bytes and never its shape, so a closed-plan bullet dropped into the active-plans table passes

> **Filed 2026-08-30** at Plan 0134's close, by the reviewer making the mistake and having the gate
> wave it through. Not reported by a lane - found by inspection after the fact.

`scripts/check-index-rows.mjs` enforces [ADR-0116](adrs/0116-an-index-row-is-a-pointer-and-a-gate-holds-it-to-one.md):
every row inside a `<!-- roster:begin cap=N -->` region is a pointer, held to N bytes. **Bytes is
the only thing it measures.** Inside a region it accepts a line matching either `TABLE_ROW` or
`BULLET`, records `{ line, bytes, cap }`, and asks nothing about which of the two the surrounding
region is made of.

`docs/plans/README.md` has two regions and they are different kinds: the **active-plans** region is
a markdown table (`| Plan | Title | Status | Owner | Live constraint |`), the **recently-closed**
region is a bullet list. A closed-plan bullet inserted into the table region is a `BULLET` inside a
region, under cap, so the gate reports `0 over cap` and exits 0.

**Observed 2026-08-30**, during this plan's own close: a `- [0134 ...] - closed 2026-08-30 ...`
bullet landed immediately under the active roster's `roster:begin` and above its table header,
because the insertion anchored on the first `Recently closed` string in the file rather than the
section. `check-index-rows.mjs` passed it (`2 regions, 117 rows, 0 over cap`) and
`check-doc-links.mjs` passed it too - the link resolves, it just points from the wrong list. It was
caught by eye before the commit, and nothing in the toolchain would have caught it after.

**Impact.** Low severity, high silence. The failure mode is a closed plan that reads as active, or
an active plan that reads as closed, in the one file `docs/plans/README.md` that every session opens
first - and both rosters keep passing. It is the same class as the rot ADR-0116 was written for: the
convention is stated in prose above the markers and nothing holds anyone to it.

**What a fix looks like:** infer each region's kind from its first measured row and reject rows of
the other kind - roughly four lines, in the loop that already classifies every line as `TABLE_ROW`
or `BULLET`. A region containing both is a real possibility elsewhere, so the honest form is
per-region and inferred rather than a hardcoded expectation per file. `scripts/fixtures/` already
holds seeded bite checks for these gates, so the fix comes with a fixture that is a bullet in a
table region.

- **Verified 2026-08-30** - the gate accepts either row shape anywhere inside a region: `present: !TABLE_ROW\.test\(line\) && !BULLET\.test\(line\) in: scripts/check-index-rows.mjs`
- **Verified 2026-08-30** - and the only thing it records per row is the byte count: `present: bytes: Buffer\.byteLength\(line, "utf8"\) in: scripts/check-index-rows.mjs`
- **Verified 2026-08-30** - the two regions in the plans index really are different kinds, which is what makes the confusion reachable: `present: \| Plan \| Title \| Status \| Owner \| Live constraint \| in: docs/plans/README.md`
- **PROMOTED 2026-09-01 -> [Plan 0136](plans/0136-the-gates-can-convict.md) Phase 3** (added at that plan's 2026-09-01 amendment). A shape check
  lands **beside** the length check, not instead of it: a 200-byte bullet in a table region is under cap
  and still wrong.

## 0170 - the comment-hygiene gate walks the filesystem, so a gitignored vendored tree is invisible to CI and blocks every local push

> **Filed 2026-08-30** while pushing the Plan 0134 close. Found because the gate went from green
> to 490 findings between two pushes twenty minutes apart, with no commit in between touching it.

`scripts/check-comment-hygiene.mjs` enumerates with `readdirSync` from the repo root and skips a
hardcoded `SKIP_DIRS` set. **It never asks git what is tracked.** A gitignored directory is
therefore absent from CI's fresh clone - so the CI job is green by construction - and present in
every working tree, where it is scanned in full.

Two such trees exist here, both third-party, both gitignored, neither ours to judge:

- `.venv/` (`.gitignore:68`) - the Python virtualenv `tools/sd-filter` installs into. Its
  `site-packages` ship torch, numpy and markupsafe C and C++ headers: **419 findings**, all
  `plan-relative narration` on words like `used to` and `no longer` in vendor comments.
- `plugin-foobar/sdk/` (`.gitignore:15`) - the foobar2000 SDK. **71 findings**, same class.

**Impact.** It blocks `git push` outright, for everyone whose working tree has either directory,
with a diagnostic pointing at files no one in this project wrote. The natural escape is
`--no-verify`, which is what makes it worth recording: a gate that fires on vendor code teaches its
users to skip the gate that fires on theirs. The immediate instances are patched by name at Plan
0134's close (`SKIP_DIRS` gained `.venv`, a new `VENDORED_TREES` holds the SDK path), which fixes
these two and not the class - the next `pip install` or unpacked SDK re-breaks it.

**What a fix looks like:** enumerate from `git ls-files` rather than `readdirSync`, which makes
"code we own" and "code the gate judges" the same set by construction and costs one call. The one
thing to preserve is the seeded bite check - `node scripts/check-comment-hygiene.mjs scripts/fixtures`
must still report its 10 findings, and those fixtures are tracked, so `ls-files` reaches them. Worth
checking whether the sibling gates share the shape: `check-doc-links.mjs` walks markdown the same
way and is green today only because neither vendored tree happens to carry a relative-linked `.md`.

- **Verified 2026-08-30** - the walk is a filesystem walk with no git in it: `present: readdirSync\(dir, \{ withFileTypes: true \}\) in: scripts/check-comment-hygiene.mjs`
- **Verified 2026-08-30** - nothing consults git's ignore rules: `absent: check-ignore in: scripts/check-comment-hygiene.mjs`
- **Verified 2026-08-30** - the by-name patch this entry says is not the fix is present: `present: const VENDORED_TREES = new Set in: scripts/check-comment-hygiene.mjs`
- **Verified 2026-08-30** - and both ignore rules that make the trees invisible to CI still stand: `present: plugin-foobar/sdk/ in: .gitignore`
- **PROMOTED 2026-09-01 -> [Plan 0136](plans/0136-the-gates-can-convict.md) Phase 7** (added at that plan's 2026-09-01 amendment), taking the
  `git ls-files` enumeration this entry names as the fix, and **removing the by-name patches** Plan 0134's
  close added. The sibling question this entry raises about `check-doc-links.mjs` is in that phase's
  done-when, and silence on it is not an answer.


## 0171 - a backlog probe about a run of spaces is collapsed to one space before it is matched, so it can never fire

> **Filed 2026-08-31** at Plan 0144's review, by moving 0168 into the probed section and watching
> the gate hand its own probe back with the spaces gone.

`scripts/check-backlog-claims.mjs:225` extracts each probe from its bullet with

```js
.map((m) => m[1].replace(/\s+/g, " ").trim())
```

which is right for the reason it was written - a markdown bullet may wrap across source lines, and
the pattern has to survive that - and wrong for one class of claim. **A probe whose regex contains a
run of two or more spaces is silently rewritten into a different regex**, one that matches
single-spaced text the tree does not contain. It does not error, it does not warn; it reports `no
match` and reads exactly like decay.

**The instance, which is the reason this is filed rather than noticed.** Entry 0168 is *about* the
broken-literal defect - a string carrying a run of spaces mid-sentence - and both of its probes
quoted the run verbatim:

```
present: cannot be longer      than it in: core/src/dsp/mod.rs
present: is not a flag, but the                  embedded set in: standalone/src/stream.rs
```

Neither has ever been capable of matching. That went unseen for a second reason - 0168 sat above
`## Open entries`, where nothing probes at all - so the two defects hid each other, and the entry
looked green by being unread. Both probes now use `\s{2,}`, which carries the same claim and holds
no space character to collapse.

**Impact.** Low frequency, and bounded: after 0168's repair no live probe contains a space run.
It is filed because the failure is silent in the direction that matters - an author writes the
claim they mean, the gate accepts the bullet, and the probe is dead on arrival. ADR-0108's whole
argument is that a claim about the repo carries something re-runnable; a probe that cannot fire is
the one shape that satisfies the letter of that and none of it.

**What a fix looks like.** Two options, and the cheap one is probably right. **Refuse it:** after
collapsing, if the extracted span differs from the source span, print the entry and the probe and
exit non-zero - the author is told to use `\s{2,}` and nothing is silently rewritten. Roughly five
lines, and it converts an unfalsifiable probe into a build error the moment it is written. **Or
preserve it:** join wrapped bullet lines with a single space but leave interior runs alone, which
is more faithful and more code, and needs care where a bullet wraps mid-pattern. The gate's own
grammar note already says *"Regex source, so a literal dot needs escaping"*; whichever way this
lands, it should say the same about a space.

- **Verified 2026-08-31** - the collapse is still unconditional and still happens before the verb is read: `present: replace\(/\\s\+/g, " "\) in: scripts/check-backlog-claims.mjs`
- **Verified 2026-08-31** - and nothing tells the author it happened: there is no diagnostic on the rewrite, which is the whole of why the probe reads as decay rather than as a mistake: `absent: collapse in: scripts/check-backlog-claims.mjs`
- **Verified 2026-08-31** - the grammar note tells an author to escape a dot and says nothing about a space: `present: a literal dot needs escaping in: scripts/check-backlog-claims.mjs`
- **PROMOTED 2026-09-01 -> [Plan 0136](plans/0136-the-gates-can-convict.md) Phase 5** (added at that plan's 2026-09-01 amendment), as a done-when
  on the phase that already opens `check-backlog-claims.mjs`. The wrap must still be absorbed; what stops
  is a run of spaces inside a pattern being collapsed - proved by a fixture probe that **fires**.

## 0172 - the seeded preset directory is never pruned, so an operator's roster drifts from the shipped set and can hold two presets under one name

> **Filed 2026-08-31** at Plan 0144's review. Surfaced because Phase 3 gave the window a
> `--preset <name>` that matches the display name exactly, which makes the drift operator-visible
> for the first time.

`preset::seed_dir` writes an embedded preset into the per-user directory only when the file is not
already there, and **removes nothing**. That is the correct rule for the problem it solves - it must
never overwrite an operator's edits - but it has no counterpart. A preset that is renamed, retired
or re-slugged upstream is written once and then stays in that directory for the life of the install,
while the new file arrives beside it on the next launch.

**What that adds up to on a machine that has tracked this project for a while.** The shipped set is
81 presets with no duplicate display name. The development box's own
`%APPDATA%\light-music-visualizer\presets` holds **118**, and two of them are named `Coral`.
`Renderer::select_preset_by_name` takes the first exact match, so the second is unreachable by name
from the window, from `--stream`, and from anything else that selects by name - while both still
appear in the browse overlay and both still take a turn in the rotation.

**Why it is worth recording rather than shrugging at.** Nothing is broken and no gate can see this:
the repo is clean by construction, and the drift lives entirely in a directory that exists in no
checkout. Every preset-set judgement this project makes - the curation sweep at a plan close,
`shot --presets presets --report`, the distinctness gate - reads `presets/`, which is the set an
operator with a fresh install has and nobody who has been here a while does. The reachability loss
is the sharp end; the general form is that the thing being demonstrated and the thing shipped are
different sets, and no instrument reports the difference.

**What a fix looks like.** Not a prune - deleting from a directory the operator is invited to edit
is the wrong default and would eat their work. The honest options are to **report** rather than
repair: seeding already prints `seeded {n} curated preset(s)`, and the same pass knows which files
in the directory are not in the embedded set and whether any display name is claimed twice. One
extra line at startup naming both counts would make the drift visible to the person who can decide
about it. A `--list-presets` flag - the one `stream.rs`'s own error message says is not a flag -
would serve the same end deliberately rather than as a side effect.

- **Verified 2026-08-31** - seeding is write-if-absent and has no removal arm: `present: if !path.exists\(\) in: core/src/preset/mod.rs`
- **Verified 2026-08-31** - selection by name is a first-exact-match, so a duplicate name makes one preset unreachable: `present: position\(\|n\| n == name\) in: core/src/render/mod.rs`
- **Verified 2026-08-31** - `unprobeable: the drift itself is a property of a machine's %APPDATA% preset directory, which exists in no checkout - presets/ is clean by construction and a probe against it would pass forever while saying nothing about the condition`

## 0173 - the literal gate is blind to the defect in its unrejoined form, and the fixture README states that silence as a general truth

> **Filed 2026-08-31** at Plan 0144's review, by seeding a two-line literal under
> `scripts/fixtures/` and watching the gate stay silent.

Plan 0144 Phase 4 gave `scripts/check-comment-hygiene.mjs` a string-literal pass, which is what
closed 0168. `brokenLiteral` decodes each literal the way rustc does and then **returns `null` for
any literal whose decoded text still contains a newline**, on the stated grounds that such a literal
is a formatted block whose column spacing is layout the author typed - *"prose does not carry a
newline in the middle of itself"*.

**A lost `\` continuation is prose carrying a newline in the middle of itself.** That is the whole
mechanism: the newline survives, the next line's indent survives, and the reader gets a run of
spaces mid-sentence. So the gate catches the defect only *after* someone joins the lines, and is
silent on it in the form an author actually types. The finding message it prints - `(a line break
with no trailing \)` - names the shape it structurally cannot see.

**Why this is low and not a re-opening of 0168.** Every instance this tree has held arrived already
single-line: `core/src/dsp/mod.rs:57`, `standalone/src/stream.rs:393` and `milkconv/src/convert.rs:430`
were all one source line with the run baked in, which is why the 12-space rule convicted 15 sites and
`cargo fmt` never disturbed them. The authoring path that produces this defect here emits the joined
form. The gap is real and the gate's own documentation overstates itself; the frequency is not known
to be nonzero.

**What a fix looks like.** One more arm, not a rewrite: alongside the single-line run, report a
**newline immediately followed by 12 or more spaces and then a non-space** - which is precisely a
continuation indent and is not what a column table looks like, since a table's rows start at a
column and carry interior runs rather than a leading one. Seed both spellings. Failing that, the
honest minimum is to correct `scripts/fixtures/README.md`'s silence row and the `LITERAL_RUN`
comment so neither claims a generality the check does not have.

**Also here, because it is one line of the same file.** The finding message's `\` is written `\)`
inside a JS template literal, so it is consumed as an escape and the operator reads `(a line break
with no trailing )`. It needs `\`.

**Impact:** low. Nothing is mis-reported; a class is under-reported, and the documentation says
otherwise, which is the property that gets the next reader to stop looking.

- **Verified 2026-08-31** - the newline exclusion is still unconditional and still ahead of the run check, so the unrejoined form still returns before it is judged: `present: text\.includes.{0,40}return null in: scripts/check-comment-hygiene.mjs`
- **Verified 2026-08-31** - the fixture roster still states the silence as a general property: `present: prose does not carry one mid-sentence in: scripts/fixtures/README.md`
- **Verified 2026-08-31** - the swallowed backslash is still in the message the operator reads: `present: with no trailing \\\) in: scripts/check-comment-hygiene.mjs`
- **PROMOTED 2026-09-01 -> [Plan 0136](plans/0136-the-gates-can-convict.md) Phase 7** (added at that plan's 2026-09-01 amendment), which takes
  the unrejoined form **and** the fixture README sentence that states the gate's blindness as a general
  truth.

---

## 0174 — two of the four colour tags the render path pins do not survive into the container, and the doc calls all four the half most likely to ship wrong

- **Raised:** 2026-09-01, by `dev` during [Plan 0139](plans/done/0139-the-render-path-validates-before-it-spends.md)
  Phase 2, reported and deliberately not acted on.
- **Verified 2026-09-01** — the tags are still emitted unconditionally:
  `present: color_primaries in: standalone/src/shot/render.rs`
- **Verified 2026-09-01** — and nothing reads back what the produced file actually carries:
  `absent: color_primaries in: standalone/tests/shot_cli.rs`

**Observed.** `ffprobe` reads a file produced by the generated command line as
**`bt709/unknown/unknown`**. `-color_trc bt709` and `-color_primaries bt709` are both on that
command line. It was identical on both arms of Phase 2's `--crf` comparison, so it is a property of
the shipped invocation and not of the new flag.

**Why it matters.** `docs/capturing.md` states that *“the four colour tags are the half most likely
to ship wrong — an untagged file is one the player expands from studio swing and shows washed
out”*, and the same paragraph argues `-color_trc bt709` over `iec61966-2-1` on the ground that
*“every player assumes the former”*. If two of the four do not reach the container, that reasoning
is being applied to arguments that have no effect, and the doc is asserting a guarantee the artifact
does not carry. Nothing here is known to be *wrong* on screen — the range tag is the one that
produces the washed-out failure and it does survive — but the claim is stated as a property and is
not verified as one.

**What a fix looks like, in order.** (a) Establish what is actually true: `ffprobe -show_streams` on
a produced file, against what the same `ffmpeg` writes when the tags are passed as output-side
`-bsf:v h264_metadata` or via `-color_primaries` placed after `-c:v`. Argument *position* is the
first suspect — colour options ahead of the output codec can bind to the input. (b) If they are
being dropped, move them and assert the readback in `standalone/tests/shot_cli.rs`, which already
gates on `ffmpeg_on_path()`. (c) If they cannot survive H.264-in-MP4 the way this command writes
them, correct the paragraph rather than the command.

**Impact:** low-medium. No reported visual defect; a documented guarantee that is not checked, on
the path [Plan 0103](plans/0103-the-project-gets-an-audience.md) publishes from. **No ADR needed.**
- **PROMOTED 2026-09-01 -> [Plan 0148](plans/0148-the-shipped-artifacts-carry-their-own-guarantees.md) Phase 3**, whose done-when is written around *establishing what is
  true* and admits both repairs this entry names - move the arguments, or correct the paragraph.

---

## 0175 — the render path's spend-nothing ordering is a structural property with no end-to-end guard

- **Raised:** 2026-09-01, by `architect` at [Plan 0139](plans/done/0139-the-render-path-validates-before-it-spends.md)'s
  close review.
- **Verified 2026-09-01** — no test in the CLI suite drives `--render` with a name the roster does
  not hold: `absent: unknown preset in: standalone/tests/shot_cli.rs`

**The claim nothing checks.** Plan 0139 Phase 1's done-when is that a misspelt `--preset` *“exits 1
naming the roster's keys, spawns no child process, builds no GPU device, and leaves **nothing** at
`<path>`”*. The first clause is asserted; the last three are not. `resolve_preset` is tested as a
pure function, and it returns the same answer whether its call site sits before or after
`Encoder::spawn` — so the ordering that is the entire defect (a valid, playable, 262-byte audio-only
MP4 left at the destination) is held up by nothing but the current line order in `render::run()`.
Plan 0139's own risks section warns against *“merging it into a larger refactor of
`render::run()`”*; that refactor reintroduces the artifact with a green suite.

**What a fix looks like.** About ten lines in `standalone/tests/shot_cli.rs`'s existing `--render`
section, using helpers already in that file — `render_clip()`, `run()`, `scratch()`,
`assert_failed_naming()`. Drive `--preset attractor_leviathan` (the reproduction: a filename against
a roster keyed on `name`) with `--ffmpeg no_such_encoder_binary --out <scratch>/nothing.mp4`, then
assert stderr names `Leviathan`, does **not** name `--ffmpeg`, and that the output path does not
exist. It needs no real encoder: if the ordering regresses, the spawn failure arrives first and the
assertion on the roster keys fails. `a_missing_encoder_names_the_flag_rather_than_falling_back`
asserts the mirror property of the same ordering and is the template.

**Also here, because it is the same file and the same shape.** Plan 0139 Phase 2 added two
cross-flag rejections — `--crf` without `--ffmpeg`, and `--crf` outside `--render` — and neither is
covered. They live in `parse_args`, which reads `std::env::args` and is reachable only through the
binary. Their `--ffmpeg` siblings are asserted three lines apart in that same test.

**Impact:** medium. The shipped behaviour is correct; what is missing is the guard on the one
property the plan exists to hold. **No ADR needed.**
- **PROMOTED 2026-09-01 -> [Plan 0148](plans/0148-the-shipped-artifacts-carry-their-own-guarantees.md) Phase 1**, using this entry's own reproduction and helper list.

---

## 0176 — `shot`'s usage text and its parser can drift with nothing checking, and the other CLI in this repo has exactly that guard

- **Raised:** 2026-09-01, by `architect` at [Plan 0139](plans/done/0139-the-render-path-validates-before-it-spends.md)'s
  close review, from noticing `--crf` reached `print_usage()` by hand.
- **Verified 2026-09-01** — nothing in the CLI suite reaches the usage text:
  `absent: print_usage in: standalone/tests/shot_cli.rs`
- **Verified 2026-09-01** — while the `lmv` binary holds precisely this property:
  `present: fn the_help_text_prints_every_rostered_flag in: standalone/src/main.rs`

**The asymmetry.** [Plan 0144](plans/done/0144-the-flags-mean-what-they-say.md) built a flag roster
for the `lmv` binary and two tests over it — `every_scanner_flag_literal_is_rostered` and
`the_help_text_prints_every_rostered_flag` — so that binary's help cannot fall behind its scanner.
`shot` has the same failure mode, no roster and no test. Its flags are matched in one `match` arm
each and re-typed by hand into `print_usage()` and again into `docs/capturing.md`'s flag table.
Plan 0139 added `--crf` to all three correctly; nothing would have reported it if it had not.

**Why it matters here specifically.** `shot` is the CLI the `preset-author` lane drives, and that
lane has no other way to discover a flag — `CLAUDE.md` routes it to `docs/capturing.md`, and the
guide's flag table is transcribed from the usage text nobody checks. A flag that exists and is
undocumented is invisible to the only consumer that needs it.

**What a fix looks like.** The cheap half is one test asserting `--help`'s output contains every
literal the parser's `match` arms accept, extracted the way `every_scanner_flag_literal_is_rostered`
extracts them. The expensive half — a shared roster type both binaries build from — is not obviously
worth it for two CLIs and is not proposed.

**Impact:** low. No known drift today; the property Plan 0144 decided was worth holding for one
binary is unheld for the other. **No ADR needed.**
- **PROMOTED 2026-09-01 -> [Plan 0148](plans/0148-the-shipped-artifacts-carry-their-own-guarantees.md) Phase 2**, taking the cheap half only - one test that `--help` prints
  every literal the parser accepts. **The shared roster type this entry declines is declined there too.**

---

## 0177 — the component's size cap has a trigger and no carrier: the recipe builds the DLL and never reads its own output's length

**Raised by:** `architect`, from [Plan 0141](plans/done/0141-the-plugin-seams-stop-drifting.md)'s
close review (2026-09-01). **Owner if taken:** `dev`.

- **Verified 2026-09-01** — the recipe that produces the shipped DLL knows nothing about the cap it
  is measured against: `absent: soft cap in: packaging/foobar/build-component.ps1`

### The finding

Plan 0141 Phase 2 replaced a re-measure trigger that could not fire (*"when a dependency is added to
this crate"* — the growth arrived as code behind the ABI, with no new crate) with one that always
fires: **at every release**. That is a real improvement, and it is still a duty a person performs
from memory.

`packaging/foobar/build-component.ps1` produces `foo_lmv.dll`, then runs seven fatal checks over it
— it is an x64 PE, it exports `foobar2000_get_interface`, it carries the workspace version, the
archive holds exactly one file. It parses the PE headers by hand to do this. **It never reads the
file's length**, which is the cheapest fact about the artifact and the only one NFR §4 constrains.

So the series in [`docs/specs/0001-c-abi.md`](specs/0001-c-abi.md) is only as current as the last
person who remembered to look, and the history says that is not often: the component grew
**+910,848 B across Plan 0097 to Plan 0141** and every byte of it was noticed retroactively, twice,
by a reviewer rather than by the build.

### What a fix would be

One `Check` line in the recipe printing the DLL's size, and a **warning** — not a `Die` — when it is
within some margin of NFR §4's cap. The cap is soft and the recipe must not start failing releases
over a number the NFR writes with a tilde; the point is that the figure appears in the release log
where a human already reads output, rather than in a spec nobody opens to cut a tag.

The open question, and the reason this is filed rather than done: **the recipe has no cap constant
today, and giving it one imports NFR §4's ambiguity** — "~10 MB" names no unit, its subject is the
standalone exe, and the plugin is covered only by *"in the same ballpark"*. Deciding what the
plugin's own cap is, and whether it is 10,000,000 or 10,485,760, is an NFR question that should be
settled before a script starts asserting on it.

### Priority

**Low.** Nothing is over cap and the trigger now at least fires on an event that happens. This is
the difference between a duty and a guard.
- **PROMOTED 2026-09-01 -> [ADR-0159](adrs/0159-the-component-gets-its-own-size-cap-and-the-recipe-carries-it.md) + [Plan 0148](plans/0148-the-shipped-artifacts-carry-their-own-guarantees.md) Phase 4.** The open question this entry filed
  rather than answered - what the plugin's own cap is - is settled by the ADR at **12,582,912 B (12 MiB)**,
  derived as today's size plus one more step the size of the `text` step. The recipe prints the length
  always and warns above 90 %; it never fails a release.

---

## 0178 — the +510,464 B the component gained after 2026-08-18 is unattributed, and it is the larger half of the growth

**Raised by:** `architect`, from [Plan 0141](plans/done/0141-the-plugin-seams-stop-drifting.md)'s
close review (2026-09-01), on `dev`'s own note that the window was outside Phase 3's scope.
**Owner if taken:** `dev`.

- **Verified 2026-09-01** — the spec records the movement and says it is unattributed:
  `present: is not attributed at all in: docs/specs/0001-c-abi.md`

### The finding

Plan 0141 Phase 3 bisected the window [backlog 0118](design-backlog-archive.md) named — Plan 0097's
close to Plan 0107's close — and attributed **98.4 %** of it to Plan 0100's MilkDrop conversion work.
That is a clean result and it closed the entry it was filed against.

It is also the **smaller** half. Phase 2's re-measure found `foo_lmv.dll` had reached 9,789,952 B on
2026-09-01, which is **+510,464 B beyond** the 2026-08-18 figure the bisect ended at — larger than
the +400,384 B that was worth filing a backlog entry over. Phase 3's scope was the window the plan
named, correctly, so this one has never been looked at.

**What makes it worth a second bisect rather than a shrug** is that the first one paid off: the
suspicion in the entry was confirmed rather than assumed, and the answer turned out to be one plan
rather than "the sum of many small things". Roughly twenty plans closed between 2026-08-18 and
2026-09-01.

### What a fix would be

The same method, and it is written down now: build only `core-cabi` at each `chore: Release` commit
in the window and read `lmv_core_c.dll`. Two things Plan 0141's bisect learned that this one should
carry:

- **Record the `rustc` version at every point.** Rebuilding `22bb460` in 2026-09 gave a number
  **13,312 B** from the one measured at that commit in 2026-08, under the same build command, and
  nothing recorded at either date can now explain the gap. That is the working noise floor for this
  column, and it makes any single step under ~13 KB uninterpretable.
- **Read the cdylib, ship the number for `foo_lmv.dll`.** The shim links the staticlib, so the
  cdylib is a proxy — fine for locating a step, not the artifact the cap is about.

If the growth is attributable and unwanted, *that* is a third entry; this one asks only what moved.

### Priority

**Low-medium.** Nothing is over cap, and the component is at 97.9 % of the decimal reading of a soft
cap written with a tilde — which is a reason to know what is in it, not a reason to panic. Cheaper
now than after another twenty plans.
- **PROMOTED 2026-09-01 -> [Plan 0148](plans/0148-the-shipped-artifacts-carry-their-own-guarantees.md) Phase 5**, carrying both method constraints this entry names: the
  `rustc` version recorded at every point against the ~13,312 B noise floor, and the cdylib read as a
  proxy while the number shipped is `foo_lmv.dll`'s.

## 0179 — `cargo doc` is the one CI gate no local step mirrors, so making an item public cannot fail until after the push

**Raised by:** `architect`, at [Plan 0137](plans/done/0137-the-metrics-measure-light.md)'s close
review (2026-09-01), from a red `main` the close ceremony's own gate list could not have caught.
**Owner if taken:** `dev`.

- **Verified 2026-09-01** — CI runs the doc gate:
  `present: RUSTDOCFLAGS in: .github/workflows/ci.yml`
- **Verified 2026-09-01** — and the hook that mirrors every other CI gate does not:
  `absent: cargo doc in: .githooks/pre-push`

### The finding

`.github/workflows/ci.yml:118` runs `cargo doc --workspace --no-deps` with
`RUSTDOCFLAGS: -D warnings`, added by Plan 0144 Phase 6. `.githooks/pre-push` runs the five Node
gates, `fmt`, `clippy --workspace --all-targets -D warnings` and a narrowed `nextest` — and no
`cargo doc`. The exclusion is **deliberate and documented** in `ci.yml`'s own comment: the hook's
budget is ~28 s and a full `cargo doc` does not fit. That reasoning is sound and this entry does not
dispute it.

What the entry is about is the **consequence nobody priced**: `cargo doc` is now the only CI gate
with no local counterpart at any cadence — not the hook, not the `dev` per-phase gate, and not the
architect close ceremony, whose written gate list is `fmt` + `clippy --workspace --all-targets` +
`cargo nextest run --workspace` + the five Node scripts. So a rustdoc error is **structurally
unreachable** until a push has already happened.

Plan 0137 is the demonstration. It made `srgb_decode_lut` public (Phase 1) and added a public
`mean_lit_level` (Phase 2). Both doc comments carried `[`linear_diff`]` and `[`luma`]` intra-doc
links, and both targets are private — which is fine for a private item and an **error** for a
public one under `-D warnings` (`rustdoc::private_intra_doc_links`). The links had been correct for
as long as `srgb_decode_lut` was private; *making it public is what turned them red*, and that is
the general shape: **the trigger is a visibility change, not a doc edit.** `dev` did not see it,
the hook could not see it, and the close review ran every gate the ceremony names and still shipped
a red `main` and a `chore: Release` tag on top of it.

Three errors, all in `core/src/render/metrics.rs`, on both `macos-latest` and `windows-latest`.
Repaired the same day by naming the two private helpers instead of linking them.

### The shape of a repair, not a decision

Two candidate cadences, and the choice between them is the design question:

- **At the close.** One line in the architect ceremony's gate list, beside the `nextest --workspace`
  it already owes once per plan. Costs ~10 s on a warm tree, catches it before the tag rather than
  after. Cheapest, and it is the cadence at which visibility actually changes.
- **In the hook, scoped.** `cargo doc -p lmv-core --no-deps` rather than `--workspace`, which is
  where every public surface in this project lives. Needs measuring against ADR-0033's budget
  before anyone claims it fits — the figure above is a warm-tree guess and nothing here measured it.

A third option worth naming only to reject: adding `#[allow(rustdoc::private_intra_doc_links)]` at
the module level. It would have made this specific failure impossible and would also have made the
next real broken link invisible.

