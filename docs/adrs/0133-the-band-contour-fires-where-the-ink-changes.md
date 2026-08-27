# ADR-0133 — The band contour fires where the ink changes, and equality is the test

> **Status:** proposed
> **Date:** 2026-08-27
> **Related plan(s):** [0121](../plans/0121-a-rate-an-ink-edge-and-a-motion-reading.md)
> **Supplements:** [0078](0078-banding-is-a-palette-coordinate-operation.md), [0021](0021-shared-palette-system.md)

## Context

[ADR-0078](0078-banding-is-a-palette-coordinate-operation.md) put banding on the palette
coordinate and drew the contour from the same fractional position:

```wgsl
let f = t * steps;
let d = min(fract(f), 1.0 - fract(f));
return 1.0 - amount * (1.0 - smoothstep(0.0, w, d));
```

That is a pure function of position within the band grid. It never reads the LUT, so it darkens a
hairline at **every** band boundary whether or not the colour changes there. On the smooth gradients
the ADR was written against, every boundary *is* a colour change, so the distinction never came up.

The mono cohort made it come up. A limited-ink look — the two-ink-plus-accent print
`shape_contourmono` renders — is written as **plateaus**: runs of bands holding one colour, which is
the only way to get flat ink out of `palette_steps`. Its 20 steps sit in 5 runs (black 5 / white 5 /
black 4 / white 4 / red 2), so 15 of its 20 band edges are white-meets-white or black-meets-black. On
those the contour is a `smoothstep` grey line drawn across flat colour, which is precisely the
shading a two-ink print is defined by not having. At the 5 real run boundaries it has nothing to add
either, because black already meets white and that is maximum contrast.

So on any plateau palette the parameter has exactly one usable setting, and it is off. Four of the
nine presets that name `palette_contour` set it to `0` (design-backlog 0138), and the count of
limited-ink looks in this library went from one to four on 2026-08-27.

The constraint that shapes the fix: `band_contour` is **one function written three times**. The
canonical copy lives in `core/src/render/palette.rs`, and `fragment_field.rs`,
`reaction_diffusion.rs` and `shape_field.rs` each carry it verbatim, asserted by
`the_contour_reaches_the_fragment_sites_and_not_the_vertex_one`. Whatever it learns to do, it has to
learn identically at all three sites — and the sites bind their LUTs at different groups and
bindings.

## Decision

We will make `band_contour` read the LUT and fire only where the ink actually changes: at the nearest
band edge it samples the palette at the two adjacent band centres, `(n - 0.5) / steps` and
`(n + 0.5) / steps` for `n = round(t * steps)`, and draws nothing when those two resolve to the same
colour. **The test is equality, not similarity** — the contour is suppressed only when the two
samples agree to within half a code value (`0.5 / 255`), which is below the LUT's own 8-bit
quantization.

That threshold is the whole reason this needs no new parameter and no mode flag. On a smooth
palette, two distinct band centres sample two distinct positions on a ramp and differ by *at least*
one code value, so every edge still draws exactly as it does today, at any `palette_steps` and any
`color_span`. Inside a plateau the LUT is literally constant, linear filtering between two identical
texels returns that same value, and the samples are bit-equal — so the contour vanishes there and
survives at the run boundaries. One rule produces both behaviours; the parameter keeps one meaning.

The function takes the two LUT textures, the sampler and `palette_mix` as **explicit WGSL
parameters** rather than reaching for module-scope globals. All three sites happen to name them
`lut_a` / `lut_b` / `lut_samp` today, so implicit capture would compile — and would make the shared
function silently bind to whatever a future site happens to call its textures, which is exactly the
coupling the verbatim-copy discipline exists to compensate for.

## Consequences

### Positive

- **`palette_contour` becomes usable on the one family of scenes where it works at all**, on the
  looks that most want a drawn edge. A limited-ink print gets its run boundaries outlined and its
  interior left flat, which is what a print *is*.
- **Nothing a preset already sets changes.** The equality test cannot suppress a contour on a smooth
  palette, at any step count, so the five presets with a non-zero `palette_contour` render as before.
- **No new name, no new uniform slot, no new mode to document.** The parameter answers "how strong",
  as it always did; where it applies stops being a separate question.
- **ADR-0078's scoping is untouched.** This is about *which* edges, not about which shader stages
  have derivatives — the vertex-stage sites stay contourless for the reason that ADR gives.

### Negative

- **Four extra LUT samples per pixel at three sites** (two band centres, each from both the A and B
  LUTs so the crossfade is honoured). They are reads from a 256×1 texture that is already resident
  and already sampled twice per pixel, so the cost is cache-warm and small — but it is not nothing,
  and it is paid by every preset on those three scenes, including the ones with `palette_contour = 0`
  unless the early-out is kept ahead of the samples. It must be.
- **The shared function grows four parameters**, and the verbatim-copy assertion grows with it. The
  three sites must be edited together or the drift test fails — which is the test doing its job, and
  is still three edits where a self-contained function needed one.
- **A near-plateau is not a plateau.** A palette whose "flat" run is built from two stops that differ
  by one code value still contours inside the run. That is correct by the stated rule and will
  nonetheless read as a surprise to whoever authors it; `docs/preset-palettes.md` has to say so.
- **A run boundary that does not align with a band boundary is a real colour change and draws.** A
  custom-stop palette whose hard transition lands mid-band gets a contour at the nearest band edge
  rather than at the stop. Correct, and worth stating.

### Neutral

- The suppression is invisible in the reactivity, animation, distinctness and golden gates on every
  currently shipped preset, because none of them combine a plateau palette with a non-zero
  `palette_contour` — the four that would have are the four that set it to zero. The evidence that
  this works is a new preset, which is why the plan ends with a content pass.

## Alternatives considered

### Alternative A — a new `palette_contour_mode` parameter (0 = every edge, 1 = ink changes only)

Explicit, and it cannot disturb a shipped preset because nothing changes unless a preset opts in.
Rejected because the mode is not a choice anyone makes twice: for a given palette exactly one setting
is right, and which one is right is a property of the palette rather than of the author's intent. A
parameter whose correct value is derivable from data the shader already has is a parameter the shader
should derive. It would also add a name to three scenes, a uniform slot to three layouts, and a row
to `presets/README.md` for a decision the engine can make.

### Alternative B — scale the darkening continuously by how different the two colours are

The obvious "LUT-aware" reading, and the one this ADR started from. Rejected because it **breaks
existing presets in proportion to their step count**. Adjacent band centres on a smooth ramp differ
by roughly `color_span / steps` of the LUT's range, so a preset at `palette_steps = 32` would see its
contour scaled down by a factor of thirty-two against one at `palette_steps = 1`. Any normalization
that fixed this would need the palette's maximum neighbour-to-neighbour difference, which is a
whole-LUT reduction the fragment shader cannot perform. The continuous form is only definable with a
reference scale that does not exist.

### Alternative C — overload the sign of `palette_contour` (negative = run boundaries only)

No new name, no new uniform slot, no golden movement, and one line of shader. Rejected because it
spends a sign on an encoding no reader would guess, on a parameter documented in three operator docs
— and because it is Alternative A wearing a disguise: it still makes the author choose a mode whose
correct value the engine can read off the palette.

## Notes

`n = round(t * steps)` is the band edge nearest the sample, so `n - 1` and `n` are the bands it
separates and their centres are `(n - 0.5) / steps` and `(n + 0.5) / steps`. The existing
`steps < 1.5 || amount <= 0.0` early-out stays first, ahead of the samples.
