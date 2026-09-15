# The conductor

A Node program that takes approved plans off a queue and runs them to a merged `main` with no owner
action in between, in up to two git worktree lanes. For each plan it opens a lane, starts one fresh
headless `claude -p` session per contiguous same-owner run of phases, checks each session's claim
against `git`, runs its own gate, starts a fresh headless `architect` session that reviews and closes
the plan on the branch, fast-forwards `main`, and removes the lane.

**It never pushes.** Everything it does stays on this machine until you read what happened and push.

**Anything it cannot decide parks the plan**, and the lane moves on to the next plan. That covers a
`human` phase, a plan's own stop condition, a red gate, a review still failing after two fix rounds,
a spend cap, or a session whose claim `git` does not bear out.

The decision and its rejected alternatives are ADR-0205. The plan that built it is Plan 0187.

## Before the first run

1. **Write `tools/conductor/local.json`.** Copy `local.example.json` and set your own figures. The
   file is gitignored, and the conductor refuses to start without it, because it carries no default
   spend.

   ```json
   { "budget_usd": { "implement": 8, "fix": 4, "review": 6 }, "max_open_worktrees": 3 }
   ```

   Every session gets its step's figure as `--max-budget-usd`. **The cap is checked between turns**,
   so a step can overrun it by one turn's cost (see `spike/README.md`). Optionally,
   `"model": { "implement": "opus", "fix": "opus", "review": "opus" }` picks the model per step. When
   `model` is absent, sessions use your own default.
2. **Check the queue.** `queue.json` is committed, and the architect owns its sequencing: an ordered
   plan list per lane, an optional `after` list of plans that must merge first, and optional
   `add_dirs` for a plan that reads outside the repository. A queued plan must read
   `Status: approved`.

   ```json
   { "lanes": { "a": ["0175", "0185"], "b": [] }, "plans": { "0181": { "after": ["0185"] } } }
   ```
3. **Run the preflight:** `node tools/conductor/conductor.mjs check`. It refuses when `local.json` is
   missing, when `queue.json` names a plan that is not approved or depends on a plan it cannot reach,
   or when `claude --version` is not a version the conductor was verified on.
4. **Leave the main checkout alone while a run is live.** Work in a lane of your own. A fast-forward
   refuses a dirty main checkout, so work in progress there parks every close.

## Commands

All of them run from the main checkout.

| Command | What it does |
|---|---|
| `run [--lane a\|b] [--once]` | Runs the queue: both lanes, or one. `--once` stops a lane after one plan. A second conductor is refused while one runs. |
| `status` | Per lane: the plan, the step, the time in it, the spend so far. Then every parked plan with its reason. Regenerates the digest and ends with its path. |
| `resume NNNN` | Queues a parked plan again. Refused while the park's reason still holds, e.g. a `human` phase the plan's log does not yet mark done. |
| `park NNNN` | Parks a plan that has not merged, with an inbox entry. |
| `abort` | Stops a running conductor and every session under it. Steps in flight run again on the next `run`. |
| `check` | The preflight alone. |

Ctrl+C on `run` does the same as `abort`.

`run` prints one line per milestone as it happens, each one `HH:MM NNNN <what>`. A line indented
under a plan number happened inside a step or a gate:

```text
10:02 0182 implement-01 start  phases 1-3 (dev)
10:09 0182   commit 3f2a1bc feat(shot): the report hears the musical clock in a count column
10:09 0182   phase  1 done
10:14 0182   tests  nextest run -p standalone: 212 passed, 0 failed; lock wait 2m10s, ran 3m02s
10:15 0182   denied PowerShell: cd studio; npx vitest run
10:31 0182   usage  5h 0.27 (resets 14:30); 7d 0.02 (resets 09-22 16:00)
10:40 0182 implement-01 end    phases_done, 38 min, $5.83, 64 turns
10:40 0182 gate pre-review  checks ok (15, 9s)
10:43 0182   gate   cargo nextest running
10:54 0182   gate   cargo nextest ok 10m51s (1940 passed, 0 failed, 6 skipped)
```

