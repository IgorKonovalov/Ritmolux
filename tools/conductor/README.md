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
   or when `claude --version` is not a version the conductor was verified on and not a patch above one.
   A patch above one passes with a warning.
4. **Leave the main checkout alone while a run is live.** Work in a lane of your own. A fast-forward
   refuses a dirty main checkout, so work in progress there parks every close.

## Commands

All of them run from the main checkout.

| Command | What it does |
|---|---|
| `run [--lane a\|b] [--once]` | Runs the queue: both lanes, or one. `--once` stops a lane after one plan. A second conductor is refused while one runs. |
| `status` | Per lane: the plan, the step, the time in it, the spend so far. Then every parked plan with its reason, and whether the repository has already settled it. Regenerates the digest and ends with its path. |
| `digest [--history]` | Rewrites `digest.md`. `--history` writes the per-run account to `digest-history.md` instead, and is the only thing that ever writes that file. |
| `resume NNNN` | Queues a parked plan again. Refused while the park's reason still holds, e.g. a `human` phase the plan's log does not yet mark done. |
| `park NNNN` | Parks a plan that has not merged, with an inbox entry. |
| `finding NNNN [<ref> --done\|--wontfix\|--filed <reason>]` | With no verb, lists that plan's closing verdict with an index per finding. With one, records your disposition against the finding `<ref>` names, and the digest stops carrying it. |
| `adopt-close NNNN` | Records the close a lane already carries, when a session committed one and then lost its outcome. Verifies the branch first and writes nothing unless it passes. |
| `abort` | Stops a running conductor and every session under it. Steps in flight run again on the next `run`. |
| `check` | The preflight alone. |

Ctrl+C on `run` does the same as `abort`.

`run` prints one line per milestone as it happens, each one `HH:MM NNNN <what>`. A line indented
under a plan number happened inside a step or a gate:

```text
10:02 0182 implement-01 start  phases 1-3 (dev)
10:09 0182   commit 3f2a1bc 7m01s feat(shot): the report hears the musical clock in a count column
10:09 0182   phase  1 done, 7m03s
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
  failing tests, every change in the 5-hour or 7-day usage window, and every denied command. A commit
  and a phase line carry **how long since the previous phase line**, or since the step started before
  the first, so a 28-minute phase does not read like a 2-minute one.
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

- **`tools/conductor/digest.md`** is the **current state**, in two sections and nothing else
  (ADR-0214):
  - **Needs you** — the whole worklist, first, so that a page whose first section is empty means
    nothing is waiting on you. It opens with a one-line count and then lists, in this order: the
    CLI-version warning when the last run carried one; every standing park with its age, the worktree
    it holds (or the branch `resume` reopens it from), the usage reading its session ended on and its
    resume command; every lane stopped at the worktree cap, naming what holds the slots; every lane
    still on disk after a merge; and every merge's open findings with their `file:line`.

    A finding leaves that list when the close repaired it (`fixed_in`) or when you closed it with
    `finding NNNN <ref> --done|--wontfix|--filed <reason>` (ADR-0216). **One line then counts every
    finding you have closed**, and names the two commands that read them back — never a per-plan
    breakdown, which is the same accumulation one indent further in. The count line is left out at
    zero, and the section's opening count is of **open** findings only, so a page with nothing on it
    is one line.

    **Already settled, clear the record** closes that section: a park the repository itself shows as
    finished, counted apart from the live ones and never listed among them. Two conditions decide it,
    both narrow, both read from the tree, and **nothing else — never an age, never a branch's commits,
    never a tag**: the plan is under `docs/plans/done/` **in the main checkout** with `Status: done`
    (a close that landed outside the conductor never touches `state/conductor.json`), or a
    `human_phase` / `claude_dir` park sits on a phase the plan's own `## Implementation log` now marks
    `done` — read in the lane when the worktree is still there, in the main checkout when it is gone.
    A close committed in a lane that has not merged is **not** settled. `status` prints the same
    verdict on the same park. **The digest writes nothing back**: `resume NNNN` clears the record, and
    it stays your explicit act.
  - **Now** — per lane, the plan, the step and how long it has been in it, and what the plan has
    spent. When no run is live, the last run's end time and its one-line totals.

  It is gitignored and rewritten from `state/` and `git` after every step, so deleting it loses
  nothing. Nothing on it is per-run: **what last night produced is `digest --history`**, or the
  closed plan's own committed `## Close review`.
