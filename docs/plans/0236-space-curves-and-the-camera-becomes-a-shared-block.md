# 0236 — Space curves, and the camera becomes a shared block

> **Status:** approved (2026-10-01). Runs after Plan 0235 closes.
> **Created:** 2026-10-01
> **Owner skill(s):** dev, human
> **Related ADRs:** [ADR-0258](../adrs/0258-a-system-takes-depth-through-one-shared-camera-block-and-its-3d-mode-forgoes-what-seg3d-does-not-draw.md) (proposed), [ADR-0257](../adrs/0257-a-shared-camera-projects-3d-primitives-and-depth-of-field-is-a-per-endpoint-circle-of-confusion.md), [ADR-0180](../adrs/0180-a-mathematical-world-joins-a-system-as-a-family-and-a-structural-parameter-is-held.md), [ADR-0059](../adrs/0059-line-scenes-colour-along-their-generator-axis.md), [ADR-0045](../adrs/0045-quality-tiers-floor-and-rich.md)

## TL;DR

`parametric_curve` gains two **space-curve families**, `torus_knot` and `lissajous_3d`. They are
drawn through the shared camera and `seg3d`, so a knot turns in perspective and blurs away from its
focal plane along one continuous line. First, the camera's six params and its lens logic move out of
`plexus` into **one shared block** (ADR-0258), which this plan and Plans 0237 to 0240 all splice.
The plan also measures the one tier cap, `seg3d_segments`, that every non-plexus 3D line system
sizes its buffer from. The 14 curve presets and the curve golden do not move.

## Context & problem

