# scripts/fixtures — the trees the doc checkers bite on

Seven checkers in `scripts/` take an optional `root` argument so they can be run against a tree
other than this repository. This directory is that tree. Most files under it are **deliberately
wrong in a named way**, so that "the checker still catches things" is a command anyone can run
rather than a property nobody has re-tested since the day it was written. `index-rows/` and `toc/`
are the exceptions and invert it — and `index-rows-red/` is the half that restores the usual
direction. See those sections below.

`check-doc-links.mjs`, `check-index-rows.mjs`, `check-filter-figures.mjs`,
`check-comment-hygiene.mjs` and `toc.mjs` skip this tree **by path** on an ordinary repo walk — `scripts/fixtures`,
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
| 0006 | four probes that still hold, two of them about whitespace | not reported |
| 0007 | a live entry with **no verification bullet at all** | reported at the heading's own line |

Two of 0006's four are the pair that pins how a span is read, and they pull in opposite directions
so neither can be satisfied by deleting the other:

| Probe | Asserts | Dies to |
|-------|---------|---------|
| `present: SEEDED_SPACE_RUN     is separated in: presets/README.md` | a run of spaces the author typed reaches the matcher intact | collapsing every whitespace run — which rewrites the pattern into one matching single-spaced text the tree does not hold, reports `no match`, and reads exactly like decay |
| a span that **wraps across two source lines** mid-pattern | the newline and its continuation indent still become one space | leaving whitespace alone entirely, which is why the collapse was written |

Restoring the collapse takes this fixture from five breaks to **six**, and the sixth is the
space-run probe reported as `no match` with the run silently gone from the message. That is the
whole of the defect: a probe that satisfies the letter of ADR-0108 and can never fire.

0004 and 0007 are the two halves of ADR-0108's Decision sentence and they fail differently: 0004
has a bullet with nothing runnable in it, 0007 has no bullet, which a check built out of the
bullets it finds cannot see. 0007 is last in the fixture on purpose — its absence runs to the end
of the file, the one position a heading-driven check could get wrong.

The same seven run inside `node scripts/check-backlog-claims.mjs --self-test` (13 of 13), together
with three assertions pinned to the **real repository** rather than to this tree, because that is
the only place they mean anything:

- the non-vacuity one, reconstructing entry 0082's own claim and requiring it to **fail**;
- that this script is tracked and `target` is not — the trackedness check, which cannot be seeded
  here because an untracked file is by definition absent from the fixture;
- that a **directory** answers for the files under it, since probe paths are as often `core/src`
  as a single file.

**Trackedness is a different question from existence**, and the gap runs one way: the pre-push
hook and the close ceremony read a full working tree with ignored files in it, and the CI `links`
job reads a checkout that by definition holds none. A probe into `renders/`, `target/` or `spike/`
therefore passed at both local call sites and could only ever fail on the runner, after the push.
Seeded by hand to confirm the repair — a probe naming a file under `renders/` reports *"probe path
is not tracked"* locally, where before it reported nothing at all.

## `doc-links/` — for `check-doc-links.mjs`

```
node scripts/check-doc-links.mjs scripts/fixtures
```

Expect **exit 1 and exactly five breaks**, across the four classes the checker knows about. Class 1
was the only one it had until Plan 0084, and checking one of markdown's two link forms was a green
light over 85 broken links of the other:

| File | Class | Seeded as |
|------|-------|-----------|
| `doc-links/broken.md` | 1 — inline | a target that does not exist |
| `doc-links/broken.md` | 2 — definition | a definition whose target does not exist |
| `doc-links/broken.md` | 3 — use with no definition in this file | a label `doc-links/defines.md` defines and this file does not |
| `doc-links/broken.md` | 4 — a backlog reference carrying a fragment | **twice**, once per link form: an inline `](design-backlog.md#…)` and a `[label]: design-backlog.md#…` definition |

Class 3 is scoped per file because that is markdown's own scope, and it is what a close ceremony
breaks when it moves link-dense prose between documents: the *uses* travel with the paragraph and
the *definitions* stay behind.

**Class 4 is a form rule, not a resolution failure** ([ADR-0149](../../docs/adrs/0149-a-backlog-reference-is-a-bare-number-and-a-file-link.md)),
and `doc-links/design-backlog.md` exists in this tree precisely so the seeded link **resolves** —
without it one seeded line would produce a class-1 break as well, and the fixture would be
asserting the wrong thing. That resolution is the whole reason the class needed a rule: the gate
reported 87 such references clean while every one of them landed at the top of a 280 KB document
instead of at the entry. It is seeded in both link forms deliberately, since covering one of the
two is this gate's own founding defect.

