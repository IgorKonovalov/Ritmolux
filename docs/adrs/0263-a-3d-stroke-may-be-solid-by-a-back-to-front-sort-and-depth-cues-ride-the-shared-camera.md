# ADR-0263 — A 3D stroke may be solid by a back-to-front sort, and the depth cues ride the shared camera

> **Status:** proposed
> **Date:** 2026-10-05
> **Related plan(s):** [0248](../plans/0248-3d-strokes-gain-joins-depth-cues-and-a-solid-mode.md)
> **Relates to:** [ADR-0257](0257-a-shared-camera-projects-3d-primitives-and-depth-of-field-is-a-per-endpoint-circle-of-confusion.md)
> (the camera and the `seg3d` stroke this extends),
> [ADR-0258](0258-a-system-takes-depth-through-one-shared-camera-block-and-its-3d-mode-forgoes-what-seg3d-does-not-draw.md)
> (the shared camera block every 3D system splices),
> [ADR-0041](0041-line-joins-are-per-endpoint-on-the-segment-instance.md) and
> [ADR-0158](0158-a-joined-end-carries-its-own-miter-length.md) (the 2D joins this carries into 3D)

## Context

The owner judged the first two systems drawn through the shared camera live: the space curves of
Plan 0236 on 2026-10-02, and the waterfall of Plan 0238 on 2026-10-03. Both read as depth only
weakly, and both verdicts came back to the same three properties of the `seg3d` stroke, filed as
backlog 0279, 0280 and 0281:

- **Nothing occludes.** `LineRenderer::new_3d` builds the pipeline with
  `ADDITIVE_LIGHT_SATURATING_COVERAGE` and no depth attachment. A near strand crossing a far one
  hides nothing and the crossing brightens. On a knot that reads as glowing wire rather than an
  object; on a waterfall, which is a surface, the rows behind a peak show through it and the
  landscape tangles.
- **Nothing says "far".** A far strand is as bright and as saturated as a near one. The curves
  colour along their path, and only `plexus` colours by depth.
- **The blur breaks up.** A `Segment3dInstance` carries no join, so neighbouring blurred quads
  overlap at every joint and the additive blend sums each overlap into a ridge. The wider the blur,
  the worse the comb.

The engine has no depth buffer anywhere, and the `seg3d` instances are already uploaded from the
CPU every frame. Every 3D line system to come (Plans 0237, 0239, 0240) splices the same block, so
whatever is decided here is decided for them too.

## Decision

We will give the `seg3d` stroke three things, all through the shared camera layer so every system
that draws through it inherits them.

**Joins.** A 3D segment carries its neighbours, and the vertex shader mitres each joined end in
screen space, with the bevel fallback ADR-0158 uses in 2D. An end with no neighbour (a `plexus`
link, an open curve's first and last point) draws as it does today.

**Depth cues.** The camera block gains `fog`, a light falloff toward black over the volume's depth
on the same 0-nearest scale as `focus`. The space curves gain `hue_axis`, which moves their colour
axis from the path toward depth. Both default to off, so every existing preset keeps its bytes.

**A solid mode, chosen per preset.** The camera block gains `solid`. Off, the stroke is the
additive glow it is today. On, the renderer sorts the frame's segments far to near on the CPU into
a preallocated scratch buffer and draws them with premultiplied-over blending, so a near strand
paints over a far one. The waterfall, in solid mode, gives each row a black skirt down to the
ground, drawn in the same order as its line, which is hidden-line removal for a ridgeline.

We chose the sort because a blurred stroke's soft edge is translucent, and "over" in depth order
composites translucency correctly, while a depth buffer cannot without a two-pass split.

## Consequences

### Positive

- A preset can ask for an object instead of a light sketch, and the waterfall can read as terrain,
  without either losing the glow look the existing `plexus` presets rely on.
- Depth of field and occlusion compose: the soft edge of a near, blurred strand lies over what is
  behind it as a translucent veil, which is how a defocused foreground looks.
- One mechanism serves every 3D line system, present and planned, so Plans 0237, 0239 and 0240
  inherit occlusion and fog by splicing the block.
- No new GPU attachment and no change on any tier's memory budget.

### Negative

- **A CPU sort every frame in solid mode**, up to `seg3d_segments` (8,000 on Floor, 20,000 on
  Rich). Its cost is measured in the plan, not assumed.
- **A painter's sort orders whole segments.** Two segments that cross each other in depth inside
  their own lengths draw in one order along their whole length, so one crossing can be wrong. The
  segments are short (a few pixels on a curve, one band on a waterfall row), which bounds the error
  to a pixel-scale seam at the crossing. That is accepted here; a depth buffer would remove it at
  the price above.
- **Solid mode loses additive brightening on purpose.** A crossing no longer glows. Presets that
  want both glow and occlusion choose one.
- **The joins move pixels on every system that draws joined 3D polylines** (the space curves and
  the waterfall), so their golden baselines move once. Blessing is WARP-only today, so the moved
  baselines need a Windows session, or Plan 0218's move of blessing to lavapipe.

### Neutral

- `plexus` links are unjoined, so the joins leave its pixels alone; its nodes and links gain `solid`
  and `fog` like any other system.
- The waterfall's own `fade` stays. It dims rows by age; `fog` dims anything by camera depth. On a
  waterfall seen from the front the two coincide, and from the side they do not.

## Alternatives considered

### Alternative A — a depth buffer

Give the `seg3d` pass a depth attachment: solid strokes write depth and hide what is behind exactly,
with no CPU sort and no ordering error. **Rejected because the blurred edge is translucent.**
Depth-writing a translucent fringe punches dark cutouts into everything behind it, so it needs a
two-pass split, with a sharp core writing depth and a soft fringe only testing it. That is a second
pass per draw and the engine's first depth target, for an ordering error the short segments already
bound to a pixel.

### Alternative B — darken behind

Keep additive light, and let each near strand subtract light from what lies behind it in a halo.
**Rejected because it is not occlusion.** It is cheapest, with no sort and no depth, but the owner
asked for real hiding chosen per preset, and a darkening halo reads as a glow effect rather than as
a near thing in front of a far one.

### Alternative C — solid everywhere

Make every 3D stroke occlude, with no choice. **Rejected because the shipped `plexus` look is
additive.** Its fine links read well as glowing filaments, and the owner chose a per-preset switch
so that look survives.
