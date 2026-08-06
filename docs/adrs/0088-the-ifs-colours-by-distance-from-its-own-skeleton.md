# ADR-0088 — The IFS colours by distance from its own skeleton, and the age channel is retired

> **Status:** proposed
> **Date:** 2026-08-06
> **Related plan(s):** [0074](../plans/0074-the-figure-colours-by-how-far-it-has-come.md)
> **Supplements:** [ADR-0087](0087-the-ifs-particle-carries-its-age-and-its-last-map.md), whose age
> channel this replaces rather than repairs

## Context

[ADR-0087](0087-the-ifs-particle-carries-its-age-and-its-last-map.md) gave every particle two
channels and claimed both would colour the figure. One does. Its Outcome section, written at Plan
0073's close, records that the other does not, and **why it structurally cannot**:

- The age channel's stated reading is "distance-from-the-fixed-points in disguise" — a young
  particle has been iterated only a few times, so it sits near one of the four points the figure
  contracts toward.
- That holds only for a particle's first handful of steps. The family's probability-weighted
  per-step contraction of `0.742` decorrelates position from age after roughly ten iterations.
- The first **eight** steps are exactly what `EMERGENCE_STEPS` deliberately makes invisible, because
  a just-restarted particle sits on one of four points and a thousand of them per frame would burn
  four dots into the trail field.

So the ramp hides precisely the material the age channel exists to colour. The two constants
ADR-0087 treats as independent look knobs are in opposition. Measured at Plan 0073 Phase 6 in the
most favourable configuration the preset surface can build — bare fern, `fade = 0` so no trail could
average anything, a three-stop maximum-contrast ramp, `age_tint = 0.75` — and swept across the whole
morph range. The figure comes out a uniform tint carrying fine multi-coloured noise.

The gap is not that the *idea* was wrong. Distance from the figure's contraction points is real
structure, permanent, and exposed by nothing else in this family. The gap is that **age was a proxy
for it, and the proxy decays.** [backlog 0074](../design-backlog.md) names three ways out; this is
the second, chosen because it delivers what ADR-0087 wanted rather than negotiating with what it
built.

Two facts about the code constrain the answer. The four fixed points live on the **step (compute)
uniform** and not on the draw uniform, so the colour path cannot reach them today. And ADR-0087
deliberately left **two spare `Particle` words**, named rather than implicit, described in that ADR
as "the budget for the next per-particle channel".

## Decision

We will **compute each particle's distance to the nearest of the drawn maps' fixed points in the
step shader, normalise it against the figure's own skeleton, and store it in one of the two spare
`Particle` words**, reaching the picture through the same two routes ADR-0087 established — a
palette coordinate (`root_tint`) and a hue rotation (`root_hue`). **`age_tint` and `age_hue` are
retired** in the same change, so the roster does not grow.

The channel is a **pure function of position**, recomputed every step from the point the particle
currently occupies. That is the whole difference from the age channel, and it is the reason this one
does not decay: a particle five hundred steps old sitting near a fixed point reads a distance near
zero, exactly as a freshly restarted one does. The emergence ramp still dims the small fraction of
the population that has *just* respawned, but the neighbourhood of each fixed point is otherwise
occupied by old, bright particles — the fixed point is on the attractor, so the invariant measure
has support there. **This is the claim on which the whole decision rests, it is the same *kind* of
claim ADR-0087 got wrong, and Plan 0074 therefore gates on it mid-plan rather than at the end.**

**The normaliser is the diameter of the fixed-point set**, `max over drawn j, k of ‖p_j − p_k‖` — at
most six pairwise distances, computed on the CPU beside the points themselves and shipped as a
reciprocal. It is closed-form, exact, deterministic, and moves exactly as smoothly across the morph
as the fixed points do, because it is a continuous function of them. It is also the *meaningful*
scale: the figure's own skeleton, not a bounding box drawn around its excursions.

