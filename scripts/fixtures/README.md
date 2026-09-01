# scripts/fixtures — the trees the doc checkers bite on

Five checkers in `scripts/` take an optional `root` argument so they can be run against a tree
other than this repository. This directory is that tree. Most files under it are **deliberately
wrong in a named way**, so that "the checker still catches things" is a command anyone can run
rather than a property nobody has re-tested since the day it was written. `index-rows/` is the
exception and inverts it — see its section below.

`check-doc-links.mjs`, `check-index-rows.mjs`, `check-filter-figures.mjs` and
`check-comment-hygiene.mjs` skip this tree **by path** on an ordinary repo walk — `scripts/fixtures`,
enumerated once in the script — and scans it when it **is** the root, which is the only way the
seeded breaks below are reachable. Without that skip, this directory would red the link gate on
every push. The skip matched the directory *name* until Plan 0094 Phase 1, which meant it also
swallowed `core/tests/fixtures/` and its twelve relative links; a second seeded tree has to add
its own path to that list rather than inherit the skip from what it is called.

The seeded cases are kept **per checker, in per-checker subdirectories**, because a shared
fixture that drifts into serving one of them stops being an instrument for the other and says
nothing about it.

## `backlog-claims/` — for `check-backlog-claims.mjs`

```
node scripts/check-backlog-claims.mjs scripts/fixtures/backlog-claims
```

Expect **exit 1 and exactly five breaks**. Seven live entries, six seeded cases — plus `0099`
above the `Open entries` marker, whose probe is deliberately violated and must never be read:

| Entry | Case | Expected |
|-------|------|----------|
| 0001 | a violated `absent:` probe | reported, naming the contradicting `file:line` |
| 0002 | a violated `present:` probe | reported, naming the path searched |
| 0003 | a malformed probe (unclosed regex group) | reported as malformed, **never a crash and never a silent skip** |
| 0004 | a verification bullet with no probe and no opt-out | reported |
| 0005 | a valid `unprobeable:` opt-out | **not** reported; rostered in the summary |
| 0006 | two probes that still hold | not reported |
| 0007 | a live entry with **no verification bullet at all** | reported at the heading's own line |

0004 and 0007 are the two halves of ADR-0108's Decision sentence and they fail differently: 0004
has a bullet with nothing runnable in it, 0007 has no bullet, which a check built out of the
bullets it finds cannot see. 0007 is last in the fixture on purpose — its absence runs to the end
of the file, the one position a heading-driven check could get wrong.

The same seven run inside `node scripts/check-backlog-claims.mjs --self-test`, together with the
non-vacuity assertion that is pinned to the real repository rather than to this tree.

## `doc-links/` — for `check-doc-links.mjs`

```
node scripts/check-doc-links.mjs scripts/fixtures
```

Expect **exit 1 and exactly three breaks**, one per class the checker knows about. Class 1 was
the only one it had until Plan 0084, and checking one of markdown's two link forms was a green
light over 85 broken links of the other:

| File | Class | Seeded as |
|------|-------|-----------|
| `doc-links/broken.md` | 1 — inline | a target that does not exist |
| `doc-links/broken.md` | 2 — definition | a definition whose target does not exist |
| `doc-links/broken.md` | 3 — use with no definition in this file | a label `doc-links/defines.md` defines and this file does not |

Class 3 is scoped per file because that is markdown's own scope, and it is what a close ceremony
breaks when it moves link-dense prose between documents: the *uses* travel with the paragraph and
the *definitions* stay behind.

Note the root: this checker is pointed at `scripts/fixtures`, not at `scripts/fixtures/doc-links`,
so the run also asserts that the backlog and index-row fixtures' own markdown is link-clean. Keep
it that way — a broken link seeded outside `doc-links/` would make the count above wrong for the
wrong reason. That constraint is why `index-rows/` ships five one-line stub documents: its roster
rows carry real relative links, and a row shaped like a real row has to point somewhere real.

## `comment-hygiene/` — for `check-comment-hygiene.mjs`

```
node scripts/check-comment-hygiene.mjs scripts/fixtures
```

Expect **exit 1 and exactly twelve findings, across five files**. Note the root: like the two above,
this checker is pointed at `scripts/fixtures` rather than at its own subdirectory, so the run also
asserts that `backlog-claims/core/src/tier.rs` — the tree's other `.rs` file — is hygiene-clean.
Keep it that way; a seeded finding outside `comment-hygiene/` would make the counts below wrong for
the wrong reason.

**One rejected form per file, and the counts are the instrument.** A file that stopped biting shows
up as a number that moved rather than as a silence nobody noticed, which is the whole reason the
totals are written down here instead of being re-derived.

