# ADR-0195 — A low `density` is a trace count, and the density law scales only a cloud

> **Status:** accepted 2026-09-17
> **Date:** 2026-09-14
> **Related plan(s):** [0183](../plans/done/0183-a-low-density-is-a-trace-count.md)
> **Amends:** [0140](0140-a-sample-budget-is-a-density-against-the-render-target.md) (the sample
> budget is a density against the render target)
> **Supplements:** [0069](0069-the-attractor-trades-sample-count-for-trace-length.md) (`[particles]
> density`), [0065](0065-the-attractor-deposit-is-normalized-by-particle-count.md) (the deposit is
> normalized by the drawn count)

## Context

ADR-0069 and ADR-0140 each resolve one number, and they were written for different pictures.

ADR-0069 added `[particles] density` for the **trace**. A low density draws few enough trajectories
that each one can be followed, and the count is the look. `attractor_thomas.toml` carries the ladder
it was chosen from, rendered rather than reasoned: *"1.0 / 0.02 / 0.005 / 0.002 goes blot, drawing,
sketch, scribble"*. ADR-0140 made the budget that density is a fraction of scale with the render
target, `clamp(round(anchor * target_px / REFERENCE_PX), anchor, ceiling)`, and argued it entirely
for the **cloud**. At `density = 1.0`, more samples is the same total light with less shot noise,
because `deposit_scale` divides by the drawn count (ADR-0065). ADR-0140's own Outcome records that
the two do not compose for the trace, and backlog 0186 has the arithmetic.

The scene draws `round(budget * density)` (`active_particles` in
`core/src/render/scenes/particles/mod.rs`). Against the shipped tier constants (`core/src/render/tier.rs`:
`Rich` anchor 150,000, live ceiling 600,000, offline ceiling 2,700,000; `REFERENCE_PX` 230,400), a
trace world at `density = 0.02` draws:

| target | `Rich` budget, window | drawn, window | `Rich` budget, `--render` | drawn, `--render` |
|---|---|---|---|---|
| 640x360 | 150,000 | **3,000** | 150,000 | **3,000** |
| 1280x720 | 600,000 | 12,000 | 600,000 | 12,000 |
| 1920x1080 | 600,000 | 12,000 | 1,350,000 | 27,000 |
| 3840x2160 | 600,000 | 12,000 | 2,700,000 | 54,000 |

Each stroke also carries a quarter (or a ninth) of the deposit, because that is how total light
stays invariant. For a cloud, that is the intended result. For `fragment_sumi`, whose brief is
*"glowing calligraphy"* and whose header says the mechanism is being *"sparse enough that each
trajectory is a stroke, not a fog"*, it is a different picture.

**Nothing can see this.** Every golden and sanity baseline is `Tier::Floor` at 128x128 or 96x96,
where the law is a no-op twice over. `Floor`'s live ceiling is its anchor, and both sizes are under
`REFERENCE_PX`. The disagreement exists only at the size the app runs at. This is ADR-0037's lesson
one level over.

**The authored population has a gap, and the gap is what makes a boundary drawable.** Every
`[particles] density` in `presets/` on 2026-09-14 is either a trace or a figure/cloud, with nothing
between 0.06 and 0.18:

| density | presets | what the header says the value buys |
|---|---|---|
| 0.006 | `attractor_lorenzgallery` | sparse, so dense roster entries do not fill in |
| 0.02 | `attractor_thomas`, `_valentine`, `_torusknot`, `_walkknot`, `_walkrho`, `_lorenzknot`, `fragment_sumi` (layer) | one stroke per trajectory |
| 0.03 | `attractor_walkthomas`, `_thomasgallery` | the same |
| 0.06 | `attractor_thomasred` | the same, denser |
| 0.18 | `attractor_fernmono` | *"the plant's solidity rather than a stroke count"* |
| 0.4, 0.5, 0.5 | `attractor_walkdejong`, `_cliffordgallery`, `_dejonggallery` | map families, point clouds |
| 1.0 (absent) | `attractor_fern`, `attractor_leviathan` | the whole budget |

The eight worlds in backlog 0186's table, plus `attractor_lorenzknot` at 0.02, which that table
missed, are all at or under 0.06.

## Decision

We will resolve the drawn count against an **effective budget** that is the tier's anchor for a
trace, the ADR-0140 budget for a cloud, and a linear blend between them over a transition band that
sits inside the authored gap:

```text
w         = clamp((density - TRACE_DENSITY) / (CLOUD_DENSITY - TRACE_DENSITY), 0, 1)
effective = anchor + (budget - anchor) * w
drawn     = clamp(round(effective * density), 1, budget)

TRACE_DENSITY = 0.08    at or below: the anchor's count at every target size
CLOUD_DENSITY = 0.16    at or above: ADR-0140's law, unchanged
```

`budget` is still what ADR-0140 resolves, `anchor` is still the tier's `attractor_particles`, and the
allocation is still made once at the ceiling. What changes is only the number drawn out of that
allocation below `CLOUD_DENSITY`.

The formula has four properties, and the plan asserts each one on the value rather than on pixels:

1. **A trace is a fixed count.** At or below `TRACE_DENSITY`, `w = 0` and the drawn count is
   `round(anchor * density)` at every target size under both ceilings: 3,000 at `Rich` and 1,000 at
   `Floor` for `density = 0.02`. The tier still means something, as ADR-0069 intended.
2. **A cloud is untouched.** At or above `CLOUD_DENSITY`, `w = 1` and the result is today's
   `round(budget * density)`.
