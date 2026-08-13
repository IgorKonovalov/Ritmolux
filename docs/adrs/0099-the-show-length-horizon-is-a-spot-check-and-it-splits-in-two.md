# ADR-0099 — the show-length horizon is a spot-check, and it splits in two

> **Status:** accepted (2026-08-13, user approval)
> **Date:** 2026-08-13
> **Related plan(s):** [0085 — the show-length horizon gets an instrument](../plans/0085-the-show-length-horizon-gets-an-instrument.md)
> **Supplements:** [ADR-0010](0010-accept-gpu-driver-memory-floor.md),
> [ADR-0045](0045-quality-tiers-floor-and-rich.md)

## Context

Every instrument this project owns lives in the first seconds of a preset's life. The four
synthesized behavioral gates capture 30 frames at 1/60 s — **half a second**. The PCM-driven
reactivity gate measures a few seconds of hops. `shot` strips are shorter still. The live-show use
case runs for hours.

Three separate observations have now landed in that blind spot, from three different lanes:

- **A world collapsed live, three times, while the suite stayed green.** Plan 0075 cohort 4's
  Shatter piled onto its flow field's attractors over **minutes** of sustained force and stayed
  there. Nothing went red; nothing was flagged
  ([backlog 0086](../design-backlog.md)).
- **Resident set grew 385 → 663 MB over three minutes** of preset switching at Rich tier, against
  ADR-0010's ~327 MB driver-dominated floor — **with no no-feedback control beside it**, so nothing
  separates "expected cost of two new accumulation buffers" from "growth that does not stop"
  ([backlog 0083](../design-backlog.md)).
- **`frame_ms_p99` hit 25.0 ms while `frame_ms_avg` never passed 8.7 ms and zero frames dropped**
  out of 28,698. The spikes coincide with preset switches and a fullscreen toggle — GPU resource
  rebuilds, not steady-state cost. The unbuilt adaptive-quality governor (roadmap item 3 / R0) is
  **specified to read p99**, so it would demote a preset running at 165 fps, on the one event that
  is already visually disruptive ([backlog 0082](../design-backlog.md)).

The tempting response is to extend the gates. It is the wrong one, and the reason is arithmetic
already on the record: [backlog 0080](../design-backlog.md) measured what *seconds* cost —
86 s → 167 s over 41 presets when the reactivity gate went from synthesized frames to real PCM. A
minutes-long capture per preset over a 37-preset library is not a price this suite can pay, and a
sampled subset would be a gate whose coverage nobody could state.

There is a second reason, and it is the one that shapes the decision. **The three observations do
not share a mechanism.** Shatter's collapse is a property of the *simulation* — forces integrated
against injected `dt`, deterministic since [ADR-0019](0019-eased-parameters.md) and Plan 0014, and
therefore reproducible headless at capture cadence. RSS growth and p99 spikes are properties of the
*process* — GPU resource churn, driver allocation, surface reconfigure — which no headless render
loop reproduces, because the thing that causes them is the live app switching presets and
rebuilding resources. Building one instrument for both would produce something that measures
neither well.

## Decision

The show-length horizon is a **documented spot-check, never a gate**, and it is **two instruments,
not one**:

- **A headless simulation horizon.** `shot` grows a long-run mode that renders N simulated minutes
  at capture cadence and reports image-domain drift statistics at intervals — reusing
  `metrics`'s existing coverage / footprint / peak-to-mean instruments rather than inventing new
  ones. It is deterministic, machine-independent in what it *reports about the simulation*, and run
  by the content lane on worlds whose mechanism has an accumulation axis, with the verdict recorded
  in the preset's own header the way the fold-edge verdicts were.
- **A live resource horizon.** The existing `--soak` log learns a **preset-switch marker** and a
  steady-state frame-time statistic beside `p99`. That one addition serves both process-side
  entries: it separates per-switch cost from monotone growth for the RSS question, and it gives the
  future governor a column that does not spike on a rebuild.

Neither is wired to CI, neither fails a build, and neither is run per preset by default.

## Consequences

### Positive

- **A real failure class stops being invisible.** Any slow-divergence look — accumulating forces,
  feedback with net gain, populations that migrate — can be checked before it ships, by the lane
  that authors it, at a cost that is one sitting rather than a CI budget.