| File | Findings | Seeded as |
|------|---------:|-----------|
| `seeded.rs` | 2 | class 1, a relative link definition; class 2, the phrase `this plan` |
| `seeded-elapsed.rs` | 5 | one line per elapsed-time preposition in front of a numbered citation — `before` / `since` / `until` / `after` / `pre-` |
| `seeded-residue.rs` | 1 | the residue phrase, in a sentence explaining why something is absent |
| `seeded.cpp` | 2 | the same two classes as `seeded.rs`, in the dialect the foobar shim is written in |
| `seeded-literal.rs` | 2 | class 3, a string literal carrying a run of 12+ spaces mid-sentence — one already rejoined onto a single line, one at the width a continuation indent produces |

`seeded-elapsed.rs` seeds five rather than one because the pattern is an alternation and a
dropped branch is exactly the regression a single seeded line cannot see.

`seeded-literal.rs` seeds two convictions and **three silences**, which is the unusual ratio in
this tree and is the point of it: a lost `\` continuation and a hand-aligned column are the same
construct, so that file is where the width rule's cost is pinned rather than argued.

The silences are the load-bearing half, because a gate that cries wolf gets escaped rather than
obeyed ([ADR-0127](../../docs/adrs/0127-a-comment-carries-the-mechanism-and-the-decision-record-stays-in-docs.md),
Negative 3):

| Case | Seeded in | Expected |
|------|-----------|----------|
| a rustdoc intra-doc link | `seeded.rs` — `[Seeded::render]`, `[the renderer](crate::render::Renderer)` | not reported — `rustc` resolves these, so they cannot rot silently |
| a bare-number citation | `seeded.rs`, `seeded-elapsed.rs`, `seeded.cpp` | not reported — this is the form the links and the datings are replaced *with* |
| the same citation as a sentence's subject | `seeded.rs` — `the Plan 0045 Phase 4b defect` | not reported — `the plan` is exempt in front of a number, or the gate would convict its own fix |
| a preposition with no citation behind it | `seeded-elapsed.rs` — `after the warp and before the blur chain` | not reported — describing pass order has to stay legal, which is why the pattern requires plan / adr / phase and a number |
| the two words adjacent across a phrase boundary | `seeded-residue.rs` — `any input with more stops` | not reported — a pattern that dropped the space would fire here |
| an escaped false positive | `seeded.rs`, `seeded-elapsed.rs`, `seeded.cpp` — `hygiene-allow: <reason>` | not reported — the escape covers its own line and the next |
| a comment marker inside a string | all four — a URL, both raw-string spellings, an escaped `"` | not reported — the checker lexes source rather than grepping lines |
| a C block comment, which does not nest | `seeded.cpp` | not reported, and the rest of the file still is — Rust's nesting rules applied here would swallow the file and report nothing at all |
| a C char literal | `seeded.cpp` — `'"'` and `'''` | not reported — C has no lifetimes, so every `'` opens one, which Rust's rule gets wrong in the other direction |
| a correct `\` continuation | `seeded-literal.rs` — a literal wrapped across two lines with the escape present | not reported — the escape removes the newline **and** the next line's indent, so the reader gets one sentence; convicting this shape would convict most wrapped literals in the tree |
| a formatted block | `seeded-literal.rs` — a column table carrying a `\n` | not reported — a literal holding a line break is layout the author typed, and prose does not carry one mid-sentence |
| hand-typed column alignment | `seeded-literal.rs` — `note     : …` | not reported — nine spaces is under the width a continuation indent produces; this is the deliberate half of the ambiguity the width rule accepts missing |

The last three are why this checker is a lexer and not a `grep`, and why it takes the dialect as an
argument rather than guessing: `//` inside a URL literal and `"` inside a comment both defeat the
line-oriented form, and the three places C and Rust disagree — nesting, raw-string syntax, and what
a `'` opens — each have a silence pinned above so a shared lexer cannot quietly serve one dialect
by mis-reading the other.

An escape with **no reason after the colon** is itself a finding. That is not seeded — it would
make the counts above wrong — but it is asserted by the checker's own header and is the reason an
escape cannot silently become a silencer.

## `index-rows/` — for `check-index-rows.mjs`

```
node scripts/check-index-rows.mjs scripts/fixtures
```

Expect **exit 0** — this is the fixture that passes rather than fails, and it is only half the
instrument. A byte cap is trivially red on any tree with a fat row in it, so the interesting
assertion here is the reverse: that the checker measures the rows it should and stays silent about
everything else. Green here means all four hold at once:

