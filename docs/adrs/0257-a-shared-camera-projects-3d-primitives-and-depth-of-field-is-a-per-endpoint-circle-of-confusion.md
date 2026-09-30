# ADR-0257 — A shared camera projects 3D primitives, and depth of field is a per-endpoint circle of confusion

> **Status:** proposed
> **Date:** 2026-09-30
> **Related plan(s):** [0235](../plans/0235-the-plexus-system-and-a-shared-camera-with-depth-of-field.md)

## Context

The owner asked for presets in the look of a "plexus" still: a few hundred points in 3D space, each
joined by a thin line to its near neighbours, seen through a perspective camera with a shallow depth
of field. One line runs sharp through the focal plane and dissolves toward both ends. That last
property is what makes the picture read as depth rather than as a flat network, and it is the one
the engine has no vocabulary for.

The engine renders nothing through a 3D camera today. The only shared view is the 2D
`ViewTransform { zoom, pan }` (`core/src/render/scenes/lines/mod.rs`, ADR-0018), which every scene
applies in its own WGSL. The attractor keeps a real view depth (ADR-0076). It turns that depth into
a scalar `magnify = 1/(1 - perspective*dn)`, not a projection, and writes `z = 0`. The swarm invents
a per-particle `z` that drives four cues (ADR-0044). No render pipeline anywhere has a depth-stencil
attachment. Stroke softness (ADR-0124) is one value per draw call, not per segment, and
`SegmentInstance` carries 2D endpoints and one width.

Three renderers would want the capability: the shared `LineRenderer`, the instanced-quad
`marks` path, and the attractor's own sprite pipeline. They share no code today.

The scenes this serves are **additive**. ADR-0044 already argued the consequence for the swarm:
additive blending is commutative, so there is no occlusion to resolve and no sort to pay for. The
same fact rules out the textbook depth-of-field technique. A post-process gather blur needs a depth
per pixel, and an additive pixel is the sum of many primitives at many depths, so it has no single
depth.

## Decision

We will add one **shared camera** to the core: a Rust `Camera3d` (orbit `yaw` / `pitch`, `distance`,
`fov`, `focus`, `aperture`) that builds a view-projection matrix, and one WGSL snippet (`camera.wgsl`)
that every 3D pipeline includes. The snippet has two jobs: `project(world) -> clip`, and
`coc(view_depth) -> pixels`, the thin-lens circle of confusion
`aperture * |depth - focus_depth| / depth`, clamped to a tier cap. **Depth of field is computed per
primitive endpoint, not per pixel.** A 3D segment computes a CoC at each end. Its half-width grows
by the CoC and its edge softness widens with it, both interpolated along the segment. Its intensity
is scaled by `w / (w + coc)`, so the integrated energy across the stroke is preserved and a blurred
line dims instead of flaring. A 3D point sprite does the same, with an area factor
`(r / (r + coc))²`. There is no depth buffer and no sort, for ADR-0044's reason.

The capability arrives as **new pipelines alongside the existing ones, chosen rather than branched**
(ADR-0212's shape): a `seg3d` pipeline inside `LineRenderer` and a `quad3d` pipeline beside
`marks::InstancedQuads`. Every existing 2D scene keeps its pipeline, and its bytes, unchanged. The
screen aspect is the **render target's**, never an internal grid's (ADR-0037). The engine-wide `zoom`
divides the field of view, and `pan_x` / `pan_y` shift after projection, so both keep their meaning on
a 3D scene. The attractor adopts the shared **CoC** function on the view depth it already computes.
Its projection stays, because its tuple rosters' framing is curated against it (ADR-0093).

## Consequences

### Positive
- One focus model across every 3D primitive in the engine. A preset author learns `focus` and
  `aperture` once, and they mean the same on the plexus, on its nodes and on the attractor.
- Continuous depth of field along a single line, which is the look being asked for. A post-process
  blur cannot produce that on additive content.
- The cost is proportional to what is drawn (overdraw from widened primitives), not to the screen.
  That matters for the integrated-GPU floor in NFR section 1.
- No existing golden moves: the 2D pipelines are untouched, and the attractor's default `aperture`
  of `0` reproduces it byte for byte.

### Negative
- **Blurred primitives cost fill.** A far segment with a 20 px CoC covers roughly ten times the pixels
  of a sharp one. That cost is bounded only by a tier cap on the CoC and on the primitive count, and
  the cap is a content limit that an iGPU operator sees as a shallower blur.
- **It is a fake, and it has the fake's failure.** A per-primitive blur has no neighbourhood, so two
  blurred lines crossing do not blend into one disc the way an optical blur would. They sum, which
  additive content forgives. An opaque scene (`shape_collage`) could not use this, and does not try.
- **The camera exists twice**: once in WGSL, and once in Rust for the CPU work of near-plane
  clipping and frustum culling that a scene does before upload. That is the
  `particles/projection_mirror.rs` hazard again, and it needs the same test pinning the two equal.
- The attractor gets depth of field without the real camera, so "shared" is only half true for it
  until a later plan moves its framing onto `Camera3d`.

## Alternatives considered

### Alternative A — a post-process depth-of-field stage over a depth buffer
It is general, since any scene that writes depth gets it. It lost on additive content: the
composite's pixels have no single depth, so the gather blur has nothing to read. Adding a depth
attachment to every pipeline to fake one would still produce halos where sharp lines cross blurred
ones, and a gather blur at 1080p is a fixed cost on the integrated-GPU floor even when nothing is
out of focus.

### Alternative B — a new all-in-one 3D primitive renderer
One renderer for 3D points and segments, with depth of field built in, which the attractor and the
curve scenes would migrate onto later. It lost because it would be a third stroke implementation
beside `LineRenderer`. The stroke profile (ADR-0124), its metric (ADR-0160) and the joins
(ADR-0041, ADR-0158) would have to be re-derived and would then drift, which is the exact cost that
putting the line work in one shared renderer was meant to end.

### Alternative C — project on the CPU and widen the 2D segment
Project each endpoint on the CPU and give `SegmentInstance` per-endpoint width and softness. It
lost on two counts. Widening the instance changes the layout under every 2D line scene, including
`warp_mesh`, whose softness pin is byte-sensitive. And the attractor runs on the GPU, so it could
not use a CPU camera at all and would need a WGSL copy of the maths anyway.

## Notes

Two-layer depth (a blurred back layer and a sharp front layer through ADR-0090's composition) stays
available to any preset. It is how a preset fakes depth without this ADR, and it composes with it.
