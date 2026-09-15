# 0189 — The conductor can be watched, and stops re-proving a green tree

> **Status:** approved
> **Created:** 2026-09-15
> **Owner skill(s):** dev, human
> **Related ADRs:** [0205](../adrs/0205-an-approved-plan-runs-under-a-conductor-and-every-judgement-it-cannot-make-parks-the-plan.md),
> [0207](../adrs/0207-a-suite-run-the-conductor-observed-green-is-not-run-again-on-the-same-tree.md) (proposed),
> [0208](../adrs/0208-a-patch-cli-update-runs-with-a-warning-and-every-session-proves-the-hooks-ran.md) (proposed),
> [0209](../adrs/0209-a-conductor-close-repairs-the-prose-and-comments-its-findings-name.md) (proposed)
> **Closes:** design-backlog 0222, 0223, 0224, 0225
> **Built by:** human-started `dev` sessions, not the conductor. A conductor editing its own code
> while it runs is circular, and 0187 and 0188 were built the same way.

## TL;DR

The owner watches a conductor run in the one terminal that started it. Each line is a milestone: a
step starting and ending with its time and spend, a commit landing, a phase marked done, a test or
gate command finishing with its counts, a lock wait, a usage-window reading, a denied command. The
digest reports the usage windows, time that excludes parks, every plan still parked, and only the
findings still open. A full workspace suite is no longer run twice on the same tree: a plan with no
fix round and an unmoved `main` runs it twice instead of up to five times. A patch CLI update warns
instead of blocking, and every session proves the project hooks ran. A close repairs the comment and
prose findings it would otherwise merge open.

## Context & problem

