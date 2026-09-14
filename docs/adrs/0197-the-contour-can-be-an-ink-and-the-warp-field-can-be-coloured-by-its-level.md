# ADR-0197 — The band contour can be a hard ink, and the warp field can be coloured by its own level

> **Status:** proposed
> **Date:** 2026-09-14
> **Related plan(s):** [0184](../plans/0184-a-contour-that-is-an-ink-and-a-warp-field-that-bands.md)
> **Supplements:** [0133](0133-the-band-contour-fires-where-the-ink-changes.md) (the contour fires
> where the ink changes), [0138](0138-limited-ink-is-a-supported-palette-class-defined-at-the-draw-seam.md)
> (limited ink is a supported palette class), [0078](0078-banding-is-a-palette-coordinate-operation.md)
> (banding is a palette-coordinate operation), [0105](0105-the-mark-roster-becomes-a-fullscreen-distance-field.md)
> (a palette coordinate that is a distance)

## Context

ADR-0138 made limited ink a supported palette class, with its guarantee defined at each scene's draw
seam. Two mechanisms still put shading into a limited-ink frame, or keep a scene from making one.

**The contour is always a soft darkening.** `band_contour` returns
`1.0 - amount * (1.0 - smoothstep(0.0, w, d))`, a scalar multiplied into the colour and ramped over
one `fwidth`. ADR-0133 fixed *where* it fires and deliberately left *what* it draws alone, with no
parameter. Backlog 0140 measured the cost on `shape_contourmono` at 640x360, fully driven: **9**
distinct colours with the contour off and **684** at the shipped `palette_contour = 1.0`. The set of
touched pixels is fixed by geometry and `amount` only sets how dark they go, so a low value is not a
compromise: `0.25` already costs the jump from 9 colours to 80 and draws a line nobody can see. On a
palette quantized by `palette_steps`, every band edge is already hard, and the contour is the only
source of intermediate values in the picture.

**`warp_mesh` colours its light before the feedback, so the palette cannot band what the feedback
builds.** The deposit pass computes `coord = hue + color_center + color_span * (ang / 2pi)`, bands
it, samples both LUTs and writes premultiplied colour into the field. The warp then decays and drags
already-coloured pixels, and the present pass has no LUT at all. `palette_steps` quantizes the light
going in rather than the structure coming out. Backlog 0146 rendered a 20-band plateau palette with
arms and decay, and got a smeared coloured blob with **no bands at all**. The look the content lane
asked for is a mono world whose hard ink bands are the feedback field's own decay contours, an
op-art ladder marching outward. Nothing else in the engine makes a decay contour, and `shape_field`
already shows the shape of the answer: its palette coordinate is a *distance* (ADR-0105), which is
what makes `palette_steps` draw concentric contours.

Three facts about the tree constrain both halves.

- **The contour is one WGSL function written six times**, and the drift test covers four of them.
  `fragment_field.rs`, `reaction_diffusion.rs`, `shape_field.rs`, `warp_mesh/shaders.rs`,
  `analytic_field/shader.rs` and `cellular/shader.rs` each carry `fn band_contour`, and all six are
  byte-identical on 2026-09-14. But `the_contour_reaches_the_fragment_sites_and_not_the_vertex_one`
  in `core/src/render/palette.rs` iterates only the first four. This is ADR-0133's Outcome again,
  two sites later.
- **The field already carries a level.** It is `Rgba16Float`, premultiplied. The deposit adds light
  with `ADDITIVE_LIGHT_SATURATING_COVERAGE`, and the warp multiplies all four channels by
  `decay^dt`. So a pixel's light is exactly the accumulated, decayed deposit. An uncoloured deposit
  would make any channel of it the field's level, with no second texture.
- **Default output has to stay where it is.** Every shipped preset on the six contour scenes and
  every native `warp_mesh` preset sets neither new control. The golden corpus includes
  `warp_mesh.toml`, `warp_mesh_shader.toml` and `shape_field.toml`.

