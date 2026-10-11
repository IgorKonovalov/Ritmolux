# ADR-0272 — A solid voxel takes light from its neighbours through a coarse field rebuilt each generation

> **Status:** proposed
> **Date:** 2026-10-11
> **Related plan(s):** [0263](../plans/0263-solid-voxels-light-each-other.md); builds on [ADR-0269](0269-a-solid-voxel-is-lit-by-one-key-light-and-its-own-occlusion-and-its-light-blooms-like-any-other.md), [ADR-0268](0268-a-3d-automaton-is-a-voxel-system-marched-as-an-emitting-absorbing-volume.md)

## Context

ADR-0269 lights a solid cell with one key light, an ambient term and the occlusion of its own
corners. Nothing in that shade reads another cell's light. Backlog 0295 records what that costs. The
voxel set's three solid worlds (Ember, Neon, Ice) want cubes that glow and spill colour onto their
neighbours. They fake it with `ambient = 0.75` and a heavy bloom over a `0.55` threshold. The halo
spills in screen space, so it lights empty space and the background as much as the next cube, and it
cannot tell an inner face of a structure from an outer one.

Two facts shape the answer. First, the volume changes only when the automaton steps. That happens at
`step_rate` generations a second (8 on the shipped set), not every frame, and a pass already runs
after every state-changing frame: the brick pass that marks occupied 8³ bricks for the march to skip.
Second, the march's live frame rate is an open question (backlog 0290). A term that adds work to
every lit pixel of every frame is the most expensive place to put new cost.

## Decision

We will give `voxel` a modal, bindable `bounce` parameter, default `0`, read only under
`present = "solid"`, and feed it from a **coarse light field rebuilt per generation**:

- **The field** is a 3D `rgba16float` texture of side `grid / 4` (16³ on `Floor`, 32³ on `Rich`).
  Each texel holds the live fraction of the 4³ block of cells it covers and the mean palette
  coordinate (age) of those live cells. Decay-stage cells do not emit into it.
- **It is rebuilt beside the brick pass**, after every frame that changed the state (a generation,
  a reseed stamp, the seed). It is built in two compute dispatches: a splat that reduces each 4³ block,
  then one 3³ blur into a second texture, which the march samples with trilinear filtering.
- **The shade gains one term.** At a solid hit, the march samples the field half a cell in front of
  the face it entered:

```
L     = field(hit + n * half_cell)        # trilinear: (intensity, age)
shade = albedo * (ambient * ao + key * max(0, dot(n, l)))
      + bounce * palette(L.age) * L.intensity * brightness * shell(cell)
```

  The colour is looked up at shade time, so a bound `hue`, `palette_mix` or `hue_spread` stays
  current between generations. Only the intensity and the age are a generation old.
- **At `bounce = 0` the field does not exist.** The texture and both pipelines are built lazily on
  the first frame with `bounce > 0`, filled from the current state that same frame, and the march
  reads a constant that skips the fetch. So every shipping frame is byte-identical, and an unbound
  preset pays nothing. This is bloom's skip discipline.

The bounce term enters the composite as linear light, and bloom treats it like any other light, as
ADR-0269 decided for the key light.

## Consequences

### Positive
- Cubes that glow light the faces near them, and that light falls off with distance in the volume
  rather than on screen. An inner face lit by its neighbours reads differently from an outer face
  lit only by the key.
- The per-frame cost is one trilinear fetch per lit pixel. The rest is paid per generation, over a
  texture 64 times smaller than the grid.
- Bloom stops being the only way to make a solid world glow, so a preset can lower its bloom and keep
  the look's light inside the structure.

### Negative
- **Light leaks through walls.** The field has no occlusion: a cell lights a face on the far side of
  a solid slab as readily as an open one. At a 4-cell texel and one blur, the leak reaches about
  6 cells. This is the price of not tracing rays, and it reads as translucency on thin structures.
- **It is soft by construction.** One texel spans 4 cells, so the light cannot pick out a single
  neighbour. A crisp one-cell glow is not reachable with this field.
- **The emitter's shell is not the receiver's.** The spectrum's shell gain is applied at the
  receiving face, not the emitting cell, so on a strongly shelled preset the light a face receives
  follows that face's own band rather than its neighbours' band. Accepted as an approximation.
- **The field lags by up to one generation.** At 8 generations a second that is 125 ms, and the
  automaton itself moves on the same clock, so it does not read as lag.
- One more lazily built texture pair and two pipelines, on a system that already holds a ping-pong
  pair, a brick texture and three pipelines.

## Alternatives considered

### Alternative A — gather the neighbours at every hit
At the hit, sum the emission of the 3x3 slab of cells in front of the face (or a 3³ block). This is
exact, needs no new pass, and gives a crisp one-cell glow. It lost on cost and reach. It costs 9 to 27
texture fetches on every lit pixel of every frame, on the march whose live frame rate is already in
question (backlog 0290), and light reaches only one cell.

### Alternative B — keep the bloom fake
This is the status quo, and the shipped look is accepted as it stands. It lost because the halo is
screen-space: it lights the background and the space between cubes, and it brightens a face whether
anything is next to it or not. It stays available, since `bounce` is opt-in and composes with bloom.

### Alternative C — propagate light with occlusion (light propagation volumes, cone tracing)
This iterates light through the field while respecting solid cells, so walls block it. It lost on
cost and complexity: several dependent passes per generation, a second field for occlusion, and
iteration counts that would need tuning per grid. The leak in the Negative section is what we accept
in exchange.
