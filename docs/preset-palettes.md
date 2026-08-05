# Preset colour: the palette surface

Presets control colour through a **shared palette** (ADR-0021, Plan 0020). A
palette is a gradient baked once at load into a lookup table that the four
**shader-coloured** scenes sample:

- `fragment_field`
- `swarm`
- `reaction_diffusion`
- `attractor`

**All four line scenes** sample the same LUT too, but **on the CPU, once per
segment** rather than per pixel or per particle:

- `spectrum` — see [Spectrum — colour along the frequency axis](#spectrum--colour-along-the-frequency-axis).
- `parametric_curve`, `lsystem` and `star_pattern` joined it in Plan 0054
  ([ADR-0059](adrs/0059-line-scenes-colour-along-their-generator-axis.md)) — see
  [The line scenes — colour along the generator's axis](#the-line-scenes--colour-along-the-generators-axis).

So **every** scene reaches `[palette]`. Before Plan 0054 those three did not, and
`hue` was the only colour control they had; a note anywhere still saying a
`[palette]` table is inert on a line scene is stale.

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
array of `0..1` floats:

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
last good preset (it never crashes). Values `0..1` are used directly (no gamma /
perceptual management — that is a future addition).

**A LUT value is a linear-light colour, and since Plan 0045 it stays one all the
way to the display.** A stop's `0..1` components are taken as linear
coefficients; the scene multiplies them by whatever luminance it has and writes
the result into a floating-point composite that is free to exceed 1.0, and the
engine tonemap at the end of the frame is the only place anything is compressed.
So what a preset does to a palette colour is now *arithmetic on light* rather
than on bytes, and two consequences follow for authoring: a stop is never
silently clipped on its way through the composite, and the roll-off that finally
applies is hue-preserving (see the ramp notes below). What has **not** changed is
the mapping from a hex stop to a coefficient — `#ff5500` is still read as
`(1.0, 0.33, 0.0)` with no sRGB decode, so the gradients look exactly as they
did. Deciding whether that mapping *should* decode is the perceptual work
ADR-0021 deferred; linear light is the structure that makes it possible, not the
change itself.

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

### Shared (every palette-coloured scene)

| Param | Default | What it does |
|-------|---------|--------------|
| `hue`        | `0.0` | Rotates the gradient sample coordinate (the pre-existing hue knob). |
| `saturation` | `1.0` | Scales chroma toward luma. `1` unchanged, `0` grayscale, `>1` oversaturated. |
| `palette_mix`| `0.0` | A/B crossfade position (see below); `0` = palette A. |

### Fragment field & reaction-diffusion — where the field sits in the gradient

| Param | Default | What it does |
|-------|---------|--------------|
| `color_span`   | `0.6` (fragment) / `0.85` (RD) | Multiplies the scene's field level to set how much of the gradient it spans. **Low = a cohesive single-family mood**; high = a wide sweep. |
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

Plan 0054 ([ADR-0059](adrs/0059-line-scenes-colour-along-their-generator-axis.md))
gave `parametric_curve`, `lsystem` and `star_pattern` the same surface `spectrum`
had: `[palette]`, `[palette_b]`, `palette_mix`, `hue_spread` and `saturation`,
sampled on the CPU per segment. The arithmetic is `spectrum`'s exactly —

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
  empty is the rosette's *interior* — at `star_rosette`'s 12-fold / 20° the
  strokes live between radius 0.54 and 0.90, so the inner 60% of the disc is bare,
  and `star_lantern`'s 55° variant empties 87% of it. That is a generator
  question, still open; the ramp comes alive by itself the day a construction puts
  segments at different radii.

---

### The backdrop — `bg_hue`, and the one surface still outside the palette

> **This section describes today's behaviour and is scheduled to change.**
> [ADR-0086](adrs/0086-the-backdrop-colours-through-the-preset-palette.md) /
> [Plan 0072](plans/0072-the-backdrop-joins-the-palette.md) bring the backdrop into the palette;
> until that lands, what is below is what renders.

The background pre-pass (`bg_hue`, `bg_bright`, `bg_vignette` — the full roster is in
[`presets/README.md`](../presets/README.md)) draws a tinted gradient *under* the scene. **It does
not read your `[palette]`.** `core/src/render/background.rs` carries its own copy of the default
cosine and binds no LUT, so:

- **`bg_hue` always walks the `spectrum` ramp**, whatever palette the preset declares. An `ember`
  preset draws an ember figure over a spectrum-cosine sky.
- **`saturation` and `palette_mix` do not reach the backdrop.** An A/B crossfade moves the figure
  and leaves the sky where it was.
- **`bg_hue` is therefore readable off the table below**, because that table *is* this ramp — the
  backdrop's cosine and the line scenes' default are the same `d = (0.10, 0.42, 0.62)`. `bg_hue`
  `0.30` is cornflower blue, `0.45` aqua, `0.85` amber, in every preset in the library.

This is the only place in the colour surface where a preset's declared gradient does not apply, and
it is an accident of ordering rather than a decision — the pass predates the shared palette module
and nothing failed when the rest converged onto it.

---

## The line scenes' cosine ramp — what `hue` actually looks like

This is the **default** palette — what a line scene colours through when its
preset declares no `[palette]`. It is the `spectrum` gradient, i.e. the same iq
cosine the engine has always used, and it is **not a hue wheel**. The three
channels are cosines at the same frequency with different phases, which means the
ramp walks a fixed loop through colour space rather than rotating through hues at
an even rate. Guessing a value costs a render round-trip; this table is so you do
not have to.

(Until Plan 0054 this was the *only* thing `parametric_curve`, `lsystem` and
`star_pattern` could colour through. It is now their default rather than their
ceiling — set a `[palette]` and these swatches stop applying.)

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

  **Since Plan 0045 it no longer approaches *white*.** The composite carries
  linear light past 1.0 and the engine tonemap rolls it off by scaling all three
  channels by the *same* factor, taken from the brightest one — so the ratios
  between R, G and B survive the roll-off exactly and a saturated crossing keeps
  its hue instead of climbing toward white a channel at a time. A dense figure
  still reads paler at its crossings, because the sum genuinely is less
  saturated; it no longer goes colourless.

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
- [Plan 0020 — Shared palette system](plans/done/0020-shared-palette-system.md): the
  phased implementation.
- `docs/presets.md`: the main preset authoring guide (systems, expressions, file
  layout).
- `presets/README.md`: the in-repo authoring note, including the per-system
  parameter roster.
