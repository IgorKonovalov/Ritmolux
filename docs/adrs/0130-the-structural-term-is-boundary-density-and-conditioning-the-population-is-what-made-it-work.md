# 0130 — The structural term is boundary density, and conditioning the population is what made it work

> **Status:** proposed
> **Date:** 2026-08-26
> **Related plan(s):** [0119](../plans/0119-the-flatness-gate-gets-its-second-term.md)
> **Supersedes the Decision of:** [ADR-0129](0129-the-structural-term-is-measured-at-composition-scale-not-pixel-scale.md)
> — its composition-scale statistic was measured and is not what ships. **Its criterion 2 survives
> intact and is the reason this ADR exists**; see that ADR's dated `Outcome`.
> **Implements:** [ADR-0128](0128-a-tonally-flat-picture-is-a-blot-only-if-it-is-also-structureless.md)
> — the conjunction it decided, finally with a statistic under it.
> **Relates to:** [ADR-0126](0126-the-sanity-lens-measures-departure-from-the-frames-own-ground.md)
> (the ground the first term reads against),
> [ADR-0071](0071-a-numeric-test-contract-states-a-property-or-names-its-machine.md)
> (why the constant below is a measurement and must say so)
> **Raised from:** [Plan 0119](../plans/0119-the-flatness-gate-gets-its-second-term.md) Phase 1,
> measured 2026-08-26 at `8389f2a`

## Context

[ADR-0128](0128-a-tonally-flat-picture-is-a-blot-only-if-it-is-also-structureless.md) decided that
`MAX_TONAL_FLATNESS` stops being a verdict and becomes one of two terms. Plan 0116 Phase 8 measured
three candidate second terms — `boundary`, `components`, `sobel`, all computed on the binary lit mask
— and all three failed its stop condition.

[ADR-0129](0129-the-structural-term-is-measured-at-composition-scale-not-pixel-scale.md) read that
failure as **two** findings and acted on the second only as a correction. Its primary claim was that
the three candidates are *one axis chosen three times* — pixel-scale mask geometry — and that the
axis is wrong, because a particle cloud's mask at 96x96 is noise and noise is jagged. Its secondary
finding was that Phase 8's stop condition had been evaluated over the **whole library** when a
conjunction's second term is only ever asked about frames that already failed the first.

Plan 0119 Phase 1 added 0129's composition-scale candidate to the same instrument, swept its tile
count over `[4, 6, 8, 12, 16]`, and re-judged all four columns under the corrected condition. **The
secondary finding was the whole of it. The primary claim does not survive its own measurement.**

### Conditioned correctly, the pixel-scale axis works

The conditional population is two members, exactly as 0129 predicted: the frozen `Blown Out`
(`tonal_flatness` `0.9154`) and the held `Tiled Rosette Mono` (`0.9413`). **Nothing else in the
corpus is above the `0.90` ceiling at all**, so criterion 2 is inert on every column — there is no
frame that can be both in a gap and reachable by the conjunction. The stop condition therefore
reduces to separation plus a threshold, and under it:

| candidate | blot -> composition | margin | threshold | verdict, conditioned | verdict at Plan 0116 Phase 8 |
|---|---|---|---|---|---|
| `1 - tonal_flatness` (control) | `0.0846` -> `0.0587` | — | — | fails — not separated | fails |
| `boundary` | `0.2631` -> `0.3602` | 1.37x | `0.3116` | **passes** | fails |
| `components` | `4.4223` -> `10.2980` | 2.33x | `7.3601` | **passes** | fails |
| `sobel` | `0.2199` -> `1.3876` | 6.31x | `0.8037` | **passes** | fails |

Not one number in the first column moved. What moved is the population they were judged against —
which means **the conditioning error was not hiding a design error, it was the entire error.** That
is the opposite of what 0129's Alternative A concluded, and it is 0129's own instrument that says so.

### The tiled statistic is resolution-coupled, and the sweep is how we know

0129 declined to name a tile count and handed it to a measurement phase. That was right, and the
measurement is decisive:

