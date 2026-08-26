# 0129 — The structural term is measured at composition scale, not at pixel scale

> **Status:** accepted 2026-08-26 (Plan 0119) — **Decision superseded by [0130](0130-the-structural-term-is-boundary-density-and-conditioning-the-population-is-what-made-it-work.md)**;
> its corrected stop condition survives. See the dated `Outcome` below.
> **Date:** 2026-08-26
> **Related plan(s):** [0119](../plans/done/0119-the-flatness-gate-gets-its-second-term.md)
> **Completes:** [ADR-0128](0128-a-tonally-flat-picture-is-a-blot-only-if-it-is-also-structureless.md)
> — that ADR decided flatness becomes a conjunction and left the second term's *mechanism* to a
> measurement phase, which fired its stop condition. This one names why it fired and what to measure
> instead. 0128's Decision is unchanged and is not superseded.
> **Relates to:** [ADR-0126](0126-the-sanity-lens-measures-departure-from-the-frames-own-ground.md)
> (the ground this reads tone against), [ADR-0071](0071-a-numeric-test-contract-states-a-property-or-names-its-machine.md)
> (why the threshold below is a measurement and must say so),
> [ADR-0123](0123-a-flat-graphic-scene-paints-its-own-paper-and-composites-opaque-elements-in-one-pass.md)
> (the scene family that makes flat-by-construction a shipping idiom)
> **Raised from:** re-running Plan 0116 Phase 8's own instrument on 2026-08-26 at the Plan 0113 tip

## Context

[ADR-0128](0128-a-tonally-flat-picture-is-a-blot-only-if-it-is-also-structureless.md) decided that
`MAX_TONAL_FLATNESS` stops being a verdict and becomes one of two terms: a picture is a blot when it
is tonally flat **and** spatially structureless. It deliberately did not name the structural
statistic, and [Plan 0116](../plans/done/0116-the-sanity-lens-finds-the-ground.md) Phase 8 measured
three candidates against a mechanical stop condition. All three failed, Phase 9 did not run, and
`presets/pending/fragment_tiledmono.toml` is still held out of the shipped set.

Re-running that instrument
(`each_structure_candidate_is_tabled_against_the_library`, `#[ignore]`d in `core/tests/sanity.rs`)
surfaces two facts its `Outcome` does not name. Both are re-derivable from the same command.

### The three candidates are one axis chosen three times

`boundary length per unit area`, `connected components per unit area` and `Sobel density` are all
computed on the **binary lit mask**, at the suite's **96x96** capture. They differ in formula and
agree in what they ask: *how jagged is the outline of the lit set, pixel by pixel?*

At 96x96 a particle cloud's mask is **noise**, and noise is jagged. So the frozen `Blown Out` blot
scores as *structured* on every one of them — and, decisively, **above the sparsest legitimate
content in every column**:

| candidate | `Blown Out` (the blot) | sparsest legitimate | derived threshold | convicts the blot? |
|---|---|---|---|---|
| boundary | 0.2631 | 0.0440 (`Torus Knot`) | 0.0220 | no |
| components | 4.4223 | 0.1088 (`Neon Tunnel`) | 0.0544 | no |
| Sobel over the mask | 0.2199 | 0.0430 (`Neon Tunnel`) | 0.0215 | no |

That is the whole of the reported failure. The threshold ceremony this project derives constants by —
half the sparsest legitimate content — cannot help but land *under* a defect that outscores the
library's own quietest members. **This is not three unlucky choices. It is one axis, and the axis is
wrong**: pixel-scale mask geometry measures granularity, and granularity is what a blot has most of.

### The stop condition was evaluated over a population the term cannot reach

A conjunction's second term is only ever asked about frames that already failed the first. Phase 8's
two rejection criteria — *frames in the gap* and *half the sparsest legitimate content* — both ran
over the **whole** library instead.

The four frames that put `boundary` out, with the flatness that decides whether the second term ever
sees them:

| frame | boundary (in the gap) | `tonal_flatness` | reaches term two? |
|---|---|---|---|
| `Banded Mandala` | 0.2695 | 0.5262 | no |
| `On White` | 0.3064 | 0.2388 | no |
| `Mitosis` | 0.3406 | 0.1517 | no |
| `Verdigris` | 0.3487 | 0.1606 | no |