Three silences ride with it, and each is load-bearing:

| Case | Seeded in | Expected |
|------|-----------|----------|
| the same reference in the form ADR-0149 asks for | `broken.md` — `[backlog 0001](design-backlog.md)` | not reported — this is the fix, and a rule that convicted it would have nowhere to send its reader |
| a code span describing the retired form | `broken.md` | not reported — a document that *describes* link syntax is not making a link, which is why ADR-0149's own Context survives this gate |
| an `#anchor` into any other file | `broken.md` — `exists.md#no-such-heading` | not reported — one prohibited form, not a fragment checker; ADR-0149 records that it made a general one less likely |

Note the root: this checker is pointed at `scripts/fixtures`, not at `scripts/fixtures/doc-links`,
so the run also asserts that the backlog and index-row fixtures' own markdown is link-clean. Keep
it that way — a broken link seeded outside `doc-links/` would make the count above wrong for the
wrong reason. That constraint is why `index-rows/` ships five one-line stub documents: its roster
rows carry real relative links, and a row shaped like a real row has to point somewhere real.

## `comment-hygiene/` — for `check-comment-hygiene.mjs`

```
node scripts/check-comment-hygiene.mjs scripts/fixtures
```

Expect **exit 1 and exactly thirteen findings, across five files**. Note the root: like the two
above, this checker is pointed at `scripts/fixtures` rather than at its own subdirectory, so the run
also asserts that `backlog-claims/core/src/tier.rs` — the tree's other `.rs` file — is
hygiene-clean. Keep it that way; a seeded finding outside `comment-hygiene/` would make the counts
below wrong for the wrong reason.

**The file set comes from `git ls-files`, not from a filesystem walk**, which is what makes "code
we own" and "code this gate judges" the same set by construction. A walk cannot tell them apart: a
gitignored tree is absent from CI's fresh clone and present in every working tree, so the CI job is
green by construction and the local push is not. This gate went from green to **490 findings**
between two pushes twenty minutes apart with no commit touching it — 419 in `.venv/`'s torch, numpy
and markupsafe headers, 71 in the unpacked foobar2000 SDK, none of it written here. Patching those
two by name fixed those two; the next `pip install` would have re-broken it. These fixtures are
tracked, so `ls-files` reaches them and the counts below are unaffected.

`check-doc-links.mjs` was given the same enumeration in the same phase, and not on principle:
seeding one `.venv/pkg/README.md` with two relative links made it report both and exit 1. It was
green because neither vendored tree happened to carry a relative-linked `.md`, which is luck rather
than a property. `check-filter-figures.mjs` reaches the same end by a different route — it keeps the
whole working tree in scope on purpose and moves an untracked hit into an advisory. The remaining
two walk sets that are already correct.

**One rejected form per file, and the counts are the instrument.** A file that stopped biting shows
up as a number that moved rather than as a silence nobody noticed, which is the whole reason the
totals are written down here instead of being re-derived.

| File | Findings | Seeded as |
|------|---------:|-----------|
| `seeded.rs` | 2 | class 1, a relative link definition; class 2, the phrase `this plan` |
| `seeded-elapsed.rs` | 5 | one line per elapsed-time preposition in front of a numbered citation — `before` / `since` / `until` / `after` / `pre-` |
| `seeded-residue.rs` | 1 | the residue phrase, in a sentence explaining why something is absent |
| `seeded.cpp` | 2 | the same two classes as `seeded.rs`, in the dialect the foobar shim is written in |
| `seeded-literal.rs` | 3 | class 3, a string literal carrying a run of 12+ spaces mid-sentence — one already rejoined onto a single line, one at the width a continuation indent produces, and one **still unrejoined**, in the form an author actually types |

`seeded-elapsed.rs` seeds five rather than one because the pattern is an alternation and a
dropped branch is exactly the regression a single seeded line cannot see.

