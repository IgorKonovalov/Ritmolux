# ADR-0098 — the line renderer draws arcs as per-pixel distance fields

> **Status:** accepted (2026-08-13, user approval)
> **Date:** 2026-08-13
> **Related plan(s):** [0087 — the line renderer draws a curve](../plans/done/0087-the-line-renderer-draws-a-curve.md)
> **Supplements:** [ADR-0007](0007-line-geometry-generators.md),
> [ADR-0041](0041-line-joins-are-per-endpoint-on-the-segment-instance.md),
> [ADR-0079](0079-the-mandala-interior-is-rings-of-motifs-inside-star-pattern.md)

## Context

Every curve this engine draws through a line scene is a **parametric outline sampled to straight
segments** and expanded into instanced camera-facing quads (`renderer.rs:1`). That covers four
scene families with one implementation and it has been the right primitive for four-and-a-half
plans. At ornament scale it stops working, and the evidence is a user verdict twice over rather
than a measurement:

> *"maximally lame — all lines are half transparent, line connections are visible, there is no curve
> lines"* — and after a retune to solid strokes at `glow = 1.0` with no trails, *"we don't have
> curves, anything curved is based on several lines, and it's easy to see them — lines look
> upscaled and half baked"*.

All three ring-mandala presets were retired on that verdict
([backlog 0073](../design-backlog.md)), and the look shipped instead as `fragment_mandala` — a
Gray-Scott field's **analytic iso-contours**, curves evaluated per pixel in a shader with no
geometry and therefore no vertex at any resolution. That is the existence proof: per-pixel curves
read as curves in this engine, on this composite, today. What has no route is a per-pixel curve in a
*line* scene.

**Two distinct defects sit behind the one verdict, and they have different mechanisms.**

- **Faceting.** `Motif::vertex_count` (`star.rs:665`) is a constant per variant —
  `SMOOTH_SAMPLES` for circle/petal/teardrop, `TREFOIL_SAMPLES` for the rose — and there is no
  authorable resolution. At motif `scale` 0.13–0.46 a `circle` is visibly a polygon.
- **Vertex beads.** Joins are *not* missing; they work exactly as
  [ADR-0041](0041-line-joins-are-per-endpoint-on-the-segment-instance.md) specifies. Each joined
  endpoint extends its quad **backward or forward by the half-width** (`renderer.rs:129`), so
  adjacent quads deliberately overlap by half a stroke on both sides of every shared vertex. The
  composite is **additive**, so that overlap sums, and a vertex renders brighter than the strokes it
  joins. The bead is a consequence of the join mechanism working, not of it failing — which is why
  [backlog 0073](../design-backlog.md)'s own hedge ("verify against what Plan 0040 landed before
  assuming joins are absent") resolves in favour of the joins.

Raising the sample count attacks the first and *worsens* the second per unit length, while spending
against `TierConfig::max_segments` — the tier cap a floor-tier machine already reaches on a dense
mandala. So the two defects are not both reachable from the same lever, and the lever that exists
points the wrong way on one of them.

There is also a **decision already taken and unbuildable**. [ADR-0079](0079-the-mandala-interior-is-rings-of-motifs-inside-star-pattern.md)
left open whether the reference image's scalloped outer boundary is a ring of touching motifs or a
separate boundary curve. Plan 0065 Phase 2 rendered it both ways, the user was shown explicitly that
side B was 40 overlapping `arc` motifs faking continuity, and **chose the real primitive anyway**
([backlog 0071](../design-backlog.md)). `star.rs:599` records that in the shipped code: *"It is an
approximation and the user chose the real thing… the engine does not have [a boundary curve
primitive]. Nothing here fakes one."* That choice has had no route to being built for a week.

## Decision

We will add a **second instance kind to `LineRenderer`: a circular-arc instance whose stroke is a
signed distance evaluated per pixel in the fragment shader**, drawn as one bounding quad, alongside
today's `SegmentInstance` and in the same additive pass. An arc carries a centre, a radius, an
angular span and a half-width; the fragment shader's distance to it is `abs(length(p - c) - r)`
inside the span and the distance to the nearer endpoint outside it — a handful of ALU operations,
exact, and independent of resolution.

