# ADR-0127 — a comment carries the mechanism, and the decision record stays in `docs/`

> **Status:** **accepted** (Plan 0118, closed 2026-08-27)
> **Date:** 2026-08-25
> **Related plan(s):** [0118 — the comments stop narrating the plans that wrote them](../plans/done/0118-the-comments-stop-narrating-the-plans-that-wrote-them.md)
> **Supplements:** [ADR-0116](0116-an-index-row-is-a-pointer-and-a-gate-holds-it-to-one.md),
> whose argument about index rows this applies one layer down

## Context

This workspace holds **28,064 comment lines against 53,489 lines of code** — 0.52 comment lines per
code line, with twelve files where comments outnumber code and module headers running to 191, 140,
125, 118 and 106 lines. That is not the product of one careless session. It is what a plan-driven
harness produces when every phase commit is written by someone holding a plan document open, and it
has three distinguishable layers that have been treated as one thing.

**The load-bearing layer** says what the code does, what invariant it holds, and what breaks if you
change it. `tonemap.rs`'s *"Not skippable — every other pass in `render/` skips when its amount
param is off. This one cannot: it is the format boundary"* is a trap a reader needs and cannot
derive. This layer is why the codebase is navigable and is not in question.

**The duplicated decision record** restates, in the code, what an ADR already decided: which
alternative lost, what was measured, what a threshold was argued from. **2,497 comment lines
(8.9 %) cite a `Plan` or `ADR` number**, and 573 name a specific plan *phase*. `tonemap.rs` spends
four lines restating ADR-0046's three requirements for the tonemap curve — which ADR-0046 states, in
more detail, with the alternatives it rejected. This is a second copy of a document that already
exists, and a second copy is the copy that drifts. That is
[ADR-0116](0116-an-index-row-is-a-pointer-and-a-gate-holds-it-to-one.md)'s whole argument, made
about index rows and equally true here.

**The plan-relative narration** is written from inside a session and dies at its close: `this plan`,
`is new`, `no longer`, `used to`, `previously`. **252 comment lines** carry it. `tonemap.rs:10` reads
*"ADR-0028 / ADR-0032 are unchanged by this plan — only the pass that hands ink its input is new"*,
which was a useful sentence for one review and has been a puzzle ever since. There is no "this plan"
any more; there is only the code.

Cutting across all three, **89 relative links** (`[label]: ../../docs/adrs/…`) sit in Rust comments.
Eleven are broken on `main` today, found at Plan 0117's close, and `scripts/check-doc-links.mjs`
cannot see any of them — it walks `.md` files only. One of the eleven is a `plans/done/` move that
the close ceremony's own step 1b exists to prevent and did not catch. Worth naming plainly: these
links do not work in rendered rustdoc either, where the href is emitted as written and resolves
against the generated HTML tree rather than the repository. They have only ever resolved for a
reader looking at the source file in an editor or on GitHub.

## Decision

**A comment states the mechanism and the trap. The decision record stays in `docs/` and is cited by
bare number.** Concretely, in every `.rs` file in this workspace:

- **What belongs in a comment:** what this code does; the invariant it holds; the trap that would
  bite someone changing it; and any formula, derivation or constant a reader cannot re-derive from
  the code in front of them.
- **What goes to the ADR or plan instead:** why this approach beat the alternative, what was measured
  to decide it, what a threshold was argued from, and what the code used to do.
- **How to cite it:** the bare number — `ADR-0046`, `Plan 0045 Phase 3`. **No relative links in Rust
  comments.** `grep -rn 0046 docs/adrs` resolves it in one command and there is no path left to rot.
  Rustdoc **intra-doc** links are unaffected and stay — `rustc` resolves those, they cannot go stale
  silently, and they are the linking mechanism this rule deliberately leaves in place.
- **No plan-relative narration.** A comment describes the code as it is. If the previous behaviour
  matters, it is stated as a property of the code (*"the phase is locked, not free-running"*), never
  as a history (*"used to be free-running until Plan 0095"*).

**The mechanical half is gated; the length is not.** `scripts/check-comment-hygiene.mjs` joins the
three gates already running at `pre-push`, in the CI `links` job and in the close ceremony, and
rejects exactly two things: a relative link in a `.rs` comment, and the plan-relative vocabulary.
**It says nothing about how long a comment may be**, because length is not the defect — a sixty-line
block explaining a genuinely hard invariant is right, and a twelve-line block restating an ADR is
wrong. That judgement belongs to the Mode 4 review, and this ADR gives it a rule to judge against.

## Consequences

### Positive

- **The rot class the gate cannot see disappears rather than being guarded.** Deleting 89 links ends
  the failure mode permanently; no checker has to be built, run or maintained to protect them.
- **One authority per fact.** A reader who wants to know why the knee sits at 0.6 goes to ADR-0046,
  which has the alternatives and the argument, instead of to a code comment holding a summary of it
  that nothing compares against the original.
