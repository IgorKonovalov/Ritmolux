# ADR-0266 — The maze route is a double sweep relaxed on the GPU against a frozen snapshot

> **Status:** proposed
> **Date:** 2026-10-07
> **Related plan(s):** [0253](../plans/0253-the-labyrinth-shows-its-longest-path.md); design-backlog 0274

## Context

At the Plan 0232 retune the owner asked for `cellular_labyrinth` to draw the longest path through
the maze it grows, in red (backlog 0274). The `cellular` scene has no notion of a path. It holds a
state and an age per cell, and paints from those alone. Asked at the 2026-10-07 interview, the owner
chose two things. The route refreshes **every generation**, not every few seconds and not only once
the maze settles. It is painted as **one solid colour by default, with an author option to grade it
by distance along the route** through the palette.

"Every generation" rules out the cheap shape: copy the grid back to the CPU every second or two and
search it there. The search has to run on the GPU. On the GPU the obvious search, a breadth-first
wavefront advancing one cell per pass, cannot finish in a frame. The arithmetic, with open cells
(dead cells, the corridors) about half the grid:

| grid | cells | open cells, about | worst path (a tree maze threading every open cell) |
|---|---|---|---|
| 192 (`cellular_labyrinth`) | 36 864 | 18 000 | ~18 000 steps |
| 512 (the Floor tier's cap) | 262 144 | 131 000 | ~131 000 steps |
| 1024 (`MAX_GRID`, Rich) | 1 048 576 | 524 000 | ~524 000 steps |

One pass per step is 18 000 dispatches at the shipped grid in the worst case, against a frame that
already encodes up to `MAX_GENERATIONS_PER_FRAME = 8` step passes. A maze grown by `B3/S12345` is not
a tree: it has loops and shortcuts, and its real diameter is likely far below the worst case. That is
unmeasured, so this ADR does not rely on it.

Three more facts shape the answer. A **longest path** is NP-hard on a general graph. The tractable
question is the longest *shortest* path, the graph's diameter. A double sweep (search from any cell,
take the farthest cell B, search from B, take the farthest cell C) finds that path exactly on a tree
and gives a lower bound on a graph with loops. The maze also **changes while the search runs**:
reseed discs bite patches out, and a loosened rule partly dissolves it. Finally, the scene is held to
reproducibility. `thirty_and_one_hundred_forty_four_fps_reach_the_same_field` already pins that the
field does not depend on the frame rate, and the route must not reintroduce that dependence.

## Decision

We will compute the route on the GPU as a **double sweep of breadth-first distance fields, each
relaxed over many frames against a frozen snapshot of the maze, and commit the finished route as a
per-cell buffer the present paints from.**

- **The graph.** Open cells are the dead cells of the state channel, joined to their four orthogonal
  neighbours, across the seam when `[cellular] wrap` is on. Live cells are walls. `cyclic` has no
  dead state, so the route is inert there, as `trail` already is.
- **An epoch.** An epoch starts by copying the open mask into a snapshot buffer, and only if that
  mask differs from the last epoch's. A maze that stands still costs one compare pass per frame and
  nothing else. Inside an epoch, three sweeps run against the snapshot. Sweep 1 starts from source
  `s` and ends at the farthest cell B. Sweep 2 starts from B, keeps its field `dB`, and ends at the
  farthest cell C. Sweep 3 starts from C and keeps `dC`. A **commit pass** then writes the route:
  every open cell with `dB(x) + dC(x) == dB(C)`, holding `t = dB(x) / dB(C)` in `[0, 1]`. Every other
  cell gets a sentinel. That membership test is the route extraction: it is fully parallel and never
  walks the path cell by cell.
- **Relaxation.** A sweep is a compute pass repeated across frames. A 16x16 workgroup loads its tile
  and a one-cell halo from the read buffer into workgroup memory, and relaxes
  `d(x) = min(d(x), 1 + min over open neighbours d(n))` there for a fixed number of local
  iterations. It writes the other buffer of a ping-pong pair and counts the tiles that changed. Ping-pong, not
  in-place atomics, is what makes every pass a pure function of the previous one, so the frame a
  sweep converges on does not depend on how an adapter schedules workgroups. A sweep has converged
  when a whole pass changes nothing. A one-workgroup **control pass** after each relax pass reads that
  count and advances the epoch's phase on the GPU. The phases are sweep 1, sweep 2, sweep 3, commit,
  idle. The CPU never reads the phase back.
- **Endpoints, deterministically.** The farthest cell is found in two reduction passes: the largest
  finite distance first, then the lowest cell index at that distance. Ties are therefore broken by
  index, never by scheduling. The first epoch after a configure starts from the open cell nearest
  the grid's centre, again lowest index on a tie. Every later epoch starts from the previous route's
  end C if that cell is still open, so the route stays in the component it found and tends to
  lengthen rather than jump.
- **Every generation, honestly.** The search advances on every frame that has route passes owed,
  and a new epoch starts on the first pass after a commit if the maze has moved. The drawn route is
  always the last **committed** one, painted only where its cells are still open now. A corridor cut
  since the snapshot shows as a gap until the next commit closes it. It is never drawn through a
  wall. How long an epoch takes depends on the maze's diameter and the grid. That latency is the
  price, stated rather than hidden.
