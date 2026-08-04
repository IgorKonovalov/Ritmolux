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

**Author against the floor tier.** Shipped presets are authored and gated on
`Floor` — every `shot` capture and every CI gate (`sanity` / `reactivity` /
`animation` / `distinctness`) pins it (Plan 0044 /
[ADR-0045](../docs/adrs/0045-quality-tiers-floor-and-rich.md)). `Rich` raises
**capacity**, not behavior: more particles, more segments, a larger internal grid.
No expression, param or structural field changes meaning, and nothing in the
grammar can read the tier. The one edge worth knowing is the segment cap — it is a
tier value, so geometry that overflows and truncates at the floor's 20 000 may fit
at rich. Compose so the floor's cap is the one you tuned for, and treat the extra
headroom as headroom.

> **That claim was false for the `attractor` family until Plan 0057, and how it
> was false is worth knowing** ([ADR-0065](../docs/adrs/0065-the-attractor-deposit-is-normalized-by-particle-count.md)).
> The attractor draws its particles with an **additive** blend into a linear
> accumulation, so 150 000 points at `Rich` deposited three times the light of
> 50 000 at `Floor` into the same texels — same `fade`, same `size`, a picture
> three stops hotter. For an accumulating scene, capacity *is* the picture; and no
> capture in this project could render `Rich` to notice. Four shipped presets were
> retuned **downward** to survive it (commit `00d99d0`), which is a compensation
> for an engine defect carried in content.
>
> The deposit is now divided by the particle count, so the total light laid down
> per frame is invariant. At `Floor` the factor is exactly `1.0` and nothing moves;
> at `Rich` the same figure is drawn from three times the samples at a third the
> weight each. **A tier now buys less shot noise in the same picture, which is what
> a capacity tier was always supposed to buy.** Measured on `Clifford` at
> 1280x720: mean display luminance `17.37` at `Rich` before the change, `10.86`
> after, against `Floor`'s `10.34`.
>
> Two things follow for you. A preset can no longer buy brightness by running at
> `Rich` — it never could on purpose, but it is what the shipped pictures did. And
> anyone comparing a pre-Plan-0057 `Rich` screenshot will find the new one dimmer:
> that is the fix, not a regression. Capture both tiers with
> [`shot --tier`](../docs/capturing.md#captures-pin-the-floor-tier) rather than
> reasoning about it.

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

- **Variables** (19): `bass mid treb onset beat bar time tempo novelty
  bass_raw mid_raw treb_raw onset_raw beat_index time_since_beat beat_in_bar
  bar_index bar_phase index`. `beat` is 0/1; `bar` is the 0..1 **beat** phase
  despite the name (`bar_phase` is the real one); `time` is seconds; `tempo` is
  **BPM**, not a 0..1 band; `novelty` is an experimental track-change transient;
  `index` is the element's own 0..1 position in a per-element binding, and `0`
  everywhere else. The four `*_raw` escapes carry the pre-v2 absolute magnitudes
  (see below). The five musical-time variables are ADR-0050's: `beat_index` and
  `time_since_beat` are always tracked; `beat_in_bar`, `bar_index` and `bar_phase`
  come from a **gated** downbeat estimator and are counter-derived whenever it is
  not confident — which, measured on real music, is most of the time, so build an
  arc on `beat_index` and treat the bar trio as decorative.
- **Functions:** `sin cos abs floor sqrt log min max pow mod clamp lerp
  smoothstep select bin hash noise` (17). `log` is the **natural** log; `bin(x)`
  reads the spectrum at a normalized position; `hash`/`noise` are the seeded
  randomness below.
- **Constants:** `pi`, `tau`.
- **Comparisons:** `> < >= <= == !=`, each yielding `1`/`0`, plus
  `select(cond, x, y)`. No boolean operators — `min` is and, `max` is or,
  `1 - c` is not.

Full grammar reference: [`../docs/presets.md`](../docs/presets.md#the-expression-language).

### What range the bands actually occupy

**Since ADR-0049, `bass`/`mid`/`treb`/`onset` really are `0..1`.** Each is a
fraction of that signal's *own slowly-decaying recent peak*, so `> 0.5` means
"loud for this track" — on any track, at any gain, under any stimulus. This is the
single biggest change to authoring in the project's history, and it deleted a
whole class of defect: a threshold can no longer be quietly unreachable because
the levels turned out to be a hundred times smaller than they looked.

| variable | min | mean | max | note |
|---|---|---|---|---|
| `bass` | 0.035 | **0.661** | 1.000 | |
| `mid` | 0.031 | **0.575** | 1.000 | |
| `treb` | 0.002 | **0.281** | 1.000 | |
| `onset` | 0.001 | **0.145** | 1.000 | normalized envelope now, not raw flux |
| `bass + mid + treb` | 0.078 | 1.517 | 3.000 | the three-band sum, per hop |
| `bin(x)` | 0.000 | **0.089** | 1.000 | still on its own scale — see below |

Measured 2026-07-30 over `--signal dynamic:110`. **Re-measure rather than guess**
— any filmstrip run prints the band table:

```sh
cargo run -p standalone --example shot -- --signal dynamic:110 --out strip.png
```

Three things to carry from that table.

**Full scale is now reachable.** All four hit `1.000`, so `--set bass=1` is a
*peak*, not the 100×-a-real-mean fiction it used to be. Calibrating against a
`--set` capture is finally reasonable — with the caveat that you are looking at a
held peak, so a gate that only fires there fires rarely.

**`bin(x)` is still on its own scale, for a new reason.** The band array
normalizes against one peak shared by all 64 bands (which is what keeps a
`bin(hi) - bin(lo)` contrast meaningful), so a single band only reaches `1.000`
when it *is* the loudest in the frame. Its typical value is `0.089`, an order of
magnitude under the scalars. A threshold that works on `bass` will be far too high
on `bin()`.

**The `*_raw` escapes read on the old scale.** `bass_raw`, `mid_raw`, `treb_raw`
and `onset_raw` are the pre-v2 magnitudes, unchanged — means of `0.040 / 0.006 /
0.006 / 0.002`. Reach for them only when a look genuinely wants absolute loudness
(a quiet track *should* look quieter), and when you do, the old warnings below
apply to them in full.

Comparison gates written against magnitudes on the wrong scale are **dead code**:
six shipped presets had their defining mechanism disabled for months because their
thresholds sat above anything music produces, and every one of them looked healthy
in
`--report`. Two rules follow:

- **A threshold goes between the mean and the max of what it reads.** Above the
  max it never fires; below the min it always does — and *both* halves of that bite.
  On the v2 scale `bass + mid` has a mean of `1.24` against a max of `2.0`, so
  `select(bass + mid > 1.5, 12, 6)` is a live gate while `select(bass + mid > 0.085,
  12, 6)` is the constant `12` and `> 2.4` is the constant `6`. Pre-v2 the
  low-threshold failure was rare because the levels were tiny; now it is the
  commoner mistake, and it is the one Plan 0048 Phase 7 found in nine shipped
  bindings and retuned.
- **Calibrate a continuous parameter against the mean and a percussive one
  against the peak.** A zoom or a hue drift spends its life near the mean; a
  flash or a burst exists to fire on the hit.
- **A GAIN can be wrong the same way.** `clamp(band * G, 0, C)` reaches its
  ceiling at `C / G`; if that is below the typical level, the term is a constant
  no matter how reactive it reads. Phase 7 found **263 of 332** clamped band terms
  in that state at once. **This one is now checked** — `--report`'s `occ` column
  names the binding and `core/tests/saturation.rs` fails the build on it (Plan
  0056 / [ADR-0062](../docs/adrs/0062-clamp-occupancy-is-the-saturation-instrument.md),
  and the `[occupancy]` table [below](#a-clamp-is-a-limit-not-a-gain--the-occupancy-table)).
  Do the division while composing anyway: the gate fires at occupancy `0.9`, so a
  term pinned for half a track passes it. The rule that came out of the retune:
  pick `G = C / 0.85` for `bass`/`mid` and `C / 0.60` for `treb`/`onset`, which
  puts a typical passage near half the cap and a peak at it.

`--report` now checks all of this for you: the second table reads at these levels,
and its `gates` / `ceils` / `occ` columns name any `select()` that never went both
ways, any `clamp()` ceiling the value never reached, and any `clamp()` that never
let its ceiling go
([`../docs/capturing.md`](../docs/capturing.md#reachability-gates-the-probe-never-drove-both-ways)).

### Seeded randomness — `hash`, `noise`, and `[generator] seed`

`hash(x)` scatters (neighbouring arguments unrelated, `[0, 1)`); `noise(x)`
wanders (smooth over one unit of `x`, `[0, 1]`). Both are **pure functions of the
argument and the preset's seed** — same input, same frame, same number.

```toml
[params]
hue       = "noise(time * 0.3)"                 # organic drift, not a ramp
burst     = "0.4 + hash(floor(time * 2)) * 0.6" # a new value twice a second
thickness = "2 + hash(index * 64) * 3"          # scatter a per-element readout

[generator]
seed = 12          # any non-negative integer: the same look, every time
# seed = "random"  # a different look every time the preset loads
```

`"random"` redraws on every **load** — app start, and every hot reload of the
preset folder — so while you are editing a file it re-rolls on each save.

- **Prefer `noise(time * k)` to a sum of detuned sines** in new presets. The
  older files fake a wander with three or four sines whose periods do not line
  up (`attractor_dejong` has four); one `noise` call is shorter and has no period
  at all. Existing ones are fine as they are — this is not a rewrite.
- **Sum `noise` calls at different rates** for a richer wander:
  `noise(t*0.11)*0.6 + noise(t*0.43)*0.3`. There is no octave machinery and does
  not need to be.
- **Offset the argument, not just its scale**, to decorrelate two parameters:
  `noise(time * 0.2)` and `noise(time * 0.2 + 50)` wander independently.
- **`seed` is not an L-system key** despite living in that table. Any preset of
  any system may carry a `[generator]` table holding nothing but a seed.

> **`seed = "random"` is invisible to the harness.** Every capture path — `shot`,
> the goldens, `--report`, the behavioral gates — forces the numeric fallback
> (`0`), so a filmstrip of a random-seeded preset shows *an* instance, not the one
> the app will draw. Tune with a number; switch to `"random"` last, if at all.

## Systems and their named parameters

| System            | Named `[params]`                                                         |
|-------------------|--------------------------------------------------------------------------|
| `fragment_field`  | `warp` `hue` `zoom` `glow` `flash` · `pan_x` `pan_y` · `saturation` `color_span` `color_center` `palette_mix` |
| `swarm`           | `force` `spin` `burst` `field_freq` `hue` `brightness` `size` · `zoom` `pan_x` `pan_y` · `saturation` `hue_spread` `hue_center` `palette_mix` |
| `parametric_curve`| `n` `d` `phase` `samples` `thickness` `hue` `spin` `scale` `radial_offset` `brightness` `glow` `draw_progress` · `zoom` `pan_x` `pan_y` `mirror_order` `mirror_reflect` · `saturation` `hue_spread` `palette_mix` |
| `lsystem`         | `visible_depth` `rotation` `hue` `draw_progress` `thickness` `scale` `brightness` `glow` · `zoom` `pan_x` `pan_y` `mirror_order` `mirror_reflect` · `saturation` `hue_spread` `palette_mix` |
| `star_pattern`    | `variant` `rotation` `hue` `draw_progress` `thickness` `scale` `brightness` `glow` · `zoom` `pan_x` `pan_y` `mirror_order` `mirror_reflect` · `saturation` `hue_spread` `palette_mix` |
| `reaction_diffusion` | `feed` `kill` `flow` `inject` `hue` `contour` `hatch` `glow` · `zoom` `pan_x` `pan_y` · `saturation` `color_span` `color_center` `palette_mix` |
| `attractor`       | `a` `b` `c` `d` `size` `hue` `fade` `reseed` · `zoom` `pan_x` `pan_y` · `saturation` `hue_spread` `hue_center` `palette_mix` |
| `spectrum`        | `base` `scale` `curve` `span` `baseline` `radius` `rotation` `thickness` `hue` `brightness` `glow` · `zoom` `pan_x` `pan_y` `mirror_order` `mirror_reflect` · `saturation` `hue_spread` `palette_mix` |

Unbound parameters fall back to each system's defaults. An **unknown** parameter
name is reported as a load-time warning naming the param and the system — the
preset still loads and its other bindings apply (ADR-0020). The params after the
first `·` are the shared **view transform** and
line-**mirror** controls (Plan 0018) — see [Engine-wide controls](#engine-wide-controls-plan-0018);
the trailing group on the palette-coloured scenes (the four shader ones, plus
`spectrum` since Plan 0034) is the shared **palette** colour surface (Plan 0020) —
see [Colour — the palette surface](#colour--the-palette-surface-plan-0020).
Every system additionally accepts the engine-stage params `bg_*`, `trails`,
`kaleido_*`, `bloom_*`, `exposure`, and the final `ink_*`/`paper_*` remap
documented there.

### Attractor detail sharpness (Plan 0027)

The `attractor` accumulates its trails into an offscreen grid sized to what it is
drawing into, up to 2560x1440 — so at 1080p the filaments are pixel-crisp rather
than upscaled from the old fixed 640x360 grid. Nothing to bind: it follows the
window. Two authoring consequences: `size` now reads *finer* at high resolution
(a value tuned when the field was soft may look thin — nudge it up), and turning
`trails` or `kaleido_*` on routes it through those stages' grid instead. Since Plan
0033 that grid follows the render target too, so stacking them no longer softens the
attractor the way it did against the old fixed 1280x720 — the two caps still differ,
so a very large window resolves the attractor alone slightly finer.

### Swarm flow-field structure (Plan 0043)

`field_freq` is the swarm's **spatial** lever and the one that decides how many
distinct streams a frame can hold. The scene steers every particle by a scalar
potential sampled at `world * field_freq`, so it sets the size of a current
relative to the frame. Default `2.3`, which is the constant it replaced — a preset
that does not bind it is unchanged. Measured across the range on `Drift`:

| value | what the frame does |
|---|---|
| **0.8** | the field's structure is *larger than the frame*, so there are no legible channels at all — a uniform dense mass with one slow convergence front migrating across it over tens of seconds |
| **2.3** | broad rolling currents with visible density waves: crests gathering, wakes thinning |
| **6.0** | a few sharply resolved sinuous ribbons with genuinely empty dark space between them |

Note which direction reads as *busier*: it is the low end, not the high one. A
high `field_freq` does not make confetti — it separates the swarm into distinct
streams and gives back negative space, while a low one packs the frame edge to
edge because every particle is inside the same enormous current.

Read it against `spin`, which is the **temporal** lever and a different question
entirely: `field_freq` says how finely the field is divided, `spin` how fast it is
rewritten. The family's apparent flocking is not a per-particle rule — it emerges
from neighbours falling onto the *same* streamline and travelling together — so a
coarse field (low `field_freq`) with a near-frozen one (low `spin`) is the
formation-holding end, and raising either dissolves the flock toward shimmer.

Two things the swarm does on its own, with nothing to bind.

The **toroidal domain follows the render target** and extends a quarter past the
frame, so the wrap seam is off-screen. That is what retired the bright top/bottom
bar, and it is why `zoom` below `1.0` no longer exposes a hard domain rectangle —
the camera is usable down to about **`0.84`**. Note that figure is the *near depth
layer's*, not the domain's: `1.25 * zoom` alone would reach the frame edge at
`0.80`, but the near layer takes 1.25x of the zoom deflection too, so it is the
first thing to show an edge and it does so at `1.25 * (1 + (zoom - 1) * 1.25) = 1`.
Pan pulls that further up — see the arithmetic in `swarm_drift.toml`.

And **every particle carries a depth**, 0 far to 1 near, fixed for its life from
the seeded scatter. It scales the sprite (0.55x–1.5x), fades brightness with
distance, gives the particle its own parallax against `zoom`/`pan_*` — a near one
traverses the frame about 1.9x faster than a far one, so panning now sweeps the
near field past the far one instead of sliding a flat sheet — and offsets *which
current it rides*, so the layers follow different streamlines rather than the same
ones at several sizes. There is no sorting and no perspective: the scene blends
additively, so draw order does not matter, and two particles at different depths
that overlap simply sum. That means **no occlusion** — the illusion is strongest
on a sparse field and flattens as density rises, so a very dense preset is the one
place depth reads weakest.

Both cost visible density — the margin puts about a quarter of the particles
off-screen, and the fade dims the far half — which is priced into the shipped
presets' `size` and `brightness`. A swarm preset carried over from before Plan
0043 will read dimmer and sparser than it used to; raise those two rather than
assuming something broke.

### Line-art parameter notes — strokes, joins, and per-scene shape

**Strokes join at their interior vertices** (Plan 0039,
[ADR-0041](../docs/adrs/0041-line-joins-are-per-endpoint-on-the-segment-instance.md)).
A line scene builds its figure as a list of segments, but a *connected* run of
them renders as one continuous stroke: where the stroke turns, the quads either
side of the vertex overlap instead of leaving the wedge of background they used
to. There is no parameter for this and it is not authorable — each scene declares
its own connectivity, so a genuinely separate segment keeps its exact ends and
does not grow. A `spectrum` bar still stands exactly on `baseline`, a radial
spoke still starts exactly on `radius`, and an L-system branch still begins where
it begins.

Two things follow for authoring. **A thin stroke is a real option now**: the dark
tick that used to cross every vertex scaled with `thickness`, so a figure with
sharp turns — a `polyline` spectrum above all — had to be drawn heavy to hide it,
and that constraint is gone. And **a very sharp turn reads slightly bright**
rather than broken, since the two overlapping quads add under the additive blend.
That is the trade ADR-0041 took in place of a miter limit; it is far less visible
than the gap it replaced, but if a near-reversal is meant to read as a fine point
rather than a bead, soften the turn rather than the stroke. On `star_pattern` the
turn is `[generator] contact_angle_deg`, not a `[params]` binding: a low angle
makes the outer points sharper, so their studs tighten and the point blunts by
about a half-width. The bead is most distinct in the middle of the range, where
the two strokes leave the vertex far enough apart to overlap in a compact spot
instead of along the whole point.

- `thickness` — stroke weight (roughly 1–5); scaled to a projector-friendly glow.
  Pick it for the weight you want: it no longer trades against a joint artifact.
- `glow` — the line renderer's **per-segment falloff** multiplier, default `1.0`
  (exactly what these scenes drew before it was bindable), whole-figure on all
  four. It scales the shader's core-to-edge term straight into the stroke colour:
  below `1` the stroke thins toward a dim hairline, above `1` it saturates the core
  and only widens the visible skirt. **There is more range downward than upward** —
  measured per lit pixel, `1.0 → 0.3` moves a rose about 0.25 while `1.0 → 2.5`
  moves it 0.17 — so reach for `glow < 1` when you want the figure to recede.
  **It is not the same quantity as `glow` on `fragment_field` or
  `reaction_diffusion`**, which is a term inside those shaders; the name is shared,
  the meaning is not, and a value that reads well on one will not transfer.
  Also **not** a post-process bloom — there is no bloom stage in the engine.
- `hue` — where the figure sits in the palette (add `time * k` for a slow drift).
  With no `[palette]` that palette is the line scenes' own cosine ramp, and **it
  is not a hue wheel**. The measured swatch table is in
  [`docs/preset-palettes.md`](../docs/preset-palettes.md#the-line-scenes-cosine-ramp--what-hue-actually-looks-like);
  read it rather than guessing, because a guess costs a render round-trip.
  `hue` is no longer the only colour control — see the colour axes below.
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
  `rotation` is an angle in radians, so multiply by `time` yourself. Its
  `hue_spread` walks the palette by **generation depth** — see
  [the colour axes](#the-line-scenes-colour-axes--what-hue_spread-walks) below.
- `star_pattern`: `variant` is a **continuous contact angle** in `0..2` (clamped),
  not an index — see [below](#star_pattern-variant--a-continuous-contact-angle);
  `rotation` is an angle in radians. Its `hue_spread` axis is radius, and it is
  inert on the current rosette — see
  [the colour axes](#the-line-scenes-colour-axes--what-hue_spread-walks).

#### `star_pattern` `variant` — a continuous contact angle

Since Plan 0054 / [ADR-0060](../docs/adrs/0060-star-pattern-variants-interpolate.md)
`variant` sweeps the Hankin construction's **contact angle** rather than indexing
one of three precomputed rosettes. `0` is 24° below the `[generator]
contact_angle_deg` (a sharper star), `1` is the base angle, `2` is 24° above (a
blunter one), the whole thing clamped into the 8–80° range that makes a sensible
figure. Values outside `0..2` clamp to the ends.

**`0`, `1` and `2` still name the same three rosettes they always did**, so an
existing preset is unchanged. What is new is everything between them:

- **`[smoothing]` on `variant` now morphs** instead of stuttering. Easing it used
  to spend its time on fractional values a `floor` threw away, which is why both
  shipped star presets `floor` it and why their comments told you not to smooth
  it. That advice is now backwards.
- **A `floor` around `mod(…, 3)` is no longer the way to drive it.** The shipped
  presets still carry one, and removing it *alone* would be worse than the cut it
  replaces — a sawtooth snaps 2 → 0 at every wrap. Drive it with something
  continuous and closed instead: a triangle wave (`1 + sin(time * 0.14)` is the
  cheapest one), or a band term easing between two angles.
- **The morph is quantized to 0.1° of contact angle**, so the figure rebuilds only
  when the request has walked that far. That is 480 steps across the whole
  `variant` range and at most a 1.1 px jump at 1080p on the sharpest rosette — a
  resolution, not a shape. A `variant` jittering inside one step does not rebuild
  at all.

#### The line scenes' colour axes — what `hue_spread` walks

Every line scene honours `[palette]` / `[palette_b]` / `palette_mix` /
`hue_spread` / `saturation`
([ADR-0059](../docs/adrs/0059-line-scenes-colour-along-their-generator-axis.md)),
sampled on the CPU per segment. `hue` places the whole figure in the palette;
`hue_spread` says how far the palette travels **across** the figure, and each
generator walks the axis its own construction makes meaningful. **The axis is a
property of the scene, not a parameter you pick** — this table is the thing you
cannot infer from the param names:

| System | `hue_spread` walks | `0` is | `1` is |
|--------|--------------------|--------|--------|
| `parametric_curve` | **position along the traced path** — chord `0` is the walk's first | one flat `hue` | first chord at `hue`, last a full palette away |
| `lsystem` | **generation depth** — branch nesting, `0` on the trunk and one more per open `[` | one flat `hue` | trunk at `hue`, deepest twigs a full palette away |
| `star_pattern` | **radius from the rosette centre** — but see the warning below: **inert today** | one flat `hue` | one flat `hue` (no radial spread to walk) |
| `spectrum` | **band index** — element `0` is the bottom of the spectrum | one flat `hue` | low band at `hue`, top band a full palette away |

`hue_spread = 0` is the default on every one of them and reproduces exactly the
single flat colour these scenes drew before the palette reached them, so adding
a `[palette]` to an existing preset changes its colours and adding nothing
changes nothing.

**On `lsystem` the axis is the figure's own, so a grammar without branches has
nothing to ramp along.** `lsystem_fern` reaches generation 11 at
`visible_depth = 6`; `lsystem_arrowhead` has no `[` in its rules at all, so every
segment of it sits at generation 0 and `hue_spread` is a no-op there however
large. That is a property of a Sierpinski arrowhead — every segment genuinely is
at the same recursion level — not a missing feature. Such a preset still reaches
`[palette]`; what it cannot reach is a ramp. The ramp is normalized over the
**figure's own** deepest generation, so `hue_spread = 1` spans the palette once
whatever the grammar's branching factor and whichever depth is visible.

**On `parametric_curve` the divisor is `samples`, not the revealed prefix.** So a
`draw_progress` riding `bar` *draws the gradient on* — a half-revealed curve shows
the palette's first half — rather than re-tinting every chord it already drew.

> **`hue_spread` does nothing on `star_pattern` today, and that is measured.** A
> Hankin rosette is `2n` congruent segments about the frame centre, so every one
> of them occupies the *same* radial interval: the spread of segment radii across
> the whole figure measures `1.2e-7` (f32 noise) at every tiling order and every
> contact angle, including both shipped presets and all their variants. There is
> no range for a radial ramp to walk, so the ramp collapses to the flat `hue`
> rather than sweeping on noise. What `star_pattern` *does* gain is `[palette]`
> itself — the rosette can be an ember or an ice figure instead of a point on the
> built-in cosine — plus `saturation` and `palette_mix`. What is empty is the
> rosette's **interior**: at `star_rosette`'s 12-fold / 20° the strokes live
> between radius 0.54 and 0.90, so the inner 60% of the disc is bare, and
> `star_lantern`'s 55° variant empties 87% of it. That is the open half of
> design-backlog 0007 and it is a generator question, not a colour one; the ramp
> comes alive by itself the day a construction puts segments at different radii.

### `spectrum` — the frequency-axis readout (Plan 0034)

A line system like the three above, but its figure is not a generator's geometry:
it is the engine's log-spaced band array, divided into elements by the
[`[spectrum]`](#spectrum--for-spectrum) table. Element 0 is the bottom of the
spectrum and the last is the top, so **nothing in `[params]` maps audio to
position** — that mapping is the scene. What the params say is how the elements
look and how far they reach.

- `base` — the length every element has *before* any audio, in world units
  (the frame is 2 units tall). Deliberately not zero by default: at zero the
  readout vanishes completely in a silence, which reads as a broken preset. Bind
  it to `time` for a resting breath.
- `scale` — multiplier on the element's own band level. The bands read **small**
  on real music, the same caveat as `bass`/`mid`/`treb`, so useful values are
  well above 1.
- `curve` — the level-shaping **exponent**, default `1.0` (exactly linear, the map
  this scene had before it existed). `level^curve`, applied to the element's raw
  level *before* the easing; `0.5` is a square root and lower values compress
  harder, which is how you get a dB-like readout where the quiet elements are
  legible instead of stubbed. Per element, so it can be walked with `index`. The
  level is floored at `0` and the exponent clamped to `[0.05, 4.0]`, so no
  expression can produce a `NaN` length.
- `radius` — **`radial_ring` only**: the inner circle the spokes stand on. No
  effect on `bars` or `polyline`.
- `span` — the figure's **half-width in world units**, default `1.0`.
  **`bars` and `polyline` only** (the ring is sized by `radius`).
- `baseline` — the world y the elements stand on, default `-0.85`, so bars grow
  upward from near the bottom edge. **`bars` and `polyline` only.**

**`curve` and `scale` are not independent, and the factor is large.** Measured
typical band levels are ~0.02–0.05. At `curve = 0.5` a level of `0.03` becomes
`0.173` — a **5.8x** boost — so a preset adopting a curve has to bring `scale`
down by roughly that factor or the readout leaves the top of the frame. That is
why the default is exactly `1.0`: a curve is opt-in, and opting in means retuning
`scale` in the same edit. Say so in the preset's header comment, with the factor.

**What `curve` does *not* disturb is the timing.** The easing runs on the curved
value, which is the value you see, so a fall's time constant is exactly the
`release` you wrote — at `curve = 1.0`, at `0.5`, at `0.25`. The two knobs are
independent *in time*; what a curve changes is amplitude. (Curving after the
easing instead would have made the effective release `release / curve`, silently
doubling every fall at `curve = 0.5`. That ordering was measured and rejected —
[ADR-0040](../docs/adrs/0040-spectrum-level-curve-applies-before-the-easing.md).)
Neither ordering produces an *even* fall: a one-pole is exponential, so it covers
the first half of its travel in about 30 % of its settling time at any `curve`.

**How wide the readout is, and why `span` is not a "fill the frame" switch.** The
line renderer divides world x by the target's aspect — the same rule every line
scene follows, and the reason a `radial_ring` comes out a circle rather than an
ellipse. `span` is therefore a **world** quantity, not a fraction of the frame:
the default `1.0` makes `bars` and `polyline` span the frame's **height** in
pixels, which is about **56 % of its width at 16:9**. `span ≈ 1.78` fills a 16:9
frame edge to edge — and leaves an ultrawide short, because 1.78 is still 1.78
there. That is correct behaviour for a world quantity and the reason there is
deliberately **no `fit`/`auto` mode**: a scene that sized itself from its render
target's aspect is exactly the trap
[ADR-0037](../docs/adrs/0037-internal-grid-is-a-resolution-not-a-shape.md) exists
to forbid, and it has shipped twice in this codebase. Pick the `span` that suits
the frames you care about, or bind it and accept that the number means world
units. Note `zoom` is still not a substitute for either lever — it scales y with
x, so widening the comb also lifts it.

**`baseline = 0` is how you get a centre-mirrored readout.** With the default
`-0.85`, `mirror_reflect` produces two combs pinned against the top and bottom
edges growing toward each other — which reads as a bug and is not. The geometry
mirror reflects across the **x-axis** on every line scene, so a figure standing at
`-0.85` reflects to one hanging from `+0.85`. Move the feet onto the axis and the
same mirror gives the symmetric "landscape and its reflection": bars up, their
copy down, sharing one foot line on the frame centre. `pan_y` cannot substitute —
the mirror runs on world coordinates in `update()` while the view transform is
applied later in the shader, so panning moves the mirrored pair together.
- `rotation` — turns the whole figure about the frame centre, in radians, on
  every layout. On the ring it is the natural motion; on bars and the polyline it
  tilts the readout, which is what makes those two worth folding with `mirror_*`.
- `hue_spread` — walks the palette across the elements: `0` (the default) colours
  the whole figure one hue, `1` spans the full palette from the lowest element to
  the highest, so you can see *where* a peak is without counting positions. On
  `radial_ring` a spread of exactly `1` makes the wrap continuous in colour as
  well as in position.

**Per-element bindings (Plan 0034 Phase 4).** This is the one system where a
single `[params]` expression can say something *different about each element*. A
binding whose text names `index` — the element's normalized `0..1` position along
the frequency axis — is evaluated once per element instead of once per frame:

```toml
[params]
thickness = "0.01 + bin(index) * 0.05"   # thick where that element's band is loud
base      = "0.16 + index * 0.12"        # a longer rest toward the quiet top end
hue       = "index * 0.3 + time * 0.02"  # colour walked by hand instead of hue_spread
```

The params that genuinely vary per element are the ones describing a single
element — `base`, `scale`, `curve`, `thickness`, `brightness`, `hue`. The rest
describe the whole figure (`span`, `baseline`, `radius`, `rotation`, `glow`,
`hue_spread`, `palette_mix`, `saturation`, the view transform, the mirror), so a
series aimed at one of those takes its `index = 0` value rather than being
dropped. `index` reads `0` on every other system, and a `[smoothing]` entry naming
a per-element binding is a surfaced **warning** — ease the levels with
[`[spectrum] smoothing`](#spectrum--for-spectrum) instead. Full semantics in
[docs/presets.md](../docs/presets.md#index--one-binding-evaluated-once-per-element).

**What this system honors**, since a silent no-op would be worse than an absence:
the shared view transform, the geometry mirror, the palette surface
(`[palette]`/`[palette_b]`/`palette_mix`/`saturation`, sampled per element — this
is the **only line system that reads `[palette]`**; the other three still colour
from the built-in cosine), and all the engine stages (`bg_*`, `trails`,
`kaleido_*`, `bloom_*`, `exposure`, `ink_*`). On `radial_ring` the geometry
mirror is close to a no-op
for the same reason it is on `star_pattern` — the figure is already rotationally
symmetric about the frame centre, so the copies land on the original. On `bars`
and `polyline` it is genuinely transformative.

## Engine-wide controls (Plan 0018)

These sit in the **render composite** (ADR-0018), not in one scene, so they are
audio-bindable like any other param. All default to *off/identity*, so a preset
that binds none renders exactly as before.

### Shared view transform — `zoom`, `pan_x`, `pan_y`

A camera zoom about the frame centre, then a pan. Applied by **every** coloured
scene: `fragment_field`, `swarm`, the line systems (`parametric_curve` /
`lsystem` / `star_pattern`, and `spectrum` since Plan 0034), and — since Plan
0025 — `reaction_diffusion` and `attractor`.

- On the **line**, **swarm**, and **attractor** scenes, `zoom > 1` moves the camera
  *in* (geometry bigger); `zoom = 1` is no zoom. `pan_*` shift in world units. Try
  `zoom = "1 + bass * 0.6"` for a kick-driven pump.
- On the **fragment field** and **reaction-diffusion**, `zoom` scales the *sample*
  coordinates of the present pass, so a *higher* `zoom` shows *more* of the field
  (the opposite sense to the geometry scenes, kept for the shipped fragment
  presets); `pan_x` / `pan_y` slide the sampled window.

**Reaction-diffusion's field is finite but toroidal**, and that is what makes the
view transform usable on it. The simulation runs on a fixed grid whose edges wrap
— growth leaving the right edge re-enters on the left — so the field is seamless
even though it is not infinite. Since Plan 0033 the present pass samples it that
way too:

- `pan_x` / `pan_y` are a **seamless infinite scroll**. Pan as far as you like in
  either axis; the field repeats, and there is no edge to fall off.
- `zoom > 1` **tiles** the field rather than running out of it. At `zoom = 1.4`
  you see the whole field plus a wrapped border on each side, joined seamlessly.

Before that change the present sampler clamped, so anything past the field edge
was the boundary row smeared outward — `zoom > 1` produced vertical bars and
rectangular blocks, and any real `pan_*` walked off into them. That is why the
shipped `reaction_*` presets are pinned near `zoom = 0.99` with a whisper of pan,
and why those pins are now unnecessary.

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

**Since Plan 0051 the backdrop composites correctly under every scene, and a lit
`bg_bright` is worth revisiting on anything in the `swarm_*` and line families.**
Until then the swarm's sprites and the line renderer's strokes emitted a constant
alpha across their whole quad while only their colour carried the falloff, so
every sprite punched four black rectangular notches beside itself and every
stroke drew itself a black rim — invisible at `bg_bright = 0`, and worse the
fatter the stroke. Raising `thickness` against a lit backdrop therefore widened
the rim as fast as it widened the glow, which is why several presets in these
families sit at a near-black floor. That constraint is gone
([ADR-0056](../docs/adrs/0056-additive-scenes-emit-premultiplied-alpha.md)); the
values themselves were not re-tuned.

**The ceiling on `bg_bright` moved rather than disappeared, and the new one is
worth knowing before you raise it.** A scene that draws into the chain occludes
the backdrop by its **coverage**, whatever light it emits — the frame resolves
`c * g + bg * (1 - g)`, so a fragment *darkens* the backdrop wherever its own
light `c` is dimmer than the backdrop `bg`. Raise `bg_bright` past the **dimmest**
emitted luminance in the figure and the dim parts stop fading out and start
reading as dark speckle: on the swarm, the depth-parallaxed far particles go
first; on a line scene, a stroke dimmed by `glow` or by a low-amplitude band.
Rendered at `bg_bright = 0.35`, a swarm at `brightness = 0.02` is black specks on
a lit field. So the working limit is the darkest part of the figure you still
want visible, not a fixed number — sweep it and look, do not assume.

None of this is worse than before: pre-fix the whole sprite quad held the
backdrop out, so every value is brighter now than it was. The question of whether
additive light should occlude *at all* is a look decision left open in ADR-0056
and carried in [`docs/design-backlog.md`](../docs/design-backlog.md).

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

### Screen-space kaleidoscope — `kaleido_order`, `kaleido_angle`, `kaleido_center_x`, `kaleido_center_y`

Folds the finished frame into `kaleido_order` mirrored wedges before present.
`kaleido_order < 2` (default) is a passthrough; `>= 2` folds (clamped to 48).
`kaleido_angle` (radians) rotates the fold — ride it on `time` for a turning
kaleidoscope. Works on any scene.

**The fold covers a disc, not the whole frame.** It is a polar operation on a
rectangular picture, so it can only reach the largest circle the frame contains —
radius half the **shorter** side, centred on the fold axis. Past that the picture
fades out to the backdrop, and the corners are backdrop. That is a deliberate
vignette (ADR-0047), and it replaces the hard streaks and chevron debris the fold
used to leave out there, which were worst in a portrait window. Two consequences
worth composing around:

- **The fold crops.** At 16:9 the disc is 56 % of the frame's width, so a figure
  that filled the frame before the fold will not fill it after. Scale the figure
  up (`scale`, `zoom`) if you want it to reach the disc's edge.
- **The corners are the backdrop's**, so `bg_hue` / `bg_bright` / `bg_vignette`
  now decide what the frame's edge looks like on any folded preset. They compose
  with the fold instead of being overwritten by it.
- **The backdrop is underneath the fold, not inside it** (ADR-0055). It is painted
  first and the folded scene composites over it, so `bg_vignette`'s darkening stays
  centred on the *frame* however you drive `kaleido_center_*`, and the backdrop is
  never chopped into the wedges. A lit backdrop is the way to give a folded preset
  a frame edge that is not black — and it is worth turning `bg_bright` up while
  composing a fold, because the disc's boundary is much easier to judge against a
  lit corner than a dark one.

`kaleido_center_x` / `kaleido_center_y` place the fold axis in the frame, `0..1`
in normalized screen coordinates, default `0.5` (the centre). This is what makes
`pan_x` / `pan_y` and the fold usable together: pan the scene and drive the fold
centre with the same expression and the rosette travels with its own axis instead
of sliding off it. Both are clamped into the frame; an off-centre axis shrinks the
disc on the side it moved toward, which the falloff absorbs.

**`kaleido_order` is a stepped parameter — it is rounded to a whole number.** A
wedge count has to divide the circle evenly or the frame tears along a horizontal
ray to the left (the fold's angle wrap cannot absorb `atan2`'s branch cut at a
fractional order), so the engine rounds to the nearest integer. Bind or smooth it
however you like — an eased `kaleido_order` still eases, it just **snaps at each
half-integer** instead of sweeping continuously. If you want a continuous
kaleidoscopic motion, ride `kaleido_angle`; that one is fully smooth.

### Mirror or kaleidoscope? They are not the same cost

`mirror_*` and `kaleido_*` both give you N-fold symmetry, and on a line scene they
can look nearly alike — but they work at opposite ends of the pipeline:

| | `mirror_order` / `mirror_reflect` | the `kaleido_*` family |
|---|---|---|
| what it folds | the **geometry**, before rasterization | the finished **pixels** |
| when | while the scene builds its segments | a post stage, after the scene has drawn |
| resolution cost | **none** — every replicated segment is drawn at full target resolution | resamples through the stage's internal grid |
| works on | line systems only (`parametric_curve`, `lsystem`, `star_pattern`) | any scene |

**On a line scene, prefer the mirror whenever either would do.** Replicated
geometry is rasterized at the target's own resolution, so a 24-fold mirror is
exactly as crisp as no mirror at all; the screen-space fold has to resample.

Since Plan 0033 the post stages **follow the render target** rather than sitting at
a fixed 1280x720 (see below), so the fold's cost is far smaller than it was — but
it is still a resample, and the mirror is still free.

### The composite stages render at the target

`trails` and `kaleido_*` used to run at a fixed 1280x720 internal grid and present
stretched, which on any display above 720p upscaled the **whole frame** — including
line art that had just been rasterized at full resolution. That is why a preset
could look sharp with the stages off and "upsized from something smaller" with them
on, and why the shipped line presets were written without `trails`.

Both stages now size their grid from the render target (quantized to a 256 px step,
capped), so composing a look no longer costs sharpness and the line presets can take
their feedback back. Nothing to bind — it follows the window.

Two consequences worth knowing:

- **A resize clears the trail.** Crossing a quantization step reallocates the
  accumulation, so the afterglow restarts. Rare rather than continuous, but a slow
  window drag will blink it.
- **The cap is a real ceiling.** Above it the grid stops growing and both axes scale
  by one factor, so a very large or very wide window resolves the stages slightly
  softer than native rather than unboundedly expensively.

**Composing a stage never changes the picture's proportions**, at any window size.
The grid's own shape is not the picture's: it is quantized, so it is only roughly
your window's ratio, and the scene is drawn at the **window's** aspect and then
stretched back by exactly the inverse when the stage presents. The two cancel, so a
circle is a circle with `trails` on and with it off (ADR-0037).

That was not true until Plan 0035 — the scene took its aspect from the grid, so
turning a stage on stretched the whole frame by up to 28 % on common window sizes
(1280x800 and 1366x768 among them). If you are reading a preset comment written
before then that talks about a stage changing the shape of a figure, it is stale.

### Bloom — `bloom_amount`, `bloom_threshold`, `bloom_radius` (Plan 0045)

The **last** stage in the per-preset chain, after the fold, so it blooms the
finished composite. It picks out the parts of the frame that are brighter than
`bloom_threshold`, blurs them across a pyramid, and adds the result back —
so bright things spill light into the pixels around them.

`bloom_amount = 0` (the default) switches the whole stage off, offscreens and
all: a preset that does not bind it pays nothing and renders exactly as before.

| Param | Default | Meaning |
|---|---|---|
| `bloom_amount` | `0` | Strength of the halo added back. `0` = off. Around `0.5–1.5` is a glow; past `2` the frame is mostly halo (clamped at `4`). Bindable — this is the one to put a beat on. |
| `bloom_threshold` | `1.0` | Where "bright" starts, in **linear light**. At the default only light that is genuinely over range blooms — see below. Lower it to bloom mid-tones too; `0` blooms everything. |
| `bloom_radius` | `1.0` | How far the halo scatters, `0..4`. Low is a tight rim around the figure; high is a wide wash. It does not change how much light there is, only where it goes. |

**`bloom_threshold = 1.0` is a meaningful default, not a placeholder.** Since Plan
0045 the composite carries light *above* 1.0 (see below), so a threshold of 1
means precisely: bloom the light the display could not have shown anyway. Turn
bloom on with nothing else and you get halos exactly where the frame used to
clip, and nowhere else. That is usually what you want; reach for a lower
threshold when you want the whole figure to glow rather than just its hot spots.

**The consequence, and it is the thing that bites first: a preset authored to the
old additive-ceiling habit gets *nothing* from this stage.** For years the
guidance on this page was to keep light under the ceiling so it would not clip.
Bloom's default threshold selects the light that is *over* range — so a preset
that dutifully holds everything below 1.0 hands the bright-pass an empty frame,
and the stage does exactly nothing. This is measured, not theoretical: a draft
holding `brightness` under 1.0 with bloom switched on rendered **pixel-identical**
to the same file at `bloom_amount = 0`. **Something in the frame must deliberately
cross 1.0.** The cheapest fuel is `glow`, because it drives the stroke's *core*
rather than its width — raising `thickness` instead spreads the same light over a
larger quad and can move the peak the wrong way. `brightness` works too, and on
the fullscreen scenes it is the only lever. Read `star_lantern.toml`'s header:
that preset exists to be a worked example of this, and it records what the renders
taught.

```toml
[params]
brightness      = "0.8 + clamp(bass * 2, 0, 1.2)"   # peaks over 1.0 on a hit
bloom_amount    = "0.4 + clamp(onset * 2, 0, 0.9)"  # ...and the hit blooms
bloom_radius    = "1.2"
```

**Verifying a bloom preset from a still is harder than for any other stage, and
`--set bass=1` will lie to you about it.** A held-high band is already an
over-flattering stimulus (see `docs/capturing.md`), but the threshold makes it
much worse here than elsewhere: at `bass = 1` the figure sits far over range and
the halo is enormous, while on real material — where a bass *mean* is around
0.007 against peaks near 0.19 — the frame may never cross the threshold at all.
Every other stage degrades smoothly between those two worlds; this one is a
cliff. Check a bloom preset with `--signal dynamic:<bpm>` or `--audio`, and treat
a `--set` still as a look at the loudest single frame the preset will ever have.

`bloom_amount` and `bloom_radius` are independent on purpose: raising the radius
spreads the same energy wider rather than adding more, so a preset can ride the
radius on a build-up without the frame getting brighter as it goes.

**Cost.** Bloom is the most expensive engine stage — a dozen-odd passes over a
shrinking pyramid — and the pyramid is one level deeper on the rich tier, so the
halo reaches a little further there. Everything else about it is identical
between tiers.

### Linear light and `exposure` (Plan 0045)

Worth knowing even if you never bind anything here, because it changed what the
other luminance params mean.

**The composite is no longer capped at 1.0.** Every stage from the scene to the
transition blend now carries floating-point linear light, and a single engine
tonemap turns it into a displayable picture at the very end. So two
full-brightness strokes crossing no longer land on the same white as one, a
`glow` above 1 is no longer thrown away, and `brightness` past 1 keeps carrying
information instead of flattening.

What that changes for authoring:

- **The old "additive ceiling" habit is softer, not gone.** Stacking luminance
  used to go **flat white** and take the colour with it; now it rolls off — the
  hue survives, the structure inside a hot core stays readable, and a peak reads
  as *intense* rather than as *broken*. Holding luminance nearly flat and
  spending the peak on structure is still the better-looking choice, but it is
  now a taste rule rather than a cliff.
- **The roll-off is engine-fixed**, and it is the identity below about `0.6`, so
  everything a preset was already doing in the low and mid range is untouched.
  Above that it compresses, and it never reaches pure white at any input.
- **`glow` above 1 now does something.** It used to widen a skirt around an
  already-saturated core; now the core itself carries the extra energy, which is
  also what makes it something for bloom to find.

| Param | Default | Meaning |
|---|---|---|
| `exposure` | `1.0` | A linear multiplier on the whole frame before the tonemap. `2` is a stop up, `0.5` a stop down. Engine-wide and *not* per-stage, so like `ink_*` it crossfades across a preset dissolve rather than snapping. |

`exposure` is the honest way to make a whole preset brighter or darker: it
scales everything together and then rolls off, where raising `brightness` on
each element re-balances the picture against its own background. Bind it
sparingly — an `exposure` riding the bass pumps the entire frame, which reads as
a camera reacting rather than as the music, and it is the same trap `glow` on a
beat has always been.

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

**Ink reads the frame after the tonemap**, which is what it has always read —
display-referred pixels in `0..1`. So nothing in this section changed with Plan
0045's linear light, with one practical consequence worth stating: `exposure` and
`bloom_amount` are *upstream* of the remap, so raising either pushes more of the
frame toward the ink pole. That is the lever for an ink preset whose drawing is
coming out too faint, and it is a better one than widening the two poles — see
the note below on why the poles do not do what they look like they do.

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

**Swapping the poles does not make a dark look on a continuous field.** The pass
is `mix(paper, ink, luminance)` — it **interpolates**, it does not map. Setting a
dark paper against a bright ink is the obvious way to try to turn a print into a
glow, and it works only where most pixels already sit near 0 or near 1: a line
scene against black, not a continuous field. A source at *mid* luminance lands
halfway between the poles no matter how far apart you set them. Measured:
`paper_bright = 0.055` against `ink_bright = 0.94` on a developed Gray-Scott
field renders **flat slate grey**. To darken a continuous field, change what the
scene produces — exposure, contour density, the palette — not the poles.

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
`reaction_diffusion`, `attractor`) — and, since Plan 0034, `spectrum`, which
samples the same LUT on the CPU, one colour per element — colour through a shared
**palette** (ADR-0021): a gradient — a built-in `name` or custom `stops` — baked
into a lookup table the scene samples. An optional top-level `[palette]` table
picks it; a `[palette_b]` + bindable `palette_mix` crossfades between two. Colour
modulation (`saturation`, `color_span`/`color_center`, `hue_spread`/`hue_center`,
`palette_mix`) is normal audio-bindable `[params]`. All defaults reproduce each
scene's prior look (`[palette]`-less = the classic `spectrum` cosine), so a preset
that sets none is unchanged. **Since Plan 0054 / ADR-0059 the other three line
scenes join them** — `parametric_curve`, `lsystem` and `star_pattern` all sample
the same LUT on the CPU, each walking `hue_spread` along its own generator's axis
(see [the colour axes](#the-line-scenes-colour-axes--what-hue_spread-walks); the
star's is inert until its interior is redesigned). With no `[palette]` that LUT is
the line scenes' familiar cosine — [the swatch
table](../docs/preset-palettes.md#the-line-scenes-cosine-ramp--what-hue-actually-looks-like)
is what its `hue` values actually look like.

```toml
[palette]
name = "ember"                 # or: stops = [ {at=0.0, color="#0b0b2a"}, ... ]

[params]
color_span = "0.3"             # fragment/RD: low = a cohesive single-family mood
hue_spread = "0.15"            # swarm/attractor: low = a coherent-colour cloud
saturation = "0.8 + mid * 2"
```

**`color_center` (and `hue_center`) is a CYCLIC coordinate.** It slides a window
along the gradient, and the coordinate **wraps** — it does not clamp. So pushing
it negative to reach the palette's dark end lands the field in the palette's
*bright* stops instead, and the picture gets brighter. That cost the content lane
three rendered iterations of chasing exposure and contour density, all downstream
of a cause that was neither. To darken, change the palette's stops or the scene's
own exposure; to *move* the tonal centre, keep the centre inside `0..1` and know
that `-0.1` and `0.9` are the same place.

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

## A clamp is a limit, not a gain — the `[occupancy]` table

`core/tests/saturation.rs` is a **HARD gate**: it walks every shipped preset's
expressions over 12 s of `dynamic:110` and fails the build on any `clamp()` whose
inner value sits **at** its upper bound for 90 % or more of the hops
([ADR-0062](../docs/adrs/0062-clamp-occupancy-is-the-saturation-instrument.md)).

That number is called **occupancy**, and it names the one defect nothing else
here can see. Write

```toml
warp = "clamp(bass * 16, 0, 0.3)"
```

and the ceiling is reached at `bass = 0.019` — below the quietest hop of any real
material. The binding is not a parameter any more; it is the constant `0.3` with
a brief flicker near silence. Every reactivity instrument in this project scores
it as **perfectly reactive**, because they all compare a driven band against
*silence*, and against silence a binary switch is maximally responsive. Plan 0048
Phase 7 measured what that costs: 263 of 332 clamped band terms in exactly this
state, 14 presets with no live audio term at all, behind a fully green suite.

The fix is always the same arithmetic: **divide the gain until the bound is
reached only on peaks.** With `bass` averaging `0.661`, `clamp(bass * 0.45, 0,
0.3)` reaches the ceiling around `bass = 0.67` — on hits, and nowhere else. The
failure message states the level the ceiling is already reached at, so you can do
the division from the failure alone.

Check before you commit — `occ` is the column, and there is a `SAT` line per
finding:

```sh
cargo run -p standalone --example shot -- --report --presets presets
```

The healthy library sits well under the threshold: across 339 clamped bindings
the highest occupancy is `0.61` and the next is `0.44`.

**If the pin is the design**, say so in the preset. A safety rail that exists to
bind at peak is not this defect:

```toml
[occupancy]
exempt = ["fade"]   # this clamp is a rail, not a gain: pinning is intended
```

An exemption silences the **gate** and nothing else — the binding still appears
in `--report`'s `occ` count and its `SAT` line, so it stays visible in review.
Reach for it when you can say in one sentence why the bound must hold; reach for
the division otherwise. A name no `[params]` entry binds is inert and warns at
load.

## A world-space param is not bounded by its clamp — the frame is the bound

The section above is about a band term that stops moving. This one is about a band
term that moves too far, and it is the **opposite** failure with the same cause: a
gain written against the wrong scale.

`clamp(bass * 0.45, 0, 0.3)` cannot leave the frame. It is a *unitless* multiplier
into brightness, or width, or a hue offset, and the `0.3` is a real ceiling. But
some params are **world-space**: they multiply a band into a coordinate. `scale` on
a `spectrum` readout is a world height per unit of band level; `span`, `baseline`,
`size` on the attractor, and any `bin()`- or `index`-driven geometry are the same
kind of quantity. Write

```toml
scale = "3.20"     # spectrum, [spectrum] layout = "polyline"
```

and a fully-driven element stands **3.2 world units** tall. The visible half-height
is `1.0`. The figure is not clipped, not squashed and not dimmed — it is *outside
the picture*, and there is no `clamp` anywhere in that line for a reviewer, a
reachability walk or `--report`'s occupancy column to catch it on.

**This shipped.** `spectrum_ridge` carried `scale = 3.20` from before
[ADR-0049](../docs/adrs/0049-analysis-v2-dual-resolution-axis-normalized-bands.md)
normalized the bands to `0..1`. Afterwards the same constant multiplied a value
roughly five times larger, and the preset rendered as an **empty frame** under
`--signal noise:7` for its entire life while every automated gate stayed green.
The repair was `3.20 -> 0.60`, chosen so a fully-driven element lands just inside
the frame.

So when a band drives a coordinate, do the arithmetic in world units before you
render anything:

> **at the loudest the band can go, where does the element land?** If the answer is
> past `1.0` vertically or past your `span` horizontally, the preset has an
> invisible top end however good it looks at rehearsal level.

Two things that will *not* save you, both learned the hard way:

- **A `clamp` on the term does not make it safe** unless its upper bound is itself
  in world units and inside the frame. `clamp(bass * 3.2, 0, 3.0)` is still off
  frame; the clamp bounds the number, not the picture.
- **Looking at it at a normal listening level does not save you either.** The
  defect lives at the top of the range. `spectrum_comb` and `spectrum_corona`
  clip their tallest bars off the top edge on every beat and still look fine,
  because a comb roots each bar on a shared baseline and only the tips leave.

Check it at the top of the range, not in the middle:

```sh
cargo run -p standalone --example shot -- --preset-file presets/yours.toml --signal noise:7
```

`core/tests/sanity.rs` catches the **total** case — a figure so far out that the
frame comes back empty — and since Plan 0058 it measures the scene against black
with the backdrop suppressed, so a `bg_vignette` can no longer stand in for a
figure that is not there. It also prints a per-preset **excitation ratio**
(coverage when fully driven over coverage at a moderate level) on every run. That
ratio is a report rather than a gate, and the reason is worth knowing: a partial
over-scale that clips only the tips costs almost no pixels, so the number comes
back healthy. **Nothing automated will catch a tip that leaves the frame.** That
one is yours to check.

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
| `seed`      | integer or `"random"` | The salt for `hash()`/`noise()` — see [Seeded randomness](#seeded-randomness--hash-noise-and-generator-seed). **Not** an L-system key: the expansion is deterministic and ignores it, and any system's preset may declare one. Default 0. |

Turtle vocabulary in the expanded string: `F`/`G` draw forward, `f` moves without
drawing, `+`/`-` turn by `angle_deg`, `[`/`]` push/pop the branch state, any other
character is an inert grammar variable.

### `[generator]` — for `star_pattern`

| Key                 | Values                                                | Notes                        |
|---------------------|-------------------------------------------------------|------------------------------|
| `tiling`            | `square`/`4`/`4.4.4.4`, `hexagon`/`6`/`6.6.6`, `octagon`/`8`/`4.8.8`, `dodecagon`/`12`/`3.12.12` | Star order `n`. Required. |
| `contact_angle_deg` | number                                                | Hankin contact angle. Default 30. |

### `[particles]` — for `attractor`

| Key       | Values                                       | Notes                                             |
|-----------|----------------------------------------------|---------------------------------------------------|
| `family`  | `de_jong`, `clifford`, `thomas`, `lorenz`    | Which strange attractor the compute step iterates. Optional — absent means `de_jong`. |
| `density` | `0.0005` .. `1.0`                            | What fraction of the tier's particle budget to draw. Optional — absent means `1.0`, the whole budget. |

```toml
system = "attractor"

[particles]
family = "lorenz"
density = 0.02
```

**`density` and `fade` are one look, not two settings.** `density` decides how
many points you are drawing with; `fade` decides how long each one stays on
screen. What you see is the product:

| | low `fade` (short trail) | high `fade` (long trail) |
|---|---|---|
| **high `density`** | a crisp stipple — the attractor's *measure*, every point an independent sample | **fog** — so many overlapping trails that the figure fills in solid |
| **low `density`** | a sparse scatter of dots, usually too thin to read | **curves** — few enough trajectories that you can follow each one, which is the classic plotted attractor |

The bottom-right cell is the one `density` was added for. At `density = 1.0` the
scene draws 50 000 independent samples of the attractor, and raising `fade` just
makes a denser cloud — you cannot get a *trace* out of it, because 50 000
simultaneous trails overlap into a solid. Drop to `0.02` or below and the same
`fade` reads as banded spiral curves instead.

Two things worth knowing before you reach for it:

- **Total light does not change when you move it — but the picture does.** The
  engine divides each particle's deposit by how many are drawn, so the *sum* is
  invariant: a sparser cloud is not a dimmer one. That is what makes `density` a
  structural choice rather than an exposure control. **It is not a free re-aim,
  though**, and the first two presets to go sparse both had to pay for it: the
  same light landing on a fraction of the pixels is far brighter *per texel*, so
  a sparse preset needs a cut on the order of `trail frames / density` to stay
  off the tonemap shoulder. `attractor_lorenz` ships `exposure = 0.03` at
  `density = 0.002`, and `attractor_thomas` `0.10` at `0.02` — both chosen off
  rendered ladders, not derived. **Buy as much of that level as you can with
  `size` and `fade` first**: those are scene-local and blend as pixels, where
  `exposure` is engine-wide and crossfades across a preset dissolve, so an
  extreme value drags the ~1 s transition through a badly-exposed frame.
- **The tier caps the top, it does not set the value.** `density` is a fraction
  of whatever the current quality tier allows (50 000 at the standard tier,
  150 000 at the rich one), so `density = 0.02` is 1 000 points on one and 3 000
  on the other. You are choosing a proportion, not a count.

It is structural: set once when the preset loads, and **not bindable** to audio.
An eased particle count would re-decide the picture every frame.

**The two flow families draw strokes, not dots.** `thomas` and `lorenz` integrate
a trajectory, so each particle's position this frame is one step along a path
from where it was last frame — and the engine draws that whole step as a short
segment with rounded ends. `de_jong` and `clifford` are *maps*: each iteration
throws the point somewhere else entirely on the figure, so a line between
successive positions would be a bright chord across the picture rather than a
piece of it. Those two keep drawing dots, and nothing you can set changes either
choice — it is a property of the attractor, not a preference.

What that means when you are tuning:

- **On `lorenz` and `thomas`, low `density` is finally worth reaching for.** The
  segments join end to end with no gap, so a sparse cloud reads as continuous
  curves instead of a dotted line. This is the combination the table above calls
  "curves", and before the segment existed you could not actually get there — a
  sparse cloud just looked sparse.
- **They are brighter than they used to be, and you may need to pull them back.**
  A segment covers more of the frame than a dot, so the same preset now lays down
  more light: measured on a plain figure, mean frame luminance went up **2.0x on
  `thomas` and 3.1x on `lorenz`**. This is deliberately *not* compensated in the
  engine — a faster-moving particle drawing a longer, brighter streak is arguably
  the correct rendering of speed. If it is too hot for your look, the levers are
  `exposure`, `size` and `fade`, in that order.

The family sets the map **and** the meaning of the four bindable coefficients
`a`/`b`/`c`/`d`, each of which defaults to that family's canonical value
(De Jong `1.641 1.902 0.316 1.525`; Clifford `-1.4 1.6 1.0 0.7`; Thomas uses `a`
alone at `0.19`; Lorenz `sigma/rho/beta` = `10 28 2.667`, `d` unused). Bind them
to bands for a morphing cloud, but move them **slowly and by a little** — these
are chaotic maps, and a large jump reads as a hard cut rather than a morph.

**Each family is viewed in its own plane**, and it matters the moment you reach
for `zoom` or `pan_*`, because those aim at the figure the plane produces:

| Family     | Dimensions | Viewed in | The spin turns `x` against |
|------------|------------|-----------|----------------------------|
| `de_jong`  | 2-D map    | x–y       | — (in-plane rotation)      |
| `clifford` | 2-D map    | x–y       | — (in-plane rotation)      |
| `thomas`   | 3-D flow   | x–y       | `z`                        |
| `lorenz`   | 3-D flow   | **x–z**   | `y`                        |

Lorenz is the exception, and it is one deliberately: its butterfly *lives* in
x–z, and viewed x–y the two lobes are edge-on — a hard X that reads as a dense
core inside a diffuse cloud rather than as a figure
([ADR-0068](../docs/adrs/0068-the-projection-basis-is-a-per-family-property.md)).
Thomas is 3-D too and keeps x–y, so the plane is a **per-family property**, not
something you can infer from the dimension count.

One consequence to author around: the 3-D families spin as a turntable about the
**vertical** axis, so a quarter turn necessarily leaves the family's own plane.
Lorenz reads as the butterfly near 0° and 180° and as a low-structure cloud near
90° and 270° — the plane buys the shape, not the shape at every angle.

### `[spectrum]` — for `spectrum`

| Key         | Values                                | Notes                                                                                      |
|-------------|---------------------------------------|--------------------------------------------------------------------------------------------|
| `elements`  | integer `2..=64`                      | How many elements the frequency axis is divided into. Default 24. Optional.                  |
| `layout`    | `bars`, `polyline`, `radial_ring`     | Which figure the elements form. Default `bars`. Optional.                                    |
| `smoothing` | seconds, or `{ attack, release }`     | Per-element temporal easing, in the same vocabulary as [`[smoothing]`](#eased-parameters--the-smoothing-table). Default: none (instant). Optional. |

```toml
system = "spectrum"

[spectrum]
elements  = 26
layout    = "bars"
smoothing = { attack = 0.025, release = 0.22 }
```

The whole table is optional — `system = "spectrum"` alone renders the default
readout. An out-of-range `elements` or an unknown `layout` is a surfaced load
error naming what it expected, never a silent fallback.

**Why 64 is the ceiling.** The engine analyses 64 log-spaced bands, and the scene
reduces them to `elements` by averaging each element's own contiguous slice — a
real partition, so no band is dropped or counted twice. Above 64 that stops being
possible, and a readout finer than its own data is a lie rather than a feature.
**The axis is only half logarithmic, and the low end is the coarse end.** Band
edges follow a log curve, but each band is floored at one FFT bin (23.4 Hz at
48 kHz), which binds up to ~750 Hz — so the bottom **31 of the 64 bands are
linear**, and band 0 alone spans 23–47 Hz, a full octave. At 24 elements the
lowest element therefore covers *more* musical range than any other, not less.
The measured mapping is tabulated with
[`bin(x)`](../docs/presets.md#binx--reaching-the-spectrum).

**`smoothing` here is per element, not per binding.** It is the one piece of
easing an expression cannot reach: the element levels are scene state, computed
from the band array rather than evaluated from a binding, so `[smoothing]` has no
name to attach to. Asymmetric values earn their keep more here than almost
anywhere else — the bands are the rawest signal in the engine, so a fast `attack`
keeps a transient's shape while a slow `release` lets the elements fall like a
meter instead of strobing on every analysis hop. Like all easing in this engine it
is expressed in seconds against the real frame time, so it looks the same at 60
and 144 Hz — **and the same at any `curve`**, since what it eases is the curved
level, which is the one you see. Engaging a
[`curve`](#spectrum--the-frequency-axis-readout-plan-0034) costs you a `scale`
retune, not a `release` retune.

Two attractor params behave unlike anything else in the set:

- `fade` (default `0.94`) is the fraction of the trail accumulation kept per
  1/60 s, applied frame-rate-independently. `0` clears every frame (no trails);
  values near `1` smear toward permanence — high `fade` plus `ink_amount` is the
  documented "blot" trap (the page fills in solid).
- `reseed` is **edge-triggered**, not a level: it fires once when the bound
  expression rises past `0.5`, so `reseed = "beat"` fires on each beat instead of
  every frame the flag is held. What it fires is a **disturbance**: every particle
  is kicked a bounded distance from wherever it currently is, and the map's own
  mixing pulls it back onto the figure over the next few frames. The figure is
  shaken; it is not erased.

  > **It used to re-scatter, and "re-scatter" was generous**
  > ([ADR-0066](../docs/adrs/0066-a-reseed-disturbs-the-cloud-rather-than-replacing-it.md)).
  > A reseed *replaced* the cloud with a uniform fill of an axis-aligned box sized
  > to the family's native extent — so it read as a burst of flat speckle with hard
  > straight edges, followed by a visible convergence back onto the attractor. That
  > is what a user reported on `attractor_ink`, and `Rich` tripled the particles
  > into the same rectangle. Measured under the new behaviour, 0 % of the cloud
  > ends up off the figure where the old re-fill put 100 % of it.
  >
  > Two consequences for composing. **A reseed is now a smaller event than it was**,
  > because a disturbance is smaller than a wipe — if you were leaning on the wipe
  > as a structural beat, that is gone, and the jitter magnitude is the lever rather
  > than returning to the box. And **the cloud no longer re-randomizes**: a full
  > re-fill re-sampled the basin every time, so over a long session the population
  > now stays the one it converged to. The map's own mixing is what explores the
  > attractor anyway.

  The seed box survives where it is correct — the initial fill and a family change,
  the two places there is no existing cloud to disturb.

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
| `rose_web` | a scene over a vignetted **background** gradient (`bg_*`), and the **geometry mirror** with reflection toggled on a hard onset |
| `reaction_reef` | the screen-space **kaleidoscope** turning a field into a figure |
| `rose_trails` | **feedback trails** smearing a spinning curve into a spiral |
| `fragment_kaleido` | the screen-space **kaleidoscope** over a fragment field |
| `fragment_smooth` | beat-driven flash/glow **eased** through a `[smoothing]` table |
| `attractor_ink` | the terminal **ink-on-paper** remap (`ink_*` / `paper_*`) |
| `rose_overflow`, `rose_web` | the audio-morphable curve **shape** params (`phase`, `radial_offset`) |
| `rose_draw` | `draw_progress` ridden on `bar` for a per-beat **line-draw-on** |
