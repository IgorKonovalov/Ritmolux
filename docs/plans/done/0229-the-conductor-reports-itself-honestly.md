# 0229 — The conductor reports itself honestly

> **Status:** done - Phases 1-4 `0b08037a`, `8bf46530`, `3f151a49`, `6d2d252a`; round 1 fixes
> `f1994976`, `53089d56`, `d63afa27`. Round 2 review clean: no blockers, no majors, no minors; full
> suite 1805 passed. Version none (conductor tooling). Closed 2026-09-24.
> **Created:** 2026-09-24
> **Owner skill(s):** dev
> **Related ADRs:** [0207](../../adrs/0207-a-suite-run-the-conductor-observed-green-is-not-run-again-on-the-same-tree.md),
> [0249](../../adrs/0249-a-human-phase-may-be-owed-after-the-merge.md),
> [0250](../../adrs/0250-the-conductor-stays-up-and-resumes-what-the-repository-shows-settled.md),
> [0205](../../adrs/0205-an-approved-plan-runs-under-a-conductor-and-every-judgement-it-cannot-make-parks-the-plan.md),
> [0210](../../adrs/0210-a-claude-repair-is-the-owners-and-a-session-that-needs-one-parks-with-the-edit.md)
> **Closes:** none

## TL;DR

Four ways the conductor currently misdescribes its own state, all found in one session on
2026-09-24 and each cheap to repair. A flaky suite failure names no test. The digest calls a settled
park live. A merged change to the conductor does not reach the running conductor. And a session
spends turns routing around denials for capabilities its own tools already grant.

## Context & problem

Each of these cost real time on 2026-09-24. None is a defect in what the conductor *decides*; all
four are the machinery describing itself wrongly.

**1. A flaky failure is unattributable.** Plan 0228's close recorded
`1805 tests run: 1804 passed (11 slow), 1 failed` for tree `7f08a04b`; a rerun of the same tree
recorded `1805 passed`. The ledger keeps the exit code and the summary line and nothing else, and
the close session read the ledger rather than echoing `nextest`'s output — so **the failing test's
name exists nowhere on disk**. A flake at a close is therefore diagnosable only by luck.

**2. The digest calls a settled park live.** Plan 0228 taught `parkStillTrue` that an `owed` row
settles a `human_phase` park when the phase carries `**Blocks merge:** no`. `settledPark`
(`tools/conductor/lib/digest.mjs:199`) still tests `donePhases` alone, so `status` and the digest's
`Needs you` keep listing such a plan as waiting on the owner. 0228's own review raised this and a
close could not repair it, because it is code. The README's promise that the digest shows what the
repository shows as settled does not hold for this case.

**3. A merged conductor change does not reach the running conductor.** `run` is one long-lived Node
process that imports `lib/*.mjs` once. After Plan 0228's guard fix merged to `main`, `resume 0220`
was still refused — by a process started before the merge, executing the old guard from memory. The
same command from a fresh process was accepted seconds later. Nothing said so; the refusal named the
plan's log row, which was correct and had been correct for an hour.

**4. A session spends turns on denials its own tools make moot.** Eight distinct commands were
denied across the day's sessions — `ls`, `sed`, `grep`, `cp`, `printenv`, a bare binary path,
`git grep -E` with an alternation, and `gh`. For most of these the session already holds the
capability through `Read`, `Glob`, `Grep` and `Write`, so the denial buys no safety and costs turns.

## Decision

Repair all four. None changes a decision the project has taken; each makes one mechanism agree with
a decision already recorded — [ADR-0207](../../adrs/0207-a-suite-run-the-conductor-observed-green-is-not-run-again-on-the-same-tree.md)
for the ledger, [ADR-0249](../../adrs/0249-a-human-phase-may-be-owed-after-the-merge.md) for what
settles a park, and [ADR-0250](../../adrs/0250-the-conductor-stays-up-and-resumes-what-the-repository-shows-settled.md)
for a resident run that is supposed to reflect the repository.

**No ADR.** Three of the four have no alternative worth recording. The fourth, the stale-process
check, has exactly one rejected option and it is recorded here rather than in an ADR because it
rejects nothing the project believes: **the conductor detects and refuses to start, it does not
restart itself.** An auto-restart would kill in-flight sessions mid-plan, which is the failure
ADR-0205 already forbids for backgrounded work — the same loss, differently caused.

## Implementation phases

### Phase 1 — The ledger names what failed
- **Owner skill:** dev
- **What:** a failed suite record keeps the failing test names, not only the count.
- **Files touched:** `tools/conductor/suite-record.mjs`, `tools/conductor/with-lock.mjs` as the seam
  dictates, `tools/conductor/test/`.
