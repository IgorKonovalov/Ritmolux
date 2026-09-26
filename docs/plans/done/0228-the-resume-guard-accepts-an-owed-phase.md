# 0228 — The resume guard accepts an owed phase

> **Status:** done - closed 2026-09-24 by a conductor-run close. Phase 1 `0717f508`, Phase 2
> `7bd1bc42`. Round 1 review: no blockers, no majors, two minors and one nit; the README minor and
> the nit repaired at the close, the digest minor left open for `dev`. Full suite green at the tagged
> tip. Version: none (conductor tooling, nothing shipped changes).
> **Created:** 2026-09-24
> **Approved:** 2026-09-24 (user) — queued in lane b. Plan 0220 is parked on this and cannot clear.
> **Owner skill(s):** dev
> **Related ADRs:** [0249](../../adrs/0249-a-human-phase-may-be-owed-after-the-merge.md),
> [0250](../../adrs/0250-the-conductor-stays-up-and-resumes-what-the-repository-shows-settled.md),
> [0205](../../adrs/0205-an-approved-plan-runs-under-a-conductor-and-every-judgement-it-cannot-make-parks-the-plan.md)
> **Closes:** none
> **No ADR.** [ADR-0249](../../adrs/0249-a-human-phase-may-be-owed-after-the-merge.md) already decided
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

## Close review

The round 1 review, graded at `28fa4c3c` and reproduced in full. No earlier round raised a finding,
so there is no fix-round line to add. No phase is `owed`.

**Close notes.** The README minor is repaired in `ea94a754` (`tools/conductor/README.md`, the
self-resume list). The nit is repaired by this section: **the Decision's claim that `parkStillTrue`
is "the one place" that reads `done` alone is false of the tree it shipped to - `settledPark` in
`tools/conductor/lib/digest.mjs` is a second reader the plan missed**, and the Decision stays as
written, as the dated record. The digest minor is code and stays open for `dev`: until it lands, the
digest and `status` list a `human_phase` park settled by an `owed` row as live. Version: none, on the
conductor-only precedent of 0191 and 0226. Backlog probes: exit 0. Translation advisory: three
stale rows (`how-it-works`, `running`, the foobar `READ-ME-FIRST`), none moved by this plan.

### Plan 0228 — close review, round 1

Graded at `28fa4c3c1d4463897491c0314081583562ca2014` on `plan-0228-the-resume-guard-accepts-an-owed-phase`
(lane `/home/igor/Work/rlx-plan-0228`, `main` already merged in by the conductor).

**Verdict: Plan 0228 landed as written. No blockers, no majors, two minors, one nit.** The guard now
accepts an `owed` row only on a human phase the plan marks `Blocks merge: no`, and refuses a bare
`owed` row, which was the hole the plan warned against. Every done-when holds. The findings cover two
readers the plan did not name: the digest/`status` "Already settled" check still reads only `done`,
and one README sentence says the same.

#### Evidence

- **Full suite:** `node "/home/igor/Work/Ritmolux/tools/conductor/with-lock.mjs" suite -- cargo nextest run --workspace`:
  ran here in the foreground, not skipped from the ledger. It waited 319.2 s for the lock (held by
  `0227-fix-1`) and held it 511.4 s. `Summary [ 510.780s] 1805 tests run: 1805 passed (5 slow), 7 skipped`,
  exit 0.
- **Rustdoc:** `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`: green.
- **Conductor tests:** `node --test tools/conductor/test/`. The first run exited 1. Its output was
  truncated by the harness, so the failing test is not recorded. Two reruns right after
  (`--test-reporter=dot`) ran all 426 tests green on the same tree, with nothing changed. The
  suite-lock holder at the time was another plan's gate (`0227-fix-1`), so a timing-sensitive CLI test
  under load is the likely cause. That is not proven, and this review raises no finding for it.
- **Doc links:** `node scripts/check-doc-links.mjs`: OK, 566 files.

#### Lens 1 — alignment with the plan

- Both phases landed, one commit each (`0717f508`, `7bd1bc42`), and each has one in-vocabulary
  `**Owner skill:** dev` tag. The log is shorter than the phases section.
- **Phase 1 done-when 1** (owed + `no` resumes; `not started` refuses with the message unchanged):
  `tools/conductor/test/lane.test.mjs:1356`. It asserts `parkStillTrue(...) === null` for an owed row
  on a `no` phase, and an anchored regex match on the unchanged refusal message for `not started`.
- **Done-when 2** (a bare `owed` on a blocking phase refuses): the same test asserts a refusal both
  with no `Blocks merge` field and with `Blocks merge: yes`. `settledAs`
  (`tools/conductor/lib/lane.mjs:108`) reads the marker from the phase through `nonBlocking`, and
  `nonBlocking` also requires `owner === "human"`. A `claude_dir` park, whose phase is `dev`- or
  `studio-builder`-owned, therefore never settles on an `owed` row, which is the right answer.
- **Done-when 3** (the new cases fail against the old guard): the log states this, and the code
  confirms it. The old guard read only `donePhases`, so the test's first assertion cannot pass
  against it.