3. **Where the law was already a no-op, so is this.** When `budget == anchor` (every `Floor` window,
   and every target at or under `REFERENCE_PX` at either tier), `effective` is `anchor` whatever `w`
   is. Every committed baseline therefore resolves exactly today's count.
4. **Continuous and monotone in `density`.** Both factors of `effective * density` are
   non-decreasing in `density`, and the two outer arms meet the blend at its ends. So two presets
   0.001 apart never differ by the 4x or 18x step a single threshold would put between them.

The band's two constants are a **classification of authored intent**, not a frame-time measurement.
They bracket the population's gap with margin on both sides: 0.08 is a third above the densest trace
(0.06), and 0.16 sits below the sparsest figure (0.18), a doubling apart. A density inside the band
is legal and scales partly with the window. The arithmetic of that partial scaling is in the
Negative section.

## Consequences

### Positive

- **An authored trace keeps its look at every window size, live or rendered.** The count the author
  picked from a rendered ladder is the count drawn, and so is the per-stroke weight
  `FLOOR_PARTICLES / drawn` (ADR-0065).
- **The two stale headers become true again instead of being rewritten around a moving number.**
  `attractor_thomas`'s *"1 000 particles at the floor tier, 3 000 at rich"* and `fragment_sumi`'s
  *"~1000 particles"* describe the `Floor` and `Rich` anchors, and after this decision they hold at
  every size.
- **A trace world gets cheaper where it was most expensive.** At a 1080p `Rich` window, a
  `density = 0.02` world steps and draws 3,000 particles instead of 12,000, and 3,000 instead of
  27,000 under `--render`. `attractor_lorenzknot`'s header records the app falling to ~20 fps at
  `density = 1.0`, which is the direction this moves.
- **No golden moves**, by property 3, and the property can be asserted on the count.
- **It stops recurring.** A trace preset authored after this lands is correct at every size without
  a per-display retune.

### Negative

- **Two more constants in a law that ADR-0140 already said a reader cannot look up.** The effective
  budget is now a function of density, the anchor, the target and the ceiling.
- **The transition band is steep by construction.** Across it, the drawn count rises by
  `(CLOUD_DENSITY / TRACE_DENSITY) * (budget / anchor)`. That is 8x at a 1080p `Rich` window (12,000
  at 0.08 to 96,000 at 0.16) and 36x at a 4K `Rich` render (12,000 to 432,000), while density only
  doubles. A world authored inside the band gets a look that depends partly on the window, which is
  the original defect in a weaker form. No shipped preset is inside it, but nothing stops one being
  authored there.
- **The boundary is judged, not measured.** A density of 0.08 is a trace here because nothing
  authored between 0.06 and 0.18 says otherwise. A future world that wants a 0.10 trace, or a 0.12
  cloud, finds the band in the way. The response is an amendment, not a per-preset key (Alternative
  C).
- **A trace no longer gets less shot noise at a large target.** The size at which ADR-0140 improved
  grain is exactly the size at which a trace's sparseness is the look. That trade is this decision.
- **`--render` stops being uniformly denser than a window.** A trace renders the same count as a
  window of the same tier. `docs/capturing.md`'s table has to say so.

### Neutral

- Only `attractor` reads `[particles] density`, so only its scene changes. `swarm` and `emitter`
  stay out of scope for the reason ADR-0140's Neutral section gives.
- The resolved **budget**, which `shot --render`'s header prints and `Scene::sample_budget` reports,
  is unchanged. Only `active_count` moves.

## Alternatives considered

### Alternative A — Accept the law and retune the trace presets per display

Leave ADR-0140 alone and route the ten files to `preset-author`. **Rejected because this is the
per-preset duty ADR-0140's own Alternative B lost for.** A preset cannot be retuned *per display*,
only for one display, so each world would be correct at one size and wrong at every other, and the
duty would recur for every trace authored after it.

### Alternative B — A single density threshold

`drawn = round(anchor * density)` below one value, `round(budget * density)` above it. The smallest
possible change. **Rejected because it puts a step of `budget / anchor` into the count at one
density value**: 4x at a 1080p `Rich` window and 18x at a 4K `Rich` render. `density` is structural
and never eases across it within a frame, but two presets authored a hair either side of the
threshold would draw different pictures for no visible reason. The band costs one extra constant and
removes the step.

### Alternative C — A per-preset `[particles]` key choosing trace or cloud scaling

An explicit `scale = "trace" | "cloud"`. It needs no boundary to be judged, and an author can put a
0.12 trace wherever they like. **Rejected because it adds a name to the surface for a choice the
density already makes for every shipped preset.** Every trace in the library is sparse and every
cloud is dense, so the key's correct value is derivable from data the scene already has. That is
ADR-0133's Alternative A argument, and it applies here unchanged. If a world ever needs a dense trace
or a sparse cloud, that preset is the evidence for the key.

### Alternative D — A power law instead of a band (`effective = anchor * (budget / anchor)^(density^k)`)

A smooth curve with no constants beyond `k`. **Rejected because it never makes a trace exactly
fixed.** At `k = 1` a `density = 0.02` world still scales by `(budget / anchor)^0.02`, which is 1.06x
at 4K render. The decision the user took is that a trace count is fixed, not nearly fixed, and only a
flat arm below a boundary delivers that.

## Notes

- Raised as backlog 0186 (2026-09-04), at Plan 0128's close review.
- Why the arms are written as today's expression: the scene resolves the count in `f32`, and a
  reimplementation in `f64` could round a `.5` case differently. The plan keeps both outer arms on
  the existing `(n as f32 * density).round()` form, so property 2 and property 3 are exact rather
  than within one particle.