The 2026-09-15 pilot (ADR-0205's third Outcome entry) was the first run to merge plans. Three merged,
nothing parked, in 3 h 38 min. The owner's verdict afterwards: improvements are needed, on five
fronts.

- **Nothing between the coarse events can be seen.** `run` prints a line when a lane opens, a step
  starts, a plan parks, closes or fast-forwards (`conductor.mjs` `eventLine`). An implement session
  runs 20 to 80 min with no output. So does a 12 min gate. The data exists: `step.mjs` already gets
  every stream-json chunk and appends it to the transcript, and the worktree's `git log` shows each
  commit.
- **The gate re-proves green trees** (backlog 0223, ADR-0207's Context). Up to five full suites per
  plan at ~10.7 min each, 72 min of the pilot's 218 in the conductor's gate alone. 14 of the 15
  reported suite-lock wait minutes were `cargo nextest list` calls, which run no tests, blocked behind
  full runs: the hook pattern at `.claude/hooks/conductor-suite-lock.js:18` matches any `nextest`
  subcommand.
- **The digest answers the wrong questions.**
  - It reports dollars, while the owner's constraint is the 5-hour and 7-day usage windows. Those are
    recorded and never shown (backlog 0222).
  - 0185's line says "16 h 34 min" because `span(rec.started, rec.ended)` includes a night spent
    parked.
  - The newest run's **Needs you** drops every park from an earlier run, so 0175 and 0180, parked
    since 2026-09-14 and holding two worktrees until the owner removed them by hand on 2026-09-15,
    have no line in the latest section.
  - It gives open findings only as "merged with N minors - see Closed".
- **A CLI update blocks the queue** (backlog 0224; owner's call: warn on a patch, refuse otherwise,
  per ADR-0208).
- **Findings nobody fixes** (backlog 0225; owner's call: the close repairs the prose and comment
  ones, per ADR-0209).

## Decision

**One plan, `dev`-owned throughout, ending in a `human` run.** It covers:

- a richer `run` output, fed from the stream `step.mjs` already reads, plus a polled `git log` in
  the worktree;
- a digest that reports usage, active time, standing parks and open findings;
- `nextest list` outside the suite lock;
- ADR-0207's suite ledger, and the conductor-mode reordering that makes identical trees happen;
- ADR-0208's patch warning and hook tripwire;
- ADR-0209's close repairs;
- the pilot's four leftover prose findings, repaired under the new rule.

Rejected during the interview:

- a separate `watch` command that attaches to a running conductor: `run`'s own terminal is enough,
  and it avoids a second reader of state;
- a periodic fix-up plan for leftovers (ADR-0209 Alternative A);
- skipping the suite on docs-only changes: Rust tests read `docs/` (ADR-0207 Alternative A);
- splitting this into two or three plans.

## Architecture diagram

```mermaid
flowchart LR
    subgraph session["claude -p session (worktree)"]
        S[stream-json stdout]
        W["with-lock.mjs suite -- cargo nextest run --workspace"]
        H[conductor-suite-lock.js hook]
    end
    subgraph conductor["tools/conductor (main checkout)"]
        ST[step.mjs line reader]
        LV[live.mjs formatter]
        GT[gate.mjs]
        LG[(state/suite-ledger.jsonl)]
        HL[(state/hooks/label.log)]
        DG[digest.mjs]
    end
    TERM[run terminal + state/live.log]
    S --> ST --> LV --> TERM
    GT --> LV
    W <--> LG
    GT <--> LG
    H --> HL
    HL --> ST
    ST --> DG
    GIT[(worktree git log)] --> LV
```

## Implementation phases

### Phase 1 — The run can be watched
- **Owner skill:** dev
- **What:** `run` prints one line per milestone as it happens, and appends the same lines to
  `tools/conductor/state/live.log` under a run header.
  - **Step boundaries:** start (the label, phase range and owner skill) and end (outcome kind or
    park reason, duration, `$` spend, turns).
  - **Commits:** each commit landing in the worktree during a session, as short sha and subject.
    The worktree's `git log <step-start>..HEAD` is polled while a session runs.
  - **Phases:** each phase whose `## Implementation log` row flips to done, read from the plan file
    in the worktree after a new commit.
  - **Tests and suite runs inside a session:** a `cargo nextest` / `cargo test` / `cargo clippy` /
    `cargo doc` call starting and finishing. The end line shows nextest's pass, fail and skip counts
    or the first failing test names, the lock wait and the run time. A run started in the background
    reports its end when its task notification arrives.
  - **Gate commands:** each cargo command's start and end, with duration and result. The node, studio
    and sd-filter checks print one aggregate line, or one line per failure.
  - **Usage:** the 5-hour and 7-day utilization and reset time, printed whenever either reading or
    the status changes.
  - **Denials:** a `system/permission_denied` event as a line naming the tool and the head of the
    command.
  - **Standing parks:** at run start, one line per plan still parked, with its age and the worktree
    it holds.
- **Files touched:** `tools/conductor/lib/live.mjs` (new: stream-event and gate-event to line,
  pure), `lib/step.mjs` (split stdout chunks into lines and hand each parsed event to an
  `onStreamEvent` callback; the transcript append is unchanged), `lib/lane.mjs` (commit and phase
  polling during a session, standing parks at start), `lib/gate.mjs` (per-command start and end
  callbacks, per-command duration kept on `rec.gates[].commands`), `with-lock.mjs` (prints its wait
  and hold on stderr at exit), `conductor.mjs` (wire it, and append to `live.log`),
  `test/live.test.mjs` (new), `test/fake-claude.mjs` (emit tool-use, tool-result, task-notification,
  rate-limit and permission-denied events), `README.md` (`## What to read afterwards`, and what the
  run terminal shows).
- **Done when:**
  - A `lane-scenario` run against the fake CLI prints, in order, a start line, a commit line naming
    the sha the fake committed, a phase-done line, a tests end line carrying the fake's nextest
    counts, a usage line, a denial line and an end line carrying the fake's spend. `live.log` holds
    the same lines.
  - A commit made by the fake mid-session is printed before the session ends. The point is that
    commits show while the session runs, not afterwards, and this is what proves it.
  - The formatter reads the usage reading in both recorded shapes: 2.1.272's
    `unifiedWindows.{five_hour,seven_day}` and 2.1.270's top-level `rateLimitType` + `utilization`.
  - A malformed or unknown stream line prints nothing and does not throw.
  - Every line is ASCII. The run terminal is a Windows console.

### Phase 2 — The digest reports what the operator spends, holds and still owes
- **Owner skill:** dev
- **What:** The digest changes in five places.
  - **Totals** carries the usage windows at run start and run end: 5-hour and 7-day utilization with
    reset times, taken from the first and last rate-limit reading of the run, in either shape. It
    also carries gate minutes, split into full-suite minutes and everything else, with the count of
    suite runs skipped (zero until Phase 4).
  - Each closed plan's line reports **active time** (the sum of its steps and gates in this run)
    beside **wall time within this run**, never a span across a park.
  - The newest run's **Needs you** gains **Still parked from an earlier run**: each current park
    whose `at` predates the run, with its age, reason, worktree and resume command.
  - Every park from a session carries that session's last usage reading.
  - "merged with N minors" becomes a list of the findings still open, each with its `file:line`. It
    counts every minor and nit until Phase 5 adds `fixed_in`.
- **Files touched:** `lib/digest.mjs`, `lib/state.mjs` (keep the first and last rate-limit reading
  per step), `test/digest.test.mjs`.
- **Done when:**
  - Regenerating the digest from a state fixture shaped like the pilot, with a plan started in one
    run, parked overnight and merged in the next, reports that plan's active time within a minute of
    the sum of its step and gate durations. It does not report the span from its first start.
  - The same fixture's newest run lists the two plans parked in the earlier run under **Still
    parked**.
  - Regenerating from the same state twice yields the same bytes. That is the digest's existing
    property, and it is kept.

### Phase 3 — Listing tests takes no lock
- **Owner skill:** dev
- **What:** In conductor mode a bare `cargo nextest list` is allowed by the suite-lock hook. The
  wrapper runs `suite -- cargo nextest list ...` without taking the lock and without writing a lock
  log entry. `nextest run` and `cargo test` are unchanged.
- **Files touched:** `.claude/hooks/conductor-suite-lock.js`, `tools/conductor/with-lock.mjs`,
  `tools/conductor/test/hooks.test.mjs`, `tools/conductor/test/with-lock.test.mjs`.
- **Done when:**
  - `cargo nextest list -p rlx-core` passes the hook with `RLX_CONDUCTOR=1`, and
    `cargo nextest run -p rlx-core` is still denied.
  - `cargo nextest list --workspace && cargo nextest run` is still denied on its second command.
  - A wrapped `list` whose lock is held by another process starts at once.

### Phase 4 — A green suite is not run again on the same tree
- **Owner skill:** dev
- **What:** ADR-0207's ledger.
  - `lib/ledger.mjs` records and looks up runs of exactly `cargo nextest run --workspace`, keyed by
    `HEAD^{tree}`. A run is recorded only on a worktree clean at start and end.
  - The gate consults it before its `cargo nextest` step and records after.
  - `with-lock.mjs` does the same when `RLX_SUITE_LEDGER` is set, which `lane.mjs` sets for every
    session. A skip prints one notice naming the record: tree, writer, time, `Summary` line.
  - A skip shows in the live log (Phase 1) and in the digest's skipped count (Phase 2).
- **Files touched:** `tools/conductor/lib/ledger.mjs` (new), `lib/gate.mjs`, `lib/lane.mjs`,
  `with-lock.mjs`, `test/ledger.test.mjs` (new), `test/gate.test.mjs`, `test/with-lock.test.mjs`,
  `README.md` (`## The gate`).
- **Done when:**
  - Two gate runs on one clean tree run the nextest command once. The second records a skip that
    names the first.
  - A one-byte change to any tracked file, docs included, makes the next run execute.
  - A run that started on a dirty worktree, or ended on one, is not recorded.
  - A red run is recorded and never skipped on.
  - A wrapped `cargo nextest run --workspace --no-fail-fast`, or any other argument vector, neither
    skips nor records.
  - Without `RLX_SUITE_LEDGER` the wrapper behaves byte-for-byte as before. Human-started sessions do
    not change.

### Phase 5 — The conductor-mode close orders its work so the gate runs once, and repairs prose findings
- **Owner skill:** dev
- **What:** The skill and prompt text for ADR-0207's ordering and ADR-0209's repairs, and the outcome
  field that carries them.
  - **`dev` conductor mode:** the last implementer run no longer runs the full suite. The close
    block's `Full suite:` bullet reads *owed to the conductor's pre-review gate (ADR-0207)*.
  - **`architect` conductor mode:** runs Mode 4's suite through the wrapper and accepts a printed
    ledger record as lens 1's full-suite evidence. The close runs in this order:
    1. repair every ADR-0209 finding, committed;
    2. `git merge main`;
    3. bookkeeping, version bump and studio sync, committed;
    4. the whole gate on the tip;
    5. the annotated tag;
    6. `check-release-tag.mjs`.
  - A red gate at step 4 parks `check_red`.
  - A finding in `closed.verdict.findings` may carry `fixed_in` (a sha).
  - `verifyClose` checks that each `fixed_in` commit is on the branch and changes that finding's
    file. The digest's open list excludes them.
  - The `.claude/skills/` fact-versus-rule line from ADR-0209 is written into the `architect`
    skill's conductor mode.
- **Files touched:** `.claude/skills/dev/SKILL.md` (Conductor mode), `.claude/skills/architect/SKILL.md`
  (Conductor mode), `tools/conductor/prompts/review.md`, `tools/conductor/prompts/implement.md`,
  `lib/outcome.mjs`, `lib/close.mjs`, `lib/digest.mjs`, `test/lane.test.mjs`, `test/plan.test.mjs`
  or a new `test/outcome.test.mjs`.
- **Done when:**
  - A closed outcome with a `fixed_in` naming a commit that does not touch that finding's file parks
    as `disagreement`.
  - A `fixed_in` on a sha not on the branch parks the same way.
  - A lane scenario whose fake review closes with one `fixed_in` minor and one open minor shows
    exactly the open one in **Needs you**.
  - In a lane scenario on the fake CLI with no fix round and an unmoved `main`, where the fake
    sessions call the wrapper where the skills now say to, the ledger shows **two** executed full
    suites (`pre-review` and the close's gate) and **two** skips (the review's, and `post-close`'s). The count is the property the
    reordering exists for.

### Phase 6 — A patch CLI update warns, and every session proves its hooks ran
- **Owner skill:** dev
- **What:** ADR-0208.
  - `preflight` returns a warning, not an error, for a version sharing major and minor with a
    `VERIFIED_CLI` entry at a higher patch. The warning is printed, recorded as `run.cli`, and shown
    in **Needs you** until the version is listed.
  - `conductor-suite-lock.js` appends one line per call to `RLX_HOOK_LOG` when `RLX_CONDUCTOR=1` and
    the variable is set. `lane.mjs` sets it per step.
  - After a session ends, before its outcome is verified, the conductor checks two things. The
    transcript must hold at least one `Bash` or `PowerShell` tool use with a non-empty hook log. The
    `system/init` event's `skills` must contain the invoked skill. Either failure parks
    `cli_contract`.
- **Files touched:** `tools/conductor/conductor.mjs`, `lib/step.mjs` or `lib/lane.mjs`,
  `lib/outcome.mjs` (the park reason), `lib/digest.mjs`, `.claude/hooks/conductor-suite-lock.js`,
  `test/cli.test.mjs`, `test/hooks.test.mjs`, `test/step.test.mjs`, `README.md` (`## When the CLI
  updates`).
- **Done when:**
  - With `VERIFIED_CLI = ["2.1.272"]`, preflight passes `2.1.280` with one warning, refuses `2.2.0`
    and `3.1.272`, and passes `2.1.272` silently.
  - The test builds its expected messages from the constant, never from literal versions: the shape
    `3381990` fixed.
  - A fake session that makes a shell call and writes no hook log parks `cli_contract`.
  - A fake session whose init lists no invoked skill parks the same way.
  - A fake session that makes no shell call is not parked for the hook log.

### Phase 7 — The pilot's leftovers, under the new rule
- **Owner skill:** dev
- **What:** Repair the four prose findings the 2026-09-15 closes left open, and remove the README
  table that restates `defaultGate()`, which is Plan 0188's open minor.
  - `core/src/render/scenes/particles/mod.rs:1706`: the spin comment still says `advance` runs before
    bindings are routed.
  - `core/src/render/tests.rs:2360`: the assertion message says "first frame" of the second frame.
  - `core/src/render/scenes/fragment_field.rs:218`: "Alpha 1.0:" over a line that returns `occlude`.
  - `.claude/skills/preset-author/references/render-loop.md:170`: the report sample lacks the count
    column Plan 0182 shipped.
  - `tools/conductor/README.md` `## The gate`: replace the per-command table with a pointer to
    `gateForStage` and one paragraph on stages and the ledger.
- **Files touched:** the five named.
- **Done when:**
  - Each named line states what its code does today. Line numbers may have moved; the finding is the
    text.
  - The render-loop sample matches a `shot --presets presets --report family=star` header as it
    prints now.
  - The conductor README no longer lists gate commands by name.
  - `cargo doc` with `-D warnings` stays green.

### Phase 8 — A run, watched
- **Owner skill:** human
- **What:** The owner queues at least one approved plan with no `human` phase and runs the conductor
  from one terminal with the new code.
- **Done when:**
  - The owner has watched the run's live output, and says in the plan's `### Notes` whether it told
    them what was happening without opening a transcript.
  - The plan's log records, from `state/suite-ledger.jsonl` and `state/locks.jsonl`, how many full
    suites each merged plan executed and how many it skipped, and the digest's gate minutes.
  - For a plan with no fix round whose `main` did not move, executed full suites are at most two. If
    not, the log names which run was not skipped and why.

## Data shapes

```jsonc
// illustrative: one line of state/suite-ledger.jsonl
{"tree": "9c1e4a0...", "cmd": "cargo nextest run --workspace", "exit": 0,
 "summary": "1940 tests run: 1940 passed, 6 skipped", "by": "gate 0182-pre-review",
 "at": "2026-09-15T12:03:11Z", "ms": 652000}
```

```text
illustrative: the run terminal
10:02 0182 implement-01 start  phases 1-3 (dev)
10:09 0182   commit 3f2a1bc feat(shot): the report hears the musical clock in a count column
10:09 0182   phase  1 done
10:14 0182   tests  nextest -p standalone: 212 passed, 0 failed; lock wait 2m10s, ran 3m02s
10:15 0182   denied PowerShell: cd studio; npx vitest run ...
10:31 0182   usage  5h 0.27 (resets 14:30); 7d 0.02 (resets 09-22 16:00)
10:40 0182 implement-01 end    phases_done, 38 min, $5.83, 64 turns
10:40 0182 gate pre-review     node checks ok (15, 9s)
10:43 0182   gate   cargo clippy ok 2m41s
10:43 0182   gate   cargo nextest running
10:54 0182   gate   cargo nextest ok 10m51s (1940 passed, 6 skipped)
```

```jsonc
// illustrative: a finding a close repaired (ADR-0209)
{"severity": "minor", "file": "core/src/render/tests.rs", "line": 2360,
 "what": "assertion message names the first frame for the second", "fixed_in": "a1b2c3d"}
```

## Risks & open questions

- **The stream is the CLI's, not ours.** The Phase 1 reader parses event kinds observed on 2.1.272:
  `tool_use` in `assistant`, `tool_result` in `user`, `system/task_notification`,
  `system/permission_denied`, `rate_limit_event`. A renamed event only drops a line, since the
  formatter ignores what it does not know. That is the right failure for a display, and it is also
  why the display cannot be the evidence for anything.
- **No running dollar figure inside a session.** On 2.1.272 the per-turn `assistant` events carry
  token usage but no cost, and `total_cost_usd` arrives only on the `result` event. Spend is printed
  at step end. An estimate from tokens would need a price table this repository does not keep.
  Unverified whether a later CLI adds a running figure.
- **Phase 5's count needs the fake sessions to follow the new skill text.** The done-when proves the
  conductor's arithmetic given that behaviour. Only Phase 8 shows that real sessions follow it.
- **ADR-0208's warning covers nearly every CLI update**, because the CLI numbers almost everything as a
  patch. The owner chose it knowing this, and the ADR's Negative says what the tripwire does not see.
- **The ledger's key leaves out the toolchain.** A `rustup update` between the review and
  `post-close` on one tree would skip a run on a changed compiler. Rare on an unattended run, and
  written in ADR-0207 rather than guarded.
- **Plan 0187's other two open minors stay open**: the version cross-check in `close.mjs`, and the
  PowerShell here-string and `cd studio` allowlist gaps. Phase 1's denial line makes the second
  visible on every run, which is the evidence it lacked.

## What this plan does NOT do

- **Lane b stays off.** ADR-0205's Outcome reconsiders it after 0223. Phase 8's figures are that
  input, and the decision is a later `architect` session's.
- **No `probe.mjs --compare`** (ADR-0208 Alternative A).
- **Nothing about `-P fast` inside `dev` phases.** Backlog 0221 owns the run-alone override's cost,
  and it is not a conductor defect.
- **The pilot's two code-shaped findings stay open**: `standalone/src/shot/report.rs:110`
  (`CAPTURE_DT` mirrors a private constant) and `core/src/render/scenes/cellular/tests.rs:71` (the
  test driver runs the retired order). ADR-0209 gives them no route. If the owner wants one, each
  becomes a backlog entry.
- **No resolution for 0175 and 0180.** Both parked `plan_wrong` on real defects in their plans.
  Phase 2 makes them visible; amending those plans is separate `architect` work.
- **No `watch` command and no second terminal.**

## Implementation log

**Lane:**

| phase | owner | state | commit |
|---|---|---|---|
| 1 — The run can be watched | dev | not started | |
| 2 — The digest reports what the operator spends, holds and still owes | dev | not started | |
| 3 — Listing tests takes no lock | dev | not started | |
| 4 — A green suite is not run again on the same tree | dev | not started | |
| 5 — The conductor-mode close orders its work so the gate runs once, and repairs prose findings | dev | not started | |
| 6 — A patch CLI update warns, and every session proves its hooks ran | dev | not started | |
| 7 — The pilot's leftovers, under the new rule | dev | not started | |
| 8 — A run, watched | human | not started | |

### Notes

### Close triggers
