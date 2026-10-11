# 0262 — `bloom_amount` declares the range it reads

> **Status:** done - closed 2026-10-11. Phase 1 in 4c9e215f; conductor close round 1: no blockers, no majors, two minors (one fixed), one nit (fixed). Full suite green on the tagged tip. v0.176.1.
> **Created:** 2026-10-11
> **Owner skill(s):** dev
> **Related ADRs:** [ADR-0170](../../adrs/0170-a-parameters-reference-row-is-generated-from-the-declaration-the-engine-reads.md)
> **Closes:** design-backlog 0296

## TL;DR

Widen `bloom_amount`'s declared `ParamSpec::range` from `[0, 1]` to `[0, 2]`. Then regenerate the
parameter reference, the editor schemas and the player schema, and bring the hand-written bloom
prose in line with them. The clamp stays at 4. The visible result is the generated table, the
editor schemas `.taplo.toml` routes to, and the studio's slider all reaching the `1.3`–`1.5` that five shipped
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

## Close review

### Plan 0262 — close review, round 1

Graded at tip `025893532923149f8ac3464cd99941f844b31823` on
`plan-0262-bloom-amount-declares-the-range-it-reads`.

**Verdict: Plan 0262 landed as written. No blockers and no majors; two minors and one nit.** The
declaration, the regeneration and the prose edit are all present, and the suite is green. The plan's
Decision rests on a premise the tree contradicts: "it covers every shipped value with room to spare".
A shipped representative preset, Tidepool, binds `bloom_amount` to `2.2`–`3.1`, so `[0, 2]` still
excludes it. The edited prose repeats the omission. It is graded minor, not major, for three reasons.
A range never clamps, so no pixel moves. Tidepool's value is an expression, so no slider holds it. And
`[0, 2]` is a strict improvement on `[0, 1]`. Whether to widen further is the owner's call, and a `dev`
fix session cannot make it.

#### Lens 1 — Alignment

- **Implementation log:** present, shorter than the phases section, lane recorded, one phase mapped
  to `4c9e215f`. Its note about `.taplo.toml` is accurate. `.taplo.toml` carries no per-param range,
  so regeneration left it alone.
- **Owner tag:** Phase 1 carries `**Owner skill:** dev`, the only tag in the plan. Well-formed.
- **The declaration:** `core/src/render/bloom.rs:888` reads `range: Some([0.0, 2.0])`. The done-when
  `git grep -c "range: Some(\[0.0, 2.0\])" -- core/src/render/bloom.rs` holds: the diff shows exactly
  that line. `MAX_AMOUNT` is untouched and no preset changed, which matches "does NOT do".
- **Regeneration:** the diff touches `presets/preset.schema.json`, all 17 files under
  `presets/schema/`, `docs/specs/player-schema.json` and the generated parameter block in
  `presets/README.md` (`0` – `2`). Each is a one-line change. The full suite includes
  `preset_schema::` and `preset::the_parameter_reference_block_is_current` with no update variable
  set, and it is green, so the generated files match the engine.
- **Goldens:** `cargo nextest run -p rlx-core --test golden` runs as part of the full suite, which was
  green with `RLX_BLESS` unset. No pixel moved.
- **Full suite:** the wrapper skipped the run and printed the ledger record. That record is the
  evidence:
  `with-lock: skipped cargo nextest run --workspace: tree 1581856 is green in the suite ledger, run by gate 0262-pre-review at 2026-10-11T17:15:06.645Z: 2104 tests run: 2104 passed (17 slow), 9 skipped`.
  The `Full suite:` bullet in the close triggers defers to that gate. In conductor mode that is
  correct.
- **`cargo doc`:** `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` is
  clean.
- **Backlog probes:** `node scripts/check-backlog-claims.mjs` exits 0: 48 reductions across 23 live
  entries. Entry 0296 is in the archive, so its probes do not run.
- **No tests were promised** beyond the existing drift guards, and those ran.

#### Lens 2 — Layering, real-time safety, contracts

Nothing applies. The change is a `const` declaration's metadata. No ABI, no protocol address and no
hot-path code is involved. The player schema's `hash` moved, which is expected when a declared range
changes. The studio reads the schema and does not pin the hash.

#### Lens 3 — Docs and bookkeeping

- The hand-written bloom row in `presets/README.md` was rewritten as the plan asked. It now asserts
  something false about the shipped set (minor 1).
- No other prose states a `0..1` range for `bloom_amount`. Searching `docs`, `presets`, `studio`,
  `site`, `.claude`, `core` and `standalone` finds only the archived backlog body and the plan.
  `.claude/skills/preset-author/references/systems.md:739` gives no range for amount.
- **Version bump owed: patch.** This is a fix-only change: a declared range, no behaviour change.
- Backlog 0296 has `**Closes:**` in the plan header, so step 3c applies at the close: its body is
  already archived, and the close adds the CLOSED marker and moves the ledger row.
- Step 3b (preset curation) is triggered only by `presets/README.md` and the schemas. No `.toml`
  changed. Tidepool is the one preset whose value the change leaves outside the range (minor 2).