| Case | Seeded as | Expected |
|------|-----------|----------|
| a row inside a region, at width | a 258-byte row, just under ADR-0116's 269-byte worst case | measured, not reported |
| a second region in the same file | a bullet region below a table region | **2** of the 3 regions counted, proving the count is a count |
| a row outside every region | a ~1,100-byte unmarked ledger row | not measured at all |
| a fenced row inside a region | an over-cap row inside a ```` ```markdown ```` block | not measured — a document describing a roster is not one |

The third case is the gate's own documented hole ([ADR-0116](../../docs/adrs/0116-an-index-row-is-a-pointer-and-a-gate-holds-it-to-one.md),
Negative 3): a row moved past a marker escapes silently. It is pinned here as **behavior** rather
than left as an accident, so that a future attempt to close it fails this fixture loudly instead
of changing what the markers mean without anyone noticing.

The per-file region count the checker prints on success is the mitigation for that hole, and it
is what the second case exercises — a deleted marker shows up as a region that vanished rather
than as a file that quietly stopped being checked.

### The counts are asserted, not printed

**Exit 0 above is not on its own an assertion.** This tree holds no over-cap row inside a marker
and neither does the repository, so a detector that has stopped matching anything exits 0 from
both: replace `TABLE_ROW` and `BULLET` with regexes that match nothing and the fixture reports
`3 regions, 0 rows, 0 over cap`, which reads exactly like a clean tree. That is backlog 0104's own
reduction, and the per-file counts it collapses were the documented mitigation — *printed*, and
compared to nothing.

```
node scripts/check-index-rows.mjs --self-test    # expects exit 0, 6 of 6
```

The six split into a fixture half and a repository half, and they die to different mutations:

| Half | Asserts | Why that shape |
|------|---------|----------------|
| fixture | **exactly** 3 regions and 4 rows, nothing over cap, no malformed marker | this tree changes only when someone changes it on purpose, so an exact number is affordable — and **4 rather than 6** is what holds a table's header and its `\|---\|` delimiter to being structure rather than rows |
| repository | a **floor** of 20 rows in each of `docs/adrs/README.md`, `docs/plans/README.md` and `docs/design-backlog.md`, and 100 across the tree | those three gain a row at every close, so an exact count would be red on the next one and would be raised without being read; a floor still goes to zero the moment the detectors stop matching, which is the only thing it is for |

Under the mutation above the self-test reports **1 of 6** and exits 1, while the plain run still
exits 0 — which is the whole reason the self-test is where the assertion lives rather than in the
exit code of the ordinary run.

## `filter-figures/` — for `check-filter-figures.mjs`

```
node scripts/check-filter-figures.mjs scripts/fixtures/filter-figures
```

Expect **exit 1 and exactly five breaks**. Note the root: unlike the three above this checker is
pointed at its own subdirectory rather than at `scripts/fixtures`, because it needs a
`docs/diffusion-filter.md` at the root it is given — the canonical page is the reference set every
other file is measured against, and there can only be one of it per tree.

| File | Case | Expected |
|------|------|----------|
| `README.md` | an orientation figure quoting a number in no canonical region | reported, naming the number |
| `orientation-elsewhere.md` | the orientation marker used outside `README.md` | reported as a misplaced whitelist |
| both of the above | **two** orientation lines carrying figures | reported separately — ADR-0122 allows one |
| `stray.md` | a cost figure in a section that names the filter | reported |
| `tools/sd-filter/README.md` | a cost figure in a file whose **path** names the filter, under no naming heading | reported |

The silences matter more than the breaks here, because over-broad scanning is how this gate would
become useless: `docs/capturing.md` is two thousand lines about tooling that *does* ship and is
full of unrelated timings. Four cases assert it stays quiet:

| Case | Seeded as | Expected |
|------|-----------|----------|
| a figure in a section that does not name the filter | `~150 ms` and `30 s` under two unrelated headings in `stray.md` | not reported |
| a figure inside fenced code, in a section that does | a `2.966 s` comment in a `bash` block | not reported — a command line is not a claim |
| a figure in a dated record | `docs/plans/0001-a-record.md`, full of them | not scanned at all |
| a figure spelled in words | *Thirteen minutes* in `tools/sd-filter/README.md` | not reported — hole 1, named in the checker's header |

The third of those is ADR-0122's own accepted scope limit, and the second and fourth are the
checker's documented holes. All four are pinned here as **behavior** rather than left as accidents,
so that a future attempt to widen the scan fails this fixture loudly instead of quietly convicting
prose it was never meant to reach.
