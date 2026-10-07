# ADR-0266 — The maze route is found in the quiet stretches by a double sweep on the GPU against a frozen snapshot

> **Status:** accepted 2026-10-07 (Plan 0253), with an Outcome
> **Date:** 2026-10-07
> **Related plan(s):** [0253](../plans/done/0253-the-labyrinth-shows-its-longest-path.md); design-backlog 0274

## Context

At the Plan 0232 retune the owner asked for `cellular_labyrinth` to draw the longest path through
the maze it grows, in red (backlog 0274). The `cellular` scene has no notion of a path. It holds a
state and an age per cell, and paints from those alone.

The owner answered the cadence question twice on 2026-10-07. At the interview they chose a route
that refreshes **every generation**, over "every few seconds" and over "only once the maze
settles". The first draft of this ADR priced that: a route that chases a maze that never stops
moving trails it by one whole search, with the lag set by the grid. Shown that price, the owner
**rethought it and chose the quiet stretches**: the route builds while the maze stands still, fades
out when a bite or a rule change moves it, and builds again, so it reads as being *discovered*
rather than tracked. The look was settled at the interview and did not change: **one solid colour
by default, with an author option to grade it by distance along the route** through the palette.

`cellular_labyrinth` is what the quiet stretches are made of. Its rule, `B3/S12345`, knots soup into
corridors that connect and then stand still. A disc of fresh soup lands on every fourth beat and
bites a patch out, and for 6 beats in every 32 `survive` drops to `S1234`, so the maze thins and
shifts until the rule returns and freezes the new layout. Between those events the maze is still,
or close to it, at 14 to 28 generations a second. Whether it is *exactly* still between bites, or
carries a few flickering cells, is unmeasured.

The search itself still has to run on the GPU, because a route found in a quiet stretch has to be
found inside that stretch. A breadth-first wavefront advancing one cell per pass cannot finish in a
frame. With open cells (dead cells, the corridors) about half the grid:

| grid | cells | open cells, about | worst path (a tree maze threading every open cell) |
|---|---|---|---|
| 192 (`cellular_labyrinth`) | 36 864 | 18 000 | ~18 000 steps |
| 512 (the Floor tier's cap) | 262 144 | 131 000 | ~131 000 steps |
| 1024 (`MAX_GRID`, Rich) | 1 048 576 | 524 000 | ~524 000 steps |

A maze grown by `B3/S12345` is not a tree: it has loops and shortcuts, and its real diameter is
likely far below the worst case. That is unmeasured, so this ADR does not rely on it.

Three more facts shape the answer. A **longest path** is NP-hard on a general graph; the tractable
question is the longest *shortest* path, the graph's diameter. A double sweep (search from any cell,
take the farthest cell B, search from B, take the farthest cell C) finds that path exactly on a tree
and gives a lower bound on a graph with loops. And the scene is held to reproducibility:
`thirty_and_one_hundred_forty_four_fps_reach_the_same_field` pins that the field does not depend on
the frame rate, and neither the route, nor when it appears, nor how far it has revealed may
reintroduce that dependence.

## Decision

We will find the route **only while the maze is quiet**, as a **double sweep of breadth-first
distance fields relaxed over frames on the GPU against a snapshot frozen when the quiet began**,
reveal it along its length once it commits, and fade it out when the maze moves again.

- **The graph.** Open cells are the dead cells of the state channel, joined to their four orthogonal
  neighbours, across the seam when `[cellular] wrap` is on. Live cells are walls. `cyclic` has no
  dead state, so the route is inert there, as `trail` already is.
- **Quiet is a property of the open mask, read every generation.** After each generation's step
  pass, a compare pass counts the cells whose open bit differs from the previous generation's into
  the control buffer. A generation is **still** when that count is at most `QUIET_TOLERANCE`, a
  fraction of the open cells; the maze is **quiet** once `QUIET_HOLD` consecutive generations have
  been still. Both are constants in the route module, not params, and both are set from Plan 0253
  Phase 1's reading of the labyrinth's own per-generation change counts. The property they must
  hold: the labyrinth's standing stretches between bites read quiet, and the first generation after
  a bite or a rule change does not. If the maze is exactly still between bites the tolerance is
  zero; a nonzero tolerance exists only if the reading shows flicker, and then it is sized to the
  flicker. A rule change needs no trigger of its own: it reaches the route only by moving the mask.
- **An epoch is one quiet stretch.** The first quiet generation copies the open mask into a
  snapshot buffer and starts the epoch. Three sweeps run against the snapshot. Sweep 1 starts from
  source `s` and ends at the farthest cell B. Sweep 2 starts from B, keeps its field `dB`, and ends at
  the farthest cell C. Sweep 3 starts from C and keeps `dC`. A **commit pass** then writes the route:
  every open cell with `dB(x) + dC(x) == dB(C)`, holding `t = dB(x) / dB(C)` in `[0, 1]`. Every other
  cell gets a sentinel. That membership test is the route extraction: it is fully parallel and never
  walks the path cell by cell. After the commit nothing is relaxed again until the quiet breaks and
  returns: a standing maze costs one compare pass per generation.
