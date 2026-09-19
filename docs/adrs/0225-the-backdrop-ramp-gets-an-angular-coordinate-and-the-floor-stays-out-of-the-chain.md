# ADR-0225 — The backdrop ramp gets an angular coordinate, and the floor stays out of the chain

> **Status:** proposed
> **Date:** 2026-09-19
> **Related plan(s):** [0203](../plans/0203-the-figure-gains-the-levers-it-was-measured-to-lack.md)
> **Rests on:** [0090](0090-a-preset-composes-two-scene-layers.md) (the one layer slot),
> [0106](0106-two-tone-graphics-come-from-a-multiply-layer.md) and Plan 0091 Phase 1's measurement
> of what a multiply layer does over a lit backdrop

## Context

The backdrop's ramp is swept along `bg_angle` and its coordinate is repeat-addressed, so repeating
**parallel** stripes ship today at no cost. Nothing produces an **angular** one. The third of the
user's Plan 0091 reference images is a collage whose floor is white with black stripes converging
to a vanishing point; everything else in that image is now reachable — the flat red figure is
`shape_field` (ADR-0105), the dark-on-light treatment is a multiply layer (ADR-0106) — and the floor
is not. What is missing is only the coordinate.

Folding the existing ramp is not available: `post.rs` puts the backdrop outside the chain's input,
so no blend mode and no kaleidoscope fold reaches it. That is the same structural fact Plan 0091
Phase 1 measured from the other side — at the default `occlude = 1` the backdrop is *absent* rather
than darkened, and at `occlude = 0` it is added after the junction and floors the frame, so a
multiply layer reaching display luma 18.9 over black reaches only 171.3 over a lit sky.

The consequence for this decision is sharp and worth stating before choosing: **a fan drawn on the
backdrop and a dark figure drawn over it are mutually exclusive at the tones the reference has.**
The collage's black stripes would have to come from the backdrop's own palette stops, and a figure
over them cannot then be darkened into that floor. So the cheap route buys the picture's floor and
not the picture.

The alternative Plan 0091 Phase 1's measurement promoted is to draw the floor as a **scene in the
chain**, where every blend mode reaches it — at the cost of the preset's one layer slot
([ADR-0090](0090-a-preset-composes-two-scene-layers.md)).

## Decision

We will add an angular coordinate to the backdrop ramp — a coordinate mode plus a movable centre —
so the existing swept, repeat-addressed stripes become a fan about a vanishing point. Every default
is an arithmetic identity, so no shipped preset and no baseline moves. We will **not** move the
backdrop into the chain: the floor stays outside it, and a world that needs its ground darkened by a
layer draws that ground as a scene and spends the slot deliberately, which is a preset's choice
rather than an engine change.

## Consequences

### Positive
- A vanishing point becomes expressible with two parameters and no slot cost, on a stage that is
  already swept and already repeat-addressed.
- Nothing moves. The identity default means the whole golden roster and every card is untouched,
  which is what makes this cheap enough to take before anyone has authored the world that wants it.

### Negative
- **It buys the floor, not the collage.** The reference image's dark figure over a light ground still
  cannot be had this way, for the reason measured above, and someone reading only this ADR could
  reasonably expect otherwise. That limit is the first thing the parameter's own documentation says.
- **A second way to get a ground exists**, and the two are not interchangeable: the backdrop's is
  free and unreachable by blends, a chain scene's costs the slot and is reachable. Authors have to
  know which one they are using.
- The collage's floor is also bounded by a **horizon**, which the ramp expresses only through stop
  placement — so a fan may not be the whole of what that image is doing, and the world that wants
  this may come back asking for more.

### Neutral
- It is the third decision on this pass after ADR-0094 and ADR-0095, and it follows their shape: a
  mode on an existing stage, identity by default.

## Alternatives considered

### Alternative A — Draw the floor as a scene in the chain
A ground scene in the preset's layer slot is reachable by every blend mode, which is exactly what
the backdrop is not. Plan 0091's Phase 1 measurement argues *for* it rather than against it, and it
loses here on cost rather than on capability: it spends the one slot ADR-0090 gives a preset, on a
floor, for every preset that wants one. Recorded rather than dismissed — a world that needs its
ground darkened should take this route, and nothing in this decision stops it.

### Alternative B — Move the backdrop inside the chain's input
Then a fold or a blend could reach it and both routes collapse into one. Rejected: the backdrop is
outside the chain by construction, the ordering is what makes `occlude` mean what it means, and
moving it would change every preset that has a backdrop at all.

## Notes

Raised as backlog 0095, 2026-08-16, at Plan 0091's close, from that plan's Phase 7 — which was
designed as a cut point and was cut. That entry's own instruction is that the two routes *"should be
judged by rendering, not by argument, which is what Phase 7's own first done-when said"*, so
[Plan 0203](../plans/0203-the-figure-gains-the-levers-it-was-measured-to-lack.md) renders both before
building either, and this decision is superseded rather than implemented if the rendering disagrees
with it.