- **The comments that remain get read.** A 100-line module header is skimmed; a 19-line one that is
  all mechanism is read. This is the argument that moved the close write-ups out of
  `docs/plans/README.md`, and it held there.
- **The `dev` lane gets a rule it can apply while writing**, rather than a taste it has to infer from
  the surrounding files — which, given those files, currently teaches the opposite.

### Negative

- **A bare number is not clickable, and that is a real loss for a reader in an editor.** Accepted
  because the link was already not clickable in rustdoc, was broken eleven times over in the source,
  and `grep` costs one command. This is the price, paid knowingly.
- **Deleting the duplicated record assumes the ADR actually holds it, and sometimes it will not.** A
  comment may carry the only surviving statement of a measurement or a caveat. The sweep in Plan 0118
  must therefore *check the ADR before deleting* and promote anything missing — which is the
  expensive part of the work and the part most likely to be skipped under time pressure. A sweep that
  deletes on sight destroys knowledge and would be worse than the verbosity it fixes.
- **The gate's vocabulary list will produce false positives.** "The plan" occurs in legitimate
  sentences. The gate needs a documented escape, and an escape is a thing that gets over-used.
- **Nothing holds the length down.** This ADR consciously declines to gate the actual verbosity,
  which means verbosity can regrow, exactly as index rows did before ADR-0116. The bet is that a
  stated rule plus a review lens suffices *for a judgement call* where it did not for a mechanical
  one. If comment volume is measured again in a later plan and has regrown, that bet lost and a cap
  is the successor decision.

### Neutral

- Comment volume becomes a measurable property of the repo — 0.52 comment lines per code line at
  `e022a5d` — so the bet above can be settled with a number rather than an impression.

## Alternatives considered

### Alternative A — extend `check-doc-links.mjs` to walk `.rs` files

This is what [design-backlog 0129](../design-backlog.md) proposed at Plan 0117's close, and it is the
obvious repair: the checker already extracts links, skips code spans and reports `file:line`; one
directory walk and one extension test wider, and the eleven breaks become visible.

**It lost because it institutionalizes the thing being removed.** It commits the project to
maintaining a checker forever in order to protect 89 links that do not work in rustdoc, exist only to
save a reader one `grep`, and encode a file path — the most brittle possible reference to a document
whose normal lifecycle includes being moved into `plans/done/`. Deleting them is a bounded one-time
edit; guarding them is unbounded. The backlog entry was filed before this question was asked, and is
superseded by this ADR rather than wrong.

### Alternative B — gate contiguous comment-block length, in ADR-0116's shape

A hard ceiling on a comment block, enforced the way the 320-byte roster row is. Today `N = 40` would
fail 61 blocks, `N = 30` would fail 91, `N = 20` would fail 193.

**It lost on what it would actually catch.** Length is a symptom that correlates weakly with the
defect: the rule fires on `kaleidoscope.rs`'s 191-line header, which is mostly genuine mechanism, and
passes a 12-line block restating an ADR verbatim. Worse, it is gameable in the one way that makes
things strictly worse — a blank line splits any block in two and satisfies the gate without changing
a word, which teaches authors to fragment prose rather than cut it. ADR-0116's cap works because an
index row is a *fixed-shape* object where length and defect are the same thing. A comment is not.

### Alternative C — write the convention down and sweep nothing

Free today, and existing comments would decay toward the rule as files are touched.

**It lost on this project's own evidence.** Plan 0061 Phase 7b wrote *"One line per plan."* into
`docs/plans/README.md`, three lines above the rows it governed; eight days later that section had
regrown **7.1x**, which is the finding ADR-0116 exists to record. A rule with nothing behind it has
been tried here and did not hold. The compromise this ADR takes is to gate the half a script can
judge, and sweep the rest once.

### Alternative D — sweep all 28,064 comment lines

Thorough, and it would settle the question in a single pass.

**It lost on collision.** It touches all 125 `.rs` files, and nine plan lanes are live (0087, 0092,
0098, 0103, 0104, 0113, 0114, 0115, 0116). A diff of that shape would have to run with every other
lane parked, and the merge cost would exceed the benefit. Plan 0118 takes the bounded subset — the 89
links, the 61 blocks over 40 lines, the 252 narration lines, across 106 files — and leaves the 7,913
blocks of 40 lines or fewer to the convention.

## Notes

Measured 2026-08-25 over `core/src`, `standalone/src`, `lmv-ring/src`, `core-cabi/src`, `core/tests`
and `standalone/tests`, at commit `e022a5d`. The 28,064 / 53,489 figures cover the four `src` roots;
the sweep totals (89 links, 3,850 lines inside 61 blocks of 40+, 252 narration lines, 106 files)
include the two test roots, which is why they exceed the numbers quoted in Context. **Every block
count in this ADR is over the sweep scope**; the 191/140/125/118/106-line headers named in Context
are `src`-root figures and are a subset of it.

The eleven broken links that prompted this were found at
[Plan 0117](../plans/done/0117-the-downbeat-log-sees-the-counter-it-folds-over.md)'s close, in the
course of repointing that plan's own new link by hand.
