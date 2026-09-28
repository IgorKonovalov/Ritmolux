# Scene & parameter catalogue — authoring guidance

> **The authoritative param roster is `presets/README.md`, generated from each scene's `PARAMS`
> (`ParamSpec` declarations, beside its `set_param`); the scene's `DEFAULT_*` consts are derived from
> those same declarations via `default_of`.** This file adds what those don't: what each scene is *for*,
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
position (`parametric_curve`), generation depth (`lsystem`), radius (`star_pattern` — flat on a
bare interlace, live once `rings` is declared; see below), band index (`spectrum`). `hue_spread = 0` everywhere is one flat `hue`, which is
what these scenes drew before. See `docs/preset-palettes.md` and `presets/README.md`'s axis table.

**Almost every scene takes** the shared view transform (`zoom`, `pan_x`, `pan_y`) — the exceptions
are `shape_field` and `shape_collage` (no `zoom`) and `warp_mesh` (no `pan_x`/`pan_y`). **Every scene
takes** the engine stages `bg_hue`/`bg_bright`/`bg_vignette` + the ramp
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

## `parametric_curve` — line curves
*Precise, geometric, hypnotic.* `[curve]` is optional; inside it `family` is required and names
one of five: `maurer_rose` (the default when the table is absent), `lissajous`, `hypotrochoid`,
`superformula`, `harmonograph`. The outside-rolling epitrochoid is the same family as
`hypotrochoid` with a **negative `n`** — the sign picks which circle rolls.

**`n` and `d` mean something different in every family**, and so do `pen`, `sym`, `sharpness`,
`lobe` and `decay`, which only some of them read. The per-family ranges are in
`presets/README.md`, generated from the engine's own declarations — read them there rather than
from a copy here, which is how this section went stale in the first place. The rows below are the
**rose's** reading.

