# 0170 — The horizon reads the frame's own ground

> **Status:** approved 2026-09-14
> **Created:** 2026-09-11
> **Owner skill(s):** `dev`
> **Related ADRs:** [0126](../adrs/0126-the-sanity-lens-measures-departure-from-the-frames-own-ground.md)
> (accepted — this plan extends its estimator to the two `shot` readings that never adopted it)
> **Closes:** design-backlog 0210, 0211.

## TL;DR

`--horizon` takes its background colour from the top-left pixel of frame 0, and on a seeded cellular
field that pixel can be a live cell: a field frozen into rings on black read coverage **0.9818**
against a true **0.0182**. This plan gives `--horizon` and `--report` the ground estimator the sanity
lens already uses (`metrics::modal_ground`, ADR-0126), pooled over every sampled image for the
horizon so the ruler still never moves. The second phase settles the Larger than Life default: it is
not changed, and the one test and the grammar reference stop claiming it keeps moving.

## Context & problem

- **0210 (Medium):** `measure` in `standalone/src/shot/horizon.rs` samples the ground once, from
  `images.first()`'s corner, and holds it for every row, so that the mask cannot move under the
  statistics. Coverage, footprint and peak/mean all read through it. The first image is frame 0, a
  cellular world's seed soup, whose corner is live about half the time. `--report`
  (`standalone/src/shot/report.rs`) uses the same corner convention on the frame it scores, and
  `docs/capturing.md` describes its `cover` column that way.
- **0211 (Low):** the default rule (radius 5, birth 0.28-0.385, survival 0.26-0.47) was chosen by a
  test on a 128-cell torus from two salts. On Tide Bugs' own 256 grid with reseed off, nothing moves
  from 90 s on. The test's name, `larger_than_life_is_still_moving_after_two_thousand_generations`,
  claims a property of the family that one grid and two seeds cannot carry.

## Decision

The interview chose the pooled modal ground and documentation over a re-sweep. The horizon takes
`modal_ground` over the histogram of **every** sampled image together, computed once and held — so
the noisy frame 0 is outvoted by the settled frames, and the ruler still does not move between rows.
`--report` takes `modal_ground` of the frame it scores. We rejected `modal_ground` of frame 0 alone (a
50 % soup is a near-tie, and ties resolve to the brighter band, the live cell), keeping the corner with
a warning (the reading stays wrong), and a scene-declared clear colour (widens what the shell asks the
core, for a problem the existing estimator solves). For 0211 we rejected a re-sweep: it costs GPU test
time with no guarantee a sustaining rule exists, and the one shipped world already reseeds on purpose.

## Architecture diagram

```mermaid
flowchart LR
    subgraph core["core/src/render/metrics.rs"]
        MG["modal_ground (ADR-0126)"]
        MP["pooled form: one histogram<br/>over many images"]
        MG --- MP
    end
    subgraph shot["standalone/src/shot"]
        H["horizon measure:<br/>bg = pooled over every sample, held"]
        R["report: bg = modal_ground(scored frame)"]
    end
    MP --> H
    MG --> R
    H --> ROWS["coverage / footprint / peak-mean rows"]
    R --> COVER["cover column"]
```

## Implementation phases

### Phase 1 — `--horizon` and `--report` measure from the frame's own ground
- **Owner skill:** dev
- **What:** A pooled form of `modal_ground` in `core/src/render/metrics.rs` (one luminance histogram
  summed over a slice of images, the same band mean and the same `NO_GROUND` rule). `measure` uses it
  once over all sampled images; `report.rs` uses `modal_ground` on the frame it scores. The horizon's
  header prints the ground it measured from. `docs/capturing.md` stops saying "corner".
- **Files touched:** `core/src/render/metrics.rs`, `core/src/render/metrics/tests.rs`,
  `standalone/src/shot/horizon.rs`, `standalone/src/shot/report.rs`, `docs/capturing.md`.
- **Notes for the implementer:**
  - Treat `NO_GROUND` exactly as `core/tests/sanity.rs` does. Do not invent a third treatment.
  - The pooled form must give the same answer as `modal_ground` on a slice of one image, so the two
    cannot drift; test that equality.
  - `--report`'s `cover` column moves on any preset whose corner was not its modal ground. Run
    `shot --presets presets --report` before and after and put the `cover` changes in the log. The
    property to check is that **every** change is on such a preset; one that is not is a finding.
- **Done when:**
  - a metrics test pools one half-lit frame with six mostly-black frames and gets black;
  - the pooled form equals `modal_ground` on a single image;
  - re-running backlog 0210's recipe prints coverage from 60 s on as the complement of what it printed
    on 2026-09-11 — about 0.018 where it printed 0.9818 — and the header names a black ground.

### Phase 2 — The Larger than Life default is documented as settling
- **Owner skill:** dev
- **What:** Rename the sustain test so it states what it measures (the default rule, a 128-cell torus,
  salts 12 and 13), and rewrite its doc so it does not claim the property for the family. Add a
  paragraph to `docs/presets.md`'s `larger_than_life` section: a world on this family with no reseed
  settles, on the default rule within about a minute, so `reseed` is how a world stays alive.
- **Files touched:** `core/src/render/scenes/cellular/tests.rs`, `docs/presets.md`.
- **Notes for the implementer:**
  - The default values do not change, so `presets/README.md` (generated) does not either.
  - `docs/presets.md` is a reader document: no bare plan or ADR citations in it
    (`scripts/check-reader-prose.mjs`).
  - Backlog 0211's second probe names the old test function and goes red on this phase by design.
- **Done when:** the test carries its new name and an accurate doc, and `docs/presets.md` says an
  unreseeded `larger_than_life` world settles.

## Risks & open questions

- **A duotone preset has two grounds.** `modal_ground` breaks ties toward the brighter band, and
  ADR-0126's Outcome records that as arbitrary but deterministic. The horizon inherits that; the
  header naming the ground is how a reader sees which one it picked.
- **Pooling assumes the settled frames outnumber frame 0.** A two-row horizon (an interval equal to
  the length) pools two images and can still tie. The header line is the visible check.
- **`--report`'s near-duplicate flags do not read the ground**, so they should not move; if they do,
  that is a finding.

## What this plan does NOT do

- **It does not change the Larger than Life default** or re-sweep it.
- **It does not re-sample the ground per row.** The horizon's fixed ruler stays; only its source moves.
- **It does not touch the sanity lens**, which already measures this way.

## Implementation log

> Written by `dev` — one row per phase as that phase's commit lands, and the close block after the
> last one. **The phases above are the contract; everything here is what happened.**

**Lane:** _(to be filled on the first phase commit)_

| phase | owner | state | commit |
|---|---|---|---|
| 1 — `--horizon` and `--report` measure from the frame's own ground | dev | not started | |
| 2 — The Larger than Life default is documented as settling | dev | not started | |

### Notes

### Close triggers

- **`presets/` touched:**
- **Plan header `Closes:`** design-backlog 0210, 0211
- **What shipped:**
- **Operator docs touched:**
- **Backlog probes (`node scripts/check-backlog-claims.mjs`):**
- **Full suite:**
- **Outstanding `human` phases:**

## Followups (after this lands)
