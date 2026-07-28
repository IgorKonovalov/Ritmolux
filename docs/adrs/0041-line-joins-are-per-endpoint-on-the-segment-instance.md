# ADR-0041 — Line joins are a per-endpoint flag on the segment instance, not a global cap rule

> **Status:** proposed
> **Date:** 2026-07-28
> **Related plan(s):** [0039](../plans/0039-line-joins.md)
> **Supplements:** [ADR-0007](0007-instanced-quads-not-native-line-primitives.md) (the instanced-quad
> line primitive this extends), [ADR-0023](0023-golden-fixtures-are-frozen.md) (the golden re-bless
> this forces)
> **Backlog entry closed:** [0023](../design-backlog.md)

## Context

`LineRenderer` draws every segment as an independent quad built from **its own** perpendicular
(`core/src/render/scenes/lines/renderer.rs`, the `SHADER` const):

```wgsl
let nrm  = vec2<f32>(-dir.y, dir.x);
let base = mix(a_s, b_s, c.x);
let pos  = base + nrm * c.y * width;
```

Nothing joins consecutive quads. Where the stroke direction changes by `theta`, the two rectangles
share the centre point but their outer corners diverge, leaving a wedge of roughly
`width * tan(theta/2)` on the outside of the turn. It reads as a thin dark tick across the stroke at
**every** vertex — reported by a user watching `spectrum_ridge` full-screen, and confirmed in the
shader rather than inferred.

The artifact is not new; what changed is how visible it is. The three generator-driven line scenes
draw figures whose consecutive segments are near-collinear, so `theta` is small and the notch is
sub-pixel. `spectrum` with `layout = "polyline"` is the opposite: consecutive points are adjacent
frequency bands, which are genuinely uncorrelated. **Plan 0038 then made it worse by design** — the
`curve` lever exists to increase the height contrast between neighbouring elements, and on a
polyline height contrast *is* turn angle. The lever aggravates the artifact precisely by doing its
job, which is why this surfaced now.

**The constraint that shapes the whole decision: not every segment end is joined.** The five
producers of `SegmentInstance` disagree about connectivity, and the codebase contains all three
cases:

| Producer | Connectivity |
|----------|--------------|
| `curves.rs::maurer_rose` | chained polyline — every interior vertex is a joint |
| `lsystem.rs` | turtle walk with branch push/pop — chained, but broken at every branch |
| `hankin.rs::star_rosette` | pairs meeting at a shared petal tip (the `b` end of both); `m0`/`m1` are free |
| `spectrum` `Polyline` | chained |
| `spectrum` `Bars`, `RadialRing` | **isolated** — one segment per element, both ends free |

`SegmentInstance` is `{ a, b, color, width }` and carries **no connectivity information at all**. So
the renderer cannot currently distinguish a joint from a free end, and any fix that treats all ends
alike damages the isolated cases. Concretely, at `spectrum_comb`'s shipped `thickness = 13` the
half-width is `13 * 0.003 = 0.039`, so extending both ends of every quad grows a bar by `0.078`
against a resting length of about `0.13` — a **60 % increase at rest**, with bars protruding *below*
`baseline` (which breaks the `baseline = 0` centre-mirror that Plan 0038 just shipped) and ring
spokes growing inward through `radius` to fill the inner circle.

## Decision

**`SegmentInstance` gains a per-endpoint "this end is joined" flag, and the vertex shader extends the
quad along its own direction by the half-width only at a flagged end.** Connectivity is supplied by
the producer, which is the only place that knows it; the renderer stays a dumb primitive.

This buys a round-ish join for the cost of two multiply-adds in the vertex shader: adjacent quads
overlap by half a stroke on both sides of the joint, and the additive blend plus the existing
quadratic falloff fill the wedge. For a soft glowing stroke that is visually a round join, and it
needs no extra geometry, no extra draw call, and no change to the instance count.

The flag is **per endpoint, not per segment**, because the star rosette needs exactly one end joined
(`b`, at the petal tip) and both L-system branch starts and polyline outer ends need exactly one.

The decisive consequence, and the reason this shape was chosen over the simpler ones: **a producer
that flags nothing is byte-identical to today.** `spectrum`'s `Bars` and `RadialRing` layouts pass
no flags and their goldens do not move — the blast radius is exactly the scenes that actually have
joints, rather than every line scene in the set.

