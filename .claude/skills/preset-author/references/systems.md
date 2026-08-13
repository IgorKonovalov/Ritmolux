# Scene & parameter catalogue — authoring guidance

> **The authoritative param roster is `presets/README.md`; defaults are the `DEFAULT_*` consts
> beside each scene's `set_param`.** This file adds what those don't: what each scene is *for*,
> the typical working range of each param (distilled from the shipped set, not an engine limit),
> and which audio input it naturally rides. Where the two disagree, the code wins.

**Naming:** `system = "…"` is the underscore name; it differs from the scene's display name
(`system = "lsystem"` → display "l-system").

**Param lifecycle:** each frame the renderer resets every param to its default, then applies the
preset's bindings. **Any param you don't bind keeps its default** — you only write what you drive.

**Colour:** **every** scene colours through the shared **palette LUT** — `[palette]`,
`[palette_b]`, `palette_mix`, `saturation`, plus either `color_span`/`color_center` (fragment,
reaction-diffusion) or `hue_spread` (+ `hue_center` on swarm/attractor). The shader scenes sample
it per pixel or per particle; the four **line** scenes sample it on the CPU per segment. Since
Plan 0054 / ADR-0059 each line scene walks `hue_spread` along **its own generator's axis** — path
position (`parametric_curve`), generation depth (`lsystem`), radius (`star_pattern`, currently
flat — see below), band index (`spectrum`). `hue_spread = 0` everywhere is one flat `hue`, which is
what these scenes drew before. See `docs/preset-palettes.md` and `presets/README.md`'s axis table.

**Every scene also takes** the shared view transform (`zoom`, `pan_x`, `pan_y`) and the engine
stages `bg_hue`/`bg_bright`/`bg_vignette` + the ramp
(`bg_angle`/`bg_hue_span`/`bg_shade`/`bg_shade_end`/`bg_ramp_gamma`) + the band
(`bg_band_amount`/`bg_band_angle`/`bg_band_pos`/`bg_band_width`/`bg_band_curve`/`bg_band_hue`/`bg_band_hue_span`),
`trails`,
`kaleido_order`/`kaleido_angle`/`kaleido_center_x`/`kaleido_center_y`,
`bloom_amount`/`bloom_threshold`/`bloom_radius`, `exposure`, `ink_amount`/`paper_*`/`ink_*`.
Line scenes additionally take `mirror_order`/`mirror_reflect`.

---

## `fragment_field` — full-screen domain-warp field
*Ambient, nebula, aurora.* All colour in the pixel shader; reads no audio itself, so every bit of
life comes through the bindings. Draws **opaquely**, so `bg_*` has no visible effect here.

| Param | Default | Typical | Controls / natural driver |
|-------|---------|---------|---------------------------|
| `warp` | `0.4` | `0.25 – 2.6` | fold amount — the structure. Gained bass. |
| `hue` | `0.0` | `0 – 1` (+drift) | palette rotation. `time * 0.02..0.06` + a little treble. |
| `zoom` | `1.0` | `0.8 – 2.0` | **inverted sense**: higher shows *more* field. Breathe on `bar`. |
| `glow` | `0.7` | `0.3 – 1.2` | brightness/bloom. Band energy over a floor. |
| `flash` | `0.0` | `0 – 1` | additive white flash. `clamp(onset * 3, 0, 1)`. |
| `color_span` | `0.6` | `0.2 – 0.7` | how much of the gradient the field spans. **Low = cohesive mood.** |
| `color_center` | `0.0` | `0 – 1` | where that window sits. Slide on treble. |
| `saturation` | `1.0` | `0.6 – 1.4` | chroma. |

## `swarm` — ~10k-particle CPU flow swarm
*Kinetic, dancey, physical.* Additive sprites, so density reads as glow.

| Param | Default | Typical | Controls / natural driver |
|-------|---------|---------|---------------------------|
| `force` | `1.4` | `1.4 – 7` | steering toward the flow field. Bass. |
| `spin` | `0.3` | `0.3 – 2.3` | how fast the field evolves. Mid. |
| `burst` | `0.0` | `0 – 12` | radial kick from centre. `beat * 9..11`. |
| `brightness` | `0.8` | `0.8 – 1.8` | global multiplier. |
| `size` | `1.0` | `1.0 – 2.5` | particle size — watch overdraw on the iGPU floor. |
| `hue` | `0.0` | `0 – 1` | gradient offset. |
| `hue_spread` | `1.0` | `0.1 – 1.0` | width of the per-particle hue band. **`1.0` is full rainbow; drop it for a coherent cloud.** |
| `hue_center` | `0.5` | `0 – 1` | centre of that band — two presets differing only here read as different colours. |

