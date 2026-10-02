# 0242 — Readiness is read when the plan is approved, and a judgement phase defaults to owed

> **Status:** done (closed 2026-10-01 by a conductor close; Phase 3 applied by the owner 2026-10-02). Phase 1 `7c3dbffe`, Phase 2 `47b75ecf`; Phase 3, the owner's two `.claude/` edits, landed after the merge. Round 1 review: no blockers, no majors, one minor (applied with Phase 3), one nit (open). Full suite green via the suite ledger. Version 0.160.0.
> **Created:** 2026-10-01
> **Owner skill(s):** dev, human
> **Related ADRs:** [ADR-0248](../../adrs/0248-the-pipeline-repairs-before-it-parks.md), [ADR-0249](../../adrs/0249-a-human-phase-may-be-owed-after-the-merge.md), [ADR-0210](../../adrs/0210-a-claude-repair-is-the-owners-and-a-session-that-needs-one-parks-with-the-edit.md)

## TL;DR

A new `conductor readiness NNNN` command runs ADR-0248's readiness check from the main checkout at
approval time, while the architect and the owner are still in the session that wrote the plan. The
`ready` verdict it records against the plan's contract hash is the one the lane reads later, so the
lane does not run the check a second time. The same check also prints **advisories** that never park.
The main one names a blocking `human` phase that no later phase reads, which is a candidate for
`Blocks merge: no`. Last, the architect skill and the plan template make `Blocks merge: no` the
default for a judgement or on-device phase.

## Context & problem

**`plan_wrong` is caught late.** In the 2026-09-22 to 09-30 run record, 9 parks were `plan_wrong`,
waiting 36 h in total. The readiness check catches them cheaply: 23 sessions averaged 0.8 min. It
runs when the conductor reaches the plan, which is often hours after approval, so each catch is a
park the owner comes back to. Plan 0212 alone parked three times in a row this way, on a venv python,
a live windowed player and a missing seam. Each is a one-minute edit while the plan's author is still
in the room.

**Judgement phases still block the merge.** `human_phase` parks waited 186 h, the largest single
cause. ADR-0249 has let a judgement phase be owed after the merge since 2026-09-24, but 7
human-phase parks came after that date. One example is 0212's Phase 3, "a full track, judged", which
nothing after it reads. The field is available. The authoring habit has not caught up with it.

## Decision

Run the existing readiness session earlier and keep its verdict, rather than writing a second checker.
Make the cheaper park-free outcome the default at authoring.

**Rejected alternatives:**
- **A static Node linter for plans:** most `plan_wrong` findings are semantic. One example is "this
  seam is outside Files touched", which only a reading session sees.
- **Keep readiness in the lane and resolve the plan faster:** that does not move the catch to where
  the author is.
- **Have the conductor flip human phases to non-blocking automatically:** whether a phase's output
  is an input is the plan author's judgement (ADR-0249).

## Architecture diagram

```mermaid
sequenceDiagram
    participant A as architect session (main checkout)
    participant C as conductor readiness NNNN
    participant R as readiness session (read-only)
    participant F as state/readiness.jsonl
    participant L as lane (later run)
    A->>C: after committing the plan
    C->>R: plan file, main checkout, allowlist
    R-->>C: ready + advisories, or parked plan_wrong
    C->>F: {plan, hash, verdict, advisories}
    C-->>A: verdict printed; fix in-session
    L->>F: ready record with the same contract hash?
    F-->>L: yes: skip the readiness session
```

## Implementation phases

### Phase 1 — `conductor readiness NNNN` runs the check at approval
- **Owner skill:** dev
- **What:**
  - **The command:** a `readiness` entry in `conductor.mjs`'s `COMMANDS`. It refuses a plan file
    that is uncommitted or dirty in the main checkout, because the hash must be of committed text. It
    then runs the readiness session through the same `session` path the lane uses, with `lane` set to
    the main checkout. It checks that `HEAD` and `git status --porcelain` are unchanged across the
    session, prints the verdict, and appends `{plan, hash, verdict, detail, at}` to a new append-only
    `state/readiness.jsonl`.
  - **Next to a live run:** the command is allowed while a `run` is live, because it writes no
    `conductor.json` state.
  - **The lane's side:** `readiness()` in `lib/lane.mjs` first looks for the newest
    `state/readiness.jsonl` record for the plan whose `planContractHash` matches. A `ready` record
    skips the session and is copied into `rec.readiness`. Any other record, or none, runs the session
    as today.
