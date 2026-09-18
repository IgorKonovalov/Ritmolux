# 0193 — The digest says what is happening, and where you are needed

> **Status:** done (2026-09-18) - both phases at `04b18360`, `65a0f62e`, with the close review's two
> prose repairs in `35141369`. Verified: the two renderers build from one reader, `settledPark` is
> the same verdict `status` prints, and the stale rule's negative case is asserted. Full suite: the
> suite ledger's `0193-pre-review` record on tree `6bf1b26`, 2010 passed; the conductor's own 334
> Node tests green. Mode 4 round 1: no blockers, no majors, three minors, one nit. Version: 0.132.0.
> **Created:** 2026-09-18
> **Approved:** 2026-09-18 (user)
> **Owner skill(s):** dev
> **Related ADRs:** [0214](../../adrs/0214-the-digest-is-a-current-state-page-and-history-is-regenerated-on-demand.md)
> (accepted), [0205](../../adrs/0205-an-approved-plan-runs-under-a-conductor-and-every-judgement-it-cannot-make-parks-the-plan.md)

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
[ADR-0214](../../adrs/0214-the-digest-is-a-current-state-page-and-history-is-regenerated-on-demand.md)
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

**Lane:** `C:\Users\Igor Konovalov\WORK\rlx-plan-0193` on branch `plan-0193-the-digest-says-what-is-happening-and-where-you-are-needed`

| phase | owner | state | commit |
|---|---|---|---|
| 1 — the page leads with the worklist, and the history moves behind a flag | dev | done | `04b18360` |
| 2 — a park the repository has already settled reads as stale | dev | done | `65a0f62e` |

### Notes

- Phase 1 adds a fifth item to **Needs you** that the plan's list does not name: a merged plan whose
  lane the conductor could not remove, gated on the worktree still existing on disk. It is a standing
  obligation that holds a `max_open_worktrees` slot, and the plan's enumeration would have dropped it
  from the default page. `tools/conductor/test/lane.test.mjs` covers the four enumerated items; this
  one is covered only by the history page's existing test.
- Phase 2 edits `tools/conductor/conductor.mjs`, which its **Files touched** does not list. Its last
  done-when — *"`status` agrees with the page"* — is a change to `cmdStatus`, which lives there;
  `status` now calls the same `settledPark` the renderer does.
- Phase 2's first condition reads the plan **only from the main checkout**, where the plan's *How*
  says "from the tree" without naming a checkout. A close committed in a lane that has not merged is
  under `done/` on that branch and is unfinished work, so treating it as settled would be the wrong
  verdict the phase's risk names. `tools/conductor/test/digest.test.mjs` asserts that case.
- The Phase 1 test `the page leads with the worklist and carries no run's own totals` changed in
  Phase 2's commit: its fixture repository holds plan 0175 under `done/`, so that park now renders
  under the stale heading rather than among the live ones.

### Close triggers

- **`presets/` touched:** no.
- **Plan header `Closes:`** the header carries no `Closes:` line.
- **What shipped:** a feature, entirely inside `tools/conductor/` plus one `.gitignore` rule. No
  file under `core/`, `core-cabi/`, `rlx-ring/`, `standalone/`, `plugin-foobar/`, `presets/` or
  `studio/` changed, so no shipped artifact moved.
- **Operator docs touched:** `tools/conductor/README.md` — the commands table, *What to read
  afterwards* (now two pages), and the two lines elsewhere that named a section of `digest.md`.
- **Backlog probes (`node scripts/check-backlog-claims.mjs`):** exit 0 — 98 reductions across 39
  live entries, 3 unprobeable, 46 advisory path-moved rows.
- **Full suite:** owed to the conductor's pre-review gate (ADR-0207).
- **Outstanding `human` phases:** none — the plan has two phases, both `dev`, both committed.

## Close review

> Round 1, 2026-09-18, a fresh session started by the conductor with the plan and the lane and
> nothing an implementer wrote (ADR-0205). No blocker and no major, so the close ran.

**Verdict: Plan 0193 landed cleanly — no blockers, no majors, three minors and one nit.** Both
phases are in the tree as the log says, the two renderers are what ADR-0214 decided, and the
stale-park rule is as narrow as the ADR made it, negative case asserted. The open items are one
contradictory summary line on the current page, one operator doc still describing the page's old
shape, one Needs-you item the current page renders untested, and one enumeration order in the
conductor README.

- **Lane:** `C:\Users\Igor Konovalov\WORK\rlx-plan-0193` on branch
  `plan-0193-the-digest-says-what-is-happening-and-where-you-are-needed`
