# ADR-0209 — A conductor close repairs the prose and comments its findings name

> **Status:** accepted 2026-09-16 ([Plan 0189](../plans/done/0189-the-conductor-can-be-watched-and-stops-re-proving-a-green-tree.md))
> **Date:** 2026-09-15
> **Related plan(s):** [0189](../plans/done/0189-the-conductor-can-be-watched-and-stops-re-proving-a-green-tree.md)
> **Amends:** [0205](0205-an-approved-plan-runs-under-a-conductor-and-every-judgement-it-cannot-make-parks-the-plan.md)
> (what a close does with the minors it merges)

## Context

A close closes when its review has no blockers and no majors, and minors and nits merge with it. On
2026-09-15 three conductor closes merged seven minors and three nits. Two were repaired in the close
commit; the other eight were left open. The digest says "merged with N minors - see Closed", and no
lane reads that line as work. A fix session exists only for blockers and majors. `preset-author` does
not read reviews, and `architect` next sees the file at a later close, if ever.

Most of the eight describe no defect in what the code does. Four are a comment, an assertion message
or a reference sample that a plan made false. Each is a one-line edit, and a reader of that file is
misled until someone makes it. One, 0182's `.claude/skills/preset-author/references/render-loop.md:170`,
was left open *"because this session may not edit .claude/"*, and no permission rule or skill rule
says so (backlog 0225). The restriction that probably exists is ownership: a `dev`-lane plan's close
editing another lane's skill. It is written nowhere, so it reads as a hard limit.

## Decision

**A conductor-run close repairs every `minor` or `nit` whose repair cannot change what any program
does, before it runs its gate, and records the commit on the finding.** "Cannot change what any
program does" is a closed list:

- the text of a comment or doc comment in any source file;
- the message text of an assertion or a panic;
- Markdown prose anywhere in the repository, including every file under `.claude/skills/`, except
  a generated region, which is repaired by regenerating it.

Everything else stays open: code, a test's logic, a constant, an instruction a skill gives. The
closed verdict marks each repaired finding with `fixed_in`, the repairing commit. The conductor
checks that the commit is on the branch and touches the finding's file, and parks on disagreement. The
digest's **Needs you** then lists only the findings still open, each with its `file:line`.

**A close may correct a fact in any lane's skill material that its plan made false, and never
changes a rule.** A sample output, a column list, a flag spelling or a count in
`.claude/skills/<lane>/` is a fact, and it follows the tree whichever lane owns the skill. An
instruction (what a lane must or must not do) is that lane's contract. A close that thinks one is
wrong raises a backlog entry and leaves it.

## Consequences

### Positive

- **The class that made up most of the pilot's leftovers stops accumulating.** Four of the eight open
  findings were in it.
- **Backlog 0225's unwritten rule is written**, and where the line falls is stated as fact versus
  rule, not as a directory.
- **The repair lands before the close's gate**, so it is gated like everything else in the close.
  With ADR-0207, nothing is re-run for it.

### Negative

- **A close edits files its plan did not touch**, so a close diff can reach into another lane's
  source file for a comment. The `fixed_in` check bounds it to the finding's file. Nothing bounds how
  much of that file changes.
- **A doc comment is not free.** A rustdoc link in one can turn `cargo doc -D warnings` red, so a
  one-line repair can park a close that would otherwise have merged.
- **Code-shaped minors still have no route.** The pilot left `standalone/src/shot/report.rs:110`
  (a duplicated constant) and `core/src/render/scenes/cellular/tests.rs:71` (a test driver in the
  retired order) open, and this decision leaves them open, now more visible.

## Alternatives considered

### Alternative A — A periodic fix-up plan
The conductor collects open findings into a small queued plan. Rejected because each comment repair
would pay a whole plan's cycle: implement session, gate, review and close. It would also land weeks
after the reader was misled, when the close already has the file open and the finding in hand.

### Alternative B — The digest lists them and the owner routes each
The status quo, better formatted. Rejected on the pilot: ten findings, none routed, and the owner's
morning spent on ones a close could have repaired.

### Alternative C — Every minor goes to a fix session before close
Rejected. It adds a session and a gate to every plan with a minor, most plans have one, and the
blocker-and-major threshold exists so that minors do not hold a merge.

## Outcome — 2026-09-16, from Plan 0189 Phase 8 and this plan's own close

**The mechanism works and one clause of the Decision is unreachable as written.** 0175's and 0177's
closes repaired six findings between them and marked each with `fixed_in`; `verifyClose` checked
every one against the branch and the finding's file, and the digest's **Needs you** then listed only
what was left open. That is the Decision delivering exactly what it promised.

The unreachable clause is *"Markdown prose anywhere in the repository, including every file under
`.claude/skills/`"*. A headless `claude -p` session's `Edit` and `Write` under `.claude/` are refused
by the CLI although `settings.conductor.json` allows both tools. Backlog 0225 read the restriction as
one of ownership, and this ADR answered that reading; the restriction is in fact the CLI protecting
its own configuration directory. Seen three times: 0182's close left `render-loop.md:170` open, and
0177 Phase 8 parked `check_red` because its own done-when greps `.claude/skills/` — the owner
committed that fix by hand as `f0cf263`. [Backlog 0230](../design-backlog.md) carries the three
shapes, including amending this ADR to except `.claude/` and name who repairs it.

**The fact-versus-rule line held on its first test, in the other direction.** This plan's own close
found the `dev` field guide still carrying a rule Phase 5 had changed in three other places. That is
a fact following the tree rather than a close overruling a lane's contract, and it was repaired
(`925a599`). Two findings this close raised — a digest line and a test's fixture — fell outside the
closed list and were filed as [backlog 0234 and 0235](../design-backlog.md) instead, which is the
Negative *"code-shaped minors still have no route"* behaving as written.
