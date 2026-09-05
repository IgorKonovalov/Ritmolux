# Preset colour: the palette surface

Presets control colour through a **shared palette**. A palette is a gradient baked
once at load into a lookup table that the five **shader-coloured** scenes sample:

- `fragment_field`
- `swarm`
- `reaction_diffusion`
- `attractor`
- `shape_field` — the one whose sample coordinate is a **distance** rather than a
  level, which is what makes banding it draw offset contours

**All four line scenes** sample the same LUT too, but **on the CPU, once per
segment** rather than per pixel or per particle:

- `spectrum` — see [Spectrum — colour along the frequency axis](#spectrum--colour-along-the-frequency-axis).
- `parametric_curve`, `lsystem` and `star_pattern` colour along their generator's
  axis ([ADR-0059](adrs/0059-line-scenes-colour-along-their-generator-axis.md)) — see
  [The line scenes — colour along the generator's axis](#the-line-scenes--colour-along-the-generators-axis).

So **every** scene reaches `[palette]`, and a note anywhere saying a `[palette]`
table is inert on a line scene is stale.

**The backdrop reaches it too**
([ADR-0086](adrs/0086-the-backdrop-colours-through-the-preset-palette.md)): the background pre-pass
samples the same baked LUT, so `bg_hue` is a coordinate in the preset's own gradient — see
[The backdrop](#the-backdrop--bg_hue-is-a-coordinate-in-your-gradient). There is no longer any
surface in the engine that colours outside `[palette]`, and a note saying `bg_hue` walks a fixed
cosine whatever the preset declares is stale.

A preset that declares no `[palette]` gets the default **`spectrum`** palette —
the exact iq cosine the scenes used before this system existed — so every shipped
preset renders unchanged. (The one exception: `reaction_diffusion` previously used
a slightly different cosine and was unified onto `spectrum`, so the coral presets'
colours shifted; they are being re-authored to exploit palettes.)

This is the authoring counterpart to
[ADR-0021](adrs/0021-shared-palette-system.md) and
[Plan 0020](plans/done/0020-shared-palette-system.md). It documents **colour only** —
the systems, expression grammar, and file layout live in the main preset guide
(`docs/presets.md`).

---

## The `[palette]` table

An optional top-level table that selects the gradient. Give **either** a built-in
`name` **or** custom `stops` — not both (setting both, or neither, is a load
error).

### Built-in palettes — `name`

```toml
[palette]
name = "ember"
```

| Name       | Gradient |
|------------|----------|
| `spectrum` | The classic full-hue iq cosine — the **default**. |
| `ember`    | Warm embers: deep red → orange → pale gold. |
| `ice`      | Cool ice: deep blue → cyan → near-white. |
| `mono`     | Grayscale black → white. |
| `aurora`   | Deep green → teal → violet. |

### Custom gradient stops — `stops`

A gradient through your own colour stops. Each stop is `{ at = <0..1>, color =
<colour> }`, where `<colour>` is a `#rrggbb` hex string **or** an `[r, g, b]`
array of `0..1` floats. **Both are sRGB** — the colour a picker or an eyedropper
gives you, and the colour that renders:

```toml
[palette]
stops = [
  { at = 0.0, color = "#0b0b2a" },   # deep night blue
  { at = 0.5, color = "#ff5500" },   # hot orange
  { at = 1.0, color = [1.0, 0.87, 0.53] },  # pale gold
]
```

Validated at load: at least two stops, each `at` in `0..=1` and sorted ascending,
every colour parseable. Any violation is a surfaced error and the engine keeps the
last good preset (it never crashes).

**The stop is decoded to light once, when the LUT is baked**
([ADR-0151](adrs/0151-palette-stops-are-authored-in-srgb-and-converted-at-load.md)),
so what you write is what the frame carries: a plateau authored `#c81423` comes
back `#c81423`. That is exact **below the tonemap's knee at linear `0.6`**, where
the curve is the identity and the load decode and the display encode are inverses
— to within the LUT's own 8-bit storage, which costs up to two levels on a very
dark channel. Above the knee all three channels scale together, so the plateau
survives as the same ink at a lower level. There is no opt-out and no second
reading of a hex triple.

**Everything after that decode is a linear-light colour, and it stays one all the
way to the display.** The scene multiplies the decoded stop by
whatever luminance it has and writes the result into a floating-point composite
that is free to exceed 1.0, and the engine tonemap at the end of the frame is the
only place anything is compressed. So what a preset does to a palette colour is
*arithmetic on light* rather than on bytes, and two consequences follow for
authoring: a stop is never silently clipped on its way through the composite, and
the roll-off that finally applies is hue-preserving (see the ramp notes below).

The gradient **repeats** past its ends: a colour coordinate that wraps past `1`
comes back around at `0`.

> [!IMPORTANT]
> **Repeating is not the same as continuous.** The wrap is seamless only if the
> gradient's **last stop is the same colour as its first**. The `spectrum` cosine
> is genuinely periodic, so it wraps invisibly; **all four stop-list palettes
> (`ember`, `ice`, `mono`, `aurora`) run from a dark end to a light one**, so the
> wrap is the sharpest transition in the gradient, not the softest. That does not
> matter when the coordinate stays inside `0..1` — which is the usual case — but
> it matters a great deal anywhere the whole gradient is walked around a closed
> figure (see [Spectrum](#spectrum--colour-along-the-frequency-axis)). If you need
> a cyclic gradient, write custom `stops` that return to the starting colour at
> `at = 1.0`.

---

## Bindable colour parameters

Everything that *modulates* the gradient is a normal `[params]` expression over
the audio vocabulary (`bass mid treb onset beat bar time` …) — only the gradient
*shape* above is static config. Defaults reproduce each scene's prior look.

### Shared (every palette-coloured scene — **and the backdrop**)

| Param | Default | What it does |
|-------|---------|--------------|
| `hue`        | `0.0` | Rotates the gradient sample coordinate (the pre-existing hue knob). Scene only. |
| `saturation` | `1.0` | Scales chroma toward luma. `1` unchanged, `0` grayscale, `>1` oversaturated. |
| `palette_mix`| `0.0` | A/B crossfade position (see below); `0` = palette A. |
| `palette_steps` | `0.0` (off) | Quantizes the gradient into that many hard bands — see [Hard bands](#hard-bands--palette_steps-and-palette_contour). |
| `palette_contour` | `0.0` (off) | Darkens a hairline at each band edge **where the ink actually changes**. **Fragment-field, reaction-diffusion, shape-field and warp-mesh only**; inert elsewhere — same section. |

`saturation` and `palette_mix` are **one binding with two consumers**: the active scene and the
background pre-pass. You write them once and the whole frame answers — see
[the backdrop](#the-backdrop--bg_hue-is-a-coordinate-in-your-gradient). `hue` is the exception, and
deliberately: it offsets the *scene's* sample coordinate, and the backdrop has its own coordinate in
`bg_hue`.

### Fragment field, reaction-diffusion & shape field — where the field sits in the gradient

| Param | Default | What it does |
|-------|---------|--------------|
| `color_span`   | `0.6` (fragment and shape field) / `0.85` (RD) | Multiplies the scene's field level to set how much of the gradient it spans. **Low = a cohesive single-family mood**; high = a wide sweep. |
| `color_center` | `0.0` | Where that window sits in the gradient. Slide it (e.g. `0.1 + treb*0.2`) to move the tonal centre. **Cyclic** — see below. |

A warm, cohesive field: a warm palette + a low `color_span`.

**`color_center` is a cyclic coordinate, and it wraps rather than clamps.** It rides
the same repeating gradient described above, so pushing it *negative* to reach a
palette's dark end lands the field in that palette's **bright** stops instead and the
picture gets brighter — the opposite of the intent. This is not hypothetical: it cost
the content lane three rendered iterations of chasing exposure, contour density and the
palette ramp, all downstream of a cause that was none of them (design-backlog 0027).
Keep the centre inside `0..1` and know that `-0.1` and `0.9` are the same place. To
actually darken a field, change the palette's stops or the scene's own **level
param** — `glow` on the fragment field and reaction-diffusion, `brightness` on the
three particle scenes. **Not `exposure`**: that is the whole-frame stop, it moves
the backdrop and every other stage with the field, and `bloom_threshold` is
measured against it
([ADR-0080](adrs/0080-the-attractor-owns-its-level-and-bloom-thresholds-exposed-light.md)).

**Reaction-diffusion needs a much larger `color_span` than the fragment field.**
`color_span` multiplies each scene's *native* field level, and those ranges differ:
the fragment field runs the full `0..1`, but reaction-diffusion's field level (the
Gray-Scott V concentration it colours by) only reaches roughly `0..0.4`. So at the RD
default `0.85` the sampled coordinate never climbs past about `0.34` — the warm end of
a custom gradient (any stop with `at` above ~`0.4`) is never reached, and a
`teal -> orange -> gold` reef renders all-teal. To span a full custom gradient on RD,
set `color_span` to roughly **`2.0`–`2.5`** (the shipped coral presets use ~`2.2`); the
exact value shifts a little by regime, since a high-fill pattern drives the field level
higher than a thin maze. On the fragment field, keep `color_span` in the usual
`0.2`–`0.7` range.

### Swarm & attractor — the per-particle hue band

| Param | Default | What it does |
|-------|---------|--------------|
| `hue_spread` | `1.0` (swarm) / `0.15` (attractor) | Width of the per-particle hue band. **Low = a coherent, single-family cloud**; `1.0` on the swarm is the full-wheel rainbow. |
| `hue_center` | `0.5` (swarm) / `0.075` (attractor) | Centre of that band in the gradient. Two presets that differ only here render as different colours. |

Particle hues occupy `hue_center + (particle_seed - 0.5) * hue_spread`. `hue_center`
is cyclic for the same reason `color_center` is — a negative centre wraps into the
bright end of the gradient, it does not clamp toward the dark one.

**`depth_hue` rides this same coordinate on the two 3-D attractor flows, and what
it does is whatever moving along *your* ramp does** — it has three regimes, none
of them discoverable from the roster ([design-backlog 0062](design-backlog.md)):

1. **It reads as a hue cue only on a ramp that travels in hue at roughly constant
   lightness.** Rendered side by side at `perspective = 0.5`: against the
   since-retired `attractor_lorenz`'s night-blue → teal → mint → solar-white ramp, a
   `depth_hue` of `0.4` reads as the near material getting **brighter** — a second
   contrast lever pointing the same way as `depth_fade`, not an independent cue.
   Against a constant-lightness hue-travel ramp (blue → cyan → gold → orange →
   rose) the identical figure at the identical value puts a cool cyan on the far
   wing and warm gold on the near one, which is the atmospheric reading the
   parameter was built for. A dark-to-light ramp is what an additive glow scene
   is normally tuned for, so this is the trap the *default* tuning walks into.
2. **It wraps, and the wrap can make far material look near.** The offset is
   `±depth_hue/2` on a coordinate the LUT sampler **repeats** — keep it under
   **`2 * min(hue_center, 1 - hue_center)`** or one end walks off the ramp and
   wraps to the other. `attractor_lorenz`'s `hue_center` ran as low as `0.13`,
   so a `depth_hue` much above `0.26` sends the far end negative and lands the
   far material on the same bright mint as the near — the cue inverts into a
   collision. At `depth_hue = 1.0` with `hue_center = 0.20`, both ends of the
   depth range sample the *same* coordinate (`0.70`, and `-0.30` which wraps to
   `0.70`). The repeat is the LUT's documented behaviour everywhere else in the
   colour surface, so it is not clamped specially here.
3. **It is structurally dead under `ink_amount = 1`**, like `saturation`: the
   terminal remap keys on luminance and discards hue, and a depth *tint* is
   exactly the cue an ink preset cannot show. Not quite inert on a `mono`
   palette (the coordinate shift moves lightness, which the remap does see),
   but measured at `depth_hue = 0.4` it moves 42 % of pixels by a **mean of
   2/255** — a rounding error against what `depth_fade` does deliberately on
   the same frame.

**On the five IFS figures that coordinate has two more terms**, and they are the
reason a wide `hue_spread` can make them look broken
([ADR-0087](adrs/0087-the-ifs-particle-carries-its-age-and-its-last-map.md),
[ADR-0088](adrs/0088-the-ifs-colours-by-distance-from-its-own-skeleton.md)).
The full expression there is

```
hue_center + (particle_seed  - 0.5) * hue_spread
            + (which_sub_copy - 0.5) * map_tint
            +  how_far_from_skeleton * root_tint
```

**The third term is not the same shape as the other two.** `map_tint` is
*centred* — it spreads either side of the colour you chose — while `root_tint` is
**anchored at zero**: a point sitting on one of the figure's contraction points
takes no shift at all and keeps your colour exactly, and the rest of the figure
ramps away from there in one direction. So the third term only ever pushes the
coordinate one way, which matters when you are budgeting.

**And it is a budget.** All three write the same number, so **adding one means
taking authority away from another**, not stacking a third term on top. It has
been measured twice on the same preset:

| the change | why |
|---|---|
| `attractor_fern`'s `hue_spread` `0.16..0.42` → `0.05..0.125` | at the wider spread the random per-particle scatter smeared the parts together and `map_tint` was a faint wash at *any* setting |
| the same fern's `map_tint` `0.46` → `0.22` | stacked at full strength alongside `root_tint` the plant washes out; the stock preset looked better once the budget was split |

**The `*_hue` routes are the escape, and that is what they are for.** Neither
`map_hue` nor `root_hue` touches this coordinate — they rotate the hue of the
colour the ramp already returned — so a figure whose coordinate is fully spent
can still take a part separation or a depth cue through the hue route. That is
also why they throw a part *out* of a narrow palette rather than moving it
within one.

**On the fern the escape won, so the second row is a measurement rather than the
shipped tuning.** Judged against `root_hue` — which did not exist when the split
was measured — the fern keeps `map_tint` at its full `0.46` and takes its depth
cue through the hue route instead, giving up nothing. Every shipped IFS preset
binds `root_hue`; none binds `root_tint`. The budget is real; paying it is the
second choice.

One more trap on `root_tint` specifically: **its effective range is per figure**,
from `0.41` on the spiral to `1.05` on the dragon, so the same binding is not the
same look across figures. See
[the full section in `presets/README.md`](../presets/README.md#what-made-this-point-and-how-far-into-the-figure-it-is)
for the table and for the `age_*` params these replaced.

### Spectrum — colour along the frequency axis

`spectrum` is the one **line** system on this surface, and it uses the palette for
something the shader scenes cannot: it colours by *position in frequency*, so the
gradient becomes a legend. Element `i` of `n` samples the LUT at

```
hue + hue_spread * (i / n)
```

then crossfades by `palette_mix`, desaturates by `saturation`, and scales by
`brightness` — one CPU sample per element per frame, not per pixel.

| Param | Default | What it does |
|-------|---------|--------------|
| `hue_spread` | `0.0` | How far the palette is walked from the lowest element to the highest. `0` = the whole figure is one hue; `1` = the full gradient spans the axis, so you can see *where* a peak is without counting positions. |
| `hue` | `0.0` | The offset the walk starts from — rotate the whole readout through the gradient, or drive it off the audio. |

Three notes specific to this scene:

- **On `radial_ring`, `hue_spread = 1` walks the gradient exactly once around the
  circle** — the sample coordinate steps evenly from `0` to `(n-1)/n`, so the last
  spoke sits one step short of the first and the ring is covered without a repeat.
  Whether that *reads* as continuous is a property of the **gradient**, not of the
  layout: only a cyclic gradient meets itself at the wrap. `spectrum` (the cosine)
  is cyclic; **`ember`, `ice`, `mono` and `aurora` are not** — each runs dark to
  light, so at `hue_spread = 1` the ring carries a hard dark/light seam wherever
  `hue` currently puts the wrap. Either pick `spectrum`, write custom `stops` that
  return to their starting colour at `at = 1.0`, or keep the seam deliberately (it
  marks where the frequency axis begins).
- **`hue` may be per element.** It is one of the five `spectrum` params that accept
  a [per-element binding](presets.md#index--one-binding-evaluated-once-per-element),
  so `hue = "bin(index) * 0.4"` colours by *loudness* rather than by position —
  a different reading of the same figure. `hue_spread` itself is whole-figure.
- **`hue = "index"` is not the same walk as `hue_spread = 1`,** and the difference
  shows on a ring. `index` normalizes over the *span* (`i/(n-1)`, so the last
  element is exactly `1.0`), which is what makes `bin(index)` cover the whole
  spectrum; `hue_spread` normalizes over the *count* (`i/n`), which is what makes
  the steps around a closed figure even. Walking `hue` from `0` to `1` by hand
  therefore lands the last element on the same colour as the first — a duplicated
  colour rather than a closed loop. Use `hue_spread` for a ring, and `index` when
  you want the ends to be the ends.

### The line scenes — colour along the generator's axis

`parametric_curve`, `lsystem` and `star_pattern` carry the same surface `spectrum`
does ([ADR-0059](adrs/0059-line-scenes-colour-along-their-generator-axis.md)):
`[palette]`, `[palette_b]`, `palette_mix`, `hue_spread` and `saturation`, sampled
on the CPU per segment. The arithmetic is `spectrum`'s exactly —

```
hue + hue_spread * u
```

then crossfade by `palette_mix`, desaturate by `saturation`, scale by
`brightness`. What differs per scene is **`u`**, and it is a property of the
generator rather than something a preset picks:

| System | `u` is | Notes |
|--------|--------|-------|
| `parametric_curve` | position along the traced path, `i / (samples - 1)` | Normalized over the **full** curve, so `draw_progress` draws the gradient on rather than re-tinting the chords already drawn. |
| `lsystem` | generation depth over the figure's deepest generation | Branch nesting: `0` on the trunk, one more per open `[`. Normalized over the **built figure's own** maximum, not `visible_depth`. |
| `star_pattern` | normalized radius from the rosette centre | **Inert today** — see below. |
| `spectrum` | band index, `i / n` | The original; unchanged. |

`hue_spread = 0` is the default everywhere and reproduces exactly the single flat
`hue` these scenes drew before, so adding the surface moved no shipped preset.

Two honest limits, both measured rather than estimated:

- **A grammar without branches has one generation.** `lsystem_arrowhead`'s rules
  (`F -> G-F-G`, `G -> F+G+F`) contain no brackets, so every segment of it sits at
  generation 0 at all seven of its depths and `hue_spread` does nothing there
  however large. That is a property of a Sierpinski arrowhead, not a gap: every
  segment genuinely *is* at the same recursion level. `lsystem_fern` by contrast
  reaches generation 11 at `visible_depth = 6`, which is why the divisor is the
  figure's own maximum — dividing by 6 there would clamp most of the plant at the
  palette's far end.
- **`hue_spread` does nothing on `star_pattern`.** A Hankin rosette is `2n`
  *congruent* segments about the frame centre, so every one of them occupies the
  same radial interval; the spread of segment radii measures `1.2e-7` (f32 noise)
  at every tiling order and contact angle. There is no range to walk, and the
  normalization collapses to `u = 0` rather than sweeping on noise. The scene
  still gains `[palette]`, `saturation` and `palette_mix`, which are real. What is
  empty is the rosette's *interior* — at the retired `star_rosette`'s 12-fold /
  20° the strokes lived between radius 0.54 and 0.90, so the inner 60% of the
  disc was bare, and the retired `star_lantern`'s 55° variant emptied 87% of it.
  The generator question has since been answered: `rings` puts motifs at several
  radii, the ramp is computed over the combined figure, and on a rings preset
  (`star_rosewindow` ships one) `hue_spread` is a real lever — see the
  `star_pattern` notes in `presets/README.md`.

---

### The warp mesh — colour by angle around the deposit

| Param | Default | What it does |
|-------|---------|--------------|
| `color_span`   | `1.0` | How much of the gradient one full turn around the deposit centre covers. `1.0` is the whole ramp once per revolution; low values give a near-single-tone ring. |
| `color_center` | `0.0` | Where that sweep starts in the gradient. **Cyclic**, exactly as on the field scenes — `-0.1` and `0.9` are the same place. |

The deposit — the scene's light source, a gaussian ring — samples the LUT **per
pixel in its fragment stage**, so the full surface applies here: `[palette]`,
`[palette_b]`, `palette_mix`, `saturation`, and both `palette_steps` *and*
`palette_contour` are live (see the scoping table below). The coordinate is

```
hue + color_center + color_span * (angle / tau)
```

where `angle` is measured about the deposit centre (`deposit_x`/`deposit_y`), so
the ring is laid down as an angular sweep through your gradient. What makes this
scene's colour behave unlike any other: the warp then **drags that colour into
structure**, so most of what is on screen at any moment is the palette's *history*
— a `decay` near `1` keeps many seconds of old sweep in flight, and a moving
`color_center` paints time as a spiral of shifting tone.

**A converted MilkDrop preset is outside this surface by design.** A preset
carrying a `[milk]` table switches the deposit off and draws its own light — the
waveform, custom waves and shapes coloured by the `.milk` file's own authored
`wave_r`/`_g`/`_b` values. The palette params above govern the *native*,
hand-authored `warp_mesh` look; they do not re-tint an imported one.

---

### The backdrop — `bg_hue` is a coordinate in *your* gradient

The background pre-pass (`bg_hue`, `bg_bright`, `bg_vignette` — the full roster is in
[`presets/README.md`](../presets/README.md)) draws a tinted gradient *under* the scene, and it
colours through the same `[palette]` everything else does
([ADR-0086](adrs/0086-the-backdrop-colours-through-the-preset-palette.md)).

| Param | Default | What it does |
|-------|---------|--------------|
| `bg_hue` | `0.0` | Where in the preset's gradient the backdrop's tint is taken from — and, with a span, where the ramp **starts**. **Cyclic** — same wrap trap as `color_center`, below. |
| `bg_hue_span` | `0.0` | How far that coordinate **travels** across the frame. `0` takes a single sample; anything else paints a *segment* of the gradient. |

- **`bg_hue` selects a colour from the gradient you declared**, not from a fixed ramp. An `ember`
  preset draws an ember figure over an ember sky, and a custom `crimson -> gold` gradient tints its
  own backdrop. Changing `[palette]` therefore changes what every `bg_hue` value means.
- **`saturation` and `palette_mix` reach it too.** Desaturating a look desaturates the sky with it,
  and an A/B crossfade moves the whole frame rather than the figure alone.
- **When the preset declares no `[palette]`, `bg_hue` is readable straight off
  [the ramp table below](#the-line-scenes-cosine-ramp--what-hue-actually-looks-like)** — that table
  *is* the default `spectrum` gradient, so with no palette declared `bg_hue` `0.30` is cornflower
  blue, `0.45` aqua, `0.85` amber. Declare a palette and those readings stop applying, exactly as
  they stop applying to a line scene's `hue`.

**The wrap is the same trap `color_center` has**, and on the backdrop it is easier to miss because
the surface is dim. `bg_hue` rides the repeating gradient, so `-0.1` and `0.9` are the same place: a
negative value nudged toward a palette's dark end lands in its **bright** stops instead. On the
default `spectrum` the cosine is genuinely periodic and the wrap is invisible; on the four stop-list
palettes it is the sharpest transition in the gradient. To darken a backdrop, use `bg_bright`.

**`bg_hue` is a *position*, not a colour name.** It is a coordinate in the preset's own gradient,
so the same value does **not** mean the same thing in two presets: a `bg_hue` lifted from another
preset arrives at whatever colour *your* gradient holds at that coordinate, which is usually the
point and occasionally a surprise.

**With `bg_hue_span`, your stops' `at` positions become the sky's vertical layout**
([ADR-0094](adrs/0094-the-backdrop-paints-a-directional-ramp.md)). The backdrop sweeps the segment
`[bg_hue, bg_hue + bg_hue_span]` along a screen axis, so the gradient you already authored *is* the
gradient in the frame — near-black at one stop, a hot band at another — and moving a stop moves that
band up or down the picture. There is no separate placement param, on purpose: positions already
live here, and a second mechanism could disagree with them. The direction, the brightness ramp and
its easing exponent live in
[`presets/README.md`](../presets/README.md#the-directional-ramp--bg_angle-bg_hue_span-bg_shade-bg_shade_end-bg_ramp_gamma),
with a worked dusk example.

**The wrap matters much more to a span than to a point.** A single `bg_hue` that wrapped landed on
one surprising colour; a *segment* that leaves `[0, 1]` paints the palette's far end back into the
frame, so a `bg_hue = 0.8, bg_hue_span = 0.5` sky is hot at **both** ends with a hard seam between
them. Same repeat addressing, much more visible surface.

**One warning specific to shaping a sky through stops:** the `[palette]` is shared with the scene
and with the `[layer]`, so re-spacing stops to bend the backdrop's falloff re-colours the figure
too. When it is only the sky's *response* you want to shape, `bg_ramp_gamma` is the lever that
touches nothing else.

**The band takes a *second* coordinate in the same gradient**
([ADR-0095](adrs/0095-the-backdrop-paints-a-curved-band.md)). The backdrop also paints one soft
curved band of light over that ground, and it colours through the same `[palette]`, the same
`palette_mix` crossfade and the same `saturation` — one colour language, two coordinates:

| Param | Default | What it does |
|-------|---------|--------------|
| `bg_band_hue` | `0.0` | The band's own coordinate in *your* gradient. **Absolute**, not an offset from the ground's — so the arc keeps its colour whatever the ramp underneath it is doing. Cyclic, same wrap. |
| `bg_band_hue_span` | `0.0` | How far that coordinate travels **along** the band (not across it), so one end can brighten toward a galactic core. |

The shape params that place and bow the band live in
[`presets/README.md`](../presets/README.md#the-curved-band--bg_band_amount-bg_band_angle-bg_band_pos-bg_band_width-bg_band_curve-bg_band_hue-bg_band_hue_span),
with a worked Milky Way example. What belongs here is the colour consequence, and it is the one real
authoring constraint the band creates:

**Your palette must now serve three consumers at once — the ground ramp, the band, and the scene
(plus any `[layer]`).** They all read the same stops, and there is no second palette to reach for:
`palette_mix` already owns the A/B pair for preset crossfade, so pinning the band to B would fight
every dissolve and a dissolve would recolour the galaxy on its way past. A dusk palette spent
entirely on a horizon — near-black through deep blue to hot amber, every stop earning its place in
the ramp — has **no room left for a pale arc**, and discovering that after tuning the ground is the
expensive order to discover it in. Budget the stops for both before you tune either.

Where that bites hardest is a band and a ground wanting *opposite* temperatures, which is exactly
the Milky Way case: a cool blue-white arc over a warm horizon needs cool stops the ramp never
visits, which usually means giving the ramp a shorter `bg_hue_span` so it leaves a stretch of the
gradient free, and pointing `bg_band_hue` at that stretch.

**The band's wrap behaves like the ramp's**, for the same reason and with the same surface: the
segment `[bg_band_hue, bg_band_hue + bg_band_hue_span]` is repeat-addressed, so a span leaving
`[0, 1]` paints the palette's far end back into the arc with a hard seam. It is easy to walk into
here, because a band that wraps into the ground's own colours stops reading as a separate object at
all.

---

## Hard bands — `palette_steps` and `palette_contour`

Everything above treats the gradient as a **smooth ramp**: a scene computes a
palette coordinate, the LUT is sampled with linear filtering, and the result is a
continuous blend. `palette_steps` breaks that ramp into hard graphic **bands**, and
`palette_contour` draws a darkened hairline where the picture crosses from one band
to the next ([ADR-0078](adrs/0078-banding-is-a-palette-coordinate-operation.md)).

This is the difference between a field that reads as a *gradient* and one that
reads as *designed* — flat areas of colour with drawn edges, the way a printed
poster or a topographic map does.

| Param | Default | Range that reads | Accepted |
|-------|---------|------------------|----------|
| `palette_steps`   | `0` (off) | **`4`–`12`** | `0` = smooth, integers up to `64` |
| `palette_contour` | `0` (off) | **`0`–`0.5`** | `0` = none, up to `1` |

```toml
[params]
palette_steps   = "6"                     # six flat bands
palette_contour = "0.35"                  # a drawn edge between them
color_span      = "1.4"                   # walk enough gradient that the bands differ
```

### What it does to the coordinate

The **palette coordinate is quantized, not the baked LUT**:

```text
t' = (floor(t * N) + 0.5) / N
```

immediately before the sample, landing on each band's *centre* — so a band takes the
colour the smooth ramp had in the middle of it, not at its edge. The `[palette]`
bake is untouched, which is the whole point: the band count has to be **bindable to
audio**, and re-baking a 256-entry LUT every frame is exactly the work the bake
exists to remove. Bind `palette_steps` to `bar` or latch it on a beat and the
picture re-quantizes per frame at no cost.

Two consequences worth having up front:

- **`palette_steps` is stepped.** It rounds to a whole number, like `kaleido_order`.
  A fractional band count does not step — it leaves every boundary *crawling* across
  the field one frame at a time, which reads as shimmer rather than as colour. An
  eased or bound `palette_steps` still eases; it snaps at each half-integer.
- **Off is the exact identity.** `palette_steps <= 1` and `palette_contour = 0` take
  the unquantized path bit-for-bit, so adding these to an existing preset changes
  nothing until you actually turn them on.

### The ranges, and what is outside them

These were picked off a rendered sweep. Outside them nothing breaks — it stops
being the *graphic* look:

- **`4`–`12` bands** is where the field reads as flat areas with real edges.
- **`16` and up approaches the smooth ramp again.** The bands become narrower than
  the eye separates them at, so the picture converges back on the unbanded
  gradient. A legitimate destination if you want *almost* continuous with a hint of
  structure — not what to bind if you want bands.
- **`palette_contour` up to about `0.5`** is an edge between two colours.
- **`0.8` is a deliberate topographic look**, not an error: past roughly `0.5` the
  dark line stops being an edge and becomes the dominant mark, and the field reads
  as a contour map with colour fill. Arrive there on purpose, not by easing.

### `color_span` decides whether banding is visible at all

`palette_steps` quantizes the **coordinate**, so it can only separate colours the
coordinate actually reaches. A `fragment_field` at its default `color_span` of `0.6`
walks barely half the gradient, and six bands there land on six *neighbouring*
shades — technically banded, visually a texture. Widen the window and the same six
bands become six distinct colours. Since the LUT is repeat-addressed, a
`color_span` above `1` wraps and walks the whole gradient (see the
[cyclic-wrap caution](#the-palette-table) — on the four stop-list palettes the wrap
is the sharpest transition there is, which under banding becomes a visible seam
rather than a soft one).

**So tune `color_span` and `palette_steps` together.** If banding "isn't doing
anything", the span is the first thing to check.

### Where the contour falls — it reads the palette

> [!IMPORTANT]
> **The contour draws only where the two bands it separates are actually
> different colours.** It samples the palette at the two band centres either side
> of the nearest edge, and draws nothing when they come back the same
> ([ADR-0133](adrs/0133-the-band-contour-fires-where-the-ink-changes.md)).

This is what makes the parameter usable on a **limited-ink** look. A two-ink
print is written as *plateaus* — runs of bands holding one colour, the only way
to get flat ink out of `palette_steps` — and a contour drawn at every boundary
*inside* those runs is a grey hairline across flat colour: exactly the shading
such a look is defined by not having. `shape_contourmono`'s twenty steps sit in
five runs, so fifteen of its twenty band edges are white-meets-white or
black-meets-black; without the palette read, its only usable setting would be `0`.

**Nothing on a smooth palette is suppressed.** The test is **equality**, not
similarity: the line is dropped only when the two samples agree to within half
an 8-bit code value, which is below the LUT's own quantization. Two distinct band
centres on a ramp always differ by at least one code value, so on a ramp every
edge draws, at any `palette_steps` and any `color_span`.

Two edges of the rule are worth knowing, because both will read as a surprise:

- **A near-plateau is not a plateau.** A "flat" run built from two stops that
  differ by one code value still contours inside the run. Correct by the stated
  rule, and avoidable: write a plateau as the *same* colour on both stops.
- **A run boundary that does not land on a band boundary still draws at the
  nearest band edge, not at the stop.** The contour lives on the band grid; a
  custom-stop palette whose hard transition falls mid-band gets its line at
  whichever band edge is closest. Place a run boundary on a band edge — the way
  the shipped mono presets write their stops wide of the seam — if you want the
  line where you drew it.

### The scene scoping — banding reaches every scene, contours do not

> [!IMPORTANT]
> **`palette_contour` is inert on `attractor`, `swarm`, `emitter` and the four line
> scenes, and nothing warns you.** The parameter is accepted there because it *is* a
> known name, so no unknown-parameter warning fires. This section is the warning.

A contour needs a **gradient across a fragment** to sit in — its width comes from
`fwidth`, which measures how fast a value changes between neighbouring pixels and
**exists only in a fragment shader**. That is what keeps the hairline a constant
width on screen instead of thick where the field is flat and invisible where it is
steep.

Where the LUT is sampled decides whether that derivative exists at all:

| Scene | Where it samples the LUT | `palette_steps` | `palette_contour` |
|-------|--------------------------|-----------------|-------------------|
| `fragment_field` | per pixel, fragment stage | ✅ | ✅ |
| `reaction_diffusion` | per pixel, fragment stage | ✅ | ✅ |
| `shape_field` | per pixel, fragment stage | ✅ | ✅ |
| `warp_mesh` (the native deposit; a `[milk]` preset draws its own colours) | per pixel, fragment stage | ✅ | ✅ |
| `attractor` | per particle, **vertex** stage | ✅ | ❌ inert |
| `swarm` | per particle, on the CPU | ✅ | ❌ inert |
| `emitter` | per particle, on the CPU | ✅ | ❌ inert |
| `spectrum`, `parametric_curve`, `lsystem`, `star_pattern` | per segment, on the CPU | ✅ | ❌ inert |

A point sprite or a stroke segment carries **one** palette coordinate for its whole
extent, so there is no crossing from one band to the next *within* it to draw a line
at. This is a fact about the pipeline, not a policy someone chose, and it is not
fixable by turning the parameter up.

**So: banding reaches every scene; contours reach the continuous-field scenes.** If
you want drawn edges on a particle or line look, the mark's own geometry is where
they come from — the swarm's `shape`, a line scene's `thickness` — not from
`palette_contour`.

### `palette_contour` under `shape_field`'s two coordinates

`shape_field` can hand the palette either of two figure coordinates
([ADR-0111](adrs/0111-the-shape-field-gains-a-scaled-copy-coordinate.md)): the
normalized **distance** (`coord_mode = "0"`, the default), whose contours are
offsets of the outline, or `r / r_boundary(theta)` (`"1"`), whose contours are
**scaled copies** of it. Both are `0` at the figure's centre and `1` on its
outline, and `palette_contour` works on both.

**The hairline keeps its weight across the switch, and that is worth stating
because the arithmetic suggests otherwise.** The two fields have genuinely
different gradients — the distance rises at `1/inradius` everywhere, while the
radius rises at `1/r_boundary(theta)`, which varies with direction by the shape's
own circumradius-to-inradius ratio. What absorbs that is the contour's own
construction: it is drawn within one **pixel** of a band edge, because the width
comes from `fwidth` of the banded coordinate rather than from a fixed value in
coordinate space. That normalization is exactly what a changing gradient runs
into, so the line comes out the same. Measured on a nine-ring heart at
`palette_contour = "0.75"`, as the darkening the parameter adds:

| | inner rings | outer rings |
|---|---|---|
| `coord_mode = "0"` | 27.3 mean over 492 px | 32.6 mean over 2131 px |
| `coord_mode = "1"` | 29.8 mean over 466 px | 31.3 mean over 1929 px |

The two modes differ by less than the inner and outer rings differ *within*
either one. So do not re-tune `palette_contour` when you switch modes.

**What does change is where the rings are**, which is the whole point of the
second coordinate. Under `"0"` they are evenly spaced in distance, so they hug
the outline's offsets and the innermost ones round off any reflex corner; under
`"1"` they are evenly spaced as fractions of the boundary radius, so they fan out
in proportion — further apart toward a tip, closer toward a valley — and every
one of them is the same figure at a smaller size.

### Banding fights bloom

The bright pass blurs exactly the hard edges banding creates
([ADR-0046](adrs/0046-linear-light-hdr-composite-bloom-tonemap.md)), so a preset cannot have crisp
bands *and* heavy `bloom_amount` at full strength. Pick one, or keep the bloom low
enough that it haloes the bright bands without softening their edges.

### This is not how you get a cyclic look

The cyclic-hue character of a banded reference image is already reachable without
any of this: the LUT is repeat-addressed, so a `color_span` above `1` wraps it and
repeats the whole gradient across the field. `palette_steps` adds only the **hard
edge** between one cycle's colours and the next.

---

## Limited ink — a supported palette class, at the draw seam

A palette quantized into flat plateaus — black, white and one red; two inks on a
cream ground; any set you can count — is a **supported class**, not a trick that
happens to work on some scenes
([ADR-0138](adrs/0138-limited-ink-is-a-supported-palette-class-defined-at-the-draw-seam.md)).
The guarantee is:

> On a scene drawing through an **opacity-preserving seam**, with a fully
> quantized palette, the **scene's own output** contains only colours the palette
> names. Every later stage that introduces intermediate values is enumerated
> below, and each one names the parameter that disables it.

**The guarantee is at the seam and not over the finished frame, and that is not a
hedge.** A seam is a property of a draw call and the engine decides it. The
finished frame is the product of a post chain *you* compose, and a guarantee over
it would have to encode the exemption list as tolerances — a colour count "near
enough" to N, which is a number with no mechanism behind it. Follow the list below
and you get a frame whose colours are the palette's; keep a stage on it and you
know exactly what you traded.

### What the class needs

1. **A fully quantized palette.** Either `palette_steps` on a scene that supports
   it, or `stops` written as pairs that jump — `{ at = 0.1249, … }` then
   `{ at = 0.1251, … }` — so there is no ramp between two inks. A continuous
   gradient is not limited ink and nothing here applies to it.
2. **A scene drawing through an opacity-preserving seam.** These are in the class:

   | system | how it holds the ink |
   |---|---|
   | `fragment_field`, `shape_field`, `reaction_diffusion` | colour is resolved per pixel per frame, so nothing overlaps itself |
   | `shape_collage` | forms are painted **opaque**: one is in front of another rather than added to it |
   | `parametric_curve`, `lsystem`, `star_pattern`, `spectrum` | **only with `stroke_blend = "1"`** — see [`presets/README.md`](../presets/README.md). At the default `0` these draw additively and white over red is pink |

   These are **outside** it. `swarm`, `attractor` and `emitter` are a different
   renderer with nothing equivalent sitting in it. `warp_mesh` colours its light
   at deposit time, so the palette never bands the accumulated field at all
   ([backlog 0146](design-backlog.md)).
3. **A coordinate that lands inside a plateau, not on its edge** — see the LUT
   entry in the table below, which is the one leak that is about *your* numbers
   rather than about a stage you can switch off.

### Every stage that puts a colour in the frame the palette did not name

Two kinds, and the difference decides whether you care. A **mixer** produces a
colour *between* two of the frame's — that is what destroys a plateau and turns
white-over-red into pink. A **remap** moves every colour somewhere else but maps
one to one, so a three-ink frame is still three flat regions; they will not be the
palette's literal RGB, and they will still read as a limited-ink print.

**Mixers — each with its off switch:**

| stage | what it mixes | off |
|---|---|---|
| the backdrop | the scene composites over it with premultiplied alpha, so anything short of full coverage lands between the ink and the backdrop's colour | bind no `bg_*` (the default ground is a black clear); `occlude = 1` is already the default |
| trails | this frame lerped onto the decayed past, so a moving edge leaves a ramp of every value between two inks | `trails = "0"` (the default) |
| kaleidoscope | resampling through the fold, with **linear** filtering, so a texel straddling two plateaus comes back as their average | bind no `kaleido_*` (the stage is inactive without a fold, a radial term or a tile) |
| bloom | a blurred bright-pass added back over the frame — a blur is a mixer by definition | `bloom_amount = "0"` (the default) |
| the internal post grid | **any** active post stage routes the frame through a capped internal grid and presents it with a linear stretch, so the resample mixes neighbours even where the stage itself would not | the same switches as the three stages above: with none active there is no internal grid and no resample |
| `palette_contour` | a soft scalar darken toward black at each band edge — it has no ink of its own, which is exactly [backlog 0140](design-backlog.md)'s subject. Measured there: `shape_contourmono` goes from 9 distinct colours to 684 | `palette_contour = "0"` (the default) |
| the A/B palette crossfade | `palette_mix` between two palettes samples a value in neither of them | declare no `[palette_b]`, or pin `palette_mix` to exactly `0` or `1` |
| the duotone ink pass | every pixel lerped along the paper→ink axis by its luminance, which is a continuum by construction | `ink_amount = "0"` (the default; the pass is not even built) |
| an `over` layer join | the layer is blended into the main scene's composite at `mix`, through `add`, `screen`, `multiply` or `overlay` — **every one of the four is a mixer**, and there is no replacing blend | use `join = "under"` instead, where the layer draws into the *same scene target* through the same seam and stays in the class if it too draws opaque; or declare no `[layer]` |
| a preset dissolve | two whole frames crossfaded while a transition runs | not preset-controllable, and **transient** — it ends when the dissolve does |

**Remaps — they move the colours but do not mix them.** There is no switch list
here, and you do not want one: a remap is one colour to one colour, so three inks
still leave three flat regions.

**Your inks are not one of these.** A stop is sRGB and the engine decodes it when
it bakes the LUT ([ADR-0151](adrs/0151-palette-stops-are-authored-in-srgb-and-converted-at-load.md)),
so the display encode cancels rather than lifts it and a plateau written `#c81423`
comes back `#c81423`. That is exact **below the tonemap knee at linear `0.6`**,
where the curve is the identity — the only residual is the LUT's 8-bit storage,
which costs up to two levels on a very dark channel. Above the knee all three
channels scale by one factor, hue and saturation survive, and the plateau reads as
the same ink. What the table below moves is where a plateau *sits*, never how many
of them there are.

| stage | what it does |
|---|---|
| the tonemap's curve | below the knee at `0.6` it is **exactly the identity**; above it, all three channels scale by one factor, so hue and saturation survive and a plateau stays a plateau |
| `brightness`, `glow` | scale the ink's colour |
| `saturation` | pulls each colour toward its own luminance |
| a scene's own opacity and ground terms | `shape_collage`'s `paper` and `opacity`, a field's `brightness` — each moves where a plateau sits without splitting it |

**And one with no off switch at all, which is the honest end of this list:**

- **The display dither.** The tonemap adds a static triangular dither of at most
  **one encoded level** to each channel before the 8-bit store
  ([ADR-0096](adrs/0096-the-display-write-dithers.md)). It is a pure function of
  pixel coordinates, so it does not shimmer — but a flat mid-tone plateau is
  stored as a ±1/255 speckle rather than as one value, and a colour count over a
  captured PNG will say so. **It vanishes at the rails**: the dither amplitude is
  scaled by the distance to black and to white, so pure `#000000` and pure
  `#ffffff` come through exact and only the inks between them are speckled. There
  is no parameter for it, and there should not be — it is what keeps every
  gradient in the engine from banding.

### The leak that is about your numbers

The palette is baked into a **256-entry LUT sampled with linear filtering**. A
stop pair written to jump — `0.1249` then `0.1251` — is narrower than one LUT
texel, so the whole transition lands *inside* a single texel and any coordinate
falling on that texel reads a blend of the two inks.

That is not a bug and there is no switch for it; it is the reason a quantized
palette wants its **sample coordinate placed at a plateau's centre**. With eight
plateaus, sample at `k/8 + 1/16` rather than at `k/8`. `collage_mono` says exactly
this in its palette header, and it is the one item on this page you fix by writing
better numbers rather than by turning something off.

### Reading the result — measured on the shipped `collage_mono`

There is **no gate** for any of this — no test counts colours in a frame, because a
count with an exemption list is a tolerance with no mechanism behind it
([ADR-0138](adrs/0138-limited-ink-is-a-supported-palette-class-defined-at-the-draw-seam.md)).
Check it by rendering:

```sh
cargo run -p standalone --example shot -- --preset-file presets/<name>.toml --out a.png
```

`collage_mono` at 1280x720, three inks, every mixer above at its off value, comes
back with **592 distinct colours** — and the shape of that number is the whole
point:

| what | in the PNG | why |
|---|---|---|
| the black | `#000000`, exact, 85 606 px | a rail: the dither's amplitude falls to zero at black and at white |
| the paper | `#e7e7e7`, with `#e6e6e6`/`#e8e8e8` beside it | one plateau, speckled by the display dither |
| the red | `#d63131`, with its ±1 neighbours | one plateau, speckled by the display dither |
| everything else | ~580 values, tens of pixels each | the forms' anti-aliased edges — the coverage ramp the class excludes by construction |

**So count plateaus, not values.** Three inks gave three flat regions, which is
what the class promises. Two of the three sit a little off the ink the palette
names — `#ffffff` arrives as `#e7e7e7` and `#d83232` as `#d63131` — because the
remaps above moved them without mixing them, and a raw colour count folds the
dither and the edges in with the inks. A histogram sorted by pixel count separates
all three in one look.

## The line scenes' cosine ramp — what `hue` actually looks like

This is the **default** palette — what a line scene colours through when its
preset declares no `[palette]`. It is the `spectrum` gradient, i.e. the same iq
cosine the engine has always used, and it is **not a hue wheel**. The three
channels are cosines at the same frequency with different phases, which means the
ramp walks a fixed loop through colour space rather than rotating through hues at
an even rate. Guessing a value costs a render round-trip; this table is so you do
not have to.

(On `parametric_curve`, `lsystem` and `star_pattern` it is a default rather than a
ceiling — set a `[palette]` and these swatches stop applying.)

**It doubles as the backdrop's table.** `bg_hue` reads the same gradient, so with no `[palette]`
declared these swatches are what a `bg_hue` value looks like — and, for the same reason, they stop
applying to `bg_hue` the moment a preset declares one. There is deliberately no second table for the
backdrop: an independently-measured colour table drifted from the one in the code once already, and
every name in it was wrong (design-backlog 0014).

Swatches are the ramp at `brightness = 1`, sRGB, computed from the shader's own
arithmetic and confirmed against rendered strokes:

| `hue` | swatch | reads as |
|-------|--------|----------|
| `0.00` | `#F44667` | coral rose |
| `0.05` | `#E6188B` | cerise |
| `0.10` | `#D30DAB` | magenta |
| `0.15` | `#BC3EC6` | orchid |
| `0.20` | `#9F67DC` | lavender-violet |
| `0.25` | `#7D8BEC` | periwinkle |
| `0.30` | `#57ABF8` | cornflower blue |
| `0.35` | `#2BC6FE` | sky |
| `0.40` | `#00DCFF` | cyan |
| `0.45` | `#2BECFA` | aqua |
| `0.50` | `#57F8EF` | turquoise |
| `0.55` | `#7DFEDF` | pale aquamarine |
| `0.60` | `#9FFFCA` | mint |
| `0.65` | `#BCFAB1` | pale green |
| `0.70` | `#D3EF92` | lime cream |
| `0.75` | `#E6DF6F` | pale yellow |
| `0.80` | `#F4CA46` | gold |
| `0.85` | `#FCB118` | amber |
| `0.90` | `#FF920D` | orange |
| `0.95` | `#FC6F3E` | vermilion |

Reading it:

- **It is cyclic**, unlike `ember` / `ice` / `mono` / `aurora`: `hue = 1.0` is
  `hue = 0.0`, so a binding that sweeps `hue` continuously never hits a seam.
- **The ends of the loop are the warm half.** Red through orange lives at the
  wrap (`0.90`–`0.05`); there is no deep-red-to-black end, because the ramp never
  leaves full saturation. If you want a dark look here, that is `brightness`, not
  `hue`.
- **The cool half is over-represented.** Roughly `0.30`–`0.65` is blue through
  green, a third of the range for what a hue wheel gives a quarter of. Fine
  distinctions between two blues need smaller steps than between two ambers.
- **`brightness` scales the ramp; overlap does not preserve it.** The line
  renderer blends **additively**, so where strokes cross, channels sum and the
  crossing reads paler than the swatch. A dense figure at high `brightness`
  therefore looks less saturated than its `hue` says. Lower `brightness` (or
  `thickness`) rather than chasing the colour with `hue`.

  **It does not approach *white*, though.** The composite carries linear light
  past 1.0 and the engine tonemap rolls it off by scaling all three channels by
  the *same* factor, taken from the brightest one — so the ratios between R, G
  and B survive the roll-off exactly and a saturated crossing keeps its hue
  instead of climbing toward white a channel at a time. A dense figure reads
  paler at its crossings, because the sum genuinely is less saturated; it does
  not go colourless.

---

## A/B palette crossfade — `[palette_b]` + `palette_mix`

Declare a second palette and bind `palette_mix` (`0..1`) to crossfade between the
two per frame — smooth, audio-driven palette *selection* with no flicker:

```toml
[palette]
name = "ember"

[palette_b]
name = "ice"

[params]
palette_mix = "bar"        # warm on the beat, cooling across the phase
```

`[palette_b]` takes the same shape as `[palette]` (a `name` or `stops`).
`palette_mix = 0` is exactly palette A alone; `1` is palette B; between, the two
LUTs are lerped. With no `[palette_b]`, `palette_mix` is a no-op.

---

## One palette serves both layers — the `[layer]` rule

A preset that composes a second scene (`[layer]`, see `presets/README.md`) has
**no `[layer.palette]`**, deliberately: the preset's single `[palette]` (and
`[palette_b]` pair) is baked once and handed to the main scene, the layer's
scene and the backdrop alike. Two colour languages in one frame read as two
presets stacked rather than one world, and a second bake would double the LUT
work for a coherence loss, not a gain — the shared gradient is precisely what
makes two layers *one* look. The omission is deliberate
([ADR-0090](adrs/0090-a-preset-composes-two-scene-layers.md)).

What still differs per layer is how each scene **samples** that shared
gradient: the layer has its own `hue`, `saturation`, `color_span` /
`hue_spread`, `palette_mix` and friends in `[layer.params]`, so the two layers
can sit at different points of one gradient — a dim ground at the palette's
floor under a bright figure at its crest — without leaving the family.

---

## Dark on light — the two-tone route

Everything else in this document adds light. **A `multiply` layer takes it
away** ([ADR-0106](adrs/0106-two-tone-graphics-come-from-a-multiply-layer.md)), and
that is the whole route to a dark figure on a light ground.

```toml
system = "fragment_field"     # the LIGHT GROUND: flat, bright, full-frame
name   = "Two Tone"

[palette]
stops = [
  { at = 0.0, color = "#fff4e0" },
  { at = 1.0, color = "#ffd9a0" },
]

[params]
color_span   = "0"            # one coordinate = one flat tone across the frame
color_center = "0.2"
glow         = "1.1"

[layer]                       # the DARK FIGURE: it subtracts, it does not add
system = "swarm"
join   = "over"
blend  = "multiply"
mix    = "1"

[layer.params]
brightness = "0.15"           # <- the darkening operand. LOW is DARK.
size       = "5.0"
```

The rule under it is one sentence: **`multiply` darkens by the layer's colour,
within the layer's coverage.** Both halves matter, and the second is the one
that decides which scene you reach for.

### `brightness` runs backwards here, and nothing warns

In every additive preset you have ever written, `brightness` up is *more*
visible. In a `multiply` layer it is the **darkening operand**: the blend
un-premultiplies the layer's colour and multiplies the ground by it, so
`brightness = 0` is an opaque black mark and a high `brightness` clamps to white
and does nothing at all. This is a parameter whose meaning is inverted by a
setting in a *different* table, and no gate or warning can see it.

### Which scene: it is a question of footprint, not of capability

| layer scene | coverage | what a `multiply` slot gives you |
|---|---|---|
| `fragment_field`, `reaction_diffusion`, `shape_field` | every pixel (alpha = `occlude`, 1 by default) | the whole frame is darkened — a graded wash, a banded gradient, a figure *and* its surround |
| `swarm`, `emitter` | inside each mark only (alpha = the mark's geometric falloff) | discrete dark marks on an untouched light ground |

Both routes reach a dark figure, and the particle one reaches **darker**: a frozen
swarm at `brightness = 0` takes a light chain from display luma 174.1 to **0.9**,
where the field route bottoms out at **18.9**. Pick by the shape you want, not by
what you think can darken.

> **If you have read [ADR-0106](adrs/0106-two-tone-graphics-come-from-a-multiply-layer.md)
> or design-backlog 0069, they say a particle layer cannot darken at all.** That
> is wrong and this table is the correction. The claim was that a particle's
> alpha *is* its brightness; in fact `swarm.rs` emits `vec4(color * g, g)` where
> `g` is the mark's **geometric** falloff and is independent of its colour, and
> `layer_blend.rs` un-premultiplies before taking the mode. `core/tests/layer.rs`
> carries the measurement.

### The light ground must come from the chain, never from `bg_*`

The tempting version of the recipe above is a bright **backdrop** with a dark
layer over it. It does not work, and the reason is structural rather than a
tuning problem: **the backdrop is not in the chain's input** (`post.rs`) — it is
composited *underneath* the finished chain, so no blend mode ever sees it.

Both ways it can go:

| what you set | what you get |
|---|---|
| `occlude = 1` (default), any blend | the frame is **byte-identical** to the same preset over a black backdrop. The backdrop is not darkened, it is gone — coverage held it out |
| `occlude = 0` | the backdrop is added *after* the blend, so it is a **floor**: the same multiply layer that reaches luma 18.9 over black reaches only 171.3 over a lit sky (the sky alone reads 196.9) |

So `bg_bright` cannot be the white paper of a two-tone graphic. Make the ground
a flat, bright, full-frame scene in the **chain** — `color_span = 0` pins a field
to one tone — and spend the `[layer]` slot on the figure.

### Two costs worth knowing before you commit to this

- **It uses the preset's only `[layer]`.** A preset carries one layer table, so a
  two-tone graphic cannot also carry a second figure as counterpoint.
- **"Black" is dark grey unless the marks are.** The field route's floor is luma
  ~18.5 with bloom and the tonemap both live; the particle route at
  `brightness = 0` gets to 0.9. If you need a true black, that difference is the
  lever.

---

## Flat colour on `shape_collage` — stay under the knee

`shape_collage` is the one system that draws a **graphic** rather than light: flat
opaque elements on their own paper, hard-edged, in painter order. Everything that
makes that look work comes from a single palette constraint, and it comes off the
tonemap rather than off any parameter.

**Keep every element colour's brightest channel at or under linear `0.6` — sRGB
byte `0xcb` in the hex you write.**

That is [ADR-0046](adrs/0046-linear-light-hdr-composite-bloom-tonemap.md)'s
`KNEE`, and below it the tonemap curve is **exactly the identity** — so the fill
survives the whole post chain **unshaded**: the value the element wrote is the
value that leaves the tonemap, to within the 8-bit write's own rounding. Bloom's
threshold sits *above* that same knee, so a canvas living under it also gets no
halo and its edges stay hard. **One constraint, both properties, and neither is a
parameter you can reach for instead.** Author a brighter palette and you lose the
flat fill and the hard edge together, with nothing to tell you why.

> [!IMPORTANT]
> **Under the knee the hex you typed *is* the hex on screen.** A stop is sRGB and
> is decoded once at load — the same mapping stated at the top of this file — so
> the decode and the display encode are inverses and the identity curve between
> them changes nothing. On `collage_suprematist`: `#494949` renders `#494949`,
> `#c24f63` renders `#c24f63`. The residual is the LUT's own 8-bit linear
> storage, up to two levels on a very dark channel and nothing on a bright one.
> What the knee buys on top of that is that no element is shaded, tinted or
> haloed on the way — which is the whole of the flat-graphic look. **The paper is
> the exception, because it is the one plateau deliberately above the knee**: at
> linear 0.851 the tonemap is no longer the identity, so `#eeece5` renders
> `#e2e0da`. Author the elements by the hex; author the paper by the result.

**The paper is the deliberate exception.** The curve's own table records
`f(1.0) = 0.800` and 1.0 is asymptotically unreachable, so pure white paper does
not exist: `#ffffff` presents at about 80 % and anything near it costs a linear
emission well over 1, which re-enters bloom's threshold and forfeits the free hard
edge. **Off-white is the affordable ground**, and both of the reference canvases
this system was built from are off-white anyway. `#e6e3d8` and `#d9d5c8` are what
the two shipped presets use.

**Author the palette as plateaus, not as a gradient.** An element takes *one*
colour off the LUT at its coordinate, so a smooth ramp shades every element by
wherever it happens to sit. A pair of stops a ten-thousandth apart is a hard
transition:

```toml
[palette]
stops = [
  { at = 0.0000, color = "#1a1a1a" },   # band 0 — an element at coord 0.0625
  { at = 0.1249, color = "#1a1a1a" },
  { at = 0.1251, color = "#8c1c24" },   # band 1 — an element at coord 0.1875
  { at = 0.2499, color = "#8c1c24" },
  # … six more, the last one the paper …
]
```

**Eight bands is not arbitrary.** The layout grammar draws each element's colour
from eight evenly spaced coordinates at band centres (`k/8 + 1/16`) and reserves
the last for the paper, so an eight-plateau palette resolves every generated
element exactly and no element ever draws the ground's own colour and vanishes.
Other counts work; their elements just land wherever those eight coordinates fall.

`presets/collage_suprematist.toml` and `presets/collage_onwhite.toml` are both
built this way and carry the arithmetic in their headers.

---

## Worked example — a cohesive warm fragment field

```toml
system = "fragment_field"
name   = "Ember Field"

[palette]
stops = [
  { at = 0.0, color = "#1a0500" },
  { at = 0.55, color = "#c8320a" },
  { at = 1.0, color = "#ffd070" },
]

[params]
warp        = "0.4 + clamp(bass * 12, 0, 1.4)"
glow        = "0.7 + clamp((bass + mid) * 6, 0, 0.8)"
color_span  = "0.3"                    # narrow = one cohesive warm mood
color_center= "0.15 + clamp(treb * 3, 0, 0.4)"
saturation  = "0.85 + clamp(mid * 3, 0, 0.3)"
```

Low `color_span` over a warm custom gradient holds the whole field in one family;
`color_center` and `saturation` breathe with the treble and mids.

---

## Related documents

- [ADR-0021 — Shared palette system](adrs/0021-shared-palette-system.md): the
  baked-LUT decision and the rejected alternatives.
- [ADR-0078 — Banding is a palette coordinate operation](adrs/0078-banding-is-a-palette-coordinate-operation.md):
  why `palette_steps` quantizes the coordinate rather than the bake, and why an RGB
  posterize post stage was rejected.
- [Plan 0020 — Shared palette system](plans/done/0020-shared-palette-system.md): the
  phased implementation.
- `docs/presets.md`: the main preset authoring guide (systems, expressions, file
  layout).
- `presets/README.md`: the in-repo authoring note, including the per-system
  parameter roster.
