# ADR-0163 — A long document carries a generated contents block, and spent prose archives at the close

> **Status:** accepted 2026-09-04 (Plan 0151)
> **Date:** 2026-09-02
> **Extends:** [ADR-0116](0116-an-index-row-is-a-pointer-and-a-gate-holds-it-to-one.md)
> **Related plan(s):** [0151](../plans/done/0151-the-long-documents-become-navigable.md)

## Context

The working record has outgrown the one instrument it has for finding anything inside it, which is
find-in-page. Measured 2026-09-02, the five largest markdown documents outside `docs/images/`:

| File | Lines | Bytes | Shape | Growth, 2026-08-29 → 09-02 |
|---|---:|---:|---|---:|
| `docs/plans/README-archive.md` | 6,682 | 591 KB | 133 close write-ups as **one flat bullet list** — 4 headings in the whole file | +1,063 (+19 %) |
| `docs/design-backlog-archive.md` | 8,447 | 579 KB | 109 archived entries, `##` each | +1,602 (+23 %) |
| `docs/design-backlog.md` | 3,528 | 262 KB | 44 live entries, **90 % of the file** | −343 |
| `docs/capturing.md` | 2,415 | 155 KB | 36 sections, no contents block | +263 (+12 %) |
| `docs/plans/README.md` | 1,267 | 118 KB | one **685-line** execution-sequence section | +198 (+19 %) |

`presets/README.md` sits beside them at 4,192 lines and 264 KB across 53 headings, of which
`## Systems and their named parameters` alone is 1,660 lines. None of these documents has a table of
contents.

**Three findings shape the decision, and the first is that there is almost nothing dead to delete.**
The archival lifecycle this project wrote for itself is working: `design-backlog.md` *fell* 4,268 →
3,528 lines over the same window in which both archives grew, all 44 of its live entries carry
current dated probes, and no closed entry is sitting in the open section. The backlog is long
because 44 real design gaps are open. Deletion is not the available lever.

**Second, the one archive step that has no carrier is sequencing prose.** `docs/plans/README.md`'s
`## Recommended execution sequence` runs 685 lines and names 97 plan numbers, **87 of them closed**.
Its `### What this sequence assumes` subsection is 345 lines and declares itself spent twice in its
own text — *"Superseded 2026-08-18, kept as the record"* and *"Prior sequence notes follow, and they
are the record of how the previous roster ordered itself"*. `README-archive.md` already carries a
`## Prior sequencing notes (superseded)` section built for exactly this, holding **three** items
against those 335 live lines. The mechanism exists and has never been reached for, which is the same
shape as the three-sweep accumulation the backlog documents at its own head: a rule whose carrier
lives in a different file from the ceremony that would execute it.

**Third, one of the long sections is a second copy of a normative reference.** `design-backlog.md`
lines 12–66 restate the probe grammar in 55 lines; `scripts/check-backlog-claims.mjs` lines 42–68
already carry the same three forms, the path rule, the regex-escaping caveat and the rationale — and
the script is the authority, because it is what parses them. This project has twice recorded that
class rotting: `CLAUDE.md` retired its paraphrase of the C ABI roster after it drifted twice, and the
`preset-author` lane's private catalogues rotted while `presets/README.md` stayed current.

Two constraints bound any answer. [ADR-0149](0149-a-backlog-reference-is-a-bare-number-and-a-file-link.md)
prohibits `design-backlog.md#…` fragments in references between documents, and priced the cost in its
own Negative section: *"A reader loses one click. Landing at the top of a 280 KB backlog and searching
for `0072` is worse than landing on the heading, and this ADR makes that the permanent experience."*
And [ADR-0154](0154-the-reader-facing-docs-publish-as-a-site.md) / Plan 0143 publish the
reader-facing subset as a site while forbidding any edit to a source file made to serve the site — so
the site resolves the *rendering* problem for `capturing.md` and `presets/README.md`, does not
shorten either one, and will build its route map and its 926-link rewrite against whatever layout it
finds.

## Decision

**No document is split.** Every markdown document over roughly 400 lines carries a generated contents
block between `<!-- toc:begin depth=N -->` and `<!-- toc:end -->` markers, produced by
`scripts/toc.mjs` from the headings that follow it. The block is regenerated, never hand-edited, and
`--check` reports drift.

`depth` is per-document because the corpus is not uniform: the backlog files repeat `### Priority`,
`### The finding` and `### What a fix would be` under every entry, so they take `depth=2` and get one
row per entry; the reader-facing manuals take `depth=3` and get one row per section.
`plans/README-archive.md` is the exception that has to be prepared first — its 133 close write-ups
are top-level bullets with no headings at all, so nothing in it is addressable, and those bullets
become `###` headings before it can carry a block.

