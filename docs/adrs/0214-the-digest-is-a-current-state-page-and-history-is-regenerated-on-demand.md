# ADR-0214 — The digest is a current-state page, and history is regenerated on demand

> **Status:** accepted 2026-09-18
> **Date:** 2026-09-18
> **Related plan(s):** [0193](../plans/done/0193-the-digest-says-what-is-happening-and-where-you-are-needed.md)
> **Amends:** [0205](0205-an-approved-plan-runs-under-a-conductor-and-every-judgement-it-cannot-make-parks-the-plan.md)
> (the digest it describes), [0208](0208-a-patch-cli-update-runs-with-a-warning-and-every-session-proves-the-hooks-ran.md)
> (the CLI warning that appears on it)

## Context

`tools/conductor/digest.md` is what the owner reads after a run. It is **regenerated in full from
`state/` and `git` after every step**, newest run first, each run carrying its own **Needs you**,
**Not started**, **Closed**, **Failed and parked** and **Totals** sections. It is gitignored,
because it is derived.

Twelve runs in, the page is ~40 KB and the answer to *"what is waiting on me?"* is spread across it:
the newest run's **Needs you** carries this run's parks and a **Still parked from an earlier run**
subsection, while a park settled two runs ago still has its original row further down, worded in the
present tense. Every closed plan's findings stay on the page forever, including the ones already
repaired by the close that reported them.

On 2026-09-18 that cost an hour. Of five plans the page reported as parked and needing the owner,
**four were already finished** — 0166, 0179 and 0180 had been closed and merged by hand, and 0178's
blocking phase had landed on its branch — and nothing on the page said so, because a plan closed
outside the conductor never touches `state/conductor.json`. The signal that mattered, one real park,
sat among four that did not.

The page is the operator's whole interface to a program that runs unattended and spends money. What
the owner needs from it is two things: **what is happening now**, and **where they are needed**.
Everything else on it is a record that `state/conductor.json` already holds in full.

## Decision

**`digest.md` renders the current state only, and leads with the worklist.** **Needs you** first —
every standing park with the one command that clears it, every lane stopped at the worktree cap,
every open finding from a merge, the CLI warning — then **Now**: what each lane is doing this
minute, in which step, for how long, at what spend. A page whose first section is empty means
nothing is waiting on the owner, and that is the property the page is for.

**The per-run history is regenerated on demand**, by `conductor.mjs digest --history`, into
`digest-history.md` beside it — same content the page carries today, same gitignored, same derived
from `state/`. It is written when asked for and not otherwise.

**A park the repository has already settled is reported as stale rather than as work.** The digest
reads the plan from the tree: a parked plan that is under `docs/plans/done/`, or whose blocking
phase its own log now marks `done`, is reported as a record to clear with `resume`, under its own
heading, never as an obligation.

## Consequences

### Positive

- **The page answers its two questions in its first screen**, and shrinks by roughly the number of
  runs that have happened.
- **An empty worklist is legible as an empty worklist.** Today a quiet run and a busy one produce
  pages of similar length.
- **Nothing is lost and nothing has to be migrated.** Both pages are derived from the same state; the
  flag builds the old one from the same data whenever it is wanted.
- **The stale-park rule removes the failure that motivated this**, and removes it at the source: the
  page stops asserting, in the present tense, that finished work is waiting.

### Negative

- **A run's `Closed` section, with its findings and its spend, leaves the default page.** Whoever
  wants to know what last night produced runs the flag, or reads the plan's own `## Close review`,
  which is committed. The per-run record is one command away rather than zero, and the command has
  to be remembered.
- **Two renderers can disagree.** They are held together by building from one state reader and by
  the existing `test/digest.test.mjs`; nothing structurally prevents drift, and a second page that
  nobody reads is exactly where drift hides.
- **Reading the tree to decide a park is stale puts `git` in the digest's path.** It already reads
  `git` for plan titles, so the dependency is not new, but a wrong answer here is worse than a
  wrong title: it would tell the owner a real park is finished. The rule is therefore written to be
  conservative — a stale report requires the plan to be `done` in the tree or the phase's own log
  row to say so, never an inference from timing.

## Alternatives considered

- **Write both pages every time.** Rejected: two files always on disk, one of which is read
  approximately never, and the more often a page is written the more confidently it is believed
  when it is stale. The flag makes the history a deliberate act, which is what it is.
- **Drop the history entirely and keep only `state/conductor.json`.** Rejected: the per-run spend,
  the usage windows and the closed plans' findings are the only human-readable account of what an
  unattended night produced, and re-deriving a renderer later costs more than keeping this one
  behind a flag.
- **Keep the page as it is and rely on the reader scrolling to the newest run.** Rejected by the
  incident above: the newest run's **Needs you** was correct and still had four stale rows in it,
  because staleness came from outside the conductor rather than from the page's ordering.
- **Have the digest delete or rewrite `state/conductor.json` entries it judges stale.** Rejected
  outright: the digest is a renderer. A page that edits the record it renders can lose a park, and
  the recovery for a lost park is that nobody ever finds out.
