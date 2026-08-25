# 0118 — the comments stop narrating the plans that wrote them

> **Status:** draft
> **Created:** 2026-08-25
> **Owner skill(s):** dev
> **Related ADRs:** [ADR-0127](../adrs/0127-a-comment-carries-the-mechanism-and-the-decision-record-stays-in-docs.md)
> (proposed, and this plan is what accepts it),
> [ADR-0116](../adrs/0116-an-index-row-is-a-pointer-and-a-gate-holds-it-to-one.md)
> **Supersedes:** [design-backlog 0129](../design-backlog.md), which proposed guarding the links
> this plan deletes

## TL;DR

28,064 comment lines against 53,489 of code. 89 relative links in Rust comments that no gate can see
and eleven of which are already broken. 252 lines of narration written from inside a plan session
(`this plan`, `used to`, `no longer`). 44 comment blocks over 40 lines, five of them module headers
over 100.

ADR-0127 splits a comment into what it must carry — mechanism, invariant, trap, un-derivable
formula — and what belongs in an ADR and is merely being copied. This plan builds the gate for the
two mechanical rot classes, deletes all 89 links in favour of bare-number citations, removes the
narration, and trims the 44 long blocks. **It leaves the ~2,900 short comments alone** and does not
gate length, which ADR-0127 argues would fire on the wrong thing and be gameable by a blank line.

## Context & problem

The verbosity is a harness artifact, not carelessness. Every phase commit in this project is written
by someone with a plan document open, and the plan's reasoning leaks into the code it produces. The
result is a codebase whose comments are, in bulk, a lossy second copy of `docs/` — with the two
properties a second copy always has: nothing compares it against the original, and it outlives the
context that made it legible.

`core/src/render/tonemap.rs` is the representative case. Its 100-line header carries a genuine trap
(*"Not skippable ... it is the format boundary"*), a formula and a C1-continuity argument worth
having in the code — and alongside them four lines restating ADR-0046's requirements, a note that
*"ADR-0028 / ADR-0032 are unchanged by this plan"*, and framing like *"the clipped composite this
plan exists to retire"*. The first group is why the file is navigable. The second is why it is 100
lines.

The links are the sharpest edge because they fail silently in three ways at once: they break when a
plan moves to `plans/done/` (which is a routine step of every close), they are invisible to
`scripts/check-doc-links.mjs`, which walks `.md` only, and they do not resolve in rendered rustdoc
at all. Eleven are broken on `main` right now, found at Plan 0117's close only because that plan's
own new link had to be repointed by hand.

**Measured at `e022a5d`**, over `core/src`, `standalone/src`, `lmv-ring/src`, `core-cabi/src`,
`core/tests` and `standalone/tests`:

| | total | outside contended trees | inside them |
|---|---|---|---|
| files in scope | 106 | 75 | 31 |
| relative links in comments | 89 | 56 | 33 |
| lines inside blocks of 40+ | 3,850 | 3,124 | 726 |
| plan-relative narration lines | 252 | 182 | 70 |

"Contended" is `core/src/render/scenes/` and `core/src/render/scenes/lines/`, where the live
`plan-0113-shape-collage` and `plan-0087-arc-primitive` worktrees are working. That split is why
Phases 6 and 7 are separate.

## Decision

**Take ADR-0127's rule, sweep the bounded subset, and gate only what a script can judge.**

