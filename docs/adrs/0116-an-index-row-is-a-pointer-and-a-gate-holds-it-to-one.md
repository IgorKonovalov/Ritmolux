# ADR-0116 — An index row is a pointer, and a gate holds it to one

> **Status:** accepted (2026-08-16, user approval)
> **Date:** 2026-08-16
> **Related plan(s):** [0105](../plans/done/0105-the-indexes-go-back-to-being-indexes.md)

## Context

Three files in `docs/` are rosters: [`adrs/README.md`](README.md),
[`plans/README.md`](../plans/README.md), and the closed-entry ledger inside
[`design-backlog.md`](../design-backlog.md). Each exists so a session can find the right document
without opening a hundred of them. All three have the same defect, and it is measured rather than
felt.

**`docs/adrs/README.md` is 188,820 bytes across 128 lines — 16 % of the 1,204,423-byte ADR corpus
it indexes.** The growth is monotone in ADR number: rows 0001-0020 average **152 bytes**, rows
0041-0060 average **1,608**, and rows 0101-0115 average **3,302** — a **22x** widening of the same
column. The recent end is the fat end, which matters because it inverts the obvious remedy: ADRs
0001-0040 are **5.7 %** of the file, so archiving the oldest third saves nothing. The file grew
**+41,705 bytes in the three days** from 2026-08-13 to 2026-08-16.