| Param | Default | Typical | Controls / natural driver |
|-------|---------|---------|---------------------------|
| `n` | `6.0` | `2 – 12` | petal frequency (rose). Keep integer-ish (`floor`). |
| `d` | `71.0` | `2 – 360` | angular step (rose) — the "web" density. |
| `phase` | `0.0` | `0 – tau` | radians **inside** the sine: reshapes petals as it advances (distinct from `spin`, which rotates the finished figure). Morph on `bar`/`bass`. |
| `radial_offset` | `0.0` | `-1 – 1` | added to the radius — opens the rose into spiral/annular/rosette forms. Nonzero pushes `r` past `[-1,1]`; large values blow past the frame (intended, the renderer clips). |
| `samples` | `361.0` | `120 – 720` | chord count; capped by the tier's segment cap (`TierConfig::max_segments`). |
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
with no `[` (`lsystem_rime`'s Koch rule `F = "F+F--F+F"`) has exactly one generation and the ramp is
flat there however large `hue_spread` gets — a property of that figure, not a gap. A branching
grammar (`lsystem_bower`'s `F = "F[+F]F[-F]F"`) ramps across its whole depth, because the divisor is
the built figure's own deepest generation.

## `star_pattern` — Hankin star rosette
*Symmetric, architectural, mandala.* `[generator]` **required** (`tiling` 4/6/8/12 or `none`,
`contact_angle_deg`, and optional `rings` — concentric rings of repeated motifs inside the rosette,
ADR-0079; `none` with no `rings` is a load error).

| Param | Default | Typical | Controls / natural driver |
|-------|---------|---------|---------------------------|
| `variant` | `1.0` | `0 – 2` | **continuous** contact angle (pointy↔blunt), ±24° around `contact_angle_deg`. `0`/`1`/`2` are the three shapes it used to index; everything between is a real rosette. Sweep it with something closed — `1 + sin(time * 0.14)` — and ease it in `[smoothing]`. |
| `rotation` | `0.0` | radians | absolute angle. |
| `draw_progress` | `1.0` | `0 – 1` | draw-on reveal. |
| `hue` | `0.5` | `0 – 1` (+drift) | where the figure sits in the palette. |
| `hue_spread` | `0.0` | `0 – 1` with `rings` | radius axis — **inert** on a bare interlace, live once `rings` is declared (see below). |
| `ring_phase` / `ring_spread` / `ring_scale` | identity | — | move the `rings` ornament: counter-rotate alternate rings / multiply every radius / multiply every motif's size. Inert without `rings`. |
| `saturation` | `1.0` | `0 – 2` | shared chroma modulation. |
| `palette_mix` | `0.0` | `0 – 1` | A/B crossfade with `[palette_b]`. |
| `thickness` | `2.0` | `2 – 6` | stroke weight. |
| `scale` | `1.0` | `0.8 – 1.0` | size. |
| `brightness` | `1.0` | `0.8 – 1.6` | multiplier. |

**Two things measured, so you don't re-derive them.** (1) On a bare interlace `hue_spread` does
nothing: the rosette is `2n` congruent segments about the frame centre, so every segment sits at the
same radius and there is no range to walk — `[palette]` itself works. (2) A bare interlace leaves the
interior empty: the strokes live in an outer annulus. **`rings` is the answer to both** — it puts
segments at several radii, so the ramp spans the combined figure and the interior fills. On a
composite (rings *plus* a tiling) the interlace sits at one end of the ramp and the ornament spreads
along the rest. Shipped: `star_corona` and `star_mandala_bordered` (rings only), `star_zellij`
(rings plus an 8-fold interlace). **Animate a mandala on `ring_spread` / `ring_scale`, not spin
alone** — a many-fold ring figure turned by any angle lands almost on itself, so rotation reads as
frozen to the `animation` gate and, at a distance, to the eye; spend `ring_phase` on the
counter-rotation as ornament.

**A `floor` around `mod(…, 3)` is the old idiom and is now wrong** — the floor was there because
`variant` used to index. Removing it alone is worse (a sawtooth snaps 2 → 0 at the wrap); replace
the whole driver with a triangle or sine sweep, as `star_rosewindow` does.

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
(optional; defaults `de_jong`), or an IFS figure from the same namespace —
`fern | tree | dragon | sierpinski | spiral` (`attractor_fern`, `attractor_dragon` ship). The map
family sets the map **and** the meaning of `a`/`b`/`c`/`d`, each defaulting to that family's
canonical value; the `tuple` roster and walk below are the map families' surface.

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

## `shape_field` — one silhouette at frame scale

*Emblematic, graphic, still.* The `marks` roster drawn as a **fullscreen
signed-distance field**: the palette coordinate is the *distance* to the figure, so
`palette_steps` draws concentric offset contours of the shape rather than of a
circle, and `palette_contour` outlines them. It draws opaquely and reads no audio
itself. `coord_mode = 1` swaps the coordinate for `r / r_boundary(theta)`, whose
contours are **scaled copies** of the outline instead of offsets — that is the one
to reach for when a notch or a corner must stay sharp at every ring.

**A `[path]` table replaces the roster with a silhouette you drew** — inline SVG
path data, one closed contour. Two params come with it, and both work on a
rostered figure too:

| Param | Typical | Controls / natural driver |
|-------|---------|---------------------------|
| `stroke` | `0.04 – 0.15` | draws the figure's **outline** at this half-width instead of filling it, in coordinate units (`1` is the whole interior). `0` fills, and is an exact identity. Fill and outline are the same field, so a stroke cannot drift off its figure. |
| `morph` | `0 – 1` | travels a `[path]` towards its `morph_to` silhouette. Inert without one. This is the param to hang a `beat` or a `[latch]` on — a figure *becoming* another figure is the whole reason the table takes a second contour. |

**Three walls worth knowing before you author against it**, all of them the
engine's rather than yours: `samples` is capped at **64** and asking for more is a
load error; **`A`/`a` and a second subpath are refused by name**, so a traced
letterform with a counter and most rounded-corner exports do not load as drawn;
and a **morphing** pair costs more per frame than a static one, because a static
curve is drawn as a fitted arc chain and a morphing one cannot be. The subset,
the refusals, the measurement behind the ceiling and the rules for which pairs
morph well are all in
[`presets/README.md`](../../../../presets/README.md#path--for-shape_field).

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
| `scale` | `0.5 – 2.5` | how far a **full** band pushes the element above its `base` (default `1.2`). Bands are `0..1` since ADR-0049, so this is a length, not a rescue gain. |
| `radius` | `0.2 – 0.6` | **`radial_ring` only** — the inner circle the spokes stand on. No effect on the other two layouts. |
| `rotation` | radians | turns the whole figure; the natural motion on the ring, a tilt on bars/polyline. |
| `hue_spread` | `0 – 1` | walks the palette across the elements, so you can see *where* a peak is. `1` on `radial_ring` wraps continuously. |

Like every scene it reads `[palette]` (ADR-0059), walking `hue_spread` across the band index. It
honours the view transform, the geometry mirror (transformative on `bars`/
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
world units above the source line (at `y = -1.12` unless `source_y` moves it), and a frame is
`|y| <= 1`. So a crest *inside* the frame draws a visible horizontal ceiling where the population
piles up. `emitter_perseids.toml` puts its crest off-frame deliberately; `emitter_emberjet.toml` is
the inverse — a fountain, where the arc turning over in frame *is* the figure — and its header does
the arithmetic both ways.

### The source, and the two warm-ups (Plan 0090)

The source line is **geometry you own**
([ADR-0104](../../../../docs/adrs/0104-the-emitters-source-is-authorable-geometry.md)). Every default
below is exactly the line the scene always drew, so binding none of them changes nothing.

| Param | Default | Controls |
|-------|---------|----------|
| `source_y` | `-1.12` | the line's world `y`. **May sit inside the frame** — that is what makes a slow look reachable. |
| `source_width` | `1` | half-width **as a fraction of the frame's**. `1` spans it; `0` is a point source. |
| `spawn_fade` | `0` | fraction of a life spent ramping up from black. The answer to an inside-frame source. |
| `prewarm` | `0` | lifetimes of spawns back-dated at scene start, so the population **begins** at steady state. Clamped at `2`. |

**A point fountain is `source_width = 0`; an off-centre jet is that plus `pan_x`** (the line is centred
on the world origin and `pan_x` carries the scene). Close `spread` too if the throw should be a column
rather than a fan.

**Two warm-ups, and they are independent.** Moving the source inside the frame removes the *travel*
warm-up — from below the frame an object has 2.12 world units to cross before it is visible, which at
sky speeds is seconds of empty picture. It does nothing about the *population* warm-up: the pool starts
empty and fills at `spawn_rate` over a whole `lifetime`. `prewarm` is that half. Both matter because
every behavioral gate captures **half a second**: a slow draft measured `0.0074` coverage with 0 of 10
radial shells at `prewarm = 0` — convicted blank — and `0.1470` with 10 of 10 at `prewarm = 1`, changed
in nothing else.

**Two prices, both yours to judge.** An inside-frame `source_y` at `spawn_fade = 0` **pops** — the mark
switches on at full brightness where the eye is — and nothing validates the pair. And a prewarmed world
is **full on its first frame**, which is right for a sky and wrong for a cascade: a preset switching
into one arrives already populated. Record the verdict in the world's header the way the fold-edge
verdicts are recorded.

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
No source *shape* either: the source is a line, and a ring or an area is engine feedback (the two
scalars above reach every shape anyone has asked for). And an object **vanishes** at its death time
rather than fading out — the retirement margin is what usually puts that off-frame, which an
inside-frame `source_y` undoes; a fade-*out* to match `spawn_fade` does not exist yet.

Full parameter roster and defaults: [`presets/README.md`](../../../../presets/README.md).

---

## `warp_mesh` — the past frame, resampled through a per-vertex grid
*Tunnels, weather, smoke, op-art contour prints.* Family `warp` (`warp_*.toml`). **This scene draws
nothing of its own.** It lays light into a feedback field (the **deposit**, a ring that can be
broken into arms) and then **moves the field**. The motion is a grid of per-vertex transforms bound
in `[per_vertex]` with `x y rad ang` in scope. So a world is authored as *motion over a seed*, not as
a shape. Turn the deposit off and the frame is black within a second. The `[per_vertex]` and `[mesh]`
tables are in `presets/README.md` under `warp_mesh`.

It has two modes, split by `color_source`, and they want opposite settings for almost everything:

- **Light** (`color_source = "0"`, the default; 5 of the 7 shipped: Wellhead, Millrace, Cauldron,
  Sirocco, Smoke). The deposit is coloured by its own angle and the loop drags the colour around.
  It glows and blooms like the other additive scenes.
- **Print** (`color_source = "1"`; Ladder, Tracery). The field accumulates an uncoloured level, and
  the present reads that level as the palette coordinate. A stepped palette then prints the loop's
  **contour map** in flat inks (ADR-0197).

| Param | Typical (light / print) | Controls / natural driver |
|-------|-------------------------|---------------------------|
| `decay` | `0.16 – 0.22` radial, `0.55 – 0.78` directional / `0.93 – 0.955` | The tail, and the structural decision. A zooming or turning world wants it short: Wellhead at 0.5 was one smooth blob, and at 0.16 its arms stay legible. A world whose tail *is* the figure wants it long, like Sirocco's bands and Smoke's climb. Print worlds run long because the level has to reach the frame edge. Bass, `+0.06 – 0.12`, smoothed 0.4 – 0.5 s. |
| `deposit` | `1.5 – 3.2` base / `4.8 – 6.5` | Fuel. **The source is where audio shows.** Mid `+0.9 – 1.2`, onset `+0.4 – 0.5`, asymmetric smoothing (attack 0.04, release 0.5). Smoke puts bass there instead (`+4`). In print mode bass `+0.9` walks a contour ring outward. |
| `deposit_arms` | `3 – 12` / `0` or `7` | **Arms are the texture the motion is legible against.** Cauldron and Smoke were first drafted without them and came back as featureless gradients: `warp` displaces the field and creates no structure. Every light world ships with arms. Fewer arms under a shearing `rot` (Millrace, 3), because shear stretches each one into a long spiral. |
| `deposit_radius` / `_width` | `0.14 – 0.44` / `0.05 – 0.09` | A narrow ring (a wire) gives the field edges to smear. Ladder's print source is a blob (`0.06` / `0.28`), so its level is one hill and every rung is a closed contour. A ring would print a ripple in a pond. |
| `deposit_twist` / `_spin` | `0.6 – 3.2` / `±0.25 – 0.45` | Twist leans the arms into spirals. It is a good mid lever (Smoke `+0.8`, Tracery `+0.34`, smoothed 0.2 – 0.8 s), and it *is* visible to `--report`. Spin integrates a phase, so treble `+0.3` is safe on it. Print worlds turn slowly (`0.045`). |
| `zoom` (per-vertex) | `1.0 → 1.45` at the rim / `0.965 – 0.99` | Above 1 the past expands outward (Wellhead's tunnel, on `smoothstep(0, 1, rad)`). Just below 1 it creeps inward (Millrace, Ladder, Tracery). Sirocco holds it at exactly 1, because any zoom makes a directional world radial again. |
| `rot` (per-vertex) | `0.05 – 0.6`, or signed across `rad` | A rate that changes with `rad` is **shear**. Millrace's `1.05 - rad * 1.3` changes sign partway out, which is the whole look. |
| `dx` / `dy` (per-vertex) | `0.03 – 0.33` / `-0.09 – -0.59` | Drift in frame-heights per second. A `sin` over `y` gives Sirocco bands that shear against each other. `(1 - y)` factors give Smoke a buoyancy gradient (`y` is 0 at the **top**). |
| `warp` / `warp_scale` / `warp_speed` | `0.085 – 0.16` print; up to `1.8` (Cauldron) and `5.9` (Smoke) where it is a gradient / `0.3 – 3.1` / `0.45 – 1.05` | The only non-affine output: a boil with no fixed point. It needs arms to stir. `warp_speed` integrates a phase (ADR-0132), so it is safe on treble or mid (Cauldron `+0.5`). |
| `darken_center` | `0.06 – 0.15` | Stops a *zooming* loop saturating at its fixed point. Set it to `0` on a world with no fixed point (Sirocco), where it reads as a hole. |
| `palette_steps` | match `deposit_arms` / `12 – 14` | In light mode, steps matched to the arm count (Wellhead 6/6, Millrace 3/3) give each arm one flat colour. That is the only crisp edge a scene made of gaussians can draw. `0` for a boil or smoke. In print mode the steps are load-bearing: quantizing the coordinate also quantizes the coverage, so Ladder goes from 851 distinct colours to 60. |
| `color_span` | `0.8 – 0.9`, Smoke `0.12` / `0.085 – 0.13` | Light: how much of the palette the angle walks. Smoke is kept low with `saturation 0.22` so it stays smoke and not fire. Print: the **rung spacing**, because it scales the level. Treble crowds it by about a tenth (`+0.01`), eased 0.5 s. |
| `color_center` | `time * 0.0045 – 0.007` / `time * 0.032 – 0.045` | Light: a slow colour walk. Print: **what makes the rungs march outward**, not the zoom. An onset push (`+0.13 – 0.15`, attack 0.08, release 0.55) jumps the ladder a notch. |
| `brightness` / `exposure` | `0.5 – 0.86` (+mid ≤ `0.12`) / `0.72 – 0.8` (+bass `0.2`) | Light mode only. The swing is kept to about a seventh. **Print mode binds no luminance at all**: the present writes `ink * coverage`, so any brightness term writes values between the inks. |
| `bloom_amount` | `0.22 – 0.3` (+onset `0.3 – 0.4`), threshold `0.9 – 0.95`, radius `1.3 – 1.6` / `0` | Light worlds bloom on the transient. Print worlds set bloom, `trails` and `echo_alpha` to 0, because each blurs the edges the runs exist to make. |
| `echo_alpha` / `echo_orient` | `0.18` rest, onset `+0.45` / `1` | Only Cauldron uses it: a left-right mirror blend that snaps symmetric on a hit and relaxes. It blends toward the copy, so it cannot brighten, and it writes nothing back, so it cannot accumulate (ADR-0119). |

**Four things to know before tuning one:**

- **`[smoothing]` cannot ease a `[per_vertex]` binding.** A series has no single value, so the
  engine warns and ignores the line. Keep audio inside `[per_vertex]` small, and prefer `bass`, the
  smoothest band (`+0.025 – 0.15` on zoom, `+0.04` on `dx`). Smoke's onset, treble and mid terms
  there are all multiplied by a height factor that is 0 at the source. Put the eased audio on the scalars.
- **A field lever is statistically invisible and a source lever is not.** Smoke's mid on `warp`
  and `warp_speed` alone read `0.000` on `--report`, because reshuffling wisps moves neither
  coverage nor luminance. Binding `deposit_twist` fixed it. Its bass on `deposit` alone *failed* the
  reactivity gate (0.014 against a floor of 0.02), because the deposit sits upstream of a slow
  accumulator. Adding `deposit_radius` and a small `brightness` term, which land in the same frame,
  passed. When a band column reads dead, give that band a source lever.
- **Every world starts with an empty field.** A two-second still shows the warm-up (Smoke's single
  finger, Sirocco's plume), not what runs on stage. The first horizon row is the fill, so read the
  rows after it. All seven headers carry a horizon verdict; copy the shape.
- **Write sharp features with `smoothstep`, not a comparison.** Interpolation between vertices is
  linear, so `rad > 0.5` renders as a polygon of the mesh. Shipped grids run 32x24 to 48x48, inside
  the Floor tier's 64x48 ceiling, so they render the same on every machine. A boil needs no more than
  the default 32x24. `wrap = "1"` only suits a tunnel that should never run out of material
  (Wellhead). On Sirocco it was measured to buy nothing, and on Cauldron it would show the pot's
  opposite edge.

---

## `shape_collage` — flat opaque elements on their own paper
*Posters, constructivist canvases, cut paper.* Family `collage` (`collage_*.toml`). **This is the
one scene that draws a graphic instead of light.** A pixel starts at the paper colour and composites
each element `over` it in array order, so one form genuinely sits in front of another and the array
index is the depth. Nothing is additive, so the additive ceiling does not apply. The rule that
replaces it is the palette's:

**Keep every element colour under the tonemap's knee: linear 0.6, which is sRGB byte `0xcb` in the
hex you write.** ADR-0046's curve is the identity below it, so a fill leaves the post chain unshaded.
Bloom's threshold sits above it, so the edges stay hard. All four shipped canvases obey it; reach
past it and the flat fill and the hard edge go together, silently. The paper is the one plateau
allowed above the knee, and pure white is unreachable anyway (`f(1.0) = 0.800`), so it is always
off-white or a dark ground (Nocturne's `#272930`).

**The palette is eight plateaus, not a gradient.** A layout grammar gives each element one of eight
band centres (`k/8 + 1/16`) and reserves the last for the ground. Stop pairs about 0.0002 apart are
the hard transitions, and a smooth ramp would shade every element. All four shipped files pin
`paper = "0.9375"`, the eighth centre, as a raw coordinate that `color_span` and `palette_shift`
cannot move. Weight the colours by repeating plateaus: Collage Mono gives five black slots to two
red, so the red arrives as an event.

| Param | Typical | Controls / natural driver |
|-------|---------|---------------------------|
| `layout` | `0`, `1`, `2` | `0` is the authored fourteen-element canvas (Suprematist) and ignores the four rows below. `1` is anchor-and-satellites, where the picture has a subject (Nocturne). `2` is diagonal-axis, where it has a direction (On White, Collage Mono). **`3` (size-hierarchy) ships in no preset**, so there is no working range for it. |
| `roster` | `0` or `1` | `1` is the Kandinsky vocabulary (bar, ring, segment, arc, checker, about one in four translucent) on top of the suprematist three. The two roster-1 canvases run 40 elements, and the two roster-0 ones run 14 and 17. |
| `count` | `14 – 40` | **40 is the Floor tier's element cap** (`TierConfig::collage_elements`). A 41st element is dropped silently. Nocturne needed 40 because anchor-and-satellites leaves paper showing: at 26 it covered 0.169 of the frame, too little for any lever to register. |
| `size_hierarchy` | `0.62 – 0.82` | Higher makes the leading forms dominate. It is what makes an anchor an anchor (Nocturne 0.82). Forty elements need at least ~0.6, or the canvas reads as gravel. |
| `angle_bias` | `-24`, or `-18 ± 6`, on diagonal; `14 ± 9` on anchor | Degrees. It wraps, so `sin(time * 0.04 – 0.05) * 6 – 9` gives a slow lean that never snaps. |
| `density` | `0.72 – 0.78` base, +mid `0.15 – 0.28` | Elements fade in and out over about half a second, in stable birth order, so a rise only ever adds. Mid is the natural driver; Nocturne adds onset `+0.3`. Smoothed 0.3 – 0.35 s. A denser canvas should start higher and travel less. |
| `scale` | `1.0` ± a `0.04 – 0.045` breath, +bass `0.08 – 0.2` | The whole canvas leans in. **Smooth it about 0.5 s**: at 0.15 the bass term reads as a zoom. |
| `pan_x` / `pan_y` | amplitude `0.06 – 0.15` / `0.04 – 0.10` | Rate chooses the character. `0.07 – 0.11` rad/s is a print not quite square to the wall (Mono, Suprematist). `0.52 – 0.77` is where Nocturne and On White get their idle motion. Use incommensurate x and y rates so the path does not repeat. |
| `drift` / `spin` | `0.55 – 1.3` +bass `0.4 – 0.8` / `0.3 – 0.7` +mid `0.3 – 0.5` | Multipliers on each element's own seeded travel and turn, so elements move against each other. They do not move the canvas as a whole; that is `pan_*`. Smoothed 0.6 s, or they stutter on each hit. Slower for a denser canvas. |
| `recompose` / `recompose_blend` | a `[latch]` / `0.45 – 0.9` | Edge-triggered on the rise past 0.5, so a latch with `hold = 0` is the right source. Three of four ship one armed in a window of a clock cycle and fired by an onset: 24 s on a dark ground, 100 – 130 s on paper, onset threshold 0.6 – 0.75. On White's older `hash(beat_index) > 0.93` gate has no period at all (ADR-0109), which is why the other three moved off it. A composition must stay still long enough to be read, so a slow cycle calls for a long blend. |
| `pump_size` / `pump_alpha` | bass `0.12 – 0.15` or onset `0.3` / bass `0.2` or onset `0.35` | Per-element swells with a per-element phase, so the canvas never pulses as one sheet. Only the depth is authorable. `pump_alpha` pays off on roster 1, whose translucent elements' crossings breathe. Use attack 0.03 and release 0.45 when an onset drives it. |
| `saturation` | `0.9 – 0.95` +treb `0.07 – 0.1` (Mono pins `1.0`) | **The treble goes on chroma, because there is no `brightness` param and nothing to blow out.** Rest the base under 1 so there is room to come up. |
| `opacity` / `edge_softness` | `1` / `0` | All four sit there, and two pin it explicitly. The hard analytic edge is the look, not a quality knob. |

**The family is onset-deaf by construction, and Nocturne is the fix.** A recompose that fires twice
a set cannot show in a measurement, so Plan 0104 read onset at 0.000 on the first three canvases. To
make a collage answer transients, put onset on a continuous lever (`density`, `pump_*`), not on the
event. Set `bloom_amount` and `trails` to `"0"` explicitly: either would put light between the flat
fills. A canvas covers every pixel, so the motion gate's mean takes the whole frame. Do not raise
pan or drift rates just to pass a measurement; Suprematist's header records the gate being fixed
instead.

---

## `analytic_field` — one fullscreen pass, a closed-form function of position
*Fractals and vibrating plates.* Family `analytic` (`analytic_*.toml`). The picture is a pure
function of the pixel's position: no state, no accumulation, nothing to warm up. `[field] family`
chooses between two different instruments that share a palette surface, and the params of the one
not chosen are inert. The `[field]` table (`family`, `map`, `trap`) is in `docs/presets.md`.

**`escape_time`: Julia and Mandelbrot sets** (9 shipped). Colouring is by smooth iteration count,
or with `trap` by the orbit's closest approach to a circle, line, point or cross.

| Param | Typical | Controls / natural driver |
|-------|---------|---------------------------|
| `c_re` / `c_im` | a region, then an audio swing of `0.01 – 0.06` | Choose the region first; it is the look. Shipped regions: the main cardioid's edge (Julia Circuit tours it), fixed dendrites (`-0.1 + 0.95i` Searchlight, `-0.8 + 0.156i` Pearl String), just past the cusp (`0.274`, Parabolic Dust), Seahorse Valley (`-0.762 + 0.085i`, Two-Band Julia), inside the cardioid (`-0.30 + 0.45i`, Stained Glass). **Keep the audio swing small and smoothed** (0.12 – 0.8 s). Near a boundary the set's size is very sensitive to `c`: Parabolic Dust's first 0.07 swing read as a chaotic zoom and shipped at 0.04. Bass is the natural driver. Two-Band Julia splits bass onto `c_re` and treble onto `c_im`. |
| `iterations` | `20 – 64` | Cost: every pixel pays it. **The Floor tier caps it at 64**, so stay at or under 64 and both tiers draw the same picture. With a trap, fewer is better. Stained Glass needs 20, because with more every orbit eventually grazes the cross. Seahorse drives it from bass (40 → 160) as the structural lever on the Mandelbrot map, which carves the black back. That is a Rich-only picture, and 400 dropped the app to 15 fps. |
| `interior` | `0`, `0.1 – 0.15`, or `1` | `0` is the textbook black set. `0.1 – 0.15` (+onset `0.12`) lifts it a touch. **Every trap preset sets `1`**, because a trap colours the interior too, and a black hole would punch out the filaments. |
| `trap_radius` | `0 – 0.25` base, +bass `0.4 – 0.8` | **The trap presets' bass lever**, in all four. Moving the trap re-cuts every filament or pane at once, a structural change rather than a brightness one. Smoothed 0.12 – 0.2 s. |
| `trap_rotate` | `time * 0.01 – 0.05` | A sweep (Searchlight's beam, Pearl String's sliding beads). Stained Glass adds a mid nudge (`+0.25`, eased 0.35 s). |
| `power` | `2`, `3`, or whole `3 – 7` | A whole power `p` gives p-fold symmetry. Multibrot re-rolls it per bar, held by `[hold] power = "bar"`, and its `c` radius has to grow with the power (0.53 + 0.085 per step), or high powers go to dust. No shipped preset uses a fractional power. |
| `escape_radius` | `24 – 64` | Larger smooths the band spacing. The shipped files that set it use 24 – 64; the rest leave the default 16. |
| `zoom` | `0.72 – 0.9` whole set; `1.85`; `3.6`; `~220` | Most frame the whole Julia set just under 1. Stained Glass sits at 1.85 and Parabolic Dust about 3.7x in on one spiral arm (with `pan_*` aimed at it). Seahorse is ~220x into the Mandelbrot map with pan at the valley and `pow(2.2, noise - 0.5)` breathing; f32 holds there. A mid nudge of ≤ `0.07 – 0.2` on zoom, eased 0.3 – 1.2 s. |
| `color_span` | `0.46 – 0.55` glow; `0.9 – 1.8` trap; `4` panes | Low for a continuous boundary glow (Julia Circuit, Parabolic Dust, Two-Band Julia). Higher for trap distances. 4 on Stained Glass only because its useful distances all sit under 0.1. |
| `brightness` | `1.0 – 1.4` | Constant in every escape-time preset: the music goes into `c`, the trap, or `iterations`, not into light (only `interior` takes a small onset lift). Five of the nine bloom at `0.3 – 0.4`, threshold `0.8 – 0.9`. |

**Palettes do the lighting.** A trap palette is bright only near distance 0 and near-black by
0.3 – 0.6, so only the filaments light (Ring Orbit, Searchlight, Pearl String). The palette repeats
past 1, so a trap's exterior shows as striped fringes and cannot be sent to one dark colour. Stained
Glass records that as an engine gap.

**`chladni`: the vibrating plate** (3 shipped). Nodal lines of two standing waves.

| Param | Typical | Controls / natural driver |
|-------|---------|---------------------------|
| `mode_n` / `mode_m` | low `1 – 3 / 4 – 7`; mid `7 – 10 / 11 – 15`; top `12 – 13 / 14 – 16` | The figure. Low reads as a figure (Echo Plate), the middle as a dense contour map (Standing Wave), the top as a textile (Lace Grid). Pick each with `floor(clamp(band, 0, 0.99) * k)`, and **hold it with `[hold]`** on `bar` or `beat`. Re-taking the figure every beat under feedback read as chaos, so Echo Plate holds on the bar. **`mode_n == mode_m` is a blank plate**; every shipped file keeps the two ranges disjoint so they can never meet. |
| `line_width` | `0.004 – 0.03` | Width of the lit nodal band in plate units. Lace Grid rides treble between 0.004 and 0.016, a hairline at 640x360. Judge the top mode range at 1280x720 too, because it sits near the pixel pitch and aliases at thumbnail size. |
| `plate_mix` | `0`, `0.55`, `1` | `0` is sand on the nodal lines only. `0.55` is a sheen under the thread. `1` is the whole signed wave, where the nodal lines become the black seams. |
| `palette_steps` / `palette_contour` | `6 – 11` / `0.55` | Standing Wave cuts the whole wave into terraces with a hairline at each edge, and rides the step count on mid, which changes structure rather than light. |
| `zoom` | `0.9 – 1.12` | Bass breathes it `+0.12` (Standing Wave, eased 0.25 s). A slow `pan_*` sine (amplitude 0.09 – 0.35 at 0.037 – 0.05 rad/s) keeps a held plate moving in a quiet passage. |

The plate draws no history; `trails` gives it one. Echo Plate runs `trails 0.955` with
`fb_zoom 1.04` (+onset `0.015`, eased 0.8 s) and turns it into a stack of the figures the music
played. On Lace Grid, bloom (`0.45`, threshold `0.9`) finds the crossings where two lines
overlap.

---

## `cellular` — a discrete cellular automaton on a ping-pong grid
*Colonies, embers, mazes, spiral chemistry.* Family `cellular` (`cellular_*.toml`). A real
automaton stepping on a grid of cells, and the one scene whose picture is the history of a rule.
The `[cellular]` table chooses `family` (`life_like`, `larger_than_life` or `cyclic`), `grid` (cells
per side) and `wrap`; it is in `docs/presets.md`. **`grid` is the stroke width.** Labyrinth's walls
read as a maze you could trace at 40 and as texture at 130. The shipped files run 40 to 256; 180 –
256 reads as a field.

**Pick the rule by sweeping, and record the sweep in the header.** Every shipped rule was found
that way, and the neighbouring settings fail in ways a still will not predict:

- **`cyclic`**: `states 3 / threshold 3` organises noise into rotating spirals within seconds
  (Spiral Bloom). `12 / 1` gives travelling wavefronts and a marbled damask (Wavefront). `8 / 3`
  never organizes, and the higher state counts fold into blocky terraces. Wavefront takes those
  terraces as its subject. Give a cyclic palette a **closed loop** (the last stop equals the
  first) or the wrap shows a seam.
- **`life_like`**: `birth` / `survive` are bitmasks over neighbour counts. `8 / 12` is Conway's
  B3/S23 (Ember Life). `8 / 30` is mazectric B3/S1234, which runs long straight corridors, where
  S12345 (`62`) only knots into short stubs (Labyrinth).
- **`larger_than_life`**: `radius 5` with the default birth/survive windows grows crawling blobs
  (Tide Bugs). The Floor tier caps `radius` at 6.

| Param | Typical | Controls / natural driver |
|-------|---------|---------------------------|
| `step_rate` | `5 – 10` base, +band `8 – 14` → `5 – 24` gen/s | **The scene's main audio lever: tempo of life.** Mid in three presets (busy music runs the colony faster), bass in two (Wavefront, Tide Bugs, where the tide moves on the low end). Smoothed 0.3 – 0.35 s. |
| `reseed` | `max(event, mod(time, T) < 0.2)` with `T = 5 – 15` s | **Every shipped preset has a clock floor, and the floor is not optional.** Left alone, Tide Bugs' rule froze into ~30 still rings within a minute (motion 0.000 from 60 s on), and Life settles into still-lifes. The event is either `onset > 0.75 – 0.8` or `beat * (hash(beat_index * k) > 0.4 – 0.66)`, a seeded fraction of detections. A rise past 0.5 drops one disc of soup per rise, so a sustained 1 does nothing extra. |
| `trail` | `20 – 26` base, +bass `12` | How long a dead cell glows (`life_like`, `larger_than_life`). Bass holds the embers longer (Ember Life, Labyrinth). Smoothed 0.4 – 0.45 s. |
| `age_tint` | `0.7 – 0.9` | How far along the palette a wake travels as it fades. **The palette runs from the live colour at 0 to char at the far end**, so a shipped ramp reads hot → cold. Labyrinth's white-origin palette is two-tone at rest because every wall is live. The warm bands only bloom where a reseed has bitten. |
| `saturation` | `0.55 – 0.95`, +mid or treb `0.24 – 0.45` | A chroma lever that works here as on the collage. Wavefront's mid moves it from ashen to full magenta. |
| `palette_mix` | `smoothstep(0.45, 0.8, treb)` or a slow noise | Wavefront throws the plate between two complementary loops on treble. **A mix parked halfway averages complementary loops to grey**, so the smoothstep sits between treble's typical level and its peak, which makes the plate snap rather than rest. |
| `hue` | `time * 0.008 – 0.012`, or `noise(time * 0.025 – 0.03) * 0.05 – 0.06` | A cyclic loop can walk the whole ring. A life ramp only wanders a few percent, or the fire stops being fire. |
| `brightness` | `0.8 – 1.0` | Constant in all five. |
| `zoom` | `1.0 – 1.05` | Tide Bugs breathes it on bass (`+0.05`, eased 0.12 s). |
| `palette_steps` / `palette_contour` | `4` / `0.5` +treb `0.3`, style `3` | Only Labyrinth uses them. The engraved edge sharpens on treble, and it shows only in a bitten region. |

**The horizon is the check for this scene** (ADR-0099): a rule either breathes or settles, and
half a second shows neither. Ember Life and Spiral Bloom record ten-minute verdicts, both alive. Run
one on any new rule before trusting it. Tide Bugs' header says mid widens `birth_hi`, but the file
binds it as a constant `"0.385"`. No shipped preset drives a birth/survive window, so there is no
working range for that lever yet.

---

## Engine-wide stages (any system)

| Param | Default | Note |
|-------|---------|------|
| `zoom` / `pan_x` / `pan_y` | `1` / `0` / `0` | camera **in** on line/swarm/attractor; **out** (shows more) on fragment/RD. Not declared everywhere: no `zoom` on `shape_field`/`shape_collage`, no `pan_*` on `warp_mesh`. |
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
