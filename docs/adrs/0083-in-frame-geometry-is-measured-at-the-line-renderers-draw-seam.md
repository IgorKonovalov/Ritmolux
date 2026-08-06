# ADR-0083 — In-frame geometry is measured at the line renderer's draw seam

> **Status:** accepted 2026-08-06 (see **Outcome**)
> **Date:** 2026-08-04
> **Related plan(s):** [0069](../plans/done/0069-the-instrument-that-sees-a-figure-leave-the-frame.md)
> **Supplements:** [ADR-0067](0067-coverage-measures-the-scene-not-the-backdrop.md)

## Context

[Plan 0058](../plans/done/0058-the-gate-can-see-an-empty-frame.md) closed the *total* case — a
figure so far out of frame that nothing is drawn — by measuring the scene against black instead of
against a sampled corner pixel. It then tried to close the *partial* case, a figure whose tips
overshoot the frame, with a stimulus-relative check: capture at two excitations and assert the
louder frame does not draw less picture. That shipped as a **report, not a gate**, and the numbers
are the reason:

```text
 ratio   cov@0.4  cov@1.0  preset
 0.8552   0.2878   0.2461  De Jong          <- lowest legitimate (correct content)
 0.9568   0.3164   0.3027  Leviathan        <- correct content
 1.0514   0.3866   0.4065  Spectrum Corona  <- OVER-SCALED, scale = 5.20
 1.0891   0.5088   0.5541  Spectrum Comb    <- OVER-SCALED, scale = 3.80
    inf   0.0000   0.0000  Spectrum Ridge (pre-repair)  <- 0/0, no denominator
```

**No threshold on this axis convicts anything it was built for.** The two over-scaled presets score
*above* 1.0 — they draw more when loud — because a comb roots every bar on a shared baseline and a
corona roots every spoke at a centre, so clipping the tips costs a rounding error of lit pixels
while the body stays where it was. Meanwhile the only content near a plausible threshold is
*correct*: the attractor family's peak-buys-structure idiom, which
[ADR-0062](0062-clamp-occupancy-is-the-saturation-instrument.md) already records as real. A gate at
`0.80` would sit `0.055` from De Jong and catch none of the three known-defective configurations.

So the measure is wrong, not the threshold. Tips are almost no pixels; asking a pixel-coverage
statistic about a figure that overshoots its frame is asking the wrong question.

ADR-0067 named the successor in its Alternatives — the fraction of *drawn geometry* that lands
inside the render target — and rejected it as the primary mechanism partly because it "needs a
`Scene`-adjacent accessor", and widening the `Scene` trait is
[ADR-0002](0002-layered-preset-architecture.md) territory. That objection assumed the measurement
had to be taken inside each scene. It does not.

`LineRenderer::draw` (`core/src/render/scenes/lines/renderer.rs:306`) already receives
`segments: &[SegmentInstance]` — every endpoint of the whole figure — **and** the render target's
aspect, in one place, for all four line-family scenes. The quantity ADR-0067 wanted is computable
there, from data already in hand, without any scene knowing it is being measured.

