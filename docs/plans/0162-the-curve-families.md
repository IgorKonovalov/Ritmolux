# 0162 — The curve families

> **Status:** draft
> **Created:** 2026-09-09
> **Owner skill(s):** dev
> **Related ADRs:** [0180](../adrs/0180-a-mathematical-world-joins-a-system-as-a-family-and-a-structural-parameter-is-held.md)
> (rule 1 — a new world joins a system as a family),
> [0007](../adrs/0007-line-geometry-generators.md) (the `CurveFamily` seam this widens),
> [0029](../adrs/0029-parametric-curve-shape-params.md) (which rejected the superformula on a
> premise this plan spends),
> [0098](../adrs/0098-the-line-renderer-draws-arcs-as-per-pixel-distance-fields.md) (the arc fit every new family
> inherits for free)
> **Depends on:** [0161](0161-the-structural-parameter-is-held.md) — the integer levers below are
> worth binding only once `[hold]` exists.

## TL;DR

`CurveFamily` goes from one variant to five: the existing `maurer_rose` plus **Lissajous**,
**hypotrochoid/epicycloid**, **superformula** and **harmonograph**. All four are new arms on the
existing `parametric_curve` scene, drawn through the same `LineRenderer`, coloured through the same
palette, and — unlike the Maurer chord web — fitted to G1 arc chains by the biarc fitter that
already exists, so they arrive smooth rather than faceted. This is
[`generative-techniques-catalogue.md`](../generative-techniques-catalogue.md)'s number-one
payoff-per-effort item, unbuilt since 2026-07-25.

## Context & problem

`core/src/render/scenes/lines/mod.rs:246` carries the whole of the engine's parametric-curve
vocabulary:

```rust
pub enum CurveFamily {
    /// The Maurer rose — `sin(n * theta)` walked at a fixed angular step.
    MaurerRose,
}
```

Its own doc comment says *"Extend as Plan 0010's follow-ups add curve families (epicycloids,
Lissajous, ...)"*. Those follow-ups never came. Five shipped presets name `parametric_curve` and all
five draw the same figure with different numbers, which is a concrete instance of the roadmap's
*"~55 % of the library is one template per family with different numbers"* finding.

The seam to widen is already the right shape. `parametric.rs:525` and `:537` are two `match
self.family` arms — one choosing the arc-fitted path, one the polyline fallback — so a family is a
sampler plus a fit decision, not a new scene. And the arc fitter is generic over a sampled outline
([ADR-0098](../adrs/0098-the-line-renderer-draws-arcs-as-per-pixel-distance-fields.md)); `biarc.rs:13` already
breaks the chain at genuine corners, so a superformula's cusps survive it.

The one thing that made this unattractive before is gone. ADR-0029 rejected the superformula as
"more than the routed need" — [`roadmap-visual-richness.md`](../roadmap-visual-richness.md) replaced
that premise, and [ADR-0180](../adrs/0180-a-mathematical-world-joins-a-system-as-a-family-and-a-structural-parameter-is-held.md)
records the replacement as a decision.

## Decision

We add four `CurveFamily` arms to `parametric_curve`. Each supplies a sampler over `theta` and a fit
verdict; the four new ones all verdict **true** — they are curves, not chord webs, so they take
ADR-0098's arc path unconditionally and the `corner_fraction` heuristic that guards the Maurer case
is not consulted for them.

Parameter surface: `n`, `d` and `phase` are **reused** with family-specific meaning, following the
`attractor` precedent where `a`..`d` mean different things per family
(`particles/family.rs:84`), and four genuinely new levers join for the shapes that need them —
`sym`, `sharpness`, `lobe`, `pen`, `decay`. Per
[ADR-0180](../adrs/0180-a-mathematical-world-joins-a-system-as-a-family-and-a-structural-parameter-is-held.md)
rule 4 the generated reference names the family each one reads on, so an inert parameter is visibly
inert.

We rejected giving each family its own `SystemKind` (ADR-0180 Alternative A) and giving each its own
private parameter namespace (it would fork the `[curve]` table five ways for four numbers).

## Architecture diagram

```mermaid
flowchart TD
    subgraph preset["preset (.toml)"]
        CT["[curve] family = superformula"]
        PB["[params] sym, sharpness, lobe, n, d"]
        HOLD["[hold] sym = bar"]
    end
    subgraph lines["core/src/render/scenes/lines/"]
        CF["CurveFamily::{MaurerRose, Lissajous,\nHypotrochoid, Superformula, Harmonograph}"]
        SAMP["curves::*_pieces\n(sampler + fit verdict)"]
        BI["biarc::fit\nG1 arc chain (ADR-0098)"]
        POLY["polyline fallback\n(Maurer chord webs only)"]
        LR["LineRenderer\nshared by every line scene"]
    end
    CT --> CF --> SAMP
    PB --> SAMP
    HOLD --> SAMP
    SAMP -->|"fitted"| BI --> LR
    SAMP -->|"declined"| POLY --> LR
```