- **Steps:** a start line with the phase range or round and the owner skill, and an end line with the
  outcome or park reason, the duration, the spend and the turns. Spend arrives only at the end: the
  CLI reports no running cost inside a session.
- **Inside a session:** each commit as it lands, each phase its log row marks done, each `cargo
  nextest`, `cargo test`, `cargo clippy` or `cargo doc` call starting and ending with its counts or
  failing tests, every change in the 5-hour or 7-day usage window, and every denied command.
- **Gates:** one line for the node, studio and sd-filter checks, or one per failure, then each cargo
  command's start and end.
- **At run start:** every plan still parked, with its age and the worktree it holds, or the branch
  `resume` reopens it from when the worktree is gone.
- **Also:** a lane opening, a park, a close, a fast-forward.

The same lines go to `state/live.log`, under one header per run. They are a display, read from the
CLI's stream: an event kind the reader does not know prints nothing, so a missing line is never
evidence that something did not happen.

**A lane stops when opening its next plan would exceed `max_open_worktrees`**, and says so, naming the
plans that hold the worktrees. That cap is the disk bound, not a queue: the lane does not wait for a
slot. Remove a finished lane or settle a parked one, then `run` again. **The cap counts worktree
directories that exist on disk**, so a lane you removed with `git worktree remove` stops counting at
once, whatever `state/conductor.json` says.

## What to read afterwards

- **`tools/conductor/digest.md`** is the morning-after record, newest run first:
  - **Needs you:** every park with its resume command and the usage reading its session ended on,
    every lane that stopped at the worktree cap, then every merge's open findings with their
    `file:line`. The newest run adds **Still parked from an earlier run**: each plan still parked
    from before it, with its age, the worktree it holds (or the branch `resume` reopens it from) and
    its resume command.
  - **Not started:** each queued plan the run did not open, with why: `worktree cap`, `--once`, or
    `after NNNN (parked)` naming the plan it waits on and that plan's status. Left out when the run
    opened everything it could.
  - **Closed:** each merged plan's tag, merge commit, fix rounds, active time (its steps and gates)
    and wall time within the run it merged in, spend, and every review finding exactly as the
    reviewer emitted it.
  - **Failed and parked:** gate reds with the failing tests, disagreements, spend-cap hits, session
    errors.
  - **Totals:** merged and parked counts, spend, time spent waiting on each lock, the 5-hour and
    7-day usage windows at run start and run end, and gate minutes split into the full suite and
    everything else, with the count of suite runs skipped.

  It is gitignored and regenerated from `state/` and `git` after every step, so deleting it loses
  nothing.
- **`state/live.log`** holds every line the run terminal printed, one header per run, for a run you
  did not watch.
- **`## Close review` in each closed plan** holds the review itself, committed with the close.
- **`state/inbox.md`** gets one entry per park: the reason, the file to read, the worktree it holds,
  and the resume command.
- **`state/`** holds everything else: `conductor.json` (the runtime record), `transcripts/` (every
  session's stream), `prompts/`, `reviews/`, `gates/` (each gate command's output), `locks.jsonl`
  and `live.log`.

## Acting on a park

| Reason | What to do before `resume` |
|---|---|
| `human_phase` | Do the phase. Mark its row `done` in the plan's `## Implementation log` **in the lane** (`WORK/rlx-plan-NNNN`) and commit it there. `resume` checks the row. |
| `stop_condition`, `plan_wrong`, `question` | Read the transcript the inbox names. Settle it in a human-started `/architect` session. |
| `gate_red` | Read the gate log. Fix the defect in the lane. The conductor never retries a red. |
| `review_failed` | Read the last review under `state/reviews/`. Resuming grants two fresh fix rounds. |
| `disagreement` | A session's claim and `git` differ. Read the detail and the transcript before trusting the lane. |
| `budget`, `api`, `no_outcome`, `bad_outcome` | Raise the budget in `local.json`, or wait out a usage limit. Resuming re-runs the step from what the plan log and `git` show. |
| `merge_conflict`, `merge_failed`, `main_dirty` | Resolve it in the lane, or clean the main checkout. A resumed plan goes straight back to the fast-forward. |

