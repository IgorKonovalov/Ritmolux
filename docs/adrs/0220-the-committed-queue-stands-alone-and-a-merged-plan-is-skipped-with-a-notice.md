# ADR-0220 — The committed queue stands alone, and a merged plan is skipped with a notice

> **Status:** accepted
> **Date:** 2026-09-19
> **Related plan(s):** [0197](../plans/done/0197-the-conductor-becomes-operable.md)
> **Amends:** [0205](0205-an-approved-plan-runs-under-a-conductor-and-every-judgement-it-cannot-make-parks-the-plan.md)
> (the committed queue)

## Context

`tools/conductor/queue.json` is committed and accumulate-only: nothing removes a plan from a lane
list, and a plan that has merged stays listed. Two mechanisms keep that harmless and they do not
agree about what they read.

`merged(ctx, plan)` in `lib/lane.mjs` skips a merged plan when picking the next one, and asks two
sources — the plan's status in `state/conductor.json`, **or** `findPlan(ctx.repo, plan)?.done ===
true`, which needs no state at all because the file being under `docs/plans/done/` is enough.
`validateQueue` runs first and has only the `mergedPlans` set its caller builds out of
`state/conductor.json`. It has no `done/` fallback of its own, and a listed plan it finds under
`done/` is a **fatal** preflight error.

`tools/conductor/state/` is gitignored, so it never travels. A clone, a wiped `state/` or a second
machine therefore turns every merged plan still listed into a refusal to start the run at all —
demonstrated against this repository on 2026-09-16, with `0190` closed and `0191` approved:
`validateQueue` with the real state returns no errors, and the same call with empty sets returns
*"plan 0190: already closed"*. The queue is portable only as long as a gitignored directory travels
with it, which it never does.

The reason this has never fired is not a guard. The five plans merged before 2026-09-16 were pruned
by hand when both lanes filled, so the committed queue happens to hold no merged plan — and nothing
records that as something anyone must keep doing.

There is a real check inside the fatal path, and it is the reason this is a decision rather than an
edit: queueing a plan that was closed by hand is a typo, and today's error is what catches it.

## Decision

We will give `validateQueue` the `done/` fallback `merged()` already has, so a plan listed in the
queue and found under `docs/plans/done/` is **skipped rather than fatal**, whatever the state says —
and we will keep the check it was carrying by **reporting the skip as a notice** that names the plan
and the file, rather than by failing the run. Separately, `conductor.mjs prune` rewrites
`queue.json` with every merged plan dropped and prints what it removed, so the accumulate-only file
has a carrier rather than a habit. The queue then stands alone: a clone can start a run.

## Consequences

### Positive
- A clone, a second machine or a wiped `state/` can run the conductor. The committed file means what
  it says without a gitignored directory beside it.
- The two predicates agree. `merged()` and `validateQueue` read the same two sources in the same
  order, and the class of bug where one tolerates what the other refuses is closed.
- Pruning becomes a command anyone can run and a line anyone can read, instead of an unwritten
  discipline whose only evidence is that the file happens to be tidy.

### Negative
- **A typo demotes from fatal to a notice.** Queueing `0190` when `0191` was meant now prints a line
  and continues with the rest of the lane rather than stopping. The notice is the whole of the
  mitigation, and a notice in a long run is read less often than a refusal.
- **One more command on a tool whose surface is already eight.** `prune` is the second command whose
  only job is bookkeeping, and it edits a committed file, so a careless run is a diff to review.
- The fallback costs a filesystem lookup per queued plan at preflight, which is nothing, and makes
  the validator depend on the repository layout, which is a coupling it did not have.

### Neutral
- Nothing changes for a run on this machine today: the state is intact and the queue is already
  pruned, so the fallback is inert until the day it is not.

## Alternatives considered

### Alternative A — Leave it fatal, and prune by ceremony
Add a step to the close: drop merged plans from the lane list beside the roster refresh. Keeps the
typo check exactly as it is and costs no code. Rejected on this project's own record: the
backlog-archive step went undone through three sweeps — 2026-08-04, 2026-08-13, and again hours
later the same day — until a carrier existed for it. A rule whose only enforcement is a ceremony
someone remembers is the shape that has already failed here three times.

### Alternative B — `prune` alone, without the fallback
The command keeps the file tidy, so the fatal path is never reached in practice. Rejected because it
leaves the portability defect exactly where it is: the queue still only validates against a
gitignored directory, and the first clone still refuses to start — the failure this decision exists
to remove.

### Alternative C — Make the queue read the state's merged set only, and commit the state
Removes the disagreement by making the gitignored file travel. Rejected because `state/` holds run
records, transcripts, gate logs and the suite ledger — machine-local by construction, and committing
it would put a growing, conflict-prone directory into every lane's merge.

## Notes

Raised as backlog 0240, 2026-09-16, by the owner asking when a plan leaves the queue, during the
first two-lane run.
