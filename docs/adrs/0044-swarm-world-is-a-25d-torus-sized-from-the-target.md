# ADR-0044 — The swarm's world is a 2.5D torus sized from the render target, and additive blending is why the depth axis is nearly free

> **Status:** accepted 2026-07-30 (implemented by [Plan 0043](../plans/done/0043-swarm-depth-and-domain.md))
> **Date:** 2026-07-29
> **Related plan(s):** [0043](../plans/done/0043-swarm-depth-and-domain.md)
> **Supplements:** [ADR-0037](0037-internal-grid-is-a-resolution-not-a-shape.md) (the aspect rule
> this applies to a simulation domain for the first time)
> **Closes backlog:** [0029](../design-backlog.md), and the depth half of
> [0025](../design-backlog.md)

## Context

The `swarm` scene simulates 10 000 particles on a 2D torus and draws them as instanced additive
sprites. Two independent findings have now converged on its world model.

**The domain's edge is on screen.** `BOUND_X = 1.8` / `BOUND_Y = 1.0` are hard constants, and
`BOUND_Y = 1.0` *is* the NDC frame edge. The wrap is toroidal — a particle crossing the bound
teleports to the opposite side — so that line is the one place on screen every wrapping particle is
guaranteed to paint. The feedback stage integrates it: at the `trails` values the family uses
(0.7–0.9), a per-frame deposit at a fixed `y` accumulates into a saturated bright bar across the top
and bottom of the frame within a few hundred frames, while the interior visibly drains. This was
reported by the user from the running app and reproduced headless; it affects all five swarm
presets and predates the 2026-07-29 content retune.

No preset lever reaches it. `trails` dims the bar and the figure equally and the bar wins, because
it is re-deposited every frame while the figure moves. `zoom` cannot move the seam off-screen
either: below `1.0` the camera pulls back far enough to expose the *domain rectangle* itself as a
hard inset edge, so the family is pinned at or above `1.0`, which is exactly where the seam is.

The constants also encode a resolution-era assumption the project has since ruled against. Their
own comment reads *"x is wider so the field fills 16:9 after the shader's aspect divide"* — a fixed
16:9, written before [ADR-0037](0037-internal-grid-is-a-resolution-not-a-shape.md) established that
anything computing screen-destined geometry takes its aspect from the render target. On a 16:10 or
21:9 target the field under-fills or over-fills horizontally for the same reason the composite did.

**The scene cannot express what it is for.** Backlog 0025 records the user's standing complaint —
*"they should look like flocks of birds, swirling and dancing in 3d-like space"* — and its
diagnosis: `Particle.pos` is `[f32; 2]`, there is no z, no parallax and no perspective, and the only
depth cue in the scene is incidental (`bright = (0.25 + speed * 0.7) * p.bright`, so fast particles
read as nearer). Two thirds of that complaint turned out to be a preset defect and was fixed in
content; the missing depth axis is the real remainder. The 2026-07-29 audit adds that the family is
the weakest-measuring one in the library, and the user has since chosen to cut it from five presets
to two or three.

The decisive technical fact is easy to miss: **the scene draws additively.** Additive blending is
commutative, so a depth axis imposes no ordering requirement — the thing that normally makes 3D
particles expensive (a per-frame sort, or depth-buffered blending that additive rendering cannot
use anyway) simply does not arise here.

## Decision

We will give the swarm a **2.5D world**: `Particle` gains a single `z` in `0..1`, used for three
purely visual effects — sprite scale, a parallax offset against the shared view transform, and an
atmospheric brightness fade — with **no z-sorting and no perspective projection**, because additive
blending is order-independent. Depth layers additionally sample the flow field at a z-dependent
offset, so near and far particles ride *different* currents rather than the same streamlines at two
sizes; that is what makes the result read as volume instead of as a scaled sprite sheet.

In the same change the domain stops being a pair of constants. **The toroidal bounds derive from the
render target's aspect** (ADR-0037) and carry a margin so the wrap seam sits outside the visible
frame at the family's working `zoom`, which removes the bright-bar artifact at its source rather
than hiding it. `FIELD_FREQ` becomes the bindable named param `field_freq`, defaulting to exactly
the `2.3` it replaces — backlog 0025 identifies it as the cheapest lever with real visual range
(few broad currents versus many tight swirls), and it composes directly with the new depth offset.

We are not adding boids rules.

## Consequences

