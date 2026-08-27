# ADR-0132 — A rate parameter integrates a phase; multiplying scene time makes an audio-bound rate a teleport

> **Status:** proposed
> **Date:** 2026-08-27
> **Related plan(s):** [0121](../plans/0121-a-rate-an-ink-edge-and-a-motion-reading.md)
> **Related:** [0013](0013-c-abi-v4-render-dt.md) (the injected `dt` this rests on), [0019](0019-eased-parameters.md) (the precedent for render-layer state driven by that `dt`)

## Context

`fragment_field` animates at three rates and exposes none of them
(`core/src/render/scenes/fragment_field.rs:137-142`): two fold rates, `t * 0.7` and `t * 0.6`, and
a field sweep, `t * 0.5`. A preset can only reach the fold rates through `warp`, which scales them,
so "slow this world down" and "flatten this world out" are the same knob. The content lane hit that
wall on the mono cohort and bought the field rate back with an accident — `pan` lands in the same
sum the field sine reads, so a symmetric `pan = -0.25 * time` cancels the clock — at the cost of the
pan binding (design-backlog 0137).

Adding a rate parameter therefore has to happen. The decision is not *whether* but *what a rate
parameter means when the preset binds it to audio*, and this engine already has one answer on the
books that does not survive the question.

`warp_mesh` computes `let wt = time * wspeed` (`warp_mesh/mod.rs:486`), where `time` is the shared
scene clock and `wspeed` is the bindable `warp_speed`. At `t = 100 s`, a `warp_speed` that swings
from `1.0` to `1.5` on a bass hit moves the warp's phase by **fifty seconds** in one frame. The
picture does not speed up; it jumps. Nothing has caught this because no shipped preset binds
`warp_speed` — the parameter defaults to `1.0` and the library has zero `warp_mesh` worlds. The
first preset to bind it finds the bug, and the first preset to bind a new `field_speed` written the
same way finds it again.

The engine already holds the machinery for the alternative. `Scene::advance(&mut self, dt)` is
called every frame with the injected real elapsed time (Plan 0014), and the per-frame order in
`core/src/render/mod.rs:602-701` is `set_time` → `advance` → `reset_params` → `set_param` →
`update`. So a scene can hold `dt` and integrate a phase against *this* frame's parameter values
without any widening of the `Scene` trait — ADR-0002's seam is untouched.

## Decision

We will make every bindable rate parameter in this engine **integrate a phase** rather than scale
absolute scene time: the scene keeps a phase accumulator, adds `rate * dt` to it once per frame, and
the shader reads the phase. A rate is a per-second quantity multiplied by `dt`, so two half-length
frames compose to one full-length one — the same composition `warp_mesh`'s per-vertex transforms
already honour (`let rot = in.t0.y * dt`, four lines above the clock that does not) — and a rate
that changes mid-run bends the motion instead of translating it.

This binds two sites: the new `field_speed` and `fold_speed` on `fragment_field`, and the existing
`warp_speed` on `warp_mesh`, which is corrected to match rather than left as a counterexample.

**At a constant rate the integrated phase equals `rate * t` by construction**, so the default
(`1.0`) reproduces today's picture and nothing a preset does not opt into changes. The one caveat is
arithmetic, not semantics: a sum of `rate * dt` in `f32` differs from `rate * (N * dt)` in the last
place or two, so a golden may move by a rounding-scale amount. That is far below `golden.rs`'s own
`0.02` mean-channel drift floor, and the plan asserts the property rather than assuming it.

## Consequences

### Positive

- **A rate becomes bindable without a trap.** `field_speed = "0.2 + clamp(bass * 0.4, 0, 0.3)"` is
  a world that quickens under bass and returns, which is the reactivity the content lane keeps
  asking for and could not previously express in the time domain at all.
- **The phase is continuous by construction, and that is testable on the CPU** — no rendering, no
  adapter. Step the rate parameter between two frames and the accumulator advances by exactly
  `rate * dt`, whatever the elapsed scene time.