None is convictable by the conjunction at *any* boundary score. Neither is `Torus Knot`
(flatness 0.3344), whose 0.0440 set the threshold that made the gate vacuous.

**The corpus is more lopsided than that suggests.** The highest `tonal_flatness` anywhere in the
shipped library is **0.5262** (`Banded Mandala`), against a ceiling of **0.90**. Exactly two frames
in the whole measured corpus are near it: the frozen `Blown Out` at **0.9154** and the held
`Tiled Rosette Mono` at **0.9413**.

That is a correction to the method and it is also a warning: conditioned correctly, the population
this term is calibrated on has **two members**, one of which is the preset the change exists to
admit. Any second term, this ADR's included, has to face that.

## Decision

We will measure the structural term at **composition scale** rather than at pixel scale, by
quantizing the frame **spatially** before asking about structure, and we will evaluate it against a
stop condition **conditioned on the population the term can actually reach**.

The candidate is **modal-band tile transition density**:

1. Tile the capture into a grid of equal tiles.
2. Give each tile the **modal luminance band** of its own pixels, using the same 16-band binning
   [`tonal_flatness`](../../core/src/render/metrics.rs) uses.
3. The statistic is the fraction of **adjacent tile pairs whose modal bands differ**, over all
   adjacent pairs.

**The tiling is the mechanism, not an implementation detail.** It is a low-pass filter, and the
signal that defeated all three prior candidates is high-frequency. A particle blot's raggedness lives
*below* the tile and is averaged away; a tiled ornament, a stroke lattice or a suprematist
arrangement have periods *above* it and survive. The statistic asks whether tone is **arranged
across the frame**, which is the question "is this a composition or a fill?" actually is.

Two deliberate departures from the statistic it qualifies, both of which make it orthogonal rather
than a restatement:

- **Tone, not the binary mask.** The three failures threw tone away and kept shape; this keeps tone
  and throws away fine shape.
- **All pixels, not lit ones.** In a duotone the ink and the paper are *both* the composition —
  which is exactly what ADR-0128's Context demonstrates when it solves `Tiled Rosette Mono`'s frame
  from its two population shares. Restricting to lit pixels is what makes the first term structurally
  unable to see a duotone, and inheriting that would inherit the defect.

**This is not all-pixel tonal flatness**, which ADR-0128 Alternative B rejects and which stays
rejected. That statistic is a *share of a population* and passes an additive blot on a black frame at
`0.70` because the background dominates. This is a *spatial arrangement*, and the same blot reads
**low** on it: one contiguous region of mass-band tiles, one of background-band tiles, and few
transitions between them.

**This ADR names neither the tile size nor the threshold.** Plan 0119 measures both, and the
precedent for insisting is two ADRs deep: ADR-0126 named a diagnosis without measuring it and Plan
0116 Phase 1 falsified it; ADR-0128 named a mechanism without measuring it and Phase 8 killed it.
This ADR names an *axis* and hands the numbers to a measurement phase.

### The corrected stop condition

Plan 0119's gate passes only if **all three** hold:

1. **Separation.** `Blown Out` reads below `Tiled Rosette Mono`.
2. **Nothing convictable in the gap.** No shipped frame lies between them **that is itself above
   `MAX_TONAL_FLATNESS`** — because no other frame can reach this term. Frames in the gap that are
   not flat are reported, not disqualifying, and the report must print both columns so the reading
   is checkable rather than asserted.
3. **The threshold convicts the blot with margin, and is named for what it is.** With two members in
   the conditional population, *half the sparsest legitimate content* is not available: there is one
   legitimate member and it is the preset being admitted, which would be circular. The number is
   therefore a **measurement** in ADR-0071's sense — it names its corpus and the two frozen fixtures
   it was taken between — or the candidate does not ship. A threshold that cannot fail `Blown Out`
   fails this ADR exactly as it failed 0128's.

If the candidate misses any of the three, **Plan 0119 stops**, this ADR takes a dated `Outcome`, and
`fragment_tiledmono` stays held. Same stop condition, same reason, third time.

## Consequences

### Positive

- **The axis matches the failure.** A blot is *fine-grained and uniform*; a graphic composition is
  *coarse-grained and arranged*. Tiling separates exactly those, where pixel-scale mask geometry
  conflates them.
