# ADR-0076 — The attractor keeps the depth it already computes

> **Status:** proposed
> **Date:** 2026-08-04
> **Related plan(s):** [0063](../plans/0063-the-attractor-keeps-its-depth.md)

## Context

The user asked whether the strange attractors can be made to look more three-dimensional. Two
families are genuinely 3-D — Thomas's cyclically-symmetric knot and the Lorenz butterfly — and both
render as flat patterns despite turning continuously.

The cause is specific and it is not a lack of shading. `project()`
(`core/src/render/scenes/particles/mod.rs:628-641`) rotates the 3-D state in the plane spanned by
`x` and the family's `bh` axis, then dots the result against two one-hot masks to get screen `x`
and `y`. The third orthogonal component — `−p.x·sin + dot(p,bh)·cos`, the exact rotation partner of
the horizontal term — is computed implicitly by that rotation and then discarded. The vertex shader
writes `out.pos = vec4(ndc, 0.0, 1.0)` (`:731`): the projection is orthographic and depth is
literally the constant zero.

An orthographic projection of a rotating transparent structure carries **no information about which
way it is turning**. The image at rotation `π` is the exact `x`-mirror of the image at rotation `0`
— at `cs = −1, sn = 0` the horizontal term becomes `−p.x` and the vertical term is unchanged — so
the two half-revolutions are indistinguishable up to reflection. With additive blending there is no
occlusion to break the tie either. The result is the classic bistable structure-from-motion
percept: the visual system cannot resolve the direction, flips between readings, and settles on
"flat pattern". The current spin makes this worse rather than better by being slow — `SPIN_RATE =
0.18` rad/s is **one revolution per 34.9 seconds**, long enough that a viewer never accumulates
motion evidence for one reading over the other.

This project has solved a version of this problem once. Plan 0043 /
[ADR-0044](0044-swarm-world-is-a-25d-torus-sized-from-the-target.md) gave the swarm a depth axis
after nearly the same complaint — flocks that "should look like birds, swirling and dancing in
3d-like space" — using four cues and explicitly no sort, because additive blending is commutative
and the per-frame sort a 3-D particle system normally pays buys occlusion an additive scene does not
have. Its own comment (`swarm.rs:68`) calls it "an honest fake": the `z` is invented per particle,
uncorrelated with anything the flock is doing.

The attractor is in a better position than the swarm was in three ways, and a worse one in a
fourth. Its depth is **real** — it is the figure's own third coordinate, so depth is correlated with
structure rather than scattered across it. Its deposit accumulates into a decaying trail, so any
depth grading persists in the *history* and not only in the current frame. Its continuous families
draw `prev → pos` segments ([ADR-0069](0069-the-attractor-trades-sample-count-for-trace-length.md)),
whose two endpoints have different depths. And two of its five families — De Jong, Clifford, plus
every IFS figure from [ADR-0075](0075-ifs-family-morphs-in-singular-value-space.md) — have no third
coordinate at all, so whatever is decided must leave them untouched.

## Decision

We will compute the view-space depth `project()` already implies, and spend it on a **perspective
divide plus two atmospheric cues** — never on occlusion. Concretely: the vertex shader magnifies
both position and sprite size by `m(d) = 1 / (1 − perspective · d_n)`, where `d_n` is the depth
normalized to roughly `[−1, +1]` by the family's own half-extent in the rotation plane; a bindable
`depth_fade` attenuates brightness with distance; and a bindable `depth_hue` shifts the palette
coordinate with distance. The hardcoded spin becomes a bindable `spin`, integrated as a phase rather
than computed from elapsed time.

`perspective` is defined as **the figure's depth half-extent as a fraction of the camera distance**,
which makes its arithmetic legible rather than magic: the near-to-far magnification ratio is
`(1 + p) / (1 − p)`, so `p = 0.5` gives 3:1 and `p = 0.8` gives 9:1. It is clamped to `0.8`, which
is where the far end has shrunk to 0.556 and the near end has grown to 5.0 — well short of the
singularity at `p = 1` where a point reaches the camera plane.

Every new lever's default is exactly the current behaviour — `perspective = 0`, `depth_fade = 0`,
`depth_hue = 0`, `spin = 1` — so no existing capture moves and no golden baseline is re-blessed.

**The 2-D families opt out by arithmetic rather than by a branch.** The CPU sends an *inverse* depth
extent, which is `1 / half_extent` for a 3-D family and **exactly `0`** for a 2-D one. Then `d_n = 0`
identically, `m(0) = 1`, the fade multiplier is 1 and the hue offset is 0 — every cue collapses to
the identity with no shader branch, no division, and no possibility of a `NaN`. The match that
produces it is exhaustive over `AttractorFamily`, so a sixth family has to answer the question. This
is also why the golden fixture cannot move: `core/tests/fixtures/attractor.toml` is De Jong.

We will **not** sort or depth-test. Additive blending stays, and distance haze stands in for
occlusion — far material is attenuated until it no longer competes with near material, which is what
reads as depth for a diffuse cloud.

## Consequences

### Positive

- **The bistability is destroyed by construction, and the property is exactly checkable.** Under
  orthography the projection at rotation `π` is the `x`-mirror of the projection at `0`; under
  perspective it is not, because `m(h) ≠ m(−h)` for any `h ≠ 0` whenever `p ≠ 0`. That is a
  dimensionless algebraic property of the formula — it holds on every machine, every adapter and
  every resolution, and it is the assertion the plan pins the change with.
