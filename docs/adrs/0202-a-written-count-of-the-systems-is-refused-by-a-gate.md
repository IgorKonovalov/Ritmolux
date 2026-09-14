# ADR-0202 — A written-out count of the systems is refused by a gate, and the threshold is five

> **Status:** proposed
> **Date:** 2026-09-14
> **Related plan(s):** [0178](../plans/0178-what-the-operator-reads-is-true.md)
> **Extends:** [0168](0168-the-reader-documents-address-a-reader-and-the-record-stays-a-link.md)
> (a reader-document rule carried by a gate), [0163](0163-a-long-document-carries-a-generated-contents-block.md)
> and [0149](0149-a-backlog-reference-is-a-bare-number-and-a-file-link.md) (the same convention-to-gate
> substitution)

## Context

`CLAUDE.md` asks for count-free phrasing: *"Prefer count-free phrasing ('the whole embedded set')
over hard numbers that re-drift."* Nothing enforces it. The cost comes back once per new system,
permanently. `analytic_field` and `cellular` landed three days apart, and each close found sentences
that had written the count down: *"all twelve"* in `docs/capturing.md`, *"All twelve systems have
one"* in `docs/preset-guide.md`, and five in `core/src/render/scenes/common.rs`. One of those five
sits inside an assertion **message**, a string literal rather than a comment. All of them were found
by eye, one close late (backlog 0208).

Counts outside the plan and ADR records were swept on 2026-09-14, and the pattern is clear. The roster
has **14** systems (`SystemKind`, `core/src/preset/schema/system.rs`).

| reads | where | what it is |
|---|---|---|
| seven, ten, twelve | `core/src/render/evaluate.rs:406,480`, `common.rs:3,366`, `warp_mesh/mod.rs:690` | stale roster totals, or complements of a total ("the other ten systems") |
| nine / fourteen, nine / ten, twelve | `.claude/skills/preset-author/SKILL.md:80`, `docs/content-brief.md:300`, `presets/proposed/ROSTER.md:53` | a total recorded as of a date, in a live document |
| six of the eight, eight | `core/tests/sanity.rs:277,1800` | a historical total inside a doc comment |
| 82 | `core/tests/hygiene.rs:642` | not a roster count at all |
| one, two, four | `presets/README.md:2407`, `system.rs:174,188`, `preset-guide.md:229` | a singular, a pair, a named cohort |

Two things follow. **A count goes stale whether it is correct today or not.** A true "fourteen
systems" is wrong the day the fifteenth lands, so a gate that compares prose against the live roster
size lets through exactly the sentences it exists to stop. And **the harmless forms are all small
numbers.** None of the recorded stale counts is below five, and none of the legitimate ones is above
four.

Prose cannot be generated. The parameter reference works because it is emitted from the declarations
(ADR-0170), and no emitter can write *"the other ten systems take exactly the path they took before"*.
So the carrier has to be a gate.

## Decision

We will add `scripts/check-system-counts.mjs`, run at pre-push and in CI's `links` job, which rejects
a written-out count of systems:

- **The match.** A count token followed, within at most two words, by `system` or `systems`,
  case-insensitive. A count token is an English number word from `five` to `thirty`, or a numeral from
  `5` to `99`. `file system`, `operating system` and `build system` are not matches.
- **The threshold is five.** It is an argued constant, not a derived one. It sits between the largest
  legitimate count on 2026-09-14 (four) and the smallest stale one (seven). It is recorded here so
  that a change to it is a change to this decision.
- **The scope.** Every tracked `.md`, `.rs`, `.ts` and `.tsx`, including `.claude/skills/`, **except**
  the dated records: `docs/plans/**`, `docs/adrs/**`, `docs/design-backlog.md`,
  `docs/design-backlog-archive.md` and `node_modules`. A plan or ADR states what was true on its date,
  and ADRs are append-only. Fenced code in markdown is skipped. `.rs` is scanned whole, comments and
  string literals alike, because the recorded instances include a string.
- **The escape.** `count-allow: <reason>` on the matching line suppresses it, as an HTML comment
  (`<!-- count-allow: … -->`) in markdown. It is the same shape as ADR-0127's `hygiene-allow:`, and a
  reason is required.
- **Reporting.** `file:line  <matched text>`. An optional `root` argument, and a seeded tree under
  `scripts/fixtures/system-counts/` with an exact expected break count, following its siblings.

## Consequences

### Positive

- **The staleness is caught when it is written, not one close after a new system lands.** A count
  written today fails today, whether or not it is currently true.
- **It covers the instance class the reader-document sweep cannot**: `.rs` doc comments and assertion
  messages, which is where the count survived both closes.

### Negative

- **The threshold is a property of today's prose.** A real cohort of five or more, such as *"the six
  line systems"*, needs an allow marker, and every marker is a reason that is reviewed rather than
  checked.
- **It rejects only one noun.** *"Seven Node gates"*, *"112 presets"* and *"five zips"* go stale the
  same way and stay unguarded. Plan 0178 makes the gate inventory count-free by hand. Extending the
  grammar to other nouns is its own decision, and this ADR does not make it.
- **A count spelled another way escapes it.** *"a dozen systems"*, *"all of the thirteen built-in
  scene systems"* (three words between), and a count in a table cell with the noun in the header.
- **One more step on every push.** It is cheap: a text scan like its siblings.

## Alternatives considered

### Alternative A — Keep fixing counts by hand at each close

**Rejected because that is the status quo that produced this record.** Two closes each found the
class, each repaired the instances someone thought to grep for, and each left `common.rs` standing.

### Alternative B — Gate against the live roster size (reject only a count that is not 14)

**Rejected because a correct count is the next stale count.** It would let `"fourteen systems"`
through today and fail it on the day a system lands. That puts the cost back on the close that adds
the system, which is exactly the close that has kept missing it.

### Alternative C — A generated count token substituted into prose

A `{{systems}}` placeholder, expanded at site build time. **Rejected because the source is the
document.** GitHub, editors and the rustdoc a contributor reads would show the placeholder, and
`docs/` is the single source the site reads in place (ADR-0154). A sentence that needs the number
usually reads better without it.

### Alternative D — Match any number word beside any noun

**Rejected for its false positives.** "two passes", "three bands", "four quadrants" are properties of
the design and do not drift. A noun list has to be chosen noun by noun, which is the extension named
in Negative above.