It carries one hazard that is not hypothetical. Two drawn maps' fixed points can approach each other
as the morph interpolates between two tables, and if all of them coincide the diameter goes to zero
and its reciprocal diverges. So the reciprocal is **floored**, and the floor is not a hope: Plan 0074
sweeps the whole morph range and both lever extremes and asserts the measured minimum diameter sits
above it, printing the observed margin and where it occurred.

**The step uniform does not grow.** `StepUniform` carries `_pad: [u32; 3]`, three explicitly named
padding words that exist because the scalar block has to round up to the `vec4` table's alignment.
The reciprocal takes one and leaves two. The struct stays **192 bytes**, the bind-group layout gains
no binding, and the collision surface [ADR-0058](0058-bind-group-layout-collisions-carry-evidence.md)
reasons about does not change shape.

`Particle::age` **stays**. It is what drives the emergence ramp, which is load-bearing. Only the two
params that read it for colour are retired.

## Consequences

### Positive

- **ADR-0087's intended picture actually ships.** A permanent gradient from the figure's contraction
  points outward, on a figure that is otherwise a photograph at fixed levers — the complaint
  [backlog 0066](../design-backlog.md) raised about this family.
- **The roster does not grow.** Two dead params out, two live ones in. ADR-0087's Consequences
  flagged "four new params on a scene that just took five" as a knowing risk; this settles it
  without adding to it.
- **No struct growth, no uniform growth, no bind-group change.** The channel costs one already-paid
  spare word and one already-paid padding word. `Particle` stays 48 bytes with one word still free.
- **The normaliser is provable rather than sampled.** Six distances in closed form, against a Monte
  Carlo estimate that this repo has already proved does not converge (below).
- **Determinism is unchanged in shape.** The distance is a pure function of the particle's position
  and the resolved table, both of which are already deterministic.

### Negative

- **It spends one of the two words ADR-0087 reserved.** One remains. The next per-particle channel
  after this one is a struct change to a type four families share, which is exactly the cost
  ADR-0087 paid 48 bytes to defer — deferred once more, not avoided.
- **The claim that the gradient survives the emergence ramp is argued, not yet measured.** It is the
  same *class* of claim ADR-0087 made and got wrong. The argument here is different in kind — a
  property of position rather than a proxy for it — but the honest position is that this ADR is
  betting on reasoning that its own predecessor's reasoning failed at. Plan 0074 gates on it after
  one phase for that reason.
- **A per-step `min` over four distances on every particle.** Four subtractions, four dot products
  and three comparisons in the IFS arm of the step shader, at up to 150 000 particles per fixed
  step. Small against the maps themselves, and paid only by the IFS.
- **`attractor_ifs.png` moves again** — the fixture binds `age_tint`/`age_hue` today, and both the
  retirement and the new pair change the picture. It is the only baseline that may move; a second is
  a defect.
- **The diameter floor is a constant with no principled value**, in the same position ADR-0075's
  `0.97` and ADR-0087's `180` occupy. It is bounded by measurement rather than chosen by taste,
  which is better, but it is still a number somebody picked.

### Neutral

- The distance is exactly `0` at the instant of respawn, by construction — the particle *is* at a
  fixed point. That is self-consistent rather than a special case: it is one end of the ramp.
- Every other family leaves the word at its seeded `0.0`, so both new params are exactly inert
  there, by the same mechanism that makes `map_tint` inert — the engine zeroes the whole channel row
  off the IFS rather than relying on a default.

## Alternatives considered

### Alternative A — two-step map history (16 sub-copies instead of 4)

Remember the previous map alongside the last one, giving `f_j(f_k(A))` — sixteen sub-sub-copies
rather than four. Purely categorical, so it needs no distance, no normaliser and no morph guard, and
it extends the one mechanism that **demonstrably works today**.