## Decision

We will make two additions. Each is off by default and reproduces today's arithmetic textually when
off.

**1. The contour gains a style and an ink, on every scene where it is live.** Two parameters join
the shared palette block (`common::PaletteParams`) and each of the six contour scenes' `PARAMS`:

- `palette_contour_style` is **Structural**, `0`..`3`, rounded on the CPU and decoded into two flags
  the way `echo_orient` is:
  - `0`, the default, is today's soft darkening, and its arm keeps today's expression verbatim.
  - `1` is a **hard** darkening. The line covers the pixels with `d < w`, the same footprint the soft
    ramp spans, as a step instead of a ramp.
  - `2` is a soft line in the ink colour.
  - `3` is a hard line in the ink colour.
- `palette_contour_ink` is **Modal**, `0`..`1`, a palette coordinate. Styles 2 and 3 draw the line in
  the palette's own colour at that coordinate, crossfaded A/B by `palette_mix` like every other
  sample. It is an absolute LUT position, not shifted by `hue`, so it names a stop.

The shared function becomes `band_contour_ink(col, …) -> vec3<f32>` and returns the finished colour.
Its ink arm is `mix(col, ink, amount * cover)`, where `cover` is the ramp or the step and `ink` is
black (styles 0 and 1) or the LUT sample (styles 2 and 3). ADR-0133's early-out stays first, ahead
of every sample, and so does its equality test for where the line fires. At `amount = 1` a hard line
writes the ink exactly. On a plateau palette whose inks already include it, that adds no colour the
contour-off frame lacked. The drift test iterates every copy.

**2. `warp_mesh` gains a second palette coordinate, the field's level, and the two coordinates
exclude each other.** A scene parameter `color_source` (**Structural**, `0`..`1`, rounded) selects
which one is live:

- `0`, the default, is today. The deposit pass colours by angle and the present pass has no palette.
- `1` is **the level**. The deposit writes uncoloured light (`vec3(amount)`, no LUT sample, no band,
  no contour) with the same coverage it writes today. The present pass reads the field's level as
  `max(c.r, c.g, c.b)`, computes `coord = hue + color_center + color_span * level`, bands it with
  `band_coord`, samples both LUTs, applies `saturation` and the contour (a seventh copy of the
  function, on the level coordinate), and writes `ink * coverage` with the coverage alpha it writes
  today. The LUT is repeat-addressed, so a level past `1 / color_span` wraps onto the palette again,
  and the ladder repeats outward instead of clipping.

The two paths exclude rather than compose. Colouring by angle and then by level would make the level
depend on the angle's colour (a coloured deposit has a hue-dependent maximum channel). The bands
would then be decay contours bent by the palette, which is neither look. The level is taken
**after** the video echo mixes its two samples, so the echo mixes levels and the result is coloured
once. MilkDrop's composite remaps still apply after that, as they do today. A converted `[milk]`
preset never sets `color_source`, so its path is untouched.

## Consequences

### Positive

- **A limited-ink print can carry a drawn key and stay a limited-ink print.** At style 1 or 3 and
  `amount = 1`, the contour is one more flat ink rather than 675 intermediate colours.
- **The key can be the palette's own ink**, for example the red on `shape_contourmono`, instead of
  whatever `col * k` lands on.
- **`warp_mesh` opens to hard-ink and posterized looks**, which backlog 0146 says it is closed to
  today. The level coordinate is the only decay contour in the engine.
- **The drift gap closes as a side effect.** The test that holds the contour copies together covers
  all of them, including the two it has silently skipped since Plans 0163 and 0164.
- **No golden moves**, because both new controls default to the existing arms and the arms keep
  their expressions.

### Negative

