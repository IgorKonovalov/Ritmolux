# ADR-0150 — The level question is asked in linear light, over the lit set

> **Status:** proposed
> **Date:** 2026-08-29
> **Related plan(s):** [0137](../plans/0137-the-metrics-measure-light.md)

## Context

`core/src/render/metrics.rs` is the shared instrument for judging a rendered frame, and it answers
*shape* questions well: `coverage`, `peak_to_mean`, `tonal_flatness`, `boundary_density`,
`quadrant_spread`, `radial_shell_occupancy`. It answers **no level question at all**. Nothing
reports how much light a frame carries, so *"this change made the library brighter"* is not a
statement anything in this repository can produce a number for, and `shot --report` has no column
for it.

The reachable substitute is a mean over `luma()`, and it is in the wrong space. `luma()` applies
Rec.601 weights to the stored `u8`, so it measures **gamma-encoded code value**. Measured on
`star_rosewindow` during Plan 0114's Phase 6 retune, `brightness` `1.0 → 0.70`:

| quantity | change |
|---|---|
| frame's encoded mean | 5 % |
| frame's **linear** light | 13 % |
| the profile change it was meant to offset | **1.87x** |

A level match read off the encoded mean lands nowhere near. The lane doing that retune had no
instrument and wrote one in a scratch directory.

**The module already contains the argument against itself.** `linear_diff` decodes to linear light
before differencing, and its doc comment says why: sRGB's transfer curve is concave, so a parameter
easing linearly toward its target *"crosses 90 % of its pixel change early on the way up and late on
the way down, and a symmetric `[smoothing]` entry would measure asymmetric."* That reasoning is not
specific to a step response — it is the reasoning for any comparison of two levels — but the decode
is confined to `frames_to_settle` / `segment_settled` / `step_response`, and `srgb_decode_lut` is
private. The other statistics never see it. The decode is meanwhile re-derived by hand in
`core/src/render/ink/tests.rs` and `core/tests/transition.rs`, because the table that exists cannot
be reached.

Two of this project's standing rules are level claims that nothing measures. The **additive
ceiling** — the first thing `references/craft.md` teaches, and the failure behind most broken
presets — is a statement about light stacking past what the tonemap can hold; `coverage` sees a
frame that has *already* blown out, and cannot see one approaching. And ADR-0124's Phase 4 asked
whether a crisper stroke reads brighter *because no test in this repository settles it*.

The decision is not *whether* to add a level statistic — it is **what the statistic is**, and that
is a real choice because the obvious answer is background-dominated. On `star_rosewindow`, the
background carried enough of the frame that a 30 % source trim read as **3 %** on a frame mean.

## Decision

**Add one level statistic to `metrics`, defined as the mean linear light over the lit set**, where
"lit" is the predicate `coverage` already defines. `srgb_decode_lut` becomes reachable inside the
module and is used by it; the hand-rolled decodes in `ink/tests.rs` and `core/tests/transition.rs`
are replaced by it.

The lit-set restriction is the substantive half. A frame mean is simple and answers a question
nobody asks — a preset's background is a deliberate authored constant, and folding it into the
level reading makes the reading mostly a measurement of that constant. Restricting to the lit set is
what made Plan 0114's Phase 6 numbers legible.

**The threshold inherits code space, and that is accepted rather than solved.** `coverage`'s
predicate is a threshold on code values, so "which pixels are lit" is decided in the space this ADR
is moving *away* from. The alternative — a lit predicate in linear light — changes `coverage` itself
and would move blessed baselines across the whole suite for no gain to the question being asked
here. The statistic is therefore *linear light over a code-space-selected set*, and its doc comment
must say exactly that.

**The existing shape statistics do not move.** `coverage` and `peak_to_mean` are threshold and ratio
measures where code space is defensible, and `peak_to_mean` documents its 8-bit saturation
deliberately. The claim is that the *level* question is missing, not that the shape ones are wrong.

**`shot --report` gets a column.** The table is already nine columns and a level number has no
per-preset baseline to compare against, which is the honest case against — but the retune that
produced this entry needed exactly a cross-preset comparison, and a column is how this repository
makes one.

## Consequences

### Positive
- "Did this change make the library brighter, and by how much" becomes answerable with a number.
- The additive-ceiling rule gets an instrument that can see a frame *approaching* the tonemap knee,
  not only one that has already washed out.
- One sRGB decode in the codebase instead of three, and the private table stops being private for no
  reason.
- A look gate stops having to serve as the instrument for arithmetic it was never meant to do.

### Negative
- **A tenth `--report` column**, on a table that is already wide, carrying a number that is only
  meaningful in comparison. Somebody reading one preset's row learns nothing from it.
- **The lit predicate is still a code-space threshold**, so the statistic is not clean linear-light
  reasoning end to end. This is written into the doc comment rather than hidden, and it is a real
  seam a future reader can trip on.
- **Two hand-rolled decodes get deleted**, which touches `core/tests/transition.rs` — a file whose
  assertions are about timing, not level. A mistake there is a broken test in an unrelated area.
- The statistic is background-independent by construction, which means a preset that goes wrong *by
  changing its background* is invisible to it. That is the correct tradeoff for the question, and it
  is a blind spot.

### Neutral
- No blessed golden moves: this adds a statistic and a column and changes no rendering.

## Alternatives considered

### Alternative A — Frame-mean linear light, no lit-set restriction
Simpler to define, simpler to explain, and free of the code-space threshold seam this ADR accepts.
Rejected on the measurement that motivated the entry: on `star_rosewindow` a 30 % source trim read
as **3 %** on a frame mean, because the background dominated. A statistic that reports 3 % for a
30 % change is not a level instrument, it is a background detector.

### Alternative B — Move every statistic to linear light
Internally consistent, and it would retire the "what space is this in" question for good. Rejected
because it moves blessed baselines across the entire suite for no gain to the question being asked:
`coverage` is a threshold measure and `peak_to_mean` is a ratio that documents its 8-bit saturation
on purpose. Changing them would produce a large diff of moved numbers in which a real regression
would be invisible.

### Alternative C — Leave it in the scratch directory; the look gate is the real instrument
The status quo, and it has an argument: judging whether a frame reads brighter is a human question
and the look gate answers it. Rejected because the look gate should not also have to be the
instrument for the *arithmetic*. The retune needed to know the honest size of a profile change
(1.87x) in order to set a parameter, and it got 33 % from the encoded mean — a plausible number in
the wrong space, which is worse than no number.

## Notes

Discharges [design-backlog 0132](../design-backlog.md), whose "shape of a repair, not a decision"
section poses the three questions this ADR answers: what the statistic is, whether a `--report`
column earns its width, and whether the existing statistics should move.
