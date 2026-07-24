# ADR-0029 — Enrich the Maurer curve family via named shape params (radial offset + phase), not new families or a superformula

> **Status:** proposed
> **Date:** 2026-07-24
> **Related plan(s):** [0028-parametric-curve-shape-params](../plans/0028-parametric-curve-shape-params.md)

## Context

The `preset-author` lane, exploring an audio-driven Maurer rose against a reference
implementation (`github.com/IgorKonovalov/Maurer_Rose`), demonstrated that the *shape* of
a rose can already be swept live from a preset — `n`/`d`/`samples`/`scale` are per-frame
expressions, so binding them to audio morphs the geometry (no engine change). But two of the
reference's shape levers are unreachable, because our sampler lacks them.

Our sampler (`core/src/render/scenes/lines/curves.rs`) computes `r = sin(n·θ)`, then
`(x, y) = (r·cos θ, r·sin θ)`, scaled and rotated. The reference's polar form is
`r = sin(k·φ − rotate)·size + gg`. Mapping term by term: `size` → our `scale` (have it), the
angular step → our `d` (have it), but:

- **`gg` (radial offset)** — an additive term on `r`. With a nonzero offset the rose stops
  passing through the origin and opens into spiral / annular / rosette forms. This is the
  reference's single most dramatic lever, and we have no equivalent — `r` is a bare sine.
- **`rotate` (phase)** — a phase *inside* the sine (`sin(n·θ + phase)`). This is distinct from
  our `spin`, which rotates the whole figure in screen space via a 2-D rotation matrix. A phase
  term reshapes the petal structure as it advances; a figure rotation does not.

Both missing levers are intrinsic to the curve formula, cheap (one add each), division-free
(no `NaN`/`Inf` hazard), and default to a no-op. The decision is *how* to grow the curve
scene's expressive range to reach them — and this fork recurs every time we want a richer
line curve, so it is worth recording.

## Decision

We will add the two missing levers as **named per-frame shape parameters on the existing
`parametric_curve` scene** — `radial_offset` (added to `r`) and `phase` (added inside the
sine) — evaluated through the same expression/`set_param` path as `n`, `d`, and `scale`, with
both defaulting to `0.0` so every shipped rose preset is byte-identical. The sampler becomes
`r = sin(n·θ + phase) + radial_offset`. No new `CurveFamily` variant, no superformula rewrite,
no `Scene`-trait change, no C ABI change (preset evaluation is core-internal), no new
dependency.

This is the params-not-families answer: the reference's "fantastic shapes" come from
*continuous motion* of the curve's parameters under audio, which named params deliver directly
and a fixed family enumeration cannot.

## Consequences

### Positive
- The reference's spiral/rosette/annular catalogue and phase-morph become expressible from a
  preset, driven by any audio term (`bass`, `bar`, `beat`, and — once Plan 0019 lands —
  `tempo`), with the gain-then-clamp idiom.
- Additive and zero-defaulted: the existing rose presets and the `parametric_curve` golden
  fixture (Plan 0022 / ADR-0023) are unchanged, so no baseline re-bless.
- Tiny, localized, hot-path-safe: two `f32` adds in an already-allocation-free sampler; the
  panic-denial pragma on `curves.rs`/`parametric.rs` is preserved (no indexing, no division).

### Negative
- `r` is no longer bounded to `[-1, 1]`: a large `radial_offset` (or `scale`) pushes geometry
  outside the unit disc and past the NDC frame. This is the *intended* "out of bounds" blowout
  (the motivating still), and the renderer clips cleanly, but authors can now drive the curve
  entirely off-screen — a documentation note, not a guard.
- Two more named params to keep in sync: the `parametric_curve` param list in `docs/presets.md`
  and, **once Plan 0019 Phase 4 lands its per-system declared param vocabulary**, that scene's
  `PARAMS` list (else the new names warn as unknown). A coordination note, not a blocker.

### Neutral
- The parametric-curve scene's param count grows by two; the sampler signature gains two args.
  No change to the generator (L-system / star) scenes, which do not share this sampler.

## Alternatives considered

### Alternative A — A new `CurveFamily` variant per shape (rose, rosette, spiral, …)
Each new family is a new sampler, `[curve]` plumbing, and a golden fixture, and — decisively —
a family is a *fixed structural choice made at load*, not a per-frame lever. It cannot deliver
the reference's effect, which is the shape *moving continuously* with the audio. Families are
the right tool for genuinely different curve equations (an epicycloid, a Lissajous), not for
parameters of the rose we already have.

### Alternative B — Replace the sine with a general superformula
Gielis's superformula (`r = (|cos(mθ/4)/a|^n2 + |sin(mθ/4)/b|^n3)^(−1/n1)`) spans a far wider
shape space, but it is a sampler rewrite with ~5 new params, `pow`/`abs` per sample (more hot-
path cost), and degenerate-exponent `NaN` cases to guard. It is strictly more than the routed
need: the two levers we actually lack are a two-line change that already covers the reference's
catalogue. Recorded as a possible future family (Alternative A's mechanism) if ever wanted, not
taken now.

## Notes

Supplements [ADR-0007](0007-line-geometry-generators.md) (the parametric build model this reuses).
Independent of [ADR-0020](0020-preset-grammar-v2-branching-functions-tempo.md) / Plan 0019, which
adds the `tempo` variable that these params pair naturally with — the two land independently.
The two weaker items from the same feedback are **not** addressed here: `tempo`/`bpm` exposure is
already Plan 0019 Phase 3 (not a duplicate), and beat-latched *stateful* stepping collides with
the pure-expression determinism invariant ADR-0020 deliberately holds (no state in the grammar),
so it is parked.