- **A broken quiet abandons the epoch.** A generation that is not still ends the quiet. An
  in-flight epoch is dropped where it stands, and its fields are never committed. A committed route
  starts to fade.
- **Reveal, not pop.** A committed route draws in along `t`, from B to C, over `route_reveal`
  seconds: a cell paints once the reveal front `elapsed / route_reveal` passes its `t`, with a short
  eased leading edge so the front reads as moving rather than stepping. `route_reveal = 0` paints
  the whole route at once.
- **Fade out.** From the generation that breaks the quiet, the drawn route's strength eases to zero
  over a quarter of `route_reveal`, a smoothstep in time, and the route is then cleared. Throughout,
  a route cell paints only where it is open **now**, so no route is ever drawn through a wall, even
  mid-fade over a patch a bite has just filled.
- **Relaxation.** A sweep is a compute pass repeated across frames. A 16x16 workgroup loads its tile
  and a one-cell halo from the read buffer into workgroup memory, relaxes
  `d(x) = min(d(x), 1 + min over open neighbours d(n))` there for a fixed number of local
  iterations, writes the other buffer of a ping-pong pair and counts the tiles that changed.
  Ping-pong, not in-place atomics, is what makes every pass a pure function of the previous one, so
  the field a sweep converges on does not depend on how an adapter schedules workgroups. A sweep has
  converged when a whole pass changes nothing. A one-workgroup **control pass** after each relax and
  compare pass reads the counts and advances the state on the GPU: idle, sweep 1, sweep 2, sweep 3,
  commit, shown, fading. The CPU never reads the state back.
- **Endpoints, deterministically.** The farthest cell is found in two reduction passes: the largest
  finite distance first, then the lowest cell index at that distance, so ties break by index, never
  by scheduling. The first epoch after a configure starts from the open cell nearest the grid's
  centre, lowest index on a tie. Every later epoch starts from the previous route's end C if that
  cell is still open, and from the centre rule otherwise.
- **Frame-rate independence.** Relax passes are owed by a `RouteClock` in passes per second over the
  injected `dt`, as `GenerationClock` owes generations. Each frame's generation steps (each with its
  compare pass) and route passes are encoded **in the order of their times on one merged timeline**,
  and every compare and control pass is handed its own event's scene time (the sum of injected `dt`,
  as `u32` milliseconds since configure). The control pass stamps the commit and the quiet's break
  with those event times, and the present computes the reveal and the fade from the frame's own
  scene time against the stamps. So 30 and 144 fps reach the same quiet, the same commit at the same
  scene time, and the same reveal front at the same wall time. The exception is a stall, where both
  clocks drop a backlog past their per-frame caps, exactly as the field already does.
- **The budget is the tier's.** Passes per second is a `TierConfig` field, set per tier from a
  measurement of one relax pass at each tier's grid cap, so the route's GPU cost is proportional to
  wall time and independent of the refresh rate. Nothing is clamped: a smaller budget lengthens an
  epoch, and an epoch longer than a quiet stretch shows nothing that stretch.
- **The look.** `route` (0..1, default 0) overlays the route on the cells beneath it, and 0 encodes
  no route pass and builds no route resource. `route_coord` is where on the palette the route is
  painted. `route_grade` is how far that coordinate travels from one end of the route to the other,
  so 0 is one solid colour and anything else grades by `t`. `route_reveal` (seconds) is the reveal
  time and sets the fade. The colour comes from the palette, as every colour in this scene does. A
  red route is a red stop and a coordinate, not an RGB triple the engine has never had.

## Consequences

### Positive

- The route the owner asked for exists, appears in the maze's still moments as something found
  rather than something tracked, and is never drawn stale through a wall.
- A standing maze costs one compare pass per generation once its route is committed, so the route's
  steady cost on a settled maze is near zero.
- Route membership is a per-cell equality test over two integer fields. A CPU mirror of the double
  sweep (`mirror.rs`) can hold the GPU to it cell for cell, the way the rule already is.
- `route = 0` is an exact identity: no pass, no buffer and no binding differs. No shipped preset
  moves and no golden re-blesses.
- The distance fields are a by-product a later look could read, such as a flood graded by distance
  from a point.

### Negative

- **A quiet stretch shorter than an epoch shows no route.** The epoch's length is set by the maze's
  diameter and the tier's pass budget, and nothing is drawn until it commits. At the labyrinth's 192
  grid the worst case is ~18 000 steps. Advancing a tile's width per pass that is about 1 100
  passes, and a real maze with loops needs far fewer, so an epoch is expected to fit inside a quiet
  stretch between bites; that is unmeasured until Plan 0253 Phase 1. At 1024 the worst case is
  ~524 000 steps, tens of thousands of passes, and a preset that bites every few seconds at that grid
  will rarely or never show a route. The route is a small-grid look, or a look for a preset that
  holds still.
