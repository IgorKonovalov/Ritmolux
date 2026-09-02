# ADR-0158 — A joined end carries its own miter length, not a flag the shader expands by a half-width

> **Status:** accepted 2026-09-02 (Plan 0149)
> **Date:** 2026-09-01
> **Related plan(s):** [0149](../plans/done/0149-the-line-corners-stop-being-blunt.md)
> **Supersedes the geometry half of:** [ADR-0041](0041-line-joins-are-per-endpoint-on-the-segment-instance.md)
> (its per-endpoint *granularity* stands and is what makes this cheap; what changes is what the
> endpoint carries)
> **Supplements:** [ADR-0007](0007-line-geometry-generators.md) (the fixed-capacity instance buffer
> this grows), [ADR-0023](0023-golden-drift-guard-uses-frozen-fixtures.md) (the golden re-bless this
> forces)
> **Backlog entry closed:** [0134](../design-backlog.md)
> **Depends on:** [ADR-0160](0160-the-stroke-is-measured-where-the-screen-is-isotropic.md) — the
> measurement-space question this ADR raised as unresolved was answered, in the negative, by
> Plan 0149 Phase 2's stop gate. ADR-0160 makes the two spaces similar and this Decision correct
> as written; see the amended Negative bullet below. **Nothing in the Decision changes.**

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
let miter = width / (theta * 0.5).sin();
ext = if miter > MITER_LIMIT * width { width } else { miter };
```

with `MITER_LIMIT = 4.0`, and passes `0.0` for a free end.

**Corrected 2026-09-02, from Plan 0149 Phase 2's renders.** This block read
`(width / (theta * 0.5).sin()).min(MITER_LIMIT * width)` — a *truncated miter*, which leaves the
stroke reaching four half-widths past the vertex along its own direction. The far side of the limit
turned out not to be the untested edge this ADR assumed: measured on the shipped walks, **86.2 %**
of `curve_nightbloom`'s joints at `d = 29` are past `28.96` degrees, and **26.4 %** of the
`parametric_curve` default's, because a Maurer chord web is made of near-reversals. At those shares
the truncation is visible — the star arms' outer edges grow a burr at 1920x1080. Past the limit the
joint therefore takes `width`, the flat half-width: a **bevel**, which is what `stroke-miterlimit`
selects in SVG and what an unmitred joint always drew. The two arms are byte-identical on the
figures the miter exists for and differ only on the chord webs; the user chose the bevel from the
rendered comparison. The clamp is recorded here rather than deleted because it is what this ADR
argued for and what the measurement overturned.

Three properties carry the decision:

- **The producer is the only party that knows `theta`.** A segment does not know its neighbour's
  direction — that is the whole reason ADR-0041 put connectivity on the producer side, and the same
  argument applies one step further to the angle. The renderer stays a dumb primitive and gains no
  neighbour data, so ADR-0041's rejection of the shader-side miter is not reopened; it is routed
  around.
- **`0.0` is exactly "free end", so the flag is not lost, it is subsumed.** A producer that passes
  nothing is byte-identical to today, which is the property ADR-0041 chose its shape for and which
  keeps `spectrum`'s `Bars` and `RadialRing` baselines still. **Scoped to the miter**: it says this
  decision moves no isolated producer's picture, and nothing more.
  [ADR-0160](0160-the-stroke-is-measured-where-the-screen-is-isotropic.md) does move them on any
  non-square target — a bar is vertical by construction and takes the full aspect factor — for an
  unrelated reason, and this bullet is not a promise about that.
- **The miter limit is the near-180-degree rule ADR-0041 had to hand-wave.** `4.0` serves every
  corner down to `2 * asin(1 / 4) = 28.96 degrees` exactly; below that the joint takes the flat
  half-width and bevels exactly as it does today, which is the standard behaviour. The diamond's
  61.9-degree vertex needs a factor of **1.945** and is therefore served exactly, with the limit not
  engaged. **This bullet said the extension *clamps* to `MITER_LIMIT * width` and that the far side
  is a strictly smaller bevel than today's; both are corrected above** — a clamp is larger than
  today's extension, not smaller, and the far side is the dominant population on a chord web rather
  than an edge.

**Scope: the four line families, and not `warp_mesh`.** `SegmentInstance` has a seventh producer
this decision does not reach — `warp_mesh::draw`, the MilkDrop compatibility surface — and it keeps
the flat half-width at both of its call sites. Three independent reasons, any one sufficient:

- **The premise that supersedes ADR-0041 does not hold there.** This ADR's whole argument is that
  Plan 0114 took `DEFAULT_SOFTNESS` to `0.25` and left no blur for a blunt corner to hide in.
  `warp_mesh` does not draw at that value: it pins `MILKDROP_SOFTNESS = 1.0`, the pure quadratic
  falloff, which is precisely the regime ADR-0041 was reasoning about when it priced a mitred corner
  and a rounded one as indistinguishable. On that surface **ADR-0041 still stands on its own terms**,
  and it is not being superseded so much as left alone.
- **`draw::dots` has no angle to compute one from.** It emits a **zero-length** segment and uses the
  extension as a **cap**: the half-width at each end is what turns a degenerate quad into a round
  bead, and without it the mark is a sub-pixel dash that vanishes below 1080p (Plan 0108 Phase 4,
  design-backlog 0107). `theta` is undefined there, so `width / sin(theta / 2)` is not merely
  unwanted but meaningless.
- **It is a measuring instrument with a live measurement pending.** `warp_mesh` is judged against
  `foo_vis_milk2`, and Plan 0142 is about to write ADR-0113's third Outcome from readings taken on
  it. Moving its stroke geometry now perturbs the instrument between the question and the answer.

The rule that follows is one line: **the miter is a property of the four line families**
(`curves`, `parametric`, `turtle`/`lsystem`, `hankin`/`star`, `spectrum`), which draw at
`DEFAULT_SOFTNESS`. A producer drawing at some other softness passes `width` and keeps ADR-0041's
geometry.

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
- **The miter introduces a dependence on *which space* the angle is measured in, and the flat
  extension had none.** `width` is a constant: it is the same number at every orientation, so
  ADR-0041's geometry never had to ask whether the producer's space and the shader's agree. A miter
  is `width / sin(theta / 2)`, and the shader applies it along `dir` computed **after** the aspect
  divide, while the producer computes `theta` in **world** coordinates. If those two spaces are not
  similar, the length is wrong by a factor that varies with the corner's orientation — swept
  numerically at 16:9 for the diamond's 61.9-degree vertex, the world-space factor lands between
  **0.705x and 1.609x** of the aspect-corrected one. **It is identically 1.000x at aspect 1.0**,
  which is the square fixture this ADR's own measurement was taken on and the one the plan specifies.
  Whether the two spaces are in fact similar is not established here; the plan carries it as a stop
  gate with a measurement at a non-square target, because no fixture at 1:1 can answer it.

  **Answered 2026-09-01: they are not similar.** Plan 0149 Phase 2's stop gate ran on the Phase 1
  tree and measured a stroke's on-screen thickness against its orientation at four targets — the
  vertical/horizontal ratio is `1.0000` at 1000x1000, `1.5789` at 1280x800, `1.7843` at 1920x1080
  and `0.6333` at 800x1280. **The ratio tracks the aspect**, because the half-width is offset along
  a normal computed after the aspect divide, in a space where x is compressed relative to y. So a
  producer cannot compute this ADR's miter length from world coordinates alone, and the phase
  stopped as instructed.

  **The repair is not in this ADR.** Every option that keeps the clip-space metric changes what
  `SegmentInstance` carries — the neighbour direction, the bisector, or the aspect handed to
  producers that build at `configure` — and each was rejected on cost.
  [ADR-0160](0160-the-stroke-is-measured-where-the-screen-is-isotropic.md) removes the disagreement
  instead: the stroke moves into world space, `dir` becomes world-space, and the producer's
  world-space `theta` is then exactly the angle the shader extends along. **This Decision is correct
  as written and grows the instance by nothing.**
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

**Apply the miter to every `SegmentInstance` producer, `warp_mesh` included.** The consistent-looking
choice, and the one a reader of the Decision would assume. Lost on all three counts in the Scope
paragraph above, and decisively on the first: `warp_mesh` draws at `MILKDROP_SOFTNESS = 1.0`, so the
premise this ADR uses to supersede ADR-0041 is simply absent there. Applying it anyway would move
composite and MilkDrop goldens to fix a defect that surface does not have, and would give
`draw::dots` a `sin(theta / 2)` with no `theta` in it.

**Widen the constant — extend by `2 * width` instead of `width`.** Cheapest imaginable, one
character. Lost on being the same error with a different constant: it is exact at
`theta = 60 degrees` and wrong everywhere else, overshooting obtuse corners into the bright-bead
failure while still truncating sharp ones.
