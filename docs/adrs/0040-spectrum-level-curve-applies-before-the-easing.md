# ADR-0040 — The spectrum level curve applies *before* the per-element easing, and is a bindable exponent rather than a named mode

> **Status:** accepted (2026-07-28, at Plan 0038's close) — **but see
> [Outcome](#outcome-2026-07-28-after-plan-0038-phase-3s-measurement) below: the decision stands,
> its stated mechanism is falsified and corrected there.**
> Read the Outcome before quoting anything in Decision or Consequences about *even falls*.
> **Date:** 2026-07-27
> **Related plan(s):** [0038](../plans/0038-line-family-unreachable-levers.md)
> **Supplements:** [ADR-0036](0036-preset-reachable-spectrum.md) (the spectrum surface),
> [ADR-0035](0035-asymmetric-attack-release-easing.md) (the `{ attack, release }` easing this orders
> against), [ADR-0002](0002-layered-preset-architecture.md) (the expression layer)

## Context

`SpectrumScene` maps a band level to a world length with `element_length(level, base, scale)` =
`base + scale * level` (`core/src/render/scenes/lines/spectrum.rs:214`) — **strictly linear, with no
shaping lever**. Audio level is perceptually logarithmic, so a linear readout spends most of its
range on the loudest element and leaves the rest stubbed. Every conventional meter curves the value
before displaying it; this scene cannot.

It is not reachable from a preset either. The grammar has `sqrt` and `pow` but **no `log`**
(`Func::from_name`, `core/src/preset/expr.rs` — confirmed absent). And the one workaround that does
shape the length — bind `base` from `bin(index)` and set `scale = 0` — **silently discards the
`levels` that `[spectrum] smoothing` eases**, because that easing is scene state computed from the
band array, not from a binding. The author trades the scene's only temporal easing for the curve,
with no warning. Since the bands are the rawest signal in the engine, that easing is exactly what
stops the readout strobing. So the choice today is *curve or easing, never both* — which is why this
is engine work rather than a preset-authoring problem (design-backlog 0017).

Adding a curve therefore raises a question that has to be answered deliberately, because both answers
are defensible and they look different on screen: **does the curve run before or after the smoother?**

## Decision

**The curve applies to the raw downsampled level, and the smoother eases the curved value.** The
per-element pipeline becomes:

```
downsample(spectrum) -> curve -> ease  -> element_length(base, scale)
                                 ^ [spectrum] smoothing operates here, on curved values
```

This matches analog meter ballistics, which perform attack and release in the **displayed** domain
(a VU/PPM meter's time constants are specified against its dB scale, not against linear amplitude).
The practical consequence: a slow `release` reads as a **perceptually even fall**. Easing after the
curve instead makes a decaying element drop quickly through the top of its travel and then crawl
through the bottom, because a linear-domain exponential decay is not an even decay once compressed.

**The curve is a bindable `curve` parameter — an exponent — not a structural `[spectrum]` named
mode.** `level.max(0).powf(curve)`, with `curve = 1.0` the default and exactly linear. `0.5` is a
square root, lower values compress harder. It lives in `[params]` like `radius` and `rotation`, so it
is an expression like everything else and can move with the music.

**Totality is part of the decision, not an implementation detail.** This runs per element per frame
on the render path, where the project forbids `NaN` reaching a scene. The level is floored at `0` and
the exponent is clamped to `[0.05, 4.0]` before the `powf`, so no author expression can produce
`pow(0, 0)`, `pow(0, -1)` or a `NaN` length.

Separately and in support, the plan adds **`log(x)` to the expression grammar** (natural logarithm),
which serves every system rather than only this scene. It does **not** replace the `curve` param —
`log` cannot reach the scene's internal `levels` at all, so it cannot fix the easing-bypass on its
own.

## Consequences

**Positive**

- A dB-like readout becomes authorable **without** giving up `[spectrum] smoothing` — the two levers
  compose for the first time, which is the whole point of the entry.
- Because the curve is bindable, a preset can move it: compress harder as a track gets busy, or walk
  it per element with `index`.
- `curve = 1.0` is exactly `powf(level, 1.0)` = `level`, so **every existing preset and every golden
  baseline is unchanged**. The plan can assert byte-identical goldens, which makes the change safe to
  land ahead of the transient probe.
- The ordering makes `{ attack, release }` mean something stable *perceptually*, which is the domain
  an author reasons in when they say "let it fall like a meter".

**Negative — the price**

- **The smoother's time constants change meaning when a curve is engaged.** `[spectrum] smoothing`
  values were reasoned about against linear levels; under a curve they ease a compressed signal, so
  the same `release` produces a different-looking fall. This is invisible at the default and only
  bites a preset that opts in — but it means the easing and the curve are **not independent knobs**,
  and the docs have to say so.
- **`curve` interacts with `scale`, and strongly.** Measured typical levels are ~0.02–0.05; at
  `curve = 0.5` a level of `0.03` becomes `0.173`, roughly a **5.8x** boost, so a preset adopting a
  curve must bring `scale` down by about that factor for the same height. An author who sets `curve`
  alone will blow the readout off the top of the frame.
- **An exponent is not a true dB mapping.** `powf` approximates the shape; it is not `20·log10`.
  Authors wanting real dB must compose it themselves via the new `log`, on a value they can reach —
  which is *not* the scene's internal levels. The gap narrows but does not close.
- One more per-element `powf` per frame. At the 64-element ceiling that is 64 `powf` calls per frame,
  negligible against the existing per-element work, but it is not free.

## Alternatives considered

**Ease first, then curve** — the smoother keeps operating on linear level values, the domain the
existing `{ attack, release }` constants were reasoned about in, and curving becomes purely a display
concern with no coupling back into easing semantics. **Rejected because it produces the wrong
motion**: an exponential decay in the linear domain, viewed through a compressive curve, falls fast
at the top and crawls at the bottom — the artifact meter designers avoid by putting ballistics in the
displayed domain. The coupling this alternative avoids is real, but it is a documentation cost paid
once, whereas the motion artifact is visible every time an element decays.

**A structural `[spectrum] curve` key with named modes** (`linear` / `sqrt` / `log` / `db`) —
clearer intent at the call site, and `db` could mean a genuine `20·log10` mapping with a floor rather
than an approximation. **Rejected because it is fixed at load and cannot move**: it could not be
driven by an expression, which is the property that makes every other parameter on this scene useful,
and it introduces a second vocabulary (mode names) beside the expression grammar the surface already
has. A named `db` mode remains a reasonable later addition *on top of* the exponent if approximation
proves insufficient.

**Both — a named family plus a bindable strength** — most expressive, and genuinely better for an
author who wants true dB. **Rejected for this plan as premature**: it is the union of the two designs
above with the documentation and test surface of both, before there is any evidence that the exponent
alone is inadequate. Revisit if authors ask for it.

**Add `log` to the grammar and nothing else** — the smallest change, and it serves every system.
**Rejected as insufficient on its own**: an expression cannot reach the scene's internal `levels`, so
the author is still forced through the `base`-driven workaround that discards the easing. `log` is
worth having and the plan adds it, but it does not answer this entry.

## Outcome (2026-07-28, after Plan 0038 Phase 3's measurement)

**The decision stands. The reasoning given for it above is wrong, and is corrected here rather than
edited in place** (this ADR is append-only; same treatment as
[ADR-0034](0034-internal-resolution-follows-the-target.md) and
[ADR-0036](0036-preset-reachable-spectrum.md)).

Plan 0038 Phase 3 (`c9121fd`) shipped the ordering as written and measured it both ways round through
one renderer, per the plan's done-when 6. The finding routed back here is correct in its decisive
part and overstated in another; both are recorded below, because the overstated half is a defect in
the *instrument* that will otherwise mislead the next easing measurement.

### What is falsified: the "perceptually even fall"

Decision claims a slow `release` reads as a **perceptually even fall** under this ordering, against a
rejected order that "makes a decaying element drop quickly through the top of its travel and then
crawl through the bottom". **Neither half survives.** Take a step to silence from a settled level
`L`, with `release` = τ and exponent `c`:

```
curve then ease (shipped)   displayed(t) = L^c · e^(-t/τ)
ease then curve (rejected)  displayed(t) = (L · e^(-t/τ))^c = L^c · e^(-c·t/τ)
```

Both are **pure exponentials from the same starting value**. A power of an exponential is an
exponential; the curve cannot bend a shape it only rescales the rate of. So the two orderings do not
differ in evenness at all, and neither produces an even fall — an exponential spends 30 % of its
settling time on the first half of its travel (`ln2 / ln10 = 0.301`) where a linear ramp spends 56 %.
The ADR argued for a property that **no choice of ordering here can deliver**.

### What replaces it: `release` keeps its declared meaning

The ordering still matters, and the meter-ballistics intuition that motivated it was right — it was
the elaboration into "even fall" that was wrong. The real difference is in the **time constant**:

- **Shipped order.** The smoother's state *is* the displayed quantity, so the displayed fall's time
  constant is exactly τ **for every value of `curve`**. A preset's `release = 0.5` means 0.5 s at
  `curve = 1.0`, at `curve = 0.5`, at `curve = 0.25`.
- **Rejected order.** The smoother eases the linear level and the exponent is applied after, so the
  displayed fall's effective time constant is **τ / c** — engaging `curve = 0.5` would silently
  double every fall time and `curve = 0.25` would quadruple it. For a fall to a *non-zero* floor the
  displayed response is not even exponential, so `release` would stop naming any single duration.

**This inverts one of the Consequences bullets above.** "The smoother's time constants change meaning
when a curve is engaged" describes the **rejected** order, not the chosen one. Under the shipped
order the two knobs are independent *in time*; what a curve couples to is `scale`, in **amplitude**
— and that bullet (the measured 5.8x at `curve = 0.5`) is correct and remains the price. Plan 0038
Phase 5 documents the corrected version.

So the shipped ordering is the better of the two on a stronger rationale than the one it was chosen
for, and no code changes. What changes is every sentence in this repo that explains why.

### The measured evenness spread is an instrument artifact

The commit body reports fall-evenness rising with compression — 0.328 shipped against 0.385
(`curve = 0.5`) and 0.451 (`curve = 0.25`) for the rejected arm — and reads that as "the rejected
order measures more even, and the gap widens as the curve compresses harder". **That reading is not
supported, and it contradicts the closed form in the same commit body.** The spread is truncation:

`metrics::frames_to_settle` normalizes against **the segment's own last frame**, and the probe's fall
window is `WINDOW = 96` frames = 1.6 s. The rejected arm's effective time constant is τ/c = 1.0 s and
2.0 s, so at the last frame of the window it still has **20 %** and **45 %** of its travel left. That
unsettled frame becomes the "settled" reference, the measured total is short, and every threshold is
crossed early — harder for the 0.9 threshold than the 0.5 one, which is exactly what inflates the
ratio.

Solving `r(t) = 1 - f·(1 - r(1.6 s))` for each arm, with the pixel diff taken as proportional to the
level, reproduces all six frame counts within two frames:

| arm | `curve` | frames to 0.5 (pred / meas) | frames to 0.9 (pred / meas) | evenness (pred / meas) |
|-----|---------|------------------------------|------------------------------|------------------------|
| curve-then-ease | any | 19.6 / 20 | 59.7 / 61 | 0.328 / 0.328 |
| ease-then-curve | 0.5 | 30.6 / 30 | 76.0 / 78 | 0.403 / 0.385 |
| ease-then-curve | 0.25 | 38.6 / 37 | 82.1 / 82 | 0.470 / 0.451 |

Measured to settlement instead, both arms read `ln2 / ln10` = 0.301 at every `curve`, and the fall
lengths are 69 frames shipped against 138 and 276 rejected — the `1/c` speed ratio the closed form
predicts, and nothing else. **The closed-form argument is what falsifies this ADR; the pixel
measurement neither adds to it nor supports the direction claimed.**

The probe's guard against this condition — `assert!(response.fall_frames < WINDOW)`, "the measurement
is clamped rather than measured" — **cannot fail**, because normalizing against the last frame
guarantees the threshold is crossed inside the segment. Plan 0038 gains **Phase 7** to fix that:
nothing in Plan 0037's harness can currently distinguish "settled at frame k" from "still moving at
frame k", which is the whole difference between a measurement and a truncation.

### Also learned

- **A `curve` cannot buy an even fall, and no ordering can.** That want is real and now has a home:
  **[design-backlog 0021](../design-backlog.md)** — a rate-limited (slew) release, which is the
  half of backlog 0006 that [ADR-0035](0035-asymmetric-attack-release-easing.md) deliberately did not
  take. ADR-worthy as a supplement to [ADR-0019](0019-eased-parameters.md) if acted on.
- **The closed form was available before the measurement and would have caught this at design time.**
  Two lines of algebra on `Easing::step` (`core/src/preset/schema.rs:223`) settle the question with no
  GPU, no fixture and no probe. The lesson is not "measure more" — the plan was right to demand a
  measurement — it is that a claim about the *shape* of a one-pole response is arithmetic first.
- **The ordering is still worth pinning by test**, for the reason the scene's unit test gives (a later
  refactor can swap two lines invisibly). Its stated *direction* is right and its explanation of that
  direction is the falsified narrative; Plan 0038 Phase 5 corrects the comment, not the assertion.
- **This is only the step-to-silence case.** The two orders are genuinely different functions in
  general — `curve` is nonlinear, so it does not commute with the smoother for a fall to a non-zero
  floor or for a rise/fall mix. The shipped order remains the one where `release` names a duration in
  the domain the author is looking at; the rejected one does not, in any case.

## Notes

- The default being exactly `1.0` is what lets this land **before** [Plan 0037](../plans/done/0037-verifying-easing-transient-probe-and-dynamic-signal.md)'s
  transient probe exists. The ordering decision above is the one claim here that the probe would
  measure directly, and it is deliberately recorded as a *property* ("a decaying element falls
  perceptually evenly") rather than a tuned number, so the probe can confirm or refute it later
  without this ADR having invented a threshold it did not earn.
- `log(x)` follows `sqrt`'s existing posture on degenerate input rather than inventing a new rule:
  mathematically honest (`log(0)` is `-inf`, `log(-1)` is `NaN`), documented, with `select` and `max`
  as the guard idiom. The **scene's** curve is held total separately, because it has no author guard.
