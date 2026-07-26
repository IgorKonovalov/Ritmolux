# Preset authoring

Presets are TOML files (ADR-0002). A preset names a built-in **system** and binds
that system's **named parameters** to **expression strings** over the audio
analysis. Line-art systems (ADR-0007) additionally take a declarative
**structural-config table** (`[curve]` / `[generator]`) that is *not* expressions.

Files here are the curated set embedded into the binary and seeded into the
per-user preset directory on first run (Plan 0007). Seeding is **write-if-absent**,
so editing a file *here* is invisible to a frontend already reading its seeded
copy. Point both frontends at this folder instead and edit in place — the app
hot-reloads within ~150 ms, no rebuild:

```sh
$env:LMV_PRESET_DIR = "./presets"; cargo run -p standalone --release   # PowerShell
cargo run -p standalone --example shot -- --preset-file presets/<name>.toml --out a.png
```

See [`../docs/presets.md`](../docs/presets.md#a-custom-preset-folder-lmv_preset_dir)
and [`../docs/capturing.md`](../docs/capturing.md#editing-presets-live).

## Skeleton

```toml
system = "parametric_curve"   # which built-in system (see the table below)
name   = "My Rose"            # optional; defaults to the system name

[curve]                       # or [generator] — structural config (line systems)
family = "maurer_rose"

[params]                      # named parameter -> expression string
n     = "6"
scale = "0.7 + bass * 0.4"
hue   = "0.5 + time * 0.02 + treb * 0.3"
```

## The expression language

Each `[params]` value is a pure expression evaluated every frame. A malformed
expression (or structural config) makes the whole preset fail to load with a
surfaced error — the engine keeps the last good preset, never crashes (NFR 10).

- **Variables:** `bass mid treb onset beat bar time tempo novelty`
  (`beat` is 0/1; `bar` is the 0..1 beat phase; `time` is seconds; `tempo` is
  **BPM**, not a 0..1 band; `novelty` is an experimental track-change transient).
- **Functions:** `sin cos abs floor sqrt min max pow mod clamp lerp smoothstep select`.
- **Constants:** `pi`, `tau`.
- **Comparisons:** `> < >= <= == !=`, each yielding `1`/`0`, plus
  `select(cond, x, y)`. No boolean operators — `min` is and, `max` is or,
  `1 - c` is not.

Full grammar reference: [`../docs/presets.md`](../docs/presets.md#the-expression-language).

## Systems and their named parameters

| System            | Named `[params]`                                                         |
|-------------------|--------------------------------------------------------------------------|
| `fragment_field`  | `warp` `hue` `zoom` `glow` `flash` · `pan_x` `pan_y` · `saturation` `color_span` `color_center` `palette_mix` |
| `swarm`           | `force` `spin` `burst` `hue` `brightness` `size` · `zoom` `pan_x` `pan_y` · `saturation` `hue_spread` `hue_center` `palette_mix` |
| `parametric_curve`| `n` `d` `phase` `samples` `thickness` `hue` `spin` `scale` `radial_offset` `brightness` `draw_progress` · `zoom` `pan_x` `pan_y` `mirror_order` `mirror_reflect` |
| `lsystem`         | `visible_depth` `rotation` `hue` `draw_progress` `thickness` `scale` `brightness` · `zoom` `pan_x` `pan_y` `mirror_order` `mirror_reflect` |
| `star_pattern`    | `variant` `rotation` `hue` `draw_progress` `thickness` `scale` `brightness` · `zoom` `pan_x` `pan_y` `mirror_order` `mirror_reflect` |
| `reaction_diffusion` | `feed` `kill` `flow` `inject` `hue` `contour` `hatch` `glow` · `zoom` `pan_x` `pan_y` · `saturation` `color_span` `color_center` `palette_mix` |
| `attractor`       | `a` `b` `c` `d` `size` `hue` `fade` `reseed` · `zoom` `pan_x` `pan_y` · `saturation` `hue_spread` `hue_center` `palette_mix` |

Unbound parameters fall back to each system's defaults. An **unknown** parameter
name is reported as a load-time warning naming the param and the system — the
preset still loads and its other bindings apply (ADR-0020). The params after the
first `·` are the shared **view transform** and
line-**mirror** controls (Plan 0018) — see [Engine-wide controls](#engine-wide-controls-plan-0018);
the trailing group on the four shader-coloured scenes is the shared **palette**
colour surface (Plan 0020) — see [Colour — the palette surface](#colour--the-palette-surface-plan-0020).
Every system additionally accepts the engine-stage params `bg_*`, `trails`,
`kaleido_*`, and the final `ink_*`/`paper_*` remap documented there.

### Attractor detail sharpness (Plan 0027)

The `attractor` accumulates its trails into an offscreen grid sized to what it is
drawing into, up to 2560x1440 — so at 1080p the filaments are pixel-crisp rather
than upscaled from the old fixed 640x360 grid. Nothing to bind: it follows the
window. Two authoring consequences: `size` now reads *finer* at high resolution
(a value tuned when the field was soft may look thin — nudge it up), and turning
`trails` or `kaleido_*` on drops the accumulation to those stages' fixed 1280x720
grid, so a preset that stacks them is softer than the attractor alone.

### Line-art parameter notes (Plan 0010)

- `thickness` — stroke weight (roughly 1–5); scaled to a projector-friendly glow.
- `hue` — offset into the shared cosine palette (add `time * k` for a slow drift).
- `scale` — overall size in the frame; `draw_progress` in `0..1` reveals the
  figure from the start (a line-draw-on; ride it on `bar` for a per-beat redraw).
- `parametric_curve`: `n`/`d` are the rose parameters, `spin` is angular velocity
  (rotation = `spin * time`), `samples` the chord count (clamped to the segment
  cap). Two **shape** params morph the curve itself (default `0.0`, a no-op):
  `phase` (radians added *inside* the sine, `r = sin(n*theta + phase) + radial_offset`)
  reshapes the petals as it advances — distinct from `spin`, which rotates the
  finished figure; `radial_offset` adds to the radius, opening the rose off-origin
  into spiral/annular/rosette forms. Both make `r` exceed `[-1, 1]` when nonzero —
  large values push geometry past the frame (the intended blowout; the renderer
  clips). Bind them to `bass`/`bar`/`beat` for an audio-driven shape morph.
- `lsystem`: `visible_depth` picks which precomputed iteration is shown — drive
  it off a band/beat to *grow* the structure (e.g. `4 + floor(2 * bass)`);
  `rotation` is an angle in radians, so multiply by `time` yourself.
- `star_pattern`: `variant` selects one of the precomputed contact-angle variants
  (0..2, clamped) — swap it on a beat for a structural accent
  (e.g. `floor(2.99 * beat)`); `rotation` is an angle in radians.

## Engine-wide controls (Plan 0018)

These sit in the **render composite** (ADR-0018), not in one scene, so they are
audio-bindable like any other param. All default to *off/identity*, so a preset
that binds none renders exactly as before.

### Shared view transform — `zoom`, `pan_x`, `pan_y`

A camera zoom about the frame centre, then a pan. Applied by **every** coloured
scene: `fragment_field`, `swarm`, the three line systems (`parametric_curve` /
`lsystem` / `star_pattern`), and — since Plan 0025 — `reaction_diffusion` and
`attractor`.

- On the **line**, **swarm**, and **attractor** scenes, `zoom > 1` moves the camera
  *in* (geometry bigger); `zoom = 1` is no zoom. `pan_*` shift in world units. Try
  `zoom = "1 + bass * 0.6"` for a kick-driven pump.
- On the **fragment field** and **reaction-diffusion**, `zoom` scales the *sample*
  coordinates of the present pass, so a *higher* `zoom` shows *more* of the field
  (the opposite sense to the geometry scenes, kept for the shipped fragment
  presets); `pan_x` / `pan_y` slide the sampled window.

### Background pass — `bg_hue`, `bg_bright`, `bg_vignette`

An audio-tintable gradient + vignette backdrop drawn *before* the scene, engine-
wide. `bg_bright = 0` (the default) is a black backdrop; raise it to reveal the
gradient. `bg_hue` offsets into the shared cosine palette; `bg_vignette` (0..1)
darkens the corners. Visible wherever the scene leaves the frame unpainted: behind
the **sparse** scenes (lines, swarm, attractor) where the gaps show through, and —
since Plan 0025 — in the **reaction-diffusion** field's voids (both scenes now
composite over the backdrop instead of presenting opaque). The **fragment field**
is the one full-screen scene that still draws opaquely, so `bg_*` has no visible
effect there.

### Geometry mirror (line systems) — `mirror_order`, `mirror_reflect`

Replicates a line scene's segments under N-fold rotational symmetry to build a
true geometric fractal. `mirror_order` is the fold count (rounds, clamped to
`1..=24`; `1` = no mirror). `mirror_reflect >= 0.5` adds a reflected copy per
sector (dihedral). Distinct from the screen-space `kaleido_*` below: this folds
the *geometry*, that folds the finished *pixels*. High order on a dense curve is
capped at the segment limit and the drop is always surfaced — never a silent cut.
A structural overflow (an L-system depth) is reported when the preset loads; a
*live* one, where a bound expression drives `mirror_order` past the cap while the
scene runs, is reported on stderr once when it starts and once when it clears.

### Feedback trails — `trails`

Routes the composited frame through a fade-and-accumulate feedback (max-decay), so
moving shapes leave light trails. `trails = 0` (default) is off; `0 < trails < 1`
sets the per-frame decay (higher = longer trails). Best on a scene with real
motion (a spinning curve, a drifting swarm).

### Screen-space kaleidoscope — `kaleido_order`, `kaleido_angle`

Folds the finished frame into `kaleido_order` mirrored wedges before present.
`kaleido_order < 2` (default) is a passthrough; `>= 2` folds (clamped to 48).
`kaleido_angle` (radians) rotates the fold — ride it on `time` for a turning
kaleidoscope. Works on any scene.

### Ink on paper — `ink_amount`, `paper_*`, `ink_*` (Plan 0027)

The **last** stage before present (ADR-0028), and the only engine-wide one that is
not per-preset: it remaps the one finished frame, which during a cross-preset
dissolve is the *blended* frame of both presets (ADR-0032). Your `ink_*`/`paper_*`
values therefore crossfade with the dissolve rather than snapping — a switch from a
white-paper preset into a black-ink one travels between the two poles. It reads each
pixel's brightness as an *ink density* and repaints the frame between two colours:
**paper** where the frame was dark, **ink** where it was bright. The defaults are white paper and
black ink, so `ink_amount = "1"` alone gives **black marks on a white field** —
the "ink on paper" look. Works on **any** scene, sparse or full-screen, because it
operates on the finished frame rather than a scene's pipeline.

This is the one control that reaches a *dark-on-light* look at all. The scenes draw
**additively** (a lightening model), so a dark stroke colour adds nothing and a
light `bg_bright` just washes the strokes out — no combination of `bg_*` and
palette colours gets there. The remap inverts the tone at the end instead.

| Param | Default | Meaning |
|---|---|---|
| `ink_amount` | `0` | `0` = off (passthrough). `1` = full remap. Bindable — try `"beat"` to snap into ink on each hit. Read the note below before resting it between the two. |
| `paper_hue` | `0` | Paper (dark-input) hue, into the HSV wheel; wraps, so it can sweep freely. |
| `paper_sat` | `0` | Paper saturation. `0` = neutral. |
| `paper_bright` | `1` | Paper brightness. `1` = white. |
| `ink_hue` | `0` | Ink (bright-input) hue. |
| `ink_sat` | `0` | Ink saturation. `0` = neutral. |
| `ink_bright` | `0` | Ink brightness. `0` = black. |

```toml
[params]
ink_amount   = "1"          # black on white - nothing else needed

# ...or a colored duotone: indigo ink on cream paper
paper_hue    = "0.11"
paper_sat    = "0.13"
paper_bright = "0.97"
ink_hue      = "0.68"
ink_sat      = "0.85"
ink_bright   = "0.30"
```

**A partial `ink_amount` is a transition, not a resting value.** The stage
crossfades between the *remapped* frame and the untouched source, and the source
is a near-black frame — so at `0.5` the paper is half black. That reads as a dirty
page rather than a faint drawing, and the strokes lose contrast at the same time.
Bind `ink_amount` when you want to *travel* between the glowing and the drawn look
(`"beat"`, or an eased ramp via `[smoothing]`, both of which pass through the
middle rather than sit in it); pick `0` or `1` when you want to stay somewhere. A
sparse, faint drawing comes from the scene — a finer `size`, a shorter `fade` —
not from a half-strength remap.

**In ink mode a scene's palette collapses to the duotone.** The remap keys on
*luminance* only, so the hue a `[palette]` produced is discarded and every pixel
lands somewhere on the paper→ink ramp. The palette still matters — it shapes which
parts of the frame are bright, and therefore which parts become ink — but the two
colours you actually see come from `paper_*`/`ink_*`. Reach for `saturation` and
the palette to sculpt *contrast* in an ink preset, not colour.

The HUD, the browse overlay, and the diagnostics overlay draw *after* this stage,
so they are never inverted.

## Colour — the palette surface (Plan 0020)

The four **shader-coloured** scenes (`fragment_field`, `swarm`,
`reaction_diffusion`, `attractor`) colour through a shared **palette** (ADR-0021):
a gradient — a built-in `name` or custom `stops` — baked into a lookup table the
scene samples. An optional top-level `[palette]` table picks it; a `[palette_b]` +
bindable `palette_mix` crossfades between two. Colour modulation (`saturation`,
`color_span`/`color_center`, `hue_spread`/`hue_center`, `palette_mix`) is normal
audio-bindable `[params]`. All defaults reproduce each scene's prior look
(`[palette]`-less = the classic `spectrum` cosine), so a preset that sets none is
unchanged. The line scenes use their own cosine `hue` and ignore palettes.

```toml
[palette]
name = "ember"                 # or: stops = [ {at=0.0, color="#0b0b2a"}, ... ]

[params]
color_span = "0.3"             # fragment/RD: low = a cohesive single-family mood
hue_spread = "0.15"            # swarm/attractor: low = a coherent-colour cloud
saturation = "0.8 + mid * 2"
```

Full reference — built-in names, custom-stops rules, the per-scene colour params,
and the A/B crossfade — is in **[docs/preset-palettes.md](../docs/preset-palettes.md)**.

## Eased parameters — the `[smoothing]` table

An optional top-level `[smoothing]` table low-passes chosen params so band- and
beat-driven motion eases instead of snapping (ADR-0019). Each entry is a **time
constant in seconds** (a bare number, *not* an expression):

```toml
[smoothing]
zoom  = 0.12   # a punchy pump that still eases
bg_bright = 0.3
hue   = 0.4    # a slow, fluid hue drift
```

A param not listed is applied instantly (today's behaviour); `0` also means no
smoothing. The smoothing runs on real elapsed time, so it is identical at any
refresh rate, and it resets on a preset switch (a switch snaps to the new preset's
first value). Validated non-negative and finite at load.

### Snap up, glide down — the `{ attack, release }` form

One constant slows the rise exactly as much as it slows the fall, so a longer
`tau` trades a jarring attack for a mushy one. An entry may instead name **two**
constants, and the smoother picks by direction (ADR-0035):

```toml
[smoothing]
hue   = 0.4                              # unchanged: symmetric, one constant
burst = { attack = 0.02, release = 0.7 } # snap up in ~2 frames, glide down over ~0.7 s
```

`attack` applies while the incoming value is **above** the held one, `release`
while it is at or below. Both are validated the same way, and `0` on either side
still means instant on that side — `{ attack = 0, release = 0.5 }` is an
instant hit with a slow decay. A scalar entry is exactly
`{ attack = t, release = t }`, so nothing about the existing form changed.

> **The price: a two-constant entry is no longer a low-pass, it is a rectifier.**
> A direction-dependent time constant does not treat a rise and a fall alike, so
> under sustained material the output acquires a DC offset and **rides above its
> input's mean**. That is the envelope-follower behaviour you are asking for on a
> percussive accent, and a surprise on anything continuous — a fast-attack `hue`
> will drift upward rather than tracking. Reach for the pair on things that
> should *hit* (`burst`, `mirror_reflect`, `thickness` on a beat); leave
> continuous params symmetric.

## Structural config (line systems and the attractor)

Declarative data the generator/sampler consumes once at load — **not**
expressions. Validated at load; a bad value is a surfaced error.

### `[curve]` — for `parametric_curve`

| Key      | Values           | Notes                          |
|----------|------------------|--------------------------------|
| `family` | `maurer_rose`    | The curve family. Required.    |

### `[generator]` — for `lsystem`

| Key         | Type            | Notes                                                       |
|-------------|-----------------|-------------------------------------------------------------|
| `axiom`     | string          | Starting string. Required, non-empty.                       |
| `rules`     | table `k = "v"` | Each key a single character (the predecessor). Required.    |
| `angle_deg` | number          | Turn angle for `+`/`-`. Default 25.                         |
| `max_depth` | integer         | Iterations to precompute; clamped to `1..=7`. Default 4.    |
| `seed`      | integer         | Reserved for future stochastic rules (deterministic today). |

Turtle vocabulary in the expanded string: `F`/`G` draw forward, `f` moves without
drawing, `+`/`-` turn by `angle_deg`, `[`/`]` push/pop the branch state, any other
character is an inert grammar variable.

### `[generator]` — for `star_pattern`

| Key                 | Values                                                | Notes                        |
|---------------------|-------------------------------------------------------|------------------------------|
| `tiling`            | `square`/`4`/`4.4.4.4`, `hexagon`/`6`/`6.6.6`, `octagon`/`8`/`4.8.8`, `dodecagon`/`12`/`3.12.12` | Star order `n`. Required. |
| `contact_angle_deg` | number                                                | Hankin contact angle. Default 30. |

### `[particles]` — for `attractor`

| Key      | Values                                       | Notes                                             |
|----------|----------------------------------------------|---------------------------------------------------|
| `family` | `de_jong`, `clifford`, `thomas`, `lorenz`    | Which strange attractor the compute step iterates. Optional — absent means `de_jong`. |

```toml
system = "attractor"

[particles]
family = "lorenz"
```

The family sets the map **and** the meaning of the four bindable coefficients
`a`/`b`/`c`/`d`, each of which defaults to that family's canonical value
(De Jong `1.641 1.902 0.316 1.525`; Clifford `-1.4 1.6 1.0 0.7`; Thomas uses `a`
alone at `0.19`; Lorenz `sigma/rho/beta` = `10 28 2.667`, `d` unused). Bind them
to bands for a morphing cloud, but move them **slowly and by a little** — these
are chaotic maps, and a large jump reads as a hard cut rather than a morph.

Two attractor params behave unlike anything else in the set:

- `fade` (default `0.94`) is the fraction of the trail accumulation kept per
  1/60 s, applied frame-rate-independently. `0` clears every frame (no trails);
  values near `1` smear toward permanence — high `fade` plus `ink_amount` is the
  documented "blot" trap (the page fills in solid).
- `reseed` is **edge-triggered**, not a level: the cloud re-scatters once when the
  bound expression rises past `0.5`, so `reseed = "beat"` re-scatters on each beat
  instead of every frame the flag is held.

## Finding a starting point in this folder

Filenames are `<system>_<look>.toml`, so `ls` is the roster — there is no list here to
drift (adding a preset is dropping a file; `core/build.rs` globs the directory,
ADR-0022). To browse them rendered instead, shoot a contact sheet:

```sh
cargo run -p standalone --example shot -- --presets presets --all --out sheet.png
```

Presets worth reading as **worked examples** of one control each:

| Preset | Shows |
|--------|-------|
| `rose_zoom` | the shared view **zoom** pumping on bass, with a slow sine pan |
| `rose_atmosphere` | a scene over a vignetted **background** gradient (`bg_*`) |
| `rose_kaleidoscope` | the six-fold **geometry mirror**, reflection toggled on the beat |
| `rose_trails` | **feedback trails** smearing a spinning curve into a spiral |
| `fragment_kaleido` | the screen-space **kaleidoscope** over a fragment field |
| `fragment_smooth` | beat-driven flash/glow **eased** through a `[smoothing]` table |
| `attractor_ink` | the terminal **ink-on-paper** remap (`ink_*` / `paper_*`) |
| `rose_maurer_sweep`, `rose_overflow`, `rose_beat_bloom` | the audio-morphable curve **shape** params (`phase`, `radial_offset`) |
