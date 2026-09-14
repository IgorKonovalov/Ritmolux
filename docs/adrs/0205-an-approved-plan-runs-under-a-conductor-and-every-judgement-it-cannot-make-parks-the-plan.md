# ADR-0205 — An approved plan runs under a conductor, and every judgement it cannot make parks the plan

> **Status:** accepted 2026-09-14 (Plan 0187), with an Outcome
> **Date:** 2026-09-14
> **Related plan(s):** [0187](../plans/done/0187-the-conductor-runs-the-lanes.md)
> **Supersedes in part:** [0188](0188-the-two-implementer-lanes-hand-off-automatically.md) (its
> "every other seam stays manual" and its rejected Alternative D, for conductor-run plans only)
> **Amends:** [0053](0053-plan-lanes-run-in-git-worktrees.md) (who performs a lane's fast-forward
> and removal), [0120](0120-the-close-brief-is-a-section-of-the-plan.md) (a session also ends on a
> machine-readable outcome)
> **Extends:** [0156](0156-the-per-phase-gate-is-scoped-and-the-suite-is-owed-once-per-plan.md)
> (the full suite is owed once per plan), [0193](0193-a-test-that-reads-the-clock-runs-alone.md)
> (a clock-reading test runs alone)

## Context

Every plan this project runs costs the owner four interventions that carry no judgement of their
own: opening a worktree lane, typing "go" to a `dev` session whose plan they already approved,
starting a fresh `/architect` session when `dev` prints its pointer, and fast-forwarding `main` and
removing the lane when the close finishes. On 2026-09-14 eleven plans were approved in one sitting;
at that cadence the interventions, not the work, set the pace.

The manual seams were each argued for, and the arguments are worth separating from the mechanism
that happened to carry them:

- **The "go" is an approval** ([ADR-0188](0188-the-two-implementer-lanes-hand-off-automatically.md)).
  True — but the approval the "go" records already happened, at the plan's own `Status: approved`.
  The second confirmation guards against a mis-scoped *session*, and that risk is carried equally
  well by a session that is handed an exact phase range and refuses to leave it.
- **The close review is worthless from inside the session that wrote the code**
  ([ADR-0120](0120-the-close-brief-is-a-section-of-the-plan.md)). True, and it is a property of the
  **context**, not of the human who opens the session. A reviewer started as a separate process,
  given only the plan path and the lane, holds no implementation reasoning — which is exactly what a
  fresh manual session holds. What ADR-0188 forbids, invoking `/architect` through the Skill tool, is
  forbidden because that call inherits the implementer's window; a new process does not.
- **A `human` phase has no one to hand to.** True, and unchanged.

Two capabilities make a driver buildable on this machine, verified against the installed CLI
(2.1.270) on 2026-09-14: `claude -p` runs headless with `--output-format stream-json`,
`--max-budget-usd`, `--permission-mode` and a tool allowlist, and it still loads project skills,
`CLAUDE.md` and the `PreToolUse` deny hooks. And an agent started inside the harness's own worktree
isolation is refused any git operation on the main checkout, so the merge cannot be an agent's step
in any case.

## Decision

**A conductor — a Node program under `tools/conductor/`, never shipped — runs approved plans in up to
two worktree lanes and merges each finished plan to `main`.** It never pushes. For each plan it opens
the lane, starts one fresh headless session per contiguous run of same-owner phases, verifies the
result itself, starts a fresh headless `architect` session to review and close the plan on the
branch, fast-forwards `main`, and removes the lane.

**Approval is the "go", for a plan the conductor runs.** A plan becomes eligible when its `Status:`
reads `approved` and it is listed in the conductor's queue. A session started by the conductor is told
the exact phase range it owns, treats that as its "go", and stops at the range's end. Outside the
conductor every seam of ADR-0188 is unchanged: a human-started `dev` still restates and waits.

**The close review is a separate process, and that is what "fresh session" means from now on.** It is
started with the plan path and the lane and nothing else; the conductor never passes it an
implementer's transcript. It writes its review to a file the conductor names, ends on a
machine-readable verdict, and performs the close bookkeeping only when the verdict has **no blockers
and no majors**. Blockers and majors go to a fresh `dev` fix session carrying the review file, then to
a fresh re-review; after **two** fix rounds that still fail, the plan parks.

