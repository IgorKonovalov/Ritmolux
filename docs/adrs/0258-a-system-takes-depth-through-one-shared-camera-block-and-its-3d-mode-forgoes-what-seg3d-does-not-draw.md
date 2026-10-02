# ADR-0258 — A system takes depth through one shared camera block, and its 3D mode forgoes what `seg3d` does not draw

> **Status:** accepted 2026-10-02 (Plan 0236)
> **Date:** 2026-10-01
> **Related plan(s):** [0236](../plans/done/0236-space-curves-and-the-camera-becomes-a-shared-block.md), [0237](../plans/0237-the-l-system-turtle-turns-in-space.md), [0238](../plans/done/0238-the-waterfall-system.md), [0239](../plans/0239-the-swarm-moves-into-a-real-camera.md), [0240](../plans/0240-the-attractor-projects-through-the-shared-camera.md)

## Context

ADR-0257 gave the core one `Camera3d`, one `camera.wgsl`, a `seg3d` pipeline in `LineRenderer` and a
`quad3d` sprite pipeline. Plan 0235 built them for `plexus` and gave the attractor the circle of
confusion alone. The owner then asked which other systems could take the same capability. Four line
or particle systems can: `parametric_curve` (space curves), `lsystem` (a 3D turtle), a spectrum
history drawn as a landscape, and the swarm. The attractor can take the projection as well as the
blur.

Two facts about what Plan 0235 left behind shape how they take it.

- **The camera is shared as code, not as a surface.** The six camera parameters (`yaw`, `pitch`,
  `distance`, `fov`, `focus`, `aperture`) are declared inline in `scenes/plexus/mod.rs`, with doc
  strings that say "network". So is the logic that turns `focus` into a focal depth, builds the
  `Lens`, widens the cull margin by the CoC and announces `OverflowContext::Blur`. A second system
  would copy all of it, and two copies of a parameter's range and meaning drift. The 2D surface
  already solved this once, with `PanParams`.
- **`seg3d` draws less than the 2D line path.** A `Segment3dInstance` has two `vec3` endpoints, a
  colour, a width and an alpha. It has no join extensions (ADR-0158), no arc primitive (the biarc
  fit, ADR-0041), no `StrokeMetric`, and `draw_3d` is additive only, so there is no opaque
  `stroke_blend` path. `replicate_mirror` is implemented for the 2D instances only. 14 shipped
  presets draw through `parametric_curve` and 6 through `lsystem`; some of them use the mirror or
  `stroke_blend`.

## Decision

We will make the camera **one shared parameter block** and one lens helper, declared once beside
`Camera3d` in the shape of `PanParams`. Every system that projects through the camera splices the
same six `ParamSpec`s and calls the same helper for focal depth, `Lens`, cull margin and the blur
announcement. Plexus moves onto the block with its bytes unchanged.

A system **opts into depth**, and its 2D presets keep their bytes. The opt-in takes the form the
system's structure already has: a new **family** where the system has families (`parametric_curve`),
a **structural flag** where it has a structural table (`[generator] turtle = "space"` on the
`lsystem`), or the **whole system** where depth is what the system is (`plexus`, the new
`waterfall`). Two systems take depth wholesale by their own ADRs and change their existing look on
purpose: the swarm (ADR-0259) and the attractor's 3D families (ADR-0260).

**A 3D line mode forgoes what `seg3d` does not draw.** In it, the biarc fit, join extensions, the
mirror and the opaque `stroke_blend` path are unavailable. Those params are declared inert on the
3D families or modes, so the generated reference names where each reads (ADR-0180 rule 4). A curve
is drawn as a densely sampled polyline, which is how the 2D path already draws a family the biarc
fit declines. `thickness` means pixels at the focal plane, as `line_width` does on the plexus.

**Every 3D line system other than plexus sizes its `seg3d` buffer from one shared tier cap,
`seg3d_segments`.** It is measured on `Floor` at the worst-case `aperture`, as Plan 0235 Phase 6
measured the plexus caps, and it is clamped and announced rather than silently reduced (ADR-0045).

## Consequences

### Positive
- `focus` and `aperture` mean the same thing on every 3D system, and the reference and the studio
  panel describe them once.
- No existing 2D preset moves. The opt-in keeps every existing golden of `parametric_curve` and
  `lsystem` byte-identical.
- The `seg3d` stroke stays one implementation. The joins, arcs and profile metric are not
  re-derived for 3D, which is the drift ADR-0257's Alternative B was rejected to avoid.
- One measured cap covers the blurred-fill cost of the curve, turtle and waterfall systems, instead
  of three unmeasured buffer sizes.

### Negative
- **A 3D curve has visible facets at a sharp turn.** Without join extensions, two segments meeting
  at an angle leave a notch on the outside of the turn. Dense sampling hides it on a smooth curve.
  An L-system's right-angle turns will show it, at a width wider than a hairline.
- **A preset author meets params that do nothing.** `mirror_order`, `stroke_blend` and `rotation`
  are inert on the 3D families. The reference says so, but a binding to an inert param is not a load
  error.
- The shared `seg3d_segments` cap is one number for three systems with different fill profiles.
  If one of them dominates the cost, the cap is set by that one and the others get less than they
  could.
- Each 3D-capable scene owns its own `LineRenderer::new_3d` buffer, preallocated whether or not the
  active preset uses a 3D family. That is about 0.9 MB of instance buffer per scene at 20,000
  segments.

## Alternatives considered

### Alternative A — widen `seg3d` with joins, arcs and a mirror before any system adopts it
Every 2D feature would carry over, and a 3D curve would look as finished as a 2D one. It lost
because it is ADR-0257's Alternative B by another route. The join extension (ADR-0158) and the biarc
fit (ADR-0041) are derived in screen space against the 2D stroke metric (ADR-0160), and a 3D copy of
each is a second stroke implementation to keep in step.

### Alternative B — lift every existing 2D family into depth with a `z` parameter
One switch would turn any rose, Lissajous or harmonograph into a space curve. It lost on two counts.
A curve lifted into a plane and viewed head-on is the 2D curve again, so the switch adds a camera
without adding a shape. And the lifted curve would leave the biarc path for `seg3d`, so its bytes
would change at `z = 0`, which breaks the opt-in rule above.

### Alternative C — each system declares its own camera params
That is what plexus does today, and it needs no refactor. It lost because the meaning of `focus` is
"normalized from the nearest extent of the scene's volume to the farthest". That is one sentence,
and five copies of it would drift in range, default and wording.

## Notes

- The opt-in rule is about what a system's **existing** presets see. ADR-0259 and ADR-0260 are the
  two deliberate exceptions, each recorded with its own alternatives.
