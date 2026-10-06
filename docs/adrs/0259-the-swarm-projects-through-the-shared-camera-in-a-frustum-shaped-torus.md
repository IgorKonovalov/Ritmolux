# ADR-0259 — The swarm projects through the shared camera, in a frustum-shaped torus

> **Status:** accepted 2026-10-06 (Plan 0239)
> **Date:** 2026-10-01
> **Related plan(s):** [0239](../plans/done/0239-the-swarm-moves-into-a-real-camera.md)
> **Supersedes in part:** ADR-0044 (its depth model; its target-sized torus survives in a new shape)

## Context

ADR-0044 made the swarm a 2D torus with a fixed per-particle `z` in `0..1`. That `z` drives four
hand-made cues (`swarm.rs` `DEPTH_*`):

- sprite scale from 0.55 to 1.50
- brightness from 0.45 to 1.05
- parallax against `zoom` and `pan` from 0.65 to 1.25
- a flow-field phase offset of `z * 2.6`, so each depth layer rides a different current

There is no projection, no sort and no depth buffer. The torus is sized from the render target's
aspect times a margin (`bounds(aspect) = (aspect * 1.25, 1.25)`), so its wrap seam sits off-screen.
ADR-0044 rejected a true 3D field with perspective (its Alternative A) on cost against the
integrated-GPU floor and because additive blending has no occlusion to gain. Its Notes said a later
proposal should re-open Alternative A rather than extend the 2.5D design.

The shared camera (ADR-0257) changes the cost side of that argument. Projection is a few vertex-shader
instructions. Depth of field is a sprite widened by its circle of confusion, with no sort and no depth
buffer, for ADR-0044's own reason. The owner chose to replace the 2.5D model rather than add a camera
mode beside it.

Two facts constrain the replacement:

- **The swarm's sprites are not `quad3d` sprites.** The swarm draws SDF silhouettes (`shape`), an ink
  mode and a speed-driven brightness through its own WGSL. `quad3d` draws round sprites only.
- **A perspective camera's view is not a box.** A torus with fixed world bounds is cut by the frustum
  at different widths at different depths. A box sized for the near plane shows its seam at the far
  plane, and a box sized for the far plane leaves the near layers empty.

## Decision

We will **simulate the swarm in frustum coordinates and project it through `Camera3d`**. A particle
holds `(u, v, z)`:

- `u` and `v` lie in `[-1, 1)`, and are the particle's position across the frustum's cross-section at
  its own depth, times ADR-0044's margin.
- `z` is a view depth between a near and a far slab bound, in the camera's units.

The world position is `u` and `v` scaled by the frustum's half-extent at depth `z`, evaluated at the
rest `fov`. The torus wraps in `u` and `v`, so the seam stays off-screen at **every** depth, which is
ADR-0044's target-sized torus lifted to a frustum.

The flow field is evaluated in **world** coordinates, so a far layer's currents are larger on screen
than a near one's. The `z`-phase decorrelation survives as part of the field. `z` now moves on a slow
component of the flow, and a particle fades out near either slab bound before it wraps in depth,
as a plexus point fades at a cube face. A world velocity is converted into `(u, v)` by the frustum
half-extent at the particle's depth, so a far particle crosses the screen more slowly. That is real
motion parallax.

The swarm **keeps its own sprite pipeline** and prepends `camera.wgsl`, calling `project()` and
`coc()`, as the attractor's shader does. The sprite size comes from perspective. The brightness ramp
becomes a bindable `depth_fade` whose default reproduces ADR-0044's 0.45 to 1.05. The parallax cue
retires, because perspective and motion produce it.

The camera's `yaw` and `pitch` are bounded to a sway the margin covers, and `distance` is not offered,
because the slab is defined relative to the camera and an orbit would show the edge of the world.
`zoom` divides the `fov` at projection and `pan` shifts after it, as on every 3D system (ADR-0257).
Neither re-maps the simulation.

## Consequences