## Implementation phases

### Phase 1 — the family seam takes a second arm
- **Owner skill:** dev
- **What:** Generalize the two `match self.family` sites in `parametric.rs` into a dispatch a family
  can be added to without touching the scene, and land **Lissajous** through it as proof. Lissajous
  is the right first arm because it needs no new parameters at all: `n` is the x frequency, `d` the
  y frequency, `phase` the offset between them.
- **`maurer_rose` must not move.** It keeps its `corner_fraction` verdict, its polyline fallback and
  its exact sampling. The five shipped curve presets are goldens.
- **Files touched:** `core/src/render/scenes/lines/mod.rs`,
  `core/src/render/scenes/lines/curves.rs`, `core/src/render/scenes/lines/parametric.rs`.
- **Done when:** `family = "lissajous"` with `n = 3`, `d = 2` draws the 3:2 Lissajous figure as a
  closed G1 arc chain with no tangent break; every `maurer_rose` golden is byte-identical; and an
  unknown family name is still rejected at load rather than falling back to a default.

### Phase 2 — hypotrochoid and epicycloid
- **Owner skill:** dev
- **What:** One sampler covering both, because they are the same construction with the rolling
  circle inside or outside: `n` is the radius ratio, `d` the number of cusps, and a new **`pen`**
  parameter is the tracing point's distance from the rolling circle's centre — `pen = 1` is the
  cycloid proper, `pen < 1` the curtate form, `pen > 1` the prolate. Sign of `n` selects epi vs hypo,
  so no second family name is spent.
- **Files touched:** `curves.rs`, `parametric.rs` (`PARAMS` + `set_param`), `mod.rs`.
- **Done when:** `family = "hypotrochoid"` reproduces the classic spirograph figure; `pen` sweeps
  continuously from curtate through prolate without the sampler folding or the fitter breaking; and
  a negative `n` draws the epicycloid form.

### Phase 3 — the superformula
- **Owner skill:** dev
- **What:** Gielis' superformula, `r(theta) = (|cos(m·theta/4)/a|^n2 + |sin(m·theta/4)/a|^n3)^(-1/n1)`,
  as a family with **`sym`** (the integer `m`, Structural), **`sharpness`** (`n1`) and **`lobe`**
  (`n2`/`n3`, bound together on one lever with `d` skewing them apart). This is the family that
  produces starfish, flowers, polygons and rounded shells from four numbers, and it is the one that
  most rewards a `[hold]` on `sym`.
- **Guard the exponents.** `n1` near zero sends `r` to infinity; the sampler clamps rather than
  emitting `inf`, because a `NaN` vertex is a hot-path defect and the plan's own grammar docs say a
  `NaN` parameter produces undefined-looking visuals.
- **Files touched:** `curves.rs`, `parametric.rs`, `mod.rs`.
- **Done when:** `sym = 5` with a low `sharpness` draws a five-lobed star and a high `sharpness`
  rounds it toward a circle; no parameter combination inside the declared ranges emits a
  non-finite vertex; and `sym` bound under `[hold] sym = "bar"` changes the figure's symmetry once
  a bar and holds between.

### Phase 4 — the harmonograph
- **Owner skill:** dev
- **What:** The damped two-pendulum harmonograph — `x = A·sin(n·t + phase)·e^(-decay·t)`, `y` the
  same on `d` — with a new **`decay`** parameter. It is the only family whose figure *evolves along
  its own arc length* rather than closing, which makes it the family that best rewards
  `draw_progress`, already a parameter on this scene.
- **The fit verdict is conditional here, unlike Phases 1-3.** At `decay = 0` the trace closes and is
  a curve; at high `decay` the inward spiral's later turns collapse under the sampler's resolution.
  Take the verdict from the sampled outline, not from the family.
- **Files touched:** `curves.rs`, `parametric.rs`, `mod.rs`.
- **Done when:** `decay = 0` draws the closed Lissajous-like figure and a positive `decay` spirals it
  inward over the trace; the fitter is not handed a run of coincident points at high `decay`; and
  `draw_progress` sweeps the trace from start to end.

### Phase 5 — the reference tells the truth about a family-dependent range
- **Owner skill:** dev
- **What:** `ParamSpec.range` is one pair per parameter, and after Phases 1-4 `n` reads `1..24` on a
  rose, `1..12` on a Lissajous and `-8..8` on a hypotrochoid. A single printed range would be a
  claim nothing holds — which is the exact posture `ParamSpec`'s own doc comment takes about
  unbounded parameters. The generated reference (ADR-0170) prints a per-family range for a parameter
  whose meaning is family-specific, alongside the Structural/Modal grouping
  [0161](0161-the-structural-parameter-is-held.md) Phase 4 adds.
- **Files touched:** the reference generator, `core/src/render/scenes/mod.rs` (whatever carries the
  per-family range), `presets/README.md` (regenerated).
