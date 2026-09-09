# ADR-0152 — The frame delta is sanitized once, at the scene seam

> **Status:** accepted 2026-09-08
> **Date:** 2026-08-29
> **Extends:** [ADR-0135](0135-every-scene-rate-integrates-through-one-shared-phase.md)
> **Related plan(s):** [0140](../plans/done/0140-every-rate-integrates-for-real.md)

## Context

[ADR-0135](0135-every-scene-rate-integrates-through-one-shared-phase.md) put every bindable rate
behind one `scenes::Phase`, whose reason to exist is that `+= rate · dt` becomes the only way an
accumulator moves. **It does not constrain what `dt` may be.** `Phase::step` is `self.0 += rate * dt`
with no precondition, and a single non-finite frame writes `NaN` into a `Phase` **permanently** — the
type has no other mutator, so nothing can ever clear it.

The defence against that exists, byte-identical, in four separate `Scene::advance` impls:

```
fragment_field.rs:446    dt.is_finite() && dt > 0.0 else FALLBACK_DT
lines/parametric.rs:330  dt.is_finite() && dt > 0.0 else FALLBACK_DT
swarm.rs:724             dt.is_finite() && dt > 0.0 else FALLBACK_DT
warp_mesh/mod.rs:1712    dt.is_finite() && dt > 0.0 else FALLBACK_DT
```

`particles/mod.rs:1339` writes `self.dt = dt` unguarded, and `update` then runs
`self.spin_time.step(self.spin, self.dt)`. A `NaN` or negative `dt` from the shell — a suspended
window, a clock that jumps backwards, a `dt` computed across a device loss — lands in that `Phase`
and stays. **Every attractor world's display rotation would be dead for the rest of the process.**

The omission reads as safe on inspection, which is the interesting part: `FixedStep::advance` on the
line above **self-heals**, because `accumulator.min(step)` returns `step` when one operand is `NaN`.
A reader checking the neighbouring line finds a guard and moves on.

**This is the shape ADR-0135 was written against, one level down.** That ADR's own Context calls four
copies of the same three lines *"a rule enforced by a list of sites"* and rejects its Alternative A
on the ground that the duplication is how the defect returns. Four copies of the `dt` guard is that
sentence again, and the site with no copy is the one it predicts.

Nothing has been observed. The shells feed real elapsed time and no capture path produces a bad `dt`,
which is why this survived a plan whose whole subject was these accumulators. The cost of the repair
only grows: `Phase` is now the engine's one rate mechanism, so every rate added after this inherits
whichever answer is not chosen.

## Decision

**`dt` is sanitized once, in `draw_frame`, before `Scene::advance` is called.** No scene ever
receives a non-finite or non-positive frame delta, and all four in-scene copies of the guard are
deleted along with the site that lacks one.

The renderer's contract to a scene gains one clause: **the `dt` passed to `advance` is finite and
strictly positive.** That is a widening of what the renderer promises, and it is deliberate — it is
the only placement that fixes every reader at once, including the ones that are *not* `Phase`.
`swarm`'s damping `powf` and `warp_mesh`'s `pow` both consume `dt` directly, so a guard inside
`Phase::step` would leave those unprotected and the four callers would keep their own copies with a
narrower job.

`FALLBACK_DT` moves to the seam with the guard, and keeps its current value, so a frame with a bad
delta advances by a nominal step rather than freezing.

## Consequences

### Positive
- The invariant is stated and enforced in one place, which is ADR-0135's own argument applied to its
  own gap.
- Every `dt` reader is covered, not only the `Phase` ones — which is the half a `Phase::step` guard
  would have missed.
- The attractor's live hole closes without adding a fifth copy of the guard, which is the option
  ADR-0135 already rejected once by name.
- Every rate added after this inherits the guarantee rather than the obligation.

### Negative
- **It widens the `Scene` trait's contract**, which is an [ADR-0002](0002-layered-preset-architecture.md)
  question rather than an edit. The trait stays the same *shape* — no new method, no new argument —
  but what `advance` may assume about its argument changes, and that is a real interface claim that
  must be written into the trait's doc comment or it will be forgotten.
- **A scene author can no longer see the guard at the site that needs it.** The protection becomes
  invisible from inside a scene, so someone reading `particles/mod.rs` in isolation cannot tell that
  `self.dt = dt` is safe. The doc comment on `advance` is the only thing standing there.
- The four deleted guards each carried a separately-written comment giving the same reason; that
  reasoning has to survive the deletion in one place rather than four, or the *why* is lost with the
  duplication.

### Neutral
- No behavioural change in any observed scenario. The shells have never produced a bad `dt`; this is
  a guard against a class, not a fix for a symptom.

## Alternatives considered

### Alternative A — Sanitize inside `Phase::step`
One line, kills all four copies, and puts the invariant on the type that exists to hold invariants —
which is genuinely the most elegant reading. Rejected because `dt` has readers that are **not**
`Phase`: `swarm`'s damping `powf` and `warp_mesh`'s `pow` both take it raw, so those callers would
still need their own guard and the duplication survives with a narrower job and a less obvious
reason. It fixes the mechanism that was noticed rather than the class.

