# Preset colour: the palette surface

Presets control colour through a **shared palette** (ADR-0021, Plan 0020). A
palette is a gradient baked once at load into a lookup table that the four
**shader-coloured** scenes sample:

- `fragment_field`
- `swarm`
- `reaction_diffusion`
- `attractor`

The line scenes (`parametric_curve` / `lsystem` / `star_pattern`) colour through
their own cosine `hue` offset and do **not** use this palette surface.

A preset that declares no `[palette]` gets the default **`spectrum`** palette —
the exact iq cosine the scenes used before this system existed — so every shipped
preset renders unchanged. (The one exception: `reaction_diffusion` previously used
a slightly different cosine and was unified onto `spectrum`, so the coral presets'
colours shifted; they are being re-authored to exploit palettes.)

This is the authoring counterpart to
[ADR-0021](adrs/0021-shared-palette-system.md) and
[Plan 0020](plans/0020-shared-palette-system.md). It documents **colour only** —
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

The gradient **repeats** past its ends (a colour coordinate that wraps past `1`
comes back around at `0`), matching the `spectrum` cosine's periodic hue wheel.

---

## Bindable colour parameters

Everything that *modulates* the gradient is a normal `[params]` expression over
the audio vocabulary (`bass mid treb onset beat bar time` …) — only the gradient
*shape* above is static config. Defaults reproduce each scene's prior look.

### Shared (all four scenes)

| Param | Default | What it does |
|-------|---------|--------------|
| `hue`        | `0.0` | Rotates the gradient sample coordinate (the pre-existing hue knob). |
| `saturation` | `1.0` | Scales chroma toward luma. `1` unchanged, `0` grayscale, `>1` oversaturated. |
| `palette_mix`| `0.0` | A/B crossfade position (see below); `0` = palette A. |

### Fragment field & reaction-diffusion — where the field sits in the gradient

| Param | Default | What it does |
|-------|---------|--------------|
| `color_span`   | `0.6` (fragment) / `0.85` (RD) | How much of the gradient the field spans. **Low = a cohesive single-family mood**; high = a wide sweep. |
| `color_center` | `0.0` | Where that window sits in the gradient. Slide it (e.g. `0.1 + treb*0.2`) to move the tonal centre. |

A warm, cohesive field: a warm palette + a low `color_span`.

### Swarm & attractor — the per-particle hue band

| Param | Default | What it does |
|-------|---------|--------------|
| `hue_spread` | `1.0` (swarm) / `0.15` (attractor) | Width of the per-particle hue band. **Low = a coherent, single-family cloud**; `1.0` on the swarm is the full-wheel rainbow. |
| `hue_center` | `0.5` (swarm) / `0.075` (attractor) | Centre of that band in the gradient. Two presets that differ only here render as different colours. |

Particle hues occupy `hue_center + (particle_seed - 0.5) * hue_spread`.

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
- [Plan 0020 — Shared palette system](plans/0020-shared-palette-system.md): the
  phased implementation.
- `docs/presets.md`: the main preset authoring guide (systems, expressions, file
  layout).
- `presets/README.md`: the in-repo authoring note, including the per-system
  parameter roster.