- **Two parameters on a surface ADR-0133 kept parameterless.** ADR-0133's argument was that the
  engine could derive *where* the line goes, and that still holds. *What* the line is drawn in cannot
  be derived: the ink an author wants for a key is a choice about the look, and backlog 0140 names
  two different ones (the palette's dark and its red) for the same preset. The cost is two rows in
  the generated reference and two names on six scenes.
- **Six uniform layouts change.** Each contour site packs its palette controls into a scene uniform
  whose slots are mostly spent. Where no slot is free, a `vec4` is appended to the WGSL struct and the
  Rust POD, so the edit is six scenes wide.
- **The shared function's signature changes at six sites in one commit.** That is the verbatim-copy
  discipline doing its job, and it is still six edits.
- **A hard line aliases.** A step at one `fwidth` has no anti-aliasing. On a quantized palette every
  band edge already has none, which is the premise, but a hard contour on a smooth palette will read
  as jagged. The style is for limited ink, and the palette doc has to say so.
- **The level coordinate's working range is unmeasured.** The field's level depends on `deposit`,
  `decay` and the draw layer, and `color_span`'s declared range stops at `1`. If typical levels sit
  far below one, a full palette cycle needs a span the range will not accept. The plan measures the
  level on its fixture before the range is judged adequate, and a documented gain is the named
  repair.
- **The fringe is not two-ink.** In level mode the output is `ink * coverage`, and coverage decays
  with the level, so the outermost rungs fade toward the backdrop through intermediate values. That
  is ADR-0138's draw-seam guarantee (flat inks where the field is opaque), not a whole-frame one.
  Whether it reads as the ladder dissolving or as shading is the plan's look gate to judge.
- **Level mode costs the present pass per pixel**: two LUT samples, plus the contour's four when it
  is on, at display resolution. Mode 0 pays one uniform branch.

### Neutral

- `color_source` is a `warp_mesh` parameter, not a shared one. No other scene has a colour path that
  runs before a feedback loop.
- The deposit's angle coordinate, its arms and its spin keep shaping *where* light lands in level
  mode. They stop deciding its colour.

## Alternatives considered

### Alternative A — Only a hard contour, no ink colour (backlog 0140's smaller option)

**Rejected because a hard black line is one ink, and the look that raised the entry wants a key in
the palette's own colour.** The backlog is explicit that the colour option subsumes the hard one only
if it also gets a hardness, so the enum carries both axes for one Structural name instead of two.

### Alternative B — Derive the ink from the two bands the line separates (the darker, or the other one)

That would need no parameter, which is ADR-0133's shape. **Rejected because on a two-ink boundary it
draws nothing visible.** A line in the darker of black and white is black laid beside black, so it
thickens a band instead of drawing a key. A contour that reads as a line has to be a third colour,
and a third colour is an authored choice.

### Alternative C — Compose the two warp colour paths (multiply, or crossfade by a continuous control)

For example, colour by angle at deposit and modulate by level at present, or a Modal `color_source`
that crossfades. **Rejected because every intermediate state is a mixture of two inks**, which is
exactly the non-ink value the level coordinate exists to remove. A coloured deposit also makes the
level hue-dependent (Decision, 2). A preset dissolve between a mode-0 and a mode-1 world still blends
pixels, which is the transition's job and not the parameter's.

### Alternative D — A second field texture carrying a scalar level beside the coloured one

Keep the angle colour and accumulate a separate `R16Float` level alongside it, so both could exist at
once. **Rejected because it doubles the ping-pong field's memory and warp work to support a
composition Alternative C rejects on its own terms.** An uncoloured deposit already makes the existing
field the level.

## Notes

- Raised as backlog 0140 (2026-08-27, from Plan 0121 Phase 6) and backlog 0146 (2026-08-27, from the
  content lane).
- The two shared names follow the `palette_contour` prefix, so the reference groups them. The
  `echo_orient` pattern (a Structural `0`..`3` decoded into two flags on the CPU) is the precedent
  for packing two independent choices into one rounded control.