- **Done when:** `presets/README.md`'s `parametric_curve` table states `n`'s range per family, no
  hand edit produced any of it, and the ADR-0170 drift check passes.

### Phase 6 — the documentation sweep
- **Owner skill:** dev
- **What:** `docs/presets.md`'s `[curve]` table lists five families with one line each on what `n`,
  `d` and `phase` mean under each. `docs/preset-guide.md` gets one picture per new family, through
  `scripts/docs-shots.mjs`. `presets/README.md` is regenerated by Phase 5, not hand-edited.
- **Files touched:** `docs/presets.md`, `docs/preset-guide.md`, `docs/images/`.
- **Done when:** the family table is complete, the guide's pictures are regenerated rather than
  drawn, and `node scripts/toc.mjs --check` plus `node scripts/check-doc-links.mjs` pass.

## Data shapes

```rust
// illustrative — not the final interface

pub enum CurveFamily {
    MaurerRose,
    Lissajous,
    /// Epicycloid and hypotrochoid in one sampler; the sign of `n` picks which.
    Hypotrochoid,
    Superformula,
    Harmonograph,
}

/// What a family arm supplies. The scene owns the buffers, the mirror stage and
/// the colour ramp; a family owns only the walk and whether it is a curve.
struct FamilySample {
    /// Fills `points` in the frame `maurer_rose` already draws in.
    sample: fn(&CurveParams, &mut Vec<[f32; 2]>),
    /// `true` -> take ADR-0098's arc path. Constant `true` for Lissajous,
    /// Hypotrochoid and Superformula; computed from the outline for
    /// Harmonograph (a hard `decay` collapses the later turns) and from
    /// `corner_fraction` for MaurerRose, which is the existing behaviour.
    fits: fn(&CurveParams, &[[f32; 2]]) -> bool,
}
```

## Risks & open questions

- **`n` and `d` mean five different things.** This is the `attractor` precedent working as designed,
  and it is still the plan's biggest legibility cost. Phase 5 is the containment; if the per-family
  range turns out not to be expressible in the generated reference, the fallback is a hand-written
  family table in `docs/presets.md` and a `None` range in the spec — never one invented number.
- **Five inert parameters land on a scene whose presets do not use them.** `pen`, `sym`,
  `sharpness`, `lobe` and `decay` are each meaningful for one or two families. ADR-0180 accepts this
  cost explicitly; the check is that no existing preset's behaviour changes because a new parameter
  defaults to something the rose reads.
- **The superformula can produce a degenerate figure inside its own declared range.** Small `n1`
  with large `n2`/`n3` sends the radius past any sane frame. Clamping in the sampler is the answer,
  and the clamp must be a *property* the tests can state (every vertex finite and inside a stated
  radius), not a measured number.
- **`max_segments` is a `TierConfig` cap** and a superformula at high `sym` with high `samples` will
  reach it. The existing overflow path (`mirror_overflow`) already reports truncation; confirm the
  new families route through it rather than silently drawing a partial figure.
- **`family` is not bindable, by construction.** A preset picks it in `[curve]`. Changing family on
  a beat would mean rebuilding the sampler mid-frame; it is out of scope and the load error for an
  unknown name stays.

## What this plan does NOT do

- **It does not add a curve family that needs 3D.** Torus knots and spherical harmonics stay with
  the attractor idiom, which already has a projection.
- **It does not touch `lsystem` or `star_pattern`**, the other two generator scenes.
- **It does not author presets.** Four new families deserve worlds built on them; that is
  `preset-author`'s lane and it is the natural follow-on once this lands.
- **It does not add `[hold]`** — that is [0161](0161-the-structural-parameter-is-held.md), which
  this plan depends on.

## Implementation log

> Written by `dev` — one row per phase as that phase's commit lands, and the close block after the
> last one. **The phases above are the contract; everything here is what happened.**

**Lane:** _(to be filled by `dev`)_

| phase | owner | state | commit |
|---|---|---|---|
| 1 — the family seam takes a second arm | dev | not started | |
| 2 — hypotrochoid and epicycloid | dev | not started | |
| 3 — the superformula | dev | not started | |
| 4 — the harmonograph | dev | not started | |
| 5 — per-family ranges in the reference | dev | not started | |
| 6 — the documentation sweep | dev | not started | |

### Notes

### Close triggers

- **`presets/` touched:**
- **Plan header `Closes:`** none
- **What shipped:**
- **Operator docs touched:**
- **Backlog probes (`node scripts/check-backlog-claims.mjs`):**
- **Full suite:**
- **Outstanding `human` phases:**

## Followups (after this lands)

- Route to `preset-author`: four new families with no worlds built on them. The superformula and the
  harmonograph in particular are the two most likely to produce something the library does not
  already have.
- The catalogue's pick-order item 1 is discharged by this plan; item 3 (fractal flames on the
  compute path) is still open and is now the cheapest unbuilt entry on that list.
