# ADR-0250 — The conductor stays up, waits instead of stopping, and resumes what the repository shows settled

> **Status:** proposed
> **Date:** 2026-09-24
> **Related plan(s):** [0226](../plans/0226-the-conductor-stops-waiting-for-the-owner.md)
> **Amends:** [0205](0205-an-approved-plan-runs-under-a-conductor-and-every-judgement-it-cannot-make-parks-the-plan.md)
> (`resume` as the owner's act, and a lane that stops at the worktree cap),
> [0214](0214-the-digest-is-a-current-state-page-and-history-is-regenerated-on-demand.md)
> (*"`resume NNNN` clears the record, and it stays your explicit act"*),
> [0219](0219-the-conductor-can-be-asked-to-finish-and-stop-and-the-ask-does-not-outlive-the-run.md)
> (what ends a run)

## Context

A `run` today ends as soon as no lane can move: its queue has run out, every remaining plan is parked
or waits on a parked one, or a lane has hit `max_open_worktrees`, which stops the lane rather than
waiting (ADR-0205). After that nothing happens until the owner starts another run. And a park whose
cause has gone away stays parked until the owner types `resume`, even when the conductor can read
from the tree that it has gone away. `status` already prints that verdict ("already settled"), and
ADR-0214 deliberately leaves acting on it to the owner.

Between 2026-09-22 14:05 and 2026-09-24 08:03 the owner started **10 runs**. The longest lasted 2.4 h
and they totalled about 8 h, against 42 h elapsed. On 09-23, lane b stopped at the worktree cap
because three parked plans held its slots, and the owner raised the cap from 3 to 5 in `local.json`
to continue. Every `human_phase` park needed the owner to do the phase, mark the row, and then
**separately** run `resume` and `run`. That second step carries no judgement, because `resume` already
refuses when the row is not marked.

The explicit act had a reason: nothing should start while the owner is not looking, unless the owner
started it. That reason still holds for the start of a run. It does not hold for the individual
parks inside a run the owner started, where the conductor checks the same tree-read condition
`resume` itself checks.

## Decision

We will make `run` **resident**. It lasts until `pause`, `abort` or Ctrl+C, and `run --until-idle`
keeps today's end-when-nothing-can-move behaviour. `--once` is unchanged.

- **An idle lane waits and looks again.** A lane with nothing to start sleeps and re-reads, every
  60 s, `queue.json` from the main checkout, the plan files, and the state. A plan approved and
  queued while the run is up is picked up without a restart. The digest's **Now** says the lane is
  idle and watching.
- **The worktree cap is a wait, not a stop.** A lane that would exceed `max_open_worktrees` waits for
  a slot. The digest names the plans that hold the slots, as the stop does today.
- **A closed list of parks resumes itself when the tree settles them.** The list is:
  - `human_phase` and `claude_dir`, once the phase's `## Implementation log` row reads `done` in the
    lane, or in the main checkout when the lane is gone;
  - `usage_limit`, once its recorded reset has passed;
  - `main_dirty`, once the main checkout is on `main` and clean;
  - `studio_install`, retried once an hour, three times.

  These are the conditions `resume` already checks, read the same way. **A park whose worktree is
  dirty never resumes itself**, whatever its reason. **Every other reason stays the owner's**
  (`gate_red`, `review_failed`, `disagreement`, `plan_wrong`, `question`, `stop_condition`,
  `cli_contract`, `lost_background`, `budget`, `api`, `merge_*`), because none of them can be read as
  settled from the tree. Each self-resume writes an inbox entry and a run-terminal line naming the
  condition that settled it.
- **A resident run carries a spend ceiling of its own.** `local.json` gains a required
  `run_budget_usd`. A run that has spent it pauses, in ADR-0219's sense: it finishes the plans in
  flight, starts no other, and ends. The per-step `--max-budget-usd` is unchanged.
- **The CLI version is checked before every session, not once per run.** An update installed while
  the run is up would otherwise run sessions on a version the run never judged. An unverified version
  that is not a patch above a verified one pauses the run. A patch above one runs with the warning
  (ADR-0208).
- **A pause still does not outlive its run** (ADR-0219). It is now the ordinary way a resident run
  ends.

## Consequences

### Positive

- One `run` per working period, not one per park. A `human` phase done in the morning resumes by
  itself without the owner remembering its number.
- Parked plans no longer cost a lane at the worktree cap. The lane waits.
- A plan approved in a human-started architect session starts within a minute, with no second step.

### Negative

- **The machine spends while nobody is looking, for longer.** `run_budget_usd` bounds the total. It
  cannot bound a spend the owner would not have approved in the moment, because nobody is asked.
- **A resident process can die silently**, from a crash, a reboot or a closed terminal. Nothing
  restarts it, and the digest's **Now** then shows a stale "running". Supervising the process is out
  of scope; a run's record already shows its end time as missing, and `status` reads the pid.
- **Self-resume makes the inbox a log as well as a worklist.** An entry may now record something
  that has already been acted on, so the digest, not the inbox, remains the page to read first.
- **The 60 s poll re-reads the queue and every parked plan's file.** It is cheap at this repository's
  size, and it is still a background cost the one-shot run did not have.

## Alternatives considered

### Alternative A — Self-resume inside a run, then exit as now

This was the owner's second option in the interview. Rejected because it leaves the restart, which
was the step repeated ten times in 42 h, and the cap still stops a lane.

### Alternative B — A scheduler starts `run` every few minutes

It uses cron, a systemd timer or Task Scheduler, and each run is short-lived. Rejected because the
schedule is per-machine configuration on three platforms, outside the repository, and it adds nothing
the process's own poll does not. A resident process also keeps its in-memory held locks between
plans, which a restarted one does not.

### Alternative C — Self-resume every park whose record looks stale

For example, a `gate_red` whose lane gained a commit since the park. Rejected because a new commit is
not evidence that the red is fixed. Only the conditions `resume` can already verify from the tree
qualify, which is the same line ADR-0214 drew for "already settled".
