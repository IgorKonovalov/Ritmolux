# ADR-0140 — An attractor's sample budget is a density against the render target, capped live and uncapped offline

> **Status:** proposed
> **Date:** 2026-08-28
> **Related plan(s):** [0128](../plans/0128-the-rendered-file-stops-looking-upscaled.md)

## Context

`TierConfig::attractor_particles` is a flat constant — 50,000 at `Floor`, 150,000 at `Rich`
(`core/src/render/tier.rs:370`, `:391`) — with no term for the target it draws into. The trail grid
*is* surface-sized (Plan 0027, capped at 2560x1440 / 3840x2160), so the deposit spreads over whatever
the target holds. The arithmetic that follows is the whole problem:

| target | pixels | particles/px/frame at `Rich` |
|---|---|---|
| 640x360 | 230,400 | 0.651 |
| 1280x720 | 921,600 | 0.163 |
| 1920x1080 | 2,073,600 | 0.072 |

**Density falls as resolution rises.** Going to 1080p multiplies pixels 9x against a 640x360-class
window while the tier multiplies particles 3x against `Floor`, so the figure gets *grainier* the
larger it is drawn. The first real music video this engine produced — `attractor_leviathan` over a
4:41 track at 1920x1080/60, `--tier rich` — came back with the verdict *"it just looks like Leviathan
upscaled"* ([Plan 0101](../plans/done/0101-the-engine-renders-a-music-video.md) Phase 5), and that is
exactly what it is.

The engine already says this one step short of the consequence. `attractor_particles` is documented
as *"a sample count and not a brightness"* whose deposit is divided by the count
([ADR-0065](0065-the-attractor-deposit-is-normalized-by-particle-count.md)), so *"raising it buys a
smoother figure rather than a brighter one"* — correct, and the count never learned that the number
of pixels it is smoothing over is not fixed.

Two existing decisions shape what a fix may look like.
[ADR-0069](0069-the-attractor-trades-sample-count-for-trace-length.md) already split the number in
two: the tier's count is the **allocation bound**, and what is dispatched and drawn is
`round(budget * density)` from the preset's `[particles] density`. And every capture path in this
repo renders small — the golden suite captures at 128x128 and is `Tier::Floor` by construction — so
no test at the configuration this project develops on can see a resolution-bound sample budget. That
is [ADR-0037](0037-internal-grid-is-a-resolution-not-a-shape.md)'s habit generalized: a quantity
sourced from a constant and a quantity sourced from the target agree at exactly one size, and small
is the size we always measure at.

Offline is also where the fix is affordable. `shot --render` has no 60 Hz deadline and no governor to
answer to, and a render is a one-shot process where 60 MB of particle state costs nothing anybody
watches.

## Decision

We will make the attractor's drawn sample count a **density against the render target**, resolved as

```text
budget(tier, target) = clamp(round(tier.attractor_particles * target_px / REFERENCE_PX),
                             tier.attractor_particles,      // never fewer than today
                             ceiling)
```

with `REFERENCE_PX` the small-window resolution whose density is already accepted, a **live
`ceiling`** set by frame-time measurement on the reference hardware, and a **larger offline ceiling**
for headless render paths, where the only bound is memory. The preset's `[particles] density`
composes on top unchanged: it still narrows what is drawn out of the budget this law resolves.

The particle buffer is allocated **once, at the ceiling**, never at the law's current value — a
resize changes the active count and nothing else, so no GPU resource is rebuilt mid-run.

The lower clamp is load-bearing: the law can only ever *add* samples above the reference resolution,
never remove them below it. Every existing capture — the 128x128 golden suite, the 96x96 sanity
suite, every `shot` still — resolves to exactly today's count and stays byte-identical.

## Consequences

### Positive

- **A tier stops meaning "the same number of samples on every display" and starts meaning "the same
  picture".** That is what ADR-0045's capacity-not-behavior promise always claimed and what ADR-0065
  made half-true by normalizing the deposit.
- **The rendered file is the case that improves most**, which is the case that has to stand on its
  own: offline the ceiling is memory rather than frame time, so 1080p can reach the reference
  density outright.
- **No golden moves and nothing is blessed.** The lower clamp guarantees it on the value rather than
  by inspection of pixels — the same shape of argument ADR-0065 used for `deposit_scale` being
  exactly 1.0 at `Floor`.