The owner asked which systems could reuse the plexus capability, and chose four plus the attractor
(ADR-0258's Context). `parametric_curve` is the cheapest: it already draws through `LineRenderer`,
and its families are "a variant and an arm" (`curves.rs` `arm()`).

Three things stand in the way.

- **The camera surface is private to plexus.** `yaw`, `pitch`, `distance`, `fov`, `focus` and
  `aperture` are declared inline in `scenes/plexus/mod.rs`, with plexus-worded doc strings. The
  lens, cull-margin and blur-announce logic in its `render` is inline too.
- **The family machinery is 2D.** A `FamilyArm` samples into `Vec<[f32; 2]>`, and the `per_family!`
  macro behind `FAMILY_PARAMS` is hard-wired to the five family names.
- **The shared 2D `LineRenderer` has no `seg3d` pipeline.** Only `LineRenderer::new_3d` builds one,
  so the scene needs a renderer of its own, sized by a cap that has never been measured outside the
  plexus.

## Decision

We extract a `CameraParams` block and a lens helper beside `Camera3d`, in the shape of `PanParams`,
and move plexus onto it byte for byte. `parametric_curve` gains a 3D arm type and two 3D families.
They render as dense polylines through a scene-owned `new_3d` renderer, with the biarc fit, the
mirror, `stroke_blend` and `rotation` declared inert on them (ADR-0258). We rejected lifting every
2D family into `z` and widening `seg3d` with joins first (ADR-0258, Alternatives B and A).

## Architecture diagram

```mermaid
flowchart LR
    subgraph render[core/src/render]
        CP["camera.rs: CameraParams<br/>6 ParamSpecs + lens helper"]
        CAM[Camera3d / CameraView / Lens]
    end
    subgraph scenes[core/src/render/scenes]
        PX[plexus]
        PC["parametric_curve<br/>2D arms -> shared LineRenderer<br/>3D arms -> own new_3d renderer"]
    end
    TIER["tier.rs: seg3d_segments"]
    CP --> PX & PC
    CAM --> CP
    TIER --> PC
    LATER["Plans 0237-0240<br/>lsystem, waterfall, swarm, attractor"] -.splice.-> CP
```

## Implementation phases

### Phase 1 — The camera becomes a shared block
- **Owner skill:** dev
- **What:** `CameraParams` (the six `ParamSpec`s with group `Motion` for the orbit four and `Light`
  for `focus` and `aperture`, per ADR-0256, plus the field set, `set_param` and `reset`). Its doc
  strings say "the scene's volume" rather than "network". Add a helper that takes the params, the
  render target, the scene's bounding radius and the tier, and returns the `CameraUniform`, the cull
  margin and whether the blur cap bit. Plexus splices both and deletes its inline copies.
- **Files touched:** `core/src/render/camera.rs`, `core/src/render/camera/tests.rs`,
  `core/src/render/scenes/plexus/mod.rs`, `presets/README.md`, `presets/schema/`,
  `docs/specs/player-schema.json`.
- **Done when:** the plexus goldens are byte-identical. `git grep -c "\"aperture\"" -- core/src/render/scenes/plexus`
  finds no declaration there. The regenerated param reference and schemas differ only in the camera
  params' doc strings.

### Phase 2 — Walking skeleton: a torus knot in perspective
- **Owner skill:** dev
- **What:** a 3D arm type (`sample3d: fn(&CurveParams, &mut Vec<[f32; 3]>) -> bool`) beside the 2D
  `FamilyArm`, and `CurveFamily::TorusKnot`, with integer winding `p` and `q` read from `n` and `d`
  and a tube ratio `tube`. The scene owns a `LineRenderer::new_3d` sized by a provisional
  `seg3d_segments` tier field, created at construction so a frame allocates nothing. A 3D family
  draws a closed polyline of `samples` points. Colour runs along the path as on the 2D families
  (ADR-0059), and `draw_progress` reveals a prefix. The camera block is spliced and declared live on
  3D families only. `mirror_order`, `mirror_reflect`, `stroke_blend` and `rotation` are declared
  inert on them. Near-plane clipping and culling go through the helper from Phase 1.
- **Files touched:** `core/src/render/scenes/lines/mod.rs`, `core/src/render/scenes/lines/curves.rs`,
  `core/src/render/scenes/lines/parametric.rs`, `core/src/render/scenes/mod.rs`,
  `core/src/render/tier.rs`, `core/src/preset/schema/load.rs`, `core/tests/fixtures/`, `core/tests/`.
- **Done when:** a `torus_knot` fixture renders a visible knot through `shot`, and two frames with
  different `yaw` differ. The `parametric_curve` golden and every 2D curve test are unchanged. A
  `(2, 3)` knot's sampled points all lie at distance `1 ± tube` from the torus's core circle, within
  float tolerance. With `aperture > 0`, the projected stroke is wider at the knot's far side than at
  its focal side, measured on one rendered frame.

### Phase 3 — `lissajous_3d`, and the family table
- **Owner skill:** dev
- **What:** `CurveFamily::Lissajous3d`: `x = sin(n t + phase)`, `y = sin(d t)`, `z = sin(m t + phase_z)`,
  with `m` and `phase_z` as new family-scoped levers. Widen `per_family!` to the seven families, so
  `FAMILY_PARAMS` names where each lever reads. The loader rejects a 3D family name it does not know
  with the full list, as it does now.
- **Files touched:** `core/src/render/scenes/lines/mod.rs`, `core/src/render/scenes/lines/curves.rs`,
  `core/src/render/scenes/lines/parametric.rs`, `core/src/preset/schema/load.rs`, `core/tests/`.
- **Done when:** the family table test at `parametric.rs` holds against `CurveFamily::ALL` with
  seven entries. A `lissajous_3d` with `m = 0` and `phase_z = 0` has every point at `z = 0`. Each 3D
  family appears in the generated reference with the levers it reads.

### Phase 4 — The `seg3d_segments` cap, measured, and the goldens
- **Owner skill:** dev
- **What:** set `seg3d_segments` per tier by measurement on `Floor` at the worst-case `aperture` and
  `max_coc_px`, as Plan 0235 Phase 6 did for the plexus. Record the measurement in the log against NFR
  section 1. `samples` over the cap is clamped and announced through the existing overflow context.
  Add one golden per 3D family with a fixed camera and a non-zero `aperture`.
- **Files touched:** `core/src/render/tier.rs`, `core/src/render/scenes/lines/parametric.rs`,
  `core/tests/golden.rs`, `core/tests/golden/`, `core/tests/fixtures/`.
- **Done when:** both new goldens hold on the software adapter. An over-cap `samples` on a 3D family
  is clamped with a notice. The measured frame cost at `Floor`'s cap is in the log, and it is inside
  NFR section 1's budget, or the cap is lowered until it is.

### Phase 5 — Documentation and the references
- **Owner skill:** dev
- **What:**
  - `presets/README.md`'s generated block and `presets/schema/` are regenerated.
  - `docs/presets.md` names the two families and says what is inert on them.
  - `docs/preset-guide.md` gets one picture of a knot.
  - The preset-author reference's `## parametric_curve` section gains working ranges for the 3D
    families and the rule that `aperture` costs fill.
- **Files touched:** `presets/README.md`, `presets/schema/`, `.taplo.toml`, `docs/presets.md`,
  `docs/preset-guide.md`, `docs/images/`, `.claude/skills/preset-author/references/systems.md`.
- **Done when:** the schema and param-reference tests pass on the regenerated files.
  `node scripts/toc.mjs --check`, `node scripts/check-doc-links.mjs`,
  `node scripts/check-reader-prose.mjs` and `node scripts/check-system-counts.mjs` pass.

### Phase 6 — The look, judged
- **Owner skill:** human
- **Blocks merge:** no
- **What:** the owner runs a knot and a 3D Lissajous fixture live with the music track. They check that
  depth reads, that the facets at the polyline's joints are not visible at working widths, and that
  the frame rate holds with `aperture` up. Then they hand a brief to `preset-author`.
- **Files touched:** none.
- **Done when:** the owner records a keep, or a list of what is off, in this plan's log.

## Data shapes

```rust
// illustrative, not the final interface
pub(crate) struct CameraParams {
    pub yaw: f32, pub pitch: f32, pub distance: f32, pub fov: f32,
    pub focus: f32, pub aperture: f32,
}
impl CameraParams {
    pub(crate) const SPECS: [ParamSpec; 6] = [/* the six, worded for any volume */];
}

pub(crate) struct FamilyArm3d {
    pub sample: fn(&CurveParams, &mut Vec<[f32; 3]>) -> bool,
}
```

## Risks & open questions

- **Facets at joints.** `seg3d` has no join extension (ADR-0258, Negative). On a knot sampled at a few
  hundred points the turn per segment is small, and the notch is sub-pixel at working widths. If
  Phase 6 sees it, `samples` is the lever before any engine change is.
- **The cap is shared with Plans 0237 and 0238.** If one of them measures a much higher fill cost per
  segment, that plan lowers the cap and this system draws fewer segments. The cap is a content limit
  (ADR-0045), so that is a notice, not a bug.
- **`rotation` inert on 3D families** surprises an author who sets it. The reference says so. Mapping
  it onto `yaw` was considered and rejected, because a 2D in-plane rotation and a yaw are different
  motions.
- **A `torus_knot` with `gcd(p, q) > 1`** is a link of several loops, not a knot. The loop walk's
  closure check finds the period, and the rendered result is a torus link. That is a legitimate
  shape, and the reference says so.

## What this plan does NOT do

- Move any 2D family onto `seg3d`, or add `z` to an existing family.
- Give `seg3d` joins, arcs or a mirror.
- Ship presets. The 3D curve presets belong to `preset-author` after Phase 6.
- Touch the studio. The new params reach its panel through the generated schema.

## Implementation log

**Lane:**

| phase | owner | state | commit |
|---|---|---|---|
| 1 — The camera becomes a shared block | dev | not started | |
| 2 — Walking skeleton: a torus knot in perspective | dev | not started | |
| 3 — `lissajous_3d`, and the family table | dev | not started | |
| 4 — The `seg3d_segments` cap, measured, and the goldens | dev | not started | |
| 5 — Documentation and the references | dev | not started | |
| 6 — The look, judged | human | not started | |

### Notes

### Close triggers

- **`presets/` touched:**
- **Plan header `Closes:`** none
- **What shipped:**
- **Operator docs touched:**
- **Backlog probes (`node scripts/check-backlog-claims.mjs`):**
- **Full suite:**
- **Outstanding `human` phases:**

## Followups (after this lands)

- `preset-author`: a launch pair, one per 3D family.
- Plans 0237, 0238, 0239 and 0240 splice `CameraParams`, and 0237 and 0238 size from `seg3d_segments`.