Rejected, and it was close. It is a genuinely good idea and cheaper than what we chose. It loses
because it is *more of the same reading* — finer partitioning of a figure already partitioned —
where the thing ADR-0087 promised and failed to deliver is a **continuous** cue. Two colour channels
that are both categorical would leave this family with no gradient at all, and the interview
question that started Plan 0073 asked for depth. Worth revisiting on its own merits later; it does
not conflict with this decision and could use the remaining spare word.

### Alternative B — distance to its *own* map's fixed point

`map` already names which point, so this is one distance rather than a `min` over four, and it means
"how far into this sub-copy am I".

Rejected because it resets the gradient inside every part: four repeating ramps rather than one
reading of the whole figure. That is a texture, not a depth cue, and on the fern it would fight
`map_tint` — the two would partition on the same boundaries.

### Alternative C — put the four points on the draw uniform and compute the distance per frame

Leaves both spare `Particle` words free, at the cost of `DrawUniform` growing by 32 bytes and the
vertex shader recomputing up to four distances for every particle every frame.

Rejected on cost placement rather than on capability. The step shader already holds the points and
already runs per particle per step; the draw path would recompute a value that does not change
between draws of the same step. It also spends a uniform's bytes on all five families to serve one.

### Alternative D — normalise by `chaos_extent`

Use the existing Monte Carlo bounding-box estimator as the scale.

Rejected on this repository's own evidence. `chaos_extent` returns a **bounding box, which is a
supremum statistic** — fixed by the single rarest point the orbit reached. ADR-0087's Notes record
the measurement: two runs' boxes disagree by `0.046` at 20 000 iterations and `0.143` at 100 000, and
the disagreement **grows** with iteration count. Normalising a colour coordinate by that would make
the gradient's scale wobble across the morph by an amount nobody chose, and would cost a 20 000-step
Monte Carlo on every preset switch. The fixed-point diameter is exact, six operations, and smooth.

### Alternative E — make the emergence ramp authorable and keep the age channel

[backlog 0074](../design-backlog.md)'s first route: shorten or expose `EMERGENCE_STEPS`, accept some
hot spots, and let the age gradient show.

Rejected as a *replacement*, accepted as an *addition*. It trades an artifact the ramp was built to
remove against a gradient that decays anyway after ten steps, so it buys a worse version of what
Alternative-free route 2 delivers outright. But the ramp genuinely does interact with `fade`
independently of any colour channel — ADR-0087's Risks flagged that a ramp sufficient at `fade = 0.86`
may not be at `0.94` — and that reason survives this decision. Plan 0074 therefore ships
`emergence` as a param on its own merits, in a late phase, and not as the age channel's rescue.

### Alternative F — retire `age_tint` / `age_hue` and build nothing

[backlog 0074](../design-backlog.md)'s third route. Two params no preset can use is a real cost, and
deleting them closes the entry.

Rejected because it accepts the loss rather than repairing it, and the repair is cheap: the points
are already computed, already uploaded, and already correct. It remains the fallback if Plan 0074's
Phase 2 gate says the gradient does not read — in which case the retirement half of this ADR ships
and the rest does not, which is a successful outcome and is written into the plan as one.

## Notes

The distance is stored **normalised** rather than raw, so the draw path multiplies by nothing and
the value is directly a `[0, 1]` palette coordinate. Values above `1` are legitimate — the diameter
of the fixed-point set is not an upper bound on how far the attractor reaches — and are clamped at
the read rather than at the write, so the stored quantity stays a faithful measurement.

The one thing this ADR wants a future reader to check before trusting it: **the gradient's survival
is a claim about the invariant measure having support near each fixed point**, which is true as a
theorem (each `p_k` is on `A`, and `A` is the support) but says nothing about *density*. A figure
whose measure is very thin near one of its fixed points would show that region as sparse rather than
as coloured. That is a per-figure property, it is not provable in general, and it is the reason Plan
0074's gate is a rendered sample set across all five figures rather than one capture of the fern.
