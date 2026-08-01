# ADR-0057 — Objects that spawn, fall and die live in a new emitter scene, on analytic ballistics with seeded per-object individuation

> **Status:** proposed
> **Date:** 2026-08-01
> **Related plan(s):** [0052](../plans/0052-the-emitter-objects-that-spawn-fall-and-die.md)
> **Supplements:** [0007](0007-line-geometry-generators.md) (the scene-idiom split this adds a
> fifth member to), [0036](0036-preset-reachable-spectrum.md) (the per-element channel this
> deliberately does **not** use), [0044](0044-swarm-world-is-a-25d-torus-sized-from-the-target.md)
> (the torus this scene exists to not be), [0056](0056-additive-scenes-emit-premultiplied-alpha.md)
> (the seam invariant its sprite pipeline inherits).

## Context

The engine has no model for an object with a life. `preset-author` filed this as
[design-backlog 0034](../design-backlog.md) after a user request for a Solitaire-style cascade —
"падает красиво по параболе в разных направлениях" — and four pieces were verified missing:

- **No emitter and no lifetime.** `swarm::Particle` is `{ pos, vel, z }`; nothing spawns and
  nothing dies. Worse, the swarm's world is a **torus** — `bounds(aspect)` wraps every particle
  back into frame ([ADR-0044](0044-swarm-world-is-a-25d-torus-sized-from-the-target.md)) — so a
  particle *categorically cannot fall out of shot*, which is the entire motion of a cascade.
- **No gravity and no ballistic path.** Steering is a flow field plus a radial `burst`. A flow
  field bends every particle in a region the same way, which is a current, not a throw.
- **No per-object state.** A binding is evaluated once per frame. The one per-element channel,
  `Scene::set_param_series` ([ADR-0036](0036-preset-reachable-spectrum.md)), is fed
  only by `spectrum`. So `hash()` cannot give each object its own launch angle, spin or twinkle
  phase, and a starfield blinks *as one sheet* rather than as individual stars.
- **No stamped trail.** `trails` is a fade-and-accumulate smear over the finished frame; it cannot
  stamp hard copies along an arc.

The backlog entry names three candidate shapes and declines to pick: a new `SystemKind`, an
extension of `swarm`, or a general per-object expression facility. The user chose the first on
2026-08-01. Two forces make the remaining choices real decisions rather than detail.

**Frame-rate independence is a settled constraint here, not an aspiration.** Plan 0014 retired
`SCENE_DT` for an injected real `dt` precisely so animation is identical on every device, and
`Scene::advance(dt)` is the channel. A parabola integrated with semi-implicit Euler at variable
`dt` accumulates position error of order `dt` — the same class of divergence that plan removed. So
*how* a falling object gets its position is a decision, not an implementation detail.

**Per-object expression evaluation has a known cost shape and no measurement.** `set_param_series`
exists and works, but `spectrum` drives it with ~64 elements. A cascade wants thousands. Evaluating
an expression tree per object per frame on the render thread is a hot-path budget question nobody
has measured, and `docs/nfr.md` has no line for it.

## Decision

**We will add `SystemKind::Emitter`: a CPU-integrated scene, alongside `swarm` rather than inside
it, in which objects spawn from a source, follow an analytic ballistic path, age, and are retired
when their lifetime expires or they leave an off-screen bound.**

Three sub-decisions carry the weight:

**Position is analytic, not integrated.** Each object stores its spawn time, spawn position and
launch velocity; its position at scene time `t` is `p0 + v0 * (t - t0) + 0.5 * g * (t - t0)^2`.
This is *exactly* frame-rate independent by construction — there is no accumulator to drift and no
`dt` in the position at all — and it costs one multiply-add chain per object with no per-frame
state write. It also makes a test able to assert a property with earned arithmetic: an object
launched with vertical speed `v0` against gravity `g` reaches its apex at `t = v0 / g`, at height
`v0^2 / (2g)`, on any frame cadence.

**Per-object variation comes from a per-object seed, not from per-object expressions.** Each object
draws a `seed` once at spawn from the scene's `SeededRng`
(splitmix64, `scenes/mod.rs:414` — the NFR 6 discipline that visual randomness is explicitly
seeded); every individuating quantity —
launch angle within a spread, size within a range, spin rate, twinkle phase — is a pure function of
that seed and the preset's **distribution** params. The preset controls the distribution; the seed
picks within it. `set_param_series` is left alone.

**The sprite pipeline is the swarm's, and it takes
`gpu::ADDITIVE_LIGHT_SATURATING_COVERAGE`** ([ADR-0056](0056-additive-scenes-emit-premultiplied-alpha.md)).
The emitter is a third draw seam, so it owes the third lit-backdrop guard that ADR names, and it
inherits `vec4(colour * g, g)` rather than re-deriving it.

Note what this decision is not. It is **not** a shape vocabulary — objects are still the round
additive blob of [design-backlog 0033](../design-backlog.md). The backlog itself separates them:
"an emitter that throws round blobs on parabolas is buildable without touching the additive model,
and would already read as a shower." This ADR takes the half that carries the motion.

## Consequences

### Positive

- **A whole idiom becomes reachable** — showers, cascades, sparks, rising embers, falling snow,
  starfields that twinkle per star. The backlog calls "a shower of *things*" a mainstream
  visualizer idiom the library cannot currently offer at all.
