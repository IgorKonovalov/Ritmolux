# 0193 — The digest says what is happening, and where you are needed

> **Status:** draft
> **Created:** 2026-09-18
> **Owner skill(s):** dev
> **Related ADRs:** [0214](../adrs/0214-the-digest-is-a-current-state-page-and-history-is-regenerated-on-demand.md)
> (proposed), [0205](../adrs/0205-an-approved-plan-runs-under-a-conductor-and-every-judgement-it-cannot-make-parks-the-plan.md)

## TL;DR

`tools/conductor/digest.md` appends a section per run and never forgets one, so after twelve runs
the two questions its reader actually has — *what is happening now* and *where am I needed* — are
answered somewhere inside ~40 KB. This plan makes the page current-state only, worklist first, and
moves the per-run history behind `conductor.mjs digest --history`. It also makes a park the
repository has already settled read as a stale record rather than as work, which is the failure that
prompted the change.

## Context & problem

On 2026-09-18 the digest reported five plans parked and needing the owner. **Four were already
done**: 0166, 0179 and 0180 had been closed and merged in owner-led sessions, and 0178's blocking
phase had landed on its branch as `955cb766`. A plan finished outside the conductor never touches
`state/conductor.json`, so its record stays `parked` and every later run's page repeats it in the
present tense, with a resume command, indistinguishable from the one park that was real.

The page's shape compounds it. Each run adds its own **Needs you**, **Closed**, **Failed and
parked** and **Totals**; the newest run additionally lists **Still parked from an earlier run**. So
one park can appear twice, worded differently, and a finding repaired by the close that reported it
stays on the page forever.

The record itself is not the problem — `state/conductor.json` holds it, the page is derived from it
after every step, and the file is gitignored precisely because it can always be rebuilt.
[ADR-0214](../adrs/0214-the-digest-is-a-current-state-page-and-history-is-regenerated-on-demand.md)
records the decision and what it costs.

## Decision

