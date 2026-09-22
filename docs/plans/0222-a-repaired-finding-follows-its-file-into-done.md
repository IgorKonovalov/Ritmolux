# 0222 — A repaired finding follows its file into `done/`

> **Status:** draft
> **Created:** 2026-09-22
> **Owner skill(s):** dev
> **Related ADRs:** [ADR-0209](../adrs/0209-a-conductor-close-repairs-the-prose-and-comments-its-findings-name.md) (the `fixed_in` check; not amended), [ADR-0205](../adrs/0205-an-approved-plan-runs-under-a-conductor-and-every-judgement-it-cannot-make-parks-the-plan.md), [ADR-0233](../adrs/0233-a-session-allowlist-safety-claim-is-asserted-against-a-transcript.md)

## TL;DR

The conductor's `fixed_in` check stops calling a correct repair a disagreement when the close then
moves the repaired file. `repairProblems` in `tools/conductor/lib/close.mjs` accepts a finding
whose `file` is the path at the branch tip *or* the path at the `fixed_in` commit, following every
rename git sees between the two, and it recognises the plan's own `docs/plans/` to
`docs/plans/done/` move however much the close grew the file. A second phase makes the three
session prompts spell the shell forms the allowlist already allows, so the four denials 0221's run
hit stop recurring, and it pins those forms in `settings.test.mjs`. No allowlist rule is added.

## Context & problem

The conductor's first Linux run (Plan 0219 Phase 5) parked Plan 0221 as `disagreement`:

> close: finding 0 is fixed_in 19a2f9d6, which does not change
> docs/plans/done/0221-the-arch-block-names-the-studios-settings-file.md

The review session did exactly what ADR-0209 asks. It repaired a nit in the plan file at
`docs/plans/0221-….md` (commit `19a2f9d6`), then wrote the close, which `git mv`s that same file to
`docs/plans/done/` (`7b1a4811`). It then named the finding by its path at the tip, which is the path
the owner can open. `repairProblems` compares `file` against `git diff-tree --name-only <fixed_in>`,
which lists the file at its pre-move path. So **every correct repair of a finding in the plan's own
file reports as a disagreement**, and the plan file is the one file every close moves. The first
close to repair such a finding hit this, and nothing about 0221 was wrong.

The same run hit four permission denials that the sessions worked around:

| Command | Session | Where it came from |
|---|---|---|
| `git -C <lane> add …`, `git -C <lane> commit …` | implement | Habit. The session already runs in the lane, so `-C` is redundant. |
| `awk '…' docs/developing.md \| grep -c …` | implement | **0221's own done-when**, which an architect wrote as a pipe. |
| `env RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` | review | The prompts' *"No environment set by an assignment statement ahead of a command"*. That rule was written against PowerShell's `$env:X = '1'; …`, and it reads as forbidding the POSIX prefix form `RUSTDOCFLAGS=… cargo doc` that the allowlist exists to allow. The session obeyed the prompt and reached for `env`. |

## Decision

**We make the check follow renames, candidate (a), and do not ask the review to name the older
path.** Candidate (b) is rejected for two reasons. A finding's `file:line` is the line the owner
reads in the morning (the digest prints it verbatim), and after a close the older path of a plan
finding names a file that no longer exists. And (b) makes the check's correctness depend on a
session following an instruction, which is the one dependency `close.mjs`'s header rules out: *"the
session's word is never the evidence"*. A finding named by its older path already passes today
and keeps passing, so the review prompt needs no change for this defect. Neither (b) nor (c) is
taken.

The mechanism is a tree-to-tree rename map, not `git log --follow`. The map is
`git diff --name-status -M <fixed_in> HEAD`, and every `R` row pairs an old path with a new one.
It is indifferent to the `git merge main` a close makes between its repair and its bookkeeping,
where `--follow` walks history commit by commit and has to be trusted across that merge. It also catches a rename
the merge brought in from `main`. Git's rename detection needs 50 % similarity by default, and 0221
measured `R062`. The close appends the review in full, so a short plan with a long review drops
below that line, and the plan's own move is therefore also recognised **by construction**: the
conductor knows the plan's path under `done/` (`findPlan`), and the same basename directly under
`docs/plans/` is its pre-move path, whatever the similarity.

**This does not amend ADR-0209.** Its Decision says the commit must *"touch the finding's file"*,
and a file is the same file across a `git mv`. The implementation read "file" as "path string",
which is narrower than the ADR. This is a defect against an unchanged rule, and it needs no new ADR:
the one alternative rejected, (b), is recorded here, and it is a choice about the check's input
rather than about the rule.

**For the denials, the prompts change and the allowlist does not.**
- `git -C` earns no rule. `Bash(git -C * add *)` would let a session stage and commit into *any*
  checkout, the main one included, which is the lane boundary ADR-0205 draws. The prompts say that
  git runs in the lane the session was started in, with no `-C`.
- `awk` earns no rule. `awk` writes files (`print > f`) and runs commands (`system()`), so allowing
  it amounts to allowing any shell command. The prompts send text reading to the Read and Grep tools, or to
  `git grep`, which is allowed.
- `env` earns no rule, because `env *` runs any program. The prompts name the allowed prefix forms
  exactly, `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` and the two
  `RLX_UPDATE_*=1` forms, and they refuse `env`, `export` and `$env:`.