- **Done when:**
  - A failing run's ledger record carries a `failed` array of test names, parsed from `nextest`'s
    `FAIL [ ... ] <name>` lines, capped at the first 20 with a count when there are more. A green
    record is unchanged — no empty array, no new key.
  - `node --test tools/conductor/test/` passes, with a case feeding recorded `nextest` output
    containing two failures and asserting both names reach the record.
  - The park detail for a red gate names the first failing test, so the inbox entry is actionable
    without opening a log.

### Phase 2 — One reader decides a settled phase
- **Owner skill:** dev
- **What:** 0228's open finding. `settledAs` moves to `plan.mjs` and both readers call it.
- **Files touched:** `tools/conductor/lib/plan.mjs`, `tools/conductor/lib/lane.mjs`,
  `tools/conductor/lib/digest.mjs`, `tools/conductor/test/digest.test.mjs`,
  `tools/conductor/README.md`.
- **Done when:**
  - `settledPhase(plan, id)` is exported from `plan.mjs` and is the only implementation;
    `parkStillTrue` and `settledPark` both call it, and `git grep -c "donePhases(" -- tools/conductor/lib`
    shows no caller deciding this question alone.
  - A `human_phase` park whose phase is marked `**Blocks merge:** no` with an `owed` row is reported
    settled by `status` and by the digest, phrased as the review proposed:
    ``Phase N now reads `owed` in the plan's `## Implementation log` ``.
  - The README's lines about what the digest shows say `done`, **or** `owed` on a phase marked
    `Blocks merge: no`.
  - `node --test tools/conductor/test/` passes, with the digest case failing against today's code.

### Phase 3 — A run refuses to start stale
- **Owner skill:** dev
- **What:** the conductor compares the `tools/conductor/` sources it loaded against what is on disk,
  and says so rather than executing yesterday's logic.
- **Files touched:** `tools/conductor/conductor.mjs`, `tools/conductor/lib/`,
  `tools/conductor/test/`, `tools/conductor/README.md`.
- **Done when:**
  - A **live run** whose `tools/conductor/` sources have changed on disk since it started prints one
    line naming that fact and pauses in `pause`'s existing shape — the plans in flight finish and no
    other starts. It does **not** restart itself, and it does not kill a session.
  - `resume`, `park` and `status` each print the same notice when the running process is stale, so
    the next refusal a person reads is not mistaken for a real one. This is the case that cost the
    session: `resume 0220` was refused by stale code with a message that was true of the old logic.
  - Staleness is decided by content, not by timestamp — a hash of the loaded module set, recorded at
    start and re-read on each look — so a `git checkout` that restores identical bytes is not stale.
  - `node --test tools/conductor/test/` passes with a case that mutates a source mid-run.

### Phase 4 — The allowlist admits what the tools already grant
- **Owner skill:** dev
- **What:** the read-only commands a session is denied while holding the same capability through its
  tools.
- **Files touched:** `tools/conductor/settings.conductor.json`, `tools/conductor/README.md`.
- **Done when:**
  - `Bash(ls *)`, `Bash(printenv *)`, `Bash(grep *)` and `Bash(sed -n *)` are allowed, and the README
    states the principle: **a command is admitted when the session already holds that capability
    through `Read`, `Glob`, `Grep` or `Write`, and refused otherwise.** Admitting them buys turns,
    not power.
  - `Bash(cp *)`, `Bash(mv *)` and `Bash(gh *)` stay **refused**, each with one clause saying why —
    the first two because `Write` is the reviewed path for creating a file and a shell copy is how a
    session sidesteps it, and `gh` because it reaches the network and authenticates as the owner.
  - No entry is added for a command nothing was denied on. The roster grows from the record, not
    from imagination, and the log names the eight denials it was built from.

## Risks & open questions

- **Phase 4 is the one with a judgement in it.** It widens what an autonomous session may run, and
  the argument that a capability is already held through a tool is a claim about the tools, not a
  proof. If the owner disagrees with the principle, the phase is where to say so — approving this
  plan is that decision point, and reversing it later is a one-line edit per entry.
- **Phase 1 depends on `nextest`'s output shape.** A parser keyed to `FAIL [` breaks silently if that
  changes. The done-when's recorded fixture is what makes the break visible.
- **Phase 3 could be noisy in a checkout that rebuilds often.** Content hashing rather than mtime is
  what keeps it quiet; if it still fires spuriously, the reading to take is which file changed.

## What this plan does NOT do

- **It does not make the conductor restart itself.** Stated in the Decision: an auto-restart loses
  in-flight sessions, which is the harm ADR-0205 already refuses for backgrounded work.
- **It does not change what a gate runs, or when.** Phase 1 changes only what a record keeps.
- **It does not widen the allowlist beyond the enumerated read-only set**, and it adds nothing for a
  command that was never denied.