## Consequences

**Positive.**

- The notch disappears wherever a joint exists, on all four line scenes at once, from one change to
  the shared primitive.
- Free ends keep their exact current geometry, so bar tops still sit at `baseline + length`, ring
  spokes still start on `radius`, and the `baseline = 0` mirror still shares one foot line.
- `spectrum_ridge` can stop paying for the artifact. It currently ships `thickness = 4.2`, chosen as
  a compromise between hiding the notch and staying above `animation.rs`'s `0.01` motion floor — a
  preset distorted by an engine gap. Plan 0039 restores it.
- Cheap enough to be uncontroversial at runtime: no new instances, no new pass, and the extension is
  computed from data the vertex shader already has.

**Negative — these are the price.**

- **The instance format grows**, from 32 bytes to 36 (or to 32 with packing). `SegmentInstance` is
  `pub` and shared by five producers plus the mirror replicator in `lines/mod.rs`; every one must be
  updated, and a producer that forgets to flag its joints silently keeps the artifact rather than
  failing. Plan 0039 answers that with a test per producer, not with a comment.
- **Goldens re-bless for parametric_curve, lsystem, star_pattern and spectrum-polyline.** Per the
  known `LMV_BLESS` behaviour the bless is not scoped to the failing scene, so unrelated baselines
  must be restored before committing. This is the largest cost in the change and it is bookkeeping,
  not engineering.
- **Corners become slightly blunt.** Extending along each segment's own direction approximates a
  round join; it does not produce a sharp mitred corner. On the star rosette and L-system, tips that
  are currently crisp will soften by about a half-width. Accepted deliberately — see Alternatives.
- **A very sharp turn overshoots slightly.** At `theta` near 180 degrees the two extended quads
  overlap along almost their whole width, which under additive blend reads as a small bright dot at
  the joint rather than a gap. That is a better failure than the current one, but it is not nothing,
  and it is the case a miter limit would have handled.
- The flag describes connectivity the producer asserts, and nothing validates it. A producer could
  flag an end that is not actually shared, and the result would be a stroke a half-width too long
  with no error.

## Alternatives considered

**Extend every quad unconditionally (no flags).** The two-line change originally proposed in
backlog 0023, and the reason this ADR exists rather than a one-paragraph plan. It is wrong for every
isolated segment: spectrum bars grow ~60 % at rest and hang below their baseline, ring spokes fill
the inner circle. Lost on correctness, not on cost.

**A round cap on every end, unconditionally.** Same defect as above with a rounder profile — it
still lengthens every isolated segment by a half-width at both ends. Its one advantage is needing no
connectivity data at all. Lost for the same reason.

**A true miter join.** Geometrically correct sharp corners, and the better answer for the star
rosette and L-system considered alone. It needs each instance to carry its *neighbours'* endpoints
(8 floats to 12, a 50 % instance-buffer growth against ADR-0007's fixed-capacity no-alloc budget)
plus a miter-limit rule so near-180-degree turns do not shoot the corner to infinity. Lost on cost
and complexity for a soft additive stroke where the sharpness is barely visible: the falloff is
quadratic to the quad edge, so a mitred corner and a rounded one differ by less than the blur that
is already there.

**A round join drawn as a disc per interior vertex.** The textbook answer and the easiest to reason
about — one extra instanced quad per joint, discs hide every wedge regardless of angle. Rejected on
instance count: the rose draws up to `MAX_SEGMENTS = 20_000` segments, so this nearly doubles the
buffer and the per-frame upload for a primitive whose whole design point (ADR-0007) is a
fixed-capacity buffer that never allocates on the hot path. Worth revisiting only if the blunt
corners above turn out to matter.

**Do nothing and keep tuning presets around it.** Already tried, and it is what routed this here.
Stroke width is the only preset-side lever and it is weak: thinning `spectrum_ridge` far enough to
hide the notch drops the figure under the `animation.rs` motion floor. Raising `elements` makes it
*worse*, since more points across a fixed `span` shorten the x-step while the y-differences stay,
steepening every turn. The content lane cannot solve this, which is the definition of an engine gap.
