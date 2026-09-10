# ADR-0182 — A plan lane may live inside the repository, and a tool that enumerates from git already knows it

> **Status:** accepted 2026-09-10
> **Date:** 2026-09-10
> **Related plan(s):** [0165-the-release-path-stops-being-the-first-compile](../plans/done/0165-the-release-path-stops-being-the-first-compile.md) Phase 0
> **Supplements:** [ADR-0053](0053-plan-lanes-run-in-git-worktrees.md)

## Context

[ADR-0053](0053-plan-lanes-run-in-git-worktrees.md) put plan lanes in git worktrees at
`WORK/rlx-plan-NNNN` — **siblings of the repository, outside it** — and the close ceremony's
`git worktree remove ../rlx-plan-NNNN` assumes that shape. Every repository tool was written under
it, where a walk from the repo root cannot reach another lane by construction.

The harness now opens lanes **inside** the repository. On 2026-09-10 three lanes were live in two
shapes at once: `WORK/rlx-plan-0159` and `WORK/rlx-plan-0161` outside, and
`.claude/worktrees/plan-0161-structural-hold` inside, the last one `locked` to a session by pid.
The inside shape is not a mistake to be corrected — it is how the tooling creates a lane, and a
convention the tooling violates by default is not a convention. So the question is what the
repository's own checks owe a second checkout sitting under its root.

The answer turns out to be *almost nothing*, because this repository already made the decision
once, for a different reason. `toc.mjs`, `check-doc-links.mjs` and `check-comment-hygiene.mjs`
enumerate their inputs from **`git ls-files`**, falling back to a filesystem walk only when git
cannot answer, and they say which source they used (ADR-0016's shape). The stated reason is CI
parity — *"a filesystem walk cannot tell documents we own from a gitignored vendored README sitting
in the working tree, which is present locally and absent from CI's fresh clone"*. A nested lane is
exactly that: present locally, absent from CI, and untracked. The tracked-set enumeration excludes
it for free.

One gate never adopted it. `check-index-rows.mjs` walks the filesystem unconditionally, and it
therefore reads the **seeded red fixture** inside the nested checkout — whose skip is anchored to
the real repo root's absolute path and so cannot match a second copy — and convicts. That gate runs
at `.githooks/pre-push:147`, so a clean main checkout could not be pushed while a nested lane
existed, with nothing wrong in the pushed tree and CI green. `check-filter-figures.mjs` also scans
the working tree, deliberately, but leaves the exit code to the tracked half, so it can report a
nested hit without failing on one.

## Decision

A plan lane may live inside the repository. Both shapes are supported: ADR-0053's
`WORK/rlx-plan-NNNN` sibling and the harness's `.claude/worktrees/<name>/`.

The obligation that comes with it belongs to the tools, stated as one rule: **a check enumerates
the files it owns from `git ls-files`, and falls back to a filesystem walk only when git cannot
answer.** That is already the convention in three of this repository's five full-tree walkers; this
ADR makes it the rule rather than a habit, and `check-index-rows.mjs` adopts it in Plan 0165 Phase
0. A nested checkout is untracked, so it disappears from every such enumeration without any tool
needing to know what a worktree is.

We also commit `.claude/worktrees/` to `.gitignore`, where the shape is visible to every clone
rather than living only in a machine-local `.git/info/exclude` the harness happens to write.

## Consequences

### Positive
- The nested lane shape is supported rather than tolerated, and the close ceremony can keep using
  whichever shape a session was opened in.
- One rule covers every future check, and it is the rule three checks already follow. A new gate
  that enumerates from git is immune to this class before anyone thinks about worktrees.
- `check-index-rows.mjs` gains CI parity as a side effect: it currently measures gitignored files a
  fresh checkout would never hold, which is the same defect the rule was written for.
- The fix is one script rather than five, and adds no per-directory filesystem probe to any walk.

### Negative
- **A check that must read untracked files cannot use the rule**, and `check-filter-figures.mjs` is
  the live example: it scans the working tree on purpose, because a figure pasted into an untracked
  scratch file is still a figure on a second page. It keeps its walk, so it can still *report* a hit
  inside a nested lane. Its exit code is taken from the tracked half, so the report is noise and not
  a conviction — but the noise is real and this ADR does not remove it.
- **A lane inside the repository puts its `target/` inside the repository.** ADR-0053 records ~8 GB
  per lane and a disk filled mid-session; that cost now lands under the repo root, so `du` on the
  checkout stops meaning what it used to, and nothing gates it.
- **The fallback walk is still reachable.** In a tree git cannot answer for, the enumerating checks
  fall back to the walk and the nested lane becomes visible again. The checks announce which source
  they used, so the condition is legible — but it is not impossible.

### Neutral
- `git worktree remove` and the Windows "working directory inside the lane" trap are unchanged by
  this decision; only the path differs. The harness `locked` marker on a session-owned worktree is
  its own mechanism and this ADR neither adds to it nor relies on it.

## Alternatives considered

### Alternative A — Keep lanes outside the repository, and treat an inside lane as user error
ADR-0053 as written. Rejected because the tooling creates lanes inside by default, so the rule
would be violated on the next session rather than followed; and the gate failure it produces is
indistinguishable, to the person being blocked, from a real conviction in their own tree.

### Alternative B — Teach every walk to skip any directory holding a `.git` entry
The rule *do not walk into another checkout*, stated directly, accepting that entry as a file
because a linked worktree's `.git` is a gitlink file. This was the chosen shape until the evidence
came in, and it is rejected now for being a second mechanism that solves a problem four of the five
walkers do not have: three already exclude a nested lane through the tracked set, one must read
untracked files by design and would have to opt out of the new rule, and the fifth is the only one
that needed anything. It also spends a filesystem stat per directory on every walk to answer a
question `git ls-files` has already answered.

### Alternative C — Add `.claude` to each walk's `SKIP_DIRS`
One token per file. Rejected because `check-doc-links.mjs` deliberately covers `.claude/skills/**` —
it found five broken links there the first time it ran — so a name-based skip silences a check that
has already earned its place, and it would bind the fix to one harness's choice of directory.

### Alternative D — Make `check-index-rows.mjs`'s fixture skip relative instead of root-anchored
Smallest possible change to stop the conviction: resolve the seeded-tree skip against the scan root
so a second copy matches it too. Rejected as treating the symptom — the gate would still measure a
nested lane's real rosters, and still disagree with CI about which files exist. It is also *not
wrong*, and it remains the right repair for the seeded trees themselves; it is simply not what this
ADR is about.

## Notes

- The conviction is reproducible only with a nested lane present: `node scripts/check-index-rows.mjs`
  exits 1 and names `.claude/worktrees/<lane>/scripts/fixtures/index-rows-red/roster.md:47`. A fix
  verified against a tree without one has tested nothing.
- Backlog entry 0195 raised this, and was **corrected in place on the same day** — as filed it
  claimed four gates shared the conviction and that `toc.mjs` could rewrite a lane's files. Neither
  survived the reading of how those four enumerate. The entry's history is kept deliberately, as the
  worked example of why ADR-0108 asks a claim about this repository to carry a probe.
