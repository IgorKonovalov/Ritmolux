# scripts/fixtures — the trees the doc checkers bite on

Four checkers in `scripts/` take an optional `root` argument so they can be run against a tree
other than this repository. This directory is that tree. Most files under it are **deliberately
wrong in a named way**, so that "the checker still catches things" is a command anyone can run
rather than a property nobody has re-tested since the day it was written. `index-rows/` is the
exception and inverts it — see its section below.

`check-doc-links.mjs`, `check-index-rows.mjs` and `check-filter-figures.mjs` skip this tree **by path** on an ordinary repo walk — `scripts/fixtures`,
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

## `index-rows/` — for `check-index-rows.mjs`

```
node scripts/check-index-rows.mjs scripts/fixtures
```

Expect **exit 0** — the one fixture here that passes rather than fails. A byte cap is trivially
red on any tree with a fat row in it, so the interesting assertion is the reverse: that the
checker measures the rows it should and stays silent about everything else. Green here means all
four hold at once:

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
