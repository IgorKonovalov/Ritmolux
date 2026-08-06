# ADR-0079 — The mandala interior is rings of motifs, and it lives inside `star_pattern`

> **Status:** **accepted 2026-08-06** (with an Outcome section)
> **Date:** 2026-08-04
> **Related plan(s):** [0065](../plans/done/0065-the-mandala-interior.md)
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

## Outcome (2026-08-06, at Plan 0065's close)

Everything above shipped as designed: the roster closed at **seven** motifs (`star` and `triangle`
were cut from the sampled nine — one dissolves into texture by x32, the other duplicates `chevron`
at twelve times the segment cost), `rings` is optional so a bare interlace is unchanged segment for
segment, and the budget claim held with room to spare. Three things the decision did not anticipate,
recorded because they are what a reader would want to know before extending this.

**The closed-roster consequence fired on the very first content pass, not eventually.** The
open-question above — is the scalloped boundary a ring of touching motifs, or its own primitive —
went to the sample set as planned, and the user chose **the primitive**, having been shown the
approximation side by side and told it was one. So the roster's stopping point was reached the same
day it was drawn, and the engine gap is filed as
[design-backlog 0071](../design-backlog.md). That is this ADR's "routes back through `architect` +
`dev`" consequence working, but it is worth knowing it cost a round trip immediately rather than
after several presets.

**Filling the interior made the scene measure *sparser*, not denser.** `star_pattern`'s coverage
floor in `core/tests/sanity.rs` had to be re-derived downward, `0.34` → `0.12`, because the three
mandala presets score `0.2442`-`0.2544` where the bare interlace they fill scores `0.6908`-`0.7995`.
The figure is unambiguously fuller; the pixel statistic is lower, because at that test's 96x96
capture a hairline over a 46-fold ornament aliases to almost nothing and `coverage` there measures
halo and trail rather than geometry. Measured, not inferred: the bare rosette and the 46x-denser
mandala score *identically* at one tuning, and 54 % more geometry moves the number 2.6 %. The
re-derivation is Plan 0065 Phase 7; the measure's replacement is
[design-backlog 0072](../design-backlog.md), still open.

**The interlace-plus-rings composition survived the judgement that nearly cut it.** The reviewing
session read the twelve-fold interlace as a separate coarse figure sitting on top of the ornament
rather than framing it, and recommended against shipping it; the user kept it, and on the rendered
result it reads as a star frame around the ornament. Both compositions ship — `star_mandala` is the
ornament alone, `star_weave` the roster inside the interlace — so backlog 0007's composition
question is answered rather than dropped.

### Outcome addendum (2026-08-06, same day) — every preset built on this decision was retired within hours

The three compositions named above no longer exist. The live judgement Plan 0065's Phase 6 deferred
happened immediately after the close and came back against the **mechanism**, not against any tuning
or composition choice: *"we don't have curves, anything curved is based on several lines, and it's
easy to see them — lines look upscaled and half baked."* Every motif in the closed roster is a
parametric outline sampled to straight segments, so at ornament scale the vertices are visible, a
circle reads as a polygon, and additive overlap beads every joint. The solid-stroke retune had
already run, which rules out the inflated-glow explanation the backlog had been holding open
([design-backlog 0073](../design-backlog.md)).

**This does not supersede the decision, and deliberately so.** Everything the Decision section claims
is still true and still shipped: `rings` lives inside `star_pattern` rather than beside it, the scene
is no longer hollow, the roster is closed at seven, the segment budget holds, and the interior
measurement (bare rosette **1 of 10** radial shells, four-ring mandala **9 of 10**) is a property of
the geometry rather than of any preset. The Context section's premise — that a ring ornament and a
Hankin interlace are different geometry sharing one symmetry — is unaffected.

**What is now known, and what a superseding ADR would have to be about.** The unexamined assumption
was not *where* the ring generator lives but *that placed outline geometry can render an ornament at
all* through a stroke renderer that draws instanced quads over sampled polylines. It cannot, at this
scale. The mandala look ships instead as `presets/reaction_gilt.toml`: a Gray-Scott field's
**analytic iso-contours** — evaluated per pixel, no geometry anywhere in the picture, therefore no
vertex at any resolution — folded into a 10-to-18-wedge rosette by `kaleido_order`, with
`kaleido_edge = 0` so it reads as an object on black. The symmetry became a composite-stage property
instead of a placement rule.

So the honest status of this ADR is: **accepted, delivered, and currently unused.** No preset in the
library binds `rings`. Whether that capability earns its config surface without a shipped user is a
real question and belongs to whoever next touches `star_pattern` — it is not answered here, because
one rejected look is not enough to retire a tested capability.