**Every judgement the conductor cannot make parks the plan, and a parked plan never blocks its lane.**
The park triggers are closed: a `human` phase; a stop condition the plan itself states; a session that
finds the plan wrong or needs a question answered; a red gate the conductor ran; a review still
failing after two fix rounds; a spend cap hit; a merge that does not fast-forward cleanly after one
automatic re-merge; any cross-check where the session's outcome and the repository disagree. A parked
plan keeps its worktree and branch, writes an inbox entry, and the lane moves to the next queued plan
whose dependencies have merged. `resume` picks a parked plan back up after the owner acts.

**The conductor believes the repository, not the session.** A session's outcome block says what it
claims; the conductor checks the claim against `git` — the commits exist, the plan's
`## Implementation log` rows match, the tree is clean, the close moved the plan to `done/`, the tag is
annotated and sits on the branch tip — and a disagreement parks.

**Two machine-wide locks serialize what two lanes must not do at once.** A **close lock**, taken before
each review session starts and held until `main` has fast-forwarded, so a version bump and its tag
always land on the `main` they were computed against; a review that returns blockers or majors
releases it, so one lane's fix round never holds the other lane's close. A **suite lock**, taken around every `nextest`
invocation in a conductor-run session, because two lanes running GPU suites concurrently is exactly
the load under which ADR-0193's clock-reading tests failed alone-passing; within a run ADR-0193 still
isolates them, and across runs the lock does. A `PreToolUse` hook active only in conductor-run
sessions refuses a `nextest` call that does not go through the lock.

**The push stays the owner's, and it is the last human checkpoint.** The conductor writes to `main`
and tags; nothing reaches `origin` until the owner has read the close notes and pushed. A hook that
denies `git push`, `reset --hard`, `rebase` and `commit --amend` is added for every session, not only
conductor-run ones, turning a `CLAUDE.md` rule into a mechanism like the two deny hooks beside it.

**What happened while nobody watched is readable afterwards, in two records with different
lifetimes.** The **review** is committed: a conductor-run close adds a `## Close review` section to
the plan — the final review in full, plus one line per finding an earlier round raised and a fix
resolved — so the evidence of what the reviewer checked travels with the plan into `done/`. The
**digest**, `tools/conductor/digest.md` in the main checkout and gitignored, is regenerated from the
conductor's state and `git` after every step: what needs the owner first (parks with their resume
command, merges carrying minors), then each close with its verdict, fix rounds, tag, time and spend
and every finding as one line, then failures, then totals. It is the one record of parks, spend and
lock waits, none of which reach a commit, and it is kept out of git because nothing else reads it.
Its finding lines copy the verdict the reviewer emitted; the digest never paraphrases a review.

**Spend is capped per step.** Every session the conductor starts carries `--max-budget-usd` from a
machine-local configuration the conductor refuses to run without; hitting the cap parks the plan with
the spend recorded. The figure is the owner's and is never committed.

## Consequences

### Positive

- **An approved plan with no `human` phase goes from queue to `main` with no owner action.** On the
  2026-09-14 roster that is 0175, 0177, 0179, 0180, 0181, 0182 and 0185 end to end, and the dev
  phases of every other plan up to its human stop.
- **The review keeps its independence, and gains a property the manual seam never had**: the
  conductor cannot hand the reviewer a summary, because it has none to hand. The manual close was
  only as fresh as the owner's discipline about pasting.
- **Each same-owner run gets its own process**, so ADR-0188's accepted negative — `studio-builder`
  inheriting `dev`'s Rust diff in its window — disappears for conductor-run plans.
- **Close ordering stops being a race.** Plan 0099's twice-refused fast-forward and the stranded tag
  it needed are what the close lock removes.

### Negative