### Positive
- The swarm gets real depth of field: a focal layer can be sharp while the layers before and behind
  it soften, which none of the four hand-made cues could produce.
- Motion parallax comes from the simulation itself rather than from a scale factor.
- The seam argument of ADR-0044 holds at every depth by construction, rather than through a margin
  tuned against one parallax range.
- Silhouettes, ink mode and the speed cue keep working, because the pipeline stays the swarm's own.

### Negative
- **Every swarm preset changes look**, and both swarm goldens (`swarm`, `swarm_shaped`) are
  re-blessed. The five shipped presets were tuned in motion against the 2.5D cues, and their seam
  comments (`swarm_drift.toml`) describe arithmetic that no longer exists. They need a judgement and
  a re-tune.
- **`zoom` and `pan` lose their parallax.** Under ADR-0044 they moved near layers more than far ones.
  Under ADR-0257 they are a uniform magnification and a post-projection shift. A preset that relied on
  that parallax for motion has to get it from `yaw` sway instead.
- **Zooming out below the margin still shows the seam**, as it does today. The margin is not
  re-derived by this ADR.
- Depth of field widens 10,000 to 30,000 sprites by their CoC. That is the largest blurred
  population in the engine, and its fill cost on `Floor` is unmeasured until Plan 0239 measures it.
- The camera surface on the swarm is a subset of the shared block. No `distance`, and bounded
  `yaw` and `pitch` are a documented exception to ADR-0258's "the same six params everywhere".

## Alternatives considered

### Alternative A — an opt-in camera mode beside the 2.5D model
`[swarm] depth = "camera"`, with the 2.5D model as the default. Every existing preset would keep its
bytes. It lost on the owner's call: two depth models in one system are two looks to maintain, and the
2.5D cues are the part ADR-0044's own Notes marked for replacement.

### Alternative B — a world-space box wrapped at fixed bounds
This is the simplest 3D sim, and a plexus cloud already does it. It lost because a box is not the
frustum's shape: its seam is visible at one end of the depth range or its population is wasted at the
other.

### Alternative C — move the swarm's sprites onto `quad3d`
One sprite pipeline for every 3D point would be the most shared shape. It lost because `quad3d` draws
round sprites only. Moving the swarm would drop the SDF silhouettes, the ink mode and the speed cue,
or widen `quad3d` with all three for a single consumer.

### Alternative D — proximity links between particles
The owner's original question included links. They lost because the plexus already is a linked point
set in depth. Linking 10,000 to 30,000 particles is about 50 to 450 million pair checks a frame with
`plexus::link`, and a wrapped torus needs wrap-aware distances. A linked swarm would be a second
plexus.

## Notes

- ADR-0044's rejection of a depth sort and of a depth buffer still holds: blending stays additive and
  commutative.
- ADR-0044's Alternative B (boids) is untouched by this ADR.

## Outcome (2026-10-06, Plan 0239 close)

- **Three swarm presets shipped, not five**, and none was `swarm_drift.toml`, which Plan 0232 had
  already retired. The seam comment that described the retired arithmetic was `swarm_braid.toml`'s.
  The owner kept Braid and Maelstrom and re-tuned Murmuration's `zoom` from 0.78 to 0.85.
- **"Zooming out below the margin"** is, under the projection, a `zoom` below about 0.82 at the rest
  `fov`, not 1/1.25: `zoom` divides `fov` through a tangent, so the seam reaches the frame edge where
  `1.25 * tan(0.4) = tan(0.4 / zoom)`.
- **The fill cost on `Floor` was measured** by Plan 0239 Phase 3: the blurred frame costs 1.57 to
  1.63 times the sharp one, inside the budget, so no swarm-only CoC ceiling below the shared cap was
  needed. `swarm_max_coc_px` exists at the shared cap's values.
- **The goldens' re-bless is owed** (Plan 0239 Phase 7, ADR-0249): `swarm` and `swarm_shaped` moved
  off-WARP and are not yet re-blessed on DX12 WARP.
