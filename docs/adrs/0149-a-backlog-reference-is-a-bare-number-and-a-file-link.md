# ADR-0149 — A backlog reference is a bare number and a file link, never a fragment

> **Status:** accepted 2026-09-02 (Plan 0136 Phase 4), Outcome
> **Date:** 2026-08-29
> **Extends:** [ADR-0127](0127-a-comment-carries-the-mechanism-and-the-decision-record-stays-in-docs.md)
> **Related plan(s):** [0136](../plans/done/0136-the-gates-can-convict.md)

## Context

The close ceremony archives a discharged backlog entry by moving its body verbatim to
`design-backlog-archive.md` and leaving a ledger row behind in `design-backlog.md`. Anchors aimed at
the moved body — `(../design-backlog.md#0072--sanityrss-coverage-floor-forces-…)` — keep resolving
to a **file that exists**, so `scripts/check-doc-links.mjs` reports them clean: it validates paths
and deliberately never validates fragments. The link silently lands at the top of the live backlog
instead of at the entry.

Measured across every `.md` in the repository on 2026-08-27: **24 distinct entry numbers are
addressed by anchor, and 20 of them are archived** — `0009 0020 0022 0027 0033 0040 0055 0056 0057
0058 0059 0060 0061 0062 0063 0067 0070 0084 0085 0088` — across roughly 20 files, mostly ADRs and
closed plans. Two more (`0072`, twice) were repaired by hand at Plan 0118's close, which is the only
reason anyone measured.

Two facts make this a decision rather than a cleanup. First, **most of the 20 sites are append-only
documents** — accepted ADRs and closed plans. Repointing them means editing a historical record to
keep a convenience link working, which cuts directly against the rule that an accepted ADR is
amended only by a dated `Outcome`. Second, **the anchor is the part that rots and the number is
not**. An entry's heading text is its anchor, headings get reworded, and the ledger row that
replaces an archived body carries the number but not the old heading — so even a fragment checker
would only convert silent rot into loud rot at every close.

The step already exists in the architect skill and names this precisely — *"the one class of break
here that no gate will catch for you"*. The 20-site accumulation is the evidence that naming it was
not enough.

## Decision

**A reference to a backlog entry is the bare number plus a link to the file, and never a fragment.**
The form is `[backlog 0072](../design-backlog.md)` or, in prose that already links the file,
simply `backlog 0072`. This holds for live and archived entries alike, in every document type, and
it is the same answer [ADR-0127](0127-a-comment-carries-the-mechanism-and-the-decision-record-stays-in-docs.md)
reached one level down when it barred relative links from `.rs` comments: a reference whose *form*
cannot be checked is replaced by one that cannot break.

**Repointing the existing 20 sites is a mechanical link-form change, not a content edit, and is
therefore permitted in append-only documents.** Dropping a fragment does not alter what an ADR
decided or what a closed plan did; it removes a navigational detail that already fails to
navigate. Nothing else in those files is touched.

`scripts/check-doc-links.mjs` gains a rule that **rejects any `design-backlog.md#…` or
`design-backlog-archive.md#…` fragment**, which is the whole enforcement surface. It does not gain
a general fragment checker.

## Consequences

### Positive
- The class becomes unbreakable rather than merely checked: an archived body can move again and no
  reference anywhere degrades.
- The close ceremony's step 3c loses its most error-prone half. Archiving an entry stops requiring a
  repo-wide anchor sweep that nothing verifies.
- One rule now covers `.rs` comments (ADR-0127) and markdown (this ADR), stated the same way, so
  there is one habit rather than two.

### Negative
- **A reader loses one click.** Landing at the top of a 280 KB backlog and searching for `0072` is
  worse than landing on the heading, and this ADR makes that the permanent experience rather than
  the accidental one. The mitigation is that the number is greppable and the file has a ledger.
- **It edits 20 append-only documents.** The argument that a link form is not content is defensible
  and is made above, but it is an argument, and someone re-reading an old ADR's git history will
  see a commit touching it years after acceptance.
- **A general fragment checker is now less likely to be built**, since the one class that motivated
  it is handled by prohibition. Other fragment links in the repo stay unchecked.

### Neutral
- Nothing about how entries are archived changes. The ledger row, the verbatim body move and the
  append-only archive are all untouched.

## Alternatives considered

### Alternative A — Repoint every anchor to the archive and add a fragment checker
Keeps the convenience of a deep link and makes the class checkable for the first time. Rejected
because it repairs the symptom on a schedule: every future close that archives an entry breaks
every anchor aimed at it, and the checker converts that from a silent break into a red push at the
close — which is better, but it is a recurring tax on a ceremony that already carries four
bookkeeping steps and skips them. It also still requires editing the append-only documents, so it
pays that cost *and* keeps the fragility.

### Alternative B — Repoint only live documents, leave the historical ones
Avoids editing append-only records, which is its whole appeal. Rejected because it institutionalizes
a known-wrong link in the documents most likely to be read cold — an accepted ADR is exactly what a
future session opens to understand a decision, and landing it at the top of the backlog with no
indication that the target moved is the failure mode, not a lesser version of it. It also leaves the
class half-repaired, which is the state that guarantees the next measurement finds it again.

### Alternative C — Leave it; the links resolve to a real file and nobody has complained
The honest null option, and the entry itself rates the severity low. Rejected because the cost is
not the broken navigation, it is that `check-doc-links.mjs` reports these clean. A gate that returns
OK on 20 broken references is teaching its readers that green means correct, which is the same
failure this repository has now repaired in three separate gates
([Plan 0084](../plans/done/0084-two-gates-stop-lying-about-what-they-check.md),
[Plan 0094](../plans/done/0094-the-two-doc-gates-check-what-they-claim-to.md), and backlog 0104).

## Notes

Discharges [design-backlog 0143](../design-backlog.md), whose "scope call" section poses the three
alternatives above; this ADR takes its Option 3.

## Outcome — 2026-09-02, at Plan 0136's close

**The decision stands unchanged. Two things it recorded were wrong, both in the direction of
understating the work.**

**The site count.** Context above measures *"24 distinct entry numbers … across roughly 20 files"*,
and the Negative section says *"It edits 20 append-only documents."* Phase 4 re-derived the set
rather than trusting the list — which this plan instructed, because the measurement was taken
2026-08-27 and closes had run since — and found **87 links across 29 files**. The 24 was never wrong:
it counts entry *numbers*, and a number is cited from several places. Nothing else follows from the
correction, but a reader sizing this work off the Negative bullet would have sized it at a third.

Ten occurrences survive and are correct: every one is an inline code span in prose describing the
retired form, including this ADR's own Context and Decision.

**The enforcement surface had an asymmetry the new rule exposed.** `check-doc-links.mjs` matched an
inline `](target#anchor)` by stopping at the `#`, but its *definition* form (`[label]: target`) ran
the target to whitespace — so a `[label]: design-backlog.md#anchor` definition was resolved as a path
*containing* the fragment and reported as a missing file rather than as a prohibited fragment. No
such definition existed in the tree, which is the only reason the gate was green. Seeding the class
in both link forms produced two findings on one line; the definition target now drops its fragment
before the existence check, so both forms answer the same question. That repair is part of this
ADR's enforcement and was not anticipated here.

**One thing this ADR did not reach, and it mattered.** The prohibition covers links, and the gate can
only see links. The architect skill's own step 3c — the ceremony this ADR exists to simplify — still
*instructed* the archived-anchor form, inside a code span, where the new rule is structurally blind.
It was repaired at this close, but the general point stands: **a form banned by a gate that reads
links is still reachable by prose that prescribes it.**
