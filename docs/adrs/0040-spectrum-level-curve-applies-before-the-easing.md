# ADR-0040 — The spectrum level curve applies *before* the per-element easing, and is a bindable exponent rather than a named mode

> **Status:** proposed
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

## Notes

- The default being exactly `1.0` is what lets this land **before** [Plan 0037](../plans/0037-verifying-easing-transient-probe-and-dynamic-signal.md)'s
  transient probe exists. The ordering decision above is the one claim here that the probe would
  measure directly, and it is deliberately recorded as a *property* ("a decaying element falls
  perceptually evenly") rather than a tuned number, so the probe can confirm or refute it later
  without this ADR having invented a threshold it did not earn.
- `log(x)` follows `sqrt`'s existing posture on degenerate input rather than inventing a new rule:
  mathematically honest (`log(0)` is `-inf`, `log(-1)` is `NaN`), documented, with `select` and `max`
  as the guard idiom. The **scene's** curve is held total separately, because it has no author guard.
