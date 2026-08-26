# 0126 — The sanity lens measures departure from the frame's own ground, not from black

> **Status:** proposed
> **Date:** 2026-08-25
> **Related plan(s):** [0116](../plans/0116-the-sanity-lens-finds-the-ground.md)
> **Relates to:** [ADR-0067](0067-coverage-measures-the-scene-not-the-backdrop.md) (the decision
> this supersedes in its premise — the capture still suppresses the backdrop; what changes is what
> "against black" is allowed to mean), [ADR-0123](0123-a-flat-graphic-scene-paints-its-own-paper-and-composites-opaque-elements-in-one-pass.md)
> (the scene that forces this), [Plan 0075](../plans/done/0075-the-content-renaissance.md)
> (the shell-occupancy rescue, whose shape the alternatives below borrow)
> **Raised from:** [design-backlog 0128](../design-backlog.md)

## Context

`core/tests/sanity.rs` asks four questions of every shipped preset, and all four are built on one
predicate:

```rust
fn is_lit(px: &[u8], bg: [u8; 4], eps: u8) -> bool {
    px.iter().zip(bg.iter()).take(3).any(|(&c, &b)| c.abs_diff(b) > eps)
}
```

`sanity` passes `BLACK`. `coverage` is lit/total, `quadrant_spread` counts lit pixels per quadrant,
`radial_shell_occupancy` counts shells holding lit pixels, and `tonal_flatness` buckets lit pixels by
luminance. The lens therefore encodes an **unstated precondition**: the scene draws light onto a
ground it does not own, and black means nothing was drawn. ADR-0067 made that precondition
deliberate for the *backdrop* — the capture suppresses `bg_*` so every lit pixel is light the scene
put there. What it did not decide, because nothing then contradicted it, is what happens when the
**scene itself** paints a ground.

Three facts, all measured on the current tree, say that time has arrived.

**The degeneracy already ships.** Twelve of the 41 presets in the roster read `coverage = 1.0000`
exactly — every `fragment_field` preset, `Vellum`, `Facet`, `Drift`, and the attractor's
`Ink on Paper`, `Thomas` and `Valentine`. For those, `coverage` cannot distinguish a composition
from a blank fill, `quadrant_spread` is 4 by construction and `radial_shell_occupancy` is 10/10 by
construction. Their coverage floors — `0.50` for `fragment_field` — are passed without being tested.
`Ink on Paper` is the sharp case: an ink-on-paper duotone is a **light-ground world that already
exists in the shipped library**, reading `coverage 1.0000, flatness 0.7833`.

**A twelfth system universalises it.** [Plan 0113](../plans/0113-the-engine-paints-a-canvas.md) is in
flight and adds `shape_collage`, which paints its own off-white paper across every pixel. Its own
branch already records the consequence, in a `coverage_floor` arm written by `dev`: *"its lit
fraction is 1.0 by construction whatever the elements do, and the statistic this floor is made of
cannot distinguish a good canvas from an empty one."* That arm then leans on `MAX_TONAL_FLATNESS` as
the rescue — and the rescue is read at the one excitation where it cannot fire. `sanity` captures at
`LOUD`, where Phase 6's `density` lever holds the canvas at its **fullest**; the quieter capture
(Plan 0058) buys exactly one gate, `MODERATE_MIN_COVERAGE`, which is the areal statistic that is
already degenerate for this family. Phase 6 explicitly builds an emptying canvas. **The state the
plan is designed to produce is measured by nothing**, and a canvas the music emptied correctly is
pixel-for-pixel the same flat sheet of paper as one that is broken and drew no elements.

**The mirror failure convicts correct content.** A `fragment_field` preset posterized to black,
white and one red measures `flatness = 0.9346` against the `0.90` ceiling — not because it is a
blot, but because its black *ink* is excluded as unlit, leaving the statistic to measure the white
alone. The docstring frames the ceiling as catching a figure "stacked past the additive ceiling";
this is the opposite failure mode caught by the same net. Raising the second lit tone until the
number moves was measured and costs the look: it takes red to 46.6 % of lit pixels.

The two failures are not symmetric, and the weighting matters. The **false positive** is bounded —
it needs roughly 90 % single-tone. The **false negative** is unbounded and designed-in.

## Decision

We will make the sanity lens measure departure from **the frame's own ground**, derived from the
frame, rather than from a hardcoded `BLACK`. `is_lit` and the four statistics built on it take a
reference tone that is *estimated per capture*, so a scene that paints its own paper is measured
against that paper and a scene that draws light onto darkness is measured against the darkness — the
same question, asked correctly in both worlds.

**The estimator is chosen by measurement, not by argument, and this ADR does not name it.** That is
a deliberate limit on the decision's scope, because the obvious estimator is already falsified. Naive
modal tone — the most populous luminance bucket — would change the reference for **17 of the 41
shipped presets**, derived from the suite's own printed table by comparing black's share (`1 -
coverage`) against the largest lit bucket (`flatness × coverage`): `Clifford` alone reads 15.9 %
against 6.8 %, and the twelve full-coverage presets have no black to be modal at all. An estimator
that silently re-bases half the library is not a refinement of this lens, it is a different lens.
[Plan 0116](../plans/0116-the-sanity-lens-finds-the-ground.md) Phase 1 measures candidate estimators
against the whole library and Phase 2 is a stop gate on the result, in the shape Plan 0113 Phase 3
and Plan 0075 Phase 1 both use.

What this ADR fixes is the **principle** and the three alternatives it rules out.

## Consequences

### Positive

- The four statistics become meaningful for every world the engine can draw, including the two that
  already ship degenerate (`Ink on Paper`, the twelve full-coverage presets) and the one arriving in
  Plan 0113.