- **It does not fix the flake** that prompted Phase 1. Naming the test is what makes that possible;
  the test itself is unidentified and stays that way until the next occurrence.

## Implementation log

> Written by `dev` — one row per phase as that phase's commit lands, and the close block after the
> last one. **The phases above are the contract; everything here is what happened.**

**Lane:** `plan-0229-the-conductor-reports-itself-honestly` in `/home/igor/Work/rlx-plan-0229`

| phase | owner | state | commit |
|---|---|---|---|
| 1 — The ledger names what failed | dev | done | `0b08037a` |
| 2 — One reader decides a settled phase | dev | done | `8bf46530` |
| 3 — A run refuses to start stale | dev | done | `3f151a49` |
| 4 — The allowlist admits what the tools already grant | dev | done | `6d2d252a` |

### Notes

- Phase 1: the record is written by `appendRecord`/`appendServed` in `lib/ledger.mjs`, and the red
  gate's detail by `gateDetail` in `lib/lane.mjs`, so both files changed along with `lib/gate.mjs`
  (whose `failingTests` moved into `lib/ledger.mjs`, now de-duplicated because nextest prints each
  failure twice). `suite-record.mjs` is unchanged. The count past the cap is a `failed_count` key
  holding the total. Tests also touched `test/helpers.mjs`, `test/ledger.test.mjs` and
  `test/lane.test.mjs`.
- Phase 2: `git grep -c "donePhases(" -- tools/conductor/lib` reads `close.mjs:1`, `lane.mjs:1`,
  `plan.mjs:3`. The `close.mjs` caller checks that an implement step marked its own phases done and
  the `lane.mjs` one prints phases as their rows turn done; neither decides whether a park settled.
  `plan.mjs` holds `donePhases` itself, `nextStep` and `settledPhase`.
- Phase 3: the module set hashed is `conductor.mjs`, `with-lock.mjs` and every `lib/*.mjs` under the
  tool directory; `scripts/gates.manifest.mjs`, which `lib/gate.mjs` also imports, is outside
  `tools/conductor/` and is not in it. The record is `state/conductor.sources.json`, written at start
  and removed at the run's end, on interrupt and by `abort`. The notice goes to stderr. The new module
  is `lib/sources.mjs`.
- Phase 4 was built from these eight denials: `ls`, `sed`, `grep`, `cp`, `printenv`, a bare binary
  path, `git grep -E` with an alternation, and `gh`. Four are admitted (`ls *`, `printenv *`,
  `grep *`, `sed -n *`); `cp` and `gh` stay refused, as does `mv`, which was not among the eight; the
  bare binary path and the `git grep -E` alternation get no entry and stay refused, the second because
  the matcher splits the command at the `|` inside the quotes. No deny entry was added: all of these
  are refused by being absent. `test/settings.test.mjs`, outside the phase's file list, gained one case
  per denial, because its roster test fails on a rule with no case.
- `sed -n *` also admits sed's `w` and `e` commands, which write a file and run a shell command.
  `Bash(node *)` was already allowed.
- Round 1 finding 0 (major, progress counter kept in failing names; fixture hand-written): f1994976.
  The fixture was recorded from a scratch crate under the lane's gitignored `target/`, run through
  the suite lock on cargo-nextest 0.9.143.
- Round 1 finding 1 (minor, `sed -n` is not a boundary; README clause): 53089d56.
- Round 1 finding 2 (minor, `parkStillTrue` refusal names `owed`): d63afa27.

### Close triggers

- **`presets/` touched:** no
- **Plan header `Closes:`** none
- **What shipped:** feature - conductor tooling only (`tools/conductor/`); no Rust, C++ or studio code
- **Operator docs touched:** `tools/conductor/README.md`
- **Backlog probes (`node scripts/check-backlog-claims.mjs`):** exit 0 - 46 stated reductions hold
  across 21 live entries (4 unprobeable)
- **Full suite:** owed to the conductor's pre-review gate (ADR-0207). `node --test tools/conductor/test/`
  at the Phase 4 tip: 450 tests, 448 pass, 0 fail
- **Outstanding `human` phases:** none

## Close review

The round 2 review, graded at `0f75a8a3f78051e50e1b89cbe6146ba14d34f77a`, in full:

