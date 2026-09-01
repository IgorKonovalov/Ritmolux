# ADR-0158 — A joined end carries its own miter length, not a flag the shader expands by a half-width

> **Status:** proposed
> **Date:** 2026-09-01
> **Related plan(s):** [0149](../plans/0149-the-line-corners-stop-being-blunt.md)
> **Supersedes the geometry half of:** [ADR-0041](0041-line-joins-are-per-endpoint-on-the-segment-instance.md)
> (its per-endpoint *granularity* stands and is what makes this cheap; what changes is what the
> endpoint carries)
> **Supplements:** [ADR-0007](0007-line-geometry-generators.md) (the fixed-capacity instance buffer
> this grows), [ADR-0023](0023-golden-drift-guard-uses-frozen-fixtures.md) (the golden re-bless this
> forces)
> **Backlog entry closed:** [0134](../design-backlog.md)

## Context

[ADR-0041](0041-line-joins-are-per-endpoint-on-the-segment-instance.md) gave `SegmentInstance` a
per-endpoint `joined` bitfield and had the vertex shader extend a flagged end along its own
direction by exactly the half-width:

```wgsl
let ext_a = select(0.0, width, (joined & JOINED_A) != 0u);
```

**A half-width is the right extension for exactly one interior angle: 180 degrees.** A corner of
interior angle `theta` needs `width / sin(theta / 2)` to bring both quads' outer edges to the same
point. The shipped constant is the `theta -> 180` limit of that expression, so the shortfall grows
without bound as a corner sharpens, and the corner truncates to a flat bevel.

Measured 2026-08-27 (backlog 0134), a single `diamond` filling a 1000x1000 frame at
`thickness = 9`: the profile through the 61.9-degree vertex is **26 px of flat 185 and then zero,
with no taper at all**, and the corner patch reads **1.38x the stroke's own value** on the inner
side where the two quads sum. Both halves are the same missing factor —
`1 / sin(30.95 deg) = 1.945`.

**ADR-0041 saw this and priced it as invisible, on a premise that no longer holds.** It rejected a
true miter with *"a mitred corner and a rounded one differ by less than the blur that is already
there"*, and listed "corners become slightly blunt" as an accepted negative. That reasoning rested
on the quadratic falloff to the quad edge.
[Plan 0114](../plans/done/0114-the-line-stroke-reads-as-a-drawn-line.md) took `DEFAULT_SOFTNESS`
to `0.25`. **There is no blur left to hide it in**, and the defect arrived as a user complaint on
the running app at [Plan 0087](../plans/done/0087-the-line-renderer-draws-a-curve.md) Phase 7:
*"how straight lines are connected, its clearly visible and doesn't look solid"*.

The defect is also **concentrated rather than diffuse**. Plan 0087 converted the curved motifs to
arcs, and an arc has no interior joints at all. What is left on the segment path is the straight
figures — `diamond` and `chevron` are precisely the two roster members that were *not* converted.

ADR-0041's own rejection of a true miter named a second cost that this decision has to answer:
a miter needs each instance to carry its **neighbours' endpoints** (8 floats to 12, a 50 % instance
growth) plus a miter-limit rule. That is true of a miter computed **in the shader**. It is not true
of one computed by the producer.

## Decision

**A joined end carries the miter length the producer computed for it, as an `f32` in world units,
and the shader extends by that number.** `SegmentInstance.joined: u32` becomes
`SegmentInstance.ext_a: f32`, in place at shader location 4; `ext_b: f32` is appended **last**, at
location 6. The instance grows **40 bytes to 44**, and `alpha` keeps location 5 and its byte offset.

The producer computes

```rust
// illustrative
ext = (width / (theta * 0.5).sin()).min(MITER_LIMIT * width)
```

with `MITER_LIMIT = 4.0`, and passes `0.0` for a free end.

Three properties carry the decision:

- **The producer is the only party that knows `theta`.** A segment does not know its neighbour's
  direction — that is the whole reason ADR-0041 put connectivity on the producer side, and the same
  argument applies one step further to the angle. The renderer stays a dumb primitive and gains no
  neighbour data, so ADR-0041's rejection of the shader-side miter is not reopened; it is routed
  around.
- **`0.0` is exactly "free end", so the flag is not lost, it is subsumed.** A producer that passes
  nothing is byte-identical to today, which is the property ADR-0041 chose its shape for and which
  keeps `spectrum`'s `Bars` and `RadialRing` baselines still.
