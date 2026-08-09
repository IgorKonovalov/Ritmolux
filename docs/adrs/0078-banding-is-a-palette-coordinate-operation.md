# ADR-0078 — Banding is an operation on the palette coordinate, not on the baked LUT

> **Status:** accepted (Plan 0064, closed 2026-08-09)
> **Date:** 2026-08-04
> **Related plan(s):** [0064](../plans/done/0064-the-symmetry-stage-and-the-banded-palette.md)
> **Supplements:** [0021](0021-shared-palette-system.md)

## Context

Three of the user's five reference images share a colour treatment the engine cannot produce: hard
**bands** rather than smooth gradients, and in one of them a dark **contour** separating each band.
That treatment is most of why the references read as designed graphics rather than as renders, and
it is independent of the geometry — the same field, banded, becomes a different picture.

The engine's colour surface is [ADR-0021](0021-shared-palette-system.md)'s baked LUT: a 256×1
gradient texture built on the CPU when a preset loads, sampled with linear filtering and repeat
addressing. Every shader-coloured scene reads it, and since
[Plan 0054](../plans/done/0054-the-line-scenes-catch-up.md) that includes all four line scenes. The
cyclic *hue* character of the references is therefore already reachable — a large `color_span`
wraps the repeat-addressed LUT several times over the field's range. What is missing is only the
quantization: the LUT is a smooth ramp and linear filtering smooths it further.

There are exactly two places to put the quantization, and they differ in one property that matters.

## Decision

We will quantize on the **palette coordinate, in the shader, immediately before the LUT sample** —
`t' = (floor(t · N) + 0.5) / N` for `palette_steps = N` — and draw the contour from the same
fractional position, `f = fract(t · N)`, darkening where `f` is near a band edge.

The decisive reason is that `palette_steps` must be **bindable to audio**. ADR-0021's LUT is baked
once at preset load, deliberately and correctly: baking is CPU work plus a texture upload, and the
whole point of the bake was to keep it off the per-frame path. Quantizing during the bake would make
a band count that pulses on the beat cost a re-bake and an upload every frame — reintroducing
exactly the per-frame work ADR-0021 removed. Quantizing the coordinate costs one `floor` per sample
and leaves the bake untouched.

`palette_steps` is quantized to an integer CPU-side before it reaches the shader, for the reason
already recorded elsewhere in this project: an eased parameter is continuous, and a fractional band
count leaves the band boundaries crawling instead of stepping. `palette_steps ≤ 1` means off, and is
the exact identity — `floor(t·1) + 0.5` over 1 is not, so the shader takes the unquantized path
rather than a degenerate case of the quantized one.

**The contour is scoped honestly rather than universally.** A screen-constant contour width needs
`fwidth`, which exists only in a fragment shader. Two scenes sample the LUT in the **vertex** stage —
the attractor and the swarm, one sample per particle — where there are no derivatives and, more
fundamentally, no gradient to contour: a point sprite has a single palette coordinate, so a contour
across it is not a thing that exists. So banding reaches every scene; contours reach the scenes that
render a continuous field, and the docs say which.

## Consequences

### Positive

- **Two lines of shader per sample site buy the graphic look of three reference images**, and both
  reach every scene through machinery that already exists.
- **The band count is audio-drivable.** A `palette_steps` bound to a band level or latched to the
  beat is a colour-space response no other lever in this engine produces — the picture's *tonal
  structure* changes rather than its brightness or hue.
- **ADR-0021's bake stays exactly as it is.** No new per-frame CPU work, no per-frame upload, no
  change to the palette module at all.
- **The identity case is exact.** `palette_steps ≤ 1` and `palette_contour = 0` take the unquantized
  path, so every shipped preset and every golden baseline is byte-identical.

### Negative

- **The expression is duplicated at every LUT sample site.** This project has no shader include
  mechanism; the existing practice is a verbatim copy with a comment naming the source
  (`apply_saturation` is commented as mirroring `palette.rs::desaturate`). Banding follows that
  practice, and it means N copies that can drift. A test asserting the sites agree is the mitigation
  and it is weaker than not having copies.
- **Contours are not universal, and the reason is invisible from a preset.** An author binding
  `palette_contour` on the attractor gets nothing, with no warning — the same silent-no-op shape
  ADR-0020's unknown-parameter warning exists to prevent, except here the parameter *is* known and
  merely inert on that scene. It has to be documented in `presets/README.md` or it will be
  rediscovered as a bug.
- **Banding fights bloom.** [ADR-0046](0046-linear-light-hdr-composite-bloom-tonemap.md)'s bright
  pass selects light over range and blurs it; a hard band edge inside the bright region blooms into a
  soft one, so a preset wanting crisp bands and heavy bloom cannot have both at full strength. Not
  solvable here — the two effects want opposite things.
- **A high band count plus a contour is an aliasing generator.** Where the field's gradient is
  shallow the bands are wide and clean; where it is steep they collapse below a pixel and the
  contours moire. This is the same failure the symmetry stage's inner rings have
  ([ADR-0077](0077-the-symmetry-stage-owns-one-coordinate-map.md)) and it has the same real fix — an
  LOD derived from the coordinate's screen-space derivative — deliberately not taken here.

## Alternatives considered

### Alternative A — bake the bands into the LUT

Quantize during ADR-0021's CPU bake, so the 256×1 texture *is* stepped and every scene gets hard
bands with **no shader change at all** and no per-sample cost.

Rejected, and it is genuinely the more elegant option on every axis but one: the band count could
then only change at preset load. A bound `palette_steps` would force a re-bake and a texture upload
per frame, which is exactly the per-frame work ADR-0021's bake was created to eliminate. Audio-driven
banding is the capability worth having here — a static band count is a look, a pulsing one is a
response — so the elegance loses to it. Worth revisiting if the shader duplication above proves to
cost more than the bindability is worth.

### Alternative B — posterize the composited frame as a post stage

Quantize the final RGB image in a new `PostStage`, universal by construction and written once.

Rejected because it bands the wrong quantity. The references band the *field* — the contours follow
the field's iso-lines, which is why they read as structure. Quantizing RGB bands the picture's
brightness instead, which produces the flat-shaded look of a cheap colour reduction and, worse,
applies to the bloom and the backdrop as well as to the figure. It is also strictly less reachable:
a post stage cannot see the palette coordinate that makes the effect meaningful.

## Notes

The cyclic colour in the reference images is already reachable and should not be rebuilt: the LUT is
repeat-addressed, so `color_span` above 1 wraps it and produces the repeating hue sequence directly.
What this ADR adds is only the hard edge between one cycle's colours and the next.
