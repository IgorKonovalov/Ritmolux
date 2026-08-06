# ADR-0087 — The IFS particle carries its age and its last map, and respawns onto the attractor

> **Status:** proposed
> **Date:** 2026-08-05
> **Related plan(s):** [0073](../plans/0073-the-fern-unfurls-and-colours-by-what-made-it.md)

## Context

[Plan 0062](../plans/done/0062-the-chaos-game-grows-a-fern.md) landed the IFS family and deliberately
left two things out, on the grounds that they are the same per-particle channel and should be tuned
against a figure already known to be right: **continuous unfurling** and **depth / per-map colour**.
Its content pass then found a third thing, which turns out to share the same mechanism —
[backlog 0064](../design-backlog.md): the initial fill scatters particles over the figure's bounding
box, so a switch into the family shows a **legible, hard-edged, axis-aligned rectangle** for roughly
two thirds of a second, with the ~1 s preset dissolve landing entirely inside it. That is the same
artifact class [ADR-0066](0066-a-reseed-disturbs-the-cloud-rather-than-replacing-it.md) was written
to remove from `reseed`, back on a different path.

All three want the same thing: **a particle that knows how old it is, knows which map last moved it,
and restarts somewhere legal.**

Three facts constrain the answer.

**There is exactly one free word per particle, and two channels want it.** `Particle` is 32 bytes —
`pos: vec3<f32>`, `seed: f32`, `prev: vec3<f32>`, `pad: f32` — where `pad` is written to `0.0` at
seed time and read by nothing. Age and last-map are genuinely independent: age is a continuous ramp
that resets, last-map is a categorical value that changes every step. One `f32` cannot carry both
without aliasing them.

**"Somewhere legal" has a closed form, and only for this family.** [ADR-0075](0075-ifs-family-morphs-in-singular-value-space.md)'s
Notes record that any contractive map's fixed point `(I − M)⁻¹ t` lies *on* the attractor, because
`A = ⋃ fᵢ(A)` with `A` closed makes each `fᵢ`'s fixed point the limit of `fᵢⁿ(x)` for any `x ∈ A`.
That property is a consequence of the parameterization, not of any plan. **It does not exist for De
Jong, Clifford, Thomas or Lorenz** — those have no closed-form on-attractor point to restart at, and
respawning them into their seed box is precisely the ADR-0066 artifact, per particle and forever.

**The last map applied is a property of position, not of history.** A particle's coordinates are the
product of its whole chain of map choices, but the map applied *most recently* determines which
sub-copy `fₖ(A)` it currently sits in — the fern's stem, body, left frond, right frond. So a
one-value channel refreshed every step genuinely partitions the figure into its four parts. It is
not a per-particle identity that would read as noise.

## Decision