**Whatever the reason, `resume` refuses a lane whose worktree is dirty.** A session is told to leave
the tree clean and may run `git restore` to do it, but a park does not prove that it did. The park
records the dirty paths: the first 10 and a count of the rest, in `conductor.json`, the inbox entry
and the digest's **Needs you** line. The conductor never reverts them, because they may be the
evidence you need, such as the goldens a test run re-encoded. Read them, then commit them or
`git restore` them in the lane, and resume.

A merged plan whose worktree could not be removed (Windows refuses while any shell sits inside it) is
an inbox entry, not a park: close the shell, then `git worktree remove`, `git worktree prune` and
`git branch -d`.

## How it stays safe

- **The sessions.** Each one runs with `--permission-mode dontAsk` and the allowlist in
  `settings.conductor.json`, with `RLX_CONDUCTOR=1` in its environment. It gets one of the
  `prompts/` templates as its appended system prompt, and that prompt is the only thing that puts a
  skill into its `## Conductor mode`.
- **The hooks.** `.claude/hooks/block-push-and-history-rewrite.js` denies `git push`,
  `reset --hard`, `rebase`, `commit --amend` and `filter-branch` in every session, human-started
  ones included. `.claude/hooks/conductor-suite-lock.js` denies any `nextest` or `cargo test` a
  conductor session runs outside the lock, except `cargo nextest list`, which runs no test and
  takes no lock even when wrapped.
- **The locks.** `with-lock.mjs` holds two machine-wide locks. The **suite** lock stops two lanes
  running the GPU suites at once. The **close** lock runs from before a review until `main` has
  fast-forwarded, so a version bump and its tag always land on the `main` they were computed against.
- **The checks.** The conductor believes the repository, not the session. A claimed commit must
  exist and be new, the plan's log rows must match, the tree must be clean, and a close must leave
  the plan under `done/` with a `## Close review` and an annotated tag on the branch tip.

## The gate

The conductor runs its own gate in the worktree and ignores any session's claim that the checks
passed. The commands are `defaultGate()` in `lib/gate.mjs`: the pre-push hook's list at full
strength, plus `cargo doc` and these tests. They run in order and stop at the first red.

| Stage | When | Runs |
|---|---|---|
| `pre-review` | after the last implementer run, before the review | every command except `check-backlog-claims.mjs` |
| `fix-N` | after fix round N, before the re-review | every command except `check-backlog-claims.mjs` |
| `post-close` | on the close tip, before `main` moves | every command |
| `remerge` | after the automatic re-merge of a moved `main` | every command |

**The backlog probes wait for the close.** A plan can deliver exactly what a live entry's probe says
is missing, and turn that probe red. Archiving the entry is the close's job (ADR-0108), so a red
probe before the review is not a defect yet. `post-close` still parks a close that left one red.

## When the CLI updates

The conductor refuses a `claude --version` not listed in `VERIFIED_CLI` in `conductor.mjs`. To
verify a new version:

1. Run `node tools/conductor/spike/probe.mjs` (two short sessions on `--model haiku`).
2. Compare its output with the evidence table in `spike/README.md`.
3. Record the new version there, then add it to the list.

## Tests

```sh
node --test "tools/conductor/test/*.test.mjs"
```

No test spends money or needs a network. `test/fake-claude.mjs` stands in for the CLI, and
`test/lane-scenario.mjs` makes the commits a real session would, in throwaway repositories. CI's
`links` job runs the same command.
