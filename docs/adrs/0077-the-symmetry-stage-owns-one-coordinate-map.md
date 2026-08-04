# ADR-0077 — The symmetry stage owns one coordinate map, applied at one sample

> **Status:** proposed
> **Date:** 2026-08-04
> **Related plan(s):** [0064](../plans/0064-the-symmetry-stage-and-the-banded-palette.md)
> **Supplements:** [0047](0047-kaleidoscope-fold-domain-disc-with-falloff.md),
> [0061](0061-kaleidoscope-edge-treatment-is-a-per-preset-choice.md)

## Context

The user supplied five reference images and asked for them. Three of the five — a radial mandala
with concentric shrinking copies of itself, a higher-contrast variant of the same, and an infinite
zoom tunnel — are **one mechanism**, and the engine has half of it already.

Map the plane to `(log r, θ)`. Periodicity in `θ` gives n-fold rotational symmetry, which is exactly
what `kaleidoscope.rs` does today. Periodicity in **`log r`** gives *scale* self-similarity:
concentric rings, each a shrunk copy of the one outside it. That is the missing half, and it is the
difference between a flat rosette and something that reads as fractal. A shear between the two axes
is the Droste spiral, and a translation along `log r` is a zoom — with an exact property worth
naming: translating by one full period returns a **bit-identical** image, so an audio-driven zoom
can run forever and never need to reset or crossfade. The fourth image, a wallpaper-tiled field, is
the same kind of operation in Cartesian rather than polar coordinates.

So the question is not what to build — it is **where it goes**. Every `PostStage` in this project
renders to its own offscreen and the next stage samples it, so each stage costs one bilinear
resample. Fold, radial repeat, spiral and tile are all the same kind of thing: a function from the
destination coordinate to a source coordinate, evaluated before a single texture read. Chaining them
as three stages means three resamples and three accumulations of softness — into a project whose
content lane has already reported "feels like it is upsized from something much smaller" and
"quality is poor" ([design-backlog 0003](../design-backlog.md)), and which spent
[Plan 0033](../plans/done/0033-internal-resolution-and-preset-surface.md) fixing exactly that class
of complaint.

There is a second, sharper reason the terms want to be in one place. The radial repeat does not
magnify toward the centre — it **minifies**. A destination annulus at radius 0.0125 displays the
source's canonical annulus at radius 0.4, which is a 32× compression at a ratio of 2 after five
repeats. A bilinear sampler reads four texels; it needs about a thousand. So the inner rings alias
severely, and the control that stops it is an inner-radius cutoff that has to be reasoned about
*together with* the fold's existing disc and falloff ([ADR-0047](0047-kaleidoscope-fold-domain-disc-with-falloff.md))
and the edge treatments [ADR-0061](0061-kaleidoscope-edge-treatment-is-a-per-preset-choice.md) is
adding. Split across stages, three separate radius policies would each be right alone and wrong
together.

## Decision

We will grow the existing kaleidoscope into **the symmetry stage**: one `PostStage` that composes
the whole destination-to-source coordinate map — wallpaper tile, then angular fold, then log-radius
repeat, then spiral shear — and performs exactly **one** texture read. The terms keep the `kaleido_`
prefix for continuity with every shipped preset; what changes is the stage's scope, not its name.

Three properties fall out of putting them together and are the reason for the decision:

- **One resample.** The composed map is as sharp as the fold alone is today, no matter how many terms
  are active. Three stages could not be.
- **One radius policy.** The inner cutoff, `r_max`, ADR-0047's falloff and ADR-0061's edge treatment
  are decided once against one radius, rather than three times against three.
- **The spiral's seam condition is checkable in the one place that knows both periods.** Shearing
  `log r` by `k·θ` shifts the radius by `2πk` over one revolution, so the image closes only when
  `2πk` is an integer multiple of the log period `L` — that is, `k = m·L/(2π)` for integer `m`. We
  therefore expose the **winding number `m`** and quantize it CPU-side. An unquantized spiral draws a
  visible seam, and this is the same trap the project has already recorded once: an eased parameter
  sweeps continuously through values whose math needs integers, and the fix is to quantize before it
  reaches the shader.

Every new term defaults to the identity — `kaleido_radial = 1` (no repeat), `kaleido_spiral = 0`,
`kaleido_tile = 1` — so no shipped preset changes and no golden baseline moves.

