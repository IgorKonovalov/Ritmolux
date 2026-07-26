# ADR-0035 — Asymmetric easing: `[smoothing]` accepts an `{ attack, release }` pair (supplements ADR-0019)

> **Status:** proposed
> **Date:** 2026-07-26
> **Related plan(s):** [0033](../plans/0033-internal-resolution-and-preset-surface.md)
> **Supplements:** [ADR-0019](0019-eased-parameters.md)

## Context

`[smoothing]` gives each bound parameter one time constant in seconds. It is folded onto
`Binding::tau` once at load (`preset/schema.rs:270`) and applied by `Smoother::smooth`
(`render/mod.rs:310-326`) as a one-pole low-pass, `alpha = 1 - exp(-dt / tau)`, on the injected real
`dt` so the easing is frame-rate independent. One state slot per binding, no allocation, expression
layer untouched. It has been the right shape since ADR-0019.

It is also **symmetric**, and that is the complaint. The user's ask was "pulse field reaction are way
too fast and jarring, we should smoothen it up a lot - use some qubic bezziere function or
something." The `preset-author` lane's only lever was a longer `tau`, and a longer `tau` slows the
rise exactly as much as it slows the fall. The lane's verdict: it does reduce the jarring, but the
preset gets **mushy** instead of getting the snap-then-glide a beat-driven parameter actually wants.

That is the real defect, and it is not the one the ask names. A percussive parameter — `burst` on a
kick, `mirror_reflect` on a beat, a bloom amount on an onset — wants to reach its target in a frame
or two and then glide back over most of a second. A single constant cannot express that, at any
value. The literal request (a cubic bezier ease) would shape the *curve* between two points, which
is a different and much smaller part of what reads as "jarring" than the rise/fall asymmetry is.

`smoothstep` already exists in the grammar (Plan 0019) but shapes a **value**, not a trajectory over
time: expressions are pure and stateless by hard invariant (ADR-0002, restated by ADR-0019), so a
time-varying ease cannot be authored on the preset side at all today. Wherever this lands, it lands
in the render-layer smoother.

## Decision

We will let a `[smoothing]` entry be **either** today's scalar **or** an inline table naming two
constants:

```toml
[smoothing]
hue   = 0.4                              # unchanged: symmetric, one constant
burst = { attack = 0.02, release = 0.7 } # snap up in ~2 frames, glide down over ~0.7 s
```

`Binding::tau` becomes a pair resolved at the same load-time boundary, and `Smoother::smooth`
selects which constant to use by **direction**: the incoming value above the held value takes
`attack`, at or below takes `release`. The scalar form sets both to the same number, so every
shipped preset is bit-for-bit unaffected and the migration is empty.

Validation stays where ADR-0019 put it — both constants must be finite and non-negative at the load
boundary, a bad value is a surfaced load error, and `0` continues to mean "apply instantly" on that
side.

## Consequences

### Positive
- The snap-then-glide envelope is authorable, which is the thing the content lane was actually
  reaching for and could not reach at any `tau`.
- Backward compatible by construction: a scalar means both constants, so no shipped preset changes
  and no golden moves.
- **No new machinery.** The state slot the asymmetry needs is the one the smoother already keeps,
  the load-time fold is already the right seam, and the expression layer is untouched — so the purity
  and allocation-freedom ADR-0019 bought are preserved exactly.
- Still frame-rate independent: `alpha = 1 - exp(-dt / tau)` holds whichever constant is selected, so
  a preset eases identically at 60 and 165 Hz (the property Plan 0014 exists to protect).

### Negative
- **The filter stops being linear.** A direction-dependent time constant is a rectifier: feed it a
  symmetric oscillation and the output acquires a DC offset, riding higher than the input's mean. That
  is not a bug to fix — it is precisely the envelope-follower behavior being asked for — but
  `[smoothing]` can no longer be described as a low-pass, and the docs have to say so or an author
  will be surprised when a fast-attack `hue` drifts upward under sustained material.
- One branch added to the per-binding, per-frame smoothing path. Predictable and negligible against
  the `exp` already on that line, but it is on the hot path and is noted as such.
- Two ways to write one thing. The scalar form is not redundant (it is the common case and it is what
  every shipped preset uses), but the schema now has a sum type where it had a float, and the parse
  error for a malformed table has to be as clear as the one for a malformed float.
- It answers the ask obliquely. The user said "bezier"; this ships asymmetry. If the residual
  complaint after this lands is still about the *shape* of the ramp rather than its timing, that is a
  second decision and this ADR did not make it.

### Neutral
- `Preset::smoothing` stays gone (Plan 0031 Phase 3 folded it away); this widens what gets folded, not
  when.

## Alternatives considered

### Alternative A — A full parametric ease (cubic bezier control points per param)
What was literally asked for. Rejected because it needs state this design does not have: a bezier
ease is defined between a start and an end over a duration, so the smoother would have to hold a
phase, a notion of "an ease currently in progress", and a rule for what happens when the target moves
mid-ease — which, for a parameter driven by a per-frame expression over live audio, is *every frame*.
The result would be a re-triggering envelope generator, a substantially larger design, and the
perceived problem (a jarring attack, or a mushy one) is dominated by the two time constants rather
than by the curve between them.

### Alternative B — A second table, `[release]`, beside `[smoothing]`
Cheapest possible parse change: no sum type, both tables stay `param -> f32`. Rejected because two
tables keyed by parameter name must agree on those names, and nothing would make them — an author
who renames a binding fixes one table and silently orphans the other, which is exactly the class of
inert-entry footgun Plan 0019 spent a phase eliminating for parameter names.

### Alternative C — Put it in the grammar as a `slew(x, up, down)` function
Superficially attractive because it puts the control next to the expression that needs it. Rejected
outright: expressions are pure and stateless by hard invariant, and `slew` is state by definition.
Breaking that forfeits the property that makes every binding testable as a function of its inputs and
makes a preset's output reproducible from an analysis frame alone.

### Alternative D — Do nothing; document longer `tau` as the answer
The status quo, and defensible if the complaint were merely "too fast". Rejected because the lane
tried it and reported the specific failure: a long `tau` trades the jarring attack for a mushy one,
so there is no value that satisfies both ends. A one-dimensional knob cannot reach the requested
behavior.

## Notes

- The direction test is on the **held** value versus the incoming raw value, not on the raw value's
  own derivative — so a parameter already above its new target releases toward it even if the raw
  signal is rising. That is the envelope-follower convention and is what makes the behavior stable
  under a noisy input.
- The rectification in Consequences is worth a sentence in `docs/presets.md` next to the example, not
  a footnote: it is the one behavior an author will hit accidentally.