`seeded-literal.rs` seeds three convictions and **three silences**, which is the unusual ratio in
this tree and is the point of it: a lost `\` continuation and a hand-aligned column are the same
construct, so that file is where the width rule's cost is pinned rather than argued.

The three convictions are the defect in both of its forms plus the width. **Unrejoined** is the
form an author types — the `\` is missing, so the newline survives and the next line's indent
survives — and it was invisible until this fixture: `brokenLiteral` returned early for any literal
whose decoded text still held a newline, on the grounds that such a literal is a formatted block.
A lost continuation is prose carrying a newline in the middle of itself, so the gate caught the
defect only *after* someone joined the lines, while printing a message naming the shape it could
not see. **Rejoined** is what this tree has actually held: `core/src/dsp/mod.rs:57`,
`standalone/src/stream.rs:393` and `milkconv/src/convert.rs:430` all arrived single-line with the
run baked in.

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
| a formatted block | `seeded-literal.rs` — a column table carrying a `\n` | not reported — a table's rows start **at** a column and carry their runs between fields, so no line break in it is followed by a 12-space leading run. The rule is that shape, not "a literal holding a line break", which was the earlier wording and was **false**: a lost continuation is prose carrying a newline in the middle of itself, and it is now reported |
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

### `index-rows-red/` — the same checker's other half

```
node scripts/check-index-rows.mjs scripts/fixtures/index-rows-red
```

Expect **exit 1 and exactly three breaks** — one over cap, two of the wrong shape — on the model
Plans 0084 and 0094 established for the two sibling gates. A bare "exits non-zero" is also what a
crash looks like, so the counts are the assertion and the `--self-test` below additionally pins the
*shape* of each printed line.

**Length and shape are two different assertions** and this tree seeds both, because a row can
satisfy either while failing the other:

| File | Case | Expected |
|------|------|----------|
| `roster.md` | a 437-byte row inside a `roster:begin cap=320` region | reported as `roster.md:25  437 bytes (cap 320)` |
| `roster.md` | a correctly-sized row in the same region | measured, not reported — so the report picks one of two rather than convicting everything inside a marker |
| `roster.md` | a closed-plan **bullet** in a **table** region, well under cap | reported by line, naming the form expected and how many rows carry it |
| `roster.md` | the byte-identical bullet in a **bullet** region | **not** reported — the check is the row's form against its region's, and nothing else |
| `roster.md` | a region holding one row of each form | reported **once, at the region's own opening line**, naming both counts |

**This is the only thing anywhere that runs either reporting path.** Nothing tracked in this
repository is over cap or misshaped, and neither is the green fixture, so `file:line  N bytes
(cap C)` — two spaces before the count, which is what makes the line clickable — had never been
executed by any run of this gate.

### The bullet sits above the table header, and that is the whole test

A region's form is the **majority** of its rows, not its first row's, and this fixture is where the
difference is pinned. The instance backlog 0166 was filed for put the stray bullet immediately
under `roster:begin` and *above* the table header, because the insertion anchored on a string
rather than on a section. Under a first-row rule that stray row **defines** the region and every
real row below it becomes the finding: seeded into `docs/plans/README.md`, a first-row rule reports
**14 breaks and not one of them the mistake**, where the majority rule reports the one row —
`docs/plans/README.md:27  a bullet row in a table region (expected a table row; 14 of the 15 rows
in this region have that form)`.

A region split evenly has no majority to appeal to, and guessing which half is wrong would be the
same misdiagnosis in a quieter form — so it is reported at the region rather than at a row. The
fourth region seeds that.

**Note the root.** Unlike the green fixture this checker is pointed at the subdirectory rather than
at `scripts/fixtures`, and the tree names itself on the checker's `SEEDED_TREES` list for a second
reason beyond the usual one: it sits *inside* the green fixture's root, so without the skip the
green run would walk it and inherit its over-cap row, turning that run's exact counts into 2
regions and 6 rows and its exit code into 1. The two roots assert opposite things and have to stay
separable.

### The counts are asserted, not printed

**Exit 0 above is not on its own an assertion.** This tree holds no over-cap row inside a marker
and neither does the repository, so a detector that has stopped matching anything exits 0 from
both: replace `TABLE_ROW` and `BULLET` with regexes that match nothing and the fixture reports
`3 regions, 0 rows, 0 over cap`, which reads exactly like a clean tree. That is backlog 0104's own
reduction, and the per-file counts it collapses were the documented mitigation — *printed*, and
compared to nothing.

```
node scripts/check-index-rows.mjs --self-test    # expects exit 0, 10 of 10
```

The ten split three ways — the green fixture, the red one, and the repository — and they die to
different mutations:

| Half | Asserts | Why that shape |
|------|---------|----------------|
| green fixture | **exactly** 3 regions and 4 rows, nothing over cap, misshaped or malformed | this tree changes only when someone changes it on purpose, so an exact number is affordable — and **4 rather than 6** is what holds a table's header and its `\|---\|` delimiter to being structure rather than rows |
| red fixture | **exactly** 4 regions, 8 rows, 1 over cap and 2 misshaped, with each break matching its own printed form | the counts separate a conviction from a crash; the forms are the only assertion the two reporting paths have, since no other run reaches either |
| repository | a **floor** of 20 rows in each of `docs/adrs/README.md`, `docs/plans/README.md` and `docs/design-backlog.md`, and 100 across the tree | those three gain a row at every close, so an exact count would be red on the next one and would be raised without being read; a floor still goes to zero the moment the detectors stop matching, which is the only thing it is for |

Under the matches-nothing mutation the self-test reports **1 of 10** and exits 1, while the plain
run still exits 0 — which is the whole reason the self-test is where the assertion lives rather
than in the exit code of the ordinary run. Rewriting either report string alone costs one row.

Both invocations — the plain repo run and `--self-test` — are in `.githooks/pre-push` and in the
CI `links` job. A self-test nothing runs is the mechanism this fixture exists to have repaired.

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

### The fifth case cannot be seeded here, and that is why it is written down

**A hit in an untracked file is an advisory and never an exit code**, and every file in this tree
is tracked — a fixture for the untracked case would have to be a file the repository does not hold,
which is a contradiction. So it is recorded instead of seeded, verified by hand and reproducible in
two commands:

| Seeded | Expected | Verified |
|--------|----------|----------|
| a **gitignored** file naming the filter and carrying a cost figure — `renders/scratch-note.md` with `54 minutes` | named in the advisory block, **exit 0** | yes, on this tree |
| the same figure appended to a **tracked** file in scope — `docs/capturing.md` | reported as a violation, **exit 1** | yes, on this tree |

The walk deliberately stays a walk of the working tree. Narrowing it to `git ls-files` buys the
ergonomics and gives up the gate's whole reason for existing: the copy that broke this was the one
outside the list anyone was checking ([ADR-0122](../../docs/adrs/0122-a-sidecar-tool-documents-itself-in-one-place.md)).
Nothing wrong can reach `main` through a gitignored file — the CI `links` job checks out the
tracked tree — so the reach is kept and the exit code is left to the tracked half.

If git cannot answer at all, the gate says so and every hit counts toward the exit code. That is
ADR-0016's shape and it is the safe direction: a check that cannot measure trackedness must not
quietly downgrade everything it finds.

## `reader-prose/` — for `check-reader-prose.mjs`

```
node scripts/check-reader-prose.mjs scripts/fixtures/reader-prose
```

Expect **exit 1 and exactly six breaks, across three files**. Unlike every other checker here this
one does not walk a tree at all — it reads five fixed paths under the root, which is the scope
boundary ADR-0168 draws — so this subdirectory mirrors those paths rather than seeding an
arbitrary layout.

**One rejected form per line, and the counts are the instrument.** A branch of the citation
alternation that stops matching shows up as a number that moved rather than as a silence.

| File | Breaks | Seeded as |
|------|-------:|-----------|
| `presets/README.md` | 4 | the four citation forms: `ADR-0002` in a trailing parenthetical, `Plan 0063` woven into a sentence, `ADR‑0127` with a **U+2011** non-breaking hyphen, and `Plans 0027, 0078` — the **plural**, in a heading |
| `docs/presets.md` | 1 | a citation inside a mid-sentence parenthetical |
| `docs/preset-palettes.md` | 1 | a citation at a block's end |
| `docs/preset-guide.md` | 0 | clean, and must report `0 citation(s), 0 bare` rather than being skipped |
| `docs/preset-tuning-walkthrough.md` | 0 | clean, with no citation of any kind |

**The silences are the larger half of this fixture**, because every one of them is a shape the gate
must NOT convict. `presets/README.md` carries all four markdown link forms — inline, full
reference, collapsed reference, and a definition line — plus a fenced block whose commands name a
plan file and an ADR file by path, plus a `design-backlog 0062` reference that belongs to a
different corpus under ADR-0149.

**The collapsed reference is seeded inside a blockquote on purpose.** Its definition is written
`> [ADR-0098]: …`, which is exactly how the real roster writes it, and a definition-line pattern
anchored at the start of a line cannot see it. A definition the scanner misses makes every *use* of that label
look bare, so this one fixture line is what separates a working gate from one that reports two
false positives on the shipped tree — which is what it did before this file existed.

**`docs/capturing.md` is here and is deliberately out of scope.** It is an Entrance B document
carrying four bare citations, and the run must never report them. If it ever does, the filename
list inside the script has widened past what ADR-0168 decided.

## `toc/` — for `toc.mjs`

```
node scripts/toc.mjs --check scripts/fixtures
```

Expect **exit 0**, and like `index-rows/` that is only half the instrument. This tree seeds
*correct* blocks rather than broken ones, because the thing that can go wrong here is not a
detector that stops matching — it is an **anchor that is merely plausible**. Nothing downstream
catches one: `check-doc-links.mjs` validates paths and deliberately never validates fragments
([ADR-0149](../../docs/adrs/0149-a-backlog-reference-is-a-bare-number-and-a-file-link.md)), so a
wrong rule ships silently and every row in every block is wrong together.

So the committed block in `seeded.md` **is** the expected output, and the assertion is that the
generator reproduces it byte-for-byte. Green here means all six hold at once — 3 blocks, 14 rows:

| File | Case | Expected |
|------|------|----------|
| `seeded.md` | thirteen headings covering every shape this corpus contains | the committed block is regenerated identically |
| `seeded.md` | a `####` heading under a `depth=3` marker | not a row — `depth=N` means levels 2 through N |
| `target.md` | the resolve target for `seeded.md`'s linked heading | nothing else; it exists so the link fixture above still reports exactly five |
| `no-markers.md` | headings at three levels and no markers | not touched — a generator that inserted a block into any document with headings would rewrite most of this repository |
| `empty-block.md` | a marker pair with no headings after it | an empty block, **not** an error: markers get added before sections do |
| `fenced.md` | markers and a heading inside a ```` ```markdown ```` fence | one block, one row — a document *describing* this syntax is not carrying a block |

Plan 0151's own `## Data shapes` section is the real instance of `fenced.md`'s case, and a parser
without the fence rule would have rewritten the plan's worked example.