- **Brightness is already invariant.** `deposit_scale` divides the per-particle deposit by the active
  count, so 9x the samples is the same total light with less shot noise, not a figure nine times
  hotter.
- **It composes with what exists.** ADR-0069 already separated allocation from active count; this
  adds a term to the number ADR-0069 resolves, and touches no shader.

### Negative

- **The `Rich` allocation grows to the ceiling and stays there**, even in a small window: at 48 B per
  particle, 150,000 costs 7.2 MB and a 3x ceiling costs 21.6 MB of GPU buffer. That is comparable to
  one full-size float post texture (16.6 MB at 1080p) against NFR section 12's ~350 MB soft working-set
  ceiling — real, bounded, and paid whether or not the target is large.
- **A tier is no longer one number a reader can look up.** `attractor_particles` becomes an anchor in
  a formula, and two constants (`REFERENCE_PX`, the ceiling) join it. The tier table gets harder to
  read, which is the price of it being right.
- **The frame-time governor sees a heavier scene at high resolutions** — exactly where it was already
  most likely to demote. A demotion to `Floor` now also drops the density anchor, so the visible step
  is larger than it was.
- **The per-particle deposit gets small at high counts**, and the accumulation is a limited-precision
  float target. Whether the deposit quantizes at the ceiling is a **measurement this ADR does not
  make** — it is the first thing the plan's measurement phase reads, and it is what would bound the
  ceiling below the frame-time bound.
- **The offline path diverges from the live one by construction.** A render is no longer the same
  frames the app would have drawn at that size, which is a property `shot` otherwise tries to keep.

### Neutral

- Only the attractor family is in scope. `swarm_particles` and `emitter_objects` have the same
  shape of exposure and are deliberately not touched here — they draw marks whose size is authored,
  not a deposit smoothed over texels, so the argument would have to be remade rather than reused.

## Alternatives considered

### Alternative A — Raise the flat tier count

Give `Rich` more particles and leave the law out. **Rejected because the defect is a ratio, not a
level.** The density is already accepted at 640x360; a flat raise over-samples the small window on
every machine to fix a large one, spends live budget where nothing was wrong, and lands the project
back here the first time someone renders at 4K.

### Alternative B — A preset-authorable density parameter, and nothing in the engine

Put the sample budget in the content lane, where the rest of the attractor's look lives. **Rejected
because it makes resolution compensation a per-preset duty.** `[particles] density` already exists
(ADR-0069) and is the *look* lever — how sparse this world wants to be. Overloading it with "and also
correct for the display" means retuning 17 attractor worlds per target size by hand, which is the
work a tier exists to not do.

### Alternative C — Accumulate more integration steps per frame offline, instead of more particles

The cheapest-looking offline lever: same buffer, N steps per frame at 1/N the deposit. **Rejected
because a step is not a sample here.** The compute steps run off a fixed-timestep accumulator at
`FIXED_STEP = 1/60 s` of *injected real* `dt` (Plan 0014, [ADR-0135](0135-every-scene-rate-integrates-through-one-shared-phase.md)),
so extra steps per frame either advance the system faster than wall-clock or shrink the step; and the
map families (IFS, Clifford, and the rest of the four) iterate once per step, so N steps per frame is
N times the iteration rate — a change to the look, not a densification of it. More particles is
precisely what ADR-0065 normalized the deposit for.

### Alternative D — Supersample the offline render and downsample

Render at 2x or 4x and box-filter down: one lever, every scene benefits, no per-scene arithmetic.
**Rejected because it does not add samples.** The same 150,000 deposits spread over 4x the texels and
averaged back down carry the same information — the grain gets softer, not finer, and the sparsity
that produced the "upscaled" verdict is untouched. It also multiplies the post chain's memory (~66 MB
per full-size float chain at 1080p; Plan 0023's dual-live peak is already ~246 MB) and pushes the
trail grid past its cap.

## Notes

- Raised as [design-backlog 0110](../design-backlog.md) (2026-08-17), from Plan 0101 Phase 5.
- The entry names shapes (a) scale with the target, (b) a preset param, (c) offline sub-steps, and
  notes (a) and (c) compose. This ADR takes (a) with two ceilings, which is (a) and (c)'s intent
  without (c)'s mechanism — see Alternative C for why the mechanism lost.
- `REFERENCE_PX` and both ceilings are **measurements, not constants chosen here.** The plan's first
  phase reads density, frame time and deposit precision across three target sizes and sets them; an
  ADR that named them would be inventing numbers it did not take.
