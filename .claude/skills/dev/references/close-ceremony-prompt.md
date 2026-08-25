# The implementation log — field guide

This file used to hold a chat template you filled in and pasted at the end of Step 4. It no longer
does. Since [ADR-0120](../../../../docs/adrs/0120-the-close-brief-is-a-section-of-the-plan.md) the
brief is **a section of the plan** — `## Implementation log`, written as the phases land — and what
you print at Step 4 is a three-line pointer at it.

The *content* of the old template was never the problem; the **medium** was. A chat brief exists
only in one session's scrollback, so a finished lane could not sit on disk and wait its turn, a
session cleared mid-plan destroyed everything the brief would have said, and since
[ADR-0053](../../../../docs/adrs/0053-plan-lanes-run-in-git-worktrees.md) put plan lanes in git
worktrees, the only description of a finished lane lived in a window belonging to a different
session.

## Why the log is thin — the reason, not just the rule

The log carries **your observations, never your conclusions**. It says *where to look*; architect
decides *how it went*.

That is not house style. Architect must arrive at the review with a complete map and **no verdict
pre-formed**, because the entire value of the fresh-session close is that it reaches its own. A
field that tells architect where to look is free of that risk; a field that tells architect how it
went is anchoring — and **a file anchors far harder than a chat message does**, because it is still
there on the second read, it looks like a record rather than a remark, and it travels with the plan
forever. That is why the per-criterion `[pass]`/`[fail]` list the old template carried was cut
rather than moved: it sat at exactly the spot where architect's Mode 4 already warns that a green
`cargo test` is not a passing test, and a column of `[pass]` beside criteria architect is supposed
to independently check is an invitation to skim.

**The counterweight, which matters just as much.** Thinness applies to your *opinions*, never to
your *findings*. A deviation from the plan is **always** disclosed — Mode 4's first lens asks
whether any phase was missing or added without note, and a diff alone cannot answer that, so a
silent deviation turns a review *challenge* into a review *failure*. What gets cut from a deviation
note is only the justification: record **what you did differently**, not **why it was fine**.
Likewise a done-when criterion you could not satisfy as stated is always noted, with what you did
instead. Reporting non-passes only is asymmetric in the right direction — it surfaces every problem
and never pre-certifies a success.

## Why the session must still be fresh

Review is qualitatively different from implementation: it compares code against plan + ADRs, checks
layering and real-time safety, reads test bodies across the whole plan's changes. That is hard in a
context already full of implementation reasoning and tool output. A fresh session forces architect
to re-read the plan and the code with reviewer's eyes. **Nothing about that boundary changed** —
the log makes the handoff durable and asynchronous; it does not automate the seam away, and it does
not let you close your own plan.

## Filling each field

The skeleton lives in `.claude/skills/architect/references/templates/plan.md`. If the plan predates
it, copy the skeleton in rather than skipping the log.

- **`**Lane:**`** — `main` directly, or the worktree path plus its branch (`WORK/lmv-plan-NNNN` on
  `plan-NNNN-<slug>`). Architect's worktree-close sequence takes both from this line instead of
  asking the user, so give it both. Write it on the first phase commit.
- **The table** — `phase | owner | state | commit`. `state` is `done` / `not started` /
  `abandoned`, and **never a quality word**: a phase state is a fact, not an assessment. `owner` is
  the phase's own `**Owner skill:**` tag. The row for the phase committing right now reads
  `committed with this row`; backfill its real SHA on the next phase's commit, and the last row's
  on the close-block commit, so every row ends up carrying one.
- **`### Notes`** — three things belong here, one line each:
  - **deviations from the plan** — what you did differently, and the commit. Required.
  - **done-when criteria you could not satisfy as stated** — by exception only, with what you did
    instead. There is no pass list; silence carries your belief that the rest passed.
  - **followups you noticed and did not act on.**

  Nothing else belongs. Not narrative, not self-assessment, not "nothing notable", not a
  restatement of the phase text. **Empty is a valid answer and needs no sentence saying so.**
- **`### Close triggers`** — raw `git`-derived facts for architect to verify and decide from, one
  bullet per conditional close step, and **no recommendation in any of them**. `presets/` touched
  (which files); the plan header's `**Closes:** design-backlog NNNN` entries; what shipped
  (feature / fix-only / docs-chore-only); which operator docs moved, from Mode 4's sweep table; the
  exit of `node scripts/check-backlog-claims.mjs` and any entry it named; which `human` phases
  remain. **No suggested version bump** — the level is architect's, per
  [ADR-0005](../../../../docs/adrs/0005-versioning-and-release-cadence.md), and a suggestion in
  writing is an anchor.

**Size.** The log stays **shorter than the plan's own `## Implementation phases` section**. It is a
relative property so it scales with the plan, and it encodes that the contract outweighs the report.
Nothing gates it; if it is breached, architect reports it as a `minor`.

**Resume scaffolding.** When a session is cleared or compacted mid-plan, write whatever a resuming
`dev` needs — the diagnosis behind an unfinished symptom, candidate fixes and their costs, the test
and lint state at the tip — and land it as its own `docs(plans): …` commit. That reader is the same
lane continuing, so anchoring does not apply. **Then delete it when the phase it was written for
lands.** Only findings survive to the close.

## The pointer you print at Step 4

Written *after* the close block is committed. Three lines, nothing else:

```
Plan 0001 — <plan title>  (`docs/plans/0001-<slug>.md`)
Lane: `main`   (or: `WORK/lmv-plan-0001` on branch `plan-0001-<slug>`)
Next: start a fresh session and run `/architect close plan 0001`
```

## What NOT to include

- **No per-criterion pass/fail list.** The single most anchoring thing the log could carry.
- **No self-review** ("looks good to me", "landed cleanly") — architect judges.
- **No full diff** — architect reads files; the table already maps phase to commit.
- **No session recap** and no rehash of this session's reasoning. The fresh session is fresh; the
  log is the bridge, not the transcript.
- **No secrets or credentials**, in the log or in any commit message.