What grew is the `Title` column, which stopped holding titles. It now holds an abstract of each
ADR — decision, rejected alternatives, costs, execution pointer — averaging roughly a third of the
body it links to. **The duplication is verified, not assumed:** 22 rows carry entire `Outcome`
summaries in the `Status` cell (ADR-0048's alone is 1,450 bytes), and **all 22 of those ADRs carry
a `## Outcome` section in their own body**. Every fat cell is a second copy of prose that already
exists one click away.

One thing in the index is *not* a copy. The forward-reference graph — `supplemented by 0020`,
`extended by 0006, 0008, 0013`, `membership revised by 0032` — appears nowhere else, because an
accepted ADR is append-only and nobody may reach back into ADR-0003's header to record that
ADR-0013 extended it. That is roughly six rows, and it is the reason a thin index is still not a
pure projection of the bodies.

**The decisive fact is that this project has already run the obvious fix and watched it fail.**
On 2026-08-08, Plan 0061 Phase 7b moved every close write-up out of `docs/plans/README.md` verbatim
into `README-archive.md`, cutting the index to **20,438 bytes**. Eight days later it is **145,945
bytes** — **7.1x regrowth** — and 85,892 of those bytes sit in a `## Recently closed` section
whose own heading still reads *"One line per plan"* above rows averaging **1,130 bytes**. The rule
was written in the strongest possible place, inside the file it governed, three lines above the
rows that violate it, and it held for days. This is the same shape
[ADR-0108](0108-a-backlog-claim-about-the-repo-carries-an-executable-probe.md) found in the
backlog and [ADR-0033](0033-testing-strategy-coverage-ratchet-and-pre-push-gate.md) names for gates: a rule nothing
re-runs is a rule nobody follows.

## Decision

We will **cut every roster row back to a pointer — link, title, status — and enforce that shape
with a byte cap a script re-runs**, rather than trusting a prose convention that has already been
falsified in this repo.

Concretely: the per-ADR abstracts are **deleted** from `docs/adrs/README.md`, which keeps only the
link, the title as it appears in the body's `H1`, and a status cell carrying the forward-reference
graph. `docs/plans/README.md`'s `## Recently closed` section returns to the one line per plan its
heading already promises, with the write-ups going where Plan 0061 Phase 7b already put them.
The `## Closed entries` ledger in `docs/design-backlog.md` returns to link-plus-destination rows.
A new `scripts/check-index-rows.mjs` asserts a **320-byte cap** on each row inside explicitly
marked roster regions, joining `check-doc-links.mjs` at all three of its call sites — the pre-push
hook, the architect close ceremony, and the un-bypassable CI `links` job.

**The cap is derived, not picked.** Rebuilding each roster's rows from the real filenames and the
real `H1` titles gives: ADR rows median **152** / max **219** bytes, which the longest genuine
forward-reference status (`extended by 0006, 0008, 0013 — the shipped surface is v4`, 70 bytes)
raises to a worst case of **269**; closed-plan bullets max **234**; ledger rows about **280**. A
320-byte cap clears every measured worst case by 51 to 86 bytes and sits an order of magnitude
below the 3,302-byte rows it is meant to prevent.

Two boundaries are deliberate. **Live entry bodies in `design-backlog.md` are out of scope** — that
file is an inbox, its entry bodies are the content rather than an index of content, and they have
no second copy anywhere; only its closed-entry ledger is a roster. And **a claim that turns out to
live only in the index is not deleted**: it is appended to the ADR as a dated `Outcome` section on
the [ADR-0054](0054-runtime-tier-switching-rebuilds-on-the-live-context.md) / [ADR-0074](0074-a-ratio-against-an-in-run-control-is-not-automatically-portable.md)
precedent, which is how this project adds to an accepted ADR without editing it.

## Consequences

### Positive

- **The ADR index becomes readable again.** At the measured row widths it lands near 24 KB against
  today's 189 KB, which is the difference between a file a session loads and a file a session
  skips.
- **A second copy stops being able to drift.** Twenty-two `Outcome` summaries currently exist twice;
  after this, the ADR body is the only place its own outcome is written, so the two cannot disagree.
- **The failure is caught at the commit that causes it**, by the lane that causes it. The close
  ceremony is what writes these rows, so the gate fires on the architect, at the close, rather than
  surfacing months later when someone asks why the index is unreadable.
- **The remedy generalizes.** One mechanism and one cap cover three files with three different row
  shapes (an ADR table, plan bullets, a ledger table), because the marked region is the unit rather
  than the syntax.

### Negative

- **The cap measures shape, not quality.** A 300-byte row can be a bad pointer — a title that does
  not say what the ADR decided passes the gate cleanly. This buys back the file's size and none of
  its usefulness, and nothing here checks the latter.
- **The thin index still holds something nothing else does.** The forward-reference graph keeps the
  roster from being regenerable from the bodies, so a corrupted index is a real loss rather than a
  rebuild. Roughly six rows carry it today and the trim must not drop them.
- **Region markers are new syntax in three files, and a row moved outside them escapes silently.**
  The gate cannot distinguish a row deliberately placed outside a marked region from one that
  drifted out, which is the standard way a marker-based check gets defeated without anyone lying.
- **This is a fourth gate, and [ADR-0033](0033-testing-strategy-coverage-ratchet-and-pre-push-gate.md) warns that gates
  decay when they hurt.** The headroom is 51-86 bytes over the measured worst case, which is thin:
  a future ADR with a long slug *and* a forward-reference is the case that will hit the cap first,
  and the honest response there is to raise the cap with new arithmetic rather than to widen the
  region.
- **The backlog ledger needs more rewriting than its recent rows suggest** — its median row is
  already 218 bytes and its p75 is 428, so roughly a third of 89 rows are over cap, not just the
  new ones.
- **Deletion is recoverable only by `git show`**, which in practice means the abstracts are gone.
  That is the accepted price of the verification that they are duplicates; if the verification is
  wrong anywhere, the loss is silent.

### Neutral

- The index stops being greppable for decision *content*. Answering "was this ever decided?" moves
  from one `grep` over `README.md` to one `grep` over `docs/adrs/`, which is the same command with
  a different path and reads the authoritative copy instead of a summary of it.

## Alternatives considered

### Alternative A — Archive the oldest ADRs to a companion index

The literal reading of "trim it / archive", and the arithmetic refutes it: rows 0001-0040 are
**5.7 %** of 188,820 bytes. Splitting off the oldest third saves under 11 KB while leaving every
3,302-byte row in place, and it adds a second file to keep in sync. The fat is at the new end, so
an age-based split is aimed at the wrong axis.

### Alternative B — Move the abstracts to a `docs/adrs/digest.md`

Preserves the writing off the hot read path, exactly as `plans/README-archive.md` preserved the
close write-ups. It loses because the abstracts are **duplicates of the bodies**, so preserving
them creates a third copy of the same prose whose only distinguishing property is that it can go
stale — and, on this repo's own evidence, nothing re-reads an archive file. Preservation was never
the problem; the write-ups were preserved in August and the section regrew 7.1x anyway.

### Alternative C — Write the rule into the skill's close ceremony and stop there

The cheapest option and the one already disproved. Plan 0061 Phase 7b wrote *"One line per plan"*
into `docs/plans/README.md` itself — above the rows, in the file, unmissable — and the rows beneath
it now average 1,130 bytes. The same pattern is documented three times over in this repo's
ceremonies (the backlog-archive step needed three sweeps across ten days before it became a step).
An unenforced convention is what produced the defect.

### Alternative D — Cap the whole file instead of each row

Simpler to implement and impossible to defeat with markers. It fires as a function of *how many*
ADRs exist rather than how fat each row is, so it would convict the project for having 115
decisions and force exactly the age-based archive Alternative A already lost on. A per-row cap
targets the thing that actually changed.

### Alternative E — Move the forward-reference graph into the ADR bodies

Would make the index a pure projection of `docs/adrs/*.md`, regenerable and therefore unloseable.
It requires reaching into ADR-0003's accepted header to record that ADR-0013 extended it, which is
the in-place edit of an accepted ADR this project forbids. The `Outcome` mechanism is not a
substitute: it records what a plan falsified about the ADR's own claims, not that a later decision
built on it.

## Notes

Measurements taken on `3b39004` (2026-08-16). The `+41,705 bytes in three days` figure comes from
`git cat-file -p <commit>:docs/adrs/README.md | wc -c` walked back over the last twenty commits
that touched the file; the 7.1x plans-index figure compares `800f102` (the commit that created
`README-archive.md`) against the same working tree.

The row-width distributions in the Decision are **construction arithmetic from the real filenames
and `H1` titles**, not measurements of a trimmed file — no trimmed file exists yet. Plan 0105
Phase 2 is where they either hold or the cap gets revised with better numbers.

## Outcome — 2026-08-16, at Plan 0105's close

The construction arithmetic held and the cap did not move. Measured on the trimmed tree: ADR rows
median **170** / max **246**, so **74 bytes** of headroom under the 320-byte cap against the 51
this ADR predicted; `docs/adrs/README.md` landed at **21,085 bytes** from 189,305, an 89 % cut
against the ~24 KB estimated above. All three rosters together went 477,594 -> 220,626.

**Two figures here are corrected by measurement, and the second is load-bearing.**

**The over-cap count was 136, not 135.** The plans section held 26 over-cap bullets rather than 25;
the ADR (83) and backlog (27) figures were exact.

**The forward-reference graph is three rows, not "roughly six."** The Negative section estimates
six rows carrying the inbound edges that exist in no ADR body, and that number is the whole
argument against Alternative E — regenerating the index from `docs/adrs/*.md`. Phase 2 checked the
graph edge by edge and found **three** genuine inbound edges (0002 supplemented by 0020, 0003
extended by 0006/0008/0013, 0031 membership revised by 0032) plus **24 outbound trailers** in the
title cells, whose 47 targets are **all named in the ADR bodies themselves** and are therefore
redundant rather than index-only. The qualitative claim stands — a thin index is still not a pure
projection of the bodies, and a corrupted one is a real loss — but it rests on three rows. Whether
three edges are worth keeping the index hand-maintained is a smaller question than this ADR framed
it as, and it is the one Alternative E should be re-read against.

**One thing this ADR did not anticipate.** Its Negative section names the marker hole — a row moved
outside a region escapes silently — and the fixture pins that as deliberate behavior. It does not
name the **non-vacuity** hole: the fixture asserts exit 0 only, so a checker that finds *no rows at
all* is indistinguishable from a clean tree at every one of the three call sites. Demonstrated at
this close by mutating the row matcher to match nothing — fixture and repo both still exit 0.
Filed as [backlog 0104](../design-backlog.md); the repair is a `--self-test` or a second fixture
root expecting exit 1, which is what
[ADR-0108](0108-a-backlog-claim-about-the-repo-carries-an-executable-probe.md)'s gate already
carries and this one does not.
