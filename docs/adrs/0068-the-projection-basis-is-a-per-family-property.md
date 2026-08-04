# ADR-0068 — The 3-D projection basis is a per-family property: Lorenz renders x–z

> **Status:** proposed
> **Date:** 2026-08-03
> **Related plan(s):** [0059-lorenz-finds-its-plane](../plans/0059-lorenz-finds-its-plane.md) (Phase 1)
> **Supplements:** [ADR-0007](0007-line-geometry-generators.md) — the `[particles]` family roster
> **Arises from:** [Plan 0057](../plans/done/0057-the-attractors-compute-path.md) Phase 4, which
> diagnosed the cause and stopped by its own instruction

## Context

The attractor's draw shader has **one** 3-D projection, shared by every family that takes the
branch. It uses `y` as the vertical and rotates `x` against `z`
(`screen = vec2(cx*cs + cz*sn, center.y)`, `particles/mod.rs:355-360`), so the view is x–y at rest
and z–y at a quarter turn.

**The Lorenz butterfly lives in x–z, and neither of those views is it.** Plan 0057 Phase 4 confirmed
this by measurement rather than by argument, and ruled out the two alternative causes the backlog
entry had proposed:

- *Not integration, and not an un-converged seed.* Read back off the particle buffer, Lorenz occupies
  **5.89 % of its own bounding volume, stable from 60 through 240 to 600 frames**, at
  `x ∈ [-18.0, 19.2]`, `y ∈ [-25.3, 25.4]`, `z ∈ [4.4, 47.3]` — the classic attractor's bounds. An
  un-converged seed box reads ~26 %, measured the same way. Forward-Euler thickening and un-converged
  corners would both show as a fill fraction that *shrinks* with frame count; it does not move.
- *The discriminating capture*, rendered with `SPIN_RATE` pinned to `0` so the frame is the rest
  basis rather than the 41° the spin reaches by frame 240 — without that pin neither view is the one
  being reasoned about, which is why the first attempt was unreadable:

  | basis | rest view | what it renders |
  |---|---|---|
  | shipped `vec2(cx*cs + cz*sn, center.y)` | x–y | a hard **X / bowtie** — the two lobes seen edge-on, crossing. This is the reported "dense core inside a diffuse cloud", verbatim |
  | swapped `vec2(cx*cs + cy*sn, center.z - zc)` | x–z | the **butterfly silhouette** — two lobes, the notch top and bottom centre, the two fixed-point cores as vertical streaks at low gain |

Lorenz is the only family this can be wrong for. De Jong and Clifford are 2-D and never take the
branch. Thomas is cyclically symmetric under `x → y → z → x`, so no basis is privileged for it.

**Why this is an ADR and not a constant.** Changing a *shared* convention for one member of a set is
a decision about where family-specific knowledge lives, and there are three defensible places to put
it. Plan 0057 Phase 4 was written to stop and route here precisely because the answer is not a
number.

## Decision

**`AttractorFamily` gains a projection basis alongside its existing `projection()`, and Lorenz
returns x–z while every other family keeps today's x–y.** The basis reaches the draw shader as a
uniform — a pair of axis selectors, not a second pipeline — so the branch stays one draw call.

No preset surface. The six shipped presets all want the same answer for their family, which is the
same ground [ADR-0065](0065-the-attractor-deposit-is-normalized-by-particle-count.md) and
[ADR-0066](0066-a-reseed-disturbs-the-cloud-rather-than-replacing-it.md) declined a `[particles]`
key on.

**Thomas keeps x–y, and it is unchanged by construction rather than by symmetry.** Its cyclic
symmetry is why it does not *need* x–z; it is not a reason a basis change would be free. Swapping
Thomas's vertical would render a rotated figure with different pixels, so the way to leave Thomas
alone is to leave Thomas alone.

## Consequences

### Positive
- The one family carried by colour rather than by geometry gets its shape. `attractor_lorenz` has
  shipped as a dust cloud for as long as it has shipped.
- **The fix is exactly as wide as the defect.** One family's basis moves; three families' captures
  are byte-identical, and the golden fixture runs De Jong, so no baseline moves.
- A future 3-D family has to answer the question, because the match is exhaustive.

### Negative
- **A shared convention becomes a per-family table**, which is one more thing to keep in step with
  `seed_box`, `projection` and `default_coeffs`. The mitigation is that they are all `match self` on
  the same enum, side by side, and the compiler makes the set exhaustive.
- **`attractor_lorenz`'s exposure was authored against the wrong figure.** A cloud seen edge-on and a
  butterfly seen face-on do not occupy the same pixels, so the preset owes a re-tune — Plan 0057
  Phase 6 withheld it deliberately for this reason, and Plan 0059 Phase 4 pays it.
- **This does not make Lorenz legible on its own**, and the plan says so up front. Corrected to x–z
  the silhouette is right and the figure still reads as stipple. That is
  [ADR-0069](0069-the-attractor-trades-sample-count-for-trace-length.md), and the two land in one
  plan because they move the same preset's look.

### Neutral
- No C ABI change, no `Scene` trait change, no new dependency, no new pass. One uniform gains two
  fields.
- Determinism is untouched: the basis is a load-time property of the family, not a function of time
  or audio.

## Alternatives considered

### Alternative A — a preset-facing view parameter
A `[particles] basis = "xz"` key, or a continuous bindable view pitch so an author could animate the
viewing plane on a beat. **Rejected** because it makes every attractor preset answer a question five
of the six have no opinion about, and answering it wrongly is silent. It also does not remove the
need for a default, so the per-family table would still have to exist underneath it. The expressive
version is a superset of this decision and can supersede it later if a look ever wants it; taking it
first would ship a key with one correct value.

### Alternative B — re-centre Lorenz's coefficients so the butterfly lands in x–y
Swap `y` and `z` at the source of the ODE, leaving the shared projection untouched. Genuinely
smaller in the renderer. **Rejected** because it makes the shipped `sigma`/`rho`/`beta` name
something other than the textbook Lorenz system, which `attractor_lorenz.toml`'s header,
`presets/README.md` and every reference an author would check all cite. A preset author bumping
`rho` would get the documented behaviour of a different axis. Trading a renderer convention for a
mathematical one is the wrong direction.

### Alternative C — key the basis off `projection().1 == 3.0`
Free, since the two 3-D families are exactly the two that would ever want a basis. **Rejected as the
`aspect` trap in another costume** (ADR-0037's generalization, restated in the review lens): `dim`
and "wants a non-default basis" agree on today's roster of four and are not the same property. A
2-D family with a preferred orientation, or a 3-D family happy with x–y, breaks the coincidence
silently, and no test at today's roster could tell which one the code consulted. The basis is named
outright.

## Notes

Raised by `preset-author` on 2026-08-03 as [backlog 0048](../design-backlog.md), which proposed three
causes — none of them this one. Diagnosed and routed by Plan 0057 Phase 4, whose done-when was "the
cause is named and the discriminating capture is described" and which explicitly forbade `dev` from
writing the fix once the cause turned out to be a convention.