- **Commits reviewed:** `04b18360` (Phase 1), `65a0f62e` (Phase 2), `c5fc9a39` (the close block).
- **Diff over the range:** 8 files, +646/-128 — `tools/conductor/lib/digest.mjs`,
  `tools/conductor/conductor.mjs`, three test files, `tools/conductor/README.md`, `.gitignore`, the
  plan itself. No Rust, no C++, no preset, no studio file changed.

### Lens 1 — alignment with the plan and ADR-0214

**The full suite.** `dev`'s `### Close triggers` bullet reads *"owed to the conductor's pre-review
gate (ADR-0207)"*, which is the correct claim in conductor mode. Run here as exactly
`node tools/conductor/with-lock.mjs suite -- cargo nextest run --workspace`, the wrapper did not
re-run it and printed the ledger record instead:

```
with-lock: skipped cargo nextest run --workspace: tree 6bf1b26 is green in the suite ledger,
run by gate 0193-pre-review at 2026-09-18T15:42:09.889Z: 2010 tests run: 2010 passed (5 slow), 7 skipped
```

That record is this lens's full-suite evidence (ADR-0207): the tree it names is this branch's, and
the process that wrote it is the one that saw the exit code.

**The Node suite this plan actually changes** is not in `nextest` at all, so it was run separately:
`node --test "tools/conductor/test/*.test.mjs"` — **334 tests, 334 pass, 0 fail**, 42 s.

**Phases and owner tags.** Two phases, each carrying exactly one in-vocabulary `**Owner skill:**
dev`. Both are in the log with their commits, and both commits exist on the branch. Nothing was
added or dropped silently: the three deviations are all disclosed in `### Notes` — a fifth
**Needs you** item Phase 1's enumeration does not name, `conductor.mjs` edited under Phase 2 though
its *Files touched* does not list it, and Phase 2's first condition reading the plan only from the
main checkout where the *How* says "from the tree". The third is not a deviation at all but a
tightening, and it is the right one: a close committed in an unmerged lane is unfinished work, and
`test/digest.test.mjs`'s *"a plan under done/ only on its branch is not settled"* asserts it.

**The log is shorter than `## Implementation phases`** (45 lines against 58), as ADR-0120 requires.

**Every done-when, against the assertion body rather than the claim.**

Phase 1:

- *first section is **Needs you**, and no run's own totals* — `digest.test.mjs` *"the page leads with
  the worklist and carries no run's own totals"* asserts `lines.filter(l => l.startsWith("## "))`
  deep-equals `["## Needs you", "## Now"]` and that six per-run markers (`## Run `, `### Closed`,
  `### Totals`, `### Failed and parked`, `Still parked from an earlier run`, `- run: `) are absent.
  The fixture is `pilotState`, which does carry two runs, one standing park and one merge. Not
  tautological.
- *`digest --history` carries both runs with their **Closed** and **Totals*** — *"digest --history
  carries every run with its Closed and its Totals"* splits on `^## Run ` and asserts two runs, each
  containing both headings and a `- run: N merged, N parked, ` line.
- *an empty worklist is legible as one line, asserted* — *"an empty worklist is one line"*
  deep-equals the four lines after `## Needs you`, so the text is pinned, not merely present.
- *README's* What to read afterwards *describes the two pages and no per-run section of
  `digest.md`* — done, `tools/conductor/README.md` under `## What to read afterwards`.

Phase 2:

- *`human_phase` park whose log row reads `done` renders stale* — asserted, including the exact two
  rendered lines and that it is **not** among the live parks.
- *`gate_red` park whose plan is under `done/` renders stale* — asserted.
- *`human_phase` park whose row still reads `not started` stays live* — asserted, plus
  `assert.ok(!text.includes("Already settled"))`. This is the negative case the plan's own risk
  section demanded, and it is a real assertion rather than an absence of one.
- *`status` agrees with the page* — `cli.test.mjs` *"status marks a park the lane has already
  settled exactly as the digest does"* drives a real run to a `human_phase` park, flips the row in
  the lane and commits it, then asserts the exact `status` line **and** the page's
  `1 already settled.` **and** `assert.equal(state.plans["0101"].status, "parked")` — the renderer
  wrote nothing back, which is ADR-0214's own prohibition tested.

Two further assertions worth naming because they hold the ADR rather than the code: the lane reader
(`"a human_phase park reads its row in the lane when the worktree is still there"`) and the
`settledPark` return value checked directly, so the two conditions are distinguishable from each
other in the test output.

**No ADR decision reversed.** ADR-0214 is the ADR this plan implements; nothing under `core/`,
`core-cabi/`, `rlx-ring/`, `standalone/`, `plugin-foobar/` or `studio/` is touched, so the C ABI
(spec 0001) and the control protocol (spec 0003) are untouched by construction.