- **The owner no longer reads a verdict before it is acted on.** A clean review closes and merges
  unread. The mitigation is structural rather than procedural — only a verdict with no blockers and no
  majors acts, the review file and the close notes stay on disk, and nothing leaves the machine until
  the owner pushes — but a close the owner would have challenged can reach `main` first. Reverting it
  is a new commit, never a rewrite.
- **A fix loop is a model grading a model twice.** Two rounds is a cap, not a guarantee; a reviewer
  that is wrong the same way twice lands its error. This is the same risk as the manual close, run
  without the owner's glance at the verdict line.
- **Parks will be frequent.** Five of the eleven plans drafted on 2026-09-14 carry a `human` phase, and a
  red gate or an ambiguity parks rather than improvising. A lane that parks more than it merges costs
  a worktree each time: ADR-0053's disk cost returns per parked plan, so the conductor caps open
  worktrees and refuses to open a lane past it.
- **The suite lock serializes the slowest step.** Two lanes' `-P fast` runs, ~400 s each since Plan
  0174 Phase 1, queue behind each other. Two lanes stop being twice one lane.
- **A third kind of repository tool, with its own tests.** `tools/conductor/` is neither a gate nor a
  renderer; it is a program that starts other programs and spends money, and it drifts with the CLI it
  drives. The conductor checks the CLI version at start and refuses one it has not been verified on.
- **Three skills gain a conductor mode**, and a mode is a second behaviour each skill must keep true.

## Alternatives considered

### Alternative A — Keep every seam manual (ADR-0188 as written)
Rejected on the evidence of the roster: the interventions carry no judgement the plan approval did not
already carry, and at eleven approved plans they set the pace.

### Alternative B — An in-session Workflow script
Native, and quickest to write. Rejected because a workflow ends when the Claude Code session that
started it ends, and resume is same-session only, so a queue that spans days and parks for a human
would need the owner to keep a session alive — the intervention moved, not removed.

### Alternative C — The Claude Agent SDK as the driver
A typed session API and a per-call permission callback. Rejected for now because it adds a package
to a repository that counts every dependency, and the same control is reachable through `claude -p`,
a settings file and the existing hook mechanism. Worth revisiting if stream parsing becomes the
conductor's dominant maintenance cost.

### Alternative D — Merge on no blockers, and let majors land
Faster, and the owner's choice on 2026-09-14 was the opposite. A major is by this project's review
definition a finding that erodes the architecture; landing it unread is the failure the fresh review
exists to prevent.

### Alternative E — Stop the whole lane at a human phase
Simpler, and it never builds on a parked plan. Rejected because with five of eleven plans carrying a
human phase a lane would idle more than it ran. The dependency list in the queue is what keeps a lane
from building on a plan that has not merged.

### Alternative F — Agent-tool subagents with worktree isolation
Rejected on two facts: a worktree-isolated agent is refused git on the main checkout, so it cannot
merge, and a subagent lives inside the session that spawned it, which is Alternative B's problem.

### Alternative G — Let the conductor run `human` phases' mechanical parts
Rejected. A `human` tag marks something only the owner can do — a rig session, a judgement of a
picture, a push of tags to origin — and the conductor has no way to tell the mechanical part from the
judgement.

## Notes

- **What stays manual, restated for conductor-run plans:** approving a plan, every `human` phase,
  every park, and every push. What stays manual for every other plan: everything ADR-0188 lists.
- **The `preset-author` lane is not conducted.** Its seam into `architect` is a routing judgement
  (ADR-0081), and nothing here changes it.

## Outcome

**2026-09-14, at Plan 0187's close.** Two things the Decision did not say, and one it could not yet.

- **"Verifies the result itself" had a gap at the close, and the conductor now gates the close tip.**
  As built, the conductor's gate ran before the review, and the fast-forward treated the tip the
  close session left as already gated. That session merges `main`, bumps and tags, so the tree that
  reached `main` - on two lanes, the first tree holding both lanes' code - was verified only by the
  session's own claim, which is the one thing this decision says the conductor never takes. Fixed at
  the close: the fast-forward compares against the tip the conductor's own gate last passed on, so
  every close tip is gated before `main` moves. The price is one more full gate per close.
