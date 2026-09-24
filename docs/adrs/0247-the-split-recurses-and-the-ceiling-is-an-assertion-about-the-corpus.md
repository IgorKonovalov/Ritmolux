# ADR-0247 — The split recurses, and the ceiling is an assertion about the corpus

> **Status:** accepted 2026-09-24 (Plan 0225), with an Outcome
> **Date:** 2026-09-23
> **Related plan(s):** [0225](../plans/done/0225-the-split-goes-one-level-deeper.md)
> **Extends:** [0166](0166-a-published-document-splits-into-routes-by-size.md)

## Context

[ADR-0166](0166-a-published-document-splits-into-routes-by-size.md) splits a published document over
40 KB at `##`, splits any resulting section over 20 KB at `###`, and stops. It asserted a worst case
from that stop:

> Stopping at `###` leaves a worst-case route of 26,788 bytes — `### What the report's columns mean`
> in `capturing.md`. That is a 10.2x reduction from 273,211, and it is a page a reader can hold.

`ROUTE_SOURCE_CEILING = 30_000` in `site/src/plugins/split-document.mjs` is the assertion that the
arithmetic still holds, and `scripts/check-site-routes.mjs` enforces it against a built site. Both
say in their own comments that a route over it means ADR-0166 needs redoing rather than the constant
raising. This ADR is that redoing.

**The `Pages` workflow has been red on every push to `main` since 2026-09-23**, so the site has not
deployed:

```
A route's source exceeds 30000 bytes (1):
  32711 B  engine/capturing/the-shot-cli/what-the-reports-columns-mean
```

Measured 2026-09-23 against the working tree at `dc9e0bc6`, by running the site's own splitter over
the published set — 173 routes:

| | bytes |
|---|---:|
| `engine/capturing/the-shot-cli/what-the-reports-columns-mean` | **32,711** |
| `engine/on-device-validation/checklist` | 29,528 |
| `contribute/testing/the-coretests-harness/the-preset-sweeps-fan-out-in-batches` | 25,884 |
| p99 / p95 / p90 / p50 | 25,884 / 17,131 / 10,354 / 2,904 |

**The two largest routes are over for different reasons, and only one of them is about size.**

- The failing route is a `###` **with nine `####` children**. Its content is structured exactly as
  the splitter would need; the algorithm declines to cut it because the stop at `###` is
  unconditional. Cutting it at `####` yields an index of 2,428 bytes and nine routes whose largest
  is 6,487.
- The runner-up, `## Checklist` in `docs/on-device-validation.md`, carries **no `###` at all** across
  its 327 lines. No depth rule reaches it. It sits 472 bytes under the ceiling.

So the stop at `###` is what breaks today, and the ceiling is not a property the algorithm can
guarantee at any depth — it depends on the corpus having headings where the bytes are.

## Decision

**The split recurses.** A section over `SECTION_SPLIT_BYTES` splits at the next heading level,
repeatedly, until it is under the threshold or has no deeper heading. The two thresholds are
unchanged, the entry condition is unchanged, and the hard stop at `###` is removed.

**And the ceiling is restated as what it is: an assertion about the corpus, not about the
algorithm.** A route may exceed it with no deeper heading to cut at, and when it does, the repair is
editorial — headings in the source document — not a change to the splitter and not a raised
constant. `docs/on-device-validation.md`'s `## Checklist` is the standing instance.

`site/src/plugins/split-document.mjs` argues the stop this way:

> the split stops at `###` because a third level shatters coherent small sections into pages with
> nothing on them

That is true of an **unconditional** third level and false of a conditional one. The second level is
already conditional — `###` fires only inside a `##` over 20 KB — and extending the same condition
one level further costs nothing wherever nothing is oversized. Of the 173 routes measured above,
exactly one qualifies.

## Consequences

### Positive

- **The site deploys again**, and the route that broke it becomes ten pages a reader can hold.
- **The rule stops having a special case.** "Split what is too big, at the next level down" is one
  sentence with no terminal depth in it, and it needs no revisiting the next time a section grows.
- **It generalises to the corpus rather than to today's offender.** Any future `###` over the
  section threshold splits on the next build, exactly as a `##` does now.

### Negative

- **URLs move again, for the second time in eighteen days.** Every route under
  `what-the-reports-columns-mean` is new, and the nine `####` slugs did not previously exist.
  ADR-0166 accepted this class of breakage for a site deployed days earlier; the site is older now,
  and this is the cost being accepted knowingly a second time.
- **Four of the nine new routes are under 1.5 KB.** They are thin pages, and thin pages are what the
  stop at `###` existed to prevent. The trade is deliberate: a 32,711-byte page is worse than a
  1,147-byte one, and the alternative was neither.
- **The ceiling can still fail with no splitter repair available.** That is now written down rather
  than discovered, but it is not fixed: `## Checklist` has 472 bytes of headroom and no internal
  headings, so the next edit to it turns `Pages` red with nothing in this decision to apply.
- **The fragment map and the Pagefind index both grow**, by nine entries and nine routes. Neither
  was measured; both are small against 173.

## Alternatives considered

- **Raise `ROUTE_SOURCE_CEILING`.** Rejected, and both the constant's own doc comment and the gate's
  failure message reject it in advance. The ceiling is the assertion that the thresholds did their
  job; moving it deletes the only thing that reports when they stop.
- **Promote the heading in `docs/capturing.md`** — `###` to `##`, its `####` children to `###`.
  Rejected twice over: [ADR-0154](0154-the-reader-facing-docs-publish-as-a-site.md)'s one-source
  rule says no file under `docs/` is edited to make the site work, and it produces the same nine
  routes by a route that also rewrites the document's outline for every GitHub reader.
- **Shrink the section.** Rejected: [Plan 0207](../plans/done/0207-the-commitments-get-their-instruments.md)
  Phase 2 put measured content there deliberately, and deleting evidence to satisfy a layout
  constraint inverts what the document is for.
- **Split by byte budget instead of by heading level.** Rejected: it cuts mid-argument, and a chunk
  with no heading has no stable slug, which breaks the fragment-map contract ADR-0166 bought.
- **Add `###` headings to `## Checklist` in this decision.** Rejected as scope: it is the right
  repair for that document and it is an editorial judgement about a checklist someone runs by hand,
  not a consequence of the splitter. Recorded above so it is a known debt rather than a surprise.

## Outcome (2026-09-24, Plan 0225)

Two statements above are falsified by the implementation; the body stands as written.

- **More than one section qualified.** *"Exactly one qualifies"*, *"nine entries and nine routes"*
  and *"ten pages"* counted only the offender in `docs/capturing.md`. The recursion also split two
  `###` sections of `presets/README.md` that were over 20 KB with `####` children — the
  `shape_field` roster (5 children) and the `tuple` roster (4) — so the published set went from 173
  routes to 191, not 182. The largest route after the change is 29,528 bytes, `## Checklist` in
  `docs/on-device-validation.md`, exactly as the Negative section predicted.
- **The recursion looks one level down, not at any deeper heading.** The Decision's *"until it is
  under the threshold or has no deeper heading"* is implemented as *no heading at the next level*:
  a section over the threshold whose headings skip a level stays whole. No section in the corpus
  does that today; `sectionsAt` in `site/src/plugins/split-document.mjs` documents the rule.