## `parametric_curve` — Maurer-rose line curve
*Precise, geometric, hypnotic.* `[curve] family = "maurer_rose"` (the only family; optional).

| Param | Default | Typical | Controls / natural driver |
|-------|---------|---------|---------------------------|
| `n` | `6.0` | `2 – 12` | petal frequency. Keep integer-ish (`floor`). |
| `d` | `71.0` | `2 – 360` | angular step — the "web" density. |
| `phase` | `0.0` | `0 – tau` | radians **inside** the sine: reshapes petals as it advances (distinct from `spin`, which rotates the finished figure). Morph on `bar`/`bass`. |
| `radial_offset` | `0.0` | `-1 – 1` | added to the radius — opens the rose into spiral/annular/rosette forms. Nonzero pushes `r` past `[-1,1]`; large values blow past the frame (intended, the renderer clips). |
| `samples` | `361.0` | `120 – 720` | chord count; capped by `MAX_SEGMENTS`. |
| `thickness` | `2.0` | `1 – 5` | stroke weight. |
| `hue` | `0.6` | `0 – 1` (+drift) | where the figure sits in the palette. |
| `hue_spread` | `0.0` | `0 – 1` | walks the palette **along the traced path** — first chord to last. Normalized over `samples`, so `draw_progress` draws the gradient on. |
| `saturation` | `1.0` | `0 – 2` | shared chroma modulation. |
| `palette_mix` | `0.0` | `0 – 1` | A/B crossfade with `[palette_b]`. |
| `spin` | `0.1` | `0 – 1` | angular velocity (`rotation = spin * time`). |
| `scale` | `0.9` | `0.6 – 1.0` | size in frame. |
| `brightness` | `1.0` | `0.8 – 1.6` | multiplier. |
| `draw_progress` | `1.0` | `0 – 1` | line-draw-on reveal; ride `bar` for a per-beat redraw. |

## `lsystem` — branching L-system growth
*Organic, botanical, growing.* `[generator]` **required** (axiom / rules / `angle_deg` /
`max_depth ≤ 7` / seed).

| Param | Default | Typical | Controls / natural driver |
|-------|---------|---------|---------------------------|
| `visible_depth` | `1.0` | `1 – max_depth` | which cached iteration draws — the signature move: `4 + floor(2 * bass)` grows the plant on a swell. |
| `rotation` | `0.0` | radians | **absolute angle**, not a rate — multiply by `time` yourself. |
| `draw_progress` | `1.0` | `0 – 1` | draw-on reveal. |
| `hue` | `0.3` | `0 – 1` (+drift) | where the figure sits in the palette. |
| `hue_spread` | `0.0` | `0 – 1` | walks the palette by **generation depth** — trunk to deepest twig. The fern-as-growth lever. |
| `saturation` | `1.0` | `0 – 2` | shared chroma modulation. |
| `palette_mix` | `0.0` | `0 – 1` | A/B crossfade with `[palette_b]`. |
| `thickness` | `1.8` | `1 – 4` | stroke weight. |
| `scale` | `1.0` | `0.7 – 1.0` | size. |
| `brightness` | `1.0` | `0.8 – 1.6` | multiplier. |

Only depths up to `max_depth` are built, so `visible_depth` is clamped to what exists.

