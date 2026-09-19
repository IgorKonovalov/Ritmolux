# ADR-0218 — A lane makes its plan's preconditions true, and a skipped check says so

> **Status:** proposed
> **Date:** 2026-09-19
> **Related plan(s):** [0196](../plans/0196-the-gate-roster-stops-drifting.md)
> **Rests on:** [0016](0016-gpu-tests-opt-in-ci-scope.md) (skip, but say so),
> [0205](0205-an-approved-plan-runs-under-a-conductor-and-every-judgement-it-cannot-make-parks-the-plan.md)
> (the conductor's gate), [0210](0210-a-claude-repair-is-the-owners-and-a-session-that-needs-one-parks-with-the-edit.md)
> (the park that refuses in front of the work)

## Context

`runGate` drops a step whose `onlyIf` path is missing with a bare `continue`, before anything is
recorded: the step is not in `ran`, not in `timed`, and reaches neither the command terminal nor the
run's log. The only signal a reader gets is the step count in `gate <stage> checks ok (16, 1m41s)`,
and reading it requires knowing that nineteen was the number to expect.

Three of the conductor's steps are guarded that way, on `studio/node_modules`. That directory is
gitignored, so `git worktree add` never creates it and **no conductor lane has ever had one** —
verified on the live `plan-0191` lane on 2026-09-16. Every lane has therefore run its gate with the
studio's typecheck, lint and tests skipped, always and silently, at both `pre-review` and
`post-close`. `onlyIfCommand` is the same class: the sd-filter suite skips when `python3` is absent,
equally silently, and is live only because this machine happens to carry 3.11.9.

One layer up, the repository already answered this. `.githooks/pre-push` guards on the same
directory and prints *"pre-push: skipping the studio's typecheck, lint and tests (no
studio/node_modules; run: npm --prefix studio ci)"* — [ADR-0016](0016-gpu-tests-opt-in-ci-scope.md)'s
shape: skip, but say so, and name the command that would un-skip it. The gate does the same skip and
says nothing.

What makes this worth a decision rather than a one-line repair is the case where the skip is not
harmless. Plan 0179's Phase 5 was the first `studio-builder` phase ever handed to the conductor: four
`dev` phases, then a session editing `studio/`, then a gate that ran **zero** studio checks between
that work and the push. CI has a studio job and would convict after the push, which is exactly the
position the conductor exists to get ahead of. A notice alone would have made that hole legible and
left it open.

## Decision

We will do two things, and they answer different halves. **A gate step whose precondition is missing
is reported rather than silently dropped** — through the same command terminal the gate already has,
naming the step and the command that would make it run, in ADR-0016's shape, and recorded in the
gate's result so a reader of `state/gates/` sees it too. **And a lane makes its plan's precondition
true at open:** when the plan's declared files include `studio/`, the conductor runs
`npm --prefix <lane>/studio ci` once as part of opening the worktree, so the three studio checks are
real for that lane. An install that fails parks the plan under a conductor-owned reason with the
install's tail as the detail, rather than proceeding with checks that cannot run.

## Consequences

### Positive
- The conductor's gate becomes as strong as CI's for the one lane class where it was weakest, and it
  applies before the push instead of after it.
- A skip stops being invisible. The `python3` case, the studio case and any future `onlyIf` all
  report themselves, so the step count is no longer the only evidence.
- The failure moves to the front. A lane that cannot run its plan's checks parks at open, where the
  cost is one park, rather than after four phases of work.

### Negative
- **An npm install joins worktree creation.** It needs the network, takes minutes on a cold cache,
  and `node_modules` is by a wide margin the largest thing a lane will hold — against
  [ADR-0053](0053-plan-lanes-run-in-git-worktrees.md)'s already-live disk cost, which
  [ADR-0147](0147-the-shared-artifact-store-is-revoked-and-the-linker-stays.md) put back per lane.
- **A lane opened offline for a studio plan now parks where it used to run.** That is the decision
  working as intended and it is still a stop the operator did not have before.
- The trigger is the plan's **declared** files, so it inherits backlog 0236's weakness: a phase that
  edits `studio/` without naming it gets a lane with no install and the old silent skip — now at
  least announced.

### Neutral
- Nothing changes for a plan that does not touch `studio/`, which is every plan the conductor has run
  but one. The install is conditional, not unconditional.

## Alternatives considered

### Alternative A — Report the skip and stop there
The cheapest shape, and it decides nothing: it makes the hole visible without closing it. A studio
phase would still reach the push with no studio check between it and CI, which is the finding rather
than a fix for it. Rejected as half of this decision rather than as an option — the report is kept,
the stopping there is not.

### Alternative B — Refuse a studio plan outright, as ADR-0210 refuses a `.claude/` phase
A precondition the preflight checks, parking the plan before it starts. Rejected because the two
cases are not alike: the CLI's refusal to edit `.claude/` is a wall nothing in this repository can
move, while a missing `node_modules` is a command away. Refusing would take the studio out of the
conductor entirely in order to avoid a gap that an install closes.

### Alternative C — Commit or vendor the dependencies
Puts `node_modules` in git so every worktree has one. Rejected on its face: a few hundred megabytes
of third-party tree in the repository, to make one gate step runnable.

## Notes

Raised as backlog 0242 (2026-09-16), reading what lane `b` would do after Plan 0191 closed. Its
priority note called the mitigation worth doing by hand before the decision was made, which is what
happened for Plan 0179; this ADR is what stops that being a thing anyone has to remember.
