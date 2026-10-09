# ADR-0268 — A 3D automaton is a voxel system marched as an emitting, absorbing volume, and its rules are a roster indexed by one held parameter

> **Status:** proposed 2026-10-08
> **Date:** 2026-10-08
> **Related plan(s):** [0255](../plans/0255-the-voxel-automaton-glows.md), [0256](../plans/0256-the-voxels-turn-solid.md)

## Context

The owner asked for 3D cellular automata drawn as voxels, first as a glowing volume and then as solid
lit cubes (ADR-0269). The engine already has both halves of the parts list and neither of the
whole:

- **`cellular`** (Plan 0164) steps a 2D automaton on a ping-pong grid. Its counting is integer, its
  seed is a `u32` hash of the cell and the preset's pinned salt (ADR-0051), and `GenerationClock`
  turns `step_rate` into a whole number of generations per frame, so a field is a pure function of
  its inputs. Its grid is a **cap on content**, clamped and announced (ADR-0045), because a pattern is
  a fixed number of cells.
- **The shared camera** (ADR-0257, ADR-0258) projects primitives through one orbit camera and one
  parameter block. It has no depth attachment. Every 3D pipeline draws additive strokes or sprites,
  and depth of field is a per-primitive circle of confusion.

A 3D automaton differs from both in four ways that decide its shape:

- **Its state is a volume.** 128³ is 2.1 M cells, and a cell's neighbourhood is 26 cells, not 8.
- **A live cell is not a primitive the camera can project cheaply.** Drawing each cell as a sprite
  reads as a dotted lattice, and drawing each as a cube needs a depth buffer the engine does not have.
- **A rule does not fit a parameter.** A birth/survive pair over 0-26 neighbours is two 27-bit masks.
  A bound parameter is an `f32`, which is exact only to 2^24. So a mask bound the way `cellular`
  binds `birth` would silently lose its top bits.
- **Most 3D rules are dead or noise.** Only a small, known set stays alive and structured from a
  simple seed. The rest die in a few generations or fill the volume with static.

The owner also asked for the rule to change with the music on the bar, for an onset to inject
matter, and for the spectrum to light the volume in radial shells.

## Decision

We will add a new system, **`voxel`**, under ADR-0180 rule 1. It has its own state (a 3D texture),
its own pass shape (a camera raymarch), and parameters that would be inert on `cellular` (the
camera block, the 27-neighbour rules). `cellular` does not change. The two share code, not surface:
`GenerationClock` and the WGSL cell hash move to `scenes/common.rs`, and `cellular`'s output stays
byte-identical.

**The state** is a ping-pong pair of 3D textures of side `grid`. Each cell holds a state in the
Generations sense: 0 is dead, 1 is live, and 2 to `states - 1` are decay stages that do not count as
neighbours and fall back to 0. The volume wraps at its faces, as `cellular`'s torus does. `grid` is a
content cap per tier, `voxel_grid`, at 64 on `Floor` and 128 on `Rich`. It is clamped and announced,
never silently reduced. One generation is one compute pass that counts the Moore (26) or von Neumann
(6) neighbourhood. Counting is integer, so the volume is a pure function of the config, the seed and
the sequence of bound values and `dt`s.

**The present is a voxel raymarch through the shared camera.** For each pixel the scene casts a ray
from the camera through the cube `[-1, 1]³` and walks it cell by cell with an exact grid traversal
(Amanatides-Woo DDA). A cell's emission comes from its state, its age and its radial shell. Light
accumulates front to back under an **emission-absorption** model:

```
C += T * emission(cell) * len        // len: the ray's length inside this cell, from the DDA
T *= exp(-density * occupied(cell) * len)
```

`density = 0` gives pure additive glow. The ray stops when `T` drops below one 8-bit step. The scene
writes premultiplied colour with `alpha = (1 - T) * occlude`, which is ADR-0201's present with the
coverage the march computed. An occupancy grid of bricks, rebuilt each generation, lets the march
skip empty space. The march runs on its own target, a fraction of the render target capped per tier,
and is presented with a plain stretch. Its aspect is the render target's (ADR-0037). The camera's
`focus` and `aperture` are inert on `voxel`, which ADR-0258 permits a 3D mode to declare: a circle
of confusion is per primitive, and a march has no primitives.

**Rules are a list in the structural table, and one held parameter picks among them.** `[voxel]
rules` is a list of one to eight entries. Each entry is a name from a closed roster, or an inline
rule `{ birth = [..], survive = [..], states = N, neighbourhood = "moore" | "von_neumann" }` with
counts as integers 0-26 (0-6 for von Neumann). A count outside that range is a load error. The
structural parameter `rule` indexes the list. It is quantized under ADR-0180 rule 2, so a preset
moves the rule on the bar with `rule = "mod(bar_index, 3)"` and `[hold] rule = "bar"`. The masks
never pass through an `f32`. A rule change takes effect at the next generation boundary. A cell whose
decay stage is at or past the new rule's `states` falls to dead. The roster ships only rules that
survive a fixed seed for a fixed number of generations without dying or saturating, and a test holds
every roster entry to that.

