# ADR-0111 — The shape field gains a scaled-copy coordinate beside its distance one

> **Status:** proposed
> **Date:** 2026-08-16
> **Related plan(s):** [0098](../plans/0098-the-figure-nests-properly.md)
> **Closes:** [design-backlog 0096](../design-backlog.md)

## Context

[ADR-0105](0105-the-mark-roster-becomes-a-fullscreen-distance-field.md) made `shape_field`'s scalar
a **distance**, and said so as the feature: *"a band of the palette coordinate is a band of constant
distance, which is the definition of an offset curve."* It delivers exactly that, and
`presets/shape_pulse.toml` ships from it.

**The construction users reach for is not an offset family.** Two batches of reference images have
now arrived for this scene, and the nested-heart one — the subject the user asked about twice and
iterated on across four rounds — has inner rings that stay sharply heart-shaped down to a small
core. That is **self-similar scaled copies**, and no offset family can produce it.

The reason is geometric rather than a tuning shortfall. An inward offset is an **erosion**, and
erosion **rounds a reflex corner while keeping convex ones sharp**. On the heart that means the
bottom point stays crisp and the top notch fills in as the contours move inward — which is exactly
the artifact the content lane was asked to remove and could not.

**The cost is a hard coupling.** The innermost band's boundary sits at
`d = ((1/palette_steps) / color_span)^(1/gamma)`, so a notch sharp enough to read needs
`palette_steps * color_span ~ 1` — which leaves **one** band inside the figure. Measured at 9 steps
on the heart, and the last row was rendered and judged in the running app:

| core sits at | notch rounding | rings inside the figure | verdict |
|---|---|---|---|
| 0.48 | 0.33 | four | what ships today |
| 0.68 | 0.20 | two | |
| 0.81 | 0.12 | one | |
| 0.91 | 0.06 | none — a black heart with a red rim | **user rejected it** |

So "many rings inside **and** a small sharp core" is unreachable, and it is the whole of the
reference's construction.

## Decision

**`shape_field` gains a second coordinate mode, selected per preset, that hands the palette
`s = r / r_boundary(theta)` instead of the normalized distance.** For a region star-shaped about
its centre that ratio is `0` at the centre and exactly `1` on the outline — the same contract the
distance already honours — but its level sets are **scaled copies of the outline**, so a band of
the coordinate is a scaled figure rather than an offset one.

`r_boundary(theta)` is computed **per arm in closed form**, not by marching the existing SDF. Each
of the four star-shaped arms admits one: `disc` is `1`; `polygon` is `apothem / cos(f)` on the
folded angle; `star` is a ray-versus-segment intersection against the tip-valley edge the fold has
already selected; `heart` is a ray-versus-circle intersection against a lobe and a ray-versus-line
intersection against a tangent ray, chosen by the same branch its SDF already takes.

**The distance mode stays the default and stays bit-identical.** The selector's default value keeps
every shipped preset and every golden baseline on the arithmetic it has today.

## Consequences

### Positive

- **The reference construction becomes reachable**, and the coupling above dissolves: under the
  radius coordinate the ring count is `palette_steps` alone and the innermost figure is a scaled
  copy at any count, so notch sharpness stops trading against ring count.
- **`gamma` becomes a spacing control rather than a spacing-and-sharpness one.** It still decides
  where bands crowd, but no longer decides how rounded the inner figure is.
- **One shape vocabulary still.** The mode adds a function beside `mark_distance`, over the same
  closed roster and the same CPU-side quantizers, so a mark a particle wears and a figure this
  scene nests cannot drift apart — the property ADR-0084's chunk exists for.
- **It is cheap where it matters.** A closed form per arm costs a handful of ALU ops, the same
  order as the SDF it sits beside, which is what makes it a coordinate mode rather than a feature
  with a cost gate on it.
- **The figure's share of the palette becomes portable across shapes, and roughly doubles on the
  worst one.** Under the distance the exterior is divided by the shape's inradius, so a sharp star
  (inradius `0.093`) reaches `d = 26.8` at the frame corner and a preset keeping one sweep on frame
  is capped near `color_span = 0.037` — which leaves the figure's own `0..1` interior just **9.6 of
  the LUT's 256 texels**. Under the radius the interior is `0..1` for *every* shape by construction,
  and the same framing allows `color_span = 0.070`, or **17.9 texels**. That is a 1.9x improvement
  and it is worth stating honestly: it **mitigates** [design-backlog 0099](../design-backlog.md)
  rather than removing it, since the exterior range still varies with how elongated the figure is.
  The actual cure for that trap is `palette_steps`, which quantizes before the LUT read.

