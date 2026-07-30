# ADR-0053 — Plan lanes run in git worktrees, and a close merges main *into* the branch before fast-forwarding main

> **Status:** accepted
> **Date:** 2026-07-30
> **Related plan(s):** none — this records a workflow already in use since Plan 0047
> **Supplements:** [ADR-0005](0005-versioning-and-release-cadence.md) (one version bump per plan
> close), and the close-ceremony section of `.claude/skills/architect/SKILL.md`

## Context

Until Plan 0047 this repository ran one checkout and one plan at a time. `.claude/skills/architect/SKILL.md`
recorded that as a decision — *"this project runs the lightweight harness — no git-worktree parallelism
yet. If that gets added later, it becomes an ADR + extra close-ceremony steps."* That sentence has been
false since 2026-07-30. Plan 0047 was built on `plan-0047-expression-randomness` in a sibling worktree at
`WORK/lmv-plan-0047`, Plan 0048 on `plan-0048-analysis-v2`, and Plan 0049 in that same directory. Three
closes have now run through the worktree flow. This ADR is the one that sentence promised, written late.

The forcing constraint is not parallelism for its own sake. It is that **a plan close touches the version**
(ADR-0005: one `cargo release` per close, never per phase) and the version is a **workspace-global** field
in the root `Cargo.toml`. Two lanes finishing near each other both want to move the same line, and each
wants the `vX.Y.Z` tag on the commit that becomes `main`'s tip. That is a genuine ordering problem, not a
merge-conflict inconvenience, and it is what makes the *direction* of the merge a decision.

It is not hypothetical. Plan 0047's branch sat at `v0.23.0` while `main` had already reached `v0.24.0`
(verified: `v0.24.0` is an ancestor of `24a8011` but not of `24a8011^1`). Had that close bumped from the
branch's own base it would have re-issued `v0.24.0`; merging `main` in first made it the obvious
`v0.24.0 -> v0.25.0`.

Two further forces are specific to this repo and this platform. Rust's `target/` directory is per-worktree
and enormous — the Plan 0049 worktree's was **~8 GB of `target/debug/incremental` alone**, and the disk
filled to zero bytes mid-session, breaking a build. And **the git stash stack is shared across every
worktree of a repository**, so `git stash` / `git stash pop` in one lane can silently take another lane's
entry.

## Decision

We will run each plan in its own **git worktree** on its own `plan-NNNN-<slug>` branch, alongside the
main checkout, and we will close a plan in this order:

1. Merge `main` **into** the plan branch, **from the worktree**. Never update the main checkout's
   working tree from a lane — another session may be live in it.
2. Re-run the full gate (`fmt` + `clippy` + `nextest`) after that merge. It is the first moment the two
   lanes' code has met, and no earlier run covers the combination.
3. Do the close bookkeeping and run `cargo release <level>` **on the branch**, so the version is chosen
   against what `main` actually reached and the `vX.Y.Z` tag lands on the commit that becomes `main`'s tip.
4. `git -C <main checkout> merge --ff-only <branch>` — now, by construction, a clean fast-forward with no
   merge commit on `main`.
5. The user pushes. As everywhere else in this project, no session pushes.

The first-parent history of `main` therefore stays linear, and every `Merge branch 'main' into
plan-NNNN-…` commit lives on the lane where the reconciliation was actually done and tested.

## Consequences

### Positive
- **A close is a fast-forward, so `main` is never the place a conflict is discovered.** Conflicts surface
  on the lane, where the person holding that plan's context resolves them, and the gate runs against the
  resolved tree before `main` sees it.
- **The version level is decided against `main`, late.** Step 1 before step 3 is what makes the bump
  correct; deciding it at branch time would have re-issued a tag on the Plan 0047 close.
- Parallel lanes do not share a `target/` directory, so two sessions building at once do not thrash one
  cargo lock.
- A lane can be abandoned by deleting a directory.

### Negative
- **Disk cost is severe and recurring.** Each worktree carries its own `target/`; the Plan 0049 lane held
  ~8 GB in `target/debug/incremental` alone and the disk reached zero bytes mid-session, breaking a build.
  That cache is regenerable — clearing it is always safe — but it must actually be cleared, and a finished
  lane's worktree should be removed rather than left around.
- **The stash stack is shared across worktrees.** Bare `git stash` / `git stash pop` can pop another
  lane's entry. Use a WIP commit instead; if you must stash, `git stash push -u -m "<unique-tag>"`, record
  the SHA from `git stash list --format='%H %gs'`, restore with `git stash apply <sha>`, and drop the
  entry by re-finding it by tag.
- **The close is now a five-step sequence with a direction that is easy to get backwards**, and getting it
  backwards puts a merge commit and possibly a duplicate version on `main`. It is recorded in the skill and
  here because it will not be remembered.
- On Windows, `git worktree remove` fails with `Permission denied` if **any** process holds the directory —
  including a shell whose working directory is inside it. Move every shell out first. A partially-failed
  removal can leave an empty directory that git has already unregistered.

### Neutral
- `main` gains no merge commits from lanes, but the lanes' `Merge branch 'main' into …` commits do reach
  `main`'s history as second parents. That is the intended record, not noise.
- The worktree directory name need not match the branch: `WORK/lmv-plan-0047` hosted the Plan 0048 and
  0049 branches after Plan 0047 closed. Harmless, but do not read the branch off the path.

## Alternatives considered

### Alternative A — one checkout, switch branches
The status quo the stale SKILL.md sentence describes. Rejected because switching branches invalidates
`target/`, so every switch pays a full rebuild of a workspace whose test suite already runs ~132 s; and
because two agent sessions sharing one working tree can see each other's half-written files. The
`commit-explicit-pathspecs-parallel-sessions` hazard — `git commit` taking the whole index, sweeping in a
parallel session's staged file — is exactly this failure mode, observed before worktrees existed.

### Alternative B — separate full clones per lane
Gives the same isolation with no shared stash stack, which is a real advantage. Rejected on cost and
correctness: a second clone doubles the object store as well as `target/`, on a disk that has already hit
zero; and pushing between clones to reconcile is a strictly worse version of step 4, since `main` would
have to be updated across a remote rather than fast-forwarded locally.

### Alternative C — merge the lane into `main` with a merge commit (`--no-ff`) instead of rebasing history onto it
Rejected because it moves the reconciliation to the wrong side. A merge commit created on `main` is a
merge whose conflict resolution has **not** been through the gate — no `fmt`/`clippy`/`nextest` run covers
a tree that only ever existed as the merge result. Merging `main` into the branch first puts that same
resolution somewhere it can be tested before `main` depends on it, and makes step 4 trivially safe.

### Alternative D — bump the version on `main` after the fast-forward
Superficially simpler: close the plan, fast-forward, then run `cargo release`. Rejected because the tag
would then sit on a commit *after* the plan's own close commit, and because it re-opens the door to the
failure ADR-0005 exists to prevent — a bump that nobody owns, run in whichever session happens to be on
`main`. Bumping on the branch keeps the version squarely inside the close ceremony, where one skill owns it.

## Notes

Written at Plan 0049's close, three plans after the practice began, at the user's request. The gap is
itself the argument for the rule in `CLAUDE.md`: **trust `git` over stale docs** — `git worktree list` was
right about this repository for three plans while the skill doc that claimed to record the decision was
wrong.