- **Frame-rate independence is structural, not tested-in.** The analytic path has no accumulator,
  so there is no configuration in which the trajectory can diverge between devices. That closes the
  question Plan 0014 had to fix rather than reopening it.
- **Determinism is free and the existing discipline covers it.** A seeded spawn RNG plus an
  analytic path means the whole scene is a pure function of `(seed, scene time)`. The golden suite
  can pin it without a settle window.
- **`swarm` is untouched.** ADR-0044's torus is exactly right for a murmuration and exactly wrong
  for a cascade; keeping them separate means neither has to grow a mode switch.
- **The hot path stays free of expression evaluation.** Spawn is the only per-object event, and it
  is bounded by a rate param rather than by object count.

### Negative

- **A fifth scene idiom to maintain, and the wiring is duplicated.** Palette, view transform,
  mirror, composite params and the sprite pipeline all get a second CPU-particle implementation
  beside the swarm's. Some of that is genuinely shareable and some will be copied; we accept the
  copy rather than inventing a particle-scene base class for two members.
- **Analytic position forbids interaction.** No drag, no flow-field steering, no collision, no
  mid-flight force change — the path is fixed at spawn. A look that wants a falling object to
  *swirl* is not reachable, and getting there means integration and reopening this ADR. This is the
  real price and it is the one most likely to be felt.
- **Per-object individuation is bounded by what a distribution can say.** `hash(index)`-style
  authoring — an arbitrary expression per object — stays impossible. An author can widen a spread
  but cannot write "every seventh object is gold". The narrower surface is deliberate; it is also
  strictly less expressive than the rejected alternative.
- **A new scene means a new golden baseline, a new fixture, and a third lit-backdrop guard.** The
  suite grows, and the emitter's spawn behaviour is the first scene whose object count varies
  during a capture — the `sanity` / `animation` / `reactivity` gates all measure whole-frame
  statistics and none has met a scene that starts empty.
- **Retirement is a hot-path allocation hazard.** A naive spawn/die model allocates. The scene must
  hold a fixed-capacity pool with a free list, sized from `TierConfig`, and it must be impossible
  for a preset to drive spawn rate past the pool.

### Neutral

- Objects are still round additive blobs. Whether the engine should draw *shaped* marks stays open
  in [design-backlog 0033](../design-backlog.md), and an emitter is the natural first consumer of a
  shape vocabulary if one is ever built.

## Alternatives considered

### Alternative A — extend `swarm` with spawn, lifetime and gravity params

Cheapest surface: additive params on the scene that already owns CPU particles, and every existing
`swarm_*` preset keeps working untouched. **Rejected because ADR-0044's toroidal wrap is precisely
what a cascade must not have.** `bounds(aspect)` exists to keep the field populated with no
respawns and no hitches, and the wrap seam is deliberately pushed off-screen; an object that falls
out of shot is the opposite requirement. Satisfying both inside one scene means a mode switch that
changes the world topology, which is two scenes wearing one name — an open/closed violation that
would need this same ADR to justify, with worse ergonomics for the content lane (`system = "swarm"`
would mean two unrelated things depending on a param).

### Alternative B — build a general per-object expression facility first

Extend `set_param_series` from `spectrum`'s ~64 elements to arbitrary per-object evaluation, then
build the emitter on top. Strictly more expressive, and it subsumes the grammar-side "no stateful
expressions" gap the `preset-author` skill also carries. **Rejected on hot-path cost with no
measurement behind it.** Evaluating an expression tree per object per frame at cascade counts is a
per-frame cost nobody has measured, `docs/nfr.md` has no budget line for it, and the first thing it
would buy is variation a seeded distribution already provides. Building the emitter on a seed makes
the facility optional rather than load-bearing; if authors later hit the distribution ceiling, the
facility can be added *underneath* an emitter that already ships, with a real workload to measure
against instead of a hypothetical one.

### Alternative C — integrate ballistically with a fixed-timestep accumulator

`gpu.rs` already holds a fixed-timestep accumulator and the reaction-diffusion scene uses one, so
this is the house pattern and it permits drag and steering. **Rejected because the analytic form is
exactly right and strictly cheaper for the motion actually asked for.** An accumulator makes the
trajectory depend on the substep size and on when the object spawned relative to a substep
boundary, which is a reproducibility surface the analytic form simply does not have. The
capability it buys — mid-flight forces — is not in scope, and buying it now would mean paying the
determinism cost before anything needs it. If a look later wants swirl, this is the alternative to
revisit, and the ADR to supersede.

### Alternative D — stamp the trail, using the existing feedback stage

The Solitaire cascade reads partly because each card leaves hard, non-fading copies along its arc,
and `trails` is a decaying smear. **Rejected as out of scope rather than wrong** — a non-decaying
stamp is a change to the feedback stage's semantics ([ADR-0031](0031-post-stage-trait-instantiable-composite-chain.md)
territory), it affects every scene rather than this one, and an emitter without it already delivers
the motion. Recorded as a followup on the plan.

## Notes

The severity of the "no per-object state" gap is worth keeping visible even though this ADR routes
around it. The user's original ask was that stars *blink* ("мигают") and what the engine produced
was a field-wide flash, because the binding is evaluated once per frame for the whole scene. A
seeded twinkle phase per object fixes exactly that case. It does not fix the general one, and the
general one stays filed.
