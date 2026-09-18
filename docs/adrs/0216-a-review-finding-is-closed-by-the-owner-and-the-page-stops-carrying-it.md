# ADR-0216 — A review finding is closed by the owner, and the page stops carrying it

> **Status:** proposed
> **Date:** 2026-09-18
> **Related plan(s):** [0195](../plans/0195-a-finding-can-be-closed.md)
> **Amends:** [0214](0214-the-digest-is-a-current-state-page-and-history-is-regenerated-on-demand.md)
> (what **Needs you** carries), [0209](0209-a-conductor-close-repairs-the-prose-and-comments-its-findings-name.md)
> (which findings a close may repair, and which stay open)

## Context

A close review emits findings. [ADR-0209](0209-a-conductor-close-repairs-the-prose-and-comments-its-findings-name.md) lets a
close repair the narrow class whose repair cannot change what any program does, marking each with
`fixed_in`; everything else — code, a test's logic, a constant, anything under `.claude/` — stays
**open** by design, because the repair is the owner's.

[ADR-0214](0214-the-digest-is-a-current-state-page-and-history-is-regenerated-on-demand.md) then put
every open finding on the digest's **Needs you**, which is right: an unrepaired finding is
unfinished work, and the page exists to name unfinished work.

**Nothing ever takes one off.** On 2026-09-18, the first page rendered under ADR-0214 was 45 lines,
of which **39 were open findings from nine merges**, the oldest from plans closed days earlier — a
`nit` about a comment's opening words, a latent regex, a `minor` already filed as a backlog entry.
The page had traded a history that grew per run for a worklist that grows per finding, and the
signal — two parks — sat under thirty-nine lines that were not today's business.

An open finding has exactly three honest futures: someone repairs it, someone decides it is not
worth repairing, or it becomes a backlog entry that carries its own probe. All three end with it
gone from the page, and **none of them can happen today, because the record has no way to say so.**

## Decision

**A finding can be closed by the owner, with a reason, and the digest stops carrying it.**

`conductor.mjs finding <plan> <ref> --done|--wontfix|--filed <reason>` records a disposition against
that finding in `state/conductor.json`, where the finding already lives, stamped with the date. The
three verbs are the three futures above: repaired, judged not worth it, or promoted to a backlog
entry. The reason is required — a disposition with no reason is how a finding gets closed for being
old rather than for being handled.

**The digest carries open findings and a single line counting the closed**, naming the command that
lists them. `digest --history` keeps every finding with its disposition, since that is the record.

**Only the owner closes a finding.** No session, no close, no gate. A close still repairs its
ADR-0209 class and marks `fixed_in`, which is a different act: that one is evidence that a commit
changed the file, and it is checked against `git`. A disposition is a judgement and is checked
against nothing.

## Consequences

### Positive

- **The page can reach empty**, which is the property ADR-0214 was written for and did not deliver.
- **Each finding leaves the page for a stated reason, on a date.** "Handled" and "ignored for three
  weeks" stop looking identical, which is what an accumulating list makes of them.
- **`--filed` names the backlog entry**, so the promotion that already happens informally leaves a
  trace from the finding to the entry that inherited it.
- **Nothing changes about how findings are produced**, so no reviewer instruction moves and no close
  behaves differently.

### Negative

- **A disposition lives in `state/conductor.json`, which is gitignored.** Losing that file returns
  every closed finding to the page. The finding *text* is safe — it is committed in each plan's
  `## Close review` — but the judgement about it is not, and a rebuilt state would ask for all of
  them again. Accepted: the same file holds every run, park and spend figure, so its loss is already
  a bigger event than this.
- **It is a bookkeeping command, and bookkeeping commands get skipped.** A finding closed in someone's
  head still shows on the page, and the page then reads as stale in the direction that trains people
  to ignore it. Nothing prevents this; the count line is the only pressure.
- **A wrong `--wontfix` is invisible.** Unlike `fixed_in`, which the conductor verifies against the
  branch, a disposition is unverifiable by construction. It is a record of a decision, not evidence.

## Alternatives considered

- **Show only the last run's findings, count the rest.** Rejected: it hides an unrepaired finding for
  the reason that it is old, which is precisely the property that should make it *more* visible. The
  page would be short and quietly wrong.
- **Move findings behind `digest --history` entirely.** Rejected: an open finding is the clearest
  thing on the page that is genuinely the owner's, and a page of parks alone under-reports what is
  owed.
- **Expire findings by age.** Rejected for the same reason as the first, without even a count to
  admit it.
- **Write the disposition into the plan's committed `## Close review`.** Tempting — it is durable and
  reviewable in `git`, which the state file is not. Rejected because it edits a plan under `done/`
  after its close, turns a finished record into a file that keeps changing, and makes the digest
  parse prose to learn what a command already knows. The durability cost is recorded above as the
  price of that choice rather than argued away.
