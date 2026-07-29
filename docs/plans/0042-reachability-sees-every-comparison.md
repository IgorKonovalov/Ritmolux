# 0042 — Reachability sees every comparison, and the library is re-audited against it

> **Status:** draft
> **Created:** 2026-07-29
> **Owner skill(s):** dev
> **Related ADRs:** [0043](../adrs/0043-reachability-reports-comparison-nodes.md) (this plan's
> decision), [0042](../adrs/0042-reachability-measured-on-the-expression-tree.md) (the mechanism it
> extends)
> **Closes backlog:** [0028](../design-backlog.md)

## TL;DR

`--report`'s reachability check reports `select()` and `clamp()` nodes only, so a bare comparison
(`reseed = "onset > 0.55"`) is invisible and a dead band gate `min`ed with a live `tempo` gate is
reported as the tempo gate and dismissed. Five of the nine real dead gates found in the 2026-07-29
library audit were invisible to the check that exists to find them. This plan records every
comparison operator as a two-valued observation, reports one that never went both ways unless a
`select()` already names it, and then re-audits the shipped library against the corrected
instrument.

## Context & problem

Plan 0041 shipped tree-measured reachability and it earned its place immediately — it named four
presets whose defining mechanism had never executed. The 2026-07-29 `preset-author` audit fixed
those four, then found **five more of the same defect by grepping every threshold in the library by
hand**, because the check had scored them clean.

Two shapes fall through, both traced to `core/src/preset/expr.rs`:

1. **`Node::probe`'s `Node::Bin` arm only recurses.** `record_select` and `record_clamp` are its
   only writers, and `NodeObservation` has no comparison variant — so a comparison outside a
   `select()` condition is never observed. `reseed = "onset > 0.55"` is the idiomatic boolean-param
   form and contains no `select()` at all; all five attractor presets shipped without ever
   reseeding, each scoring `gates 0`.
2. **`collect_flags` names a `select()`'s whole condition.** For
   `select(min(tempo > 124, bass + treb > 0.38), 4, 1)` it prints the `min(...)`, and the report's
   own guidance says a `tempo` gate is legitimately one-sided under a single-BPM probe — so the
   reader dismisses a flag whose *other* half is separately dead (`bass + treb` peaks near `0.138`).

The second is the damaging one: it reports **clean**, not unknown, on the instrument all three lanes
verify through.

Note this is not the reporting-only change the handoff guessed. `Node::Bin` records nothing today,
so the fix is instrumentation *and* reporting.

## Decision

Per [ADR-0043](../adrs/0043-reachability-reports-comparison-nodes.md): record the six comparison
operators as a two-valued observation exactly like a `select()` condition, and report one that never
took both values **except where it is the direct condition of an enclosing `select()`** — that
`select()` already reports it, and in better words.

We rejected reporting every comparison including select conditions (doubles today's 20 flags with
the strictly less useful half of each pair), replacing select reporting with comparison reporting
(loses the "its `then` branch never ran" phrasing, and drops `select(beat, a, b)` entirely), and a
static threshold-vs-band-range check with no probe (needs interval arithmetic over the whole
grammar — a second evaluator to keep in step).

Reachability **stays advisory**. ADR-0042 said gate once the library is clean; the honest reading is
that we do not yet know whether it is clean. This plan ends with the evidence, not the gate.

## Architecture diagram

```mermaid
flowchart TB
    subgraph core["core/src/preset/expr.rs"]
        direction TB
        P["Node::probe<br/>(records; never computes)"]
        OBS["NodeObservation<br/>Untouched | Select | Clamp | <b>Compare</b>"]
        CF["collect_flags<br/>emits GateFlag"]
        P -->|"record_select"| OBS
        P -->|"record_clamp"| OBS
        P -->|"<b>record_compare</b> (new)"| OBS
        OBS --> CF
    end
    subgraph rule["suppression rule (ADR-0043)"]
        R{"is this comparison the<br/>DIRECT condition of a select?"}
        R -->|yes| S["suppress — the select flag names it"]
        R -->|no| E["emit a compare flag"]
    end
    CF --> R
    S --> OUT["--report gate section"]
    E --> OUT
```

## Implementation phases

### Phase 1 — a comparison is observed

- **Owner skill:** dev
- **What:** `NodeObservation` gains a `Compare { saw_true, saw_false }` variant and `Observations` a
  `record_compare`; `Node::probe`'s `Node::Bin` arm records when — and only when — the operator is
  one of the six comparisons. Arithmetic `Bin` nodes keep recursing without recording.
