# ADR-0137 — A latch is render-layer state, and its name resolves to a slot at load

> **Status:** proposed
> **Date:** 2026-08-27
> **Related plan(s):** [0123](../plans/0123-a-gate-a-latch-and-an-ink.md)
> **Supplements:** [0019](0019-eased-parameters.md), [0050](0050-downbeat-and-phrase-tracking-with-confidence-fallback.md)

## Context

The expression evaluator is pure by hard invariant (`CLAUDE.md`, and `core/src/preset/expr.rs` says
so in its own comments), and `[smoothing]` eases a value without ever holding one. So the grammar
cannot say *"fire once per window, on the music."* `min(mod(time, 100) > 60, onset > 0.6)` is an AND
over two instantaneous readings, and an edge-triggered binding re-fires on every onset inside the
window rather than on the first.

The want has now arrived twice from unrelated directions. Archived backlog 0034 asked for per-object
launch state on an emitter and named the grammar-side cousin of it explicitly. Backlog 0147 asked
for `collage_mono` to recompose on the first strong onset after ninety seconds — rare, and landing
on a musical moment. What shipped instead is `recompose = "mod(time + 50, 100) < 50"`: exactly one
rise per hundred seconds, deliberately metronomic, and disconnected from the music by construction.
The preset header argues that choice honestly — a `hash(beat_index) > 0.9` gate fires every three to
ten seconds, because `beat_index` counts onset detections rather than musical beats — which is why
the wall clock won, and does not make the wall clock what the look wanted.

The invariant that forbids the feature is load-bearing and is not up for casual revision:
determinism (NFR section 6) is what makes the golden suite, `--report` and every DSP assertion mean
anything.

## Decision

We will add a **`[latch]` table** whose state lives in the render layer beside `ParamSmoother`, and
whose author-chosen name is resolved **at parse time** to one of a small fixed set of reserved
variable slots. The evaluator stays a pure function of its `Variables` and its element index; it
never holds a value between frames and does not learn that latches exist.

```toml
[latch]
# armed while `arm` holds; on the first rising edge of `fire` while armed it
# outputs 1.0 for `hold` seconds, then disarms until `arm` falls and rises again
recut = { arm = "mod(time, 100) > 90", fire = "onset > 0.6", hold = 0.5 }

[params]
recompose = "recut"
```

This is the same shape `[smoothing]` already has and it is chosen for the same reason. Easing is
per-frame state too; ADR-0019 put it in the render layer rather than in the grammar, `Binding::tau`
is resolved out of the table **once, at load**, and `Preset` deliberately does not keep the
`[smoothing]` table because there is nothing left for a frame to look up. A latch follows that
precedent exactly: the table is validated and folded at load, the state is held per preset instance,
it is advanced by the injected real `dt` (so `hold` is frame-rate independent, per ADR-0014's rule),
and it is reset on a preset switch alongside the smoothers.

**The name is author-chosen and the storage is fixed.** `VAR_NAMES` is a compile-time array with
hardcoded slot bases and assertions that the names really do live where the constants say. Latch
slots are appended as a fixed reserved block **before** `index`, whose slot is derived as
`VAR_COUNT - 1` and must stay last; the name-to-slot binding happens at parse time exactly the way
`tau` does. An author writes `recut`, the loader writes slot *k*, and the per-frame path is an array
index.

**Determinism is preserved and its meaning narrows.** A latch's output is still a pure function of
the frame sequence and the `dt` sequence that produced it: same input, same output, run to run and
machine to machine. What changes is that a capture at frame N now depends on frames 0..N through the
expression layer as well as through the scene. Scenes are already stateful in exactly this way and
`capture_preset` already renders a prefix to reach frame N, so no harness changes — but it is stated
rather than discovered, because "the expression layer is stateless" was true until this ADR and is
cited in comments.

## Consequences

### Positive

- **A gate can be armed on one thing and fired by another.** That is the whole want, and it has now
  been felt on two unrelated axes.
- **The purity invariant survives intact**, and survives in the form that matters: the evaluator is
  still re-entrant, still safe to call N times per frame for a per-vertex or per-element binding, and
  still testable as a pure function.
- **No per-frame lookup cost.** Names resolve at load, latches evaluate once per frame before the
  params that read them, and a preset with no `[latch]` table pays nothing.
- **It composes with everything already there.** A latch output is an ordinary variable, so it can be
  smoothed by `[smoothing]`, multiplied, gated, or read by several bindings at once.

### Negative

- **The reserved block is a cap, and a cap is a wall.** A preset wanting more latches than the block
  holds gets a load error rather than a slower path. The number is a chosen constant with a stated
  reason, not a measurement, and raising it costs a recompile.
- **`VAR_NAMES` grows and its slot arithmetic moves.** `INDEX_SLOT`, `RAW_SLOT_BASE` and
  `CLOCK_SLOT_BASE` are hardcoded, and this project has already shipped a defect of exactly this
  shape — a field inserted before a positional one silently re-pointed every attribute after it. The
  existing name assertions are what stand between this change and that bug, and they only cover the
  blocks they already know about.
- **"The expression layer is stateless" stops being true**, and it is written in comments and read by
  the content lane. Every place that says it has to be corrected in the same change, or the next
  reader is misled by a sentence that used to be right.
- **A latch is invisible to a single-frame probe.** `--report`'s reachability walk drives expressions
  with a frame sequence, so it sees latches; anything that evaluates one frame in isolation reads a
  latch as its rest value and cannot tell an unfirable latch from a quiet one.

### Neutral

- No C ABI change, no new scene, no GPU work.
- `[smoothing]` is untouched; a latch output may be listed there like any other value.

## Alternatives considered

### Alternative A — a `latch(arm, fire)` function in the grammar

The reading-order winner: state at the call site, composes with any expression, no table. Rejected on
a mechanism rather than on taste. **One compiled expression is evaluated many times per frame in this
engine** — a `[per_vertex]` binding runs once per mesh vertex, and a binding naming `index` runs once
per element — so per-call-site state inside the compiled tree would need one state *per evaluation*,
not one per expression. There is no correct single answer for what `latch()` returns on the 400th
vertex of one frame, and the shapes that would give one (state keyed by vertex, or a latch that means
something different in a per-vertex binding than in an ordinary one) are both worse than a table. It
also makes `Expr` non-re-entrant, which is the property the per-vertex and spectrum paths rest on.

### Alternative B — reserved slots with fixed names (`latch0`..`latchN`)

Keeps the fixed array and skips name resolution entirely. Rejected because the readability is the
point of the feature and the cost it avoids is already paid: resolving a name to a slot at load is
exactly what `Binding::tau` does with `[smoothing]`, and it costs one map lookup per preset load.
`recompose = "recut"` says what the preset does; `recompose = "latch2"` requires reading a table
somewhere else to find out.

### Alternative C — one narrow parameter on the bindings that are already edge-triggered

An arming window bolted onto `recompose`, `reseed` and their siblings — `recompose_arm` beside
`recompose` — with no new grammar, no new table and no state in the evaluator. Genuinely the
cheapest thing that solves the motivating case. Rejected because the demand has now appeared on two
axes that share no binding, so the narrow version would be built twice and would still not cover the
third instance; and because the per-binding form multiplies the parameter surface of every scene that
has an edge-triggered input, which is the surface the content lane already has the most trouble
holding in its head.