> # Plan 0229 — close review, round 2
>
> Graded at `0f75a8a3f78051e50e1b89cbe6146ba14d34f77a` on `plan-0229-the-conductor-reports-itself-honestly`
> (`/home/igor/Work/rlx-plan-0229`), with `main` already merged in by the conductor.
>
> **Verdict: Plan 0229 is clean. The fix round resolved all three round-1 findings. Every gate is
> green, and there are no blockers, majors, minors or nits.**
>
> ## Evidence
>
> - **Full suite**: `node ".../with-lock.mjs" suite -- cargo nextest run --workspace` ran in full in
>   this session. It did not skip. Result: `Summary [ 409.417s] 1805 tests run: 1805 passed (3 slow), 7 skipped`,
>   and the wrapper reported `with-lock: "suite" waited 0.0s, held 409.8s`. The output contains no
>   `FAIL` line.
> - **Rustdoc**: `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` passed.
> - **Conductor tests**: `node --test tools/conductor/test/` ran 451 tests: 449 passed, 0 failed,
>   2 skipped. That is round 1's 450 tests plus the new progress-counter case.
> - **Doc links**: `node scripts/check-doc-links.mjs` is OK (567 files).
> - **Tree**: `git status --short` was empty after every run.
>
> ## Round-1 findings, re-checked against the tree
>
> 1. **major: the parser kept nextest's progress counter; the fixture was hand-written.** Resolved in
>    `f1994976`.
>    - `failingTests` (`tools/conductor/lib/ledger.mjs:226`) now drops an optional
>      `\(\s*\d+/\d+\)\s+` after the duration bracket.
>    - `RED_NEXTEST_OUTPUT` (`tools/conductor/test/helpers.mjs:128`) is now a real recorded run from
>      cargo-nextest 0.9.143, taken through the wrapper's pipe. It has the counters, the
>      `stdout ───` / `stderr ───` sections, and the `Summary` block that reprints both failures. That
>      is the same shape as this review's own suite output.
>    - A new case in `ledger.test.mjs` feeds the same failure three ways: with a padded counter, with a
>      different counter, and with no counter. It asserts that exactly one bare name comes back.
>    - The cases in `with-lock.test.mjs` and `ledger.test.mjs` now assert the recorded summary line and
>      both names.
>    - `gate.mjs:209` and `with-lock.mjs:357` both call this one parser, so both record writers get the
>      fix.
>    - The repository's nextest config sets no retries, so the `TRY n FAIL` line shape cannot appear
>      here and needs no case.
> 2. **minor: the README implied `sed -n` was a boundary.** Resolved in `53089d56`. The README now
>    carries the suggested clause almost word for word: `sed -n` carries `e` and `w`, and the refusals
>    of `cp`, `mv` and `gh` steer a session to the reviewed tool rather than fence it.
> 3. **minor: `parkStillTrue`'s refusal did not mention `owed`.** Resolved in `d63afa27`. The refusal
>    at `tools/conductor/lib/lane.mjs:130` now reads
>    `... still not marked done (or owed, on a phase marked Blocks merge: no) ...`, and the regex in
>    `lane.test.mjs` is updated to match.
>
> The implementation log's notes name all three fixes and their commits.
>
> ## Lenses
>
> - **Lens 1, alignment**: round 1's reading of Phases 1 to 4 still holds. The fix round changed only
>   what those three findings named. Every phase has exactly one `dev` owner tag, and the plan has no
>   `human` phases. The log is shorter than the phases section.
> - **Lens 2 and lens 5, layering and design**: no Rust, C++ or studio code changed. There is still
>   one parser and one settled-phase reader.
> - **Lens 3, docs and bookkeeping**: `tools/conductor/README.md` is current. The close still owes:
>   - the version bump (conductor tooling feature; the close picks the level);
>   - the `done/` move and the `Status:` flip;
>   - the plans index;
>   - the `## Close review` section, carrying the three round-1 rows above with their fix commits.
>
>   No ADRs pair with this plan, and it touched no `presets/`.
> - **Lens 4, correctness**: the parser now reads the shape this repository's nextest actually prints.
>   The fixture is a recording, so a future change to nextest's output shape shows up as a failing
>   test, which is what the plan's Risks section asked for.
>
> ## Findings
>
> None.

Findings an earlier round raised and a fix round resolved:

- Round 1, major: `failingTests` kept nextest's progress counter in a failing name, and the fixture
  was hand-written. Fixed in `f1994976`.
- Round 1, minor: the conductor README implied `sed -n` was a boundary. Fixed in `53089d56`.
- Round 1, minor: `parkStillTrue`'s refusal did not mention `owed`. Fixed in `d63afa27`.

Close notes: version **none**. Conductor tooling ships in no artifact, which is the 0191, 0226 and
0228 precedent. No paired ADR, no `presets/` change, no backlog entry closed. Backlog probes: 46 reductions hold
across 21 live entries. Translation advisory: `how-it-works.ru.md`, `running.ru.md` and the foobar
`READ-ME-FIRST.ru.md` are behind their sources, none moved by this plan.

## Followups (after this lands)

- The unidentified flake from 2026-09-24. Phase 1 makes the next occurrence name itself; nothing here
  finds this one.
- Whether a plan touching `tools/conductor/lib/` should end by restarting the run, once Phase 3 makes
  the staleness visible rather than silent.