**Two more levers serve the music.** `reseed`, as on `cellular`, refills a ball on each rise past
0.5. The ball's centre is the hash of the rise count and the salt, so it is seeded, never clocked.
**Shells** read `AnalysisFrame::spectrum` in `update`, as `waterfall` does, and downsample it to
`shells` radial bands. Low bands sit at the centre. They scale emission by `1 + shell_gain * level`,
so `shell_gain = 0` is an exact identity and the music lights the volume without touching the
simulation.

## Outcome (2026-10-09, Plan 0255)

Plan 0255 built the decision as stated, and settled four of its details otherwise. The body above
is left as accepted; these are the record:

- **The faces do not wrap by default.** `[voxel] wrap` is optional and defaults to `false`, dead
  outside the cube, the alternative Plan 0255's Risks named for a structure that crosses a face and
  reads as a cut-off in an orbit. `wrap = true` gives the torus the Decision describes.
- **Each roster rule survives its own seed, not one fixed seed.** Several rules grow only from a
  dense small core and others only from a sparse field, so a roster entry carries its own default
  seed (`RosterRule::seed`), which an absent `seed_radius` / `seed_fill` takes. The survival test
  runs each rule from that seed.
- **Absorption is weighted by the cell's glow, not by occupancy.** The march applies
  `T *= exp(-density * glow(cell) * len)`, where a live cell's glow is 1 and a decay stage's is
  `trail^(state - 1)`, so a fading cell absorbs as it emits rather than as a full cell, and a stage
  faded from sight no longer occludes.
- **The bricks are marked once a frame, not once a generation.** They are rebuilt after the frame's
  last generation, which is the only state the march reads, so the two are equivalent for the
  picture.

## Consequences

### Positive
- The engine gains a volume register without a depth attachment, a mesh or a light. Emission-
  absorption stays inside ADR-0046's linear light and ADR-0201's premultiplied present.
- The rule can change with the music through machinery that already exists (structural kind,
  `[hold]`), and no mask is ever rounded.
- The DDA gives each ray's exact path through the cells. ADR-0269's solid present reuses the same
  loop and stops at the first live cell, with that cell's face known from the axis the last step
  crossed.
- A closed roster with a survival test keeps the shipped rules alive, and inline rules keep the
  space open for the content lane.

### Negative
- **The march costs per pixel, and per cell crossed.** A ray through a 128³ grid crosses up to
  3 x 128 - 2 = 382 cells. Brick skipping helps a sparse volume and does nothing for a cloudy one.
  The march target scale is the lever, and on an iGPU it will be well below 1. The look at `Floor`
  is softer than at `Rich` for that reason.
- **The grid cap changes content, not density**, as on `cellular`. A 128³ preset on `Floor` runs on
  64³ and draws every structure at twice the size. That is announced, and it is still a different
  picture.
- **`focus` and `aperture` are inert.** The camera block is spliced whole, so a preset author meets
  two camera params that do nothing here. The reference says so (ADR-0180 rule 4).
- **The roster's survival test is long-horizon.** It runs many generations at a small grid and is a
  structural statistic (live fraction inside a band), not a pixel comparison (ADR-0180 rule 3). A
  rule that survives at 32³ is not proven to look good at 128³.
- **Rule shift on the bar leans on the downbeat estimator.** Backlog 0042 records that it locks on
  ~3 % of audible time. `bar_index` still advances on its fallback grid, so the shift happens, but
  not always on the musical bar.
- The state pair costs 2 x `grid`³ x the texel size: 16.8 MB at 128³ with a 4-byte texel. The bricks
  and the march target add a few MB more.

## Alternatives considered

### Alternative A — 3D families on `cellular`
`family = "life_3d"` on the existing system. Rejected under ADR-0180 rule 1: the state texture
changes dimension, the present changes from a stretch to a camera march, and the camera block and
27-neighbour rules would be inert on all three 2D families, while `route` and `mirror` would be
inert on the 3D one. A family that shares no pass and half no parameters is a second system that
shares only a table name.

### Alternative B — live cells as sprites through `quad3d`
Compact the live cells into a list and draw each as a camera-facing sprite, as plexus and the
attractor draw points. This is the cheapest to integrate. Rejected because a lattice of points reads
as a lattice, not as a volume or as voxels, and it gives ADR-0269's solid present nothing to build
on.

### Alternative C — textured slices
Draw `grid` camera-facing quads through the volume, blended additively, as classic volume rendering
does on fixed-function hardware. Rejected because it costs `grid` full-screen fills per frame
whatever the volume holds, cannot skip empty space or stop early, and cannot find a first hit for
ADR-0269.

### Alternative D — rule masks as bindable parameters
Mirror `cellular`'s bindable `birth` and `survive`. Rejected because a 27-bit mask in an `f32`
loses bits past 2^24, and because a mask bound to an expression wanders through rules that mostly
die. An index over a list the author chose keeps every reachable rule one the author meant.
