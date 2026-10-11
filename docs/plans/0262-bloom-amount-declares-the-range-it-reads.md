# 0262 — `bloom_amount` declares the range it reads

> **Status:** in-progress
> **Created:** 2026-10-11
> **Owner skill(s):** dev
> **Related ADRs:** [ADR-0170](../adrs/0170-a-parameters-reference-row-is-generated-from-the-declaration-the-engine-reads.md)
> **Closes:** design-backlog 0296

## TL;DR

Widen `bloom_amount`'s declared `ParamSpec::range` from `[0, 1]` to `[0, 2]`. Then regenerate the
parameter reference, the editor schemas and the player schema, and bring the hand-written bloom
prose in line with them. The clamp stays at 4. The visible result is the generated table, the
`.taplo.toml` editor hint and the studio's slider all reaching the `1.3`–`1.5` that five shipped
presets use.

## Context & problem

Backlog 0296. `ParamSpec::range` is "the range that reads", not a clamp. For `bloom_amount` it says
`0..1`, but Clifford and the fragment Nebula ship at `1.5`, Leviathan at `1.4`, Sumi at `1.35` and
Dragon at `1.3`. The stage clamps at `MAX_AMOUNT = 4.0`, and the hand-written prose has always called
everything past `2` "mostly halo". On 2026-10-11 the prose was patched to state the generated range
and the shipped values side by side, which is accurate but awkward. The declaration is the half that
is wrong.

## Decision

`[0.0, 2.0]`. Its upper end is the point the prose already names as "mostly halo", and it covers
every shipped value with room to spare. We rejected `[0.0, 4.0]`, the clamp: past 2 the frame is halo
and the tonemap's shoulder, so a slider spanning `2`–`4` is half dead travel. That is the very thing a
"range that reads" exists to exclude. No pixel changes, because a range is never a clamp.

## Architecture diagram

None. The change is one declaration plus the files generated from it.

## Implementation phases

### Phase 1 — The declaration, the regeneration and the prose
- **Owner skill:** dev
- **What:**
  - In `core/src/render/bloom.rs`, change `bloom_amount`'s `range` from `Some([0.0, 1.0])` to
    `Some([0.0, 2.0])`.
  - Regenerate with `RLX_UPDATE_PARAM_REFERENCE=1` and `RLX_UPDATE_PRESET_SCHEMA=1`. That rewrites
    the parameter block in `presets/README.md`, `presets/schema/`, `presets/preset.schema.json`,
    `.taplo.toml` and `docs/specs/player-schema.json`, whichever of them carry the range.
  - In `presets/README.md`'s hand-written bloom table, the `bloom_amount` cell says `0`–`2` reads,
    most glows sit under `1`, the shipped heavy halos sit at `1.3`–`1.5`, and the engine clamps at
    `4`. It no longer describes a disagreement with the generated table.
- **Files touched:** `core/src/render/bloom.rs`, `presets/README.md`, `presets/schema/`,
  `presets/preset.schema.json`, `.taplo.toml`, `docs/specs/player-schema.json`.
- **Done when:**
  - `cargo nextest run -p rlx-core --test suite preset_schema::` and
    `cargo nextest run -p rlx-core --test suite preset::the_parameter_reference_block_is_current`
    pass with no update variable set.
  - `git grep -c "range: Some(\[0.0, 2.0\])" -- core/src/render/bloom.rs` prints at least 1.
  - `cargo nextest run -p rlx-core --test golden` passes with `RLX_BLESS` unset, so no pixel moved.
  - `node scripts/check-backlog-claims.mjs` passes. Entry 0296 moved to the archive when this plan
    was approved, so its range probe is no longer run.

## Risks & open questions

- **The studio reads the player schema.** A wider range moves the `bloom_amount` slider's scale, so
  a value the owner set by eye at mid-travel sits at a different place on the track. Values in the
  file are unchanged. Nothing to do beyond knowing it.

## What this plan does NOT do

- It does not change `MAX_AMOUNT` or any other bloom constant.
- It does not touch any preset.
- It does not audit the other params' declared ranges against shipped values. That would be a
  worthwhile sweep, but it is its own plan.

## Implementation log

**Lane:** `plan-0262-bloom-amount-declares-the-range-it-reads`, worktree `/home/igor/Work/rlx-plan-0262`

| phase | owner | state | commit |
|---|---|---|---|
| 1 — The declaration, the regeneration and the prose | dev | done | 4c9e215f |

### Notes

- `.taplo.toml` did not change on regeneration: it carries no per-param range, so the phase's
  file list held one file the regeneration did not touch.

### Close triggers

- **`presets/` touched:** yes — `presets/README.md` (generated block + hand-written bloom row),
  `presets/preset.schema.json`, all 17 files under `presets/schema/`. No `.toml` preset.
- **Closes:** design-backlog 0296 (already archived at approval).
- **Shipped:** fix-only (a declared range; no behaviour or pixel change).
- **Operator docs moved:** `presets/README.md`, `docs/specs/player-schema.json`.
- **`node scripts/check-backlog-claims.mjs`:** exit 0 — 48 reductions hold across 23 live entries.
- **`human` phases remaining:** none.
- **Full suite:** owed to the conductor's pre-review gate (ADR-0207).

## Followups (after this lands)