**`hue_spread` needs a grammar with branches.** Generation depth is bracket nesting, so a rule set
with no `[` (`lsystem_arrowhead`'s `F -> G-F-G`, `G -> F+G+F`) has exactly one generation and the
ramp is flat there however large `hue_spread` gets — a property of that figure, not a gap. A
branching grammar (`lsystem_fern` reaches generation 11 at `visible_depth = 6`) ramps across its
whole depth, because the divisor is the built figure's own deepest generation.

## `star_pattern` — Hankin star rosette
*Symmetric, architectural, mandala.* `[generator]` **required** (`tiling` 4/6/8/12,
`contact_angle_deg`).

| Param | Default | Typical | Controls / natural driver |
|-------|---------|---------|---------------------------|
| `variant` | `1.0` | `0 – 2` | **continuous** contact angle (pointy↔blunt), ±24° around `contact_angle_deg`. `0`/`1`/`2` are the three shapes it used to index; everything between is a real rosette. Sweep it with something closed — `1 + sin(time * 0.14)` — and ease it in `[smoothing]`. |
| `rotation` | `0.0` | radians | absolute angle. |
| `draw_progress` | `1.0` | `0 – 1` | draw-on reveal. |
| `hue` | `0.5` | `0 – 1` (+drift) | where the figure sits in the palette. |
| `hue_spread` | `0.0` | — | radius axis, **inert** on this rosette (see below). |
| `saturation` | `1.0` | `0 – 2` | shared chroma modulation. |
| `palette_mix` | `0.0` | `0 – 1` | A/B crossfade with `[palette_b]`. |
| `thickness` | `2.0` | `2 – 6` | stroke weight. |
| `scale` | `1.0` | `0.8 – 1.0` | size. |
| `brightness` | `1.0` | `0.8 – 1.6` | multiplier. |

**Two things measured, so you don't re-derive them.** (1) `hue_spread` does nothing here: the
rosette is `2n` congruent segments about the frame centre, so every segment sits at the same
radius (spread `1.2e-7`) and there is no range to walk. `[palette]` itself works — reach for that.
(2) The interior is empty at every contact angle: at 12-fold / 20° the strokes live between radius
0.54 and 0.90, so 60% of the disc is bare, and 87% at 55°. That is design-backlog 0007's open half;
don't try to fill it from a preset.

**A `floor` around `mod(…, 3)` is the old idiom and is now wrong** — the floor was there because
`variant` used to index. Removing it alone is worse (a sawtooth snaps 2 → 0 at the wrap); replace
the whole driver with a triangle or sine sweep. The two shipped presets still carry the old form.

## `reaction_diffusion` — Gray-Scott field
*Coral, maze, mitosis — slow, organic, alive.* A running simulation: parameters steer a **regime**,
they don't redraw a figure, so changes take a second or two to read. Composites over `bg_*`.

| Param | Default | Typical | Controls / natural driver |
|-------|---------|---------|---------------------------|
| `feed` | `0.0367` | `0.02 – 0.06` | Gray-Scott F. **Tiny moves change the whole pattern family** — nudge, don't sweep. |
| `kill` | `0.0649` | `0.055 – 0.07` | Gray-Scott K. Same caution; the F/K pair is the regime. |
| `flow` | `1.0` | `0.5 – 2` | simulation rate — the safest audio-driven knob. |
| `inject` | `0.0` | gate | **edge-triggered** like the attractor's `reseed`: a rising edge stamps one seeded blob at a deterministic pseudo-random spot. `"beat"` blooms a new growth per beat. |
| `contour` | `6.0` | `3 – 12` | contour banding of the field. |
| `hatch` | `5.0` | `0 – 12` | hatching texture. |
| `glow` | `1.0` | `0.6 – 1.6` | brightness. |
| `color_span` | `0.85` | **`2.0 – 2.5`** for a full custom gradient | RD's field level only reaches ~`0..0.4`, so the default never reaches a warm end — see `docs/preset-palettes.md`. |
| `color_center` | `0.0` | `0 – 1` | tonal centre. |

## `attractor` — GPU compute particles on a strange attractor
*Filamentary, chaotic, luminous.* `[particles] family = de_jong | clifford | thomas | lorenz`
(optional; defaults `de_jong`). The family sets the map **and** the meaning of `a`/`b`/`c`/`d`,
each defaulting to that family's canonical value.

| Param | Default | Typical | Controls / natural driver |
|-------|---------|---------|---------------------------|
| `a` `b` `c` `d` | family canon | ±0.05 around canon | the map's coefficients — **chaotic**: move them slowly and by a little, or it reads as a cut, not a morph. |
| `tuple` | `0` | `0 .. roster-1` | **picks a whole figure, framing included** (Plan 0079/ADR-0093). Each map family carries a curated roster — `de_jong` 13, `clifford` 13, `thomas` 13, `lorenz` 12 — whose entries carry their coefficients *and* their projection + seed box, so a distant figure arrives centred and in frame where `a`..`d` alone could not reach it at all. `0` is the canonical figure. **Quantized CPU-side to the nearest whole entry**, like `kaleido_spiral`: there is no figure between two entries, so a change is a **cut** and wants a long `[smoothing]` or a slow binding. A bound `a`..`d` loses to the entry on the frame the cut lands. |
| `size` | `1.0` | `0.6 – 2.0` | point size. Reads *finer* at high resolution — a value tuned on a small capture may look thin at 1080p. |
| `fade` | `0.94` | `0 – 0.98` | trail persistence per 1/60 s. `0` = no trails; near `1` smears toward permanence (the "blot" trap with `ink_amount`). |
| `reseed` | `0.0` | gate | **edge-triggered**: re-scatters once when the expression rises past `0.5`. `"beat"` re-scatters per beat. |
| `hue_spread` | `0.15` | `0.05 – 0.4` | per-particle hue band width. |
| `hue_center` | `0.075` | `0 – 1` | centre of that band. |

**To *travel* between two figures instead of cutting, name a path** (Plan 0079/ADR-0093):
`[particles] tuple_to = <index>` turns the walk on (`tuple_from` is the near end, entry `0` by
default), and the already-existing `morph` param — continuous, the one place in this scene's surface
a param is *not* quantized — is the position along it. `tuple` goes inert while a path is
configured; a preset either steps the roster or walks a path. **Not every pair has a walk**: a tuple
partway between two others can collapse to a fixed point, which has no scale to render at, so the
engine refuses the path and the preset sits on its near end with `morph` doing nothing — the first
thing to suspect when a walk looks dead. Four shipped paths were judged in motion (`thomas` 5→8,
`lorenz` 0→1, `lorenz` 0→4, `de_jong` 1→3); the one-dimensional sweeps hold best, because
neighbouring `a`/`rho` values are neighbouring *figures*. Do **not** put `morph` in `[smoothing]` —
easing an already-slow curve only lags it. Full table and the per-roster notes:
[`presets/README.md`](../../../../presets/README.md).

---

## `spectrum` — the frequency-axis readout (Plan 0034)

*A measurement you can look at.* A line system like the three above, but its figure is the engine's
64-band log-spaced array rather than a generator's geometry. `[spectrum]` is optional:
`elements` (2..=64, default 24), `layout` (`bars` | `polyline` | `radial_ring`, default `bars`), and
a per-element `smoothing` (seconds, or `{ attack, release }`).

**Nothing in `[params]` maps audio to position — that mapping *is* the scene.** Element 0 is the
bottom of the spectrum, the last is the top. The params say how the elements look and how far they
reach.

| Param | Typical | Controls / natural driver |
|-------|---------|---------------------------|
| `base` | `0.1 – 0.6` | the length every element has **before** audio, in world units (the frame is 2 tall). Deliberately non-zero: at `0` the readout vanishes in a silence and reads as broken. Bind to `time` for a resting breath. |
| `scale` | well above `1` | multiplier on the element's own band level — the bands read **small**, same caveat as `bass`/`mid`/`treb`. |
| `radius` | `0.2 – 0.6` | **`radial_ring` only** — the inner circle the spokes stand on. No effect on the other two layouts. |
| `rotation` | radians | turns the whole figure; the natural motion on the ring, a tilt on bars/polyline. |
| `hue_spread` | `0 – 1` | walks the palette across the elements, so you can see *where* a peak is. `1` on `radial_ring` wraps continuously. |

**This is the only line system that reads `[palette]`** — the other three colour through their own
cosine `hue`. It honours the view transform, the geometry mirror (transformative on `bars`/
`polyline`, near-noop on `radial_ring` for the same reason as `star_pattern`), and every engine stage.

**Per-element bindings.** A binding whose text names `index` is evaluated once per element, with
`index` at that element's `0..1` position — so `thickness = "0.01 + bin(index) * 0.05"` thickens each
element by its own band, and `base = "0.16 + index * 0.12"` gives the quiet top end a longer rest.
Five params genuinely vary per element (`base` `scale` `thickness` `brightness` `hue`); the
whole-figure ones take the `index = 0` value rather than being dropped. `[smoothing]` cannot ease a
per-element binding (a surfaced warning) — use `[spectrum] smoothing`, where an asymmetric
`{ attack, release }` earns its keep more than almost anywhere else: the bands are the rawest signal
in the engine, so a fast attack keeps a transient's shape while a slow release lets elements fall
like a meter instead of strobing.

**Verify these with `--signal`, never `--set`** — see the spectrum footgun in `SKILL.md`.

---

## `emitter` — objects that spawn, fall, and die (Plan 0052)

*The only system whose population is not fixed.* Every other scene draws the same number of things
every frame. This one throws objects from a source line just below the frame, gives each its own
parabola, and **retires** it when its life runs out or it leaves the frame. Nothing wraps — that is
the whole difference from `swarm`, whose world is a torus and whose particles cannot leave
([ADR-0057](../../../../docs/adrs/0057-emitter-scene-analytic-ballistics-seeded-individuation.md)).

**The path is a closed form fixed at spawn** (`p0 + v0*t + 0.5*g*t²`). Two authoring consequences:
there is **no mid-flight force** — no drag, no swirl, no steering — and easing `gravity` or
`launch_speed` only changes objects thrown *after* the change, since the ones in the air keep the arc
they launched on. There is also no `dt` in it, so the motion is identical on every device.

| Param | Typical | Controls / natural driver |
|-------|---------|---------------------------|
| `spawn_rate` | `20 – 400` | objects a second — the density lever. Population settles near `spawn_rate * flight time`. The natural `onset` binding: bursts on transients, empties between them. |
| `gravity` | `1 – 10` | downward acceleration, world units per second squared. |
| `launch_speed` | `1 – 4` | speed at the source. |
| `launch_angle` | radians | **clockwise from straight up**; `0` is vertical, `1.57` throws right. |
| `lifetime` | seconds | how long an object lives if it has not left frame first. Past the flight time it is wasted pool. |

**Do the crest arithmetic before tuning.** A mark launched at `v` against `g` turns over `v² / (2g)`
world units above the source line at `y = -1.12`, and a frame is `|y| <= 1`. So a crest *inside* the
frame draws a visible horizontal ceiling where the population piles up. `emitter_sparks.toml` puts
its crest off-frame at `y = 1.28` deliberately.

### Individuation — the spread params

**A binding is evaluated once per frame for the whole scene**, so no expression can make one object
differ from another. These are the answer: each sets how *wide* a per-object draw is, and the
object's own seed picks within it, once, at spawn. The preset owns the distribution; the seed owns
the member.

| Param | What varies, per object |
|-------|-------------------------|
| `spread` | launch angle, within a cone of this width centred on `launch_angle`. At `0` the shower is a column of beads; open it and the arcs cross. |
| `size_spread` | mark size, as a fraction either side of `size`. |
| `lifetime_spread` | life, as a fraction either side of `lifetime`. |
| `spin` | how fast the mark turns, radians a second — **signed** per object, so the field turns both ways. |
| `twinkle` | a brightness oscillation whose **rate and phase both** come off the seed. |

`twinkle` is the answer to "make the stars blink and they all flash together": because each object
draws its own *rate*, the whole-frame brightness stays steady while every member swings. A shared
rate flashes as one sheet whatever the phases are. The mark `spin` turns is a soft **elongated
glint**, not a disc — a disc is rotationally symmetric, so `spin` on one would be invisible.

### Walls (route to `architect`, do not work around)

No per-object expressions — "every seventh object is gold" is not expressible; widen a spread
instead. No collision or inter-object forces. No stamped trail: `trails` decays, so behind an object
that *leaves* it reads as a comet tail rather than the hard copies a cascade wants — keep it short.
And **no positionable source** — the line spans the frame width at `y = -1.12` and cannot be moved
or narrowed, so a point fountain or an off-centre jet is engine feedback.

Full parameter roster and defaults: [`presets/README.md`](../../../../presets/README.md).

---

## Engine-wide stages (any system)

| Param | Default | Note |
|-------|---------|------|
| `zoom` / `pan_x` / `pan_y` | `1` / `0` / `0` | camera **in** on line/swarm/attractor; **out** (shows more) on fragment/RD. |
| `bg_hue` / `bg_bright` / `bg_vignette` | `0` / `0` / `0` | backdrop is black until `bg_bright > 0`. Visible behind sparse scenes and RD voids; **invisible behind `fragment_field`**. |
| `bg_angle` / `bg_hue_span` | `0` / `0` | **the directional ramp** (Plan 0080/ADR-0094): the backdrop paints a *segment* of your `[palette]` along one axis instead of one point of it. `bg_angle` is **radians**, `0` = bottom-to-top; `bg_hue_span` is how far the coordinate travels, `bg_hue` being the coordinate at the ramp's **start**. Placement is your stops' own `at` positions — there is no `bg_ramp_center`. The segment **wraps** if it leaves `[0, 1]`. |
| `bg_shade` / `bg_shade_end` | `0.72` / `1.0` | the brightness ramp's two ends, on that same axis. These two numbers **are** the fixed `0.72 -> 1.0` upward tilt the pass used to hardcode, so leaving them alone changes nothing — but a backdrop can now be brighter at the **bottom**, which it never could be. |
| `bg_ramp_gamma` | `1.0` | the ramp's **response exponent**, applied to the *position* ahead of both channels so colour and brightness reach their midpoints at the same height. `> 1` holds the ramp near its start then falls away (a hot horizon band, then a long fade); `< 1` drops fast into a dim tail. Clamped `0.05 .. 20`. It is the only shape control the brightness ramp has, and the only one that shapes the sky *without* re-mapping the figure — the `[palette]` is shared. |
| `bg_band_amount` | `0.0` | **the curved band** (Plan 0081/ADR-0095): one soft gaussian swell of light drawn *additively over the ground and under the scene*, for a Milky Way arc over a horizon. `0` draws no band and leaves the six below inert. **This alone lights the pass** — a band over a `bg_bright = 0` sky paints, which is the near-black sky the look actually wants. Hidden by an opaque scene exactly as the ramp is; **absent** behind `fragment_field`. |
| `bg_band_angle` / `bg_band_pos` | `0` / `0.5` | `bg_band_angle` is **radians** naming the direction **across** the band — same convention as `bg_angle`, so `0` runs the band *horizontally* and the band itself is perpendicular to the number you write. `bg_band_pos` is the centreline's position along that across-axis, in the same normalized `0..1` the ramp uses. |
| `bg_band_width` | `0.15` | the **`1/e` half-width**, not a full width and not an edge: the envelope has fallen to ~37 % exactly this far either side of the centreline and is still faintly visible for two or three times that, so the **visible band is several times wider than the number**. Clamped `0.001 .. 100`. |
| `bg_band_curve` | `0.0` | the **arc** — how far the centreline bows, in across-axis units, at the middle of the band. `0` is exactly straight. This is the silhouette that reads as a galaxy rather than a streak; move `bg_band_pos` to ride the arc up or down the frame and raise this to bow it further. |
| `bg_band_hue` / `bg_band_hue_span` | `0` / `0` | the band's **own** segment of the same `[palette]`, swept **along** the band so one end can brighten toward a core. `bg_band_hue` is an **absolute** coordinate, not an offset from the ground's, so the arc keeps its colour whatever the ramp underneath is doing. Repeat-addressed, so a span leaving `[0, 1]` wraps. **One palette now serves the ground, the band, the figure and the `[layer]`** — a dusk gradient fully spent on a horizon has no stops left for a pale arc, and that is the one real authoring constraint here. |
| `trails` | `0` | per-frame decay; `0` off, higher = longer. Needs real motion to read. |
| `kaleido_order` / `kaleido_angle` | `1` / `0` | `< 2` is passthrough. **The order is rounded to a whole number** (a fractional wedge count tears the frame), so it snaps at each half-integer even when smoothed — ride `kaleido_angle` on `time` for continuous motion. |
| `kaleido_center_x` / `kaleido_center_y` | `0.5` / `0.5` | the fold axis, in uv; clamped into the frame. The fold shows the largest disc around that axis and fades out past it onto the backdrop. |
| `bloom_amount` / `bloom_threshold` / `bloom_radius` | `0` / `1.0` / `1.0` | `0` amount is off and free. The threshold is in **linear light**, so the default blooms exactly what the display could not have shown. The radius spreads the same energy wider rather than adding more (`0..4`). |
| `exposure` | `1.0` | linear multiplier on the whole frame before the engine tonemap. Crossfades across a preset switch like `ink_*` does. |
| `mirror_order` / `mirror_reflect` | `1` / `0` | **line scenes only**; folds geometry (before the segment cap), not pixels. |
| `ink_amount` | `0` | `1` = black-on-white; `paper_*`/`ink_*` make it any duotone. Collapses the palette to two colours. **Not a contrast control** — a partial value is a transition, not a resting place. |
| `ink_gamma` | `1` | the **response** between the two poles (Plan 0078). Above `1` thins the mids toward paper so only the strong strokes keep full ink; below `1` inks them for a heavier print. Neither pole moves at any value. This is the lever for "the ink should bite harder" — `exposure` (upstream) and `ink_amount` (how much remap) are the other two, and they are not interchangeable. |

Details and the exact semantics: `presets/README.md`.
