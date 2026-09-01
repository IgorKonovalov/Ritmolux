# Index-row red fixture

The half of `scripts/check-index-rows.mjs`'s bite check that **fails**. Its green sibling one
directory over asserts that the gate measures the right rows and stays silent about everything
else; nothing there, and nothing in the repository, holds a row the gate would reject — so the
reporting path below is the one thing no run of this gate had ever executed.

Scanned only when this directory is the scan root. It is named on the checker's own `SEEDED_TREES`
list, so the green fixture's root one level up walks past it and its counts stay 3 and 4:

```
node scripts/check-index-rows.mjs scripts/fixtures/index-rows-red   # expects exit 1, one break
```

`--self-test` runs the same tree in-process and additionally asserts the **shape** of what is
printed, because "exits non-zero" also describes a crash.

## One region, one row over cap

<!-- roster:begin cap=320 -->

| ADR  | Title | Status |
|------|-------|--------|
| [0001](0001-a-slug.md) | A row that stays a pointer | accepted |
| [0002](0002-another-slug.md) | A row that summarizes the document it points at instead of pointing at it, naming the decision, then the alternative it rejected, then what that cost, then where the execution went — so a reader who never opens the linked file still gets a third of it here, which is the duplication ADR-0116 measured at 16 % of the corpus | accepted (superseded in part by 0004, whose own row will grow the same way) |

<!-- roster:end -->

The second row is the break. The first is here so the report has to pick one of two rather than
convicting everything inside a marker, and so a `rows` count of 2 distinguishes "measured both,
rejected one" from "matched only the long one".