- `MODERATE_MIN_COVERAGE` starts gating `shape_collage` instead of passing it by construction, which
  is what catches the emptied canvas Phase 6 builds — at the excitation where it actually happens.
- One change at the root. Re-basing `is_lit` fixes all four statistics together; fixing
  `tonal_flatness` alone would leave three still blind.
- A correct flat-graphic composition stops being convicted for having a dark ink, which removes the
  incentive to defeat the gate by tickling `EPS` (an ink of `#010101` passes today and is visually
  indistinguishable from `#000000` — the workaround this decision makes unnecessary).

### Negative

- **Baselines move for content that is not broken.** However the estimator is chosen, some shipped
  presets will be measured against a different reference than they are today, and separating "this
  preset was always defective and the old lens could not see it" from "this preset is fine and the
  new lens is wrong" is judgement per preset, not a number. Plan 0116 Phase 3 budgets for it.
- **A derived ground is a heuristic**, and heuristics have a failure mode a constant does not: a
  frame with no dominant tone gets an arbitrary reference. The plan must state what the estimator
  does there rather than discover it later.
- **`coverage` floors were calibrated against the old predicate.** Every per-system floor, and
  `MAX_FLOOR_SLACK`, is a measured constant whose measurement assumed departure-from-black. Changing
  the predicate invalidates the calibration even where it does not change a verdict.
- The capture surface grows by a reference tone that used to be a constant, and `golden.rs` and
  `reactivity.rs` pass `BLACK` to the same metrics — they must be audited even if their answers do
  not change.

## Alternatives considered

- **A per-preset or per-system declaration of the ground.** Rejected on a measured fact:
  `fragment_field` hosts both world models. `Sumi` is luminous and `Tiled Rosette Mono` is graphic,
  on the same system, so a `SystemKind` property cannot express it; and a preset-level declaration
  puts a *test-harness* concern into the authoring surface, where it is self-reported by the author
  whose preset is being judged.
- **A test-side roster of graphic presets.** Rejected because it is `KNOWN_FLAT` by another name.
  That list is documented in `sanity.rs` as a defect list that must stay empty — *"if one ever goes
  over, that is a defect to route, not an entry to re-add"* — and a second list with the same shape
  and a friendlier name defeats the same rule.
- **Measure `tonal_flatness` over all pixels instead of lit ones.** Rejected because it breaks the
  statistic's original purpose: an additive blot covering 30 % of a black frame would read 0.70 and
  pass, since the background becomes the dominant population. It fixes the false positive by
  discarding the true positive.
- **Leave the lens alone and read `tonal_flatness` at the quiet excitation too.** This is the
  cheapest fix for the false negative specifically and was seriously considered. Rejected as the
  *decision* because it treats the symptom: `coverage`, `quadrant_spread` and
  `radial_shell_occupancy` stay degenerate for every full-coverage preset, and the flat-graphic false
  positive is untouched. It survives as a candidate **inside** Plan 0116 — a second excitation may
  still be wanted once the ground is right.

## Outcome (2026-08-26)

[Plan 0116](../plans/0116-the-sanity-lens-finds-the-ground.md) Phase 1 measured the candidate
estimators against the whole library, and Phase 2 chose **`modal_luma`** — the frame's modal
luminance band. Three things this ADR asserted did not survive that measurement, recorded here rather
than edited into the body above.

**The 17-of-41 count was right and its reading was wrong.** This ADR treated naive modal tone as
already falsified because it "would change the reference for 17 of the 41 shipped presets", and
concluded that "an estimator that silently re-bases half the library is not a refinement of this
lens, it is a different lens". The count reproduces exactly. The consequence does not: re-basing
those 17 changes **no preset's verdict**, at either excitation, for any of the three candidates
tabled. The library's verdicts are insensitive to the reference, which is the opposite of what the
count was taken to imply — and the estimator was adopted on that measurement.

**The false positive is not a ground problem, and this ADR's diagnosis of it is falsified.** The
Context above attributes `fragment_tiledmono`'s `flatness = 0.9346` to its black ink "being excluded
as unlit, leaving the statistic to measure the white alone". With the ground correctly at the paper,
the ink is included and the *paper* is excluded — and the preset reads `0.9413`, marginally worse.
All three estimators found the paper at `(245,245,245)` and all three still convicted it. A duotone
has two large populations and `is_lit` removes whichever one is the ground, so the other holds ~94 %
of what remains either way. That residue is a property of `tonal_flatness`, and
[ADR-0128](0128-a-tonally-flat-picture-is-a-blot-only-if-it-is-also-structureless.md) takes it.

**"The four statistics become meaningful for every world the engine can draw" is true for eight of
the twelve, not all of them.** The estimator clears the `coverage = 1.0000` degeneracy where a ground
exists — `Tiled Rosette` to `0.1645`, `Ink on Paper` to `0.2167`, `Thomas` to `0.2917`, `Vellum` to
`0.3704`, `Valentine` to `0.4389`, `Facet` to `0.5940`, `Vitrail` to `0.7071`, `Banded Mandala` to
`0.7574`. It cannot where none does: `Sumi`, `Whorl`, `Supernova` and `Neon Tunnel` are smooth
luminous fields whose modal band holds `7.4 %` / `5.0 %` / `0.7 %` / `0.3 %` of the frame, at or under
the `6.25 %` a uniform distribution puts in one of sixteen bands. Their coverage of 1.0 is honest and
no reference tone changes it. This is the "frame with no dominant tone" the Negative section
anticipated, found in four shipped presets rather than in a hypothetical — Plan 0116 Phase 3 defines
the estimator's behaviour there rather than discovering it.

What stands unchanged is the Decision itself and all four Alternatives. The lens does measure
departure from the frame's own ground; the ground is derived per capture; and none of the four
rejected options became viable.