We will **widen `Particle` from 32 to 48 bytes**, adding `age: f32` and `map: f32` (plus the two
words of tail padding WGSL's 16-byte struct alignment forces), and use them to drive three things on
the IFS family and on no other: a **continuous staggered respawn onto the drawn maps' fixed points**,
an **age-derived colour**, and a **last-map-derived colour**. Both colour channels reach the picture
through **two routes each** — a palette coordinate and a hue offset — as four params defaulting to
zero.

The struct grows rather than packing both channels into the free word. Aliasing them (integer part
the map index, fractional part the normalized age) costs no bytes and no ADR, but it makes every
read site carry a `floor`/`fract` convention that nothing enforces, drops the age channel to roughly
twenty usable bits, and forecloses a third channel entirely. Forty-eight bytes buys two clean
channels **and leaves two words free**, so the next thing that needs a per-particle value is not
another struct change to a type four families share.

The respawn is **continuous** — every particle carries a lifetime derived from its own fixed `seed`,
so the phases are spread across the buffer and a small fraction restarts each step — rather than a
one-time unfurl at preset switch. That choice is what makes the age channel worth having: under a
one-time unfurl every age saturates within about 0.4 s and the colour goes uniform, so the feature
would be visible for the first second of a preset and then never again. Under continuous churn the
population always holds every age, the gradient is permanent, and **the startup rectangle never
forms**, because there is no bulk fill at any moment — which is how backlog 0064 dies as a side
effect rather than as a fix.

The respawn target is safe for the same reason the rest of this family is. `I − M` is invertible for
every contractive `M` (a contraction has no eigenvalue of modulus 1), and the inverse is bounded:
`‖(I − M)⁻¹‖ ≤ 1/(1 − σ_max)`, which at ADR-0075's `0.97` ceiling is at most `33.3`. So a fixed point
is finite for every reachable table — every morph position and every lever extreme — by the same
scalar comparison the whole family rests on, with no guard and nothing to check on the GPU. The
fixed points are computed on the CPU from the resolved table and uploaded with it.

**The initial seed uses the same fixed points**, which is the actual repair for backlog 0064. It is a
change to what `seed()` writes for this family — **not** to `AttractorFamily::seed_box`, because
`jitter_extent` is derived from `seed_box` as a fraction of its spread, and collapsing that spread
would make `reseed` silently inert on the whole family.

A newly respawned particle's brightness ramps from zero over its first several steps. This is not
polish: a fixed rate at 150 000 particles lands on the order of a thousand particles per frame onto
exactly four points, and the trail field integrates that into four bright dots. The ramp means those
points deposit almost no light, and by the time a particle is bright it has been iterated enough
times to have spread across the figure.

## Consequences

### Positive

- **Backlog 0064 is removed by construction rather than patched.** There is no moment at which the
  particle population is a uniform box, so there is no rectangle to fade out. The seeded fill and the
  steady state become the same thing.
- **The age channel has something to show forever.** A permanent gradient from the fixed points
  outward is structure the figure did not previously expose — the IFS is otherwise a photograph at
  fixed levers, which is the exact complaint backlog 0066 raised about the family.
- **Per-map colour partitions the figure into its parts.** A fern whose stem, body and two fronds
  are distinguishable is a different picture from one that is a single colour ramp, and it comes
  from a value the step shader already computed and threw away.
- **Two spare words remain.** The next per-particle channel is a field addition, not a layout
  decision.
- **The safety argument is unchanged in shape.** The fixed point is bounded by the same `σ_max` the
  family already clamps, so nothing new is provable only by rendering.

### Negative

- **Storage traffic rises 50 %** — 4.8 MB to 7.2 MB at the 150 000-particle ceiling, for every
  family including the four that never read the new fields. Small against the trail field, and noted
  because it is a struct four families share and the cost is paid by families that gain nothing.
- **The respawn rate and the emergence ramp are look constants with no principled values**, in the
  same position ADR-0075's `0.97` occupies. If the churn reads as twinkle, those constants are the
  lever.
- **The rate is not bindable, deliberately**, so a preset cannot make the churn a visible instrument
  — a beat cannot restart a burst. That is a real expressive limit and it was declined at interview
  to keep one look decision out of the content pass; it is the obvious follow-up.
- **Four new params on a scene that just took five.** The `attractor` roster is the longest in the
  library and its family-specific meanings are already the one place `presets/README.md` has to warn
  about inert bindings.
- **The IFS golden baseline moves.** `attractor_ifs.png` was captured against a box-seeded fill; it
  is re-blessed once. Every other baseline must stay byte-identical, and a second moved baseline is
  a defect rather than an expected cost.

### Neutral

- The step uniform grows again, by two `vec4`s carrying the four fixed points, from 160 to 192
  bytes. The bind-group layout gains no binding, so the collision surface
  [ADR-0058](0058-bind-group-layout-collisions-carry-evidence.md) reasons about does not change
  shape.
- Determinism is preserved. A particle's lifetime and its respawn target are pure functions of its
  fixed `seed`; its age is a pure function of the accumulated injected `dt`, which captures pin at
  1/60 s.
- The IFS draws no segment (`is_continuous() == false`), so a respawning particle's teleport cannot
  draw a chord across the figure the way it would on a continuous family.

## Alternatives considered

### Alternative A — pack both channels into the existing free `f32`

Keep `Particle` at 32 bytes and encode `floor(pad)` as the map index and `fract(pad)` as the
normalized age.

Rejected on maintainability rather than on capability — it does work. The convention lives in
nothing but comments, so every future read site is one forgotten `fract` away from reading an age of
`2.4`; the age channel loses roughly four bits to the index; and a third per-particle value has
nowhere to go, so the struct change is deferred rather than avoided. The saving is 2.4 MB of storage
on the largest tier, against a trail field that is already several times that.

### Alternative B — defer per-map tint again and use the free word for age alone

Ship unfurl and age colour only, at 32 bytes, and return per-map tint to the backlog.

Rejected because it has already been deferred once by Plan 0062 for exactly this reason, and the two
channels are independent — there is no version of "one slot" that serves both. Deferring again
converts a layout decision into a recurring cost, and the second deferral would face the identical
choice with no new information.

### Alternative C — a one-time unfurl at preset switch and reseed, with no continuous churn

Start every particle at a fixed point and let it grow; do not respawn thereafter.

Rejected because it makes half this decision pointless. Every particle's age saturates within about
0.4 s — the family's own probability-weighted per-step contraction of `0.742` is what sets that — so
the age-derived colour is a uniform value for all but the first second after a switch. Backlog 0064
would be fixed and the colour channel would have nothing to show, which is the more expensive half
of the work done for a startup animation.

### Alternative D — respawn into the figure's seed box rather than at a fixed point

Reuse the existing box and simply stagger the refill in time.

Rejected because it is backlog 0064 re-implemented at a smaller scale and made permanent. A particle
restarted inside the bounding box is **off** the attractor and deposits light there while it
contracts — the ADR-0066 artifact, spread thinly across every frame instead of concentrated at the
switch. The closed-form fixed point costs one 2×2 inverse per map per frame on the CPU and is on the
figure at step zero.

## Notes

`(I − M)⁻¹` is computed in closed form from the resolved 2×2, not iterated. For
`M = [[a, b], [c, d]]` the inverse of `I − M` is `1/Δ · [[1 − d, b], [c, 1 − a]]` with
`Δ = (1 − a)(1 − d) − bc`, and `Δ ≠ 0` follows from contractivity rather than being checked: `Δ` is
`det(I − M)`, which vanishes only if `M` has an eigenvalue of `1`, which a map with `σ_max < 1`
cannot.

**"On the attractor" is a theorem, and a chaos run cannot assert it.** This is worth writing down
because Plan 0073's first draft tried to, and the attempt fails in a way that looks like a tuning
problem. The membership `pₖ ∈ A` follows from `A = ⋃ fᵢ(A)` with `A` compact and each `fᵢ`
contractive: `pₖ = lim fₖⁿ(x)` for any `x ∈ A`, and `A` is closed. There is nothing left for a
measurement to establish. What a chaos game produces is a finite sample of a measure whose *support*
is `A` — so it can fail to contradict membership, and it can never certify it.

Two consequences, both found by measurement during implementation:

- **A bounding box is a supremum statistic**, fixed by the single rarest point the orbit reached, so
  two runs' boxes disagree by an amount that iteration count does not control. Measured on the fern:
  under-coverage `0.046` at 20 k iterations and `0.143` at 100 k — it *grows*. Both boxes approach the
  true extent from below at a rate set by how thin the invariant measure is at the tips, and their
  difference does not shrink like a mean would.
- **Nearest-approach measures distance to a finite sample, not to `A`.** That has a resolution floor
  at every probe, including probes genuinely on the attractor, since no orbit point lands exactly on
  a fixed point. So it does not separate a fixed point from a seed-box corner — the two are a
  difference of small numbers both dominated by sample density.

What *is* assertable is the arithmetic: the residual `‖M p + t − p‖`, whose float bound follows from
`cond(I − M) ≤ (1 + σ_max)/(1 − σ_max) ≤ 65.7`, and the magnitude bound `‖p‖ ≤ ‖t‖/(1 − σ_max)`
above. A wrong transcription of the closed form is the only thing that can actually be wrong here,
and both catch it exactly. The same residual is what separates a fixed point from any other point,
with a provable margin: `‖fₖ(q) − q‖ ≥ (1 − σ_max)‖q − pₖ‖`.

Only maps with `p > 0` are legitimate respawn targets — a padded slot's fixed point is on the
attractor only when the pad duplicates a drawn map, which is true of all five curated tables today
and is exactly the sort of thing that stops being true when a sixth figure is added. The CPU writes
the drawn maps' fixed points into all four uniform slots, duplicating as needed, so the shader picks
one of four unconditionally and needs no branch and no knowledge of the probability table.
