# Index-row fixture

The bite check for `scripts/check-index-rows.mjs`, on the `scripts/fixtures/`
convention `check-doc-links.mjs` established. Scanned only when the fixture tree
is the scan root:

```
node scripts/check-index-rows.mjs scripts/fixtures   # expects exit 0
```

It exits 0 while asserting three separate properties, so a regression in any one
of them is a red run rather than a quieter green one.

## Rows inside a region are measured and are under cap

<!-- roster:begin cap=320 -->

| ADR  | Title | Status |
|------|-------|--------|
| [0001](0001-a-slug.md) | A decision with a title of an ordinary length | accepted |
| [0002](0002-another-slug.md) | Another decision, this one carrying the forward-reference graph, which is the only thing in a roster with no second copy anywhere in the repository | accepted (extended by 0006, 0008, 0013 — the shipped surface is **v4**) |

<!-- roster:end -->

The second row is **258 bytes**: a long title plus the longest genuine
forward-reference status ADR-0116 measured. It sits just under that ADR's 269-byte
worst case, which is what the 320-byte cap was derived against — so if the cap is
ever quietly lowered toward the widths real rows actually reach, this fixture reds
instead of going quietly green on a tree that has nothing left to measure.

## A second region in the same file, with bullets rather than a table

Two regions in one file also prove the per-file region count is a count and not a
boolean.

<!-- roster:begin cap=320 -->

- [0001 — A closed plan](done/0001-a-slug.md) — closed 2026-01-01. Review: no blockers, no majors
- [0002 — Another closed plan](done/0002-another-slug.md) — closed 2026-01-02. Review: no blockers, no majors, two minors and a nit

<!-- roster:end -->

## A row outside every region is not measured

This is the documented way the gate is defeated (ADR-0116, Negative 3), and the
fixture pins it as behavior rather than leaving it as an accident. The row below
is 1,100-odd bytes and the checker must stay silent about it, because nothing
marked it as a roster:

| 0003 | A row that summarizes the document it points at instead of pointing at it, in the register the ADR index had grown into — naming the decision, then the rejected alternatives, then the costs, then the execution pointer, so that a reader who never opens the linked file still gets a third of it here, which is exactly the duplication ADR-0116 measured at 16 % of the corpus and exactly the shape that cannot be kept in sync with the body it copies, because nothing re-reads a summary when the thing it summarizes changes | [somewhere](0003-a-slug.md) |

## A fenced block inside a region is an example, not a roster

<!-- roster:begin cap=320 -->

```markdown
| 0004 | A fenced row long enough to be over the cap, written here because a document that DESCRIBES a roster is not making one, and this line is well past three hundred and twenty bytes so it would certainly be reported if fences were measured — which they are not, exactly as check-doc-links.mjs skips fenced blocks for the same reason. | [nowhere](0004.md) |
```

<!-- roster:end -->