- **`tools/conductor/digest-history.md`** is the per-run account, written only by
  `conductor.mjs digest --history`, newest run first:
  - **Needs you:** every park in that run with its resume command and the usage reading its session
    ended on, every lane that stopped at the worktree cap, then every merge's open findings with
    their `file:line`. The newest run adds **Still parked from an earlier run**: each plan still
    parked from before it, with its age, the worktree it holds (or the branch `resume` reopens it
    from) and its resume command.
  - **Not started:** each queued plan the run did not open, with why: `worktree cap`, `--once`, or
    `after NNNN (parked)` naming the plan it waits on and that plan's status. Left out when the run
    opened everything it could.
  - **Closed:** each merged plan's tag, merge commit, fix rounds, active time (its steps and gates)
    and wall time within the run it merged in, **both** what it spent in that run and what it has
    spent over its whole life, and every review finding exactly as the reviewer emitted it. The two
    `$` figures are named because a run-scoped time beside a lifetime spend reads as neither; the
    per-run one is what Totals below sums. A finding you have closed carries its verb, its reason and
    the date here, whatever the current page counts: this is the record of the judgement.
  - **Failed and parked:** gate reds with the failing tests, disagreements, spend-cap hits, session
    errors.
  - **Totals:** merged and parked counts, spend, time spent waiting on each lock, the 5-hour and
    7-day usage windows at run start and run end, and gate minutes split into the full suite and
    everything else, with the count of suite runs skipped.

  It is gitignored too, and both pages are built from one reader over `state/conductor.json`.
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
| `claude_dir` | The same, and for the same reason: the phase declares a file under `.claude/`, which the CLI will not let a session write (ADR-0210). **Nothing was run** — the park comes before the phase. The detail names the paths. Do the phase in the lane, mark its row `done`, commit; `resume` checks the row. |
| `stop_condition`, `plan_wrong`, `question` | Read the transcript the inbox names. Settle it in a human-started `/architect` session. |
| `gate_red` | Read the gate log. Fix the defect in the lane. The conductor never retries a red. |
| `review_failed` | Read the last review under `state/reviews/`. Resuming grants two fresh fix rounds. |
| `disagreement` | A session's claim and `git` differ. Read the detail and the transcript before trusting the lane. |
| `cli_contract` | The CLI ran a session without the project hooks, or without loading the skill it invoked. Read the detail and the transcript, then verify the CLI version before resuming (`## When the CLI updates`). |
| `lost_background` | The session started a command in the background and ended with it unfinished, so that work was killed with the session. Its commits are still in the lane. Read the detail for the command, check what the lane actually contains, then resume: the step runs again from what the plan log and `git` show. |
| `budget`, `api`, `no_outcome`, `bad_outcome` | Raise the budget in `local.json`, or wait out a usage limit. Resuming re-runs the step from what the plan log and `git` show. |
| `merge_conflict`, `merge_failed`, `main_dirty` | Resolve it in the lane, or clean the main checkout. A resumed plan goes straight back to the fast-forward. |

**A suite you run by hand counts.** Run one through the wrapper —
`node tools/conductor/with-lock.mjs suite -- cargo nextest run --workspace` — and, because the
wrapper can see it is running inside this repository or one of its worktrees, it records the result in
`state/suite-ledger.jsonl` as `hand`. The conductor's next gate on that same tree finds the record,
prints it and does not run the suite again (ADR-0207). The clean-at-both-ends rule is the same as for
a session's run: a tree that was dirty when the suite started or when it finished records nothing,
because the tree hash would not name what was tested. `RLX_SUITE_LEDGER` still overrides the choice,
and a wrapped run in any other repository records nothing.