### Positive
- The reported artifact is fixed at its cause, and the fix is the general one: the domain follows
  the target, so it is correct at 16:10 and 21:9 rather than only at the aspect it was written for.
- Depth costs one `f32` per particle and a handful of multiplies in a loop that already runs
  per-particle per-frame. No sort, no second pass, no depth buffer.
- `field_freq` plus the z-offset gives an author two genuinely new structural levers, which is the
  first time this scene's *form* — rather than its colour or speed — has been preset-reachable.
- Parallax against `pan_*`/`zoom` makes the shared view transform meaningful on this scene for the
  first time; today panning a flat field is indistinguishable from moving the field.

### Negative
- **It is an honest fake, and it will not survive every test.** There is no occlusion and no true
  perspective: two particles at different z that overlap simply sum, so a dense near-field never
  hides a far one. At high particle density the illusion flattens, which is exactly the regime the
  `Dense` preset lives in.
- The margin means a fraction of the 10 000 particles is off-screen at any moment, so the visible
  density drops slightly for the same `PARTICLES`. That is a real look change to the whole family on
  top of the one the content pass is already making, and it lands before the presets are re-tuned.
- `BOUND_X`/`BOUND_Y` becoming target-derived makes the scene's state depend on the target size,
  which it did not before. The scene already takes a per-frame target-size hook
  ([ADR-0030](0030-scene-target-size-hot-path-hook.md)), so the seam exists — but a resize now
  perturbs a running simulation, and the wrap must stay stable across one rather than teleporting
  every particle at once.
- Determinism now has one more input. The seeded scatter must produce the same `z` sequence for the
  same seed, and a capture at one target size is no longer bit-comparable to the same preset at
  another — golden fixtures pin one size, so this is contained, but it is a new coupling
  (NFR §6).

### Neutral
- Particle count, `DAMPING`, and the curl-field construction are unchanged. This is a wider world
  and an extra visual dimension over the same simulation, not a new one.

## Alternatives considered

### Alternative A — a true 3D field with perspective projection and depth sorting
`pos: [f32; 3]`, a 3D curl field, a real projection. Rejected on cost against a benefit that
additive blending mostly erases: the sort that a 3D particle system normally needs buys correct
occlusion, and an additive scene has no occlusion to get right — the frame is a sum either way. What
would remain is a 3D noise field sampled 10 000 times per frame instead of a 2D one, against the
60 fps @ 1080p iGPU floor (NFR 1/9), for a difference the 2.5D form already delivers most of.
Reconsider only if occlusion or non-additive blending ever becomes desirable.

### Alternative B — boids: cohesion, separation, alignment
The literal reading of "flocks of birds". Rejected as the highest-risk option on the
real-time budget: pairwise rules are O(n²) at 10 000 particles, and the spatial hash that makes them
tractable is a per-frame allocation-and-rebuild on a path that must not allocate. Backlog 0025 also
records that the *apparent* flocking users already respond to emerges from neighbouring particles
sharing a streamline — so the field, not per-particle rules, is where this scene's cohesion actually
comes from, and `field_freq` is the cheap way to control it. Not refused permanently; it needs its
own ADR and a measured budget.

### Alternative C — fade particle alpha approaching the seam, leaving the bounds alone
The narrowest fix for the artifact: keep `BOUND_Y = 1.0` and ramp brightness to zero near the wrap.
Rejected because it treats the symptom and keeps the 16:9 constant that ADR-0037 rules against, and
because a fade wide enough to hide a feedback-accumulated bar is wide enough to visibly darken the
top and bottom of every frame — trading a bright artifact for a dim one. It also leaves `zoom < 1`
still exposing the domain rectangle, so the family stays pinned above 1.0.

### Alternative D — respawn at a random position instead of wrapping
Removes the seam entirely. Rejected for the reason the existing code comments already give
(*"Toroidal wrap keeps the field populated (no respawns/hitches)"*): a respawn is a discontinuity in
a feedback-accumulated image, so it trades a static bright line for 10 000 popping streaks.

## Notes

The one-line summary worth carrying forward is that **additive blending is what makes this cheap**.
Any future proposal to add occlusion, alpha blending or a depth buffer to this scene invalidates the
reasoning above and should re-open Alternative A rather than extending this design.

The margin factor is left to the plan rather than fixed here: it is a number to be measured against
the family's working `zoom` range, not a decision with an alternative worth recording.