Two renderers over one state reader: a **current-state page** written every time, and a **history
page** written when asked for. The current page leads with **Needs you** and follows with **Now**.
A parked plan whose blocking phase the tree shows as finished is reported under its own heading, as
a record to clear, never as an obligation — and the digest decides that from the tree conservatively
(`done/`, or the phase's own log row), never by inference from timing.

We rejected writing both pages on every render (two files, one never read, and the unread one is
where staleness hides) and dropping the history outright (the per-run spend, usage windows and
closed findings are the only human-readable account of an unattended night).

## Implementation phases

### Phase 1 — the page leads with the worklist, and the history moves behind a flag

- **Owner skill:** dev
- **What:** `digest.md` becomes current-state only, in two sections; `conductor.mjs digest --history`
  writes the long form to `digest-history.md`.
- **Files touched:** `tools/conductor/lib/digest.mjs`, `tools/conductor/conductor.mjs` (the `digest`
  command and `status`'s call into it), `tools/conductor/test/digest.test.mjs`,
  `tools/conductor/README.md`.
- **How:**
  - **Needs you, first, and it is the whole worklist:** every standing park with its reason, its
    age, the worktree it holds or the branch `resume` reopens it from, and its resume command; every
    lane stopped at `max_open_worktrees`, naming what holds the slots; every open finding from a
    merge, with its `file:line`; the CLI-version warning when one applies.
  - **Now, second:** per lane, the plan, the step and round, how long it has been in it, and what it
    has spent; when no run is live, the last run's end time and its one-line totals.
  - Both sections are built from the **same state reader** the history page uses. A second traversal
    of `state/conductor.json` is what ADR-0214 names as the place the two pages would drift.
  - `digest-history.md` is gitignored beside `digest.md`; add it to the ignore rule in the same
    commit, because the first run of the flag is what would otherwise offer it for staging.
- **Done when:**
  - A state fixture with two runs, one park and one merge renders a `digest.md` whose first section
    is **Needs you** and which names neither run's per-run totals.
  - `digest --history` on the same fixture renders a page carrying both runs, each with its
    **Closed** and **Totals** — the content the page carries today.
  - A fixture with no parks, no cap stops and no open findings renders a **Needs you** section that
    says so in one line, and the test asserts that line: an empty worklist has to be legible as one.
  - `tools/conductor/README.md`'s *What to read afterwards* describes the two pages and the flag,
    and no longer describes a per-run section of `digest.md`.

### Phase 2 — a park the repository has already settled reads as stale

- **Owner skill:** dev
- **What:** A parked plan the tree shows as finished is reported as a record to clear, apart from
  the real worklist.
- **Files touched:** `tools/conductor/lib/digest.mjs`, `tools/conductor/lib/plan.mjs` (if the phase
  needs a reader it does not already export), `tools/conductor/test/digest.test.mjs`,
  `tools/conductor/README.md`.
- **How:**
  - **Two conditions, both read from the tree, both conservative.** A parked plan whose file is
    under `docs/plans/done/` with `Status: done` is finished. A plan parked `human_phase` or
    `claude_dir` on a phase whose `## Implementation log` row — in the lane if it exists, else in the
    main checkout — now reads `done` is unblocked. Anything else stays in the worklist. **Never infer
    from age, from a branch's commits, or from a tag.**
  - The stale rows go under their own heading inside **Needs you**, each naming the one command that
    clears it (`resume`, or `resume` after the lane is reopened when the worktree is gone), and are
    counted apart from the live parks in the section's own summary line.
  - **The digest writes nothing back.** ADR-0214 forbids the renderer touching the record it renders;
    clearing a stale park stays an explicit `resume`.
- **Done when:**
  - A fixture with a plan parked `human_phase` whose log row reads `done` renders it as stale, with
    its resume command, and not among the live parks.
  - A fixture with a plan parked `gate_red` whose plan file is under `done/` renders it as stale.
  - A fixture with a plan parked `human_phase` whose row still reads `not started` renders it as a
    live park — the negative case, asserted, because a false "already settled" is the one error this
    phase could introduce and it is worse than the problem it fixes.
  - `status` agrees with the page: the same plan is not printed as a bare park there while the
    digest calls it stale.

## Risks & open questions

- **A wrong stale verdict is worse than the noise it replaces.** It tells the owner that real work is
  finished. The phase's conditions are deliberately narrow and its negative case is a done-when.
- **The two pages can drift.** One state reader and the existing digest tests are what hold them;
  the history page is read rarely enough that a defect in it could sit unnoticed.
- **`git` in the renderer's path.** It already reads `git` for plan titles; Phase 2 adds reading a
  plan file from two possible locations. A worktree that has been removed must not read as a missing
  plan — the main checkout is the fallback, and the fixtures cover it.
- **This plan changes no behaviour of the conductor itself** — no lane, gate, lock or session path is
  touched. A bug here misinforms; it cannot corrupt a run.

## What this plan does NOT do

- **It does not change what is recorded.** `state/conductor.json` keeps every run, park, step and
  finding exactly as today. Only the rendering changes.
- **It does not reconcile the record with the repository.** A stale park is *reported*, never
  cleared; `resume` stays the owner's explicit act.
- **It does not touch `live.log`**, which is the run's own stream and already answers "what is
  happening now" for anyone watching in real time.
- **No new dependency.** The conductor is zero-dependency Node and stays that way.

## Implementation log

> Written by `dev` — one row per phase as that phase's commit lands, and the close block after the
> last one. **The phases above are the contract; everything here is what happened.**
> **Observations, never conclusions:** this says where to look, architect decides how it went.
> No per-criterion pass list, no self-assessment, no narrative — but a deviation from the plan or
> an unmet done-when is always disclosed. Stays shorter than `## Implementation phases` above.

**Lane:** _(`main` directly, or the worktree path plus its branch)_

| phase | owner | state | commit |
|---|---|---|---|
| 1 — the page leads with the worklist, and the history moves behind a flag | dev | not started | |
| 2 — a park the repository has already settled reads as stale | dev | not started | |

### Notes

### Close triggers

- **`presets/` touched:**
- **Plan header `Closes:`**
- **What shipped:**
- **Operator docs touched:**
- **Backlog probes (`node scripts/check-backlog-claims.mjs`):**
- **Full suite:**
- **Outstanding `human` phases:**

## Followups (after this lands)

- Whether `status` should print the same two sections the page now leads with, rather than its own
  shorter shape.
