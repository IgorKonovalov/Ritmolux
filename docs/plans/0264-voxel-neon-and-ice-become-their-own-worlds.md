# 0264 — Voxel Neon and Ice become their own worlds

> **Status:** draft
> **Created:** 2026-10-11
> **Owner skill(s):** human
> **Related ADRs:** [ADR-0081](../adrs/0081-the-content-lane-lands-presets-and-architect-curates-the-set.md)
> **Closes:** design-backlog 0297

## TL;DR

Rework Voxel Neon and Voxel Ice so each one differs from Voxel Ember in behaviour, not only in
palette. The `preset-author` lane drafts both in `presets/proposed/`, and the owner judges them
against the shipped pair. A kept draft replaces its shipped file. **Every phase is `human`, so the
conductor does not run this plan.** The owner starts it as a `/preset-author` session, handing it
the brief in Phase 1.

## Context & problem

Backlog 0297. Neon and Ice match Ember line for line apart from the `[palette]`: same rule pair
(`pyroclastic`, `coral`), seed, `step_rate`, `reseed`, camera, light and bloom. The distinctness
gate passes them because a palette alone separates three captures. At the curation of e2aed1da, the
owner chose to differentiate them, over keeping them as palette variants or cutting them.

## Decision

Content only, in the content lane, with no engine change. The shared register stays: solid cubes lit
from within under heavy bloom. Each rework changes at least two of these axes relative to Ember:
**the rule pair or list**, **the seed shape** (`seed_radius`, `seed_fill`), **the reseed cadence or
binding**, **`step_rate`**, and **the camera** (distance, orbit, pitch). Ember is untouched and keeps
`representative = true`.

The plan exists so the brief and the verdict have one place to live. The plan vocabulary has no
`preset-author` owner, which is why the authoring phase is `human`: the owner starts the lane
session.

## Architecture diagram

None. No engine surface is touched.

## Implementation phases

### Phase 1 — The two drafts
- **Owner skill:** human
- **What:** Start `/preset-author` with this brief:
  > Rework Voxel Neon and Voxel Ice (Plan 0264). Keep each one's palette and the shared look of
  > solid cubes lit from within under heavy bloom. Each must differ from Voxel Ember on at least two
  > of: rule pair, seed shape, reseed cadence or binding, step rate, and camera. Neon should read as
  > fast and electric, Ice as slow and crystalline. Draft both as `presets/proposed/voxel_neon.toml`
  > and `presets/proposed/voxel_ice.toml`, with rows in `presets/proposed/ROSTER.md`. Render each
  > against the shipped file with `--signal dynamic:110`. Run `--horizon` on each, because a voxel
  > seed can empty out (backlog 0294). Do not edit the shipped files.

  "Fast and electric" and "slow and crystalline" are starting directions, and the owner may replace
  them when starting the session.
- **Files touched:** `presets/proposed/voxel_neon.toml`, `presets/proposed/voxel_ice.toml`,
  `presets/proposed/ROSTER.md`.
- **Done when:** Both drafts are committed in `presets/proposed/`, each with a roster row naming the
  axes it changed relative to Ember and its horizon reading.

### Phase 2 — The owner's verdict
- **Owner skill:** human
- **What:** Judge each draft live against its shipped file. Keep, tune or bin. A kept draft moves
  over its shipped file in `presets/`, its gallery card is re-rendered with
  `node scripts/docs-shots.mjs voxel_<name>`, and its `ROSTER.md` row is closed. A binned draft
  leaves the shipped file as it is, and the backlog question is answered "keep as palette variant"
  for that preset.
- **Files touched:** `presets/voxel_neon.toml`, `presets/voxel_ice.toml`,
  `docs/images/gallery/presets/voxel_neon.png`, `docs/images/gallery/presets/voxel_ice.png`,
  `presets/proposed/`.
- **Done when:** Each of the two has a verdict in this log, and `cargo nextest run -p rlx-core`
  passes with the result in the tree.

## Risks & open questions

- **Plan 0263 may change the register.** If `bounce` lands first, a rework may prefer it to the
  bloom fake. Either order works: the brief does not depend on it, and a later retune can adopt it.
- **The voxel live frame rate (backlog 0290).** A rework that raises the grid or lowers the march's
  early exit makes that question worse. The lane reads `--report`'s frame-cost block for each draft.

## What this plan does NOT do

- It does not touch Ember, Nebula, Solid, Solid Dark or Section.
- It does not add an engine capability. A wall the lane hits is a feedback note, per its skill.

## Implementation log

**Lane:**

| phase | owner | state | commit |
|---|---|---|---|
| 1 — The two drafts | human | not started | |
| 2 — The owner's verdict | human | not started | |

### Notes

### Close triggers

## Followups (after this lands)
