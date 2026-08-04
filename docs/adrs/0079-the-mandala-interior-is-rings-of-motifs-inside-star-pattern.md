# ADR-0079 — The mandala interior is rings of motifs, and it lives inside `star_pattern`

> **Status:** proposed
> **Date:** 2026-08-04
> **Related plan(s):** [0065](../plans/0065-the-mandala-interior.md)
> **Supplements:** [0007](0007-line-geometry-generators.md),
> [0060](0060-star-pattern-variants-interpolate.md)

## Context

One of the user's five reference images is not a fractal at all: it is a drawn ornamental mandala —
concentric rings of discrete repeated motifs, thin bright strokes on black, each ring with its own
count, radius, scale and phase. It is line geometry, and this project has a line scene that has been
waiting for exactly this specification.

[Design-backlog 0007](../design-backlog.md) recorded `star_pattern` as reading like "a hollow ring",
verified by a sweep rather than by argument: segments sit near the rim at every `contact_angle_deg`,
with no meaningful interior change across 12 / 20 / 28 degrees. The user's verdict on the scene was
"idea is interesting but looks poor", then — after preset-side mitigation — "very nice, but can we
make morphing between shapes easier, slower". That second ask was answered:
[ADR-0060](0060-star-pattern-variants-interpolate.md) and
[Plan 0054](../plans/done/0054-the-line-scenes-catch-up.md) made `variant` a continuous contact
angle. **The first ask — the hollow interior — was never decided and the entry is still live.** The
user's standing call on it, recorded 2026-07-26, was *invest, do not cut*.

So the design question is not whether to build a ring-of-motifs generator. It is where it lives:
inside the scene that has an unfilled interior and a standing investment decision, or in a new scene
of its own.

A Hankin rosette and a ring ornament are genuinely different geometry. The first is an interlace
derived from a tiling's edge midpoints at a contact angle; the second is a placement rule — put `k`
copies of a motif at radius `r`, rotated by `2πi/k + φ`. Neither derives from the other. That is the
case for a new scene, and it is why this is a decision rather than an obvious call.

## Decision

We will add the ring generator **inside `star_pattern`**, as an optional `rings` array on its
existing `[generator]` table ([ADR-0007](0007-line-geometry-generators.md)'s declarative config
seam), alongside the Hankin parameters rather than replacing them. A preset may draw the interlace
alone (today's behaviour, and the default when `rings` is absent), the rings alone, or both
composited — the rings as an interior inside the interlace's rim is the composition backlog 0007
asked for, and the reference image is the rings alone.

Three reasons decide it against a new scene. The scene **has** the defect this fixes, and a new scene
next to it would leave `star_pattern` hollow and the backlog entry open — the investment decision
would have been made and then not spent. The scene has capability the ornament wants and would
otherwise be duplicated: `variant`'s continuous contact-angle morph, its `[palette]` reach from Plan
0054, and the shared `LineRenderer`'s stroke, join and glow surface. And the two geometries share
their only structural property, **n-fold rotational symmetry about the frame centre**, which is what
lets a ring count and a Hankin fold order be chosen together rather than fought.

The motif roster is a **closed, curated set** chosen from rendered samples, not an open grammar. Each
motif is a parametric outline sampled to segments — the same thing `parametric_curve` already does,
placed rather than drawn once.

## Consequences

### Positive

- **Backlog 0007's open half closes**, and the "invest, do not cut" decision is finally spent on
  something specific rather than on a list of possibilities ("more tilings, an off-centre mirror, or
  drawing the underlying tiling grid" — the options the lane named and nobody chose between).
- **The reference image becomes authorable directly.** Rings, counts, radii and motifs are exactly
  the vocabulary the picture is made of, so a preset reads like a description of it.
- **Per-ring audio binding is a strong look for free.** Counter-rotating adjacent rings, radii
  breathing on the beat, ring phase driven by band level — all of it is placement arithmetic on a
  scene that already eases its parameters.
- **The segment budget is comfortable and can be stated:** `TierConfig::max_segments` is **20 000**
  at `Floor` and 60 000 at `Rich`. A dense mandala of 8 rings × 32 motifs × 24 segments is 6 144 —
  well under, with room for the interlace on top.
- **No new scene, no new `SystemKind`, no new render idiom** — the ninth scene slot is not spent, and
  the exhaustive `SystemKind::ALL` factory is untouched.

### Negative

- **`star_pattern`'s config surface roughly doubles**, and half of it has nothing to do with Hankin
  tilings. A reader of `[generator]` will find contact angles and ring rosters side by side with no
  structural relationship, which is a genuine coherence cost and the strongest argument the rejected
  alternative had.
- **The motif roster is closed, so a look outside it routes back through `architect` + `dev`.** That
  is the same boundary [ADR-0017](0017-preset-author-skill-lane.md) draws everywhere, but it will
  bite here more than usual: ornament is exactly the domain where an author wants one more shape.
- **Two geometries in one scene means two ways to be slow.** The interlace's segment count grows with
  the tiling; the rings' grows as rings × count × resolution. A preset can exhaust the cap from
  either direction, and the failure (silent truncation at the cap) is the same in both.
- **Concentric thin strokes are the worst case for the animation gate.** `core/tests/animation.rs`
  renders at 96×96 and a rotationally symmetric figure is nearly invariant under rotation — the
  penalty [design-backlog 0009](../design-backlog.md) documented for `star_rosette` applies here with
  more force, since a ring mandala is *more* rotationally symmetric than the rosette was. Ring radii
  or counts must carry the animation, not spin alone.

## Alternatives considered

### Alternative A — a new `mandala` scene

Give the ornament its own `SystemKind` with a config surface designed only for it.

Rejected because it leaves the defect it was motivated by. `star_pattern` would stay hollow, backlog
0007 would stay open, and a user who chose "invest, do not cut" would have got a new scene instead of
the investment. It also duplicates `variant`'s continuous morph, the palette reach and the stroke
surface for the sake of a coherence gain that a doc-comment can mostly buy. Reconsider only if the
config surface proves genuinely unreadable in practice.

### Alternative B — fill the interior from the Hankin tiling itself

Draw the underlying tiling grid, or derive interior strokes from the same tiling that produces the
rosette — one of the three options the content lane named.

Rejected because it does not produce the reference. The interior of a Hankin interlace is structural
and regular; the reference's interior is a *sequence of distinct decorative rings*, each with its own
motif and count. Deriving it from the tiling would give a denser interlace, which is a different
picture and one the lane's sweep already suggests reads as more of the same.

### Alternative C — an open motif grammar

Let a preset author describe motifs as expressions or polylines rather than choosing from a roster.

Rejected on the same ground [ADR-0077](0077-the-symmetry-stage-owns-one-coordinate-map.md) rejects an
authorable domain warp, one level up: it is a drawing language, not a parameter. It also has no
natural stopping point — once motifs are authorable, so are their fills, their joins and their
nesting — and the project already has a curve grammar (`parametric_curve`) that a future motif can be
promoted from.

## Notes

The reference image's outer boundary is a scalloped closed curve rather than a ring of separate
motifs, which is a third element beyond rings and interlace. It is deliberately not decided here:
whether that is a motif ring whose members touch, or a separate boundary curve, is a question for
the rendered sample set rather than for this ADR.