**A close that landed without an outcome is adopted, never reviewed a second time.** A review session
commits its repairs, its `done/` move, its version bump and its tag before it prints anything, so a
session that dies after that leaves the branch closed and the record open. Before a run reviews
anything it asks the **branch**: a plan under `done/` with `Status: done` and a `## Close review` is a
finished close. It is verified exactly as a session's own close is — clean tree, annotated tag on the
tip — and then recorded, so the run goes straight to the gate on the close tip and the fast-forward.
A branch that does not verify parks `disagreement` and names why; nothing is recorded, and no second
version or tag is ever written. `adopt-close NNNN` does the same thing on demand, for a record that
needs repairing outside a run: it changes nothing unless the branch verifies, and `resume` then `run`
finishes the plan.

**Whatever the reason, `resume` refuses a lane whose worktree is dirty.** A session is told to leave
the tree clean and may run `git restore` to do it, but a park does not prove that it did. The park
records the dirty paths: the first 10 and a count of the rest, in `conductor.json`, the inbox entry
and the digest's **Needs you** line. The conductor never reverts them, because they may be the
evidence you need, such as the goldens a test run re-encoded. Read them, then commit them or
`git restore` them in the lane, and resume.

A merged plan whose worktree could not be removed (Windows refuses while any shell sits inside it) is
an inbox entry, not a park: close the shell, then `git worktree remove`, `git worktree prune` and
`git branch -d`.

## Closing a finding

A close review's `minor` and `nit` findings that the close did not repair are yours (ADR-0209), and
they stay on the digest until you say what became of one. An open finding has three honest futures,
and `finding` records which (ADR-0216):

```text
node tools/conductor/conductor.mjs finding 0181
node tools/conductor/conductor.mjs finding 0181 3 --wontfix "assertion message, no reader"
```

- **`--done`** — you repaired it. **`--wontfix`** — you judged it not worth repairing. **`--filed`** —
  it is now a `docs/design-backlog.md` entry, which carries its own probe. Writing that entry stays
  yours; the verb only records that someone did.
- **The reason is required**, and an empty or whitespace one is refused. Nothing verifies a
  disposition — unlike `fixed_in`, which is a commit the conductor checks against the branch, this is
  a judgement checked against nothing — so the sentence you type is the whole record of it.
- **`<ref>` is the index the listing prints**, or the `file:line` exactly one finding carries. A ref
  that matches nothing, or more than one, is refused naming what it saw.
- **Re-dispositioning overwrites and keeps the previous one** in the finding's history: a `--wontfix`
  you later repair should read as repaired, and that you first declined it is worth keeping.
- **Only you write one.** No session, no close and no gate may, and *recording* one is refused while
  a conductor is live, because that run would write its own copy of the record over yours; listing is
  read-only and runs at any time.

The dispositions live in `state/conductor.json`, which is gitignored: losing that file returns every
closed finding to the page. The finding *text* is safe — it is committed in each plan's
`## Close review` — but the judgement about it is not.

## How it stays safe

- **The sessions.** Each one runs with `--permission-mode dontAsk` and the allowlist in
  `settings.conductor.json`, with `RLX_CONDUCTOR=1` in its environment. It gets one of the
  `prompts/` templates as its appended system prompt, and that prompt is the only thing that puts a
  skill into its `## Conductor mode`.
