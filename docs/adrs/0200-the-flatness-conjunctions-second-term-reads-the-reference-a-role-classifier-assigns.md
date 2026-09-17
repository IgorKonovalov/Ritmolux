# ADR-0200 — The flatness conjunction's second term reads the reference a role classifier assigns

> **Status:** proposed
> **Date:** 2026-09-17
> **Related plan(s):** [0186](../plans/0186-the-flatness-gate-tells-a-figure-from-its-ground.md)
> **Supersedes in part:** [0161](0161-the-blot-anchor-becomes-a-defect-record-because-term-two-reads-the-fringe.md)
> (its defect record becomes a conviction again)
> **Extends:** [0130](0130-the-structural-term-is-boundary-density-and-conditioning-the-population-is-what-made-it-work.md),
> [0129](0129-the-structural-term-is-measured-at-composition-scale-not-pixel-scale.md),
> [0128](0128-a-tonally-flat-picture-is-a-blot-only-if-it-is-also-structureless.md)

## Context

The sanity gate calls a frame a blot when it is **tonally flat** and **structureless**. Since
[ADR-0161](0161-the-blot-anchor-becomes-a-defect-record-because-term-two-reads-the-fringe.md) the
second term convicts nothing, including the frozen `Blown Out` fixture it was built around, and the
reason is a reference rather than a statistic. `boundary_density` is read against the frame's
**derived ground**, and a saturated blot *is* its own modal band, so the term measures the blot's
fringe: the smoother the rim, the more structured it scores. `Blown Out` reads `0.5697` there
against `0.0382` areally, and `Tiled Rosette Mono` — the composition the term exists to admit —
reads `0.3602`, **below** the blot. No value of a floor separates them.

Three ADRs in this line (0126, 0128, 0129) each named a mechanism before measuring it and each was
falsified or superseded by the phase that followed. So Plan 0186 registered a four-part stop
condition *before* any number was read — separation at both anchors, nothing convictable in the gap,
a separation wider than the defect class's own spread, and the same ordering at two capture sizes —
tabled five new discriminators beside the five already in the harness, added a second blot anchor
with a **ragged** rim (`Ragged Blot`, frozen against validity conditions only), and read every
column at 96x96 and 192x192.

**Eight of the ten candidates pass.** The table, at both sizes, with the gap measured from the
higher blot to the frozen composition and the spread measured between the two blots:

| candidate | 96: blots → comp | gap / spread | 192: blots → comp | gap / spread | library min |
|---|---|---|---|---|---|
| `role_ratio` | 0.0382, 0.0934 → 0.3602 | 0.2668 / 0.0552 = **4.8** | 0.0191, 0.0475 → 0.1903 | 0.1428 / 0.0284 = **5.0** | 0.0412 |
| `min_ground` | 0.0382, 0.0934 → 0.3228 | 0.2294 / 0.0552 = 4.2 | 0.0191, 0.0475 → 0.1699 | 0.1224 / 0.0284 = 4.3 | 0.0412 |
| `ground_side` | 0.0382, 0.0998 → 0.3414 | 0.2416 / 0.0615 = 3.9 | 0.0192, 0.0513 → 0.1839 | 0.1326 / 0.0321 = 4.1 | 0.0000 |
| `modal_connected` | 0.0000, 0.1410 → 0.9398 | 0.7988 / 0.1410 = 5.7 | 0.0000, 0.1405 → 0.9391 | 0.7986 / 0.1405 = 5.7 | 0.0000 |
| `sobel` | 0.0752, 0.3956 → 1.3876 | 0.9920 / 0.3204 = 3.1 | 0.0419, 0.2019 → 0.7405 | 0.5387 / 0.1600 = 3.4 | 0.0000 |
| `tile@16` | 0.0250, 0.1708 → 0.5083 | 0.3375 / 0.1458 = 2.3 | 0.0250, 0.1708 → 0.4917 | 0.3208 / 0.1458 = 2.2 | 0.0000 |
| `tile@12` | 0.0303, 0.2235 → 0.5455 | 0.3220 / 0.1932 = 1.7 | 0.0303, 0.2197 → 0.5455 | 0.3258 / 0.1894 = 1.7 | 0.0000 |
| `tile@6` | 0.0000, 0.3833 → 0.8000 | 0.4167 / 0.3833 = 1.1 | 0.0000, 0.3667 → 0.8000 | 0.4333 / 0.3667 = 1.2 | 0.0000 |

`flatness^-1`, `boundary`, `components`, `tile@4`, `tile@8` and `border_ground` fail. `border_ground`
failing is the checked expectation rather than a surprise: a blot reaches the frame edge, so
"the ground owns the border" finds the blot.

**Passing is not choosing**, which is why this decision exists. The stop condition was registered to
reject, and eight survivors mean the argument moves to what the column *is*.

## Decision

**We will read term two against the reference a role classifier assigns, and the classifier is
`role_ratio` = `coverage(BLACK) / coverage(derived ground)`.** Above the cut the modal band is the
**figure** and `boundary_density` reads `BLACK`; below it the modal band is the **ground** and the
term reads the derived ground, exactly as it does today. The cut is **1.17 at 96x96**, the midpoint
of the lower blot's ratio (1.2718) and the highest ratio among the conditional population's non-blot
members (1.0780).