Recorded as [design-backlog 0054](../design-backlog-archive.md#0054--pixel-coverage-cannot-see-a-figure-whose-tips-leave-the-frame-and-an-in-frame-geometry-fraction-is-the-successor) (archived at Plan 0069's close).

## Decision

We will measure the **in-frame geometry fraction at the line renderer's draw seam** — the share of
total segment length lying inside the render target's world rectangle — computed inside
`LineRenderer::draw` behind a diagnostic switch that is off in the shipped render path. The `Scene`
trait does not change, no scene implements anything, and all four line-family scenes
(`parametric_curve`, `lsystem`, `star_pattern`, `spectrum`) are covered by one implementation.

The families that build no segment list — `fragment_field`, `reaction_diffusion`, `attractor`,
`swarm`, `emitter` — are **not covered**, and keep pixel coverage. This instrument is per-family by
construction, and we accept that rather than reaching for something universal and blunt.

## Consequences

### Positive

- **It measures the thing.** A comb whose bars overshoot loses in-frame *length* in exact
  proportion to the overshoot, which is what pixel coverage could not see because the lost pixels
  are the tips.
- **`Scene` stays thin.** ADR-0002's seam is untouched, ADR-0067's stated objection to this measure
  is answered rather than overridden, and no scene gains a method it would have implemented four
  times identically.
- **The aspect comes from the render target**, because that is what `draw` is already handed —
  which puts this measurement on the correct side of
  [ADR-0037](0037-internal-grid-is-a-resolution-not-a-shape.md) by construction rather than by
  discipline.
- **The confirmation half works today.** Repaired, the pixel-coverage ratio moved `1.0891 → 1.7196`
  (comb) and `1.0514 → 1.6756` (corona), so a content pass verifying its own repair already has a
  signal; a geometry fraction makes it a conviction as well.

### Negative

- **Half the scene families are out of reach**, and the split is not along a line an author would
  guess: it follows whether a scene rasterizes a CPU-built segment list. Any documentation of this
  instrument has to lead with what it cannot see, or it will be read as an engine-wide gate.
- **Length is not area.** A thick stroke leaving the frame costs more picture than a thin one of the
  same length, and this measure weights them equally. It is the right measure for *overshoot* and a
  poor one for anything else, which is an argument for keeping its stated question narrow.
- **A second code path through the hot draw call.** The switch must not change what is drawn, and
  the only way to know that is to assert it: identical output with the diagnostic on and off. Get
  that wrong and the instrument measures itself.
- **It cannot see a degenerate figure that stays in frame** — a curve collapsed to a point scores a
  perfect 1.0. `sanity.rs`'s against-black coverage is what catches that, and the two instruments
  are complements rather than a progression.

### Neutral

- Pixel coverage does not retire. It remains the right question for the field and particle
  families, and the right question for "is anything drawn at all" everywhere.

## Alternatives considered

### Alternative A — A `Scene`-adjacent accessor

Add a defaulted diagnostic method to `Scene` returning an optional extent, implemented by the four
line scenes. Rejected because it is the same computation written four times over data the renderer
already holds, and because a defaulted trait method is still trait surface: ADR-0002 keeps `Scene`
at the preset engine's vocabulary, and "the harness would like to ask" is not that vocabulary.

### Alternative B — The harness re-derives the geometry from the generator config

A preset declares its `[curve]` / `[generator]` config, so a test could rebuild the figure and
measure it without touching the engine. Rejected because it duplicates generator math outside the
generator, and this codebase has already paid for that class of duplication — a second
implementation drifts from the first exactly when the first changes, which is precisely when the
instrument is most needed.

### Alternative C — Keep pixel coverage and calibrate a threshold

Rejected by measurement rather than by argument: Plan 0058's table shows the two over-scaled presets
scoring *above* the legitimate content, so no threshold ordering exists that separates them. This is
the finding that makes a new measure necessary at all.

## Notes

- Non-vacuity fixtures already exist and any proposal here can be tested against both before it is
  trusted: `core/tests/sanity.rs` carries `pre_repair_spectrum_ridge` as a frozen fixture (the total
  case), and `git show 2efb80e^:presets/spectrum_comb.toml` is the partial case.
- The world rectangle is `[-aspect, aspect] x [-1, 1]` — the line renderer maps two world units to
  the frame **height**, which is also why [backlog 0016](../design-backlog-archive.md#0016--the-spectrum-readout-has-no-width-control-and-density-makes-it-worse)'s
  `span` is a world quantity rather than a fraction of the width.

## Outcome (added at Plan 0069's close, 2026-08-06)

The decision held: the measurement lives in `LineRenderer::draw`, the `Scene` trait and the C ABI
are untouched, all four line families are covered by one implementation, and the diagnostic is
byte-identically inert when off. Repairing the two frozen defects moves the measure by **`0.4975`**
(comb, `0.3563 → 0.8538`) and **`0.7788`** (corona, `0.1639 → 0.9428`) — the conviction pixel
coverage could not deliver on any threshold. Two things this ADR did not anticipate are worth
carrying forward.

**It is a paired instrument, not a gate — and no absolute threshold orders this library either.**
This ADR framed the new measure as the successor *gate*, on the strength of pixel coverage having no
separating threshold. Measured over the shipped set, the geometry fraction has none either:
`Rose Zoom` reads `0.3492` and `Rose Overflow` `0.3659`, **bracketing** the frozen over-scaled comb's
`0.3563`, and both are working exactly as authored — `zoom` bound to `2.15..3.09` and `scale` driven
to `2.84` are what those preset *names* mean. A length fraction cannot distinguish "deliberately
inside the figure" from "accidentally outside the frame" because they are the same picture. So
`core/tests/geometry_extent.rs` convicts a configuration against **its own repair**, by name, and an
`assert!(fraction > 0.5)` over the library would fail two shipped presets — the identical mistake
this ADR catalogues pixel coverage making, one axis over. The gain over pixel coverage is real but
narrower than "a gate": it is a *paired* comparison that now works, where before none did.

**The achievable separation is bounded by the same geometry that hid the defect.** Plan 0069 asked
for an order of magnitude on the `0.055` benchmark; the comb delivers `9.0x` and the shipped bar sits
at `5x`. A comb roots every bar on a shared baseline, so a fully-driven bar at `scale = 3.80` keeps
roughly `0.47` of its own length below the top edge whatever else is done to it, capping the
separation near `0.53` before the repaired preset's own losses. The baseline-rooted fact that made
this figure invisible to pixel coverage is still present here, as a much weaker version of itself.
Expect a *bounded* margin from any baseline-rooted family, not an unbounded one.

**One caveat on the `Nx` framing.** `separation / 0.055` divides an in-frame-length-fraction
difference by a pixel-coverage-ratio difference: those are not the same kind of quantity, so per
[ADR-0074](0074-a-ratio-against-an-in-run-control-is-not-automatically-portable.md) the ratio is
presentational and the assertion is really an absolute `0.275`. That is safe here for a reason worth
stating explicitly, because it is what makes this instrument cheap to trust: the fraction is a pure
CPU computation over segment endpoints and an aspect, with no rasterizer in the loop, so it is
**machine-independent** — the portability failure ADR-0074 records does not apply. The live exposure
is *content* drift, and the margin is a factor of `1.8` on the tighter pair.