- **The allowlist covers a phase's own scratch work, and the lane is its bound.** Making and removing
  a file or directory, `cat` / `Get-Content`, and `git clean` / `git checkout` of a path named after
  `--` all run; a deletion whose path leaves the worktree — `..`, `~`, a leading `/` or a drive
  letter — is denied, and a `git clean` with no path matches nothing. `git checkout` reaches nothing
  but a path, because a rule without the `--` would let a session move the lane's branch, and
  `git stash` is refused outright: that stack is shared by every worktree on the machine. The CLI
  reads each command of a compound call on its own, so `cd studio; npm run typecheck` is refused for
  its `cd` — the prompts tell a session to run one command per call and pass `--prefix` instead.
  **Every rule has a case in `test/settings.test.mjs`**, which fails on a rule added without one.
- **The hooks.** `.claude/hooks/block-push-and-history-rewrite.js` denies `git push`,
  `reset --hard`, `rebase`, `commit --amend` and `filter-branch` in every session, human-started
  ones included. `.claude/hooks/conductor-suite-lock.js` denies any `nextest` or `cargo test` a
  conductor session runs outside the lock, except `cargo nextest list`, which runs no test and
  takes no lock even when wrapped. `.claude/hooks/conductor-no-background.js` denies
  `run_in_background` on a shell call.
- **No session writes under `.claude/`, and nothing pretends otherwise.** The CLI denies a headless
  session an `Edit` or a `Write` there whatever the allowlist says — measured on 2.1.273 under both
  `settings.conductor.json` and settings naming `.claude/` paths explicitly, while a read is allowed
  and a write elsewhere in the same worktree succeeds (`spike/README.md`). So **a phase whose
  `Files touched` names such a path parks the plan before the phase runs**, `claude_dir`, with the
  paths as the detail; the phases before it in the same run are still handed to a session. A review
  **finding** under `.claude/` stays open, carries no `fixed_in`, and names its replacement text in
  the digest's **Needs you**, for the owner to apply. ADR-0210; it amends ADR-0209's repair list.
- **No session works in the background.** Nothing re-invokes a `claude -p` session: one that starts
  a long command in the background and ends its turn exits, the command is killed, and the result is
  lost — after the commits it already made have landed. Three layers, none sufficient alone: the
  `prompts/` and each skill's `## Conductor mode` say so, the hook above and the settings' `Monitor`
  denial refuse it, and a session that reaches its result with a background command still unfinished
  parks the plan `lost_background` before its outcome is read. The detector reads a result-text shape
  the CLI owns, so a reworded message would stop it seeing a start — which is why the other two
  layers exist.
- **The suite's three tiers never skip a test outright** (ADR-0211). The gate's suite step is
  `skipped` when a green record names **this exact tree** and nothing runs; `served` when a green
  record names some other tree and every path between the two is on a declared list, and then
  `cargo nextest run --workspace -P fast` runs in the full suite's place; `ran` otherwise, the full
  suite. **The only question the tier ever answers is *full suite or `-P fast`*, and `-P fast` is
  what CI runs on every push** — `hygiene.rs`, `preset.rs`, every `CARGO_PKG_VERSION` reader and
  ADR-0157's 24-preset sample are inside it. What a served tree trades away is the nine deferred GPU
  suites' sweep, on a diff that cannot reach them.
- **The served list is an allowlist, and the direction is the safety surface.** `SERVED_PATHS` in
  `lib/ledger.mjs` is the list: `docs/`, `.claude/`, `tools/`, `site/`, `studio/`, `packaging/`,
  `renders/`, any `*.md`, and `Cargo.toml` / `Cargo.lock` when their whole diff is a
  `version = "x.y.z"` line. A path is served only by being **named**; everything unlisted — every
  `.rs`, every `.wgsl`, `presets/*.toml`, `core/tests/goldens/`, `.config/nextest.toml`,
  `.github/` — falls through to the full suite by omission. Written the other way round, a path type
  nobody thought of would be under-gated silently; an allowlist that forgets a path is merely slow.
  The diff is measured **against the green tree**, not against the stage's own commits, so a
  `git merge main` that brought in another lane's render change re-arms the suite by construction.
