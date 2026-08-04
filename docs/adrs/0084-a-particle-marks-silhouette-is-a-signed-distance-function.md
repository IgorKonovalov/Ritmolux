# ADR-0084 — A particle mark's silhouette is a signed-distance function

> **Status:** proposed
> **Date:** 2026-08-04
> **Related plan(s):** [0070](../plans/0070-shaped-marks.md)

## Context

The engine has no shape vocabulary for a mark. `swarm.rs`'s fragment shader is three lines —
`let d = length(in.local); let falloff = max(0.0, 1.0 - d); let g = falloff * falloff;` — a radial
falloff with **no shape input at all**. The emitter's sprite and the attractor's compute points are
the same idea. There is no glyph atlas, no SDF, no shape parameter, and nothing in any `PARAMS` list
that could carry one. The only other mark-making model is the line families, which stroke a
generator's path: one figure, centred, whole-frame, and stroke-only.

This surfaced from two user requests in one session
([design-backlog 0033](../design-backlog.md#0033--every-mark-the-engine-can-draw-is-a-round-additive-blob-or-a-stroked-curve-so-no-object-has-a-shape)):
a Solitaire-style cascade of hearts, and small seven-, eight- and nine-pointed stars twinkling on
bass and beat. Both were rendered as far as the current surface reaches. The star half is the
instructive one: `swarm` can put ten thousand small marks anywhere, but they are round;
`parametric_curve` with `radial_offset = 1` gives exactly `n` lobes and can flip the count every
beat — but it is **one large centred figure**, and `mirror_order` replicates about the origin, so
copies land on each other rather than scattering. Rendered, that reads well as a starfield and not
at all as *stars with points*.

The entry names four possible routes: a shape enum on the particle sprite, an SDF glyph, an
author-supplied WGSL pass, or a fill-and-stroke draw path outside the additive model. They are not
equivalent, and one of them reopens something large: **the pipeline is additive, so a dark mark
cannot exist.** A red-filled heart with a black outline is three tones (light ground, red body,
black edge), and the only dark-on-light route in the engine is the ink stage, which is structurally
two-poled (`mix(paper, ink, luminance)`). Measured, not assumed: the cardioid `r = 1 - sin(theta)`
drawn through `parametric_curve` at `ink_amount = 1` with white paper renders its outline **grey**,
because a thin anti-aliased stroke averages to mid luminance and lands halfway down the ramp.

So there are two asks inside one entry, and only one of them is cheap.

## Decision

We will give the particle scenes a **`shape` parameter selecting a signed-distance function**,
evaluated in the existing fragment shader, with the existing quadratic falloff applied to the
normalized distance from the shape's boundary instead of from the sprite centre. `disc` remains the
default and is *exactly* today's arithmetic. `swarm` and `emitter` get it; the `attractor` does not
(its marks are a chaos-game accumulation — at the densities that make a figure, one mark is a point,
and a silhouette there would be invisible on principle rather than by tuning).

**This deliberately answers the silhouette half and not the fill-and-outline half.** A heart in
additive light is a heart-shaped glow, not a red body with a black edge. That second capability
reopens the additive-blending decision the whole composite assumes, and it stays in the backlog as
its own question rather than being smuggled in behind a shape enum.

## Consequences

### Positive

- **Nothing about the composite changes.** Still additive, still premultiplied coverage
  ([ADR-0056](0056-additive-scenes-emit-premultiplied-alpha.md)), still one pipeline per scene. The
  shader gains a branch and the params list gains two names.
- **The mainstream ask lands.** "A field of small n-pointed stars, twinkling on the beat" becomes a
  preset, and it is the form the request arrived in twice.
- **It composes with everything already built.** `size`, `spin`, `twinkle`, the depth parallax, the
  hue spread and the whole post chain apply unchanged, because the only thing that moved is where
  the falloff measures from.
- **No assets, no atlas, no filtering.** An SDF is resolution-independent, so a mark is as clean at
  three pixels as at three hundred — which matters here specifically, because the ask is *small*
  marks scattered widely.

### Negative

- **A branch in the hottest fragment shader in the engine.** The swarm draws thousands of sprites
  per frame; every fragment now selects a distance function. Uniform branching on a per-draw value
  is the cheap case, but it is not free, and the iGPU floor in `docs/nfr.md` §7 is what it answers
  to.
- **The roster is closed and the engine decides it.** An author who wants a shape not on the list is
  back to routing through `architect` + `dev`. That is the same trade
  [ADR-0061](0061-kaleidoscope-edge-treatment-is-a-per-preset-choice.md) made for fold-edge
  treatments and it has held, but it is a real ceiling.
- **`points` steps rather than morphs.** A star SDF folds the angle by the point count, so a
  fractional count is not an intermediate figure — it is a discontinuity where a lobe appears. The
  value is quantized CPU-side (the `kaleido_order` precedent) and eased `points` therefore *steps*.
  This must be documented at the parameter, because the surrounding vocabulary — `variant` since
  [ADR-0060](0060-star-pattern-variants-interpolate.md), the IFS morph in
  [ADR-0075](0075-ifs-family-morphs-in-singular-value-space.md) — has been teaching authors the
  opposite.
- **It answers half of the request that motivated it**, and the half it does not answer is the one
  the user described more concretely. Saying so plainly is part of the decision; the alternative is
  a capability that looks like it should produce hearts-with-outlines and does not.

### Neutral

- The attractor's exclusion is by argument rather than by default. If someone later wants shaped
  marks in a sparse attractor configuration, that is a fresh question with a real premise
  (`density` is low enough now that marks are visible), not an oversight to correct.

## Alternatives considered

### Alternative A — A glyph or sprite texture atlas

The conventional answer: authored art, sampled per fragment. Rejected because it introduces an asset
pipeline this project does not have (who draws the glyphs, where do they live, how are they
embedded), and because it is *worse* at the actual request: the ask is small marks, and a sampled
texture at three pixels is a filtering problem where an SDF is exact.

### Alternative B — A fill-and-stroke draw path outside the additive model

What the Solitaire-hearts request literally needs: a filled interior and a contrasting outline.
Rejected as a bundled decision rather than on its merits — it reopens
[ADR-0018](0018-engine-wide-scene-compositing.md)'s composite and ADR-0056's alpha
model, needs an ordering or sorting story that the additive pipeline has never required, and would
make the shape roster a rider on a much larger question. It stays open in the backlog as its own
entry, which is the honest place for it.

### Alternative C — Author-supplied WGSL

Maximum power: a preset carries its own fragment shader. Rejected because
[ADR-0002](0002-layered-preset-architecture.md) parked this deliberately and
`docs/roadmap-visual-richness.md` re-parks it — it trades the whole curation and QA model for
authoring power, and every preset-level gate this project has built stops meaning anything the
moment a preset can compute its own pixels.

### Alternative D — Extend `parametric_curve` to scatter copies of its figure

The n-lobed figure already exists; the missing part is placing many small copies. Rejected because
`mirror_order` replicates about the origin by construction, so scattering would mean giving a line
scene a per-instance transform — which is the particle idiom, arrived at from the wrong side and
without the particle scenes' motion, lifetime or depth machinery.

## Notes

- Measured evidence that the outline half is genuinely blocked and not merely untried: the cardioid
  through `parametric_curve` at `ink_amount = 1`, white paper, renders grey. Recorded in
  [backlog 0033](../design-backlog.md#0033--every-mark-the-engine-can-draw-is-a-round-additive-blob-or-a-stroked-curve-so-no-object-has-a-shape).
- The falloff to preserve is `g = max(0, 1 - d)^2` — quadratic, and the reason a wider `thickness`
  on the line family reads as *out of focus* past a point. The same curve over an SDF distance keeps
  the family's look consistent; changing the curve at the same time as the silhouette would make any
  visual regression ambiguous.
- [ADR-0057](0057-emitter-scene-analytic-ballistics-seeded-individuation.md)'s emitter already
  answers the *motion* half of the Solitaire request — spawn, arc, age, die. This ADR answers the
  silhouette half. What remains unanswered after both is only the two-tone fill.