- **Frame-rate independence.** Relax passes are owed by a `RouteClock` in passes per second over
  the injected `dt`, as `GenerationClock` owes generations. Each frame's generation steps and route
  passes are encoded **in the order of their times on one merged timeline**. So 30 and 144 fps
  interleave the same events in the same order, and they reach the same committed route. The
  exception is a stall, where both clocks drop a backlog past their per-frame caps, exactly as the
  field already does.
- **The budget is the tier's.** Passes per second is a `TierConfig` field, set per tier from a
  measurement of one relax pass at each tier's grid cap. That makes the route's GPU cost
  proportional to wall time and independent of the refresh rate. It is announced nowhere, because
  nothing is clamped: a smaller budget only lengthens an epoch.
- **The look.** `route` (0..1, default 0) overlays the route on the cells beneath it, and 0 encodes
  no route pass and builds no route resource. `route_coord` is where on the palette the route is
  painted. `route_grade` is how far that coordinate travels from one end of the route to the other,
  so 0 is one solid colour and anything else grades by `t`. The colour comes from the palette, as
  every colour in this scene does. A red route is a red stop and a coordinate, not an RGB triple the
  engine has never had.

## Consequences

### Positive

- The route the owner asked for exists, refreshes without a CPU round trip, and is exact on a
  tree-shaped maze.
- Route membership is a per-cell equality test over two integer fields. A CPU mirror of the double
  sweep (`mirror.rs`) can hold the GPU to it cell for cell, the way the rule already is.
- `route = 0` is an exact identity: no pass, no buffer and no binding differs. No shipped preset
  moves and no golden re-blesses.
- The distance fields are a by-product a later look could read, such as a flood graded by distance
  from a point.

### Negative

- **The route lags the maze**, by an epoch whose length is set by the maze's diameter and the
  budget. In the worst case, a tree maze at 1024 cells a side, the path is ~524 000 steps. Even
  advancing a tile's width per pass, that is tens of thousands of passes per sweep, and the route is
  tens of seconds stale. At the shipped 192 grid the same worst case is ~18 000 steps. Neither
  latency is measured yet.
- **On a maze with loops, "longest" is the double sweep's answer, not the true diameter.** It is a
  lower bound, usually close on grid graphs and unproven here. Where two shortest paths between B
  and C are the same length, the test paints **both**, so a loop or an open room shows a widened or
  forked route rather than one line.
- **The route lives in one component.** A disconnected maze shows the longest route inside the
  component the source fell in. The first epoch's centre source can land in a small pocket, and
  continuity only corrects that if the maze later joins it to more.
- **Memory.** Two ping-pong distance buffers, the kept `dB`, the snapshot and the committed route
  are five `u32` per cell: about 20 MB at 1024, beside the scene's existing 16 MB, and about 0.7 MB
  at 192. They are built lazily on the first frame `route > 0`, rebuilt with the grid, and never
  per frame.
- **The engine's first compute work outside the attractor, and its first to use workgroup
  memory.** Until now the attractor's step and decay seed were the only compute dispatches. A
  storage-buffer layout
  joins ADR-0058's roster of unique bind group layouts. The WARP caveat in `StepParams`' docs (bind
  groups differing only in a buffer were confused) applies to a new layout, so the control and relax
  passes share one buffer set, as the step passes do.
- **Two clocks merged per frame** is new CPU code on the render path. It must not allocate: the
  frame's event sequence is generated, not collected into a list.

## Alternatives considered

### Alternative A — Full convergence inside every generation

Run every pass a sweep needs, all three sweeps, before the frame's present, so the route is exact
for the generation on screen. This is what "every generation" means literally. It lost on
arithmetic: the pass count is unbounded by anything but the diameter, which is ~18 000 steps in the
worst case at the shipped grid. A frame cannot carry that, and a cap on it turns back into the
chosen design with a frame-time spike before the cap. The chosen design is this alternative with the
pass budget spread over wall time.

### Alternative B — A CPU search over a readback

Copy the open mask back every second or two and run the double sweep on the CPU, where a BFS over
36 864 cells is microseconds. It is the simplest design here and the cheapest at the shipped grid.
It lost on the owner's answer: the interview offered "every few seconds" and the owner chose every
generation. It would also add the scene's first GPU-to-CPU readback on the render path, with its
latency and stall.

### Alternative C — Jump flooding or another distance transform

A jump flood reaches every cell in `log2(grid)` passes, about 10 at 1024. But it computes
**Euclidean** distance through free space, and a maze's distance is geodesic: it goes round walls.
On a maze a jump flood draws a route through walls. A Euclidean transform is the wrong metric,
however fast.

### Alternative D — Relax the live maze in place, with no snapshot

Keep one distance field and relax it against whatever the maze is this generation. That needs no
epochs. It lost because minimum-relaxation cannot raise a distance. When a corridor closes, the cells
behind it keep their old short distances and feed them to each other, rising one step per pass until
they reach the cap: the count-to-infinity failure. A frozen snapshot per epoch makes every sweep a
fresh search with a unique fixed point.

## Notes

- The double sweep is exact on a tree because a tree's farthest cell from any cell is an end of a
  diameter. Lower-bound behaviour on graphs with cycles is the known weakness. If the owner judges
  the forked routes on loops wrong, a deterministic tie-break that keeps one shortest path is the
  next step. It needs a path walk or pointer jumping, which is why it is not taken here.
- A coarse-to-fine search (relax a downsampled maze first to seed the fine field) would cut the
  large-grid latency. It is not taken until Plan 0253 Phase 1's reading says the latency at a
  shipped grid needs it.