## Consequences

### Positive

- **Three of the five reference images become reachable from a preset**, on *any* scene, because the
  stage is screen-space. The fern, the attractor, the coral and every line scene inherit it at once.
- **The infinite zoom is exact rather than approximate.** Advancing the log-radius offset by one
  period `L` reproduces the frame bit-for-bit, so a `kaleido_zoom` bound to `time` or to `bar_phase`
  is a seamless endless tunnel with no reset, no crossfade and no drift.
- **Sharpness is preserved as terms are added**, which is the property a stage-per-term shape gives
  away first and the one this project's content lane is most sensitive to.
- **The aliasing failure is controlled by a lever the references themselves justify.** Image 5's
  bright disc at the centre of the tunnel is precisely an inner cutoff; the reference shows the fix
  as well as the feature.

### Negative

- **It collides with [Plan 0055](../plans/0055-the-fold-edge-becomes-a-choice.md), which is approved
  and lives in this shader.** One of the two inherits the other's file. This is a real sequencing
  cost and the mitigation is ordering, not cleverness: 0055 is small, already approved, and its
  `kaleido_edge` is a uniform branch on the *destination* radius, which the composed map does not
  touch.
- **The stage's name no longer describes it.** `kaleidoscope.rs` will fold, repeat, spiral and tile.
  Renaming the module would be honest and would churn every reference in Plan 0055 and in four
  documents, so the name stays and this ADR is the record of what it now means.
- **The inner rings alias, and the cutoff is a workaround rather than a fix.** The correct answer is
  a mip chain on the stage's source with an explicit LOD derived from the map's Jacobian. That is a
  bigger change — the post chain's offscreens are single-level today — and it is deliberately
  deferred. Authors will find the cutoff before they find the reason for it.
- **Four more `kaleido_*` params on a stage that has four**, going to eight (nine with 0055's
  `kaleido_edge`). The composed order is fixed and not author-selectable, which is what keeps it one
  pipeline; an author wanting tile-after-fold instead of before cannot have it.

### Neutral

- The composed order is **tile → fold → radial → spiral**, expressed destination-to-source. Read
  forwards that means the polar rosette is the motif that gets tiled, which is what the reference
  images show.

## Alternatives considered

### Alternative A — one new `PostStage` per term, beside the kaleidoscope

Add a log-polar stage and a tiling stage as siblings, each with its own offscreen, each independently
orderable in the chain.

Rejected on resampling. Three stages means three bilinear filters over the full frame, and the
softness compounds — into the one complaint this project's content lane has raised most often. It
also scatters the radius policy: the inner cutoff, ADR-0047's `r_max` and falloff, and ADR-0061's
edge treatment would each be decided in a different module against a different radius, and would be
individually correct and jointly wrong. The independent-ordering freedom it buys is not freedom
anyone asked for.

### Alternative B — a general domain-warp expression, authored per preset

Let the preset write the coordinate map as an expression, so any warp is reachable without engine
work.

Rejected because it is a different language, not a feature. The preset grammar is a **scalar**,
stateless, once-per-frame expression evaluator by hard invariant; a coordinate map is evaluated per
*pixel* and returns a vector. Making it authorable means a per-pixel expression VM in a shader —
which is a compiler, a new determinism story, and a new performance story. Four named terms with
documented ranges cover the references and cost none of that.

### Alternative C — accept the aliasing and drop the inner cutoff

Let the repeat run to the centre and treat the moire as texture.

Rejected on inspection of the reference. Image 5 has a bright, clean disc at its centre precisely
where infinite repetition would be — the look the user asked for already contains the cutoff. An
uncapped repeat also makes the stage's cost unbounded in visual noise while the frame budget in
`docs/nfr.md` §7 stays fixed.

## Notes

The seam condition and the minification figure are both arithmetic rather than judgement, and both
are worth reproducing before touching this stage. Over one revolution a shear of `k` shifts
`log r` by `2πk`, so continuity requires `2πk = mL`. And at a ratio of 2, five inward repeats take a
destination annulus at 0.0125 to a source annulus at 0.4 — a linear compression of 32, hence roughly
a thousand source texels per destination pixel against a bilinear sampler's four.

The zoom's exactness is the same identity read differently: the map is periodic in `log r` with
period `L`, so an offset of `L` is the identity map, not an approximation of it.
