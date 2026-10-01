# ADR-0234 — An instrument's system roster is derived from the enum the engine reads

> **Status:** accepted 2026-09-26 (Plan 0209)
> **Date:** 2026-09-19
> **Related plan(s):** [0209](../plans/done/0209-a-system-joins-the-instruments-by-existing.md)

## Context

This engine has repeatedly written down a list of its own systems and then grown past it. Two live
instances, found on the same day from opposite ends of the repository:

**The `distinctness` report** — the one instrument that asks whether two shipped presets have
converged — reads its roster from a hand-written array, `const FAMILIES: [(SystemKind, &str); 9]` in
`core/tests/distinctness.rs`. Five shipped families are absent: `analytic_field` (12 presets),
`shape_field` (9), `warp_mesh` (7), `shape_collage` (4) and `cellular` (3). That is 35 of 114 shipped
presets with no similarity check of any kind, and `docs/testing.md` states the consequence in its own
words: *"a new `SystemKind` does not appear in it on its own and nothing fails when one is missing."*
Both prose carriers that name the absent families name three of the five (backlog 0256).

**The content lane's scene catalogue** — `.claude/skills/preset-author/references/systems.md`, the
document that carries what the generated parameter roster cannot: what a scene is for, the working
range distilled from the shipped set, and which audio input it rides. It has a section for ten systems
and none for `warp_mesh`, `shape_collage`, `analytic_field` or `cellular` (backlog 0258).

**The pattern is the same and it is not a discipline problem.** A hand-written roster that the build
does not consult is invisible when it falls behind: nothing is red, the instrument runs, the report
prints, and a reader cannot tell a deliberate omission from a forgotten one. `core/tests/distinctness.rs`
contains the proof in its own comment, four lines above the list it is wrong about: *"A count is a
fine reason to leave a family out and a terrible one to leave written down, because it stops being
true silently."*

**This project has already solved this once.** [ADR-0022](0022-build-time-preset-embedding.md) made a
preset ship by existing — `core/build.rs` globs `presets/*.toml` and there is no array to edit and no
count to bump. [ADR-0170](0170-a-parameters-reference-row-is-generated-from-the-declaration-the-engine-reads.md)
did the same for the parameter reference, generating each row from the `ParamSpec` the engine reads.
`SystemKind::row` is already the exhaustive match that fails the build when a variant has no entry.
The mechanism is present and the instruments simply do not use it.

## Decision

**An instrument that enumerates systems derives its roster from `SystemKind`, and a variant with no
entry fails the build rather than falling out of the report.** `distinctness`'s array is replaced by a
derivation over `SystemKind::ALL` whose per-variant data comes from an exhaustive match, so adding a
variant is a compile error until the new system is given its family name — the same shape
`SystemKind::row` already uses to keep the roster honest.

**Where a roster cannot be derived because its content is human judgement, it is declared instead.**
`systems.md`'s per-scene guidance cannot be generated — a working range distilled from the shipped set
is a look judgement — so that file carries an entry per `SystemKind` even when the entry reads that no
guidance is written yet. A declared hole is probeable and a silence is not, which is the whole
difference between the two failures above.

**A count of the systems stays out of prose entirely**, per
[ADR-0202](0202-a-written-count-of-the-systems-is-refused-by-a-gate.md), and this
ADR extends that rule's reasoning from a count to a *list*: an enumeration goes stale exactly the way
a count does, and `scripts/check-system-counts.mjs` cannot see it because there is no count token to
match.

## Consequences

### Positive
- A system landing in the engine joins the similarity report by existing, which is the property
  ADR-0022 gave presets and ADR-0170 gave parameters.
- The 35 presets with no similarity check get one, including the second-largest family in the set.
- A hole in the content lane's catalogue becomes visible to the lane reading it, instead of reading as
  "this system has no presets worth writing".

### Negative
- **The derived roster reports on families nobody has calibrated.** `distinctness`'s thresholds were
  chosen against nine families; five arrive at once, and some may flag near-duplicates that are
  correct or miss ones that are not. The report is advisory, so this costs reading rather than a red
  gate — but the first run over fourteen families is evidence about the thresholds as much as about
  the library.
- **A declared-but-empty catalogue entry is an invitation to fill it badly.** Generating those four
  sections from the parameter roster would satisfy the declaration while supplying none of the
  judgement the file exists for, which is worse than the hole. The plan forbids it and nothing
  mechanical can.
- **The compile-time failure lands on whoever adds a system**, one edit further from the scene they
  were writing. That is the cost of the property and it is the same cost `SystemKind::row` already
  charges.

### Neutral
- The curation question behind backlog 0256 — whether the library should ship less and better — is
  untouched. This decision is about the instrument, and the evidence for that question is what
  [Plan 0205](../plans/done/0205-the-library-becomes-navigable.md) builds.

## Alternatives considered

### Alternative A — keep the array and add a gate that compares it to `SystemKind`
A `check-*.mjs` in the gate roster, or a test, asserting the array covers every variant. Rejected
because it is strictly more machinery for strictly less: a derivation makes the divergence
unrepresentable, while a gate makes it representable and then reports it. The gate is also a third
place the roster is written down, which is the failure mode being repaired.

### Alternative B — derive the roster from the shipped filenames instead of the enum
`presets/*.toml`'s filename families are what the report's rows are actually keyed on, and globbing
them needs no exhaustive match. Rejected because it makes the instrument blind in exactly the case
that matters: a system shipped with no presets yet, or a family renamed, produces a roster that
silently tracks the content rather than the engine. The enum is what the engine reads.

### Alternative C — write the four catalogue sections and leave both rosters hand-written
The cheapest thing that fixes today's two symptoms. Rejected because it fixes the instances and not
the mechanism, and the mechanism has now produced the same failure twice in two places within roughly
four months.

## Notes

Backlog 0256 carries the `distinctness` measurements and the two stale prose carriers; 0258 carries the
catalogue gap and the observation that the close ceremony's operator-doc sweep does not name the file.
