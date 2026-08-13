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

**A preset ships when the behavioral suite is green
([ADR-0081](../docs/adrs/0081-the-content-lane-lands-presets-and-architect-curates-the-set.md)),
so know what green is evidence of.** Five gates sweep this folder and **one of them
plays audio**: `reactivity` drives four `core::signal` clips through the real FFT,
band split and onset detector (Plan 0067 Phase 1), so it is the only one that would
notice a preset ignoring the music. `sanity`, `animation`, `distinctness` and
`golden` construct an analysis frame directly — deliberately, because their
questions are about the *frame* — and would pass a preset with every band binding
deleted. The full table is in
[`../docs/capturing.md`](../docs/capturing.md#what-the-five-preset-gates-can-and-cannot-see).
Two things green still does not say: that the preset reacts *well* — see
[the `[occupancy]` table](#a-clamp-is-a-limit-not-a-gain--the-occupancy-table)
below, which is the gate for that — and that the **library** needs another one of
these, which is `architect`'s judgement at the next plan close.

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
  (see below). The five musical-time variables are ADR-0050's, and they are **not
  equals** — see the two layers immediately below.

**The two musical-time layers, and why only one of them is load-bearing.**
`beat_index` and `time_since_beat` are unconditional: always tracked, always
meaning what they say. `beat_in_bar`, `bar_index` and `bar_phase` come from a
**gated** downbeat estimator and are counter-derived whenever it is not confident
— which, measured over 98 minutes of unambiguous 4/4 through the live app, is
**94 % of audible time** (Plan 0068 Phase 3: **6.8 %** lock on four-on-the-floor
techno, **0.14 %** on backbeat rock/pop). That is diagnosed rather than
mysterious: the accent feature is 70 % bass band, and the kick marks every beat
in four-on-the-floor and the half-bar in a backbeat, so it rarely marks the bar.
[ADR-0082](../docs/adrs/0082-the-downbeat-gate-holds-and-the-estimator-is-diagnosed-first.md)'s
`Outcome` carries the measurement and the cause.

So **build an arc on `beat_index`, and treat the bar trio as decorative.** They
are safe to bind — they stay periodic and never claim a wrong beat 1, which is
the trade ADR-0050 made deliberately — but an eight-bar structure written on
`bar_index` is, on most material, an eight-*beat* structure wearing a bar's name.
Write `mod(beat_index, 16)` when you mean four bars of four.
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
  up (the retired `attractor_dejong` had four); one `noise` call is shorter and
  has no period at all. Existing ones are fine as they are — this is not a rewrite.
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
| `fragment_field`  | `warp` `hue` `zoom` `glow` `flash` · `pan_x` `pan_y` · `saturation` `color_span` `color_center` `palette_mix` `palette_steps` `palette_contour` |
| `swarm`           | `force` `spin` `burst` `reseed` `field_freq` `hue` `brightness` `size` `size_spread` `twinkle` `shape` `points` · `zoom` `pan_x` `pan_y` · `saturation` `hue_spread` `hue_center` `palette_mix` `palette_steps` `palette_contour` |
| `parametric_curve`| `n` `d` `phase` `samples` `thickness` `hue` `spin` `scale` `radial_offset` `brightness` `glow` `draw_progress` · `zoom` `pan_x` `pan_y` `mirror_order` `mirror_reflect` · `saturation` `hue_spread` `palette_mix` `palette_steps` `palette_contour` |
| `lsystem`         | `visible_depth` `rotation` `hue` `draw_progress` `thickness` `scale` `brightness` `glow` · `zoom` `pan_x` `pan_y` `mirror_order` `mirror_reflect` · `saturation` `hue_spread` `palette_mix` `palette_steps` `palette_contour` |
| `star_pattern`    | `variant` `rotation` `hue` `draw_progress` `thickness` `scale` `brightness` `glow` `ring_phase` `ring_spread` `ring_scale` · `zoom` `pan_x` `pan_y` `mirror_order` `mirror_reflect` · `saturation` `hue_spread` `palette_mix` `palette_steps` `palette_contour` |
| `reaction_diffusion` | `feed` `kill` `flow` `inject` `hue` `contour` `hatch` `glow` · `zoom` `pan_x` `pan_y` · `saturation` `color_span` `color_center` `palette_mix` `palette_steps` `palette_contour` |
| `attractor`       | `a` `b` `c` `d` `tuple` `size` `hue` `brightness` `fade` `reseed` `spin` `perspective` `depth_fade` `depth_hue` `morph` `curl` `vigor` `lean` `bias` `map_tint` `map_hue` `root_tint` `root_hue` `emergence` · `zoom` `pan_x` `pan_y` · `saturation` `hue_spread` `hue_center` `palette_mix` `palette_steps` `palette_contour` |
| `spectrum`        | `base` `scale` `curve` `span` `baseline` `radius` `rotation` `thickness` `hue` `brightness` `glow` · `zoom` `pan_x` `pan_y` `mirror_order` `mirror_reflect` · `saturation` `hue_spread` `palette_mix` `palette_steps` `palette_contour` |
| `emitter`         | `spawn_rate` `gravity` `launch_speed` `launch_angle` `spread` `lifetime` `lifetime_spread` `size` `size_spread` `shape` `points` `spin` `twinkle` `brightness` · `zoom` `pan_x` `pan_y` · `hue` `saturation` `hue_spread` `hue_center` `palette_mix` `palette_steps` `palette_contour` |

Unbound parameters fall back to each system's defaults. An **unknown** parameter
name is reported as a load-time warning naming the param and the system — the
preset still loads and its other bindings apply (ADR-0020). The params after the
first `·` are the shared **view transform** and
line-**mirror** controls (Plan 0018) — see [Engine-wide controls](#engine-wide-controls-plan-0018);
the trailing group on the palette-coloured scenes (the four shader ones, plus
`spectrum` since Plan 0034) is the shared **palette** colour surface (Plan 0020) —
see [Colour — the palette surface](#colour--the-palette-surface-plan-0020).
Every system additionally accepts the engine-stage params `bg_*`, `trails`,
`kaleido_*`, `bloom_*`, `occlude`, `exposure`, and the final `ink_*`/`paper_*`
remap documented there.

> **The `attractor` roster is the one place a param's meaning depends on the
> family.** `a b c d` mean different things per family and mean *nothing* on the
> five IFS figures; `morph` `curl` `vigor` `lean` `bias` **and** `map_tint`
> `map_hue` `root_tint` `root_hue` `emergence` are **IFS-only** and are inert on the four map
> families; `perspective` `depth_fade` `depth_hue` reach only the two 3-D flows.
> Binding an inert one is not an error and produces no warning — it produces
> nothing at all. See
> [The five IFS figures](#the-five-ifs-figures--a-different-kind-of-family),
> [What made this point, and how far into the figure it is](#what-made-this-point-and-how-far-into-the-figure-it-is)
> and
> [Attractor depth](#attractor-depth-perspective-depth_fade-depth_hue-spin-plan-0063).

**My attractor is too bright / too dim — which knob?** `brightness`. It is a
plain multiplier on the light each particle deposits (default `1.0`), so it
changes the figure's level and nothing else: same points, same trail, same
geometry. It is the same param, under the same name, that `swarm` and `emitter`
carry ([ADR-0080](../docs/adrs/0080-the-attractor-owns-its-level-and-bloom-thresholds-exposed-light.md)).
The deposit is already normalized by particle count, so `brightness` composes
with `[particles] density` rather than fighting it. **Not `exposure`** — that is
the whole-frame stop, it crossfades as a scalar across a preset dissolve, and
`bloom_threshold` is measured against it; see
[Linear light and `exposure`](#linear-light-and-exposure-plan-0045). `size` and
`fade` also move the level, but they move the picture with it — a wider nib and a
longer trail — so use them when that is what you want.

### Attractor depth: `perspective`, `depth_fade`, `depth_hue`, `spin` (Plan 0063)

**Three of these four are exact no-ops on every flat family**, the same way
`a b c d` already carry family-specific meanings. `perspective`, `depth_fade`
and `depth_hue` do something only on the **3-D families — `thomas` and
`lorenz`.** On `de_jong`, `clifford` and the five IFS figures they are
*exactly* the identity: those maps have no third coordinate, so the engine
hands the shader a depth extent of zero and every cue collapses to a no-op.
Binding them there is not an error and produces no warning — it produces
nothing at all. (Verified by capture: at any setting, **zero pixels** differ —
asserted for all three in `core/tests/attractor.rs`.)

> **`depth_fade` joined the other two on 2026-08-09** (Plan 0075 Phase 2,
> [design-backlog 0067](../docs/design-backlog.md)). Until then it was the one
> exception — a uniform dimmer rather than a no-op: the haze is
> `1 − depth_fade · (1 − depth01(dn))`, and a flat family's `dn ≡ 0` put the
> multiplier at `1 − depth_fade/2`, so `depth_fade = 0.9` dimmed a flat figure
> **45 %** everywhere with no depth gradient (measured on `attractor_dissolve`:
> 20.1 % of pixels moved, max channel delta 97, while `perspective` and
> `depth_hue` moved **zero**). The fade term is now multiplied by the family's
> has-depth flag, so all three cues are the identity together. If an old draft
> leaned on the dimming as an undocumented brightness trim on a flat family, it
> silently brightened back — `exposure` is the parameter that means "dimmer"
> and says so.

`spin` is the exception and reaches **every** family. The flat ones rotate
in-plane through the same angle, so an audio-driven `spin` is a real look on
De Jong today, and a fern that rocks a couple of degrees either side of upright
reads as wind. That asymmetry is deliberate; do not read it as an oversight.

| Param | What it does | Range that means something |
|---|---|---|
| `perspective` | Near material grows and far material shrinks, position and point size together. Segments foreshorten, because both endpoints project independently. | `0` (orthographic, the default) .. **~`0.3` in practice**; the clamp is at `0.8` and the reason to stop short of it is below |
| `depth_fade`  | Attenuates brightness with distance — the substitute for occlusion, which this scene does not do. `1` takes the far end to black. | `0` (off) .. `1` |
| `depth_hue`   | Shifts the palette coordinate by `±depth_hue/2` across the depth range, so distance moves *colour* as well as contrast — **on a ramp that travels in hue at roughly constant lightness.** On a dark-to-light ramp (which is what the 3-D presets have shipped) it duplicates `depth_fade` instead, and under `ink_amount = 1` it is structurally dead, like `saturation`. Measured at Plan 0063 Phase 5; [design-backlog 0062](../docs/design-backlog.md) | `0` (off) .. **`2 * min(hue_center, 1 - hue_center)`**, past which the offset wraps on the LUT's repeat sampler and far material lands on the near colour |
| `spin`        | Rate multiplier on the display rotation. `1` is unchanged, `0` holds the figure still, negative reverses it. | any |

Four things you cannot discover by binding them:

- **`perspective` is clamped at `0.8`, silently.** A preset asking for more gets
  the ceiling, not a warning — the same undiscoverable-ceiling shape
  `bloom_threshold` and `vigor` already carry. The number means the figure's
  depth half-extent as a fraction of the camera distance, so the near-to-far
  magnification ratio is `(1 + p) / (1 - p)`: `0.5` gives 3:1, `0.8` gives 9:1,
  and the singularity would be at `1`.
- **`perspective` mostly MOVES the figure, and a `zoom` edit cannot recover
  that.** The magnification is applied *before* the view transform, so raising it
  does make the figure bigger — but that is the small half, and this entry used
  to stop there. **Measured** at Plan 0063 Phase 5, peak-to-peak over four spin
  phases on a bare Lorenz, 600 px square:

  | `perspective` | centre-x swing | widest span |
  |---|---|---|
  | 0.00 | 0.04 NDC | 522 px |
  | 0.15 | 0.11 NDC | 525 px |
  | 0.25 | 0.20 NDC | 529 px |
  | 0.40 | 0.37 NDC | 542 px |
  | 0.60 | 0.55 NDC | 555 px |

  The near side is magnified, so the projected centroid shifts toward whichever
  side is currently near — and as the figure turns, that shift **orbits**. The
  swing is about **0.9 x `perspective`** in NDC; the size growth across that
  whole sweep is **6 %**. A `zoom` is a static scale, so it cannot recover a
  phase-varying translation: all it can do is shrink the figure until the orbit
  fits inside the frame, which is what the 3-D presets paid for
  (`attractor_lorenz`, since retired, went 1.32 -> 1.16; `attractor_thomas`
  1.14 -> 1.02).
  **So the real ceiling is ~`0.3`, not the `0.8` clamp** — past that the figure
  visibly slides around the frame instead of turning in place, which is a worse
  artifact than the flatness `perspective` was bought to fix. The clamp is not
  where the projection breaks; at `0.8` it is still a true perspective divide and
  reads as a strong wide angle, not a fisheye. This ceiling note is
  [design-backlog 0061](../docs/design-backlog.md)'s documented resolution
  (Plan 0075); the deeper fix — re-centring the projection on the figure's
  projected centroid — remains unowned.
- **`spin` is a multiplier on 0.18 rad/s** (`spin = 1` is one revolution per
  **34.9 seconds**), and **its usable ceiling is set by `fade`, not by taste —
  `spin` and `fade` are one look.** A frame of trail drawn while the figure
  turns is a frame of *rotational smear*: push the pair too far and the
  accumulation stops being a trace of the trajectory and becomes concentric
  arcs, which destroys exactly the volume `perspective` was bought to buy.
  **Measured** at Plan 0063 Phase 5: at `fade = 0.932` (~15 frames of trail)
  the rendered ladder `1 / 2 / 3 / 5 / 8` reads *crisp, crisp, softening,
  smeared, scribble* — usable peak about **1.9**; `attractor_thomas` runs
  `fade = 0.955` (~22 frames) and its ceiling is correspondingly lower, about
  **1.3**. The arithmetic agrees: holding the smear under ~5° needs
  `rate < 0.087 / (frames / 60)` rad/s, so the ceiling *falls* as `fade`
  rises. (This bullet used to say `2`–`4` is where the rotation becomes
  legible — true for a trail-free scene, wrong for every attractor preset that
  ships; [design-backlog 0063](../docs/design-backlog.md).) The phase is
  integrated, so a `spin` bound to audio *accelerates* the figure rather than
  snapping it to a new angle — but the integration fixes the discontinuity,
  not the range: a wide binding like `1 + bass * 5` swings the figure through
  most of a revolution between transients and reads as tumbling, not drive.
  The 3-D presets have modulated narrowly (the since-retired Lorenz `1.0`–`1.8`,
  Thomas `1.0`–`1.3`).
- **The illusion has a density limit and haze does not remove it.** Nothing
  occludes anything here — two strands crossing simply sum — so as
  `[particles] density` rises the figure reads more and more as X-ray whatever
  these are set to. If depth stops reading, lower `density` before raising
  `perspective`.

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

Since Plan 0077 the swarm's marks individuate the way the emitter's do —
same names, same semantics (see [Individuation](#individuation--the-distribution-params)).
`size_spread` widens the per-mark size as a fraction either side of `size`; the
swarm's seeded scatter already varies mildly on its own, and this opens that
distribution further. `twinkle` is a per-mark brightness oscillation whose
**rate and phase both** come off the seed, so the field shimmers while the
whole-frame light sits still — which is what separates it from binding
`brightness` to an oscillator, which flashes the frame as one sheet. Both
default `0`; an unbound preset is byte-unchanged. This is the pair that makes a
sparse starfield able to twinkle at all (backlog 0068).

And since the same plan the swarm carries `reseed`, the attractor family's
percussive accent with the attractor's semantics (ADR-0066): a rising edge past
`0.5` **disturbs the population where it is** — a seeded kick of ±6 % of the
domain per axis — rather than respawning it into a box, so it reads as the
field being shaken, never erased, and the flow re-gathers it within a few
seconds. It is edge-triggered: bind a gate (`reseed = "onset > 0.6"`), not a
level — a held high fires once, not continuously. It is also the recovery
lever for a sustained-force pile-up (backlog 0085); note that the *minutes*
horizon of that recovery is outside what the test suite can see, so a world
leaning on it owes the one-off soak check Plan 0077 Phase 5 describes.

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

The swarm's mark is no longer only a round blob: `shape` and `points` give it a
silhouette — see
[Shaped marks](#shaped-marks--the-particle-silhouette-plan-0070), which covers
this scene and the emitter together.

### `emitter` — objects that spawn, fall, and die (Plan 0052)

The emitter is the only system whose **population is not fixed**. Every other
scene draws the same number of things every frame; this one throws objects from a
source line just below the frame, lets each ride its own parabola, and retires it
when its life runs out or it falls out of shot. Nothing wraps — that is the whole
difference from the `swarm`, whose world is a torus and whose particles
categorically cannot leave
([ADR-0057](../docs/adrs/0057-emitter-scene-analytic-ballistics-seeded-individuation.md)).

**The path is fixed at spawn and it is a closed form**, `p0 + v0*t + 0.5*g*t²`.
Two things follow for authoring. There is **no mid-flight force**: no drag, no
swirl, no steering, and easing `gravity` or `launch_speed` changes only the
objects thrown *after* the change — the ones already in the air keep the arc they
were launched on. And the motion is exactly the same on every device, because
there is no `dt` in it at all.

#### The throw

| param | what it does |
|---|---|
| `spawn_rate` | objects a second. The population settles at roughly `spawn_rate * flight time`, so this is the density lever |
| `gravity` | downward acceleration, world units per second squared |
| `launch_speed` | speed at the source, world units a second |
| `launch_angle` | radians **clockwise from straight up**; `0` throws vertically, `1.57` throws to the right |
| `lifetime` | seconds an object lives, if it has not left the frame first |

The geometry is one line of arithmetic and it is worth doing before tuning: a mark
launched at `v` against gravity `g` turns over after `v / g` seconds, `v² / (2g)`
world units above the source line at `y = -1.12`. **A frame is `|y| <= 1`**, so
`v² / (2g) - 1.12` is where the crest of the shower sits. Every object shares
`launch_speed`, so a crest *inside* the frame draws a visible horizontal ceiling
where the population piles up at the top of its arc; `emitter_perseids.toml` puts
its crest at `y = 1.48` (`v = 2.6`, `g = 1.3`), off frame, for exactly that reason.

`lifetime` past the flight time is wasted pool. An object that has left the frame
is retired the moment it does, so the only thing a long `lifetime` buys is slots
held by objects thrown straight up into a `gravity = 0` sky.

#### Individuation — the distribution params

**A binding is evaluated once per frame for the whole scene**, so no expression
can make one object differ from another. That is what these are for: each says how
*wide* a per-object draw is, and the object's own seed picks within it, once, at
spawn. The preset controls the distribution; the seed controls the member.

| param | what varies, per object |
|---|---|
| `spread` | launch angle, within a cone of this width in radians, centred on `launch_angle` |
| `size_spread` | the mark's size, as a fraction either side of `size` |
| `lifetime_spread` | how long it lives, as a fraction either side of `lifetime` |
| `spin` | how fast the mark turns, in radians a second — signed per object, so the field turns both ways |
| `twinkle` | a brightness oscillation whose **rate and phase both** come off the seed |

`spread` is the one that changes the *geometry*. At `0` every object rides the
same vertical parabola and the shower is a column of beads; open it and the arcs
cross. Defaults are non-zero for `spread`, `size_spread` and `lifetime_spread` —
a population with no variation is the defect this exists to fix.

`twinkle` is the answer to "make the stars blink and they all flash together".
Because each object draws its own *rate* as well as its own phase, the whole-frame
brightness stays steady while every member of the field swings; a shared rate
would flash as one sheet whatever the phases were.

`spin` turns the mark, and since Plan 0070 there is something to turn. The
emitter's default mark is a soft **elongated glint** rather than a perfect disc —
a disc is rotationally symmetric, so on one `spin` would be invisible — and that
glint is what `shape = disc` means *on this scene*. Select any other silhouette
and `spin` rotates the figure itself, which is what makes a shaped mark read as
an object rather than a stamp. See
[Shaped marks](#shaped-marks--the-particle-silhouette-plan-0070).

#### What it does not have

No per-object expressions (`hash(index)`-style authoring is out of reach — widen
a spread, but "every seventh object is gold" is not expressible), no collision or
inter-object forces, no stamped trail. `trails` reads differently here than on the
swarm and that is not a defect: a decaying smear behind an object that **leaves**
is a comet tail, where behind a wrapping particle it is a current. Keep it short
if the arcs should read as arcs.

There is also no positionable source: the line spans the frame width at
`y = -1.12` and cannot be moved or narrowed. A look that wants a point fountain
or an off-centre jet is engine feedback, not a preset.

### Shaped marks — the particle silhouette (Plan 0070)

`swarm` and `emitter` both carry `shape` and `points`. They are the only two
scenes that do: the `attractor`'s marks are a chaos-game accumulation, and at the
densities that make a figure one mark is a point, so a silhouette there would be
invisible on principle rather than by tuning
([ADR-0084](../docs/adrs/0084-a-particle-marks-silhouette-is-a-signed-distance-function.md)).

**`shape` is a numeric selector**, like `kaleido_edge` — the expression grammar
has no strings, so a star is `shape = "3"`.

| `shape` | mark | what it draws |
|---|---|---|
| `0` | `disc` | the default, and **exactly** what these scenes drew before the roster existed. On the swarm a round blob; on the emitter its elongated glint (see `spin` above) |
| `1` | `ring` | an annulus the same size as the disc: brightest at 0.7 of the mark's radius, dark at the centre, with a hole reaching 0.4 of it |
| `2` | `polygon` | a regular `points`-sided polygon, one vertex pointing along +x |
| `3` | `star` | a `points`-pointed star, valleys at 0.45 of the tip radius |
| `4` | `heart` | a heart, point down |

`points` is the count for `polygon` and `star`, `3` to `12`, default `5`. It does
nothing on `disc`, `ring` or `heart`. Past a dozen the marks these are *for* — a
few pixels across — are a disc with a rough edge.

**`polygon` has far less range than `star`, and the reason is geometry rather
than tuning.** A polygon's corners sit at radius 1 and its edges at
`cos(pi / points)` of that, so by seven sides the figure is within 10 % of a
circle everywhere and reads as one: rendered side by side, a 7-gon field and a
disc field are the same picture. Reach for `points = 3` or `4` — a field of
triangles is unmistakable — or reach for `star`, whose valleys sit at 0.45 of its
tips at every count.

**`points` is a stepped parameter: it is rounded to a whole number**, so an eased
`points` *snaps* at each half-integer rather than morphing. Both silhouettes fold
the angle by the count, and that fold only absorbs `atan2`'s branch cut when the
count divides the circle evenly — at a fractional count the mark tears along one
ray. This is `kaleido_order`'s rule for `kaleido_order`'s reason, and it is worth
stating because the surrounding vocabulary teaches the opposite: `variant`
interpolates ([ADR-0060](../docs/adrs/0060-star-pattern-variants-interpolate.md))
and the attractor's IFS morphs
([ADR-0075](../docs/adrs/0075-ifs-family-morphs-in-singular-value-space.md)). A
star's angle fold is periodic in the count, so a fractional count is a
discontinuity and not an intermediate figure. `shape` is stepped for the stricter
version of the same reason — its values are names, and there is nothing halfway
between a ring and a polygon.

Two consequences worth planning around:

- **`hash(beat_index)` on `points` is the trick.** The count flipping per beat is
  what already worked on `parametric_curve`'s `radial_offset` lobes, and it
  carries over: `points = "7 + floor(hash(beat_index) * 2.999)"` gives a field
  that re-cuts itself every beat, and the stepping is a feature there rather than
  a cost.
- **Small marks are where a silhouette earns its keep and also where it
  disappears.** A seven-pointed star three pixels across is mostly its own
  anti-aliasing. If the shape is not reading, raise `size` before anything else —
  and note that on the swarm the population is fixed, so a mark large enough to
  have a legible figure fills the frame. The engine's own shaped fixture reaches
  for `zoom` to thin the field rather than for a smaller mark.

> **A shaped mark is a silhouette in additive light — there is no fill and no
> outline.** The compositor adds, so black adds nothing and a dark edge cannot
> exist: a `heart` is a heart-shaped **glow**, brightest in its middle and fading
> to nothing at its outline, not a red body with a black rim. The same is true of
> every entry above. This is ADR-0084's deliberate scope and not a gap waiting to
> be tuned around — a two-tone object reopens the additive-blend decision the
> whole composite rests on and is its own backlog question. If a look needs one,
> that is engine feedback.

The roster is closed: a shape that is not in the table is `architect` + `dev`
work, not a preset. That is the same trade
[ADR-0061](../docs/adrs/0061-kaleidoscope-edge-treatment-is-a-per-preset-choice.md)
made for fold edges.

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
| `star_pattern` | **radius from the figure's centre** — live on a `rings` preset, inert on a bare interlace, see below | one flat `hue` | innermost ring at `hue`, outermost a full palette away |
| `spectrum` | **band index** — element `0` is the bottom of the spectrum | one flat `hue` | low band at `hue`, top band a full palette away |

`hue_spread = 0` is the default on every one of them and reproduces exactly the
single flat colour these scenes drew before the palette reached them, so adding
a `[palette]` to an existing preset changes its colours and adding nothing
changes nothing.

**On `lsystem` the axis is the figure's own, so a grammar without branches has
nothing to ramp along.** The retired `lsystem_fern` reached generation 11 at
`visible_depth = 6`; the retired `lsystem_arrowhead` had no `[` in its rules at
all, so every segment of it sat at generation 0 and `hue_spread` was a no-op
there however large (both retired at Plan 0075 cohort 1 — the property is the
grammar's, not the file's). That is a property of a Sierpinski arrowhead — every segment genuinely is
at the same recursion level — not a missing feature. Such a preset still reaches
`[palette]`; what it cannot reach is a ramp. The ramp is normalized over the
**figure's own** deepest generation, so `hue_spread = 1` spans the palette once
whatever the grammar's branching factor and whichever depth is visible.

**On `parametric_curve` the divisor is `samples`, not the revealed prefix.** So a
`draw_progress` riding `bar` *draws the gradient on* — a half-revealed curve shows
the palette's first half — rather than re-tinting every chord it already drew.

> **`hue_spread` on `star_pattern` is live if and only if the preset declares
> `rings`, and that is measured both ways.** A Hankin rosette is `2n` congruent
> segments about the frame centre, so every one of them occupies the *same*
> radial interval: on `star_rosette` and `star_lantern` (both retired at Plan
> 0075 cohort 1) the spread of segment radii across the whole figure measured
> `1.2e-7` (f32 noise) at every tiling order and every contact angle. There is no range for a radial ramp to walk, so
> the ramp collapses to the flat `hue` rather than sweeping on noise, and what
> those presets gain from the palette surface is `[palette]` itself — the rosette
> can be an ember or an ice figure instead of a point on the built-in cosine —
> plus `saturation` and `palette_mix`.
>
> The empty half was the rosette's **interior**: at `star_rosette`'s 12-fold / 20°
> the strokes live between radius 0.54 and 0.90, so the inner 60% of the disc is
> bare, and `star_lantern`'s 55° variant empties 87% of it. That was the open half
> of design-backlog 0007, and [`rings`](#rings--concentric-rings-of-repeated-motifs)
> closed it: a mandala preset puts segments at four to six different radii, the
> ramp is computed over the **combined** figure, and `hue_spread` becomes a real
> lever on exactly the presets that have an interior to spread across. On a
> composited preset — rings *plus* a tiling — the interlace's own segments are all
> at one radius, so the ramp puts the whole interlace at one end and spreads the
> ornament along the rest, which is a way to separate the two figures by colour
> rather than by brightness.
>
> **`star_rosewindow` (Rose Window, Plan 0075 cohort 1) is the shipped `rings`
> preset today**, and it stays at interlace scale deliberately — the reason is
> design-backlog 0073: every motif is a parametric outline sampled to straight
> segments, so at ornament scale the vertices show and a circle reads as a
> polygon. That is what retired the first three rings presets (`star_mandala`,
> `star_mandala_six`, `star_weave`, 2026-08-06). The ornament-scale mandala
> register moved to an analytic iso-contour fold (`reaction_gilt`, itself since
> retired at Plan 0075 cohort 3) and now lives in `fragment_mandala` (Banded
> Mandala), which has no geometry to facet. Read Rose Window's header before
> authoring the next rings preset.

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

### Background pass — `bg_hue`, `bg_bright`, `bg_vignette`, the ramp and the band

An audio-tintable gradient + vignette backdrop drawn *before* the scene, engine-
wide. `bg_bright = 0` (the default) is a black backdrop; raise it to reveal the
gradient. `bg_hue` picks the tint out of the preset's palette; `bg_vignette` (0..1)
darkens the corners.

**`bg_hue` is a coordinate in your `[palette]`**
([ADR-0086](../docs/adrs/0086-the-backdrop-colours-through-the-preset-palette.md)). The pass samples
the same baked LUT every scene samples, so an `ember` preset gets an ember sky and a custom gradient
tints its own backdrop — and `saturation` / `palette_mix` move the backdrop with the figure, so an
A/B crossfade carries the whole frame. It is **cyclic**, with the same wrap trap `color_center` has:
`-0.1` and `0.9` are the same place, and on a stop-list palette that seam is the sharpest transition
in the gradient. With no `[palette]` declared the gradient is `spectrum`, and the twenty-row swatch
table in
[`docs/preset-palettes.md`](../docs/preset-palettes.md#the-line-scenes-cosine-ramp--what-hue-actually-looks-like)
is that ramp — so `bg_hue` `0.30` is cornflower blue, `0.45` aqua, `0.85` amber *until* you declare
stops of your own, at which point read the value off your own gradient instead. A `bg_hue` copied
from another preset arrives at whatever colour your gradient holds there, not the colour it was in
the preset you took it from. Visible wherever the scene leaves the frame unpainted: behind
the **sparse** scenes (lines, swarm, attractor) where the gaps show through, and —
since Plan 0025 — in the **reaction-diffusion** field's voids (both scenes now
composite over the backdrop instead of presenting opaque). The **fragment field**
is the one full-screen scene that still draws opaquely, so `bg_*` has no visible
effect there — **including the whole ramp and the band below**. A dusk ground
under a `fragment_field` preset is not a dim ground, it is an absent one.

#### The directional ramp — `bg_angle`, `bg_hue_span`, `bg_shade`, `bg_shade_end`, `bg_ramp_gamma`

Five params turn that single tint into a **gradient across the frame**
([ADR-0094](../docs/adrs/0094-the-backdrop-paints-a-directional-ramp.md)). The pass sweeps a
*segment* of your `[palette]` along one axis instead of taking one point of it, with a brightness
ramp on the same axis and one exponent shaping both.

| Param | Default | What it does |
|-------|---------|--------------|
| `bg_angle` | `0.0` | The ramp's direction, in **radians**. `0` runs **bottom-to-top** — the same zero-is-up convention `launch_angle` uses. `1.5708` (π/2) runs left-to-right, `3.1416` (π) top-to-bottom. |
| `bg_hue_span` | `0.0` | How far the palette coordinate **travels** from the ramp's start to its end. `bg_hue` keeps its meaning as the coordinate at the *start*. `0` sweeps nowhere, which is the old one-sample behaviour. |
| `bg_shade` | `0.72` | Brightness factor at the ramp's **start**. |
| `bg_shade_end` | `1.0` | Brightness factor at its **end**. |
| `bg_ramp_gamma` | `1.0` | The ramp's **response exponent** — eases *where* things sit along the axis, applied ahead of both the colour and the brightness. Clamped to `0.05 .. 20`. |

Every default is an **arithmetic identity** with the picture the engine drew before the ramp
existed, not an approximation of it, so a preset that binds none of them renders byte-for-byte
unchanged. All five are ordinary bindable params, so a breathing horizon costs nothing extra.

**Placement is authored by your `[palette]` stops' own `at` positions.** There is deliberately no
`bg_ramp_center` or `bg_ramp_width`: positions already live in the palette, and a second placement
mechanism could disagree with it. Where the horizon sits in the frame is where its colour sits in
your gradient. The worked dusk ground:

```toml
[palette]
stops = [
  { at = 0.00, color = "#060b24" },   # near-black zenith
  { at = 0.25, color = "#1b2a5e" },   # deep blue
  { at = 0.50, color = "#c74b1d" },   # the ember band
  { at = 0.75, color = "#ff7a1f" },
  { at = 1.00, color = "#ffd06e" },   # the hot horizon
]

[params]
bg_bright     = "0.6"
bg_hue        = "1.0"    # the ramp STARTS at the palette's hot end...
bg_hue_span   = "-1.0"   # ...and travels backwards to its near-black end
bg_angle      = "0"      # bottom-to-top, so hot at the bottom of the frame
bg_shade      = "1.0"    # full brightness at the horizon...
bg_shade_end  = "0.25"   # ...a quarter of it at the top
bg_ramp_gamma = "2.0"    # hold the horizon, then fall away
```

Move the `at = 0.50` stop down and the ember band moves down the frame with it. Raise
`bg_ramp_gamma` and the band holds longer before fading; drop it below `1` and the ramp falls away
fast and leaves a long dim tail.

**A wide smooth ramp is safe now, and the dark tail is no longer the risky end.** Until Plan 0082
a gradient this broad *banded* — visibly, on a real display — and the worst of it was exactly where
this control is most useful. The reason is worth carrying: a band is a run of pixels sharing one
8-bit value, so it is widest where the ramp is **flattest**, and sRGB's near-black slope makes the
dim tail the flattest part of any ramp that reaches toward black. Measured on the dusk ground at
1080p, `bg_ramp_gamma = 0.4` spent 7.5 pixels on each level and held one value for **58 pixels**
in its tail. (The intuition that *steep* is dangerous is backwards, and it is written down wrong in
one closed plan — two pixels per level is the healthy state.)

The tonemap now dithers its output by one encoded level
([ADR-0096](../docs/adrs/0096-the-display-write-dithers.md)), always, for every gradient in the
engine at once — the same frame now spends 2.1 pixels per level and its widest plateau is 20. So
**author the ramp you want**: a long dim tail, a low exponent, a near-black sky under a bright
horizon are all ordinary requests, and none of them needs a stop added to break up a step. Nothing
to switch on and no param to bind.

**Why the exponent exists when stop placement can already shape the colour.** Your `[palette]` is
**shared with the scene** (and with the `[layer]`, under
[ADR-0090](../docs/adrs/0090-a-preset-composes-two-scene-layers.md)), so re-spacing stops to shape
the sky's falloff re-maps the figure's colours too. `bg_ramp_gamma` shapes the *backdrop's mapping
onto its axis* and touches nothing else. It is also the only shape control the **brightness** ramp
has at all — `bg_shade -> bg_shade_end` is a straight line and no stop can bend it. It applies to
the position rather than to either channel, so colour and brightness always reach their midpoints
at the same height: it is one ramp, not two that can drift apart.

**The segment wraps if it leaves `[0, 1]`.** `[bg_hue, bg_hue + bg_hue_span]` is repeat-addressed
like every other palette coordinate in the engine, so a span that runs off either end comes back
around the other side. `bg_hue = 0.8` with `bg_hue_span = 0.5` sweeps `0.8 -> 1.3`, which paints
`0.8 -> 1.0` and then `0.0 -> 0.3` — putting the palette's **hot end at both ends of the sky** with
a hard seam where it wraps. That is occasionally what you want and usually a surprise; if the ramp
has a bright band you did not author, check whether the span left the range. It is not clamped, and
deliberately: two shipped presets already drive `bg_hue` outside `[0, 1]` and depend on the wrap.

**The old fixed brightness tilt is gone.** The pass used to multiply its tint by a hardcoded
`0.72 -> 1.0` gradient welded to the vertical, always brighter at the top and unexplained by any
param. `bg_shade` / `bg_shade_end` are where it went — those two numbers *are* its constants, which
is why leaving them alone changes nothing. A backdrop can now be brighter at the **bottom**, which
it never could be.

**The backdrop is invisible to every behavioral gate, and the ramp makes that matter more.**
`sanity` measures coverage against the *scene* with the backdrop suppressed
([ADR-0067](../docs/adrs/0067-coverage-measures-the-scene-not-the-backdrop.md)), and the `animation`
gate strips `bg_*` bindings before scoring motion
([ADR-0091](../docs/adrs/0091-the-animation-gate-scores-motion-against-the-figures-footprint.md)).
So a world whose *ground* is painted here earns exactly nothing at either gate: the figure and any
`[layer]` carry the whole burden. This is correct — it is what stops a more capable backdrop being
used to game the gates — but it means a frame that looks full to you can still read as sparse to
`sanity`. Give the figure enough to stand on its own before reaching for the ramp to fill the frame.

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

**The ceiling on `bg_bright` moved rather than disappeared — and since Plan 0071
it is a choice rather than a property of the engine.** A scene that draws into
the chain occludes the backdrop by its **coverage**, whatever light it emits: the
frame resolves `c * g + bg * (1 - g)`, so a fragment *darkens* the backdrop
wherever its own light `c` is dimmer than the backdrop `bg`. Raise `bg_bright`
past the **dimmest** emitted luminance in the figure and the dim parts stop
fading out and start reading as dark speckle: on the swarm, the depth-parallaxed
far particles go first; on a line scene, a stroke dimmed by `glow` or by a
low-amplitude band. Rendered at `bg_bright = 0.35`, a swarm at
`brightness = 0.02` is black specks on a lit field. The working limit is the
darkest part of the figure you still want visible, not a fixed number — sweep it
and look, do not assume.

**The one knob for it is [`occlude`](#backdrop-occlusion--occlude), below.** It
is what decides whether that limit binds at all.

None of this is worse than before: pre-fix the whole sprite quad held the
backdrop out, so every value is brighter now than it was.

#### The curved band — `bg_band_amount`, `bg_band_angle`, `bg_band_pos`, `bg_band_width`, `bg_band_curve`, `bg_band_hue`, `bg_band_hue_span`

Seven params paint **one soft band of light across the sky**, over the ground the ramp paints and
under everything else ([ADR-0095](../docs/adrs/0095-the-backdrop-paints-a-curved-band.md)). It was
written for a Milky Way arc standing over a dusk horizon: a swell of light along a bowed diagonal,
with the scene's own stars sitting in front of it.

| Param | Default | What it does |
|-------|---------|--------------|
| `bg_band_amount` | `0.0` | The band's **intensity**. `0` draws no band at all. Everything else here is inert until you raise it. |
| `bg_band_angle` | `0.0` | The direction **across** the band, in **radians** — the same axis convention `bg_angle` uses, so `0` runs the band **horizontally** and `1.5708` (π/2) runs it vertically. |
| `bg_band_pos` | `0.5` | Where the centreline sits **along that across-axis**, in the same normalized `0..1` the ramp uses. At `bg_band_angle = 0` that axis runs bottom-to-top, so `0` is the bottom of the frame and `1` the top. |
| `bg_band_width` | `0.15` | The **`1/e` half-width** — see below. Clamped to `0.001 .. 100`. |
| `bg_band_curve` | `0.0` | The **arc**: how far the centreline bows, in across-axis units, at the middle of the band. `0` is exactly straight. |
| `bg_band_hue` | `0.0` | The band's **own coordinate** in the same `[palette]` — an absolute coordinate, not an offset from the ground's. |
| `bg_band_hue_span` | `0.0` | How far that coordinate travels **along** the band, so one end can brighten toward a galactic core. |

Every default is an arithmetic identity with the picture that shipped before the band existed, so a
preset that binds none of them renders byte-for-byte unchanged. All seven are ordinary bindable
params.

**`bg_band_width` is a `1/e` half-width, not a full width and not an edge.** The envelope is a
gaussian: it is at full strength on the centreline and has fallen to `1/e` (about 37 %) exactly
`bg_band_width` either side of it, still faintly visible for two or three times that. So the
*visible* band is several times wider than the number you type. At `0.15` on a 1080-row frame with
`bg_band_angle = 0`, the `1/e` line sits about 160 px from the centre and the band reads roughly a
third of the frame tall. Halve it for a tight ribbon; `0.4` is a wash across the whole sky.

The worked Milky Way arc, over the same dusk palette the ramp's example uses:

```toml
[params]
bg_bright        = "0.0"    # a near-black sky: the band does not need a ground
bg_band_amount   = "0.5"
bg_band_angle    = "1.2"    # the across-axis tilted, so the band runs diagonally
bg_band_pos      = "0.55"   # a little above centre
bg_band_width    = "0.12"
bg_band_curve    = "0.18"   # bowed — the silhouette that reads as a galaxy
bg_band_hue      = "0.30"   # the palette's cool blue-white...
bg_band_hue_span = "0.25"   # ...warming toward one end
```

Two levers make it operable: move `bg_band_pos` and the arc rides up or down the frame; raise
`bg_band_curve` and it bows further. Turn the whole thing with `bg_band_angle`, remembering that
angle names the direction **across** the band rather than along it — so the band itself runs
perpendicular to the number you write.

**`bg_band_amount > 0` is now enough on its own.** The backdrop pass used to skip drawing entirely
below a visible `bg_bright`; it no longer does. A band over a `bg_bright = 0` sky paints, which is
the near-black configuration this look actually wants — the reference photograph's sky *is* almost
black away from the horizon. You do not need a lit ground to hang a galaxy on.

**The band is additive over the ground and under the scene.** It adds light rather than replacing
it, which is what unresolved starlight is — so it brightens whatever the ramp already painted rather
than covering it, and the scene then draws over both. A fullscreen or opaque scene therefore hides
the band exactly as it hides the ramp, and **`fragment_field` hides it completely**. A galaxy under
a fragment-field preset is not a dim galaxy, it is an absent one.

**The band shares your `[palette]` with the ground *and* the scene *and* any `[layer]`, and that is
the one real authoring constraint this creates.** There is no second palette to reach for —
`palette_mix` already owns the A/B pair for preset crossfade, so pinning the band to B would fight
every dissolve. A sky with both a horizon ramp and a galaxy needs stops that hold **both** sets of
colours: the dusk palette in the ramp's example is fully spent on the horizon, and adding an arc to
it means finding room for the band's colours in the same list. Plan for that before tuning, not
after. Because `bg_band_hue` is *absolute*, the band keeps one colour along its whole length
whatever the ramp underneath it is doing — which is what makes a pale arc over a warm horizon
authorable at all.

**The band's coordinate wraps** exactly as the ramp's does: `[bg_band_hue, bg_band_hue +
bg_band_hue_span]` is repeat-addressed, not clamped, so a segment running off either end comes back
around the other side with a hard seam where it wraps. If the arc has a colour break you did not
author, check whether the span left `[0, 1]`.

**And the backdrop is still invisible to every behavioral gate.** This bears restating here rather
than only under the ramp, because a *more capable* backdrop makes the temptation to lean on it
stronger. `sanity` measures coverage against the scene with the backdrop suppressed
([ADR-0067](../docs/adrs/0067-coverage-measures-the-scene-not-the-backdrop.md)) and `animation`
strips `bg_*` bindings before scoring motion
([ADR-0091](../docs/adrs/0091-the-animation-gate-scores-motion-against-the-figures-footprint.md)),
so a galaxy earns a preset **nothing** at either. The figure and any `[layer]` carry both floors, no
matter how much of the frame the sky fills.

### Backdrop occlusion — `occlude`

**How much of the scene's coverage the backdrop resolves against**
([ADR-0085](../docs/adrs/0085-how-much-a-scene-occludes-the-backdrop-is-one-number.md)),
engine-wide and bindable, in `[0, 1]`:

```
out = scene + bg * (1 - alpha * occlude)
```

| value | what the figure does to the sky |
|---|---|
| **`1.0`** (default) | covers it by its own coverage — the arithmetic every frame ran before this existed |
| `0.5` | half of that; the resolve is affine in `occlude`, so a mid value is a genuine blend of the two models and not a switch between them |
| `0.0` | never covers it. Light *adds*; the backdrop arrives whole under the figure |

At the default this multiplies by a literal `1.0`, so a preset that does not bind
it renders **byte-identically** to one written before it existed.

**You cannot see what this does at the floor most presets are authored at**, and
that is the trap worth stating outright. At `bg_bright = 0.01` a black backdrop
times any coverage is still black, so both models look identical and `occlude`
appears to do nothing. It only separates over a **lit** backdrop — which is
exactly the configuration the shipped library uses least and a raised
`bg_bright` uses most. If you are reaching for this, raise the backdrop first.

Three things to weigh before setting it to `0`:

- **A dimming depth cue stops reading as depth.** The swarm's far particles and a
  `glow`-dimmed stroke are *drawn* dim to read as distant. Take the occlusion
  away and they read as transparent instead. On a scene whose depth model is
  luminance that is a real loss, and it is the thing to judge in motion.
- **It lifts the floor rather than removing the problem.** Light that never
  covers is light that always adds, so over a bright backdrop the frame blows
  out. The tonemap rolls that off ([ADR-0046](../docs/adrs/0046-linear-light-hdr-composite-bloom-tonemap.md))
  rather than clipping it, so it degrades softly — but the dark speckle is traded
  for a raised floor, not eliminated.
- **It eases.** Unlike `kaleido_order` or a shape's `points` there is no
  quantization seam, so a `[smoothing]` entry on it is a real blend and a preset
  may drive it off audio if that turns out to be interesting.

**The default stayed at `1.0`, decided by looking** rather than by argument (Plan
0071 Phase 3): two scenes with different depth models, at `occlude` 1.0 / 0.5 /
0.0, over backdrops at 0.35 and 0.60, judged in motion. The verdict was that at
shipped brightnesses the difference is almost negligible — which is the same fact
the ceiling above states from the other side, since the ceiling binds only where
the figure is *dim*. No shipped preset binds `occlude` today.

**The additive families are already unoccluded when no post stage is active.**
The swarm, line and emitter scenes blend colour `One`/`One`, so with an empty
chain their backdrop survives in full whatever `occlude` says — there is no
occlusion at that seam for it to scale. It reaches them through the chain's last
stage instead, which every shipped preset in those families has. The scenes that
present premultiplied over the backdrop (reaction-diffusion, attractor, fragment
field) consume it directly on the empty-chain path.

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
sets the decay (higher = longer trails). Best on a scene with real motion (a
spinning curve, a drifting swarm).

**`trails` is retention per 1/60 s, not per frame** (Plan 0046). The stage raises it
to the `dt`-relative power, so a `trails = 0.9` tail lasts the same *wall-clock*
time on a 48 Hz laptop and a 144 Hz monitor. It used to be applied once per frame,
which made the same preset's trails a third as long at 144 Hz. Captures are
unaffected: the harness steps a fixed 1/60 s, where the exponent is exactly 1.

### Transformed feedback — `fb_zoom`, `fb_rotate`, `fb_dx`, `fb_dy`, `fb_center_x`, `fb_center_y`

The trails stage does not only *dim* the past — it **moves** it. Each frame the
accumulation is resampled through a transform before this frame's light is
deposited on top, so the tail travels while it fades:

| param | default | unit | what it does |
|---|---|---|---|
| `fb_zoom` | `1` | factor **per second** | `> 1` pushes the past outward — a tunnel; `< 1` pulls it in |
| `fb_rotate` | `0` | radians **per second** | spins the past — a spiral |
| `fb_dx` | `0` | frame-heights **per second** | slides the past sideways |
| `fb_dy` | `0` | frame-heights **per second** | slides the past vertically |
| `fb_center_x` | `0.5` | uv (`0`..`1`, left to right) | the fixed point the three above turn about |
| `fb_center_y` | `0.5` | uv (`0`..`1`, top to bottom) | the same, vertically |

Every default is the identity, so a preset that binds none of these renders exactly
what it did before they existed — and `trails` must be **on** for any of them to do
anything, since they transform the accumulation the trails stage owns.

They are ordinary bindables, which is the point: `fb_zoom = "1 + beat * 0.8"` is a
light tunnel that kicks on the beat.

**The rates are per second, and they compose with `dt`.** `fb_zoom = 2` doubles the
past's scale every second however long a frame is; `fb_rotate = 6.28` is one
revolution a second. Do not reach for "per frame" values here — a `1.02` you tuned
as a per-frame zoom is a barely-visible 2 %/s.

**Translation is measured in frame *heights*, on both axes.** `fb_dx = 1` crosses a
16:9 frame horizontally in 1.78 s while `fb_dy = 1` crosses it vertically in 1.0 s.
That is deliberate: one isotropic vocabulary is what makes a diagonal drift a
straight line and a rotation a rotation — not a shear — whatever the display's
shape.

**What lies beyond the frame is nothing.** A zoom-out, a drift, or a rotation about
an off-centre point all pull in from outside the accumulation, and what arrives is
**transparent** — the backdrop, showing through. It is not a smear of the edge
pixel: that would compound into a permanent bar of colour along the border. So an
`fb_zoom < 1` reads as the picture receding into empty space, which is the depth cue
you want; if you want something *in* that space, put it in `bg_*`.

### The `[feedback]` table — the warp family and the deposit blend

Two **structural** choices about the same accumulation, in a table rather than as
params because each selects a code path rather than a quantity — `[curve] family`'s
rule (and see [Structural config](#structural-config-line-systems-and-the-attractor)
for the others). An unknown value is a **load error**: the preset is rejected with a
message naming what was expected, rather than quietly rendering the default.

```toml
[feedback]
warp  = "swirl"   # none (default) | swirl | ripple | fisheye
blend = "add"     # max (default)  | add
```

**`warp` picks a curated procedural distortion** that rides on top of the `fb_*`
affine, about the same `fb_center_*`. Its *strength* is the bindable **`fb_warp`**
(default `0`, so a selected warp does nothing until you drive it) — and, like every
other rate here, it is **per second**.

| `warp` | what it does to the past | `fb_warp` reads as |
|---|---|---|
| `none` | nothing — the affine alone | inert |
| `swirl` | a vortex: rotates about the centre, strongest in the middle and faded by the rim | rad/s at the centre |
| `ripple` | concentric standing waves in radius — the past breathes in rings | frame-heights/s of radial push |
| `fisheye` | a radial magnification growing with radius | positive draws the periphery in, negative pushes it out |

`swirl` and `fb_rotate` are not the same gesture: `fb_rotate` turns the whole frame
rigidly, `swirl` turns the middle faster than the edge, which is what makes a
vortex rather than a spin.

**`blend` picks how this frame's light lands on the faded past.**

- `max` (default) — `max(current, past * trails)`. A bright head with a fading
  tail; the accumulation can never exceed the brightest thing in it. This is what
  the stage did before the choice existed.
- `add` — `current + past * trails`. Overlapping echoes **sum**, so a figure
  crossing its own tail lights up where they meet. Bounded, despite appearances:
  the series converges to `1 / (1 - trails)`, i.e. at most 50x at the `0.98`
  ceiling, and the extra rolls off through the tonemap instead of clipping —
  which only works because the composite runs in linear light (see
  [Linear light and `exposure`](#linear-light-and-exposure-plan-0045)).

`add` wants a *dimmer* preset than `max`: with `trails = 0.95` a figure is already
20x its own brightness once the tail settles. Start by dropping `brightness` (or
`exposure`) by roughly `1 - trails` and tune up from there.

### One vocabulary, two buffers — read this before debugging a feedback preset

**The engine has two accumulations, and `fb_*` drives both.**

- The **`trails` stage**, which every scene composites through. Off unless the
  preset binds `trails`.
- The **`attractor` scene's own internal trail field**, the one its `fade` param
  has always controlled. Always on for that system.

They answer the same seven `fb_*` names and the same `[feedback] warp`, and each
transforms **only its own buffer**. So on an `attractor` preset that also binds
`trails`, a single `fb_rotate = "0.8"` turns *two* things at once — the scene's
field about its centre, and the composited frame about its own — and what you see
is one motion on top of another.

That is the design (learn it once, use it in both places), but it is also the
likeliest way to be confused by your own preset. When a feedback look is not
behaving:

1. **Drop `trails` to `0`.** Whatever still moves is the attractor's own field,
   driven by `fade`. Tune that first.
2. **Then bring `trails` back.** Whatever is added on top is the stage.

Two asymmetries between the sinks are worth knowing:

- **`[feedback] blend` reaches the trails stage only.** The attractor's deposit is
  additive by construction — its points draw through an additive pipeline over the
  decayed bed — so there is no `max` to choose there. `warp` reaches both.
- **The stage only shows up when its tail outlasts the scene's.** Over an
  attractor at `fade = 0.95`, a `trails = 0.9` stage is an exact passthrough:
  `max(cur, prev * 0.9)` is `cur` at every pixel, because the scene already dimmed
  the frame by more than the stage would. If turning `trails` on appears to do
  nothing, raise it above the scene's `fade` rather than assuming it is broken.

Every other system has no accumulation of its own, so there `fb_*` means the
trails stage and nothing else.

### Screen-space kaleidoscope — `kaleido_order`, `kaleido_angle`, `kaleido_center_x`, `kaleido_center_y`, `kaleido_edge`

Folds the finished frame into `kaleido_order` mirrored wedges before present.
`kaleido_order < 2` (default) is a passthrough; `>= 2` folds (clamped to 48).
`kaleido_angle` (radians) rotates the fold — ride it on `time` for a turning
kaleidoscope. Works on any scene.

**The fold reaches a disc, and `kaleido_edge` decides what happens outside it.**
The fold is a polar operation on a rectangular picture, so it can only reach the
largest circle the frame contains — radius half the **shorter** side, centred on
the fold axis. At 16:9 the frame's corner sits at 2.04x that radius and **56 % of
the frame lies outside the disc**, so this is not corner trim: it is most of the
picture, and it is a choice per preset (ADR-0061).

| `kaleido_edge` | What it does outside the disc |
|---|---|
| `0` — `falloff` | Clamps the sample radius at the disc and **fades out** past it, so the corners are backdrop. ADR-0047's treatment, and the one every preset got before this param existed. Crops. |
| **`1` — `tile`** (**default**) | Continues the picture past the disc by **mirroring the source at its own borders**, so the frame is filled with related content. No crop and no fade. |
| `2` — `squash` | **Compresses** the radius asymptotically into the disc, 1:1 at the fold axis and approaching the rim at the corners. Fills the frame with no crop and no fade, at the cost of bending geometry near the frame edge — and, unlike a clamp, it pulls the disc's *interior* inward too. |

**The default is `1`, not `0`.** A preset that binds nothing fills its frame.
`0 = falloff` keeps the numbering tied to what ADR-0047 shipped — the numbering
and the default are separate facts here, and a live A/B on a centred figure and a
border-filling field chose differently on each.

**Two shipped presets carry the instructive contrast.** `attractor_leviathan`
(a centred figure that overruns the inscribed disc) binds `tile`;
`attractor_clifford` (a ribbon whose wings reach past the frame edge) binds
`squash` — and both files carry a header comment beside the binding recording
the render that chose it. Read those two before choosing for a preset of your
own. (`fragment_kaleido`, the border-filling half of the original A/B, retired
at Plan 0075 cohort 2.) Several renaissance field worlds bind their own edge
too — `fragment_mandala`, `fragment_supernova`, `fragment_tunnel`,
`fragment_vitrail` — so the default is no longer the common case.

Which to pick is a question about your scene, and the honest answer is to look:

- A **border-filling field** (a `fragment_field`, a `reaction_diffusion`) is the
  case `falloff` visibly crops — a frame that filled before the fold becomes a
  disc with backdrop corners. `tile` or `squash` is usually what you want.
- A **centred figure** with real space around it may prefer `falloff`, whose fade
  reads as a vignette, if the fill would only bring in more of a figure that is
  already the whole subject.
- `squash` and `tile` differ in what they do to the *interior*: `tile` leaves it
  exactly as a clamp would, `squash` compresses all of it. On a figure whose scale
  you have already tuned, `tile` is the one that does not move it.

Consequences worth composing around whichever you choose:

- **`falloff` crops.** At 16:9 the disc is 56 % of the frame's width, so a figure
  that filled the frame before the fold will not fill it after. Scale the figure
  up (`scale`, `zoom`) if you want it to reach the disc's edge — or pick a fill
  treatment, which is what they exist for.
- **Under `falloff` the corners are the backdrop's**, so `bg_hue` / `bg_bright` /
  `bg_vignette` decide what the frame's edge looks like. Under `tile` and `squash`
  the corners are the *scene's*, so the backdrop retreats to whatever the scene
  leaves transparent.
- **The backdrop is underneath the fold, not inside it** (ADR-0055). It is painted
  first and the folded scene composites over it, so `bg_vignette`'s darkening stays
  centred on the *frame* however you drive `kaleido_center_*`, and the backdrop is
  never chopped into the wedges. A lit backdrop is the way to give a folded preset
  a frame edge that is not black — and it is worth turning `bg_bright` up while
  composing a fold, because the disc's boundary is much easier to judge against a
  lit corner than a dark one. That is not a style note: sixteen confirmation
  captures at `bg_bright = 0` once confirmed an edge treatment that two screenshots
  of the running app then reversed.

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

**`kaleido_edge` is stepped too, and this stage is the only one with two such
params.** It is rounded exactly like `kaleido_order` above and for a stricter
reason: it is a *selector*, so its values are names rather than amounts and there
is nothing between `tile` and `squash` to be halfway to. Bind it or ease it and
the picture **snaps at the midpoint** — 1.4 is `tile`, 1.6 is `squash`. A binding
that evaluates to a non-finite value falls back to the default rather than to a
bound, so a broken expression lands on `tile`, not on `falloff`.

Two things follow that are easy to get wrong:

- **`[smoothing]` on `kaleido_edge` buys you nothing.** It delays the snap; it
  does not soften it. If you want a treatment to change during a track, accept
  that it is a cut.
- **A *dissolve* between two presets with different treatments blends correctly**,
  and that is a different mechanism — a dissolve cross-fades two finished frames,
  so it is blending pictures rather than interpolating the selector. Two presets
  that disagree about the edge cross-fade cleanly; one preset easing across the
  boundary does not.

### The symmetry stage — `kaleido_tile`, `kaleido_radial`, `kaleido_spiral`, `kaleido_zoom`, `kaleido_inner`

The fold above is **one term of a composed coordinate map**
([ADR-0077](../docs/adrs/0077-the-symmetry-stage-owns-one-coordinate-map.md)). Five
more terms ride in the same stage, at the same single texture read, so turning them
all on costs no more sharpness than the fold alone.

| Param | Default | Accepted | What it does |
|---|---|---|---|
| `kaleido_tile` | `1` (off) | `1`–`16` cells | mirrored wallpaper cells across the frame |
| `kaleido_radial` | `1` (off) | `1.02`–`8`, **`1.3`–`2.2` reads** | the **scale ratio between successive rings** — concentric shrinking copies |
| `kaleido_spiral` | `0` (off) | integers `-8`–`8` | an **integer winding number** — the Droste shear |
| `kaleido_zoom` | `0` | any | travel through the rings, **in rings** — `1` is exactly one |
| `kaleido_inner` | **`0.06`** | `0`–`1` of the disc radius | where the repeat freezes |

The default and the reads-well range came off Plan 0064 Phase 4's rendered grid —
`kaleido_radial` × `kaleido_spiral` on a full-frame field, an accumulating attractor
and a sparse line figure, at two aspects — not off the arithmetic. Nothing outside
them is forbidden; the notes below say what you get out there.

**The composed order is fixed: `tile` → fold → `radial` → `spiral`.** It is not
author-selectable — that is what keeps the whole stage one pipeline and one
resample however many terms are live. Read forwards it means the polar rosette is
the motif the tile replicates.

**`kaleido_radial` is a ratio, not a count.** `2.0` means each ring is half the size
of the one outside it; `1.3` gives fine dense rings. Across a 10:1 radius span,
`1.3` draws about **9** rings and `2.0` about **3**. `<= 1` is off. This is the term
that turns a flat rosette into a mandala, and it works on **any** scene, with or
without a fold: `kaleido_radial = 1.3` on a preset that binds no `kaleido_order` at
all still repeats.

- **`1.3`–`2.2` is where it reads as rings.** Below about **`1.2` it stops reading
  as concentric copies at all** — the rings are packed so densely that the eye
  resolves them as a radial *starburst* out of the centre, and it is also where the
  minification is worst, so it is the region that most needs `kaleido_inner`. That
  is a degenerate look, not a forbidden one: bind it deliberately if a starburst is
  what you want, and know that is what you asked for.
- Above `2.2` there is barely more than one ring in the frame, and the term stops
  earning its cost.

**`kaleido_spiral` is stepped, like `kaleido_order` and `kaleido_edge`.** It is a
winding number: one revolution shears `log r` by exactly that many whole rings, and
only a whole number closes the image. A fractional winding draws a visible seam
along the same leftward ray a fractional `kaleido_order` tears along, so the engine
rounds it.

**And it only reads as a twist if `kaleido_radial` gives it something to twist.**
The shear is measured *in rings*, so how visible one winding is depends entirely on
how far apart the rings are:

| `kaleido_radial` | what `kaleido_spiral = 1` looks like |
|---|---|
| `1.6`–`2.2` | a clear Droste twist — this is the pairing to reach for |
| `1.3` | present, readable, milder |
| `1.15` | **nearly invisible** — the rings are so close that a whole winding barely displaces anything |

Subtler still on a **sparse** source (a low-`density` attractor, a thin line
figure): the twist is carried by content crossing ring boundaries, and a mostly
empty frame has little to carry it. If the spiral seems to do nothing, raise
`kaleido_radial` before raising `kaleido_spiral`.

**`kaleido_zoom` loops seamlessly, and its unit is the ring.** The map is periodic
in `log r`, so an offset of exactly one ring is the *identity* — a
`kaleido_zoom = "time * 0.1"` is an endless tunnel with no reset and no crossfade.
**`1` is one ring at every `kaleido_radial`**: the engine multiplies by the log
period itself, so `"bar_phase * 1.0"` advances exactly one ring per bar and keeps
doing so after you re-tune the ratio. (It used to be a raw `log r` offset, which
meant writing `ln(kaleido_radial)` by hand and silently losing the loop the moment
the ratio changed. If you find a preset binding a number like `0.2624`, it is from
before this and wants to be `1.0`.)

**`kaleido_inner` is an aliasing control, not polish — and it is the one term here
whose default is not the identity.** The repeat *minifies* toward the centre: at
`kaleido_radial = 2`, five rings in, a thousand source pixels land under one
destination pixel, and the inner rings turn to moire. `kaleido_inner` freezes the
repeat below that radius, which leaves a clean disc at the centre — the bright hub
you see at the middle of a reference zoom tunnel.

It rests at **`0.06`** rather than `0` because `0` is the one setting guaranteed to
alias, and Phase 4's grid could not tell `0.06` from `0` on any source it rendered:
it caps the worst case for free. Raise it until the churn at the centre stops, and
keep raising it if you want the hub itself as a graphic element — it grows into a
flat disc, so it is a look as well as a fix. Write `kaleido_inner = "0"` if you want
the repeat all the way to the axis; nothing floors it.

**With `kaleido_radial` active, `kaleido_edge` does nothing.** The repeat lands every
radius inside the disc by construction, so there is no outside left to treat.

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
| `bloom_threshold` | `1.0` | Where "bright" starts, in linear light **after `exposure`** — so it means the same thing at any stop. At the default only light that is genuinely over range blooms — see below. Lower it to bloom mid-tones too; `0` blooms everything. |
| `bloom_radius` | `1.0` | How far the halo scatters, `0..4`. Low is a tight rim around the figure; high is a wide wash. It does not change how much light there is, only where it goes. |

**`bloom_threshold = 1.0` is a meaningful default, not a placeholder.** Since Plan
0045 the composite carries light *above* 1.0 (see below), so a threshold of 1
means precisely: bloom the light the display could not have shown anyway. Turn
bloom on with nothing else and you get halos exactly where the frame used to
clip, and nowhere else. That is usually what you want; reach for a lower
threshold when you want the whole figure to glow rather than just its hot spots.

**The comparison happens *after* `exposure`, which is what lets that sentence stay
true when you move the stop** (ADR-0080). The bright-pass scales the light it
samples by the frame's `exposure` before comparing, so "over range" means over the
range the *display* is being asked to show, not over the scene's own linear units.
At `exposure = 1` — the default, and where most presets sit — nothing about this
is visible. It matters the moment a preset moves off it: before this, a preset at
`exposure = 0.03` put its entire figure over every threshold the engine allows
(the ceiling is `8`), so `bloom_threshold` had no discriminating range left and
authors ended up pinning it at the top and calling it capped. Whatever stop you
choose, pick `bloom_threshold` against the picture you see.

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
the fullscreen scenes it is the only lever. (`star_lantern`, the preset that
shipped as the worked example of this, retired at Plan 0075 cohort 1 — its
header survives in git history, and the measurements above are its record.)

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
| `exposure` | `1.0` | A linear multiplier on the **whole frame** before the tonemap — backdrop, scene, halo and all. `2` is a stop up, `0.5` a stop down. Engine-wide and *not* per-stage, so like `ink_*` it crossfades across a preset dissolve rather than snapping. |

`exposure` is the whole-frame correction, and specifically it is the answer to
*the tonemap*: the curve above compresses everything past about `0.6`, so a
mid-tone-dominated look can come out a few percent flatter than it was authored
and a stop up is how you give that back.

**It is not a figure's level, and reaching for it as one costs you twice.** If
one element is too bright or too dim, the lever is that scene's own level param —
`brightness` on the particle scenes (`attractor`, `swarm`, `emitter`), `glow` on
the line scenes, `bg_bright` for the backdrop. Those are scene-local: they blend
as *pixels* across a dissolve and they leave everything else in the frame where
it was. `exposure` does neither. It crossfades as a scalar, so a preset sitting
at an extreme stop drags the ~1 s blend from *any* neighbouring preset through a
badly exposed frame; and it is the number `bloom_threshold` is measured against,
so spending it on a level moves the bright-pass's units under you.

Bind it sparingly for the same reason it exists — an `exposure` riding the bass
pumps the entire frame, which reads as a camera reacting rather than as the
music, and it is the same trap `glow` on a beat has always been.

### Ink on paper — `ink_amount`, `paper_*`, `ink_*`, `ink_gamma` (Plans 0027, 0078)

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
the note below on why the poles do not do what they look like they do. It is
*not* the lever for "the ink should bite harder" — that one is `ink_gamma`, and
the three-lever note below says which does what.

| Param | Default | Meaning |
|---|---|---|
| `ink_amount` | `0` | `0` = off (passthrough). `1` = full remap. Bindable — try `"beat"` to snap into ink on each hit. Read the note below before resting it between the two. |
| `paper_hue` | `0` | Paper (dark-input) hue, into the HSV wheel; wraps, so it can sweep freely. |
| `paper_sat` | `0` | Paper saturation. `0` = neutral. |
| `paper_bright` | `1` | Paper brightness. `1` = white. |
| `ink_hue` | `0` | Ink (bright-input) hue. |
| `ink_sat` | `0` | Ink saturation. `0` = neutral. |
| `ink_bright` | `0` | Ink brightness. `0` = black. |
| `ink_gamma` | `1` | **Response** between the two poles — how fast a pixel travels from paper to ink. `1` is the identity. Above `1` thins the mid-tones toward paper, so only the strongest strokes keep full ink; below `1` inks the mids for a heavier, flatter print. Neither pole moves at any value. Bindable and continuous; clamped to `0.05 .. 20`, both far outside anything a look wants. |

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

# ...and hold the mids off the ink pole, leaving it to the strong strokes.
# The paper stays exactly where it was - see the three-lever note below.
ink_gamma    = "2"
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

**Three levers shape one response, and they are not interchangeable.** An ink
look that is "not contrasty enough" has three places to go, and picking the wrong
one costs structure:

- **`exposure` moves the level going in.** It sits *upstream* of the remap, so
  raising it pushes more of the frame toward the ink pole — and it moves the
  paper with it, because a brighter base is no longer near zero. Reach for it
  when the drawing itself is too faint.
- **`ink_gamma` reshapes the response between the poles, and only that.** The
  paper does not move: a black pixel lands on paper and a full-brightness one
  lands on ink at *every* value — that is the exponent's defining property, not
  a tuning — so raising it leaves less of the frame inked without lifting the
  page or dimming the strokes. This is the lever for "the dark pole should bite
  harder".
- **`ink_amount` is how much remap happens at all**, not how hard it bites. A
  partial value blends the remapped frame back toward the untouched source (see
  the note above), which greys the page. It is a transition, not a contrast
  control.

Measured on a 256-step grey ramp put through the remap with the default poles
(white paper, black ink) — mean output byte over the ramp's interior, higher =
more paper:

| `ink_gamma` | `0.25` | `0.5` | `1` | `2` | `4` |
|---|---|---|---|---|---|
| mean output byte | 147 | 180 | 209 | 229 | 241 |

Read the **direction and size** of the move, not the absolute: the ramp's levels
are sRGB bytes while the key is linear light, so a byte-50 % grey is already a
low density and the identity row leans toward paper before anything is bound.
The two ends of that ramp — pure black in, pure white in — came back as *exactly*
the same two bytes at every one of those five exponents.

**Which way you want to go depends on how dense the drawing already is**, and
the identity row above is the reason. Rendered on `attractor_ink` (a sparse,
mid-density figure) at the same four values: `0.5` gives a heavier, sootier
mark; `2` drops the fine tracery and leaves the strong strokes; `4` thins the
whole figure to a ghost. So on a *faint* drawing, raising it separates the
strokes and costs weight — pair it with `exposure`, or go below `1` for a
heavier print — while on a drawing that is already crowding into a blot, raising
it is what buys the page back. The cream paper was pixel-identical in all four.

Like the rest of the `ink_*` family, `ink_gamma` crossfades across a cross-preset
dissolve rather than snapping.

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

### Hard bands — `palette_steps` and `palette_contour`

The gradient is a smooth ramp, and `palette_steps` turns it into hard graphic
**bands** ([ADR-0078](../docs/adrs/0078-banding-is-a-palette-coordinate-operation.md)):
the palette coordinate snaps to one of `palette_steps` band centres just before the
LUT read. `palette_contour` then darkens a hairline where the coordinate crosses a
band edge. Both default to `0` (off), and off is the *exact* identity, so adding
them to an existing preset is the only thing that changes it.

| Param | Default | Range that reads | Accepted |
|---|---|---|---|
| `palette_steps` | `0` (off) | **`4`–`12`** | `0` = smooth, up to `64` |
| `palette_contour` | `0` (none) | **`0`–`0.5`** | `0` = none, up to `1` |

Those two ranges are Plan 0064 Phase 4's, read off a rendered sweep rather than
argued. Outside them nothing breaks — it just stops being the *graphic* look:

- **`palette_steps` 16 and up approaches the smooth ramp again.** The bands get
  narrower than the eye separates them at, so the picture converges on the
  unbanded gradient. That is a legitimate destination if you want *almost*
  continuous with a hint of structure; it is not what to bind if you want bands.
  `4`–`12` is where the field reads as flat graphic areas.
- **`palette_contour` 0.8 is a deliberate topographic look, not a mistake.** Past
  about `0.5` the dark line stops being an edge between two colours and becomes
  the dominant mark — the field reads as a contour map with colour fill. Bind it
  on purpose or not at all; do not arrive there by easing.

Three things to know before binding them:

- **`palette_steps` is stepped.** It rounds to a whole number, like
  `kaleido_order`: a fractional band count leaves every boundary crawling across
  the field rather than stepping, which reads as shimmer. An eased or bound
  `palette_steps` still eases — it snaps at each half-integer.
- **`palette_contour` is inert on `attractor`, `swarm`, `emitter` and the line
  scenes, and nothing warns.** A contour needs a *gradient across a fragment* to
  sit in, and those scenes take one palette sample per particle or per segment —
  the attractor's in the vertex stage, where the derivative the contour is measured
  against does not exist at all. **Banding reaches every scene; contours reach the
  continuous-field scenes**, `fragment_field` and `reaction_diffusion`. The
  parameter is accepted everywhere because it *is* a known name, so no
  unknown-param warning fires; this paragraph is the warning.
- **Banding fights bloom.** The bright pass blurs exactly the hard edges this
  creates, so a preset cannot have crisp bands and heavy `bloom_amount` at full
  strength. Pick one.

The cyclic *hue* character of a banded reference image is already reachable without
this: the LUT is repeat-addressed, so a `color_span` above 1 wraps it and repeats
the whole gradient across the field. `palette_steps` adds only the hard edge
between one cycle's colours and the next.

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

**Plan 0067 Phase 1 did not change this, and it is worth being explicit about
why.** `reactivity` now drives real PCM through the real analyzer instead of
hand-built frames — but it still compares a driven band against **silence**, and
that is the property this section is about. A saturating clamp is maximally
reactive against silence whether the "driven" number came from an FFT or from a
literal. `saturation` remains the only gate that asks the question this section
asks.

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
  defect lives at the top of the range. The retired `spectrum_comb` and
  `spectrum_corona` clipped their tallest bars off the top edge on every beat
  and still looked fine, because a comb roots each bar on a shared baseline and
  only the tips leave.

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
| `tiling`            | `square`/`4`/`4.4.4.4`, `hexagon`/`6`/`6.6.6`, `octagon`/`8`/`4.8.8`, `dodecagon`/`12`/`3.12.12`, `none` | Star order `n`. Required. **`none` draws no interlace at all** — the rings-only mandala below — and is a load error unless `rings` has at least one entry, because a preset with neither draws nothing. |
| `contact_angle_deg` | number                                                | Hankin contact angle. Default 30. Inert at `tiling = "none"`. |
| `rings`             | array of ring tables (below)                          | The **mandala interior** (ADR-0079 / Plan 0065). Optional — absent is the Hankin interlace alone, i.e. the scene as it was, segment for segment. |

#### `rings` — concentric rings of repeated motifs

The answer to `star_pattern` reading as a hollow ring. Each entry is one
concentric ring; copy `i` of a ring of `k` sits at angle `2*pi*i/k + phase`, at
`radius` from the frame centre, scaled by `scale`. Motifs are drawn through the
same line renderer as the interlace, so the stroke, joins, glow and palette are
the ones you already know.

| Key      | Values                | Notes                                                                 |
|----------|-----------------------|-----------------------------------------------------------------------|
| `motif`  | one of the seven names below | Required. An unknown name is a **load error** listing the legal set — the roster is closed, not extensible. |
| `count`  | `1` .. `512`          | Required. Copies around the ring. `0`, a negative, or anything over the ceiling is a load error. |
| `radius` | number                | Required. Distance from the frame centre, in the fit-normalized world the figure lands in: it spans `+/- 0.9`, so `0.9` is the rim and anything smaller is interior. `0.0` is a single centre boss. |
| `scale`  | number                | Motif size multiplier — close to the copy's diameter, since every outline spans about one unit. Default `0.25`. |
| `phase`  | number (radians)      | Angular offset of copy `0`. Default `0`. Use it to stagger a ring against its neighbours. |

**The closed motif roster is seven names**, picked from rendered samples at Plan
0065 Phase 3 and closed on purpose — a look outside it routes back through
`architect` + `dev`, it is not a preset-side extension point:

| `motif`    | what it is                                                | segments per copy |
|------------|-----------------------------------------------------------|-------------------|
| `circle`   | a plain bead; the ring that reads as a dotted orbit        | 24 |
| `petal`    | a pointed oval (vesica), cusped at both ends along the radius | 24 |
| `teardrop` | round outward, cusped inward — the only motif with an unambiguous "which way is out" | 24 |
| `diamond`  | a four-vertex rhombus, long along the radius               | 4  |
| `arc`      | an **open** outward-bulging arc, chord tangential; the nearest this roster comes to a scalloped boundary | 12 |
| `trefoil`  | a three-lobed rose — the densest member, and the one that reads as ornament rather than as a bead | 36 |
| `chevron`  | an **open** two-segment wedge, apex outward; a sawtooth border at high counts | 2  |

> **`star` and `triangle` were cut** at the same sitting and are load errors now.
> The property they were cut on is the one to judge a candidate by: *does it hold
> its identity across the whole 8-to-32 count range*. `star` is an ornament at ×8
> and dissolves into texture by ×32; `triangle` duplicates `chevron`'s role at
> twelve times the cost.

**The segment budget, and what happens when you exceed it.** A ring costs
`count × segments-per-copy`, and the interlace adds `2n` on top. The floor tier's
ceiling is **20 000** segments (60 000 at rich), so the shipped four-ring mandala
at `36 + 12×4 + 18×24 + 24×24 = 1 092` plus a 24-segment interlace uses about
6 % of it. Two things to know:

- **Truncation at the cap is silent.** The build stops emitting and nothing warns
  you — no load error, no `--report` column. The figure simply ends part-way
  through a ring, in roster order, so it is the *outermost* rings you lose. If a
  dense mandala looks like it is missing its outer ring, count your segments.
- **There are two ways to exhaust the same cap.** The interlace grows with the
  tiling and the rings grow as `rings × count × resolution`; the failure looks
  identical from either direction.

The three bindable levers (`ring_phase`, `ring_spread`, `ring_scale`) move this
roster without changing what is in it — see
[the ring levers](#the-ring-levers--and-why-two-of-them-are-radial).

```toml
system = "star_pattern"

[generator]
tiling = "none"          # the ornament alone; "12" composites it inside the interlace
rings = [
  { motif = "trefoil", count = 1,  radius = 0.00, scale = 0.46 },
  { motif = "diamond", count = 12, radius = 0.30, scale = 0.20 },
  { motif = "petal",   count = 18, radius = 0.52, scale = 0.26, phase = 0.09 },
  { motif = "circle",  count = 24, radius = 0.70, scale = 0.13 },
]
```

#### The ring levers — and why two of them are radial

| Param         | Default | What it does                                                                 |
|---------------|---------|-------------------------------------------------------------------------------|
| `ring_phase`  | `0`     | Turns **alternate rings in opposite directions**, in radians: ring 0 by `+phase`, ring 1 by `−phase`, and so on down the roster. Counter-rotation is the strongest ornamental gesture this geometry has. Wraps at one turn. |
| `ring_spread` | `1`     | Multiplies every ring's `radius` about the centre — the figure opening and closing. Clamped to `0 .. 4`. |
| `ring_scale`  | `1`     | Multiplies every motif's `scale` — the copies growing in place, without moving the rings. Clamped to `0 .. 8`. |

All three default to the exact identity, so **a preset that binds none of them
draws the roster it declared**, and one that declares no `rings` is untouched by
their existence.

> **Do not carry a mandala's animation on `ring_phase` alone.** A ring mandala is
> *very* rotationally symmetric — an 18- and 24-fold figure turned by any angle
> lands almost on top of itself — so a spin moves few pixels however fast it runs,
> and both the eye at a distance and `core/tests/animation.rs` read it as frozen.
> `ring_spread` and `ring_scale` change what the figure *is* at each radius rather
> than where it points, so they are what actually carries the motion. Spend
> `ring_phase` on the gesture and the radial pair on the liveness.

> **A mandala that must hold up in a portrait window carries a narrower `scale`
> or `zoom`.** The shared line renderer divides world x by the target's aspect,
> so at 720×1280 a figure is 1.78× wider in NDC than it is tall: a radius-0.62
> ring that fits comfortably at 16:9 runs off both sides in portrait. This is an
> authoring fact rather than a defect — the three shipped mandalas are sized for
> a landscape window and do graze the frame in portrait.

### `[particles]` — for `attractor`

| Key        | Values                                                          | Notes                                             |
|------------|-----------------------------------------------------------------|---------------------------------------------------|
| `family`   | `de_jong`, `clifford`, `thomas`, `lorenz`, `fern`, `tree`, `dragon`, `sierpinski`, `spiral` | Which figure the compute step iterates. Optional — absent means `de_jong`. The last five are **IFS figures**, not strange attractors — see below. |
| `density`  | `0.0005` .. `1.0`                                                | What fraction of the tier's particle budget to draw. Optional — absent means `1.0`, the whole budget. |
| `morph_to` | any **IFS figure** name                                          | The figure the bindable `morph` param travels towards. Optional — absent pins the figure and makes `morph` inert. **A load error on the four map families**, which have no table to interpolate. |

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
  off the tonemap shoulder. **Spend that cut on `brightness`.** It is a plain
  multiplier on the same already-count-normalized deposit, so it composes with
  `density` instead of fighting it, and being scene-local it blends as pixels
  across a dissolve. `size` and `fade` also buy level and change the picture
  while they do it — a wider nib and a longer trail are looks, not stops — so
  reach for them when you want what they do, not when you only want less light.
  (`attractor_thomas` ships `brightness = 0.10` at `density = 0.02`, and the
  since-retired `attractor_lorenz` shipped `0.03` at `0.002` — the worked
  examples of that cut. Both carried the number on `exposure` until Plan 0066
  moved it; if you are reading an older copy, the swap is level-neutral and the
  value transfers unchanged.)
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
  the correct rendering of speed. If it is too hot for your look, the lever is
  `brightness` — one multiplier on the deposit, which is exactly what "the same
  figure, less light" asks for. `size` and `fade` will also pull it back, at the
  cost of a finer nib and a shorter trace; `exposure` will too, and should be
  your last resort rather than your first, because it moves the whole frame.

The family sets the map **and** the meaning of the four bindable coefficients
`a`/`b`/`c`/`d`, each of which defaults to that family's canonical value
(De Jong `1.641 1.902 0.316 1.525`; Clifford `-1.4 1.6 1.0 0.7`; Thomas uses `a`
alone at `0.19`; Lorenz `sigma/rho/beta` = `10 28 2.667`, `d` unused). Bind them
to bands for a morphing cloud, but move them **slowly and by a little** — these
are chaotic maps, and a large jump reads as a hard cut rather than a morph.
**On the five IFS figures they mean nothing at all** — that family's shape is an
affine table, and `morph`/`curl`/`vigor`/`lean`/`bias` are what reach it.

### `tuple` picks a whole figure, framing included

The companion to the paragraph above. Moving `a`..`d` yourself reaches the
figures *near* the canonical one; it cannot reach a distant one at all, because
a family's world scale and centre are sized to its canonical tuple and a wild
tuple renders off-centre and out of frame with no preset-side recovery — `pan`
cannot span it. `tuple` selects from a **curated roster** whose entries carry
their coefficients *and their framing*, so a distant figure arrives centred and
in frame, with `reseed` and the depth cues already sized to it
([ADR-0093](../docs/adrs/0093-attractor-tuples-are-content-with-per-tuple-framing.md)).

| Param | What it does | Range that means something |
|---|---|---|
| `tuple` | Selects a roster entry for the current family. `0` is the canonical figure — exactly what the family drew before this param existed. | `0` .. one less than the family's roster length; out of range holds the last entry, and a non-finite binding falls back to `0` |

Three things to know before you bind it:

- **It is quantized CPU-side, to the nearest whole entry**, the way
  `kaleido_spiral` and `palette_steps` are. There is no figure between two roster
  entries — a fractional index would mean coefficients interpolated between two
  different attractors, which is a third, unmeasured figure with neither
  endpoint's framing.
- **So a change is a cut, and it wants a long `[smoothing]`** — the guidance
  `kaleido_order` carries, for the same reason. A fast binding is a slideshow
  rather than a morph. The cut is softened by whatever `reseed` disturbance is
  already running, and a cut *between presets* is hidden by the dissolve.
- **A bound `a`..`d` loses to the entry on the frame the cut lands**, and gets
  it back on the next one. Binding both is legal and mostly not what you want:
  the entry's coefficients are the base your `a` was modulating around, and the
  base has just moved.

#### What is in each roster

Curated at Plan 0079 Phase 3 by judging every candidate in motion. Entry `0` is
always the canonical figure — what the family drew before this param existed.

| Family | Entries | What the extras are |
|---|---|---|
| `de_jong` | 13 | Twelve gallery tuples: shells, vaulted arcs, a four-lobed bow-tie, a dense orb, a bare S-curve. |
| `clifford` | 13 | Twelve more: woven discs, an oblique ring, three separated crescents, a hard-edged chevron. |
| `thomas` | 13 | A twelve-step sweep of `a` from `0.03` to `0.22` — the family reads `a` alone, so this is one continuous axis from a space-filling scribble to a tight knot at the edge of chaos. |
| `lorenz` | 12 | `rho` walked from `24.4` to `126.52` (entry `1` is the rho ≈ 100 **torus knot**), plus three that move `sigma`/`beta` instead. |

Two facts about specific entries that a still will not tell you:

- **The Lorenz torus knot (entry `1`) blooms slowly on a `reseed`.** Its orbit is
  marginally stable, so a kick sends the cloud on a wide excursion — measured at
  about **2.2x** the figure's own extent — that takes several seconds to fall
  back. On the canonical butterfly the same disturbance is absorbed in a handful
  of frames. That is a look, not a defect, but bind `reseed` on it knowing it.
- **Thomas past `a ≈ 0.208` closes into periodic orbits.** The roster stops at
  `0.22` deliberately: further up, the flow collapses onto a short cycle, which
  has a perfectly good bounding box and draws as a few dots rather than a
  figure. Four De Jong candidates were rejected at curation for the same reason.

**Indices are names.** The shipped `attractor_*gallery` presets step these by
index and `attractor_torusknot` pins Lorenz entry `1`, so a roster edit that
inserts or reorders renames figures out from under them. Append instead.

**Each family is viewed in its own plane**, and it matters the moment you reach
for `zoom` or `pan_*`, because those aim at the figure the plane produces:

| Family     | Dimensions | Viewed in | The spin turns `x` against |
|------------|------------|-----------|----------------------------|
| `de_jong`  | 2-D map    | x–y       | — (in-plane rotation)      |
| `clifford` | 2-D map    | x–y       | — (in-plane rotation)      |
| `thomas`   | 3-D flow   | x–y       | `z`                        |
| `lorenz`   | 3-D flow   | **x–z**   | `y`                        |
| the five IFS figures | 2-D IFS | x–y | — (in-plane rotation)      |

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

#### The five IFS figures — a different kind of family

`fern`, `tree`, `dragon`, `sierpinski` and `spiral` are **iterated function
systems**, not strange attractors
([ADR-0075](../docs/adrs/0075-ifs-family-morphs-in-singular-value-space.md)). Same
scene, same trails, same `density` / `fade` / palette / view surface — different
step: four affine maps, one drawn at random per particle per step, converging
onto a *figure* rather than onto a filigree.

**Four things change for you as an author**, and the last two are the ones that
cost a session if you carry the rest of this library's habits across. All four
were measured in the Plan 0062 Phase 7 content pass.

**`a` `b` `c` `d` are inert.** An IFS's shape lives in its affine table, not in
four scalars. What reaches the figure is the five params below — plus the four
colour channels and `emergence` in [the next
section](#what-made-this-point-and-how-far-into-the-figure-it-is) — and all ten are
**IFS-only**, inert on `de_jong`, `clifford`, `thomas` and `lorenz`, the same
way `a`..`d` already carry family-specific meanings.

| Param   | Default | What it does |
|---------|---------|--------------|
| `morph` | `0`     | Position from `family` to `[particles] morph_to`. `0` is the named figure, `1` is the target, and every value between is a real figure — but **not a proportionally-different one**, see [below](#morph-is-a-travel-knob). Clamped; out of range pins to an endpoint. |
| `curl`  | `0`     | Radians added to every map's rotation — fronds curl and uncurl. |
| `vigor` | `1`     | Multiplier on the figure's contraction — a bushier, deeper, denser figure. **Has a silent ceiling; see below.** **Inverts on a space-filling figure:** the `dragon`'s two maps sit at exactly `0.7071`, already space-filling, so vigor *above* 1 overfills the region and dissolves the figure into dust — measured on the loud frame at Plan 0075 cohort 5; `attractor_dragon` ships the binding inverted below 1. |
| `lean`  | `0`     | Radians every translation is rotated by — the plant bends. |
| `bias`  | `0`     | Moves sampling weight from the trunk/body maps to the branch maps. Geometry is untouched; only the density distribution moves. Inert on `dragon`, whose two maps are both branches. |

**You cannot break the figure with them, and that is the point.** The maps are
carried as the singular value decomposition of their linear parts, so
contractivity is a comparison on two numbers and every reachable value —
including every intermediate `morph` and every combination of the four levers —
is a converging system. Drive them as hard as you like; unlike `a`..`d` on a
chaotic map, there is no cliff to fall off.

**`vigor`'s ceiling is silent**, the same shape as `bloom_threshold` and
`perspective`. Every map's contraction is held under **0.97**, and the fern's
largest is already `0.851` — so about 17 % of headroom, and asking for more than
that gets *silence* rather than an error or a warning. Past the ceiling every
value renders identically. If `vigor` seems to stop responding, that is where it
stopped.

**The framing follows `morph` but not the levers, deliberately.** The scene
measures each figure pair's extent at load and re-frames as the morph crosses,
so an intermediate figure fills the frame instead of drifting off it. It does
**not** re-frame for the levers, because a fit that did would cancel `vigor`
exactly — the figure would surge and the frame would shrink it back for a net
zero. The cost is that a hard `vigor` push can leave the frame; `zoom` is the
recourse.

<a id="morph-is-a-travel-knob"></a>

**`morph` is a TRAVEL knob, not a little-life knob**, and its visible rate is
steepest near zero — which is the opposite of what "every value between is a
real figure" suggests. Measured on fern → dragon, the lit width of the figure as
a fraction of the frame:

| `morph` | 0.00 | 0.05 | 0.10 | 0.15 |
|---|---|---|---|---|
| lit width | 0.248 | 0.448 | 0.572 | 0.584 |

By `0.05` the fern is half again as wide and reads as a curl rather than a
plant, because a few degrees of per-map rotation **compounds through the
recursion**. A cross that stays recognisably the figure you named would have to
live under about `0.03`, which is not a lever. **So: bind `morph` when the
preset is meant to travel, and leave it alone when it is meant to be one
figure** — the four levers are what change a figure without leaving it.
`attractor_fern` binds no `morph` at all; the since-retired `attractor_dissolve`
(Plan 0075 cohort 5) used the full range, and travelling was its whole point —
its file remains the worked example, in git history.

**The `spiral` is a fine figure and a poor morph target.** Anything ending there
thins into ragged streaks with half the frame empty: its dominant map contracts
at only `0.93`, so the intermediate spends nearly every sample on a map that
barely contracts and the orbit spreads instead of settling. Of five pairs swept
end to end, `sierpinski → fern` was the best by a distance and `fern → spiral`
came last.

**These figures are STILL, so your levers must move ~10× faster than the rest of
this library's drifts.** Every other attractor preset evolves on `time` sines
with 200–400 s periods, which is right for a strange attractor because the
attractor is *already* churning and the drift only stops it repeating. An IFS at
fixed levers is a photograph: everything a viewer sees moving is a lever moving.
Copying the slow periods gives `anim` around 0.018 against a 0.01 gate floor —
it **passes**, and looks like a still image. Around 30 s it reads as alive.

**And bind `spin`.** It defaults to `1`, a full revolution every ~35 s. That is
the shipped look on a chaotic cloud, but these figures have an intrinsic *up* —
`sierpinski` is an equilateral triangle, `fern` is a plant — so the default
spends half of every cycle upside down and reads as a crooked frame rather than
a turning figure. A rock (`sin(time * k) * 0.25`) is almost always what you want.

<a id="what-made-this-point-and-how-far-into-the-figure-it-is"></a>

#### Colouring by what made a point, and by how far into the figure it is (Plan 0073, Plan 0074)

Every particle carries two extra values, and **neither is guessable from the
param name**, so read this before reaching for them
([ADR-0087](../docs/adrs/0087-the-ifs-particle-carries-its-age-and-its-last-map.md),
[ADR-0088](../docs/adrs/0088-the-ifs-colours-by-distance-from-its-own-skeleton.md)).

- **`map` — which part of the figure a point belongs to.** It is the index of
  the affine map applied on that particle's *most recent* step, which makes it a
  property of **position rather than of history**: it names which sub-copy of the
  figure the point currently sits in. On the fern those four sub-copies are the
  stem, the body, the left frond and the right frond. So colouring by it
  genuinely separates the parts of the plant; it is not a per-particle identity
  that would read as noise.
- **`root` — how far a point is from the figure's own skeleton.** The distance to
  the nearest of the drawn maps' fixed points — the places the figure contracts
  toward, which on the fern are the stem base and the frond origins — normalised
  by how far apart those points are. Like `map` it is a property of **position
  rather than of history**, recomputed every step from where the particle now
  is, so an old particle sitting near a fixed point reads the same near-zero a
  freshly restarted one does. That is what makes the gradient permanent rather
  than a startup animation.

Each reaches the picture by **two routes**, which is four params:

| Param       | Default | What it does |
|-------------|---------|--------------|
| `map_tint`  | `0`     | Shifts the particle's **palette coordinate** by `±map_tint/2` across the four sub-copies. Rides your own `[palette]`, so a custom ramp, `palette_mix` and `saturation` all reach it for free. |
| `map_hue`   | `0`     | **Rotates the hue** of the colour the palette returned, by `±map_hue/2` across the same four. Leaves the coordinate alone, so it nudges a part of the figure off your ramp without editing the ramp. `1.0` is a full turn of the wheel. |
| `root_tint` | `0`     | The palette-coordinate route, across distance-from-the-skeleton. **Anchored, not centred** — see below. |
| `root_hue`  | `0`     | The hue-rotation route, across the same distance. Anchored the same way, and the escape when your palette coordinate is already spent. |

> **`root_*` is anchored at zero; `map_*` is centred. This is the one place two
> params on the same page behave differently, so do not generalise from the row
> above.** A particle *on* a fixed point takes **no shift at all** — the stem
> base keeps exactly the colour your ramp already gave it — and the figure ramps
> away from there. So `root_tint` does not open a spread around your colour the
> way `map_tint` does; it pushes one direction, and it only ever pushes.
>
> **Its effective range is per figure, and that will surprise you.** The channel
> is normalised by the figure's own skeleton, and four of the five figures reach
> less than half of it. Measured at Plan 0074 Phase 1, the fraction of the
> palette a `root_tint` of `1.0` actually buys:
>
> | figure | reaches | so for a full sweep, bind about |
> |---|---|---|
> | `spiral` | `0.41` | `2.4` |
> | `fern` | `0.46` | `2.2` |
> | `sierpinski` | `0.50` | `2.0` |
> | `tree` | `0.70` | `1.4` |
> | `dragon` | `1.05` | `0.95` |
>
> A value tuned on the fern and reused on the dragon is wrong by about 2.5×.
>
> **Because it is anchored, `root_tint` spends the ramp's *bright* end by
> construction** — it only ever pushes the coordinate up. On a palette that
> already ends bright (most of them here) it whitens exactly the regions that
> were already brightest, which is why neither shipped IFS preset binds it.
> A **negative** value is legal and is the obvious escape: it ramps down the
> ramp's dark end instead. But the coordinate is sampled by a **repeating** LUT,
> so once it crosses zero the darkest points wrap to the ramp's *brightest*
> stop and a cream speckle appears where the figure should be darkest. On
> `attractor_fern` that is around `root_tint = -0.38` (its coordinate floor is
> `hue_center`'s sine trough `0.20` minus `hue_spread/2`, against a `root01`
> ceiling of `0.46`). Do the same arithmetic for your own preset before going
> negative.

> **The `age_*` params are gone** (Plan 0074). They coloured by how many steps
> since a particle last restarted, on the theory that age proxied
> distance-from-the-restart-points — but the proxy decayed after about ten steps
> and they rendered as per-particle speckle with no gradient anywhere.
> `root_tint`/`root_hue` measure that distance directly and replace them. A
> preset still binding an `age_*` name gets the usual unknown-param warning.

**Which route do you want?** They are not peers — `*_tint` is the default and
`*_hue` is the special case. `*_tint` keeps the figure inside the ramp you
authored, so a fern stays botanical and merely separates; `*_hue` throws a part
clear of the ramp entirely (on `attractor_fern`'s greens it sends the fronds to
teal and periwinkle), which is striking and fights a palette you spent five
stops on. Reach for `*_hue` when your palette is a narrow band and you want one
part *out* of it — or when the palette coordinate is already full, which is the
next thing on this page.

**The palette coordinate is a fixed budget, and this is the one that will cost
you a session.** *Three* params write it — `hue_spread` per particle at random,
`map_tint` per part, `root_tint` per distance — so adding one means **taking
authority away from another**, not stacking a third term on top. Twice measured:

- Plan 0073 brought `attractor_fern`'s `hue_spread` down from `0.16..0.42` to
  `0.05..0.125` before `map_tint` read at all; below that the parts smeared into
  each other and `map_tint` was a faint wash *at any setting*.
- Plan 0074 then found the same fern needs `map_tint` cut from `0.46` to `0.22`
  before `root_tint` improves the picture. Stacked at full strength the plant
  washes out and the stock preset looks better.

Each time you are trading one kind of structure for another, which is a real
choice rather than a bug. **`*_hue` is the escape**: it does not touch this
coordinate at all, so a figure that is out of budget can still take a depth cue
or a part separation through the hue route.

**And on the fern, the escape is what shipped** — so do not read the `0.22`
above as the tuning in the file. Rendered against each other, `root_hue` at the
fern's *full* `map_tint = 0.46` beat the split: the body cools to jade, the
frond origins stay warm, and the part separation Plan 0073 paid `hue_spread` for
is not given back. Every shipped IFS preset binds `root_hue` and **none binds
`root_tint`**. The budget rule is real and the `0.22` measurement stands; the
conclusion drawn from it is that when a coordinate is spent you take the other
route rather than pay.

**And all four are inert on the four map families**: nothing but an IFS writes
either channel, so binding them on `de_jong` does nothing at all rather than
doing something subtle.

**The recycling's *rate* is not an instrument, but its ramp is.** A beat cannot
restart a burst of particles — that was a deliberate call, and `reseed` is the
percussive lever on this family. What you *can* set is `emergence`: how long a
just-restarted particle takes to fade up to full brightness, in **steps**
(default `8`, which is about `0.13 s` at the fixed step). It exists because a
longer `fade` integrates the four restart points over more frames, so a ramp
that hides the recycling at `fade = 0.86` may not at `0.94` — if you push the
trail long and start seeing four bright dots appear, raise this. Values below
one step are clamped silently, because a particle's age advances in whole steps.

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

## The second layer — the `[layer]` table (ADR-0090 / Plan 0076)

A preset may compose **one** optional second scene. The `[layer]` table names its
own system and carries a full authoring surface — params, easing, structural
tables — while the preset's single `[palette]` colours both layers, so a world
keeps one colour language and one baked LUT (that sharing is deliberate and not
configurable; see `docs/preset-palettes.md`).

```toml
system = "fragment_field"      # the main scene — the ground

[params]
warp = "0.4"
trails = "0.6"                 # compositing params live at the TOP level only

[layer]                        # the second scene
system = "swarm"               # any system — the same one twice is legal
join   = "under"               # "under" (default) or "over"
blend  = "screen"              # over-join only: add | screen | multiply | overlay
mix    = "0.2 + 0.8 * bass"    # over-join only: bindable amount, 0..1

[layer.params]                 # the layer scene's own params, its namespace
size = "2.5"
zoom = "0.9 + 0.3 * onset"

[layer.smoothing]              # eases layer bindings — same vocabulary as [smoothing]
zoom = 0.3
mix  = { attack = 0.05, release = 0.6 }
```

### The two joins

- **`under`** — the layer draws into the **same target** as the main scene,
  before the post chain: both share trails, the kaleidoscope and bloom, and fuse
  into one substance. A trailed, folded ground carries the layer with it. Cost:
  one extra scene draw.
- **`over`** — the layer renders into its **own offscreen** and blends into the
  chain **between the kaleidoscope and bloom**: its geometry is never smeared by
  trails nor folded by the kaleidoscope (crisp at display resolution), but it
  still blooms and tonemaps with the frame — crisp *and* glowing. Cost: one
  offscreen plus one blend pass.

**Know your layer's coverage before picking `under`.** A fullscreen scene that
presents with full coverage (a fragment field, reaction-diffusion at high
density) drawn `under` will **occlude the main scene entirely** — the layer
paints over it inside the shared target. `under` is the sparse-over-dense idiom
(particles, lines, an emitter over a field); a fullscreen-over-fullscreen pair
wants `join = "over"` with a blend mode.

### The `over` join's blend and mix

`blend` is structural (fixed at load, unknown names reject the preset):

| mode | reads as |
|---|---|
| `add` | linear-light addition — the engine's native idiom, the brightest |
| `screen` | bounded brightening (the default; cannot blow out) |
| `multiply` | darkens where the layer has coverage |
| `overlay` | multiply below mid-grey, screen above — contrast |

Every mode operates **within the layer's own coverage**: a darkening mode
darkens only where the layer actually drew. `mix` is the one bindable lever in
the table itself — the blend's amount, clamped to `0..1`, eased through a
`[layer.smoothing] mix` entry — so audio can surge the second layer in and a
preset can breathe between one world and two. `mix = "0"` renders exactly the
layerless preset. On an `under` join, `blend` is ignored (with a load warning)
and `mix` has no junction to act at.

### What the layer does and does not get

- **Its own params and easing** — `[layer.params]` reaches the layer's scene
  only, evaluated under the same clock, analysis frame and seed salt as the top
  level. Per-element (`index`) bindings work, against the layer's own
  `[layer.spectrum] elements`. Binding a compositing name (`trails`,
  `kaleido_*`, `bg_*`, `ink_*`…) inside `[layer.params]` warns at load and does
  nothing: the chain, backdrop and terminal passes belong to the preset as a
  whole and take their values from the top level.
- **Its own structural tables** — `[layer.curve]`, `[layer.generator]`,
  `[layer.particles]`, `[layer.spectrum]`, with the same per-system rules (a
  layer L-system still requires its generator table).
- **Its own scene instance** — constructed for the preset, so the same system
  twice is legal (two swarms in counterpoint, two fields at different zooms) and
  a stateful layer (reaction-diffusion, the attractor) carries its own
  simulation state.
- **Not its own palette** (the preset's serves both), **not its own
  `[feedback]` table** (the layer gets the defaults; the attractor-as-layer's
  internal trail is therefore unwarped), and **no third layer** — one `[layer]`
  table, total.

Heavy-plus-heavy pairings are an authoring responsibility: both tiers render
both layers, so an attractor ground under a reaction-diffusion layer costs what
the two scenes cost. Measure with `--report` or the diagnostics overlay before
shipping one.

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
| `swarm_drift` | the shared view **zoom** breathing with the music |
| `attractor_dragon` | a scene over a vignetted **background** gradient (`bg_*`), and a beat-latched structural re-cut (`hash(floor(beat_index * 0.25))`) |
| `fragment_tiled` | the screen-space **kaleidoscope** folding a field into a figure |
| `attractor_clifford` | **feedback trails** stretching a figure into a long exposure |
| `fragment_supernova` | beat-driven flash/glow **eased** through a `[smoothing]` table |
| `attractor_ink` | the terminal **ink-on-paper** remap (`ink_*` / `paper_*`) |
| `curve_nightbloom` | the audio-morphable curve **shape** params (`phase`, `radial_offset`) |
| `lsystem_vellum` | `draw_progress` for a **line-draw-on** |
| `star_rosewindow` | **`rings`** — concentric motifs giving a rosette an interior |
| `fragment_vitrail` | the **`[layer]` table** — a crisp `over` layer with a bindable `mix`, and per-beat `draw_progress` on the layer |
| `fragment_sumi` | a **stateful layer** — the attractor as `[layer]` scene, `add`-blended over a field |