### Lens 2 — layering, coupling, real-time safety

Not applicable in the usual places, and that is verifiable rather than assumed: the diff contains no
Rust and no C++, so there is no audio callback, no `core/` import, no wgpu call and no `extern "C"`
surface in it.

What does apply is the conductor's own layering, and it holds. `lib/digest.mjs` gained no state
mutation: both renderers go through one `readDigestState`, `settledPark` only reads, and
`conductor.mjs`'s new `cmdDigest` writes a file and nothing else. `settledPark` is exported and
`cmdStatus` calls the *same function* the renderer does rather than reimplementing the verdict —
which is precisely how the plan's last done-when is satisfied without a second traversal. The module
header states the invariant ADR-0214 names (*"a second traversal of state/conductor.json is where
the two pages would drift"*), so the trap is documented where the next editor will hit it.

No new dependency: still zero-dependency Node.

### Lens 3 — doc freshness and release bookkeeping

`tools/conductor/README.md` is swept thoroughly — the commands table gained `digest [--history]`,
`status`'s row names the settled verdict, *What to read afterwards* now describes two pages, and the
two incidental lines elsewhere that said "the digest's Totals" now say "the history's Totals". The
`.gitignore` rule gained `digest-history.md` in Phase 1's own commit, as the plan required.

**`docs/developing.md` was not swept, and it is the one reader-facing description of this page.**
Finding 2 below.

`CLAUDE.md` still reads *"Runtime record in state/, live.log and digest.md, all gitignored"* —
incomplete rather than false, since `digest-history.md` only exists once asked for. Not raised.

`.claude/skills/architect/SKILL.md` names *"the digest's **Needs you**"*, which is still exactly
where an open finding lands. No repair owed there, which matters because ADR-0210 would have left it
open.

Gates re-run on this tree, all green: `check-doc-links` (490 files), `check-index-rows` (215 + 213 +
168 rows, 0 over cap), `toc.mjs --check` (7 blocks, 619 rows, current), `check-system-counts`,
`check-comment-hygiene` (290 sources, 0 escapes).

`check-backlog-claims.mjs`: **OK — 98 reductions across all 39 live entries, 3 unprobeable.** Read
past the green line as ADR-0108 requires: the advisory names 46 path-moved rows, several of them
moved by commits dated today from other lanes rather than by this plan. **Backlog 0247 was re-read
against this diff and is not discharged or falsified by it** — it is about `suiteLedger()` resolving
beside the invoked copy of the script, which this plan does not touch.

`check-translations.mjs`: **OK — 5 stamped.** One advisory row, and it is not this plan's:
`packaging/foobar/READ-ME-FIRST.ru.md` is stamped `f2b0048b` while its English source is at
`d6e275e6` (2026-09-18). Nothing in this plan moved that source. Recorded here because ADR-0185's
drift half has no other carrier, and because a third consecutive stale close on the same page is the
signal that ADR names.

`presets/` untouched, so no curation sweep is owed. The plan header carries no `Closes:`, so no
backlog body moves.

**Version:** the plan shipped a feature — a new CLI subcommand, a second renderer and a new verdict
`status` prints — entirely inside `tools/conductor/`. Nothing shipped moved, but the repository's
own tooling gained behaviour, so this is a **minor** bump, 0.131.2 to 0.132.0, with the studio's
two copies following it.

### Lens 4 — correctness and determinism

Neither page is DSP and neither has a hot path, so what stands in for determinism here is the
property both pages are written to have: *regenerating from the same state gives the same bytes*.
That property survived the split and is asserted for both pages
(`"regenerating either page from the same state and git gives identical bytes"`), with the one
honest exception stated in the module header — the current page reads a clock for park ages and a
live lane's elapsed time, so the tests pin `now` to a constant. Making `now` an option rather than
calling `Date.now()` inside the renderer is the right shape: the clock is an input.

**No numeric assertion in the diff is a frozen measurement.** Every number the tests assert is
derived from the fixture's own timestamps (30 + 26 min steps, 12 + 11 min gates, 79 min active) or
from its spend figures, so nothing here is machine-dependent in ADR-0071's sense.

No aspect ratio, no grid, no GPU — ADR-0037's class cannot arise.

The one correctness defect found is finding 1: a summary line that can contradict the list beneath
it. It was confirmed by rendering, not inferred.

### Lens 5 — design integrity

The split is the right one. Two renderers, one reader, the reader exported and used by both, and the
verdict function exported and used by the third consumer (`cmdStatus`) — that is the opposite of the
drift ADR-0214 worried about, and it is enforced by the code's shape rather than by a comment.

`cmdDigest` is a peer of `cmdStatus` in the `COMMANDS` map, the usage string moved with it, and the
file header's command list was updated. No god function appeared: `needsYou` and `nowSection` are
each one screen and each builds one section.

`settledPark` is the one place that reaches into the tree, it is about twelve lines, and its doc
comment states both conditions, what is deliberately *not* a signal (age, a branch's commits, a tag)
and why a branch's own `done/` copy is excluded. That comment carries the mechanism and cites
ADR-0214 by bare number, which is ADR-0127's rule — `check-comment-hygiene.mjs` agrees.

No seam widened. `digest --history` is a new command, not a new protocol; nothing outside
`tools/conductor/` can observe any of it.

### Findings

#### minor — the empty-worklist line claims "Nothing" while a CLI warning is listed under it

`tools/conductor/lib/digest.mjs:298`

`needsYou` counts parks, cap stops, open findings and undeleted lanes into `parts`, but the CLI
warning is pushed into `lines` without ever being counted. With a CLI warning and nothing else, the
section renders its one-line summary as *"Nothing: no park, no lane stopped at the worktree cap, no
open finding."* and then lists the warning immediately below it.

Confirmed by rendering, not inferred — a state with one ended run carrying
`cli: { version: "2.1.400", warning: ... }` and no plans produces exactly:

```
## Needs you

Nothing: no park, no lane stopped at the worktree cap, no open finding.

- **claude 2.1.400 is not a verified CLI version** - the last run went ahead with a warning (ADR-0208): ...
```

This is not a corner: `tools/conductor/README.md` says this CLI *"numbers nearly every release as a
patch, so in practice most updates land here"*, so a quiet night on a patched CLI is the ordinary
case. The cost is that ADR-0214's stated property — *"A page whose first section is empty means
nothing is waiting on the owner"* — is asserted by the summary line while the section is not empty.

**Suggested fix:** count the warning into `parts` (`if (view.cli?.warning) parts.push("an unverified
CLI version")`) so the summary and the list agree. **Left open:** the repair is code, which ADR-0209
does not let a close touch.

#### minor — `docs/developing.md` still describes `digest.md` as the per-run page

`docs/developing.md:310`

The paragraph *"Next morning, read `tools/conductor/digest.md`"* went on to say *"The newest run also
lists what an earlier run left parked ... Totals carry the usage windows at run start and run end,
and gate minutes split into the full workspace suite and everything else, with the count of suite
runs skipped"*. After this plan none of that is on `digest.md`; all of it is in `digest-history.md`,
which the doc did not mention, and the paragraph did not mention the **Already settled** heading that
is now the plan's headline behaviour. `tools/conductor/README.md` was swept in the same commits and
this file — the only reader-facing description of the page — was not.

**Repaired in this close, `35141369`.** It is Markdown prose outside `.claude/`, which ADR-0209 lets
a close fix.

#### minor — the fifth Needs-you item is rendered by the current page and tested only on the history page

`tools/conductor/lib/digest.mjs:273`

`- **NNNN merged, lane not removed**` is a standing obligation that holds a `max_open_worktrees`
slot, and Phase 1's enumeration of what **Needs you** carries does not name it. `dev` added it
deliberately and disclosed the gap in `### Notes`: `test/lane.test.mjs` covers the four enumerated
items on the current page, and this one only through the history page's pre-existing test. So the
branch in `needsYou` that emits it — including its `laneOpen(rec)` guard, which is what stops a lane
removed by hand from being reported forever — has no assertion of its own.

The guard reads correctly, and this is a coverage gap rather than an observed defect. It is raised
because the whole value of the new page is that its first section is trustworthy, and an untested
branch in it is where that erodes first.

**Suggested fix:** one case in `lane.test.mjs` alongside the cap-stop one, with `cleanup.ok === false`
and the worktree still on disk, asserting both the bullet and the `1 lane still on disk after a
merge` summary — plus its negative, the directory gone. **Left open:** a test's logic is not
something ADR-0209 lets a close write.

#### nit — the README lists the CLI warning last, and the renderer emits it first

`tools/conductor/README.md:112`

*What to read afterwards* enumerated the section's contents as parks, cap stops, undeleted lanes,
open findings, *"and the CLI-version warning when the last run carried one"*. `needsYou` emits the
warning **before** everything else. A reader looking for it at the bottom of the section would not
find it there.

**Repaired in this close, `35141369`**, by moving the clause to the front of the enumeration so the
prose reads in the order the page renders.

## Followups (after this lands)

- Whether `status` should print the same two sections the page now leads with, rather than its own
  shorter shape.
