# Index-row red fixture

The half of `scripts/check-index-rows.mjs`'s bite check that **fails**. Its green sibling one
directory over asserts that the gate measures the right rows and stays silent about everything
else; nothing there, and nothing in the repository, holds a row the gate would reject — so the
reporting paths below are the only thing that executes them.

Scanned only when this directory is the scan root. It is named on the checker's own `SEEDED_TREES`
list, so the green fixture's root one level up walks past it and its counts stay 3 and 4:

```
node scripts/check-index-rows.mjs scripts/fixtures/index-rows-red   # expects exit 1, two breaks
```

`--self-test` runs the same tree in-process and additionally asserts the **shape** of what is
printed, because "exits non-zero" also describes a crash.

## A row over cap

<!-- roster:begin cap=320 -->

| ADR  | Title | Status |
|------|-------|--------|
| [0001](0001-a-slug.md) | A row that stays a pointer | accepted |
| [0002](0002-another-slug.md) | A row that summarizes the document it points at instead of pointing at it, naming the decision, then the alternative it rejected, then what that cost, then where the execution went — so a reader who never opens the linked file still gets a third of it here, which is the duplication ADR-0116 measured at 16 % of the corpus | accepted (superseded in part by 0004, whose own row will grow the same way) |

<!-- roster:end -->

The second row is the break. The first is here so the report has to pick one of two rather than
convicting everything inside a marker, and so a `rows` count of 2 distinguishes "measured both,
rejected one" from "matched only the long one".

## A row of the wrong shape for its region

The region below is a **table** region — two of its three rows have that form — and the bullet in
it is the mistake backlog 0166 was filed for, made at a real close in `docs/plans/README.md` and
waved through by both this gate and the link gate. It is **well under the cap**, which is the whole
point: shape and length are different assertions and a length check alone reports `0 over cap`.

**The bullet sits above the header on purpose.** That is where the real one landed — the insertion
anchored on a string rather than on a section — and it is the position that tells a majority rule
apart from a first-row one. Under a first-row rule this row defines the region and the two real
ones below become the finding.

<!-- roster:begin cap=320 -->

- [0002 — A closed plan](0002-another-slug.md) — closed 2026-01-02. Review: no blockers, no majors

| Plan | Title | Status |
|------|-------|--------|
| [0001](0001-a-slug.md) | An active plan, in the form this region holds | approved |
| [0002](0002-another-slug.md) | A second active plan, so the majority is a majority | approved |

<!-- roster:end -->

## The same row, in the region whose form it has

Byte-for-byte the same bullet as the one above, and silent here, because every row in this region
has that form. A checker that reported on length, on the word "closed", or on anything but the
row's form against its region's would convict this one too.

<!-- roster:begin cap=320 -->

- [0002 — A closed plan](0002-another-slug.md) — closed 2026-01-02. Review: no blockers, no majors

<!-- roster:end -->

## A region with no majority at all

One of each. There is nothing to appeal to, and picking one would be the same misdiagnosis a
first-row rule makes, quieter. The region is reported **once, at its own opening line**, naming
both counts rather than convicting a row.

<!-- roster:begin cap=320 -->

| [0001](0001-a-slug.md) | A row in one form | approved |
- [0002 — A row in the other](0002-another-slug.md) — closed 2026-01-02

<!-- roster:end -->