- **The thin-stroke hazard is structurally reduced.** Design-backlog 0072 measured that a hairline
  aliases to almost nothing at 96x96, which is what made `coverage` a halo-meter and what ADR-0128
  flagged as a boundary-length measure's inherited risk. A modal band is a *majority vote inside a
  tile*, so it does not care how thin the stroke is — only whether the tones it separates are
  arranged. It fails differently, and Plan 0119 measures the new failure rather than assuming it away.
- **It is the instrument the full-coverage residue needs.** ADR-0128 left `Sumi`, `Whorl`,
  `Supernova` and `Neon Tunnel` reading honest `coverage` near 1.0 with nothing asking whether they
  are compositions or fills. A composition-scale arrangement statistic is that question. This ADR
  does not spend it there, and stops the question being uninstrumented.
- **The conditioning correction outlives whichever candidate wins.** Criterion 2 above is a general
  rule about conjunctive gates and applies to any future second term.

### Negative

- **The calibration has two points, and that is the real price.** One frozen defect and one held
  composition. A third tonally flat frame — legitimate and low-transition — would be a false positive
  nothing in the corpus predicts. The threshold must say this in its own docstring; a number
  presented as derived when it was fitted between two frames is the failure mode ADR-0071 exists for.
- **The tile size is a new constant with its own resolution hazard**, in the opposite direction to
  the old one. At 96x96 an 8x8 grid is 12px tiles: coarse enough to filter particle noise, and coarse
  enough that a composition whose period is *finer* than 12px reads as one flat tile everywhere.
  Fine-ornament content is therefore a required row in the measurement, not an assumption — the three
  frozen `retired_mandalas` are already in the instrument for exactly this.
- **A new blind spot replaces the old one.** A blot that is *coarsely* mottled — an over-driven field
  with large-scale variation and no fine detail — passes this term where a pixel-scale one would have
  caught it. The library has no such frame today, which is why it is a stated risk rather than a
  measured one.
- **Inherited from ADR-0128 and unchanged:** the gate is strictly weaker, a conviction now needs both
  terms, and the failure message has to tell an author *which* of the two to fix.
- **Two statistics now read the same 16-band binning for different questions.** A future change to
  `TONE_BANDS` moves both, and only one of them has a docstring saying so today.

### Neutral

- `MAX_TONAL_FLATNESS`'s own value and derivation are untouched. Its *meaning* narrows, which is
  ADR-0128's consequence, not a new one.
- The C ABI, the `Scene` trait and the render path are untouched. This is entirely test-side.

## Alternatives considered

### Alternative A — Re-condition the stop condition and keep `boundary`

The Context finding taken as the whole decision: fix the population error, re-run Phase 8 unchanged,
and ship whichever candidate then passes. `boundary` plausibly does — it separates 0.2631 -> 0.3602
and every frame in that gap is far below the flatness ceiling.

Rejected as the *primary* decision on two counts. The margin is **1.37x** on a statistic whose
legitimate range spans 0.0440 to 0.9839, so the separation sits inside the library's own spread
rather than outside it. And it leaves the term reading pixel-scale raggedness, which means the next
blot with a slightly noisier mask defeats it for the same reason this one nearly did — the
conditioning error hid a design error rather than causing it.

**The conditioning correction itself is adopted, not rejected.** It is criterion 2 of the stop
condition above, and Plan 0119 applies it to `boundary` as a control row precisely so this
alternative stays falsifiable: if the tiled statistic fails and `boundary` passes conditioned, that
is a finding worth having in the same table.

### Alternative B — Scope the gate by idiom rather than adding a term

Read the idiom off engine-visible facts instead of measuring structure: a preset with
`palette_steps > 0` is hard-quantized upstream of the tonemap and cannot have tonal spread by
construction, and ADR-0123's `shape_collage` is a flat-graphic family by definition. `tiledmono` sets
`palette_steps = 20`, so it would be exempt; `Blown Out` sets neither, so it would still be convicted.
Cheapest route on the board — no new statistic, no new constant, no measurement phase.

Rejected because it makes the exemption **reachable by declaration**. The cheapest way past the
flatness ceiling becomes setting `palette_steps`, which is a one-line edit any preset can make, and
that is the same "tickle the gate" incentive ADR-0128's Positive section is proud of removing (an ink
of `#010101` passing the lit test). It also answers nothing about the full-coverage residue, and it
would have to be re-litigated the first time a graphic look wants a continuous palette.

### Alternative C — Raise `MAX_TONAL_FLATNESS` above the graphic idiom