- **Perspective delivers size grading and parallax as one term.** The swarm needed separate
  `DEPTH_SCALE` and `DEPTH_PARALLAX` constants precisely because it had no perspective divide to
  derive them from; here both fall out of `m(d)` and are automatically mutually consistent.
- **The trace segments foreshorten for free.** A segment's endpoints are projected independently, so
  one receding into the distance is drawn genuinely shorter — the strongest depth cue a curve has,
  at no extra cost, because ADR-0069's capsule already projects both ends.
- **The depth is the figure's own.** Unlike the swarm's seeded `z`, near and far here mean something
  about the attractor, so the Lorenz lobes separate in depth the way they actually are separated.
- **The trail inherits the grading.** A particle that was far and is now near leaves a dim streak
  behind it and a bright head — depth-shaded history, which a non-accumulating scene cannot have.
- **`spin` closes a gap the question surfaced.** No preset can currently reach the rotation of a 3-D
  attractor at all; every one of them turns at 0.18 rad/s forever.

### Negative

- **No occlusion, and the limit is the same one ADR-0044 named.** Two strands crossing simply sum,
  so the illusion degrades as density rises. Haze narrows the gap and does not close it; a dense
  `attractor_lorenz` at low `density` will still read partly as X-ray. This is the accepted cost of
  keeping the glow, and it is the user's explicit choice.
- **The segment does not taper.** Both endpoints are magnified independently, so the capsule
  foreshortens correctly, but its *width* comes from the midpoint's magnification and is uniform
  along its length. A properly tapered capsule needs the fragment's distance function to interpolate
  a radius, which is a rework of ADR-0069's one-expression point/segment unification. Left undone
  deliberately.
- **`perspective` interacts with `zoom` in a way an author must learn.** The magnification is applied
  before the view transform, so `zoom` scales an already-perspective-projected figure rather than
  changing the focal length. Pushing `perspective` up makes the figure larger overall as well as
  deeper, and recovering the framing is a `zoom` edit. Documented, not solved.
- **The `0.8` clamp is silent.** A preset asking for more gets the clamp rather than an error — the
  same undiscoverable-ceiling shape `presets/README.md` already carries for `bloom_threshold` and
  ADR-0075's `vigor`.
- **Four more names on a scene that already has fifteen params**, three of which are inert on three
  of the five families. That is the existing shape (`a b c d` are already family-specific), but it
  grows.

### Neutral

- The depth half-extent is derived from each family's existing `seed_box` — the larger of its two
  half-extents in the rotation plane, `26` for Lorenz (basis XZ, so the plane is `x`–`y`) and `4.5`
  for Thomas (basis XY, plane `x`–`z`). One derived value rather than a new hand-tuned constant per
  family, the same discipline `jitter_extent` uses.
- `spin` is integrated (`phase += spin · SPIN_RATE · dt`) rather than evaluated as `time · rate`.
  This is not a preference: a bound `spin` changing between frames would, under the multiply,
  retroactively rescale all elapsed time and snap the figure to a new angle. Determinism is
  unaffected — the phase is a pure function of the injected `dt` sequence, which captures pin at
  1/60 s.

## Alternatives considered

### Alternative A — depth-ordered rendering, so near material occludes far

Sort the particles by depth each frame, or attach a depth buffer and depth-test the deposit, so a
strand passing behind another is hidden.

Rejected on cost and on what it destroys. Sorting 150 000 particles per frame is real GPU work every
frame, and it buys nothing unless the blending changes: a depth *test* under additive blending still
sums everything that passes, so occlusion requires giving up either the additive glow or the trail
accumulation — and the accumulation is the scene. ADR-0044 declined the same trade for the same
reason and the argument has not weakened. Haze is the substitute, and its failure mode (dense
figures flatten) is visible and bounded rather than a rendering rewrite.

### Alternative B — the swarm's parallax model, with a seeded `z` extended to the 2-D families

Reuse ADR-0044 wholesale: give every particle a scattered depth, grade size and brightness by it,
and offset the view transform per depth layer so the layers separate under `pan`.

Rejected in both halves. For the 3-D families it is strictly worse than what is available — a
perspective divide produces size grading and parallax as one mutually-consistent term, where the
swarm's model needs two hand-tuned constant pairs that can disagree. For the 2-D families it is a
lie the figure contradicts: a scattered depth uncorrelated with a De Jong map's structure reads as a
flat drawing printed on panes of glass, not as a solid. The swarm gets away with it because a flock
has no structure to fight.

### Alternative C — splat into a density volume and raymarch it

Accumulate the particles into a 3-D texture and march rays through it, giving true volumetric
depth, real self-occlusion and the option of lighting.

Rejected as a different renderer for a different look. It is the honest way to get a *lit solid*,
and the user was explicitly offered it and chose otherwise — an attractor is a measure-zero point
set, and the additive glow is what makes it beautiful. It would also cost a 3-D storage texture
sized against the iGPU floor in `docs/nfr.md` §7, a march per pixel, and the loss of the 2-D trail
field every attractor preset is tuned against. Worth revisiting only if the scene's whole medium is
being reconsidered.

## Notes

The mirror identity in the Context section is the diagnosis and the test in one. It is worth
restating in the form the plan asserts: for a 3-D family, `project(q, rot = π)` equals
`project(q, rot = 0)` reflected in `x`, exactly, under orthography — and that equality must **fail**
once `perspective > 0`. A capture-level check would only say the picture changed; this says *what*
changed and why it matters.

The 34.9-second revolution figure is `2π / 0.18`. It is quoted because it is the reason a viewer
never accumulates enough motion evidence to disambiguate the rotation, and because a bindable `spin`
is the cheapest way to test whether the slowness was itself part of the complaint.
