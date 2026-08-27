# ADR-0135 — Every scene rate integrates through one shared `Phase`, and a guard asserts it

> **Status:** proposed
> **Date:** 2026-08-27
> **Related plan(s):** [0122](../plans/0122-every-rate-integrates.md)
> **Supplements:** [0132](0132-a-rate-parameter-integrates-a-phase.md) (the rule this enforces),
> [0076](0076-the-attractor-keeps-the-depth-it-already-computes.md) (the first integrated phase), [0002](0002-layered-preset-architecture.md) (the seam this does not widen)

## Context

[ADR-0132](0132-a-rate-parameter-integrates-a-phase.md) decided that **every bindable rate parameter
in this engine integrates a phase** rather than scaling absolute scene time, then enumerated two
sites. The enumeration was wrong twice over, and both errors were found by grepping rather than by
reading the list.

Plan 0121's close review found a third site. Planning this correction found two more. The engine
holds **six** bindable rates; three integrate and three multiply:

| site | rate | today | bound by shipped content |
|---|---|---|---|
| `fragment_field.rs` | `fold_speed`, `field_speed` | integrates (`Phases`) | yes — `fragment_driftmono` |
| `warp_mesh/mod.rs` | `warp_speed` | integrates (`integrate_phase`) | no |
| `particles/family.rs` | `spin` | integrates (`advance_spin`) | yes — six attractor worlds |
| `swarm.rs:812` | `spin` | `self.time * self.spin` | **yes — `swarm_shatter`, `swarm_drift`** |
| `lines/parametric.rs:410` | `spin` | `self.spin * self.time` | constants only |
| `warp_mesh/mod.rs:2070` | `deposit_spin` | `self.deposit_spin * self.time` | no |

The three that are correct are **three separate implementations of the same three lines**, written
by three different plans that never saw each other: `Phases::step`, `integrate_phase`, and
`advance_spin`. Each carries its own doc comment explaining the same rule from scratch.

That shape has a recent, expensive precedent in this repo. `band_contour` was one function written
three times; Plan 0121 Phase 5 found it was written **four** times, and that the fourth copy had
already drifted — which the drift assertion could not catch, because it iterated the three sites it
knew about and the one place drift had happened was the one place it could not look. A rule enforced
by a list of sites fails the same way whether the list is in a test or in an ADR.

The engine already holds everything the fix needs. `Scene::advance(&mut self, dt)` is called every
frame with the injected real elapsed time ([ADR-0013](0013-c-abi-v4-render-dt.md)), and the
per-frame order is `set_time` → `advance` → `reset_params` → `set_param` → `update`, so a scene can
store `dt` and integrate against *this* frame's parameter values without any widening of the `Scene`
trait.

## Decision

We will put **one `Phase` type** in `core/src/render/scenes/mod.rs`, make every bindable rate in the
engine advance through it, and add a hygiene assertion that no scene source multiplies the clock by a
settable field. The three existing implementations collapse into it; the three defective sites move
onto it.

`Phase` is a newtype over `f32` with exactly one mutator:

```rust
// illustrative — not the final interface
pub(crate) struct Phase(f32);

impl Phase {
    /// One frame's integration, at *this* frame's rate. Called from
    /// `Scene::update`, never from `advance` — `advance` runs before the
    /// frame's `set_param` calls and would use the previous frame's rate.
    fn step(&mut self, rate: f32, dt: f32) { self.0 += rate * dt; }
    fn get(self) -> f32 { self.0 }
}
```

**No constant scale is folded into the accumulation, and that is load-bearing rather than tidy.**
`advance_spin`'s own doc comment already records why: the attractor's `SPIN_RATE` multiply is
deferred to the read so that at `spin = 1` the accumulator is `Σ dt` term for term — bit-for-bit the
same summation the renderer performs for its own clock — and no golden baseline moves. Folding a rate
in would sum `0.18 · dt` instead and drift in the last bits of every capture. `Phase` therefore
accumulates `rate · dt` and nothing else; a scene with its own constant applies it on read, as
`spin_phase` does today.

**The guard is a text assertion in `core/tests/hygiene.rs`**: no source under
`core/src/render/scenes/**` matches `self.<ident> * self.time` or `self.time * self.<ident>`. It
targets a *field* multiplying the clock, so the roughly ten legitimate `time * <literal>` uses in
`warp_mesh/shader.rs` — the reference's own fixed frequencies, none of them settable — are untouched.

## Consequences

### Positive

- **The rule becomes checkable instead of enumerated.** A fourth defective site fails the build on
  the commit that writes it, which is the one thing neither ADR-0132's prose nor a reviewer's grep
  can promise.
- **One doc comment instead of four.** The reason a rate must be integrated is stated once, at the
  type, where the next scene author meets it while writing the rate rather than after shipping it.
- **`swarm_shatter` and `swarm_drift` stop lurching.** They are the only shipped presets binding a
  multiplied rate to audio, and the correction is the whole visible payoff of this ADR.