| grid | blot | composition | separated | margin | frames reading exactly `0.0000` (of 45) |
|---|---|---|---|---|---|
| `tile@4` | `0.3333` | `0.3333` | **no** | — | 22 |
| `tile@6` | `0.1333` | `0.8000` | yes | 6.00x | 18 |
| `tile@8` | `0.1429` | `0.3571` | yes | 2.50x | 14 |
| `tile@12` | `0.1061` | `0.5455` | yes | 5.14x | 10 |
| `tile@16` | `0.0833` | `0.5083` | yes | 6.10x | 6 |

The verdict flips between the first two adjacent grids, and there is no plateau above the flip: the
composition's own reading of one fixed frame swings **2.24x** between `tile@6` and `tile@8`. Plan
0119's Risks section pre-registered exactly this reading — *"a constant that only works at one tile
count is a fitted number"* — before the measurement ran.

### The thin-stroke hazard inverted

0129's second Positive claims the tiling **structurally reduces** the hairline hazard that made
`coverage` a halo-meter, because a modal band is a majority vote inside a tile and does not care how
thin a stroke is. It asked for the three frozen `retired_mandalas` as a required row rather than an
assumption. They are in the table, and they say the reverse:

| frozen thin-stroke fixture | `boundary` | `sobel` | `tile@4..16` |
|---|---|---|---|
| Star Mandala (retired) | `0.9480` | `1.7367` | `0.0000` at every grid |
| Mandala Six (retired) | `0.9641` | `1.8712` | `0.0000` at every grid |
| Mandala Weave (retired) | `0.8653` | `1.3578` | `0.0000` at every grid |

Dense thin-stroke ornament sits at the **top** of the pixel-scale statistics' range and at the
**absolute floor** of the tiled one — its period is finer than the tile, so every tile takes the same
modal band and the frame reads as one flat field. Those three are safe only because term one clears
them, which is luck rather than design. The low-pass filter that was supposed to be the mechanism
throws away the exact content the mechanism was meant to protect.

## Decision

We will use **`boundary` — the share of lit pixels having at least one unlit 4-neighbour, perimeter
over lit area** — as the flatness gate's second term, and we will adopt ADR-0129's conditioning
correction as the reason it is admissible.

`every_preset_draws_a_real_shape` convicts a preset only when `tonal_flatness > MAX_TONAL_FLATNESS`
**and** `boundary_density < MIN_BOUNDARY_DENSITY`, with `MIN_BOUNDARY_DENSITY = 0.31`.

**Why `boundary` and not the two other columns that also pass conditioned**, which is the question
this ADR is actually deciding:

- **Its denominator is the lit area, so it asks one question.** `sobel` normalizes by *frame* area
  (`sum / ((w-2) * (h-2))` in the instrument), so a frame with more lit material scores higher
  whether or not that material is structured — it partly re-asks *how much is lit*, which is
  `coverage`'s job and already a separate term of the same gate. A second term that duplicates a
  first is not the orthogonality ADR-0128 asked for. `boundary` divides by lit pixels and is
  therefore dimensionless in the direction that matters: a solid mass reads near zero however large,
  a hatched figure reads near one however small.
- **`components` inverts on exactly the content this gate exists to catch.** It counts 4-connected
  components per thousand lit pixels, so a particle field — where every speck is its own component —
  scores *highest*: `Drift Field` `224.05`, `Vellum` `198.01`, `Thomas` `102.68`. That is the axis
  inversion 0129 correctly diagnosed, surviving in the one candidate where it is structural rather
  than incidental. Its legitimate range also spans `0.1088..224.05`, a 2000x spread across which a
  2.33x separation is not a separation.
- **`boundary` reads as the sentence we mean.** "Is the lit set a mass or does it have interior?" is
  the question a blot fails and a composition passes, and perimeter-over-area is that question
  written down. Neither of the others is interpretable without a paragraph.

