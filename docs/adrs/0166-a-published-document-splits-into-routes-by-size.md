# ADR-0166 — A published document splits into routes by size, and a fragment map is the contract

> **Status:** accepted 2026-09-05 (Plan 0154), with an Outcome
> **Date:** 2026-09-05
> **Related plan(s):** [0154](../plans/done/0154-the-site-becomes-navigable.md)
> **Extends:** [0154](0154-the-reader-facing-docs-publish-as-a-site.md)

## Context

[ADR-0154](0154-the-reader-facing-docs-publish-as-a-site.md) publishes the reader-facing set one
route per document, and Plan 0143 built it that way. The site is live, and the largest problem it
was written to fix is still there — moved rather than solved.

Measured 2026-09-05 against the working tree at `9dc2183`:

| Source | Bytes | What the site does with it today |
|---|---:|---|
| `presets/README.md` | 273,211 | one route, **484 KB of HTML** |
| `docs/capturing.md` | 165,028 | one route |
| `docs/presets.md` | 75,803 | one route |
| `docs/preset-palettes.md` | 65,009 | one route |

ADR-0154's own argument was that *"a 264 KB parameter roster with no search is not a document anyone
reads; it is a document people give up on."* Search now exists, and it indexes **per page**: a query
for a parameter name returns one result titled "Parameter roster" and drops the reader at the top of
a quarter-megabyte document, to find the parameter with find-in-page. The right-hand table of
contents on that page carries 53 entries and is itself a scroll. The improvement over GitHub is
real, and much smaller than the decision that bought it assumed.

**A depth rule does not fix this, and the distribution is why.** Splitting the roster at every `##`
produces 13 routes, of which three hold 219,863 bytes — **80.5 %** of the document:

```
109,093  ## Systems and their named parameters
 63,013  ## Engine-wide controls
 47,757  ## Structural config (line systems and the attractor)
  ...    ten sections between 516 and 13,003 bytes
```

Splitting those three again at `###` leaves a worst case of 26,788 bytes. The same shape holds for
`docs/capturing.md`, whose `## The shot CLI` is 75,276 bytes across nine subsections.

**The second constraint is fragments, and nothing currently guards them.**
[`scripts/check-doc-links.mjs`](../../scripts/check-doc-links.mjs) validates that every relative
link *path* resolves and deliberately never validates the fragment — the same exposure
[ADR-0163](0163-a-long-document-carries-a-generated-contents-block.md) records for the generated
contents blocks. There are 15 fragment links into `presets/README.md` and 21 into
`docs/capturing.md`. Today every one of them lands somewhere on the correct enormous page, so being
wrong is invisible. After a split they would land on the wrong *page*, which is not invisible at
all — and the same split makes them checkable for the first time.

## Decision

A published document over **40 KB** splits into one route per `##` section; any resulting section
over **20 KB** splits again at `###`; the split stops there. The split happens at build time in
`site/`, from the parse the content loader already performs, and **no file under `docs/` or
`presets/` is edited to make it work** — ADR-0154's one-source rule stands unchanged.

The splitter emits a **fragment map**: every heading slug in a split document, keyed to the route
that now carries it. `site/src/plugins/rewrite-links.mjs` resolves `some/doc.md#slug` through that
map instead of appending the fragment to a single route. **A fragment that matches no heading fails
the build.** That is a gate the tree has never had, and it is what makes the prose rewrite in
[Plan 0155](../plans/done/0155-the-reader-documents-stop-explaining-themselves.md) safe to attempt.

**Provenance parentheticals are stripped from headings before slugs are computed.** A heading like
`## Engine-wide controls (Plan 0018)` would otherwise put a plan number in the URL itself. The strip
is the mechanical half of
[ADR-0168](0168-the-reader-documents-address-a-reader-and-the-record-stays-a-link.md); it is named
here because the ordering is a hard dependency rather than a preference — slugs are computed from
stripped headings, or the URLs are wrong permanently.

A split document contributes **one collapsed sidebar group**, not 46 flat entries, and its top route
is the document's own index: the prose before its first `##`, followed by a generated list of its
sections.

### Why these two numbers

They are chosen from the measured distribution, not from taste, and each selects exactly the set it
should:

- **40 KB selects the four documents above** and no others. The next largest published document is
  `docs/on-device-validation.md` at 37,241 bytes, which is a checklist read top to bottom.
- **20 KB selects seven sections** — the three roster sections listed above, `## The shot CLI`
  (75,276) and `## The core/tests/ harness` (52,784) in `capturing.md`, `## The expression language`
  (45,594) in `presets.md`, and `## Bindable colour parameters` (23,574) in `preset-palettes.md`.
- **Stopping at `###` leaves a worst-case route of 26,788 bytes** — `### What the report's columns
  mean` in `capturing.md`. That is a 10.2x reduction from 273,211, and it is a page a reader can
  hold.

The resulting route count, computed from the same tree: 46 for the roster (1 index + 13 sections +
32 subsections), 25 for `capturing.md`, 22 for `presets.md`, 19 for `preset-palettes.md` — **112
routes from four documents**, against 15 routes for the whole site today.

## Consequences

### Positive

- **Search becomes precise.** Pagefind indexes per route, so a hit resolves to `Attractor depth`
  rather than to a 484 KB page. This is the property ADR-0154 was bought for and did not deliver.
- **Deep links get validated for the first time.** 36 fragment links into the two largest documents
  currently resolve by accident. After this they resolve by construction, or the build fails.
- **The sidebar stays a menu.** One collapsed group per document, with the document's own index page
  doing the work a 46-entry flat list cannot.