**A curve is expressed as a G1-continuous chain of arcs (a biarc chain), not as an exact fit.** This
is the load-bearing half of the decision and the reason the primitive can be this cheap. A sampled
polyline shows its joints because it is only **C0** — the tangent jumps at every vertex, and the eye
reads a tangent discontinuity as a corner however fine the sampling. A biarc chain is **G1**: the
tangent is continuous across every joint by construction. So the same handful of pieces that reads
as a faceted polygon reads as a drawn curve, and the approximation error shows up as a curve very
slightly in the wrong place rather than as a visible vertex.

Three consequences fall out of that and they are the shape of the change:

- **The circular family becomes exact.** `circle` is **one** arc instance with zero joints, where it
  is `SMOOTH_SAMPLES` segments and `SMOOTH_SAMPLES` beads today. `arc` is one. The scalloped
  boundary [backlog 0071](../design-backlog.md) asks for *is* a chain of arcs, so it stops needing a
  new mechanism and becomes a roster entry.
- **Everything else is biarc-fitted**, at a piece count chosen for tangent error rather than for
  chord error, which is a far cheaper budget: `petal`, `teardrop`, `trefoil`, and — the reason this
  reaches past the motif roster — `parametric_curve`'s roses and `lsystem`'s stems.
- **The bead count collapses to the number of genuinely different curves**, not the number of
  samples. It does not reach zero: two arcs meeting still overlap by ADR-0041's half-width, and
  under an additive composite that still sums. What changes is that a circle has one joint instead
  of `SMOOTH_SAMPLES`, and a rose has as many as it has lobes.

We are **not** reopening the additive composite ([ADR-0018](0018-engine-wide-scene-compositing.md) /
[ADR-0056](0056-additive-scenes-emit-premultiplied-alpha.md)). The arc instance emits premultiplied
alpha in the same pass with the same blend state; the bead is reduced by having fewer joints, not by
changing what a joint does. The two-tone / occlusion question stays where it is, in
[backlog 0069](../design-backlog.md).

## Consequences

### Positive

- **Resolution independence, which is the actual ask.** A per-pixel distance field has no vertices,
  so a `circle` is round at motif `scale` 0.13 and at full frame, on a 96×96 gate capture and at
  2048×1152. The faceting cannot come back at a scale nobody tested.
- **The tier budget improves rather than degrades.** A `circle` motif goes from `SMOOTH_SAMPLES`
  instances to one. `Motif::segments()`'s budget arithmetic — the thing that makes a 40-member ring
  reach the floor tier's cap — gets an order of magnitude of headroom back, which is what would let
  a dense mandala ship at an honest tuning rather than at the washed-out one the coverage floor used
  to force.
- **[backlog 0071](../design-backlog.md) becomes buildable without a second mechanism.** The user's
  decision was for a real boundary curve; a closed scalloped outline is an arc chain, so it is a
  roster entry on the primitive this ADR adds rather than its own feature.
- **It reaches past the motif roster.** `parametric_curve` and `lsystem` draw through the same
  `LineRenderer`, so a biarc path benefits every line family — which is the same leverage
  [ADR-0083](0083-in-frame-geometry-is-measured-at-the-line-renderers-draw-seam.md) got by measuring
  at the same seam.
- **The distance field is exact and therefore machine-independent.** `abs(length(p - c) - r)` has no
  iteration, no root solve and no tolerance, so there is nothing for a software rasterizer to
  disagree about beyond the ordinary float noise the golden suite already budgets for.

### Negative

- **A second instance kind is a real widening of `LineRenderer`.** It gains a second buffer, a
  second pipeline and a second draw call, in the one module four scene families share. That is the
  cost, it is not recoverable by cleverness, and the guard against it sprawling is that the arc
  carries *only* what an arc needs — no per-instance branching on shape kind, no third variant added
  later without its own ADR.
- **Biarc fitting is new CPU work with an error budget nobody has yet set.** It runs at rebuild time,
  not per frame (the same place `hankin::star_rosette` and `turtle::normalize_fit` already run), but
  a fit that is cheap and a fit that is *good* are different programs, and the piece count is a
  quality/cost knob this ADR does not pin.
- **The bead does not go away, and this ADR must not be read as claiming it does.** Two arcs meeting
  still overlap additively. On a rose with six lobes there are six joints and they will be brighter
  than the stroke. If that still reads as beading at ornament scale, the remaining route is the
  composite question in [backlog 0069](../design-backlog.md), and this decision will have bought a
  factor rather than a fix.