- **Files touched:** `core/src/preset/expr.rs`
- **Done when:** a probed run of `"onset > 0.55"` against stimuli where `onset` never exceeds `0.55`
  leaves that node observed with `saw_true = false, saw_false = true`, and the same expression
  probed against stimuli that straddle `0.55` records both. An arithmetic node (`bass + mid`)
  records nothing, so the observation array is not populated for every node in the tree.
  `Expr::eval_probed` still returns exactly what `Expr::eval` returns across the whole embedded
  library (the existing `probed_evaluation_returns_exactly_what_eval_returns_across_the_library`
  test continues to pass unmodified — ADR-0042's no-divergence property is not weakened).

### Phase 2 — a comparison is reported, unless a `select()` already names it

- **Owner skill:** dev
- **What:** `collect_flags` gains a `Compare` arm and a `GateKind::Compare`; the recursion carries
  whether the child it is descending into is a `select()`'s direct condition, and suppresses a
  comparison flag exactly there. `shot`'s gate printer gains the wording for the new kind.
- **Files touched:** `core/src/preset/expr.rs`, `standalone/examples/shot.rs`
- **Done when:** `select(bass > 0.3, a, b)` yields exactly **one** flag, the existing select one —
  not two. `reseed = "onset > 0.55"` yields exactly one flag naming the comparison. And
  `select(min(tempo > 124, bass + treb > 0.38), 4, 1)` yields **two** flags naming
  `tempo > 124` and `bass + treb > 0.38` *separately* rather than one naming the `min(...)`, which
  is the case that reported clean before this plan. Each flagged comparison's `source` re-renders as
  text that parses back to the same tree (the property the existing
  `a_flagged_gate_is_named_in_source_that_compiles_back` test asserts, extended to the new kind).

### Phase 3 — re-audit the shipped library and record what is actually there

- **Owner skill:** dev
- **What:** run `--report` over the shipped set with the corrected check and record the result in
  this plan's Outcome section — total flags, how many are the standing `tempo` false positive, and
  how many are genuinely dead. No preset content is edited in this phase; the numbers are the
  deliverable, and they are what the later gating decision is taken on.
- **Files touched:** this plan (Outcome section only)
- **Done when:** the plan carries a measured before/after count from
  `cargo run -p standalone --example shot -- --presets presets --report`, with the genuinely-dead
  flags named per preset, and a one-line recommendation on whether the library is clean enough to
  revisit CI gating. Expect the raw count to **rise** — bare `tempo > N` comparisons now flag too —
  and say so explicitly rather than presenting a larger number as a regression.

## Data shapes

```rust
// illustrative — not the final interface
pub enum NodeObservation {
    Untouched,
    Select { saw_true: bool, saw_false: bool },
    Clamp  { peak_fraction_of_bound: f32 },
    /// A comparison operator that is not a `select()` condition. Same two-valued
    /// shape as `Select` — deliberately, so the reporting logic is shared.
    Compare { saw_true: bool, saw_false: bool },
}
```

## Risks & open questions

- **Noise before signal.** The gate section gets louder, not quieter, until the library is re-gained.
  That is expected and is why Phase 3 reports rather than gates — but if the count rises far enough
  to make the section unreadable, the follow-up is a `tempo`-aware presentation (grouping or
  demoting the known false positive), not a retreat from the check. Out of scope here.
- **Is `==` / `!=` on a float ever meaningfully two-valued?** A preset writing `bass == 0.5` is
  almost certainly a mistake, and it will now flag as one-sided forever. That is arguably the check
  working. Left as-is; if it proves noisy, exempting the two equality operators is a one-line change
  and a note in the ADR.

## What this plan does NOT do

- **It does not gate CI.** Deferred to a decision taken on Phase 3's evidence.
- **It does not edit preset content.** Nine dead gates were already fixed on 2026-07-29 (`e9a1c3c`);
  whatever Phase 3 surfaces is a separate `preset-author` pass.
- **It does not touch the stimulus levels.** `FULL_LEVELS` and `LOW_LEVELS` are unchanged, so every
  historical reactivity number keeps its meaning (ADR-0042).
- **It does not solve the `tempo` false positive.** That needs a multi-BPM probe or an exemption
  rule, and is the natural precondition for gating.