- **The rule generalises without a list to maintain.** A hand-kept roster of "documents to split"
  would rot the way hand-kept rosters in this repository have. A document that grows past 40 KB
  splits on the next build; one that shrinks stops splitting.

### Negative

- **URLs change, and some of them are days old.** Every route under the four split documents moves.
  The site was deployed 2026-09-05, so the exposure is small — but it is real, and there is no
  redirect: a static host cannot rewrite a fragment, so an old `#anchor` deep link resolves to the
  document index rather than to the section. Accepted because the alternative is freezing the shape
  of the site permanently in its first week.
- **A heading rename moves a route.** Headings in this corpus are edited freely, and under this
  decision a heading is a URL. The fragment gate catches inbound breakage; it cannot catch an
  external link, a bookmark, or a URL someone pasted into a chat.
- **Two thresholds now exist that nothing re-derives.** They are correct for the 2026-09-05
  distribution. A document could sit at 39 KB with a 38 KB section and be served badly while
  satisfying both.
- **The split is invisible in the source.** This is ADR-0154's existing complaint about the link
  rewrite, and this decision doubles it: a reader of `presets/README.md` sees one document, and the
  site serves 46 pages. When the splitter is wrong, nothing in the tree shows it.
- **112 routes is a much larger Pagefind index.** Build time and index size both grow, and neither
  was measured before this decision. The plan owes the number.

### Neutral

- The generated contents blocks (ADR-0163) stay in the source and would render on the document index
  route, duplicating the generated section list there. The plan decides whether to suppress them.

## Outcome (2026-09-05, Plan 0154)

The decision stands and the mechanism is in the tree. One measurement in **Why these two numbers**
was wrong when this ADR was written, and it changed the output rather than only the prose.

**`docs/on-device-validation.md` is 48,219 bytes, not 37,241** — and 45,417 at `9dc2183`, the commit
the Notes say every figure was taken against, so the number was never right. Three consequences,
none of which is repaired by editing the body:

- **40 KB selects five documents, not four.** The bullet claiming it "selects the four documents
  above and no others" is false.
- **The document named as the reason 40 KB is the right threshold is on the wrong side of it.** It
  splits into 10 routes. It is still "a checklist read top to bottom", and it is now ten pages.
- **The real largest published document under the threshold is `docs/nfr.md` at 33,295 bytes.**

The route prediction moved with it: **133 routes from five documents**, against the 112 from four
predicted here — 46 / 25 / 22 / 19 / 10, and the first four are this ADR's own numbers exactly. The
worst-case route is **26,893 bytes** (`### What the report's columns mean`), against 26,788
predicted, and `ROUTE_SOURCE_CEILING` holds. Plan 0154 owed the cost figures and measured them: a
cold build goes 8,551 ms to 9,593 ms (+12 %), the Pagefind index 1,290 KB to 1,706 KB (+32 %).

**The split has no floor, and this ADR argues for one without having it.** Stopping at `###` is
justified above because "a third level shatters coherent small sections into pages with nothing on
them" — but that is a property of small sections, not of depth, and the size rule applies no
minimum at the levels where it does split. **22 of 122 split leaf routes hold under 1,200 bytes**;
the smallest is 196. `docs/on-device-validation.md` is the concentrated case — three of its ten
routes are under 800 bytes, while its own `## Checklist` stays one 22,598-byte route because it
carries no `###` to split at. Both halves of that sentence are the same gap seen from two ends. A
merge floor — a section under some size staying with its neighbour rather than becoming a route —
is the lever, and it needs its own arithmetic and its own ADR. Recorded here rather than fixed,
because raising 40 KB would only move which documents suffer it.

`ROUTE_SOURCE_CEILING` is asserted against **the splitter's output only**, not against every route.
Applied to every route the two constants contradict each other: `docs/nfr.md` is 33,295 bytes and
stays one route by decision, which is the threshold working, not failing.

## Alternatives considered

### Alternative A — an in-page sticky table of contents, no split

Cheapest, no URL churn, and the option weighed first. It loses on the measurement that started this:
Pagefind still indexes one 484 KB page, so search results stay imprecise no matter how good the
in-page navigation becomes. Navigation was only half the complaint.

### Alternative B — split at a fixed heading depth

Simpler to implement and to explain, and rejected by arithmetic rather than preference: a flat `##`
split leaves 80.5 % of the roster in three routes, and a flat `###` split shatters ten coherent
small sections — the smallest is 516 bytes — into pages with nothing on them.

### Alternative C — shorten the documents in `docs/`

The documents are long because they are complete, and ADR-0154 named completeness as the property to
keep. Shortening them to fit a renderer inverts the relationship between source and site.
[ADR-0168](0168-the-reader-documents-address-a-reader-and-the-record-stays-a-link.md) *does* rewrite
this prose — for a reader, not for a page size, and it does not shorten the reference material. The
split is what makes the result navigable either way.

### Alternative D — a hand-maintained map of documents to split

Explicit, greppable, and exactly the shape that has rotted repeatedly here. `PUBLISHED` already
requires two edits in two files to add a page, with nothing checking they agree, and that is one of
the defects Plan 0154 fixes. A size rule needs no maintenance and cannot disagree with itself.

## Notes

Measurements were taken 2026-09-05 against the working tree at `9dc2183`, summing bytes between
headings with `awk` over the source files. Byte counts are of markdown source, not rendered HTML;
the 484 KB figure for the roster's built page was measured from `site/dist/` during Plan 0143 and is
recorded in that plan's implementation log.
