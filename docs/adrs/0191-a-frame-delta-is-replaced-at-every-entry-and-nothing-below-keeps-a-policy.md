# ADR-0191 — A frame delta is replaced at every entry that takes one, and nothing below keeps a policy of its own

> **Status:** proposed
> **Date:** 2026-09-11
> **Related plan(s):** [0171](../plans/0171-one-stall-policy-and-a-guarded-clock.md)
> **Amends:** [ADR-0152](0152-the-frame-delta-is-sanitized-at-the-scene-seam.md) — moves its check
> above the scene clock, and retires the three policies it left standing below it.

## Context

[ADR-0152](0152-the-frame-delta-is-sanitized-at-the-scene-seam.md) put the frame-delta check in
`draw_frame`, before `Scene::advance`, and substituted `FALLBACK_DT` for a non-finite or non-positive
delta. Three facts found since show it sits one level too low and was not the only answer in the
engine.

**The scene clock is advanced above the check.** `self.time += dt` runs before `draw_frame` in all
three `Renderer` entries that take a caller's delta: `render` (`core/src/render/mod.rs`), and
`render_tapped` and `capture_stream` (`core/src/render/capture_api.rs`). `self.time` is a bare `f32`
with no other write on the live path, handed to every scene each frame through `Scene::set_time`, so
one `NaN` from a host poisons it for the life of the process. The C ABI hands the plugin's
`dt_seconds` straight to `render`. That is backlog 0189.

**Three sites below the check keep guards, on three different policies.** The composite's
`set_dt` (`core/src/render/trails.rs`) keeps the previous frame's value; the transition's `advance`
(`core/src/render/transition.rs`) holds progress; the MilkDrop runtime (`core/src/milk/mod.rs`)
freezes the `*_att` envelopes at `alpha = 0`. The check itself substitutes a nominal step, which is a
fourth answer — while its comment says nothing below re-checks. That is backlog 0190.

**One of the three is documented as deliberate and is now unreachable.** The transition's hold is
pinned by `a_degenerate_dt_holds_progress`, whose doc says *"the frontend can inject either after a
stall"*. Since ADR-0152 nothing on the frame path can reach it, so the test pins a branch the engine
no longer takes. That is backlog 0188, which names the choice as policy: stepping keeps a dissolve on
wall-clock time and may jump on resume; holding keeps it smooth and lets a long stall stretch it.

## Decision

We will replace a non-finite or non-positive frame delta with `FALLBACK_DT` in **one function**,
called first by every `Renderer` entry that accepts a caller's delta — today `render`,
`render_tapped` and `capture_stream` — before the scene clock, the now-playing banner or
`draw_frame` sees it. `draw_frame` receives a sanitized delta and no longer checks; its doc states
that precondition. The **nominal step is the engine's only answer** to a degenerate frame: the guards
in the composite's `set_dt`, the transition's `advance` and the MilkDrop runtime's envelope step are
deleted, and the transition's hold goes with them. A hygiene test holds that population at one: a
finiteness guard on a frame delta anywhere else in `core/src/` fails the suite.

Entries that step the clock by `FALLBACK_DT` themselves — the capture paths — take no caller delta
and need no call.

## Consequences

### Positive
- The largest accumulator in the engine is covered, on every live path including the windowless
  one, which a check hoisted into `render` alone would have missed.
- One answer to one question. A reader who greps for the guard finds exactly one, and the comment
  that ADR-0152's Negative section called *"the only thing standing there"* becomes true and gated
  rather than trusted.
- Every entry added later has one function to call, not one policy to choose.

### Negative
- **A long stall no longer stretches a dissolve.** The crossfade stays on wall-clock time and advances
  one nominal step on a degenerate frame, so it may visibly jump on resume. Nothing in this repository
  measures either look, and no capture path produces a bad delta; the choice is made for consistency,
  not from an observed picture.
- **The hygiene test catches a second guard, not a missing call.** A new entry that advances the clock
  without calling the function is visible only in review. The function's doc and the `Renderer` doc
  are what stand there.
- **The MilkDrop runtime's guard may be policy for a caller outside the frame path** — a conversion
  test or a warm-up that passes `dt = 0` on purpose. Plan 0171 lists the callers before deleting it,
  and a caller that depends on the freeze turns this Decision's "only answer" into an Outcome to
  record.

### Neutral
- No observed scenario changes: the shells have never produced a bad delta. This closes a class, as
  ADR-0152 did.

## Alternatives considered

### Alternative A — Validate at the C ABI boundary only
The project's rule says validation belongs where input enters, and `rlx_render_dt` is that place for
the plugin. It lost because the standalone app calls `render` directly and never crosses the ABI, so
the clock would stay exposed on the path most users run.

### Alternative B — Both the ABI and the entries
Defence in depth, and two copies of one policy — the duplication ADR-0152 spent six deletions
removing. The entries alone cover the ABI's caller.

### Alternative C — The transition keeps its hold
Defensible on look. It lost because the hold needs a path that can reach it: the entry's substitution
would have to pass a "this frame was degenerate" marker down through `draw_frame`, widening what the
frame carries for a behaviour nobody has observed.

### Alternative D — Keep all four and correct the comment
The cheapest repair, and the one backlog 0190 names as a possible doc fix. It lost because four
answers to one question survive, and the next guard is added or removed by a reader who cannot tell
which of them is deliberate.