**Spent prose archives at the close, into the archive files that already exist.** The superseded
sequencing notes move to `README-archive.md`'s `## Prior sequencing notes (superseded)`, and the close
ceremony gains the step that makes it happen — the same repair ADR-0108 and close-ceremony step 3c
made for backlog entries.

**A second copy of a normative reference is deleted rather than moved.** The backlog's probe grammar
is replaced by a pointer at `scripts/check-backlog-claims.mjs`, which is the form `CLAUDE.md` already
uses for the C ABI roster.

The anchor algorithm is GitHub's, and it is pinned to evidence rather than to a specification:
`docs/capturing.md` already links `#--render-a-music-video-from-a-track` and
`#seeded-randomness--hash-noise-and-generator-seed`, which between them fix the treatment of
backticks, a colon, and the doubled hyphen an em-dash leaves behind. The generator reproduces both or
it is wrong.

## Consequences

### Positive
- Every long document gains an entry point that is not find-in-page, and `plans/README-archive.md`'s
  133 close write-ups become addressable for the first time.
- `docs/plans/README.md` loses 335 lines — 26 % — to a section that already exists to receive them,
  and the live index goes back to describing the live roster.
- One duplicated normative reference stops existing, so the grammar has a single authority and it is
  the parser.
- The block is generated, so it cannot drift out of agreement with the headings the way a
  hand-written contents list would.

### Negative
- **Shortness is not delivered where it was asked for.** `design-backlog.md` nets about −110 lines of
  3,528, which is 3 %. Its 44 live entries are the other 90 % and every one of them is current. This
  ADR makes navigability the deliverable for that file and leaves its length to the work closing.
- **ADR-0149's lost click stays lost.** A contents row is a fragment into the same document, so it
  helps a reader already inside the file and does nothing for a reference arriving from another one.
  Both archives are past 512 KB, where GitHub's rendering may not reach the row's target at all.
- **No cap, and no size gate.** A contents block relieves the pressure that would otherwise force a
  split, so these files keep growing and the next reader inherits a longer document with a longer
  contents block. Nothing measures that.
- **Fragments are unchecked.** `scripts/check-doc-links.mjs` validates paths and deliberately never
  validates fragments, so a generated block's correctness rests entirely on the anchor algorithm
  matching GitHub's. Two in-repo anchors pin it; a heading shape nobody has written yet would not be
  caught.
- **The bullet-to-heading conversion rewrites 6,682 lines of an append-only record.** The content is
  unchanged and the diff is mechanical, but someone reading `git log` on that file sees a commit
  touching every close write-up ever written.

### Neutral
- Numbering, the ledger, the `roster:begin` regions and ADR-0116's 320-byte cap are all untouched.
  This ADR adds an instrument beside them; it does not change what a row is.
- The two archives stay append-only and write-only. A contents block does not make them documents
  anyone reads front to back.

## Alternatives considered

### Alternative A — One file per entry
`docs/backlog/0128.md`, `docs/plans/closed/0113.md`, and so on: roughly 240 small files. This is what
makes an entry addressable, turns `[backlog 0072](backlog/archive/0072.md)` into a file link that
satisfies ADR-0149's form *and* lands on the entry, makes `grep -l` name the entry, and turns each
close into a new file instead of a 500-line diff on a shared one. It was the architect's
recommendation.

Rejected on churn against value. It rewrites `check-backlog-claims.mjs`'s single-file walk and
`check-index-rows.mjs`'s `ROSTERS` list, rewrites close-ceremony step 3c, and repoints the inbound
references in 32 files — and the two documents it fixes hardest are write-only records nobody reads
linearly, so most of that cost buys a navigation gain where navigation is least needed. The refund it
would collect is the one ADR-0149 already wrote off as permanent, which makes it a nice-to-have
rather than a debt coming due.

### Alternative B — Number-range shards
`design-backlog-archive-0001-0049.md` and siblings: about six files, minimal gate churn, no new
script. Rejected because the seams are arbitrary — a reader who does not already know an entry's
number cannot pick a shard — a reference still cannot name the entry, and each shard regrows to
today's size on the same trajectory, so the decision is re-taken in a few months with more files.

### Alternative C — Leave it alone
Rejected on the growth figures. The two archives added 2,665 lines in five days, `plans/README.md`
grew 19 % in the same window, and the sequencing prose it accumulated was already declaring itself
superseded in its own text.

### Alternative D — Let the documentation site solve it
ADR-0154 gives the reader-facing set search, a sidebar and per-heading routes, which is a better
contents block than this one. Rejected as insufficient rather than wrong: the site publishes only the
reader-facing subset and explicitly not the plans, the ADRs or the backlog, so it reaches two of the
six documents measured here and none of the three worst. This ADR sequences before it so that 0143
builds its route map against the final layout.