- **No golden may move, and that is arithmetic rather than hope.** Migrating the three correct sites
  preserves the summation term for term. Of the three defective sites, `swarm_shaped.toml`,
  `parametric_curve.toml` and `DEFAULT_DEPOSIT_SPIN` all sit at rate `0`, where the two forms are
  exactly equal; `swarm.toml`'s `spin = 0.1` is the one fixture where `Σ(rate · dt)` and
  `rate · Σ(dt)` differ at all, and they differ in the last bits of an `f32`.

### Negative

- **The guard catches a shape, not a semantics, and it is evadable in one line.** Binding the clock
  to a local first — `let time = self.time;` then `time * self.spin` — passes it. Both `swarm.rs` and
  `emitter.rs` already bind that exact local for unrelated reasons, so the evasion is not
  hypothetical; it is one edit away in two files that already have the first half. The guard raises
  the cost of the mistake; it does not make it impossible, and its own doc comment has to say so
  rather than letting a reader mistake it for a proof.
- **Three scenes' internals move in one plan**, and `particles/tests.rs` names `advance_spin` in five
  places. Those tests are the existing evidence for the integrated form and must survive as tests of
  the shared type rather than be deleted with the function.
- **Two shipped presets change how they look**, and neither has an unbound-parameter escape hatch the
  way `warp_speed` had. The correction cannot land on an equivalence assertion alone; it needs a
  content pass and a human verdict, which is why the plan carries one.
- **`Phase` is a new shared item in `scenes/mod.rs`**, which is today almost entirely the `Scene`
  trait and its registry. It is a small widening of what that module is for.
- **It does not make `update` idempotent, and `update` is called twice per frame in one case.**
  `scene_for_mut` is keyed by `SystemKind`, so during a same-system dual-live dissolve the outgoing
  and live sides resolve to one scene instance and every `Phase` in it integrates at 2x for the
  dissolve's duration. Pre-existing — `particles::spin_time` and the emitter's field step already do
  this — and structurally invisible to tests, because headless capture always answers `Freeze`. This
  ADR moves three more rates into that class without repairing it; see design-backlog 0142.

### Neutral

- The `Scene` trait is untouched. `advance` and `update` already exist and already carry `dt` and the
  post-`set_param` ordering; nothing here needs a new method, so [ADR-0002](0002-layered-preset-architecture.md)'s
  seam holds exactly as written.
- `parametric_curve` gains an `advance` it does not have today — it currently takes the trait's
  default no-op — which is a new line rather than a new concept.

## Alternatives considered

### Alternative A — the text guard alone, leaving three helper copies

The cheapest option, about twenty lines, no new abstraction and no ADR. Rejected because it guards
the defect and not the duplication, and the duplication is the mechanism by which the defect
returns. This repo has the counterexample from ten days ago: `band_contour` was three verbatim copies
under a drift assertion, and it was four copies with one already drifted. A guard that says "no site
does X" plus four independent implementations of "how to not do X" is the same structure that failed
there, and the failure is silent in both directions.

### Alternative B — put the integration in the `Scene` trait and let the engine do it

Have `Scene` declare its rate parameters and have `draw_frame` integrate them, so a scene cannot get
it wrong because a scene cannot do it at all. Genuinely tempting, and rejected on the seam:
[ADR-0002](0002-layered-preset-architecture.md) keeps `Scene` thin on purpose, and this would add a
method every implementor must answer — including the seven scenes with no rate at all, which is
precisely the ISP violation that ADR forbids. It also does not fit the shapes: the attractor defers a
constant scale to the read, `fragment_field` runs two phases against two different rates, and
`warp_mesh` runs two phases in one scene. The engine would need to know each scene's scale factors
and pairings, which is scene knowledge living in the renderer.

### Alternative C — a newtype with full arithmetic (`Deref`, `Add<f32>`, `Mul<f32>`)

Make `Phase` read like an ordinary `f32` at the use sites so the migration is a smaller diff.
Rejected because the constraint *is* the value: the type exists to make `+= rate · dt` the only way
the accumulator moves, and an `Add<f32>` impl re-opens exactly the door it was built to close. A
scene could then write `phase + self.spin * self.time` and compile.

### Alternative D — correct the three sites and add no guard and no type

The smallest diff, and the plan would be two phases instead of five. Rejected because it is the
option that has already been chosen twice. ADR-0132 corrected `warp_speed` "rather than leaving it as
a counterexample" and shipped with three live counterexamples it had not looked for; a third
correction with no mechanism behind it is the same bet a third time.

## Notes

The six rates and their three current implementations are tabulated in the Context above rather than
in the plan, because the plan will move to `plans/done/` and this table is the thing a future reader
needs when they add a seventh.

`swarm.rs`'s defect is the one with a measurable size: on `swarm_shatter` at t = 100 s, a one-pole at
`tau = 0.3` closes about 5.4 % of its gap per 60 Hz frame, so `spin` moves ~0.04 in a frame across
its 0.75 swing and the field clock advances ~4.0 s in that frame against a nominal 0.019 s. That is
the number the content pass is correcting away, and it grows without bound as a set runs.