- **`ADR-0083`'s in-frame geometry measurement has to learn the new kind or go blind.** The `geom`
  column measures drawn segment length inside the target at the `draw` seam; an arc that contributes
  nothing to it would silently shrink the denominator and make every arc-drawing preset read
  better-framed than it is. This is a correctness obligation of the change, not a followup.
- **Every golden baseline that draws a circular motif will move**, because the picture genuinely
  changes — that is the point. A re-bless is owed and it must be measured bless-to-bless against a
  control, per this repo's standing rule, since eight baselines drift from their committed bytes on
  the development box anyway.

### Neutral

- The arc is a strictly *additional* primitive: nothing forces a scene onto it. `spectrum`'s bars
  and `lsystem`'s straight stems stay segments, because a straight line is not a curve and a
  distance field would be strictly more expensive for it.
- ADR-0007's rejection of native wgpu line primitives is untouched and reinforced — the reason was
  backend-dependent width, and a distance field is width-exact by construction.

## Alternatives considered

### Alternative A — authorable sample resolution per motif

Expose segments-per-motif and raise the defaults. **Rejected because it moves the wrong two numbers
in opposite directions.** More samples reduces the chord error (the faceting) and increases the
number of additively-overlapping joints (the beading) at the same time, while spending against
`TierConfig::max_segments`, which a 40-member ring already reaches on the floor tier. It also does
nothing for [backlog 0071](../design-backlog.md)'s boundary curve, which the user chose in its
strong form specifically over an approximation built from more placed copies. It is the cheap option
and it buys the smaller half of one of the two defects.

### Alternative B — quadratic Bézier instances instead of arcs

A quadratic Bézier is the more general primitive and any sampled outline fits one piecewise, so it
would cover the roster and the rose families without a special case. **Rejected on per-pixel cost.**
The exact distance to a quadratic Bézier requires solving a cubic per pixel; on instances whose
bounding quads overlap across a dense ornament, on the NFR §1 60 fps iGPU floor, that is a
materially different budget from an arc's `length` and compare. The generality it buys is
recoverable anyway — a biarc chain approximates any smooth curve to a controllable tolerance with
tangent continuity, which is the property that made the arc sufficient. Worth revisiting only if a
look turns up that a biarc chain provably cannot hold.

### Alternative C — accept the ceiling and route curve looks to the analytic-field family

Declare placed outline geometry the wrong mechanism at ornament scale, document it, and send
curve-reading looks to `fragment_mandala`'s iso-contour route, which demonstrably works.
**Rejected because it answers the look and abandons the capability.** It leaves `star_pattern`'s
motif roster — shipped, tested and documented under ADR-0079 — with no user that reads as intended,
leaves `parametric_curve` faceted for every future rose, and closes a user decision
([backlog 0071](../design-backlog.md)) by declining it. It is the honest option if the arc primitive
turns out unaffordable, and it is what the plan falls back to if the measured cost fails the floor.

### Alternative D — fix the bead by changing the blend

Draw the stroke with a `max` blend, or accumulate coverage before compositing, so overlapping quads
stop summing. **Rejected as out of scope and larger than it looks.** It reopens ADR-0018 and
ADR-0056 for every additive scene, it needs a second pipeline in the exact place
[ADR-0058](0058-bind-group-layout-collisions-carry-evidence.md)'s WARP hazards live, and it fixes
the bead without touching the faceting — which is the defect the user named first. The bead's
reduction here is a side effect of drawing fewer joints, which needs no blend change at all.

## Notes

**The G1 argument is the whole reason the cheap primitive is enough, and it should be checked before
it is relied on.** The claim is that tangent continuity, not sample density, is what makes a chain
read as a curve. It is standard and it matches the evidence — `fragment_mandala`'s iso-contours are
smooth by construction and read; the polyline motifs are C0 and do not — but the plan renders the
comparison at ornament scale before it commits the fit, because a claim that a look will read is the
one thing no test in this repo settles.

**The three retired mandala presets are the standing regression target.** `star_mandala`,
`star_mandala_six` and `star_weave` survive in git history with their honest tunings, and Plan 0075
Phase 1 froze their coverage numbers (0.2442 / 0.2505 / 0.2544, 10/10/9 radial shells) as test
fixtures. They are the specific pictures this decision exists to make shippable, and re-rendering
them on the arc primitive is the closest thing to a before/after this change can have.
