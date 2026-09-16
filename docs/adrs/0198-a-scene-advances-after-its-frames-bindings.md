# ADR-0198 — A scene advances after its frame's bindings, and a shared scene is never evaluated twice in a frame

> **Status:** accepted 2026-09-15 (Plan 0181), with an Outcome
> **Date:** 2026-09-14
> **Related plan(s):** [0181](../plans/done/0181-a-scene-advances-after-its-frames-bindings.md)
> **Extends:** [0135](0135-every-scene-rate-integrates-through-one-shared-phase.md) (every scene rate
> integrates through one shared `Phase`), [0024](0024-cross-preset-transitions.md) (cross-preset
> transitions and the dual-live governor)

## Context

`evaluate_preset` and `evaluate_layer` in `core/src/render/evaluate.rs` call `Scene::set_time` and
`Scene::advance(dt)` **before** `reset_params` and the binding walk. So `advance` sees the values the
**previous** frame's bindings left in the scene. ADR-0135 turned that order into a rule for scene
authors: a rate integrates in `update`, never in `advance`. Every other scene follows it. `emitter`
integrates its sprite `spin` in `advance`, and `shape_collage` rebuilds its canvas and steps its
drift, spin, density fades and recomposition edge there. Both use one stale frame of parameters. On
the frame a preset switch lands, those parameters belong to whichever preset last drove that system
(backlog 0191).

The order has been kept because moving `advance` was believed to change "which frame's parameters
drive which frame's integration for every preset" and so to move every golden. It does not. A scene
that only stores `dt` in `advance` gives the same result on either side of the binding walk, and so
does a scene that drains a parameter-free fixed-step accumulator (`particles`,
`reaction_diffusion`). The only scenes whose output depends on the order are the ones already
breaking ADR-0135's rule.

Backlog 0142 raised a second worry about the same functions: during a dissolve between two presets
of one system, both sides reach the same `Box<dyn Scene>`, so `evaluate_preset` would run twice
against it and advance its state at 2x. That needs the dissolve to run dual-live. ADR-0024's
governor refuses: `dissolve_mode` passes `scenes::shares_resources(a, b)`, which is true for
`a == b`, to `dual_live_eligible`, which returns `Freeze` for any shared pair whatever the frame
budget. Under `Freeze` the outgoing side is never evaluated. The only route to such a frame is
`Renderer::begin_transition_forced`, a `#[cfg(test)]` helper that overwrites the governor's answer.

## Decision

We will call `Scene::set_time` and `Scene::advance` **after** the binding walk, the live overrides
and the `[per_vertex]` table, immediately before `Scene::update`, in both `evaluate_preset` and
`evaluate_layer`. `advance` then sees this frame's parameters, and a scene may integrate a bound value
in either `advance` or `update` with the same result. ADR-0135's "never in `advance`" becomes
unnecessary, though still harmless. The frame delta handed to the chain (`side.chain.set_dt`) and
the two `reset_params` calls keep their positions.

We will keep ADR-0024's shared-resources veto as the **only** guarantee that one scene object is
evaluated at most once per frame, and treat it as load-bearing for per-frame scene state as well as
for GPU state. `begin_transition_forced` keeps `Freeze` for a shared pair, so no test can build a
frame the governor forbids. Nothing is added to the scene to detect a second call.

## Consequences

### Positive

- **A scene cannot integrate a stale parameter**, whichever of its two per-frame methods it
  integrates in. The trap is gone, so it needs no text guard over every `advance` body.
- **The switch frame is the incoming preset's own frame.** `shape_collage` builds the bound canvas
  on frame 1 instead of the default canvas on frame 1 and the bound one on frame 2. `emitter`'s
  first `spin` step uses the bound rate.
- **One evaluation order** for the preset, its layer, and every frame, switch or not.
- **The dual-live test hatch can no longer produce a false defect report** on a shared pair.

### Negative

- **Goldens for `emitter` and `shape_collage` move**, and the collage's recomposition edge fires
  one frame earlier than before. Renders compared across the change differ by one frame on those two
  systems.
- **A same-system dissolve still freezes its outgoing side.** This ADR makes that permanent rather
  than fixing it. Animating both sides of a same-system dissolve would need a second scene instance
  per side, allocated at dissolve start (ADR-0030's mid-run allocation concern), and that is a new
  decision.
- **The `Scene` trait's documented call order changes.** ADR-0002 keeps the trait thin, and its
  ordering is part of the contract a scene author reads. Any comment arguing from the old order goes
  stale at once. Plan 0181 Phase 3 sweeps the known ones.

## Alternatives considered

### Alternative A — A once-per-frame guard on the shared scene

A frame stamp on the scene so a second `advance`/`update` in one frame is a no-op, while both sides'
expressions still evaluate. This was the interview's answer to backlog 0142. **Rejected because it
guards a path no shipped build reaches**, as the veto above shows. **It also would not make that
path correct**: both sides of a dual-live frame encode into one submission, so one scene object's
uniforms written twice are read twice as the second write. The guard would stop the state doubling
and leave the picture wrong, which is exactly why ADR-0024 put the veto on the pair.

### Alternative B — Move the integration into `update` in the two offending scenes

**Rejected because it keeps the trap.** It is pixel-equivalent to this decision, but it repairs two
instances of a rule the trait doc already stated and two scenes broke. Holding it would need a
hygiene scan that reads every `advance` body and decides which statements touch a settable field. A
text rule like that approximates what the reorder guarantees by construction.

### Alternative C — Apply the bindings before `advance` only on the frame a switch lands

**Rejected because it is two orders chosen by a frame flag**: two answers to one question, the
pattern ADR-0191 retired for the frame delta. It also leaves the one-frame lag in the steady state,
where `shape_collage`'s recomposition edge still fires a frame late.

## Outcome (2026-09-15, at Plan 0181's close)

The Decision landed as written. `evaluate_preset` and `evaluate_layer` in
`core/src/render/evaluate.rs` call `set_time` and `advance` directly above `update`, after the
bindings, the overrides and the per-vertex table, and `Scene::advance`'s doc states that order.
`begin_transition_forced` keeps `Freeze` for a pair `scenes::shares_resources` reports as shared, and
the governor test asserts `Mode::Freeze` for a same-system pair of every `SystemKind`. Two tests pin
the switch frame: a fresh `emitter` integrates exactly its bound `spin` times `dt`, and a fresh
`shape_collage` generates its bound canvas once, on frame 1.

**One Negative consequence did not happen.** No golden moved: the full workspace suite passed with no
bless, the `emitter` and `shape_collage` baselines included. A one-frame shift in a rate, and a
recomposition one frame earlier, stayed inside each baseline's tolerance on a 120-frame capture.
Renders compared across the change still differ, as the Negative says; the goldens cannot see it.

Two comments outside the plan's sweep still stated the old order at the close: the attractor's spin
comment in `particles/mod.rs` and the `cellular` test driver's doc. Both are recorded as open findings
in Plan 0181's close review.
