# 0094 — the two doc gates check what they claim to

> **Status:** draft
> **Created:** 2026-08-15
> **Owner skill(s):** dev
> **Related ADRs:** [ADR-0108](../adrs/0108-a-backlog-claim-about-the-repo-carries-an-executable-probe.md) (a backlog claim about the repo carries an executable probe), [ADR-0016](../adrs/0016-gpu-tests-opt-in-ci-scope.md) (a gate that cannot run says so and skips)
> **From:** [Plan 0093](done/0093-the-backlog-stops-asserting-things-about-a-repo-it-has-not-read.md)'s Mode 4 review — two major findings and one minor, all three in `scripts/`

## TL;DR

Plan 0093's close review found that both of this project's markdown gates currently overstate their
coverage. `check-doc-links.mjs` skips **any** directory named `fixtures`, which quietly dropped
`core/tests/fixtures/README.md` and its twelve relative links out of the walk — 257 markdown files
became 248. `check-backlog-claims.mjs` checks every verification bullet it finds but never asks
whether an entry **has** one, which is the sentence ADR-0108's Decision actually makes. And on CI —
the un-bypassable call site — the staleness advisory reads a shallow clone, where `git log -1` hands
back the tip commit for every path, so from the first run dated after the stamps it reports the whole
roster as moved.

Three small fixes in two script files. Nothing here is broken today; all three are the same failure
in different clothes — **a check that reports green over ground it never covered**, which is the exact
thing Plan 0084 was written for and the exact thing Plan 0093 was written for.

## Context & problem

**Finding 1 — the fixtures skip is wider than the thing it was approved for.** Plan 0093 Phase 1
needed a committed fixture tree carrying deliberately broken links, and that tree cannot sit in the
walk of the checker it bites. The approved expansion was three lines to skip it. What landed
(`scripts/check-doc-links.mjs:59`, `isFixtureRoot` + `entry.name === "fixtures"`) matches the
directory *name* anywhere in the tree, and this repo has a second one:

```
core/tests/fixtures/README.md            12 relative links, 4 of them into docs/plans/done/
core/tests/fixtures/scratch-0046/README.md
core/tests/fixtures/scratch-0082/README.md
```

Those three are link-clean as of 2026-08-15 (verified by running the pre-change checker over the
repo), so nothing is broken — they are simply no longer read by anything, and their links point into
`docs/plans/done/` and `docs/adrs/`, which is precisely the set a close ceremony moves. A silent
coverage hole under a green gate is the shape Plan 0084 existed to close.

**Finding 2 — the claim gate never requires a bullet.** ADR-0108's Decision reads *"every live entry
in `docs/design-backlog.md` that makes a claim about the repository **carries** a dated verification
bullet with at least one machine-runnable probe"*. The script enforces the second half and not the
first: `check()` breaks on a bullet whose spans are empty, but `entries` is built *from bullets*, so
an entry with no bullet at all is invisible to it. All 14 live entries comply today because Phase 2
did it by hand. Entry 0095, written next month by someone who has not read ADR-0108, passes green —
and "the duty lives only in prose, so it decays" is the entire argument Plan 0093 was built on.

**Finding 3 — the advisory is noise on the one call site that cannot be skipped.** The `links` job
uses `actions/checkout@v4` at its default `fetch-depth: 1`. In a shallow clone the tip commit is
grafted parentless, so every file reads as added by it and `git log -1 --format=%cs -- <path>` returns
the tip date for **every** path. Measured against a local `--depth 1` clone: `core/src/dsp/mod.rs`,
`presets/README.md` and `core/src/render/palette.rs` all reported the tip date rather than their own.
It is quiet today only because the tip and the stamps are both 2026-08-15. From the first CI run
dated later, all 25 probed paths report as moved on every run, and the `unprobeable:` roster that
shares the block — the countable set of unchecked claims, which is the whole reason the block
exists — is buried under them.

That third one is worth naming as a class and not just a bug. It is the review lens about a value
sourced from two places that **agree on the one configuration we develop at**: full history locally,
shallow on CI, identical output on the single day the feature was written. Nothing at the
development configuration could have caught it.

## Decision

Fix all three in place; add no dependency, no new script, and no new call site.

The one real choice is Finding 3, and this plan takes the **notice** route over `fetch-depth: 0`:
the checker detects a shallow repository and prints one line in place of the moved block, in
[ADR-0016](../adrs/0016-gpu-tests-opt-in-ci-scope.md)'s shape — *a check that cannot run says so
rather than reporting something it did not measure*. The rejected alternative is `fetch-depth: 0` on
the `links` job, which buys a real advisory on CI at the cost of a full-history fetch on every run,
for a block that never touches the exit code. A gate should be honest wherever it runs rather than
correct only where a workflow was configured for it — and the pre-push call site, where a human is
looking, has full history and keeps the real advisory.