#### Lens 4 — Correctness

There are no numeric assertions in the diff. The one "number" is the declared range itself. That
raises lens 4's question about what the development set cannot see. The backlog entry and the plan
built their inventory of shipped values from **constant** assignments: Clifford, Nebula, Leviathan,
Sumi and Dragon. That inventory missed two presets:

- `presets/fragment_vitrail.toml:91` sets `"1.3"`, which is inside `[0, 2]` and does no harm.
- `presets/waterfall_tidepool.toml:47` sets `"2.2 + clamp(onset * 1.5, 0, 0.9)"`, which runs from
  `2.2` to `3.1`. It is `representative = true` and has shipped since `eca27018` (2026-10-06), before
  this plan was drafted.

#### Lens 5 — Design integrity

Unaffected.

#### Findings

##### minor 1 — `presets/README.md:3965`: the hand-written row describes a shipped set without Tidepool

The cell reads: "`0`–`2` reads; most glows sit under `1`, and the shipped heavy halos sit at
`1.3`–`1.5`. The engine clamps at `4`, but past `2` the frame is mostly halo." Tidepool ships at
`2.2`–`3.1` on `bloom_threshold = 0.2`. Its header calls the pool "deliberately bright", so the
sentence is false for one shipped preset. This prose is the content lane's guide, so the preset-author
lane would write against a set that does not exist. A close may repair it, because it is Markdown
outside `.claude/`. **Replacement text** for the cell:

> Strength of the halo added back. `0` = off. `0`–`2` reads; most glows sit under `1`, and the shipped
> heavy halos sit at `1.3`–`1.5`. The engine clamps at `4`, and past `2` the frame is mostly halo: Tidepool
> runs `2.2`–`3.1` on a low `bloom_threshold` as a deliberate full-frame wash. Bindable — this is the one
> to put a beat on.

##### minor 2 — `core/src/render/bloom.rs:888`: `[0, 2]` still excludes a shipped preset, against the plan's own premise

The plan's Decision says `[0.0, 2.0]` "covers every shipped value with room to spare". The editor
schema and the studio slider still cannot reach Tidepool's `2.2`–`3.1`. That repeats the problem
backlog 0296 opened with, now for one preset instead of five. The implementation follows the plan, so
this belongs to the plan, not the code. Do not route it to a `dev` fix session. **Owner call:** keep
`[0, 2]` and treat Tidepool as the documented outlier (minor 1's text does this), or widen to
`[0, 3.2]` and record why the "half dead travel" argument fails. If the owner wants the latter, raise
it as a backlog entry at the close. The close should also record in the `## Close review` that the
Decision's premise was false when the plan was approved.

##### nit 1 — `docs/plans/0262-bloom-amount-declares-the-range-it-reads.md:14`: the TL;DR names `.taplo.toml` as a surface that shows the range

The TL;DR says "the `.taplo.toml` editor hint" now reaches `1.3`–`1.5`. `.taplo.toml` carries no
per-param range: it maps a filename family to a schema, and the implementation log says so. The
editor hint comes from `presets/schema/*.schema.json`. A close may repair this plan prose: replace
"the generated table, the `.taplo.toml` editor hint and the studio's slider" with "the generated
table, the editor schemas `.taplo.toml` routes to, and the studio's slider".

#### Bookkeeping the close owes

- Plan to `Status: done`, `git mv` to `docs/plans/done/`, links re-pointed, `check-doc-links.mjs`.
- Backlog 0296 CLOSED marker, and its row moves from `### Promoted` to `### Closed` in
  `docs/design-backlog-archive.md`.
- Recently-closed bullet and `README-archive.md` entry; `node scripts/toc.mjs`.
- **Patch** bump, the studio's two version copies, an annotated tag, and `check-release-tag.mjs`.
- No ADR to accept: ADR-0170 is already accepted, and the plan pairs no new ADR.

### At the close

- Minor 1 (the bloom row omits Tidepool): **fixed** in `ca927b43` with the review's replacement text.
- Minor 2 (`[0, 2]` still excludes Tidepool): **open**, the owner's call. It is a constant, which a
  close does not repair (ADR-0209). **The Decision's premise was false when the plan was approved:**
  `waterfall_tidepool.toml` had shipped `bloom_amount` at `2.2`–`3.1` since 2026-10-06, and the
  inventory of shipped values counted only constant assignments.
- Nit 1 (the TL;DR names `.taplo.toml`): **fixed** in `ca927b43`.
- No earlier round: round 1 was the only review.
- Backlog 0296 closed in the archive. Version 0.176.1 (patch). Upstream CI green (run 38146692208
  on `eaaa027`). Translation advisory: two rows, neither moved by this plan:
  `docs/how-it-works.ru.md` and `docs/running.ru.md`. Curation: no `.toml` changed; Tidepool is
  the one shipped preset outside the declared range, now named in the prose.

## Followups (after this lands)

- Owner call from minor 2: keep `[0, 2]` with Tidepool as the documented outlier, or widen the
  declaration to `[0, 3.2]`. Widening goes in as a new backlog entry, not a reopening of this plan.