- **Done-when 4** (the comment states the mechanism): `lane.mjs:125-131` names `nextStep` reading
  `donePhases` and `owedPhases`, and why the marker is read from the phase.
- **Phase 2**: the `human_phase` row of the park table (`tools/conductor/README.md:221`) says a
  `Blocks merge: no` phase settles with `owed`, and that the conductor writes that row itself.
- **An addition, noted:** `selfResumeWhy` now reports `reads done` or `reads owed`, and the test
  covers the `owed` wording. It is in scope.
- **The plan's Decision is not true of the tree.** It says `parkStillTrue` is "the one place that
  does not" treat `done` and `owed` the same way. `settledPark` in `digest.mjs` is a second one. See
  the first minor.

#### Lens 2 — layering, coupling, real-time safety

Not engaged. The plan changes only the conductor's Node code, with no Rust, no C ABI and no control
protocol. `settledAs` reuses `plan.mjs`'s exported predicates and adds none of its own.

#### Lens 3 — docs and bookkeeping

- The `human_phase` park-table row is updated. Line 213 of the same README, the self-resume
  paragraph, still says the park resumes "once the phase's log row reads `done`". The dev log flagged
  this and left it. See the second minor.
- Lines 166-167 (digest "Already settled") name only `done`. That is **accurate for the code as it
  stands**, because `settledPark` still reads only `done`. It is part of the first minor, not a
  separate one.
- **Version bump:** fix-only conductor tooling. Nothing shipped changes, and whether a conductor fix
  earns a patch bump is the close's call under ADR-0005. Recent conductor-only closes are the
  precedent to follow.
- No ADR is paired. `Closes: none`, so there is no backlog step. No `presets/` changes.

#### Lens 4 — correctness

- `settledAs` handles a phase id missing from `plan.phases`: `find` returns `undefined`, and
  `nonBlocking` uses `phase?.owner`, so the result is `null` and the park refuses.
- `selfResumeWhy` calls `findPlan(...)` again and dereferences `found.path` without a null check.
  That is safe only because `parkStillTrue` returned null just before, which requires `found`. A
  file deleted between the two calls would throw, which is a negligible race.
- `nextStep` (`plan.mjs:185`) treats **any** `owed` row as settled, including one on a blocking
  phase. The loop therefore still does not check the marker, and `close.mjs:216` catches it at close.
  The plan kept that out of scope ("does not touch rowIsOwed", "does not change the forward path"),
  and the guard added here is stricter than the loop, which is the safe direction. This review
  records it and raises no finding.

#### Lens 5 — design integrity

Three readers now answer "is this owner phase settled?": `parkStillTrue`/`selfResumeWhy` in
`lane.mjs`, `settledPark` in `digest.mjs`, and `nextStep` in `plan.mjs`. Each answers differently.
`settledAs` is the right shape for one shared predicate, but it is private to `lane.mjs`. The first
minor's fix is to export it, or move it to `plan.mjs`, and have `settledPark` call it.

#### Findings

##### minor — the digest and `status` still call an owed-settled park live
`tools/conductor/lib/digest.mjs:199`. `settledPark` returns non-null only when `donePhases` has the
phase. `status` (`conductor.mjs:327`) and the digest's "Already settled" list both call it. After
this plan, a `human_phase` park whose phase is marked `Blocks merge: no` with an `owed` row is
settled for `resume` and for the self-resume. The digest and `status` still list it as a live park
that needs the owner. Plan 0220, the case this plan exists to repair, shows up that way until the
owner tries `resume` or a live run self-resumes it. The README's promise (lines 161-168) that
the digest shows what the repository shows as settled no longer holds for this case.
**Fix (code, `dev`):** move `settledAs` to `plan.mjs` as an export, for example
`settledPhase(plan, id)`. Call it from `lane.mjs` and from `settledPark`, and return
``Phase ${phase} now reads `${how}` in the plan's `## Implementation log` ``. Add a `digest.test.mjs`
case beside line 506, and change README lines 166-167 from "now marks `done`" to "now marks `done`,
or `owed` on a phase marked `Blocks merge: no`".

##### minor — the self-resume paragraph still says `done` only
`tools/conductor/README.md:213`. The text reads "`human_phase` and `claude_dir` once the phase's
log row reads `done`". After Phase 1 a `human_phase` park also self-resumes on an `owed` row when
the phase is marked `Blocks merge: no`. **Fix (prose, close-repairable):** replace it with
"`human_phase` and `claude_dir` once the phase's log row reads `done`, or `owed` on a human phase
marked `Blocks merge: no`,".

##### nit — the plan's Decision names one disagreeing reader where there are two
`docs/plans/0228-the-resume-guard-accepts-an-owed-phase.md:51`. It says "`nextStep` already treats
the two the same way; this is the one place that does not". `settledPark` also does not, as in the
first minor. The plan is a dated record. **Fix (prose, close-repairable):** a note in `## Close review`
records that `settledPark` was a second reader the plan missed. The Decision stays as written.

## Followups (after this lands)

- `resume 0220`, which should then clear and let it merge. It is the live instance this repairs.
- A sweep for other plans whose trailing `human` phase is only possible after the merge; 0220's
  Phase 7 was not written wrong so much as written before ADR-0249 existed.