**The threshold is a measurement, and it is taken between two frozen frames.** `0.31` is the midpoint
of `0.2631` (the frozen `Blown Out` fixture) and `0.3602` (`Tiled Rosette Mono` as measured
2026-08-26), rounded to two places: `1.18x` above the defect, `1.16x` below the composition. It is
**not** half the sparsest legitimate content, the ceremony every other constant in
`core/tests/sanity.rs` follows — with one legitimate member in the conditional population, and that
member being the preset the change exists to admit, that derivation is circular. ADR-0071 requires
such a number to say what it is, and its docstring must say this in plain words.

**Both anchors must be frozen fixtures, and one of them is about to stop being one.**
`Tiled Rosette Mono` reaches the instrument today through
`include_str!("../../presets/pending/fragment_tiledmono.toml")`. Once Plan 0119 Phase 4 ships that
file into `presets/`, the calibration anchor becomes ordinary editable content and a preset tweak
could silently move a gate constant — with nothing to notice, because the constant would still be
green. Phase 4 therefore **freezes the measured TOML into the test** the way `retired_mandalas()`
already freezes three presets from a git revision, and the shipped copy goes on being judged by the
gate like any other preset.

## Consequences

### Positive

- **It ships.** ADR-0128's conjunction has been undeliverable across two plans and three ADRs, and
  `fragment_tiledmono` has been finished, approved and blocked on one number the whole time. This
  needs no new statistic, no new constant beyond the threshold, and no new mechanism — `boundary`
  is already written, already measured and already understood.
- **The correction generalizes and outlives the candidate.** Criterion 2 — *a conjunction's second
  term is judged only over the population that reaches it* — is a rule about conjunctive gates, not
  about this one. It is the whole reason three "failed" candidates turned out not to have failed,
  and it applies to whatever term is added next.
- **No free parameter.** `boundary_density` has one definition and nothing to tune. The tiled
  statistic would have shipped a tile count that the sweep shows changes the answer, and a constant
  chosen from a curve with no plateau is a fitted number wearing a measurement's clothes.
- **The thin-stroke family is at the safe end of it.** The three frozen mandalas read `0.87`–`0.96`,
  the top of the range, against a `0.31` floor. Design-backlog 0072's hazard — that a hairline
  aliases to nothing at 96x96 — does not reach a statistic normalized by lit area, because a hairline
  is nearly *all* perimeter.

### Negative

- **The margin is 1.37x, inside the library's own spread.** This is ADR-0129 Alternative A's
  objection and it stands unrebutted: `boundary`'s legitimate range is `0.0440..0.9839` and the
  separation occupies about a tenth of it. It is accepted because the alternative on offer was a
  wider margin drawn from a statistic that moves 2.24x when you change a constant it invents.
- **24 of 45 non-anchor frames read below `0.31`.** Over half the library is under the structural
  term and is protected only by term one. The conjunction is doing real work — but it also means a
  preset that ever drifts over the flatness ceiling has a better-than-even chance of being convicted
  by a term that was never calibrated on it.
- **The two-member calibration is unchanged, and no decision in this plan could change it.** One
  frozen defect, one composition. A third tonally flat frame — legitimate and low-perimeter — is a
  false positive nothing in the corpus predicts. Inherited from ADR-0129's first Negative and still
  the real price.
- **The known decay mode is a noisier blot.** A defect whose mask is more ragged than `Blown Out`'s
  defeats this term, which is the mechanism 0129 was written to escape. We now believe that
  escape route was never open, but the hazard it was reacting to is real and unaddressed.
- **`Blown Out` is one frozen frame doing even more work than before.** It is the sole anchor on the
  defect side of `MAX_TONAL_FLATNESS`, of ADR-0128's conjunction, and now of `MIN_BOUNDARY_DENSITY`.
  A re-bless moves three thresholds.
- **The gate is strictly weaker**, inherited from ADR-0128 and unchanged: a conviction now needs both
  terms, and the failure message has to tell an author which of the two to fix.

### Neutral