No ADR: none of the three has a rejected alternative worth a durable record beyond the paragraph
above, and ADR-0108's Decision is being **implemented** here rather than revised.

## Implementation phases

### Phase 1 — the link gate stops skipping real docs

- **Owner skill:** dev
- **What:** narrow the skip in `scripts/check-doc-links.mjs` from the directory *name* to the one
  fixture root it was approved for, so `core/tests/fixtures/` returns to the walk. The skip must stay
  path-based rather than name-based; a second seeded tree added later should have to name itself.
  Update the comment beside it — it currently explains the wrong rule.
- **Files touched:** `scripts/check-doc-links.mjs`, `scripts/fixtures/README.md` (the sentence
  describing what gets skipped).
- **Done when:** the repo walk reads **257** markdown files, up from 248, with all nine previously
  dropped files accounted for: the three under `core/tests/fixtures/` are checked again, and the six
  under `scripts/fixtures/` are still skipped. `node scripts/check-doc-links.mjs` exits 0 and
  `node scripts/check-doc-links.mjs scripts/fixtures` still reports exactly its three seeded breaks.
  (257 is the count on the tree at this plan's writing — if the repo has gained or lost markdown by
  the time this runs, the property is *the three `core/tests/fixtures/` READMEs are in the walk and
  the `scripts/fixtures/` tree is not*, and that is what to assert.)

### Phase 2 — a live entry without a verification bullet is a break

- **Owner skill:** dev
- **What:** in `scripts/check-backlog-claims.mjs`, collect the live `## NNNN` entry headings the
  parser already recognises, and report every one that carries no dated verification bullet. The
  message must name what to do — a probe or a reasoned `unprobeable:` — because the reader hitting it
  is by definition someone who has not read ADR-0108. Section preambles (`## Entry 0021 — …`,
  `## Entries 0068-0069 — …`) are already excluded by the `ENTRY` regex and must stay excluded;
  everything above `## Open entries` stays out of scope, as it is today.
- **Files touched:** `scripts/check-backlog-claims.mjs`, `scripts/fixtures/backlog-claims/docs/design-backlog.md`
  (one more seeded case), `scripts/fixtures/README.md` (its table).
- **Done when:** a sixth fixture case — a live entry heading with no bullet beneath it — is reported
  exactly once and named as such, `--self-test` covers it and still passes in full, and the run
  against the repo at `HEAD` still exits 0 with all 14 live entries accounted for. Add the count of
  live entries to the green line so the number the gate is reasoning over is visible rather than
  implied.

### Phase 3 — the advisory says when it cannot see the history

- **Owner skill:** dev
- **What:** ask `git rev-parse --is-shallow-repository` once, through the same `execFileSync`
  argument-array route `lastTouched` already uses. On a shallow clone, print one line in place of the
  moved block naming why the reading is unavailable; the `unprobeable:` roster still prints, since it
  needs no history. Say it in the header comment too, beside the existing note on why `git log` is
  exec'd at all.
- **Files touched:** `scripts/check-backlog-claims.mjs`.
- **Done when:** run against a `git clone --depth 1` of this repo, the output carries the notice and
  no moved rows, still prints the `unprobeable:` roster, and exits 0; run against the ordinary
  checkout it is unchanged from today, which includes still reporting a real moved row when an
  entry's stamp is backdated. Both halves are one command each and both go in the phase commit.

## Risks & open questions

- **Phase 2 can red the gate for a `dev` who adds an entry mid-plan.** That is the design and it is
  cheap to satisfy — but the message has to make the fix obvious in one read, or the gate becomes the
  thing someone disables (ADR-0033's standing warning about gates that hurt).
- **`--is-shallow-repository` predates nothing we care about** (git 2.15+), and the call is already
  wrapped in the `try` that returns null for "no git, not a repo". A failure to detect shallowness
  should fall back to today's behaviour rather than to the notice — reporting the moved block on a
  full clone is right, and printing the notice on one would be a new lie.
- **Nothing here makes a weak probe strong.** `absent:` on a common word still reads as verification
  while checking nothing; ADR-0108 accepts that there is no mechanical defence and this plan does not
  invent one.

## What this plan does NOT do

- **It does not extend either gate to a new scope.** Plans as a second scope for the probe grammar is
  still the followup Plan 0093 left; this plan is repairs to what already ships.
- **It does not change `fetch-depth` on any CI job**, for the reason argued in Decision.
- **It does not touch `docs/design-backlog.md`'s entries.** The one entry Plan 0093 convicted was
  corrected at that plan's close.