### Alternative B — A `Dt` newtype that cannot be constructed non-finite
Strongest guarantee: the invariant becomes unrepresentable-if-violated and the compiler enforces it
at every call site forever. Rejected for this pass on diff size — it changes the signature of
`advance` and of `step` and touches every scene and every test constructing a frame delta, which is
a wide edit across files that [Plan 0126](../plans/done/0126-the-large-files-split-along-their-seams.md)
is concurrently splitting. It is the right long-term shape and this decision does not foreclose it:
the seam guard is where a `Dt` would be constructed.

### Alternative C — Copy the fourth guard into `particles/mod.rs`
Closes the one live hole for three lines. Rejected explicitly: it leaves **five** copies, which is
precisely the option ADR-0135 rejected by name, and it would be the third time this project chose
a list of sites over an invariant.

## Notes

Discharges [design-backlog 0150](../design-backlog.md). The related question of whether the
`hygiene.rs` guard should reach `* age` is **not** settled here — see
[ADR-0153](0153-a-per-element-rate-integrates-per-element.md), which is the same rule against a
different clock.

## Correction — 2026-09-07, before acceptance

Read against the tree at `775ef18`, before Plan 0140 Phase 2 was implemented. The Decision stands;
two facts in the Context above do not, and both widen the phase rather than change it.

**The population is six, not four.** Plan 0126's splits landed two more guards on scenes after this
was written:

```
shape_collage.rs:1165    dt.is_finite() && dt > 0.0 else 0.0
warp_mesh/draw.rs:235    dt.is_finite() && dt > 0.0 else rate 1.0   (Exposure::new)
```

Both are deleted with the four. At `Exposure::new` the surrounding `.clamp(0.0, 4.0)` is **not** part
of this invariant — it caps a long frame at four nominal frames — and survives the deletion.

**The four copies were never one policy, which strengthens the argument rather than weakening it.**
The Context calls them "byte-identical", and the four `FALLBACK_DT` sites are. The two found since
are not: `shape_collage` **freezes** on a degenerate frame and `Exposure` substitutes one nominal
frame, which at `NOMINAL_FPS = 30.0` is `dt = 1/30` rather than `FALLBACK_DT`'s `1/60`. Three
different answers to one question, none of them wrong at its own site and none of them reviewable
against the others. That is what a list of sites costs even when every entry is defensible.

**`transition.rs:329` is out of scope, and it is the boundary this correction exists to draw.**
`Transition::advance` reads `dt` from inside `draw_frame` and so sits downstream of the seam, but
`Transition` is not a `Scene`: the contract clause this ADR adds lands on `Scene::advance` and says
nothing about it. Its guard **holds** the dissolve rather than stepping it, which is a different
policy and not a copy of this one, and `a_degenerate_dt_holds_progress` (`transition.rs:1203`)
asserts exactly that, documented as a stall the frontend can inject. Deleting it would trade a
stated behaviour for an unstated one under a phase whose own done-when is that the golden suite does
not move. It keeps its guard.

Whether *hold* or *nominal step* is the right answer for a stalled frame is a real question this
decision does not settle, and it is now filed rather than folded in.

## Outcome — 2026-09-08, at Plan 0140's close

The Decision is implemented and holds: one guard in `draw_frame`, six in-scene copies deleted, the
guarantee stated on `Scene::advance`, and a six-probe test asserting that a `NaN`, a negative, a zero
and an infinity fed one frame in leave the picture byte-identical three clean frames later. The full
suite is green and no golden moved. **Two things this ADR asserts about the tree are false, and both
concern the seam's reach rather than its correctness.**

**The seam sits one call too deep to be "the one place a frame delta is checked."** `render()` does
`self.time += dt` (`core/src/render/mod.rs:971`) **before** it calls `draw_frame`. `self.time` is a
bare `f32` with no reset on the live path, and it reaches every scene through `set_time`. The C ABI
passes `dt_seconds` through unvalidated, so one `NaN` from a host poisons the shared scene clock for
the life of the process — the identical one-way trap this ADR was written to close, on the largest
accumulator in the engine, immediately upstream of the guard that closes it. Filed as
design-backlog 0189.

**All three sites the seam comment names as unguarded still carry a guard, on three different
policies.** The comment reads *"Everything below reads this value and none of it re-checks: every
`Scene::advance`, the composite's per-second decay, the transition's own step."* Every clause is
false. `trails.rs:557` **keeps the previous frame's value**; `transition.rs:329` **holds** progress,
by Plan 0140's explicit design, which is what design-backlog 0188 exists to record; and
`milk/mod.rs:626`, reached from `warp_mesh`'s `update` under a `Scene::advance`, **freezes** the
`*_att` envelope at `alpha = 0`. The seam's own `FALLBACK_DT` — a nominal step — is a fourth answer.
The Negative section above calls that comment the only thing standing where six visible guards used
to be; it currently says "there is nothing else" about a tree that has three, which is the belief
that lets a fourth copy land unnoticed. The `milk/` site is the one no document in this class names
at all. Filed as design-backlog 0190.

Neither changes the Decision. Both say the population this ADR reasoned over was drawn from a grep
for one spelling inside `scenes/` — the same failure mode ADR-0135 was written against, and one this
ADR's own Correction had already caught once before implementation started.