- **Files touched:** `tools/conductor/conductor.mjs`, `tools/conductor/lib/lane.mjs`,
  `tools/conductor/lib/state.mjs` (the readiness file's path in `statePaths`, plus read and append),
  `tools/conductor/prompts/readiness.md` (the lane line may name the main checkout),
  `tools/conductor/test/cli.test.mjs`, `tools/conductor/test/lane.test.mjs`,
  `tools/conductor/README.md` (Commands table, plus one paragraph under "Before the first run").
- **Done when:** the following tests hold (each runs against the fake CLI in `test/fake-claude.mjs`),
  and `node --test tools/conductor/test/*.test.mjs` passes:
  - **Records ready:** `readiness 0999` against a committed fixture plan records one `ready` line with
    the plan's contract hash.
  - **Refuses dirty:** the same command on a dirty plan file exits non-zero and records nothing.
  - **Lane skips:** a lane test whose readiness file carries a matching `ready` record reaches the
    first implement session with no readiness step in `rec.steps`.
  - **Edited plan:** a mismatched hash, as after a plan edit, runs the readiness step as before.
  - **Moved checkout:** a readiness session that leaves the main checkout's status changed is
    reported as a disagreement and records nothing.

### Phase 2 — Readiness prints advisories that never park
- **Owner skill:** dev
- **What:**
  - **The outcome:** the `ready` outcome accepts an optional `advisories` array of one-line strings,
    validated in `lib/outcome.mjs`.
  - **The prompt:** `prompts/readiness.md` asks for exactly two advisory kinds:
    - a `human` phase with no `Blocks merge: no` whose output no later phase reads;
    - two or more adjacent `human` phases, so the owner knows they will be asked for together.
    The prompt says an advisory never parks.
  - **Where they show:** the `readiness` command prints the advisories, the lane prints them to
    `live.log`, and the digest carries them under the plan's entry until the plan merges.
- **Files touched:** `tools/conductor/prompts/readiness.md`, `tools/conductor/lib/outcome.mjs`,
  `tools/conductor/conductor.mjs`, `tools/conductor/lib/lane.mjs`, `tools/conductor/lib/digest.mjs`,
  `tools/conductor/test/outcome.test.mjs`, `tools/conductor/test/digest.test.mjs`.
- **Done when:** the following tests hold, and `node --test tools/conductor/test/*.test.mjs` passes:
  - **Parses:** an outcome test accepts `{"kind":"ready","plan":"0999","advisories":["Phase 3 ..."]}`
    and still accepts a `ready` with no advisories.
  - **Rejects bad input:** it rejects an `advisories` value that is not an array of strings.
  - **Shown:** a digest test shows an advisory under its plan.
  - **Does not block:** an advisory never changes the next step.

### Phase 3 — The architect runs it, and the template defaults judgement phases to owed
- **Owner skill:** human
- **Blocks merge:** no
- **What:** the owner applies two edits under `.claude/`, which a headless session cannot write
  (ADR-0210), in an interactive session. Nothing in this plan reads the result.
  1. **In `.claude/skills/architect/SKILL.md`, Mode 1 Step 3:** after the paragraph that begins
     "**After writing the plan, update `docs/plans/README.md`**", insert:

     > **A plan the conductor will run is read for readiness before it is approved.** Commit it,
     > then run `node tools/conductor/conductor.mjs readiness NNNN` from the main checkout. A
     > `plan_wrong` verdict is fixed in this session, committed, and the command re-run. Each
     > advisory is answered in this session, either by changing the plan or by a one-line reason in
     > the reply to the owner. The verdict is recorded against the plan's contract hash, so the lane
     > does not repeat it unless the plan changes.

  2. **In `.claude/skills/architect/references/templates/plan.md`:** replace the line
     `- **Blocks merge:** no  _(human phases only, and only when nothing after it reads its output; otherwise omit)_`
     with:

     `- **Blocks merge:** no  _(human phases only. The default for a judgement, an on-device check or a rig session: write it unless a later phase reads this phase's output. Omit it on an input such as a certificate, a corpus or a threshold measurement.)_`

- **Files touched:** `.claude/skills/architect/SKILL.md`,
  `.claude/skills/architect/references/templates/plan.md`.
- **Done when:** both edits are committed on `main`, and `node scripts/check-doc-links.mjs` exits 0.

## Risks & open questions

- **The main checkout is where the owner works.** A readiness session there is read-only, but it
  shares the tree with whatever else is going on. Phase 1 compares `git status --porcelain` before and
  after, rather than requiring a clean tree. An unrelated edit by a parallel session during those
  48 seconds would read as the session's own change and refuse the record. That is the safe
  direction: re-run the command.
- **The contract hash must be the lane's hash.** A lane branched from a `main` that carries the
  committed plan holds byte-identical plan text. A plan amended on `main` after the lane opened is
  re-read, which is right: that is exactly an edited plan.
- **Spend moves earlier.** A plan read at approval and then abandoned has spent about $0.70 it would
  not have spent. That cost is small next to what it saves.

## What this plan does NOT do

- **It does not re-mark the human phases of the already approved plans** (0232, 0235 to 0240). Doing
  so is an amendment for the owner and the architect. Running the new command over each of them
  after Phase 2 lands lists the candidates.
- **It does not change what readiness grades.** The five checks in ADR-0248 stay as they are.
- **It does not batch parks.** That is Plan 0241 Phase 2.

## Implementation log

**Lane:** `plan-0242-readiness-is-read-when-the-plan-is-approved` in `/home/igor/Work/rlx-plan-0242`

| phase | owner | state | commit |
|---|---|---|---|
| 1 — `conductor readiness NNNN` runs the check at approval | dev | done | `7c3dbffe` |
| 2 — Readiness prints advisories that never park | dev | done | `47b75ecf` |
| 3 — The architect runs it, and the template defaults judgement phases to owed | human | done | applied on main 2026-10-02, with review finding m1 |

### Notes

- Phase 1: `readiness NNNN` runs its session against a scratch record under `state/readiness/`
  (`statePaths().readinessScratch`): that record has its own `conductor.json`, transcripts, prompts
  and hook logs. The live run's `state/conductor.json` is never written.
- Phase 1, **Moved checkout**: the test runs a scenario module that the test writes into its own
  scratch tool directory. The module wraps `lane-scenario.mjs` and leaves an untracked file behind.
  `lane-scenario.mjs` itself is unchanged.
- Phase 2: `tools/conductor/test/lane.test.mjs` is touched although Phase 2's `Files touched` does not
  list it. **Does not block** is a lane test there: a `ready` with an advisory, recorded at approval,
  runs the same steps. It also checks that the lane prints the advisory to the run terminal.
- Phase 2: the readiness command's printing of advisories has no test of its own. A test would need
  the fake session in `lane-scenario.mjs`, which neither phase lists, to emit them.
- Phase 2: in the digest, advisories are a bullet of their own per plan in `## Needs you`, named by the
  plan number and counted in the summary line. They do not sit inside the park bullet or the `## Now`
  lane line.

### Close triggers

- **`presets/` touched:** no
- **Plan header `Closes:`** none (the header has no `Closes:` line)
- **What shipped:** feature (conductor tooling: a new `readiness` command and readiness advisories)
- **Operator docs touched:** `tools/conductor/README.md` (Commands table row, plus a paragraph under
  "Before the first run")
- **Backlog probes (`node scripts/check-backlog-claims.mjs`):** exit 0, 55 reductions hold across
  27 live entries, 3 unprobeable; no entry named as failing
- **Full suite:** owed to the conductor's pre-review gate (ADR-0207).
  `node --test tools/conductor/test/*.test.mjs` passed at both phases.
- **Outstanding `human` phases:** Phase 3, which is marked `Blocks merge: no`

## Close review

Round 1 is the only round, and no earlier round raised a finding that a fix round resolved. **Phase 3
is owed** (ADR-0249). It has not yet put the readiness step into the architect skill's Mode 1, and it
has not made `Blocks merge: no` the template's default for a judgement phase. Until the owner applies
both edits on `main`, nothing tells an architect session to run `conductor readiness NNNN`.
Neither finding below was repairable at the close. m1 is under `.claude/` (ADR-0210), and n1 asks
for a test, which is code. Both stay open.

The review, verbatim:

> # Plan 0242 — close review, round 1
>
> Graded at tip `92646fbe8004085fce0674da478c9b6f374c564a` on the lane
> `plan-0242-readiness-is-read-when-the-plan-is-approved` (`/home/igor/Work/rlx-plan-0242`). The lane
> already contains `main`.
>
> **Verdict: Plan 0242 landed cleanly. There are no blockers and no majors, one minor (under `.claude/`,
> so the owner applies it) and one nit.** Phase 3 is a `human` phase marked `Blocks merge: no`. It is
> correctly logged as `owed` and does not block the close.
>
> ## Evidence
>
> - **Full suite:**
>   `node ".../with-lock.mjs" suite -- cargo nextest run --workspace` printed
>   `with-lock: skipped cargo nextest run --workspace: tree 54a4d6f is green in the suite ledger, run by gate 0242-pre-review at 2026-10-01T19:06:30.555Z: 1858 tests run: 1858 passed (5 slow), 84 skipped`.
>   That ledger record is the full-suite evidence for this tip (ADR-0207). The plan touches no Rust.
> - **`RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`:** clean.
> - **`node --test tools/conductor/test/`:** 505 tests, 503 pass, 0 fail, 2 skipped. The two skips are
>   Windows-only.
> - **`node scripts/check-doc-links.mjs`:** OK. **`node scripts/check-claude-declarations.mjs`:** OK.
>   Phase 3 declares both of its `.claude/` paths.
>
> ## Lens 1 — Alignment with the plan
>
> The implementation log maps Phase 1 to `7c3dbffe` and Phase 2 to `47b75ecf`, and lists Phase 3 as
> owed. Its notes are accurate when checked against the diff. The log is shorter than the phases
> section. Every phase has a single owner tag from the allowed set.
>
> **Phase 1:**
> - `cmdReadiness` in `tools/conductor/conductor.mjs` is registered in `COMMANDS` and the usage string.
> - It refuses a plan that is untracked, via `ls-files --error-unmatch`. It also refuses one that is
>   dirty or only staged, via a pathspec'd `status --porcelain`.
> - It runs the lane's own `session()` through `approvalReadiness`. That session runs against a scratch
>   record under `state/readiness/`, so the live `conductor.json` is never written. This is why the
>   command may run beside a live `run`.
> - It compares `HEAD` and `status --porcelain --untracked-files=all` before and after the session, and
>   appends `{plan, hash, verdict, detail, at}` to `state/readiness.jsonl`.
> - `readiness()` in `lib/lane.mjs` now uses the newest record on the matching hash first. Only a
>   `ready` record skips the session, so a newer `plan_wrong` on the same text still forces one.
> - The five done-when tests exist and assert the claims the plan makes:
>   - `cli.test.mjs`: one `ready` line on the plan's hash, and no `conductor.json` written.
>   - `cli.test.mjs`: a dirty plan exits non-zero and records nothing.
>   - `cli.test.mjs`: a stray file left by the session is a disagreement and records nothing.
>   - `lane.test.mjs`: a matching hash makes `kinds()` exactly implement, review, close.
>   - `lane.test.mjs`: an amended, committed plan runs the readiness step first, and its record has no
>     `approval` flag.
>
> **Phase 2:**
> - `lib/outcome.mjs` accepts `advisories` on a `ready` outcome only as an array of non-empty strings
>   with no newline. `outcome.test.mjs` covers acceptance and rejection, including `""`, a multi-line
>   string, `null` and a non-array.
> - `prompts/readiness.md` asks for exactly the two advisory kinds and says an advisory never parks.
> - The command prints advisories. The lane prints them to `live.log`.
> - The digest shows them in their own `## Needs you` bullet, and counts them, until the plan merges.
>   `digest.test.mjs` asserts the bullet, the sub-line, and that they are gone after the merge.
> - **Does not block:** `lane.test.mjs` asserts that a `ready` with an advisory runs the same steps and
>   still parks `human_phase` at the human phase.
> - `lane.test.mjs` is touched although Phase 2's *Files touched* does not list it. The log says so,
>   and Phase 1 already lists the file. Nothing is owed for this.
>
> **Phase 3** is owed after the merge (ADR-0249). Its replacement text is written out in the plan.
>
> ## Lens 2 — Layering, coupling, real-time safety
>
> The plan changes conductor tooling only. It touches no core, ABI or protocol code, and no new
> dependency. `approvalReadiness` reuses `session()` rather than a second session path, which is what
> the plan's Decision chose.
>
> Spend from an approval-time session lands in the scratch record, not the run's record. So it is
> bounded by `budget_usd.readiness` per session, not by `run_budget_usd`. That matches the plan's
> "spend moves earlier" risk and is not a finding.
>
> ## Lens 3 — Docs and bookkeeping
>
> - `tools/conductor/README.md` gains the Commands row and the paragraph under "Before the first run".
> - The architect skill's conductor-mode `readiness` paragraph has not been updated (finding m1).
> - **Version bump owed at close:** this is a feature plan (a new conductor command), so a minor bump is
>   likely. The level is the close's call.
> - Phase 3 is owed. The `Status:` line, the `## Close review` and the plans index bullet must name it
>   (ADR-0249).
>
> ## Lens 4 — Correctness
>
> **The hash keys the same text in both places.** `planContractHash` is computed on the committed
> main-checkout file at approval. It is computed again on the lane's copy, which a lane branched from
> `main` holds byte-identical. A plan amended on `main` and merged into the lane gets a new hash and is
> read again, as the plan intends.
>
> **A `disagreement` is never recorded, and nothing else is ever trusted.** A non-`ready` outcome kind
> and a moved checkout both return without appending. A parked session's reason is recorded, but the
> lane trusts only `ready`. This is safe, including for `budget`/`api` parks.
>
> ## Lens 5 — Design integrity
>
> The design is sound. The approval-time path and the lane path share `session()`, the prompt and the
> outcome validator. The only new state is an append-only file with its own reader.
>
> ## Findings
>
> ### minor
>
> **m1 — `.claude/skills/architect/SKILL.md:958`: the skill's conductor-mode `readiness` section does
> not know about advisories or the main-checkout case.**
> - **What is missing:** the paragraph still describes readiness as running only "before the plan's first
>   implement session", and its outcome as `ready` or `plan_wrong`. A session started by
>   `conductor readiness NNNN` learns about the main checkout and the two advisory kinds only from
>   `prompts/readiness.md`.
> - **Why it matters:** the skill and the prompt describe one session in two places, and right now only
>   the prompt is current.
> - **Fix:** headless sessions cannot edit `.claude/` (ADR-0210), so the owner applies it, ideally in the
>   same interactive sitting as Phase 3. After the line ending "Your verdict is the owner's to overrule:
>   they edit the plan or resume.", insert this new paragraph:
>
>   > The same session also runs at approval, from the main checkout, under
>   > `conductor readiness NNNN`. There the tree may carry the owner's uncommitted changes: leave every
>   > one exactly as you found it, because the conductor compares `HEAD` and `git status --porcelain`
>   > before and after. A `ready` may carry `advisories`, one-line notes that never park, of exactly two
>   > kinds: a `human` phase without `**Blocks merge:** no` whose output no later phase reads, and two
>   > or more adjacent `human` phases. Name the phase in each.
>
> ### nit
>
> **n1 — `tools/conductor/conductor.mjs:741`: nothing tests that `cmdReadiness` prints advisories.**
> - **What is missing:** the `for (const a of r.advisories) o.log(...)` line has no test. The
>   implementation log records this as a known gap.
> - **Fix:** add a `cli.test.mjs` case that uses a wrapped scenario module, in the same pattern as the
>   "Moved checkout" test, to emit a `ready` with one advisory. Assert that the output contains
>   `advisory (never parks):`. A wrapper written by the test means `lane-scenario.mjs` itself does not
>   need to change.
