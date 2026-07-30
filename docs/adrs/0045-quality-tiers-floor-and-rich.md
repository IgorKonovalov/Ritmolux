# ADR-0045 — Quality tiers: a `Rich` tier beside the iGPU `Floor`, auto-selected with a manual pin

> **Status:** accepted 2026-07-30 (implemented and reviewed at Plan 0044's close)
> **Date:** 2026-07-30
> **Related plan(s):** [0044-quality-tiers](../plans/done/0044-quality-tiers.md) — **done** (R0 of [docs/roadmap-visual-richness.md](../roadmap-visual-richness.md))

## Context

NFR §1 has always specified two quality levels: a reduced tier holding 60 fps at 1080p on the
~2015-iGPU baseline, and a richer presentation on capable hardware. Only the first was ever
built. The tier system and frame-time governor were declared "their own follow-up plan" and
then deferred by Plans 0001, 0003, 0009, 0011 and 0012 — so today there is one fixed quality,
tuned to the weakest supported machine, on every machine. The constants say so themselves:
`PARTICLES = 10_000` (swarm.rs:28) and `PARTICLE_COUNT = 50_000` (particles/mod.rs:66) are
"the number to validate against the 60 fps @ 1080p floor"; `MAX_SEGMENTS = 20_000`
(lines/mod.rs:51) is "tuned to the iGPU floor"; `POST_MAX 1920x1080` (post.rs:88) is NFR §12
memory arithmetic; every optional pass is lazily built so "the NFR §1 iGPU floor pays nothing".

The 2026-07-30 visual-richness review names this the root cause of the capability gap, and the
user's mandate is explicit: richer visuals "even at the cost of performance if needed". The
user's own machine is a midrange discrete GPU (RTX 3060 / RX 6600 class). Meanwhile the iGPU
floor remains a real commitment (NFR §2), and the golden/capture suite runs on the WARP
software adapter, where cost directly gates test wall-clock.

## Decision

We will introduce two named tiers, **`Floor`** and **`Rich`**, resolved once at renderer
construction and carried as a `TierConfig` — a struct of the values that are hardcoded
constants today (particle counts, segment budget, post-stage resolution cap, trail-grid cap,
bloom blur levels once ADR-0046 lands). `Floor`'s values are exactly today's constants.
`Rich` is calibrated against the midrange-discrete target and validated on that hardware by a
`human` phase, not asserted.

Selection: the default is `Rich`, demoted to `Floor` by a frame-time governor — a sustained
miss of the display budget flips the tier **once per session** (a one-way latch, the same
shape as the dual-live freeze at render/mod.rs:96), visibly reported in the diagnostics
overlay. An explicit pin (`--tier floor|rich` / config / `LMV_TIER`) overrides the governor in
both directions. The pin travels through the renderer's construction options; the C ABI stays
v4 — the plugin gets auto behavior, and a plugin-side tier picker is a future ABI question,
not part of this decision.

Captures and goldens pin `Floor`: headless capture entry points force the floor tier so every
baseline stays byte-reproducible on WARP and the suite's cost does not scale with the rich
tier. `Rich` is verified by capture-level spot checks plus the on-device checklist.

## Consequences

### Positive
- "At the cost of performance" becomes an engineering parameter: every later roadmap item
  (HDR chain, bloom levels, layer count, particle counts) hangs its budget on a tier value
  instead of arguing with the iGPU floor.
- The floor commitment is preserved unchanged — floor values are today's values, and the
  governor means a mispredicted rich budget degrades to a known-good state instead of
  stuttering.
- One-way demotion is predictable and testable; no oscillation design.

### Negative
- **The same preset now looks different on different machines.** This deliberately ends the
  single-uniform-output era. Authoring guidance must say which tier a preset was tuned on.
- **The QA surface doubles.** Goldens pin floor, so rich-tier regressions are only caught by
  spot checks and on-device runs — a real hole we accept and name, not a solved problem.
- Every capped constant becomes two values with two justifications; drift between them is a
  new class of doc rot.
- A one-way latch means a transient stall (window drag, driver hiccup) can demote a capable
  machine for the whole session; the pin is the escape hatch, and the overlay must make the
  demotion visible rather than silent.

### Neutral
- `TierConfig` centralizes constants that today live in six files, which is a readability
  gain independent of tiering.

## Alternatives considered

### Alternative A — manual selection only
A config default with no governor. Simplest, fully predictable — but an iGPU user's first run
is a slideshow until they find the setting, which fails NFR §1's spirit (the floor experience
must be good *by default*). Rejected for first-run behavior.

### Alternative B — one rich tier plus continuous feature-shedding
No named tiers; under budget pressure the engine sheds features stepwise (drop bloom, halve
particles, lower caps) until the frame fits. Most fluid, but the output becomes a function of
load history — untestable against baselines, unreproducible in bug reports, and impossible to
document for the content lane ("your preset may or may not have bloom"). Rejected for
predictability.

### Alternative C — keep one tier and raise the constants
Already rejected by ADR-0030's outcome ("pays the full fill bill of the largest supported
display on every machine including the iGPU floor"). Nothing has changed on the floor side.

## Notes

The governor's inputs already exist: Plan 0011's diagnostics harness measures smoothed frame
time. The demotion decision should be a pure function of the smoothed series so it is unit-
testable with injected values, per the determinism rule.