- **The governor gets its qualification before it is designed, which is the cheapest moment.**
  [backlog 0082](../design-backlog.md)'s whole point is that the instrument R0 will read needs the
  caveat now, not after a demotion fires during a set.
- **The RSS number gets a control.** A figure with no control beside it gets quoted later as either
  a clean bill of health or a known leak depending on who is quoting it, and supports neither. The
  paired run settles which.
- **The headless half is deterministic, so a drift verdict is reproducible.** Injected `dt` and
  seeded randomness mean the same world drifts the same way on every machine — which is what makes
  a recorded header verdict worth anything.
- **It reuses instruments whose meaning is established.** Coverage, `footprint_diff` and peak-to-mean
  already have derivations and known blind spots; a fresh statistic would need its own ADR-0071
  story before anyone could read it.

### Negative

- **A spot-check is only run when someone remembers.** That is the honest cost of refusing a gate,
  and it is why [backlog 0086](../design-backlog.md) parked with an explicit *trigger* rather than a
  schedule. The mitigation is a documented trigger in the content lane's own materials, not
  automation — and this project has evidence that a duty with no stated trigger gets skipped (the
  version bump sat still across five plans).
- **The headless horizon cannot see the process-side failures, and the live horizon cannot see the
  simulation ones.** Two instruments means two things to run and a real chance someone runs the
  wrong one for the question they have. Each has to say what it cannot answer, in place.
- **The headless mode is slow by construction.** N simulated minutes at 1/60 s is 3,600·N renders;
  ten minutes is 36,000. That is minutes of wall clock per world, which is fine for a spot-check and
  is exactly why it is not a gate — but it also means nobody will run it casually.
- **Image-domain statistics are a proxy for the thing that actually drifts.** Shatter's failure is a
  population piling onto attractors; what a capture sees is coverage falling and peak-to-mean
  rising. That correlation is strong and it is not identity — a world could drift in a way the
  picture hides. Reading a *trend* rather than a threshold is what keeps the proxy honest.

### Neutral

- `--soak`'s existing columns and cadence are unchanged; the switch marker is appended, matching the
  same convention `diagnostics.log` follows for its own frozen prefix.

## Alternatives considered

### Alternative A — make it a gate

Extend the behavioral suite with a minutes-long capture per preset, so a slow-divergence world
fails the build. **Rejected on measured cost.** The suite's move from synthesized frames to real PCM
on *one* gate cost 1.8x over 41 presets; a minutes-long capture over the library is orders beyond
that, on a Windows CI critical path [ADR-0073](0073-the-windows-ci-critical-path.md) already
budgets carefully. A sampled subset would be a gate whose coverage is unstateable, which is worse
than an honest spot-check.

### Alternative B — one instrument for both horizons

Build a single long-run harness and use it for simulation drift and resource growth alike.
**Rejected because the two failures have different causes and only one of them is reproducible
headless.** RSS growth and p99 spikes come from GPU resource churn in the live app; a headless
render loop that never rebuilds a surface cannot produce them. A single harness would either be the
live app (and lose determinism, so a drift verdict means nothing) or be headless (and be blind to
two of the three entries).

### Alternative C — read the existing `--soak` log harder

`--soak` already samples elapsed / fps / RSS / frames / heartbeat every five seconds, so arguably
the RSS question needs only a longer run and more attention. **Rejected as insufficient rather than
wrong**: with no switch marker the log cannot separate per-switch cost from monotone growth, which
is the *entire* question [backlog 0083](../design-backlog.md) asks. One appended column is what
turns the existing instrument into one that can answer it — which is why this decision extends
`--soak` rather than replacing it.

## Notes

**The trigger, stated once so it can be found.** Run the headless horizon on a world whose mechanism
has an accumulation axis: sustained forces, feedback with net gain, a population that can migrate or
pile up. Record the verdict in the world's header. [Plan 0077](../plans/done/0077-the-quiet-sky.md)'s
standing Phase 5 already carries a bounded instance of exactly this rider, and it predates the
instrument — so the first world to use it is already named.

**What this does not settle.** Whether the governor should exclude post-switch frames, require N
consecutive bad windows, or read a separate steady-state statistic is R0's decision, not this one.
This ADR ensures the measurement and the qualification are on the record before that decision is
taken; [backlog 0082](../design-backlog.md) names the three candidates.