- **A preset that never goes quiet never shows a route.** A rule that flickers past the tolerance
  everywhere, or a reseed every beat, keeps the route dark. That is the chosen behaviour, not a
  failure, and the author sees it at once.
- **The tolerance and the hold are constants measured on one preset.** They are argued from the
  labyrinth's change counts. A second preset with a different flicker profile may want them as
  params; that is a later change with its own evidence.
- **On a maze with loops, "longest" is the double sweep's answer, not the true diameter.** It is a
  lower bound, usually close on grid graphs and unproven here. Where two shortest paths between B
  and C are the same length, the test paints **both**, so a loop or an open room shows a widened or
  forked route rather than one line, and the reveal front advances down both branches at once.
- **The route lives in one component.** A disconnected maze shows the longest route inside the
  component the source fell in. The first epoch's centre source can land in a small pocket, and
  continuity only corrects that if the maze later joins it to more.
- **Memory.** Two ping-pong distance buffers, the kept `dB`, the snapshot, the previous generation's
  mask for the compare, and the committed route are six `u32` per cell: about 24 MB at 1024, beside
  the scene's existing 16 MB, and under 1 MB at 192. They are built lazily on the first frame
  `route > 0`, rebuilt with the grid, and never per frame.
- **The engine's first compute work outside the attractor, and its first to use workgroup
  memory.** Until now the attractor's step and decay seed were the only compute dispatches. A
  storage-buffer layout joins ADR-0058's roster of unique bind group layouts. The WARP caveat in
  `StepParams`' docs (bind groups differing only in a buffer were confused) applies to a new layout,
  so the compare, control and relax passes share one buffer set, as the step passes do.
- **Two clocks merged per frame** is new CPU code on the render path. It must not allocate: the
  frame's event sequence is generated, not collected into a list.

## Alternatives considered

### Alternative A — Chase the moving maze, with the lag stated

The first draft of this ADR: start a new epoch whenever the snapshot compare finds the maze changed,
so the route refreshes continuously and always trails the maze by one search, tens of seconds at
1024 in the worst case. It was the literal reading of the owner's first answer, "every generation".
**The owner rejected it on 2026-10-07** once the lag was priced: on a maze that never stops moving,
a route that is always one search behind reads as wrong rather than as found. The quiet stretches
keep its mechanism and change only when an epoch starts and what the route does when the maze moves.

### Alternative B — Full convergence inside every generation

Run every pass a sweep needs, all three sweeps, before the frame's present, so the route is exact
for the generation on screen. It lost on arithmetic: the pass count is unbounded by anything but the
diameter, ~18 000 steps in the worst case at the shipped grid. A frame cannot carry that, and a cap
on it turns back into a relaxed search with a frame-time spike before the cap.

### Alternative C — A CPU search over a readback

Copy the open mask back and run the double sweep on the CPU, where a BFS over 36 864 cells is
microseconds. It is the cheapest design at the shipped grid. It lost on the owner's first answer,
which declined "every few seconds", and it would add the scene's first GPU-to-CPU readback on the
render path, with its latency and stall. Quiet detection would also need the change count back on
the CPU every generation, a second readback.

### Alternative D — Jump flooding or another distance transform

A jump flood reaches every cell in `log2(grid)` passes, about 10 at 1024. But it computes
**Euclidean** distance through free space, and a maze's distance is geodesic: it goes round walls.
On a maze a jump flood draws a route through walls. A Euclidean transform is the wrong metric,
however fast.

### Alternative E — Relax the live maze in place, with no snapshot

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
- A coarse-to-fine search (relax a downsampled maze first to seed the fine field) would shorten an
  epoch, and so let a shorter quiet stretch or a larger grid show a route. It is not taken until
  Plan 0253's readings say a shipped preset's quiet stretches are shorter than its epochs.

## Outcome (2026-10-07, Plan 0253)

The sizing premise in the Context table and in the first Negative bullet does not describe the
labyrinth. Plan 0253 Phase 1 read `cellular_labyrinth`'s grown maze at generation 600 (llvmpipe):
its 17 432 open cells form **2 881 separate four-connected pockets**, the largest holding **142
cells**. The centre rule's route is **16** long, and the longest route forced into the largest
pocket is **79**. Phase 2 measured the epochs at **14 to 24 work passes**, about **0.1 s** at the
`Floor` rate of 237 passes per second, against a standing stretch of about 1.4 s between bites.

So the labyrinth draws a **short route inside one pocket**, not a path across the maze. The Notes'
"the route lives in one component" predicted the kind of this; the reading shows it is the common
case rather than the exception. Whether that route reads as "the longest path" is what Plan 0253
Phase 4, the owner's look, judges. The ~18 000-step figure stays as the worst case on a connected
maze at the 192 grid, and the per-tier figures above stand as worst cases in the same sense.