- `MAX_TONAL_FLATNESS`'s value and derivation are untouched. Its *meaning* narrows, which is
  ADR-0128's consequence.
- The C ABI, the `Scene` trait and the render path are untouched. One function moves from
  `core/tests/sanity.rs` into `core/src/render/metrics.rs`; nothing that renders changes.
- The full-coverage residue (`Sumi`, `Whorl`, `Supernova`, `Neon Tunnel` reading honest `coverage`
  near 1.0) is still uninstrumented. 0129 argued its statistic was the instrument that question
  needs; that argument is not disturbed by this ADR, and the question is still another plan's.

## Alternatives considered

### Alternative A — Ship ADR-0129's tiled statistic at `tile@16`

The widest margin on the board (6.10x) and the fewest floored frames (6 of 45), on the axis 0129
argued for.

Rejected on the sweep. The verdict flips between `tile@4` and `tile@6` and never settles: 6.00x,
2.50x, 5.14x, 6.10x across four grids that all "pass". A statistic whose reading of one fixed frame
swings 2.24x between adjacent parameter values has high variance, and a wide gap between two points
drawn from a high-variance statistic is not a wide *separation* — it is two draws. Plan 0119
pre-registered this as a stop before the numbers existed, which is the only reason the reading is
worth anything. The falsified thin-stroke Positive is the second count: 0129 asked for the retired
mandalas as a required row precisely so this could not be assumed away, and they read `0.0000`.

### Alternative B — Ship `sobel` over the binary mask

Separates 6.31x, the widest of the three pixel-scale columns, and unlike the tiled candidate it has
no free parameter.

Rejected on its denominator. It normalizes by frame area, so it conflates *how much is lit* with
*how structured the lit set is*, and the first of those is `coverage` — another term of the same
gate, measured on the same capture. Adding a second term that partly restates the first buys less
independence than the raw margin suggests. `boundary` is the same question asked cleanly.

### Alternative C — Ship `components`

Also passes conditioned, at 2.33x.

Rejected because it scores particle fields highest — `Drift Field` `224.05`, `Vellum` `198.01` —
which is the blot-shaped content this term exists to convict, at the top of the range. A 2.33x
separation inside a `0.1088..224.05` spread is noise.

### Alternative D — Stop, and leave the gate with one term

Plan 0119 Phase 2's third option, and the outcome that fired twice before.

Rejected because nothing is left to learn by stopping. The two prior stops each ended with a named
mechanism that had not been measured; this one ends with four measured columns, three of which pass
a stop condition whose defect has been found and corrected. A third stop would hold
`fragment_tiledmono` out of the set on the strength of a criterion we now know was mis-evaluated.

### Alternative E — Raise `MAX_TONAL_FLATNESS` above the graphic idiom

ADR-0128 Alternative A, ADR-0129 Alternative C. Stands rejected, unchanged: `0.9413` against `0.9815`
is a `0.04` margin between a real preset and a purpose-built defect.

### Alternative F — Scope the gate by idiom, or keep a roster of graphic presets

ADR-0129 Alternatives B and D. Both stand rejected, unchanged — the first makes the exemption
reachable by declaration (`palette_steps` becomes the cheapest way past the ceiling), the second is
`KNOWN_FLAT` by another name, and that list's own documented rule is that a preset going over is a
defect to route rather than an entry to re-add.

## Notes

Every number above comes from one run of the Plan 0119 Phase 1 instrument at `8389f2a`:

```text
cargo nextest run -p lmv-core --test sanity --run-ignored all \
    each_structure_candidate_is_tabled_against_the_library --no-capture
```

It prints, per frame, `tonal_flatness` beside all nine candidate columns, and per candidate the
three parts of the corrected stop condition including the superseded ceremony's number, so the
re-judging of the three Plan 0116 Phase 8 columns is checkable against that plan's own printed
result rather than taken on this ADR's word.

The 45 non-anchor frames are the 42 shipped presets plus the three frozen `retired_mandalas`. The
two anchors are excluded from every count and spread in this document.
