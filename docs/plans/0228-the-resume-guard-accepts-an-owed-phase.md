# 0228 — The resume guard accepts an owed phase

> **Status:** in-progress
> **Created:** 2026-09-24
> **Approved:** 2026-09-24 (user) — queued in lane b. Plan 0220 is parked on this and cannot clear.
> **Owner skill(s):** dev
> **Related ADRs:** [0249](../adrs/0249-a-human-phase-may-be-owed-after-the-merge.md),
> [0250](../adrs/0250-the-conductor-stays-up-and-resumes-what-the-repository-shows-settled.md),
> [0205](../adrs/0205-an-approved-plan-runs-under-a-conductor-and-every-judgement-it-cannot-make-parks-the-plan.md)
> **Closes:** none
> **No ADR.** [ADR-0249](../adrs/0249-a-human-phase-may-be-owed-after-the-merge.md) already decided
> that an owed phase is not waited for. This plan makes one guard agree with that decision, and the
> only alternative — asking the owner to write `done` against a phase nobody has done — is a false
> record rather than a design option.

## TL;DR

`parkStillTrue` clears a `human_phase` park only when the phase's log row reads `done`. A phase
marked `**Blocks merge:** no` gets the row `owed`, which that guard does not accept, so a plan that
parked **before** it was marked non-blocking can never be resumed. Plan 0220 is in exactly that
state now: six `dev` phases complete, unable to merge.

## Context & problem

Two functions disagree about what settles a human phase.

| Function | File | Accepts |
|---|---|---|
| `rowIsDone` | `tools/conductor/lib/plan.mjs:122` | `/^done\b/i`, or `committed with this row` |
| `rowIsOwed` | `tools/conductor/lib/plan.mjs:134` | `/^owed\b/i` |

`nextStep` reads **both**, so a plan whose last phase is `owed` proceeds to its review and merge.
`parkStillTrue` (`tools/conductor/lib/lane.mjs:122`) reads **only** `donePhases`, so it refuses the
resume with *"Phase N is still not marked done in the ## Implementation log"*.

**In the intended flow the two never meet**, which is why this shipped: a phase marked
`**Blocks merge:** no` is marked owed by `markOwed` *inside the implement loop* and never parks at
all. The gap opens only when a plan parks `human_phase` under the old rules and is marked
non-blocking afterwards — the retrofit case, which is every plan written before ADR-0249.

**Plan 0220 is the live instance and it is a deadlock.** Its Phase 7 begins *"push, then read CI on
the new tree"*, and a push requires the merge, so as a blocking phase it demanded something only
possible after the thing it prevented. It was marked `**Blocks merge:** no` (`45dc4f18`) and its row
set to `owed` (`294b5bd6`), which is byte-identical to what `markOwed` writes — and the resume is
still refused.

## Decision

`parkStillTrue` accepts an `owed` row as settling a `human_phase` or `claude_dir` park, in addition
to `done`. The phase's own `**Blocks merge:** no` marker is what makes `owed` legitimate, and
`nextStep` already treats the two the same way; this is the one place that does not.

## Implementation phases

### Phase 1 — The guard accepts owed
- **Owner skill:** dev
- **What:** `parkStillTrue`'s `human_phase` / `claude_dir` branch accepts a row that `rowIsOwed`
  matches, not only one `rowIsDone` matches.
- **Files touched:** `tools/conductor/lib/lane.mjs`, `tools/conductor/test/lane.test.mjs`.
- **Done when:**
  - A record parked `human_phase` whose plan marks that phase `**Blocks merge:** no` and whose log
    row reads `owed` resumes; the same record with the row `not started` still refuses, with the
    message unchanged.
  - A row reading `owed` for a phase **not** marked `**Blocks merge:** no` still refuses. An owed row
    is legitimate only because the marker says so, and the guard should not accept a bare `owed` as a
    way past a blocking phase — that is the hole this fix must not open.
  - `node --test tools/conductor/test/` passes, and the new cases fail against the current guard.
  - The comment at the guard states the mechanism: `nextStep` reads `donePhases` **and**
    `owedPhases`, so a guard reading only the first disagrees with the loop it protects.

### Phase 2 — The operator guide says what owed means at a resume
- **Owner skill:** dev
- **What:** one line where the park table already explains each reason.
- **Files touched:** `tools/conductor/README.md`.
- **Done when:** the `human_phase` row of the park table says a phase marked `**Blocks merge:** no`
  settles with an `owed` row and does not need `done`, and that the conductor writes that row itself
  when it reaches the phase. `node scripts/check-doc-links.mjs` is green.

## Risks & open questions

- **The second done-when is the one that matters.** Accepting any `owed` row would let a blocking
  human phase be skipped by writing one word, which is worse than the deadlock it repairs. The marker
  must be read from the phase, not inferred from the row.
- **0220 is the only known instance**, so the fix is verified against a record built in a test rather
  than against a live lane. Resuming 0220 afterwards is the real confirmation and is a Followup, not
  a phase — this plan does not close another plan's park.

## What this plan does NOT do

- **It does not change what `markOwed` writes** or when the implement loop calls it. The forward path
  is correct and shipped working.
- **It does not mark any plan's phase for it.** Deciding that a human phase is non-blocking stays a
  judgement about that plan.
- **It does not touch `rowIsDone` or `rowIsOwed`.** They are right; one caller was not.

## Implementation log

> Written by `dev` — one row per phase as that phase's commit lands, and the close block after the
> last one. **The phases above are the contract; everything here is what happened.**

**Lane:** `plan-0228-the-resume-guard-accepts-an-owed-phase` in `/home/igor/Work/rlx-plan-0228`

| phase | owner | state | commit |
|---|---|---|---|
| 1 — The guard accepts owed | dev | done | `0717f508` |
| 2 — The operator guide says what owed means at a resume | dev | done | `7bd1bc42` |

### Notes

- Phase 1 also changed `selfResumeWhy`'s settled phrase in `tools/conductor/lib/lane.mjs` from a
  fixed `reads done` to `reads done` or `reads owed`, whichever settled the park (`0717f508`).
- Phase 1's "fail against the current guard": the new test was run against the unchanged
  `lane.mjs` and failed on its first assertion (the owed, non-blocking case refused).
- Phase 2 edited only the park-table row. `tools/conductor/README.md` lines 166-167 (digest
  "Already settled") and 212-213 (the self-resume list) still name only `done`.

### Close triggers

- **`presets/` touched:** no
- **Plan header `Closes:`** none
- **What shipped:** fix-only (conductor resume guard)
- **Operator docs touched:** `tools/conductor/README.md`
- **Backlog probes (`node scripts/check-backlog-claims.mjs`):** exit 0 (46 reductions hold across
  21 live entries, 4 unprobeable)
- **Full suite:** owed to the conductor's pre-review gate (ADR-0207). `node --test tools/conductor/test/`:
  exit 0, 426 tests, 424 pass, 0 fail.
- **Outstanding `human` phases:** none

## Followups (after this lands)

- `resume 0220`, which should then clear and let it merge. It is the live instance this repairs.
- A sweep for other plans whose trailing `human` phase is only possible after the merge; 0220's
  Phase 7 was not written wrong so much as written before ADR-0249 existed.