The awk denial has a second carrier in the architect lane. A done-when that a conductor session will
run must be runnable under the allowlist, one command per call. That authoring rule belongs to the
architect skill and is not a phase here.

## Implementation phases

### Phase 1 — The `fixed_in` check follows the file across a rename

- **Owner skill:** dev
- **What:** `repairProblems` accepts a finding when the `fixed_in` commit changes `file`, or changes
  a path that became `file` between `fixed_in` and `HEAD`. There are two sources of such a path: an
  `R` row of `git diff --name-status -M <fixed_in> HEAD`, and the plan's own pre-move path, meaning
  the same basename directly under `docs/plans/` when `file` is the plan's `done/` path. The
  disagreement message names both paths when a rename was followed and still did not match, so a
  real disagreement reads as one. The README's **The checks** paragraph states the rename rule in
  one clause.
- **Files touched:** `tools/conductor/lib/close.mjs`, `tools/conductor/test/close.test.mjs`,
  `tools/conductor/README.md`.
- **Done when:** `node --test tools/conductor/test/` is green, and `close.test.mjs` carries these
  cases against a real git repository built the way the file's existing `lane()` fixture builds one:
  1. **The 0221 shape.** Commit A edits the plan at `docs/plans/0101-fixture.md`. `main` then gains an
     unrelated commit and is merged into the branch, as a close's `git merge main` does. Commit B
     moves the plan to `done/` with `Status: done` and a `## Close review`, and the tip carries an
     annotated tag. A `closed` outcome whose finding names the **`done/` path** with `fixed_in: A`
     verifies with no problems. **Run this case against the unmodified `close.mjs` first and confirm
     it fails with the `does not change` message**, and say so in the log. A regression test that
     never failed is not evidence of the defect.
  2. The same branch, with the finding naming the **pre-move path**: no problems. This already
     passes, and the case pins it.
  3. **Low similarity.** The same shape, but B's `## Close review` is longer than the whole plan was
     at A, so `git diff -M` reports no rename between A and the tip. The case asserts that it does
     not report one, so the fixture is proven to exercise the by-construction branch. The finding on
     the `done/` path still verifies.
  4. **A rename that came in from `main`.** A different file, not the plan, is renamed on `main` and
     merged in, and the repair commit had edited it at its old path. A finding naming the new path
     verifies.
  5. **The check still bites.** `fixed_in` is a commit that changes only `README.md`, and the
     finding names the plan's `done/` path. The result is exactly one problem, `does not change`.
     A follower that accepted any renamed file would pass case 1 and fail this one.

### Phase 2 — The prompts spell what the allowlist allows

- **Owner skill:** dev
- **What:** The shell bullet in `prompts/implement.md` and `prompts/review.md` is reworded, and
  `prompts/fix.md` gains the same bullet, since it has none today. The reworded bullet says four
  things. Git runs in the lane the session was started in and never takes `-C`. The only environment
  prefixes are the exact allowed forms, `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`
  and `RLX_UPDATE_PRESET_SCHEMA=1` / `RLX_UPDATE_PARAM_REFERENCE=1` ahead of `cargo` or `node`,
  while `env`, `export` and `$env:` are refused. Text is read with the Read and Grep tools or
  `git grep`, never `awk`, `sed` or a pipe into `grep`. A done-when written as a pipe is run as its
  parts, or as the equivalent Grep call, and the log says which. `settings.conductor.json` is
  **not** edited.
- **Files touched:** `tools/conductor/prompts/implement.md`, `tools/conductor/prompts/review.md`,
  `tools/conductor/prompts/fix.md`, `tools/conductor/test/settings.test.mjs`.
- **Done when:** `node --test tools/conductor/test/` is green, and `settings.test.mjs`'s own
  `decide` model holds these:
  - **allowed:** `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`, the literal the
    prompts now print, which the test reads *out of* `prompts/review.md` rather than restating. If
    the prompt and the allowlist drift apart, the test goes red.
  - **refused:** `env RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`,
    `git -C /elsewhere add x`, `git -C /elsewhere commit -m x`,
    `awk '/a/,/b/' docs/developing.md`, and the 0221 pipe, whose `awk` part is refused.
  These cases are the four commands `state/transcripts/0221-*.jsonl` records under
  `permission_denials`, which is ADR-0233's shape of a safety claim held to a transcript.

## Risks & open questions

- **A false pairing.** At a lowered similarity, git could pair an unrelated deleted file with the
  finding's added one. Detection stays at git's default, and the only by-construction pair is the
  plan's own, so a false pass needs a default-threshold rename *and* a `fixed_in` commit that edits
  its source, which is accepted.
- **Adopted closes carry no findings.** `adoptedClose` returns `findings: []`, so a close adopted
  through `adopt-close` or the lane-start path never reaches this check at all, and the state loses
  that close's `fixed_in` record. The plan's `## Close review` still holds it. This plan does not
  change that. It is why 0221 can be settled before this plan lands.

## What this plan does NOT do

- It adds no allowlist rule and removes none.
- It does not change the review prompt's instruction on which path a finding names.
- It does not touch Plan 0219 or its log, and it does not queue itself. Whether 0222 runs under the
  conductor or in a human-started `dev` session is the owner's call. Either works, because the
  conductor that runs it loads `close.mjs` from the main checkout, not from the lane.

## Implementation log