- **The miter limit is the near-180-degree rule ADR-0041 had to hand-wave.** `4.0` serves every
  corner down to `2 * asin(1 / 4) = 28.96 degrees` exactly; below that the extension clamps and the
  corner bevels as it does today, which is the standard behaviour and a strictly smaller bevel than
  the current constant produces. The diamond's 61.9-degree vertex needs a factor of **1.945** and
  is therefore served exactly, with the limit not engaged.

`MITER_LIMIT = 4.0` is SVG's `stroke-miterlimit` default. It is adopted because it is a published,
widely-implemented choice for the same geometric problem, not because it was measured here — and
the plan states it as a named constant with that provenance rather than as a tuned number.

## Consequences

**Positive.**

- The corner reaches its point. Both halves of the measured defect — the flat bevel and the 1.38x
  inner-side sum — are the same missing factor, so one change addresses both.
- **The instance shrinks its growth relative to every rejected alternative**: +4 bytes against the
  +16 a shader-side miter needs and against the near-doubled *instance count* a disc-per-vertex
  needs. ADR-0007's fixed-capacity budget is preserved.
- **The join bits stop existing, and with them a whole class of hazard.** ADR-0041's Outcome
  records generating `const JOINED_A` / `JOINED_B` into the WGSL from the Rust constants so a
  renumbering is unrepresentable, plus an assertion that catches a hand-written *swap*. A float
  extension has no bits to renumber; that machinery is deleted rather than maintained.
- The producer-side computation is testable on the CPU, where `theta` is available and an angle
  can be asserted directly, rather than only through a rendered frame.

**Negative — these are the price.**

- **Every line golden with an interior joint moves.** `parametric_curve`, `lsystem`, `star_pattern`
  and spectrum-polyline all re-bless. Per the known `LMV_BLESS` behaviour the bless is not scoped to
  the failing scene, so unrelated baselines must be restored before committing. This is the largest
  cost in the change and it is bookkeeping, not engineering.
- **Four producers must each learn to compute an angle**, and a producer that computes it wrongly
  now renders a *wrong-length* stroke rather than merely keeping the notch. That is a worse failure
  mode than ADR-0041's "forgets to flag", and the plan answers it with a per-producer test rather
  than a comment.
- **The instance grows 10 %.** At Floor's `max_segments = 20_000` that is +80,000 B of per-frame
  upload against the buffer's own 800,000 B; at Rich's 60_000 it is +240,000 B against 2,400,000 B.
  Small, and not free.
- **A closed chain's two ends still have no neighbour.** The rosette is closed and every vertex is
  a joint, but a polyline's first and last points are genuinely free and stay bevelled, which is
  correct and is what the existing first/last-point assertion pins.
- **The extension is in world units, so it does not track a per-frame `width` change on its own.**
  Producers recompute per frame anyway — they rebuild the instance buffer — but a future producer
  that caches instances across frames while animating `thickness` would desynchronize the two. The
  plan records this on the field.

## Alternatives considered

**Keep the flag and fix nothing (ADR-0041 as it stands).** Lost to its own revisit condition: it
accepted blunt corners with *"worth revisiting only if the blunt corners above turn out to matter"*,
and they now have — as a user complaint, on shipped presets, against a softness that no longer
blurs them.

**A round join drawn in the fragment shader.** Attractive because it appears to need no instance
growth. It does not deliver that: the fragment still cannot see the neighbour's direction, so the
vertex stage must carry it anyway, and the quad must additionally be grown to contain the round cap
it wants to draw. Lost on paying most of the cost for a softer corner than the figures ask for —
`diamond` and `chevron` want a point.

**ADR-0041's disc per interior vertex.** The textbook answer and the easiest to reason about. Lost
again, and for the reason it lost the first time: one extra instanced quad per joint nearly doubles
a `MAX_SEGMENTS = 20_000` buffer whose whole design point (ADR-0007) is fixed capacity with no
hot-path allocation. +4 bytes per existing instance beats +100 % instance count.

**A true miter computed in the shader, from neighbour endpoints on the instance.** ADR-0041's
original rejection, and still rejected: 8 floats to 12 is a 50 % instance growth against this
decision's 10 %, and it duplicates in WGSL an angle the producer already has in Rust.

**Widen the constant — extend by `2 * width` instead of `width`.** Cheapest imaginable, one
character. Lost on being the same error with a different constant: it is exact at
`theta = 60 degrees` and wrong everywhere else, overshooting obtuse corners into the bright-bead
failure while still truncating sharp ones.