- **The close review of the conductor was not run by the conductor.** Plan 0187 was built on `main`
  in human-started sessions and closed by a human-started `architect` session, which also wrote the
  fix above on the owner's authorization. The fix has had no review from a session that did not write
  it.
- **The pilot had not run when this was accepted.** Plan 0187 closed without its Phase 6 by the
  owner's call, to be tested live. So nothing above rests on a real conductor-run plan: the headless
  contract is observed (the spike's evidence table), the state machine is tested against a fake CLI,
  and the second lane stays disabled in `queue.json`. The pilot's wall time, spend and parks are
  owed to a fresh `architect` session, which records them here and decides lane b.

**2026-09-14, the first live run: lane a, 17:26 to 18:39.** Recorded from the digest, the inbox and
`state/conductor.json` by a fresh `architect` session. This run finished after Plan 0187 closed.

- **What ran.** Queue `a: 0175, 0185, 0181, 0180, 0182`, `max_open_worktrees = 3`, one lane. Result:
  1 h 13 min, **0 merged, 3 parked, $25.18 notional**, spent entirely by three implement sessions:
  0175 in 24 min for $5.83, 0185 in 19 min for $3.71, 0180 in 31 min for $15.64. Both lock waits were
  under a minute, as one lane would give. **No review session, fix round, close, fast-forward or
  worktree removal ran.** So the close lock, the close-tip gate added in the Outcome above, the review
  session's `--add-dir`, and the allowlist a review or close session needs have still never been
  observed live. The one fix nobody else reviewed is still unread.
- **Two parks were the Decision working.** 0175 and 0180 each parked `plan_wrong` on a real defect in
  its own plan, written into the plan's log, instead of working around it. That is the first live
  evidence against Plan 0187's "a session that improvises instead of parking" risk. It is two cases,
  not a rate.
- **One park and one silent stop were conductor defects**, each confirmed against the code:
  1. `conductor.mjs` writes `state/conductor.pid` before anything creates `state/`, so the first
     `run` on a clean checkout fails with `ENOENT`. The CLI tests never hit it because their fixture
     creates the directory first.
  2. The pre-review gate runs `check-backlog-claims.mjs`. A plan whose fix deliberately breaks a
     probe of the entry it `Closes` then parks as `gate_red`, before the review whose close would
     archive that entry. 0185 parked this way on backlog 0206, and its own Risks section predicted it.
     The gate was making the call ADR-0108 leaves to `architect`, before `architect` could make it.
  3. The session allowlist refuses `git checkout`, `git restore` and `git stash`. The `dev` skill's
     conductor mode tells a parking session to leave the tree clean, but the allowlist leaves it no
     way to undo files its own test run rewrote. 0180's lane is parked with 8 golden PNGs a bless
     re-encoded. A park does not check the tree, and `resume` would start the next session on it.
  4. The lane stops without saying so at `max_open_worktrees`. The event it raises goes to a hook
     `run` never wires up. Three parks held three worktrees, so 0182 never started and 0181 sat
     behind parked 0185. Neither the digest nor the run's output says the lane stopped on the cap.
- **The sessions reported the owner's seven-day usage at 0.84-0.85 during the run.** The conductor
  spends against that limit as well as against its notional dollar caps, and the digest shows only
  the dollars. Noted here, not acted on.
- **Plan 0187 Phase 6 does not hold.** Its done-when asked for two plans closed on `main` with no owner
  action. Zero closed, and the path that turns a plan into a merged commit has never run. The pilot is
  still owed. [Plan 0188](../plans/0188-the-conductor-survives-its-first-run.md) fixes the four
  defects and carries the pilot as its closing `human` phase: resume 0185, then 0181 behind it.
- **Lane b stays off.** Nothing a second lane adds has been seen on one lane: the close lock ordering
  two closes, two gates queued on the suite lock, a close tip holding two lanes' code. The worktree
  cap also counts across all lanes, and this single lane filled it in 73 minutes, so a second lane
  would have opened nothing more. Lane b is reconsidered only after a conductor-run plan has merged
  end to end, and only together with the cap. Two lanes of parks need either a higher cap or fewer
  parks, and this run showed neither.