**`boundary_floor` is 0.23, derived at 96x96** as the midpoint of two anchors of one kind of
quantity read on one column at one size: the higher blot's `0.0934` and the frozen composition's
`0.3602`, both `boundary_density` under the role each frame is assigned. It sits 2.4x above the blot
and at 0.63x the composition. **It is named with its capture size and it does not travel**: the
statistic goes as ~`1/L`, so the same anchors at 192x192 give `0.12`, and a gate reading at another
size re-derives from its own anchors rather than scaling this one
([ADR-0071](0071-a-numeric-test-contract-states-a-property-or-names-its-machine.md)).

We chose `role_ratio` over three candidates that separate at least as well because **it is the only
one that answers the question the defect asked.** ADR-0161's finding is that term two reads the
wrong reference on a blot; `role_ratio` names that choice and makes it, and term two stays the
statistic ADR-0130 established rather than becoming a new one.

## Consequences

### Positive

- **The defect record inverts back into a conviction.** `Blown Out` is classified figure at ratio
  27.58, reads `0.0382` against `BLACK`, and is convicted — as a blot, with the message that says
  so, rather than as "blank" on `coverage`.
- **`Tiled Rosette Mono` is admitted for the right reason.** At ratio 1.0780 it is classified
  ground, reads its full `0.3602` against the derived ground, and clears the floor by a wide margin.
- **Both anchors agree at both sizes, and the classifier is nearly size-invariant**: the two blots
  read 27.58 / 1.2718 at 96 and 27.70 / 1.2734 at 192, against a cut that moves 1 % between them.
- **Term two keeps its meaning.** The column is still `boundary_density` against a reference, so
  ADR-0130's conditioning and ADR-0129's composition-scale correction carry over untouched.

### Negative

- **Two constants where there was one.** The cut and the floor both rest on measurement, and both
  are new surface to keep honest. The cut in particular is a **two-point calibration with a
  population bound** — the lower blot against the highest non-blot in the conditional population —
  which is the shape ADR-0161 convicted when the second point was a single fixture. What makes it
  survivable is that the population end is a bound over many frames rather than one, and that the
  gap it straddles (1.0780 → 1.2718) is read identically at both sizes.
- **28 library frames sit between the higher blot and the composition at 96x96**, and every one of
  them is cleared by **term one** rather than by term two. The conjunction is doing real work there,
  and a future change that loosens `MAX_TONAL_FLATNESS` would convict them. Criterion 2 registered
  exactly this in advance: none of the 28 is above the flatness ceiling today.
- **The floor is a measurement, not a property**, so it names 96x96 and skips elsewhere in
  ADR-0016's shape. A gate that wanted to read at another size owes its own derivation.
- **A frame whose two coverages are close is classified on a small difference.** The composition sits
  at 1.0780 against a cut of 1.17 — comfortable, but the margin is a tenth, not an order of
  magnitude, and nothing in the table says where the next composition lands.

## Alternatives considered

### `min_ground` — the lower of the two boundary densities

Passes, with a separation of 4.2/4.3 and the same library minimum. **Rejected because it reaches the
right answer by the wrong route.** It convicts a frame that is unstructured against *either*
reference, so it is a conjunction hidden inside a conjunction's second term. Its failure mode is
invisible: a legitimate low-contrast print whose ink is sparse against `BLACK` but structured
against its own paper is convicted, and the record would say "structureless" about a frame that is
structured. It is the arithmetic of ADR-0161's Alternative D without its argument, which is what
Plan 0186 tabled it to test.

### `modal_connected` — the modal band's largest 4-connected component

The widest separation in the table (5.7 at both sizes) and the most size-stable figure in it
(0.7988 against 0.7986). **Rejected on its library minimum.** The column reads 0.0000 for frames
whose ground is a single connected piece, which is an ordinary composition rather than a defect, and
three of them sit within 0.007 of the ragged blot (`Ring Orbit` 0.1474, `Torus Knot` 0.1466,
`Blueprint` 0.1508 against the blot's 0.1410). A floor between the blot and the composition would
therefore sit above legitimate frames that only term one saves, and the class is not small. It also
replaces the statistic rather than its reference, discarding ADR-0130's conditioning result.

### `ground_side` — boundary density of the modal band's own mask

Passes at 3.9/4.1. **Rejected because it answers the role question implicitly and only for the blot
case.** On a print the modal band is the paper and the column measures the paper's perforation,
which is a different quantity from the ink's boundary density; the two are then compared against one
floor. Its library minimum of 0.0000 is the same hazard as above.

### `sobel`, `tile@6`, `tile@12`, `tile@16`

All pass the registered condition. **Rejected as a class**: each replaces `boundary_density` with an
unrelated statistic, each has legitimate library frames at 0.0000, and the tile family's verdict is
a function of a grid constant this project has already seen flip between grids (Plan 0119,
`tile@4` against `tile@6`). `tile@6`'s gap is only 1.1x its own defect-class spread, which is the
narrowest margin among the passers.

### Stop, and keep the defect record

The plan's second outcome, and a real one — it has already happened twice in this line. **Rejected
because a candidate passes a condition registered before the numbers were read**, which is precisely
the evidence the two earlier stops lacked.

### Stop, and retire the conviction

The plan's third outcome: accept that a HARD gate should claim no conviction it cannot demonstrate,
and let a saturated blot be caught as "blank" on `coverage` alone. **Rejected because the
demonstration now exists.** The cost it would have accepted — a flatness reading that is a report
rather than a gate — is no longer the price of honesty.