- **`warp_speed` stops being a live counterexample.** The rule holds everywhere in the engine
  rather than in the one scene that happened to be written after it.
- **`Scene` does not widen.** `advance(dt)` and `update(frame)` already exist and already run in the
  order this needs.

### Negative

- **A scene that was stateless becomes stateful.** `fragment_field` currently derives everything
  from `set_time`; after this it carries two `f32` accumulators. Reproducibility now depends on the
  scene being reset with the preset, not only on the clock — a real property, and one the preset
  switch path has to honour.
- **Precision drifts over a long run.** An accumulator summing `dt` for hours accumulates rounding
  that `rate * t` does not. At `f32` and 60 Hz the phase is in the thousands of radians after an
  hour, where the ulp is coarse enough to be visible as a slight rate error — not as a jump. The
  `--horizon` mode is the instrument if this ever matters; nothing suggests it does yet.
- **Two more names on the library's second-busiest scene.** `fragment_field` already carries about
  twenty parameters, and this adds `field_speed` and `fold_speed` to `presets/README.md`'s roster.
- **The correction to `warp_speed` is a behaviour change nothing can regress-test**, because no
  preset binds it. The plan asserts the constant-rate equivalence instead, which is the only claim
  that has evidence behind it.

### Neutral

- The two fold rates stay welded to each other in their designed 0.7 : 0.6 ratio under one
  `fold_speed`. They are a sine/cosine quadrature pair; nothing has asked for them to diverge, and
  splitting them later is additive.

## Alternatives considered

### Alternative A — one `field_speed` scaling all three literals proportionally

The smallest possible surface, and it preserves the scene's character exactly. Rejected because it
keeps welded the *one* pair the raising note was actually about: the content lane's complaint is
that "slow it down" and "flatten it out" are one knob, and a single proportional rate leaves the
fold crawl and the field sweep locked to each other just as `warp` does today. It would have shipped
a parameter that does not answer the question that produced it.

### Alternative B — three parameters, one per shader literal

Maximum authorability. Rejected because the two fold rates are a designed quadrature pair — `sin` on
one axis, `cos` on the other, at rates chosen to keep the fold from beating against itself — and
nothing in the raising note or the cohort asked for them to move independently. Three names on a
twenty-parameter scene to serve a use nobody has is the kind of surface growth ADR-0002's thinness
argument exists to resist. Splitting `fold_speed` in two later costs nothing.

### Alternative C — an engine-wide `time_scale` on the shared scene clock

One name, reaching all eleven systems at once. Rejected on two counts. It conflates with the rate
parameters that already exist per scene (`warp_speed`, `spawn_rate`, the emitter's launch rates), so
a preset setting both gets a product with no stated precedence; and scaling the shared clock changes
what `time` means inside every preset *expression* as well, which silently re-times every binding
written against it. That is a much larger decision than a rate on one scene, and it would have to
supersede the shared-clock guarantee in `Scene::set_time` rather than supplement it.

### Alternative D — keep multiplying absolute time, and document the jump

Zero code beyond the parameters themselves, and it matches what `warp_mesh` does today. Rejected
because the failure is not a documentation gap — it is that the natural thing to write
(`field_speed = "0.2 + bass * 0.5"`) produces a violent, unexplainable jump the first time the
binding moves, on a lane whose whole method is binding parameters to audio. A surface whose
documented advice is "do not bind this to audio" is a surface that should not be bindable.

## Notes

The per-frame order this relies on is `set_time` → `advance` → `reset_params` → `set_param` →
`update` (`core/src/render/mod.rs:602-701`). `advance` therefore runs **before** this frame's
parameter values land, so a scene that integrates inside `advance` would use the previous frame's
rate. The integration belongs in `update`, with `advance` storing `dt` — the same split
`warp_mesh` already uses for its own `dt`.
