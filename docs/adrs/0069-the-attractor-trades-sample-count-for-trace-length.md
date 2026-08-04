# ADR-0069 — The attractor trades sample count for trace length: `[particles] density` and the continuous-flow streak

> **Status:** **accepted** (2026-08-04, at Plan 0059's close) — with an **Outcome** section at the
> end. Phase 4 answered both questions this ADR deferred to it, and one answer **falsifies a
> Consequence written here as shipped fact**. The decision is unchanged.
> **Date:** 2026-08-03
> **Related plan(s):** [0059-lorenz-finds-its-plane](../plans/done/0059-lorenz-finds-its-plane.md) (Phases 2–3)
> **Depends on:** [ADR-0065](0065-the-attractor-deposit-is-normalized-by-particle-count.md) — the
> normalization that makes a preset-chosen count safe
> **Supplements:** [ADR-0007](0007-line-geometry-generators.md) (the `[particles]` table),
> [ADR-0045](0045-quality-tiers-floor-and-rich.md) (the tier budget this fraction is *of*)

## Context

[ADR-0068](0068-the-projection-basis-is-a-per-family-property.md) gives Lorenz its plane. It does
not make it legible, and Plan 0057 Phase 4 said so in advance: corrected to x–z the figure has the
right silhouette and still reads as **stipple** rather than as the banded wings of the iconic plot.

That is not a second defect. It is a property of what the scene draws. The engine renders **50 000
independent samples of the attractor's invariant measure**, one point each, refreshed every frame.
The legibility of a Lorenz plot comes from following **one trajectory** as a continuous curve for
thousands of steps — the bands are where that curve winds around each lobe, and they are not a
feature of the measure, which is smooth on the wings. Many samples and long traces are different
pictures, and today's engine can only draw the first.

**Two numbers bound the design, and both were measured rather than assumed.**

- **Per-frame travel is comparable to a point diameter.** Lorenz's mean speed on the attractor is
  ~60 world units/s, so one 60 Hz frame moves a particle ~1.0 world unit; at `projection()` scale
  `0.022` that is ~0.022 NDC against a point diameter of `POINT_BASE * 2 = 0.012`. Plan 0057 Phase 4
  measured the same ratio independently at ~1.2 diameters. **So a `prev → current` segment is worth
  roughly 1.8x a point's footprint — it closes the beading and it is not a trace.** Subdividing it
  into the four Euler sub-steps the compute loop already computes and discards buys nothing further:
  at 1.8 diameters, a chord and the curve it subtends are the same line.
- **Trace length can only come from persistence, and persistence needs sparsity.** The trail field
  (`fade`) already holds each particle's path across frames — that is the mechanism, and it is
  *screen-space*, so at 50 000 particles every wing pixel is hit constantly and any persistence
  reads as fog rather than as curves. At 640x360 the wings receive under one point per pixel per
  frame and accumulate to saturation within a few.

**The lever that would make the trade is the one thing `[particles]` does not expose.** It accepts
`family` and nothing else (`core/src/preset/schema.rs:851`), and the count is a tier constant, so a
preset cannot choose 1 000 long curves over 50 000 short samples. Both spend the same budget.

**What makes exposing it safe is new as of one plan ago.** Before
[ADR-0065](0065-the-attractor-deposit-is-normalized-by-particle-count.md), the particle count *was*
the frame's brightness — an unnormalized additive deposit — so a count key would have been an
exposure key wearing a structural name. `deposit_scale` now divides by the count actually drawn, so
lowering it raises the per-particle weight and total light is invariant by construction. The
capability and its safety arrived in the right order.

## Decision

**Two coupled changes, taken as one decision because neither delivers the picture alone.**

1. **`[particles]` gains `density`** — a fraction of the tier's own particle budget, defaulting to
   `1.0`. It selects an **active count**; it does not resize anything. The storage buffer stays
   allocated at the tier budget, the compute shader already returns early on `i >= step.count`, and
   the draw's instance count is the active count. So a density change is three integers in a uniform
   and a draw call, with **no GPU resource rebuilt** — the property Plan 0029's
   `PipelineResources` / `FieldResources` split exists to protect, extended by not needing a third
   axis. `deposit_scale` takes the **active** count, so ADR-0065's invariance carries across the new
   dial rather than being spent by it.

2. **The continuous families draw a segment, not a point.** The step shader already holds each
   particle's position before and after the frame's integration, so it writes both; the draw expands
   the instance into a quad spanning `prev → current` with the fragment's radial falloff becoming a
   distance-to-segment falloff. **Discrete maps keep the point**, and that is not a performance
   carve-out: De Jong and Clifford *replace* the state each iteration, so successive points are
   unrelated positions scattered across the figure and a segment between them is a chord across the
   whole attractor — meaningless geometry, drawn brightly.

The two families that get streaks are the two that integrate a flow. **The predicate is
`is_continuous()`, named outright**, even though it selects the same pair as `projection().1 == 3.0`
on today's roster — that agreement is a coincidence of four families, and ADR-0068's Alternative C
declines the same shortcut for the same reason.

## Consequences

### Positive
- A preset can choose its picture: many samples of the measure, or few trajectories held long. That
  is the first structural look choice the attractor family has had beyond `family` itself.
- **`density` is free at runtime and free in memory.** No reallocation, no rebuild, no second buffer
  sizing path; the inactive tail is inert because the compute already guards on the count.
- **It is also a performance dial pointing the right way.** The attractor's ceiling is additive fill
  rate (`tier.rs`), and a preset that wants curves wants *fewer* particles.
- The streak closes a real artifact that only becomes visible at low density — at 1.8 diameters of
  travel per frame, a sparse cloud's path is a dotted line. Fixing it at high density would have been
  invisible, which is why it has never been reported.

### Negative
- **A streak deposits more light than a point, and ADR-0065 does not compensate for it.** That
  invariance is over *count*, not footprint; a segment covering ~1.8x the pixels emits ~1.8x the
  light, and the ratio varies with local speed, so a fast pass through a wing tip is brighter than a
  slow one near a fixed point. **Accepted rather than normalized**, deliberately: a length
  normalization is a second constant with no measurement behind it, and speed-dependent brightness is
  arguably the correct rendering of a trajectory. The lever is the content pass, which runs once. If
  it proves unmanageable there, a length normalization is this ADR's own successor and it will have
  the measurement it currently lacks.
- **A reseed becomes a burst of long streaks.** ADR-0066's kick is `JITTER_FRACTION = 0.06` of the
  family's seed-box spread — for Lorenz about 15x a normal frame's travel — so the jitter frame draws
  a segment the particle never traversed. Whether that is an artifact to suppress or the percussive
  accent every shipped header claims is a **look judgement nobody has seen**, so both behaviours ship
  reachable and Plan 0059 Phase 4 decides it in motion. This ADR does not pre-empt it.
- **`Particle` grows from 16 to 32 bytes** (a second `vec3` plus its pad, std430). At the `Rich`
  budget that is 4.8 MB against 2.4 MB — inside the ~11 MB of overhead ADR-0010 measured as ours, but
  it is real, and the struct's "std430 stride is a tight 16" note stops being true.
- **A new preset-facing key is a new way to be wrong.** `density` interacts with `fade` and with the
  tier, and the interaction *is* the look, so it cannot be documented as independent.

### Neutral
- No C ABI change, no `Scene` trait change, no new dependency. The streak is a change to an existing
  pipeline's shaders, not a second pipeline.
- **No golden baseline moves**, checkable in advance rather than hoped for:
  `core/tests/fixtures/attractor.toml` has no `[particles]` table, so it runs the default De Jong —
  a discrete map at `density = 1.0`, which is both branches' unchanged path.
- Determinism holds. `density` is a load-time structural value; `prev` is a pure function of the same
  step sequence `pos` already was.

## Alternatives considered

### Alternative A — the streak alone, no `density`
Draw the segment and leave the count on the tier. **Rejected on arithmetic**: at 50 000 particles the
change is a ~1.8x footprint on a frame that already saturates, so it buys a denser fog and not
curves. It would ship a new draw primitive whose motivating look it cannot reach, and leave Lorenz's
legibility exactly where Plan 0057 Phase 4 left it.

### Alternative B — `density` alone, no streak
Cheapest by far, and it is the half that actually produces sparsity. **Rejected because it ships its
own artifact**: the beading a point cloud hides at 50 000 particles is plainly visible at 1 000, so
the first preset to use the new key would file the streak as a bug. Taking them together is what
lets the content pass judge one picture instead of two.

### Alternative C — a multi-segment polyline over the four Euler sub-steps
Emit all four sub-positions the compute loop already computes and draw a 4-segment polyline. Free in
integration cost, 4x in draw cost. **Rejected by measurement**: the four sub-steps span the *same*
1.8 diameters of travel, so the polyline and the single chord differ by less than a point radius, and
the cost is 4x the fill on a fill-bound pass. It becomes worth revisiting only if the frame `dt` or
the Thomas speed-up ever makes per-frame travel large enough to visibly bend.

### Alternative D — per-particle position history (a ring of N past positions)
The general form: each particle keeps its last N positions and draws them as a stroke, so trace
length is a dial independent of the trail field. **Rejected as the wrong first step**, not as wrong:
it is `N` times the storage and `N` times the draw for something `density` plus the existing `fade`
already approximates, and its real advantage — per-particle persistence that does not fog when
neighbours overlap — is only demonstrable *after* a sparse preset exists to compare against. If Plan
0059 Phase 4 finds `density` + `fade` cannot hold a curve, this is the successor and it will have a
rendered case to argue from.

### Alternative E — a bindable `density` rather than a structural key
Let the count follow the audio. **Rejected**: the active count sizes a dispatch and a draw, so an
eased value would sweep it continuously and the cloud's *identity* would change with the music — and
smoothing sweeps a parameter through every intermediate value, which for an integer count means
re-deciding what the picture is on every frame. Structural, load-time, like `family`.

## Notes

The stipple was named by Plan 0057 Phase 4 as an explicit hand-off — "one thing the ADR should know,
because a basis fix alone will not clear Phase 5's done-when" — together with the observation that
the levers are `fade` and the particle count, "content and capacity, not geometry". This ADR is that
observation taken seriously: one of those two levers did not exist, and Phase 2 of the same plan is
what made building it safe.

## Outcome (added at Plan 0059's close, 2026-08-04)

Phase 4 — the one content pass, `990fedc` — answered both questions this ADR routed to it. One
answer settles an alternative; the other **falsifies a Consequence written here as shipped fact.**

**`density` + `fade` holds a legible curve, so Alternative D does not get its case.** The
alternative was rejected as *the wrong first step* rather than as wrong, on the explicit condition
that *"if Plan 0059 Phase 4 finds `density` + `fade` cannot hold a curve, this is the successor and
it will have a rendered case to argue from."* The ladder was rendered and went the other way:
Lorenz at `density` 1.0 / 0.05 / 0.012 / 0.005 / 0.002 reads fog, fog, fuzz, curve, curve, and
Thomas at 1.0 / 0.02 / 0.005 / 0.002 reads blot, drawing, sketch, scribble. The sparse preset the
comparison was waiting for now ships (`attractor_lorenz` at `0.002`, `attractor_thomas` at `0.02`).
Per-particle position history is therefore **not** owed a successor plan. It remains available if a
future look wants per-particle persistence that does not fog when neighbours overlap — but it must
now argue that from its own merits, not from this ADR's deferred condition.

**"Both behaviours ship reachable" is false — the reseed streak cannot be A/B'd as wired, and this
ADR should not have said it could.** The Consequence above states that the jitter-frame streak and
its absence *"both ship reachable and Plan 0059 Phase 4 decides it in motion"*. They do not. Flipping
`RESEED_DRAWS_STREAK` to `true` and rebuilding produces **byte-identical output** under three
stimuli — a held `onset = 1` frame, a `click:120` filmstrip at the hops where onset peaks `1.000`,
and a twelve-frame strip of a variant reseeding on every beat. Zero pixels differ.

The cause is **frame order, not encoding**, and it is visible in the source:
`encode_jitter` runs before `encode_steps` in the same frame
(`core/src/render/scenes/particles/mod.rs`), deliberately and for a good reason its own comment
gives — the map pulls the disturbed points back within the frame, so the kick reads as the figure
being shaken rather than as a noise layer over it. But the jitter writes
`particles[i].prev = select(kicked, origin, step.coeffs.w != 0.0)` and the ordinary step then writes
`particles[i].prev = origin` unconditionally. On any frame with `pending_steps >= 1` — every frame,
at the fixed 1/60 step — the streak's `prev` is overwritten before anything is drawn.

**Nothing is broken and no preset is affected**: the shipped `false` is also what the scene does, so
the look is exactly what every header describes. What is wrong is a promise. The decision taken at
this close is to **retire the flag and the promise rather than reorder the dispatch**: moving the
jitter after the steps would buy the A/B by spending a documented, deliberate ordering rationale on
a look nobody has asked for. If the percussive whip is ever wanted, the mechanism is a **separate
streak origin written by the jitter and read by the trail pass** — decoupled from `prev`, so the
ordering survives — not a constant flip. That is engine work for a future plan; see Plan 0059's
Followups.

The general lesson is the one [ADR-0071](0071-a-numeric-test-contract-states-a-property-or-names-its-machine.md)
states for numbers, applied to a lever: **"both behaviours ship reachable" is a claim about the
running system, and it was written from the diff rather than from a render.** One capture would have
caught it, and the content pass that finally took one is where it surfaced.