- **A served run never becomes a green record.** Its ledger line carries `served: true` and a `cmd`
  that is not the one the key names, so neither lookup can read it back: the next stage leans on the
  full-suite record again, and one `-P fast` never chains off another. The run terminal prints a
  served step's own line naming the tier and the tree it leaned on, and the history's Totals counts
  served runs apart from full ones.
- **The locks.** `with-lock.mjs` holds two machine-wide locks. The **suite** lock stops two lanes
  running the GPU suites at once. The **close** lock runs from before a review until `main` has
  fast-forwarded, so a version bump and its tag always land on the `main` they were computed against.
- **The checks.** The conductor believes the repository, not the session. A claimed commit must
  exist and be new, the plan's log rows must match, the tree must be clean, and a close must leave
  the plan under `done/` with a `## Close review` and an annotated tag on the branch tip. A finding
  the close marks repaired (`fixed_in`, ADR-0209) must name a commit on the branch that changes that
  finding's file.

## The gate

The conductor runs its own gate in the worktree and ignores any session's claim that the checks
passed. **What runs at each stage is `gateForStage` in `lib/gate.mjs`, and nowhere else**: read it
there rather than from a copy here, which would drift. The commands run in order and stop at the
first red. The gate runs at four stages: `pre-review` after the last implementer run, `fix-N` after
each fix round, `post-close` on the tip a close produced before `main` moves, and `remerge` after the
automatic re-merge of a moved `main`. Only the last two run a step marked `afterClose`. **A full
workspace suite the conductor saw pass is not run again on the same tree** (ADR-0207), and **a tree
a green record serves runs `-P fast` in its place rather than the full suite** (ADR-0211 — the three
tiers, and the list they rest on, are in *How it stays safe* above).
`state/suite-ledger.jsonl` holds one line per suite run that conductor code observed, keyed by
`HEAD^{tree}` and written only when the worktree was clean at both ends, plus one line per skip
naming the run it relied on and one per served run naming the tree it leaned on and the diff that
served. The gate and `with-lock.mjs` both consult it: every session is handed
the ledger in `RLX_SUITE_LEDGER`. They skip on a green record for the exact tree and print that
record, and the digest counts every skip. **Only the gate serves a record forward**: a session's
wrapped suite keeps the exact-tree lookup, so the lanes' `## Conductor mode` instructions stay true
as written. Any change to a tracked file, a doc included, is a new
tree. A red run is recorded and never skipped on. Any argument vector other than the ledger's own
(`SUITE_COMMAND` in `lib/ledger.mjs`) neither skips nor records, and outside the conductor the
wrapper never reads the ledger.

**The backlog probes wait for the close.** A plan can deliver exactly what a live entry's probe says
is missing, and turn that probe red. Archiving the entry is the close's job (ADR-0108), so a red
probe before the review is not a defect yet. `post-close` still parks a close that left one red.

## When the CLI updates

`VERIFIED_CLI` in `conductor.mjs` lists the `claude --version`s the evidence in `spike/README.md` was
produced on (ADR-0208).

- **A higher patch of a listed major.minor runs, with a warning.** `run` and `check` print it, the run
  records it as `cli`, and the digest's **Needs you** carries it while that run is the latest one.
  The line stops appearing on the first run whose version is listed; the history keeps it on the run
  that carried it. This CLI numbers nearly every release as a patch, so in
  practice most updates land here.
- **Any other unlisted version is refused**, as before: a new minor or major, or a lower patch.

**Every session proves the project hooks ran in it, whatever the version.** The conductor hands each
step a hook log, `state/hooks/<step>.log`. `.claude/hooks/conductor-suite-lock.js` appends one line to it
per shell call. When a session ends, before its outcome is read, two things must hold. If its
transcript holds a `Bash` or `PowerShell` call, the hook log must be non-empty. The stream's
`system/init` must list the skill the prompt invoked. Either failure parks the plan `cli_contract`.
It proves that one hook ran, not that every one did, and a session with no shell call cannot be
checked.

To verify a new version, and clear the warning:

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
