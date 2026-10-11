# ADR-0271 — A voxel is opened by one section plane, held in the cube's own frame

> **Status:** accepted 2026-10-11 (Plan 0258)
> **Date:** 2026-10-11
> **Related plan(s):** [0258](../plans/done/0258-the-voxel-opens-along-a-section-plane.md)

## Context

The owner judged the solid voxel present a keep and asked to see holes inside the structure
(backlog 0288). Under `present = "solid"` the march stops at the first live cell (ADR-0269), so a
dense rule (`clouds`, `amoeba`, `crystal`) shows a closed skin and nothing of what is inside it. The
teaching preset answered the ask by switching to `pyroclastic`, an open foam: it changed the content
rather than the view. The glow present has the same wall from the other side: its dense rules fill
their ball, and the absorption hides the core.

The march is one loop over cells (ADR-0268). Each step already knows the cell's index, and from it
the cell's centre in the cube's `[-1, 1]^3` frame, which the shell lighting and the radial palette
coordinate both read. So a test on the cell's centre that treats the cell as dead is the whole
mechanism. The open questions are which test, and whether it is a parameter or a key.

## Decision

We will give `voxel` **one section plane in the cube's own frame**, as three modal params:

- **`cut`** (0 to 1, default 0): how much of the cube the plane removes along its normal. The plane
  sits at `dot(centre, n) = sqrt(3) * (1 - 2 * cut)`, and a cell whose centre lies beyond it is
  dead to the march. At 0 it is past the cube's farthest corner and removes nothing; at 0.5 it
  passes through the centre; at 1 it removes everything.
- **`cut_yaw`** and **`cut_pitch`**: the plane normal's direction, in the cube's frame, so the cut
  turns with the structure and not with the camera.

At `cut = 0` the test is not evaluated: the march takes the existing arm through a `select`, so
every golden is byte-identical. A cut cell is dead for **every** reader:

- **the march**, glow and solid alike;
- **the solid present's AO**, whose `face_open` treats a cut neighbour as open, so a cut face is not
  darkened by cells the viewer cannot see;
- **the brick skip**, which stays correct by marching a cut brick cell by cell rather than by
  learning about the plane.

The cut is at **cell granularity**. A solid hit behind the plane shows the face the ray entered
through, as every hit does, so the section reads as a stair-stepped face of cubes.

Being modal params, all three are bindable. `cut = "0.5 + 0.3 * sin(time * 0.2)"` sweeps the plane
through the mass, and `cut = "0.4 * bass"` opens the structure on the bass.

## Consequences

### Positive

- **Any rule can be opened.** The view, not the content, answers "what is inside": a dense
  `crystal` can be cut in half while it grows.
- **Both presents gain it**, because the test sits ahead of the branch between them.
- **Animation costs nothing new.** The expression grammar already drives any modal param, so the
  sweep the owner will want next is a binding, not a feature.

### Negative

- **The cut face is stepped, not flat.** At `grid = 64` the steps are small; at 16 they are coarse,
  and a plane at 45 degrees reads as a staircase. That is the voxel look, and it is also not a clean
  section.
- **A full brick behind the plane loses the brick skip.** A cut volume is marched one cell at a
  time until the plane, so a deep cut through a sparse field costs steps it used to skip. Plan 0258
  probes the cost.
- **Three params on a system that already carries many.** They are inert at `cut = 0`, and only `cut`
  is declared `main` (ADR-0256), so the two angles stay out of the studio's first row.

## Alternatives considered

### Alternative A — a camera depth peel

Hide every cell nearer the camera than a distance, so the cut always faces the viewer. It lost
because the opening then changes as the camera orbits: the structure's inside is revealed at a
different place every frame, and an orbiting camera is how every voxel preset is drawn. A plane held
in the cube's frame shows the same section from every side.

### Alternative B — a clip box

Min and max per axis, six params, which can take a corner out of the mass. More control than the ask
needs, and twice the params, for a look one plane already gives.

### Alternative C — a structural `[voxel]` key

Fixed per preset, with no animation. It lost because a moving section is the obvious next ask and the
param route costs nothing over the key.

### Alternative D — a capped section

Draw the plane itself as a flat colour where it cuts live cells, the way a CAD section is hatched.
It lost because the cap hides exactly the hollowness the owner wanted to see: a cut through foam
would read as a solid slab.