### Negative

- **`ring` is not star-shaped about its centre, so the mode is undefined on it.** An annulus's
  centre lies in its hole, and a ray from there crosses the boundary twice; `r / r_boundary` has no
  single value. One of the five roster arms therefore cannot take this coordinate, and the plan has
  to choose between falling back to the distance silently, refusing the combination with a load
  warning, or defining it against the outer edge and documenting that the hole is not expressed.
  **Whatever it picks, a preset can select a legal shape and a legal mode and get a third thing.**
- **`color_span` values do not transfer between the two modes**, and this is on top of a portability
  trap the same param already has. Under the distance the exterior is divided by the shape's
  inradius; under the radius it grows linearly in `r`. A preset switching modes has to re-tune its
  span, and nothing warns.
- **A second scalar doubles the per-arm surface.** Every future roster arm now owes an
  `r_boundary` as well as an `sd`, and the drift test that keeps the CPU mirror honest has to cover
  both. ADR-0084's closed roster is what keeps that bounded.
- **`palette_contour` changes character under the new mode.** It is an `fwidth` of whatever field
  it is given, and the radius field's gradient differs from the distance's — sharply so near the
  centre, where `r / r_boundary` varies fast. The hairline will not have the same weight it has
  today at the same `palette_contour` value.
- **It adds a second way to do something that already half-works.** `shape_pulse` fakes the ring
  *count* by packing stripes as gradient stops; that preset keeps working and now has a better
  route available, which is a documentation burden rather than a technical one.

### Neutral

- The exterior stays unbounded in both modes, and neither is a metric distance out there — the
  radius mode's exterior is a scaling factor, which is arguably the more useful of the two for
  contours since it is what makes outward rings scaled copies as well.

## Alternatives considered

### Alternative A — sphere-trace the existing SDF to find the boundary

Keep one scalar and find `r_boundary` numerically: march along the ray from the centre using the
signed distance, which converges for a star-shaped region. Fully generic, no per-arm math, and it
would extend to any future arm for free.

Rejected on cost and on honesty about it. It needs roughly 10-20 SDF evaluations per pixel against
the one the scene does today, fullscreen and unconditional — the same shape of cost ADR-0105
already flagged as this scene's weak point. A closed form exists for every arm in a **closed**
roster, so the generic machinery buys extensibility this project has deliberately declined
(ADR-0084) at a price it would have to measure and might not pass.

### Alternative B — reuse `kaleido_radial`

The engine already nests shrinking copies, periodic in `log r`. Point it at the shape field and let
it do the nesting.

Rejected because it nests the **frame** about a screen point, not the **figure** about its own
centre. It cannot follow `pan_x`/`pan_y`, it is periodic in radius rather than in the shape's own
metric, and it would nest the exterior field along with the figure. It produces concentric copies of
a *picture*, which is a different thing from level sets that are scaled copies of an *outline*.

### Alternative C — keep doing it in the palette

`shape_pulse` packs 18 stripes as gradient stops below the outline's coordinate, which produces the
ring count the reference has without any engine change.

Rejected as the general answer because it fakes the count and not the geometry: the level sets are
still offsets, so the inner figure still rounds off, which is the defect this ADR exists to remove.
It is also a 76-stop palette that has to be regenerated whenever the ring count changes.

### Alternative D — a composable SDF grammar

Let presets build and transform fields — scale, union, offset — in the expression language.
Strictly more powerful and it would subsume this.

Rejected on the same ground [ADR-0105](0105-the-mark-roster-becomes-a-fullscreen-distance-field.md)
rejected it: it turns the preset language into a shader language, which ADR-0002 has declined for
the project's whole life, and the want behind this decision is one coordinate on a closed roster
rather than a construction kit.

## Notes

The selector is numeric, like `shape` and `kaleido_edge`, because the expression grammar has no
strings — and it therefore inherits the same treatment: clamped and rounded CPU-side, because
`[smoothing]` and preset dissolves interpolate a binding continuously and a mode is an identity
rather than a quantity.

The `ring` question above is the one thing this ADR deliberately leaves to the plan. It is a
behavioural choice with three defensible answers, it is cheap to change, and it wants to be made
against a rendered figure rather than in the abstract.