Three things this plan deliberately does not do, each an ADR-0127 alternative with its reason:
it does not extend `check-doc-links.mjs` to `.rs` (Alternative A — that maintains a checker forever
to protect links that should not exist); it does not cap comment-block length (Alternative B — a
blank line games it, and it fires on `kaleidoscope.rs`'s mostly-legitimate 191-line header); and it
does not sweep all 28,064 lines (Alternative D — nine live lanes, 125 files).

**The rule that governs every deletion in Phases 3, 4, 6 and 7, and it is the one that makes this
plan safe:** a comment sentence may be deleted only when the fact it states is *already in* the ADR
or plan it cites. When the comment carries something the document does not — a measurement, a
caveat, a constant's provenance — **`dev` keeps the comment and records the file and line in the
implementation log.** `dev` does not write ADRs, and this plan does not change that; the architect
promotes those at the close. A sweep that deletes on sight would destroy the only copy of facts this
project paid for, and would be worse than the verbosity it fixes.

## Architecture diagram

```mermaid
flowchart TB
    C["a comment on the code"]
    C --> M["mechanism, invariant, trap,<br/>un-derivable formula"]
    C --> D["why this beat the alternative,<br/>what was measured, the threshold's argument"]
    C --> N["'this plan', 'used to',<br/>'no longer', 'is new'"]
    C --> L["[label]: ../../docs/adrs/....md"]

    M --> KEEP["stays in the comment"]
    D --> ADR["docs/adrs/ + docs/plans/<br/>cited by BARE NUMBER"]
    N --> DEL["deleted, or restated as<br/>a property of the code"]
    L --> DEL2["deleted; bare number instead"]

    N -.-> G["scripts/check-comment-hygiene.mjs<br/>pre-push - CI links job - close ceremony"]
    L -.-> G
    M -.->|"length is NOT gated"| REV["Mode 4 review, lens 3"]

    D -->|"absent from the ADR?"| FLAG["kept, and logged for<br/>architect to promote"]
```

## Implementation phases

### Phase 1 — the gate exists, and it only reports

- **Owner skill:** dev
- **What:** `scripts/check-comment-hygiene.mjs`, in the shape of its three siblings — walks the
  workspace, takes an optional `root` argument for the fixture tree, prints `file:line -> reason`,
  exits 1 on any finding. It rejects exactly two classes: a **relative link** in a `.rs` comment
  (both the `[label]: target` and `](target)` forms, targets starting `../` or `./`), and the
  **plan-relative vocabulary** (`this plan`, `the plan`, `used to`, `no longer`, `is new`,
  `previously`). Rustdoc intra-doc links are **not** a finding — `rustc` resolves those and they
  cannot rot silently. A documented escape comment suppresses one line, because the vocabulary list
  will have false positives.
  **Not wired into anything yet** — arming a red gate would block every push in the repo.
- **Files touched:** `scripts/check-comment-hygiene.mjs`, `scripts/fixtures/`.
- **Done when:** run bare, it reports both classes across the workspace and exits 1. Run against the
  seeded fixture tree it exits 1 with exactly one finding of each class and nothing else, the way
  `check-doc-links.mjs scripts/fixtures` already works. A comment containing the word "plan" in a
  sentence that is not plan-relative narration is **not** reported — the fixture tree carries one,
  because a gate that cries wolf gets escaped rather than obeyed.

### Phase 2 — the rule lands where authors read it

- **Owner skill:** dev
- **What:** ADR-0127's rule, stated once in `CLAUDE.md` under the cross-cutting non-negotiables, and
  pointed at from `.claude/skills/dev/SKILL.md` so the lane that writes comments reads it while
  writing them. Both entries say what a comment carries, what goes to the ADR, that citations are
  bare numbers, and that the gate exists. Neither restates ADR-0127 at length — that would be this
  plan committing the defect it is fixing, in the document that forbids it.
- **Files touched:** `CLAUDE.md`, `.claude/skills/dev/SKILL.md`.
- **Done when:** both name the rule in under ~12 lines each and cite `ADR-0127` by bare number. The
  ADR is the authority; these are pointers.

### Phase 3 — the 89 links go

- **Owner skill:** dev
- **What:** every relative link in a `.rs` comment becomes a bare-number citation — `ADR-0046`,
  `Plan 0045 Phase 3`. The **eleven already-broken ones are resolved by number, not deleted**: a
  broken link still names a real document, and `core/src/render/tests.rs:1284`'s `plans/0053-…`
  is Plan 0053 at its `done/` path. Where a link's label was carrying meaning the bare number loses
  (a title a reader needed), the title stays as prose.
- **Files touched:** ~40 `.rs` files across all six roots.
- **Done when:** `check-comment-hygiene.mjs` reports zero link findings. Every number that replaces
  a link resolves to a real file — check it, since a bare number that names nothing is a worse
  citation than a broken link, being unfalsifiable by any gate. Intra-doc links are untouched, and
  `cargo doc --workspace --no-deps` emits no new warnings.

### Phase 4 — the 252 narration lines go

- **Owner skill:** dev
- **What:** each is deleted or restated as a property of the code. *"used to be free-running until
  Plan 0095"* becomes *"the phase is locked, not free-running"* — same fact, no expiry. The
  **deletion rule from the Decision applies**: where the narration is the only record of something,
  it is kept and logged.
- **Files touched:** ~60 `.rs` files.
- **Done when:** `check-comment-hygiene.mjs` reports zero vocabulary findings, and every escape
  comment used is justified in one line at its site. `cargo nextest run --workspace` is green —
  comment-only edits cannot change behaviour, so a red run means something else moved.

### Phase 5 — the gate is armed

- **Owner skill:** dev
- **What:** `check-comment-hygiene.mjs` joins `.githooks/pre-push` and the CI `links` job, beside
  the three checkers already there. It runs before `fmt`, since it is the cheapest.
- **Files touched:** `.githooks/pre-push`, `.github/workflows/*.yml`.
- **Done when:** both call it, a seeded violation fails the hook locally, and CI is green on the
  swept tree. **Pushing a `.github/workflows/` edit needs the `workflow` OAuth scope on the git
  credential** — if the push is rejected, that is the cause, and `gh auth refresh -s workflow`
  is the fix rather than anything in the diff.

### Phase 6 — the long blocks come down, outside the contended trees

- **Owner skill:** dev
- **What:** the 44 blocks of 40+ lines, in the **75 files outside** `core/src/render/scenes/`, held
  to ADR-0127's rule — mechanism, invariant, trap, un-derivable formula stay; the restated decision
  record goes, cited by number. The tonemap header in ADR-0127's Context is the worked example of
  the target shape. **The Decision's deletion rule is the governing constraint of this phase.**
- **Files touched:** ~75 `.rs` files, the heaviest being `core/tests/sanity.rs`,
  `core/src/render/background.rs`, `kaleidoscope.rs`, `tier.rs`, `bloom.rs`,
  `standalone/src/shot/render.rs`.
- **Done when:** no block over 40 lines survives in these files **unless** the log names it and says
  in one line what invariant needs the length — `kaleidoscope.rs`'s 191 lines may well be such a
  case, and ADR-0127 explicitly declines to gate this, so the log is the only record. No numeric
  target is set on how far the total falls: the ratio is an outcome of applying the rule, not a
  quota to hit, and a quota would be met by deleting the wrong lines. `cargo nextest run --workspace`
  green and `cargo doc --workspace --no-deps` warning-free.

### Phase 7 — the long blocks come down, inside the contended trees

- **Owner skill:** dev
- **What:** the same work for the **31 files** under `core/src/render/scenes/` and
  `core/src/render/scenes/lines/` — 726 lines in 40+ blocks, 33 of the links, 70 narration lines.
  (Phases 3 and 4 already took the links and narration here; this phase is the trimming only.)
- **Files touched:** ~31 `.rs` files under `core/src/render/scenes/`.
- **Done when:** same bar as Phase 6. **This phase is separable and may be deferred** — the
  `plan-0113-shape-collage` and `plan-0087-arc-primitive` worktrees are live in exactly these
  directories, and a comment-only diff that collides with them buys a merge conflict for a cosmetic
  gain. Deferring it leaves the repo consistent, because the two *gated* classes are already clean
  here from Phases 3 and 4. If deferred, say so in the log and the close files it as a backlog entry.

## Risks & open questions

- **The deletion rule is the whole safety of this plan, and it is the part that erodes under
  fatigue.** Phases 6 and 7 are 3,850 lines of judgement, and the tempting move on line 3,000 is to
  delete a sentence that "looks like" ADR restatement without opening the ADR. The mitigation is
  structural rather than exhortative: `dev` logs what it keeps, and the close reads that list. A
  Phase 6 log with zero kept-and-flagged lines is itself suspicious — across 44 dense blocks it is
  unlikely that every fact was already in a document.
- **The vocabulary gate will annoy someone.** `no longer` and `the plan` occur in innocent
  sentences. The escape exists for that, and Phase 1's fixture pins one false-positive case — but if
  the escape count after Phase 4 is more than a handful, the word list is wrong and should be
  narrowed rather than escaped around. Report the count.
- **Comment-only diffs are invisible to every test in the repo.** Nothing here can be verified by
  the suite beyond "it still compiles and still passes", so the green run in each done-when is a
  floor, not evidence. The actual verification is reading, and the review is where it happens.
- **`cargo doc` behaviour on the removed links is assumed, not measured.** ADR-0127 states these
  hrefs resolve against the generated HTML tree rather than the repo; Phase 3's done-when checks
  only that no *new* warnings appear. If it turns out rustdoc was silently emitting broken anchors
  all along, that is a finding worth one line, not a change of plan.

## What this plan does NOT do

- **It does not gate comment length**, in any file, by any threshold — ADR-0127 Alternative B, with
  its reasons. Length stays a Mode 4 judgement.
- **It does not touch the ~2,900 comment blocks under 40 lines** that carry no link and no
  narration. Most of them are the load-bearing layer and are the reason this codebase reads well.
- **It does not extend `check-doc-links.mjs`**, and it retires design-backlog 0129, which proposed
  exactly that before this question was asked.
- **It does not remove rustdoc intra-doc links.** Those are resolver-checked and are the linking
  mechanism ADR-0127 keeps.
- **It changes no behaviour.** Every phase is comments, one new script, and two gate call sites. No
  Rust expression, no constant and no test assertion moves. A moved golden or a changed test result
  in any phase means something went wrong, not that a comment was cut.

## Implementation log

> Written by `dev` — one row per phase as that phase's commit lands, and the close block after the
> last one. **The phases above are the contract; everything here is what happened.**

| phase | owner | state | commit |
|---|---|---|---|
| 1 — the gate exists, and it only reports | dev | | |
| 2 — the rule lands where authors read it | dev | | |
| 3 — the 89 links go | dev | | |
| 4 — the 252 narration lines go | dev | | |
| 5 — the gate is armed | dev | | |
| 6 — the long blocks come down, outside the contended trees | dev | | |
| 7 — the long blocks come down, inside the contended trees | dev | | |