### `toc-red/` — the same checker's other half

```
node scripts/toc.mjs --check scripts/fixtures/toc-red
```

Expect **exit 1 and exactly two problems**, on the model `index-rows-red/` established. A bare
"exits non-zero" is also what a crash looks like, so the count is the assertion:

| File | Case | Expected |
|------|------|----------|
| `unpaired.md` | a stray `toc:end` before any begin | reported as `unpaired.md:5  toc:end with no toc:begin before it` |
| `unpaired.md` | a `toc:begin` with no end anywhere after it | reported as `unpaired.md:9  toc:begin with no toc:end after it` |
| `unpaired.md` | the file itself | left **byte-identical** — this is the silence that matters, because treating everything after an unclosed marker as block body would delete a document on a typo |

**Note the root**, and it is the same trap `index-rows-red/` documents: this tree sits *inside* the
green fixture's root, so it names itself on `toc.mjs`'s own `SEEDED_TREES` list. Without that entry
the green run above walks it, inherits both problems, and exits 1 — where exit 0 is the entire
point of that root. The two roots assert opposite things and have to stay separable.

### The anchors are pinned to the repository, not to this tree

```
node scripts/toc.mjs --self-test    # expects exit 0, 33 of 33
```

Six of the thirty-three are pinned to the **real** repository rather than to either tree, because
that is the only place they mean anything: the two heading texts `docs/capturing.md` and
`presets/README.md` still carry, the two anchors those two files still link, and the two slugs the
algorithm must produce from them. Between them they fix backtick stripping, colon removal, and the
**doubled hyphen** an em-dash leaves when it is deleted from between two spaces. If a future edit
rewords either heading, the self-test says so instead of the pinning quietly becoming a comparison
of two string literals.

The rest split between the shapes (`%`, `/`, `~~`, `*`, a linked heading, snake_case, and a link
label carrying a bracketed reference) and the structural refusals in the two tables above.
Verified by mutation — each of these takes the run red:

| Mutation | Reported as |
|----------|-------------|
| strip `_` as if it were an emphasis marker | `reactiondiffusion-glows`, and `seeded.md`'s block goes stale |
| make the heading detector match nothing | 8 of 33 fail, including both fixture blocks |
| drop the `-1` dedupe suffix | the second occurrence collides with the first |
| flatten the per-level indent | `level-3 rows indent one step` reports 0 of 8 |
| stop skipping fenced lines | `fenced.md` grows a row it must not have |
| flatten a link label with `[^\]]*`, which cannot cross a bracket | the target path folds into the slug: `...-kaleidoscope-seamdone0049-analysis-diagnostics-surfacemd` |

**`_` is the one worth stating twice.** GitHub keeps underscores in an anchor, and this corpus is
full of snake_case identifiers in headings — `reaction_diffusion`, `mirror_reflect`, `BASELINE_Y`.
A rule that stripped `_` alongside `*` and `~` would look right on every prose heading and be wrong
on every identifier one.
