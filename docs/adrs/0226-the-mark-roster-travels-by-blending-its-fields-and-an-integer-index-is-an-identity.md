# ADR-0226 — The mark roster travels by blending its fields, and an integer index is an identity

> **Status:** proposed
> **Date:** 2026-09-19
> **Related plan(s):** [0203](../plans/0203-the-figure-gains-the-levers-it-was-measured-to-lack.md)
> **Rests on:** [0084](0084-a-particle-marks-silhouette-is-a-signed-distance-function.md) (the roster),
> [0105](0105-the-mark-roster-becomes-a-fullscreen-distance-field.md) (the shared chunk),
> [0111](0111-the-shape-field-gains-a-scaled-copy-coordinate.md) (the second per-arm scalar);
> follows [0060](0060-star-pattern-variants-interpolate.md) and
> [0075](0075-ifs-family-morphs-in-singular-value-space.md)

## Context

Within one arm, morphing already works: `star_valley`, `star_curve` and `star_jitter` are clamp-only,
so a binding drives them continuously and the silhouette genuinely deforms with the music. Between
arms it is a cut — `mark_shape` rounds, deliberately, because ADR-0084's roster is five *identities*
and a fractional index selects no arm at all. So a preset can switch `star` to `heart` on a beat and
cannot travel between them; an eased binding steps.

This engine has twice decided the opposite for other rosters.
[ADR-0060](0060-star-pattern-variants-interpolate.md) makes the star-pattern variants interpolate,
and [ADR-0075](0075-ifs-family-morphs-in-singular-value-space.md) morphs the IFS family in
singular-value space — a considered answer to exactly the question *"these are discrete entries, so
how do you travel between them"*. The newest roster is the one without it, and the ask came from the
user directly, at `presets/shape_facet.toml`'s authoring: *"can we morph the shape with music"*.

Two constraints bound any answer. The particle path shares this chunk — `swarm` and `emitter` read
the same `mark_distance` — so whatever lands arrives there too and its default must be an exact
identity or every shipped `shape`-bearing preset moves
([ADR-0105](0105-the-mark-roster-becomes-a-fullscreen-distance-field.md)'s shared-chunk
consequence). And [ADR-0111](0111-the-shape-field-gains-a-scaled-copy-coordinate.md) adds a second
per-arm scalar, `r_boundary`, so a morph has to blend *both* the distance and the boundary radius or
be defined for the distance alone.

The honest part: a naive lerp between two signed distance functions is **not** a shape in any
principled sense. The result is a field whose zero set is a blend neither arm describes and whose
interior may not even be connected. It is also a well-known technique that often looks good, which
is the kind of trade an ADR records rather than leaving for someone to discover in a preset.

## Decision

We will let `mark_shape` travel: a fractional index blends the two neighbouring arms' distance
fields, and blends `r_boundary` alongside, so the figure crosses between silhouettes continuously.
An **integer index is an exact identity** — the same field the rounded selector produces today,
bit for bit — so every shipped preset and every baseline is unmoved, on both the shape-field and the
particle paths. We accept that the intermediate field is not a valid distance function: its zero set
is a blend, its gradient magnitude is not 1, and anything that reads the field as a *distance*
rather than as a sign — a bevel, an outline width, an accuracy claim — is undefined mid-travel and
must say so where it is written.

## Consequences

### Positive
- The user's question is answered where it was asked: a phrase can carry a figure from a heart to a
  star instead of cutting between them on a beat.
- The roster stops being the odd one out. Two other rosters in this engine already interpolate, and
  the reasoning for this one is now in the same place as theirs.
- The particle path gets it for free, since it reads the same chunk — the shared-chunk consequence
  working in the useful direction for once.

### Negative
- **The intermediate is not a shape anyone designed.** Between two arms the silhouette can pinch,
  disconnect or grow a lobe that neither arm has. Nothing can assert it is good; the look gate is
  the only judge, and an author who dislikes a particular pairing has no lever but to avoid it.
- **Field-accuracy claims stop holding mid-travel.** A jittered star's exterior is already the
  roster's least accurate field — up to 0.54 out, measured at Plan 0091 Phase 5 — and a blend has no
  bound at all. Any consumer that treats the field as a metric distance has to be found and made to
  say what it assumes.
- **Two arms are evaluated instead of one** at any fractional index. The cost is a fragment-shader
  one on the field path and a per-mark one on the particle path; at integer indices it must be the
  same work as today, which is a thing the implementation has to arrange rather than get for free.

### Neutral
- The rounding is not removed, it is the integer case of the blend. `mark_shape`'s documented
  meaning changes from "an identity" to "a position on a roster", which the parameter reference
  regenerates.

## Alternatives considered

### Alternative A — Cross-fade the two marks instead of the fields
Draw both arms and dissolve between them by alpha. It is principled — each frame shows two real
shapes — and it is not the thing that was asked for: a dissolve reads as two figures overlapping
rather than as one figure travelling, and on the particle path it doubles the draw for every mark
at any fractional index.

### Alternative B — Morph in a parameter space, as ADR-0075 does for IFS
The IFS family morphs in singular-value space because its members are matrices and the space is
meaningful. The mark roster's members are heterogeneous silhouettes — a star, a heart, a
teardrop — with no shared parameterisation to travel through. There is no such space to find here,
which is the decisive difference from the precedent this decision otherwise follows.

### Alternative C — Leave it a cut
Backlog 0101's own summary says the question that prompted it is already answered by the three
continuous star params, and that this is genuinely optional. Rejected because *"can we morph the
shape with music"* asked for travel between figures and got travel within one, and because the
identity default makes the cost of being wrong here close to zero.

## Notes

Raised as backlog 0101, 2026-08-16, by `preset-author` at `presets/shape_facet.toml`'s authoring.
That entry says whoever takes this should read
[ADR-0111](0111-the-shape-field-gains-a-scaled-copy-coordinate.md) first and that, if both are
wanted, they are probably one plan — which is why the boundary radius is in this decision rather
than deferred.