ADR-0128 Alternative A. Stands rejected, unchanged: `0.9413` against `0.9815` is a `0.04` margin
between a real preset and a purpose-built defect, and a denser duotone walks into the new ceiling
exactly as it walked into the old one.

### Alternative D — A test-side roster of graphic presets

ADR-0128 Alternative C. Stands rejected, unchanged — `KNOWN_FLAT` by another name, and it is
`KNOWN_FLAT`'s own documented rule that a preset going over is "a defect to route, not an entry to
re-add".

### Alternative E — Hue spread as the second term

ADR-0128 Alternative D. Stands rejected, unchanged: both of `Tiled Rosette Mono`'s large populations
are achromatic, so a hue-spread term reads zero for the preset it is meant to rescue.

## Notes

Every number in Context comes from re-running Plan 0116 Phase 8's own instrument at the Plan 0113
close tip (`0b9a486`, `v0.79.0`), unmodified:

```text
cargo nextest run -p lmv-core --test sanity --run-ignored all \
    each_structure_candidate_is_tabled_against_the_library --no-capture
```

The `tonal_flatness` column beside it is that same run's `flatness^-1` inverted, and the shipped
maximum of `0.5262` is cross-checked against the distribution `every_preset_draws_a_real_shape`
prints on every run.

**`Tiled Rosette Mono` clears all four other gates today**, measured 2026-08-26 by copying it into
`presets/` and running the suite: `coverage` 0.4952 against a 0.08 floor, 4 quadrants, 10/10 shells,
`reactivity` `bass=0.3493`, `animation` footprint 0.7579, and no near-duplicate geometry against
`Tiled Rosette` or any sibling. Flatness is the only thing holding it, which is what makes it a
usable second calibration point rather than a preset with several problems.

## Outcome (2026-08-26) — the correction was the finding; the axis claim was not

[Plan 0119](../plans/done/0119-the-flatness-gate-gets-its-second-term.md) Phase 1 added this ADR's
composition-scale candidate to the Plan 0116 Phase 8 instrument, swept its tile count over
`[4, 6, 8, 12, 16]`, and re-judged all four columns under the corrected stop condition. **The tiled
statistic is not what ships.**
[ADR-0130](0130-the-structural-term-is-boundary-density-and-conditioning-the-population-is-what-made-it-work.md)
takes `boundary` — this ADR's own Alternative A — and supersedes the Decision above.

Three findings, in the order they matter.

**1. The conditioning correction was the entire finding, and it holds.** Criterion 2 above is right,
is general, and is what unblocked two plans: judged over the population a second term can actually
reach, `boundary` (1.37x), `components` (2.33x) and `sobel` (6.31x) all separate the blot from the
composition, having all "failed" Phase 8 unconditioned. Not one of their readings moved — only the
population they were judged against did. The conditional population is **two members**, exactly as
this ADR predicted.

**2. The axis claim did not survive.** This ADR's Context argues the three candidates are one axis
and *"the axis is wrong"*, and its Alternative A concludes that *"the conditioning error hid a design
error rather than causing it."* The measurement says the opposite: there was no design error under
the conditioning error. Correct the population and the pixel-scale axis works.

**3. The thin-stroke Positive is falsified, and inverted.** This ADR claims tiling *structurally
reduces* the hairline hazard, and asked for the three frozen `retired_mandalas` as a required row
rather than an assumption — which is what makes this checkable. They read `0.0000` on **every** grid
in the sweep, the absolute floor of the statistic, against `0.8653`–`0.9641` on `boundary`. A tile's
majority vote does not care how thin a stroke is, but it also cannot see a period finer than itself,
so dense ornament reads as one flat field. The claimed mitigation throws away the content it was
meant to protect.

**And the tile count is a fitted number.** The verdict flips between `tile@4` (not separated, both
anchors `0.3333`) and `tile@6` (6.00x), with no plateau above it — the composition's own reading of
one fixed frame swings 2.24x between `tile@6` and `tile@8`. Plan 0119's Risks section pre-registered
that reading as a stop before the numbers existed.

**What stands:** criterion 2, the instrument, and the insistence that an ADR name an axis and hand
the numbers to a measurement phase. That insistence is the reason this ADR's own mechanism could be
falsified in one phase rather than shipped. **What falls:** the composition-scale statistic, the
"wrong axis" diagnosis, and the second Positive.
