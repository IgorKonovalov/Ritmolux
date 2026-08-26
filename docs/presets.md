# Preset authoring guide & expression reference

A **preset** is a small text file that describes one visual: it names a built-in
rendering **system** and binds each of that system's **parameters** to a short
**expression** over the live audio analysis. Editing a preset needs no Rust, no
rebuild, and no shader knowledge — you change a line, save, and the running app
picks it up.

This document is the reference for the **expression language** — the vocabulary
you write on the right-hand side of every binding — plus how presets load, where
they live, and how a mistake is reported. It is the authoring counterpart to
[ADR-0002](adrs/0002-layered-preset-architecture.md) (the layered preset
architecture) and [ADR-0020](adrs/0020-preset-grammar-v2-branching-functions-tempo.md)
(the v2 grammar). Only **layers 1-2** of ADR-0002 exist today — TOML data presets
over a pure expression language. Layer 3 (Rhai scripting) and cross-preset
blending are deferred.

**The per-system parameter tables live in [`presets/README.md`](../presets/README.md)**,
next to the preset files themselves. That is the one place they are maintained;
this document does not duplicate them.

> **New to presets? Start with [`docs/preset-guide.md`](preset-guide.md)** — the illustrated
> entrance: what each built-in system looks like, which of these three references owns which
> surface, and the loop you work in. Come back here for the grammar. The quickstart below overlaps
> the guide's opening on purpose; where they disagree, the guide is the newer one
> ([ADR-0101](adrs/0101-the-preset-docs-gain-a-tutorial-layer-rather-than-a-merge.md)).

> **Accurate as of 2026-08-13**, against the curated set in
> [`presets/`](../presets/) — one or more presets for every built-in system — and
> the v2 expression grammar (Plan 0019). The count is deliberately not written
> down here: it has moved with almost every plan, and a number in this line is a
> line that goes stale without anything noticing.

---

## Quickstart: your first preset

1. **Find your preset directory** (see [Where preset files live](#where-preset-files-live)).
   On Windows that is `%APPDATA%\light-music-visualizer\presets`. Both the
   standalone app and the foobar2000 plugin read this same folder — it is seeded
   with the curated set on first run.

2. **Copy an existing preset** as a starting point. `swarm_drift.toml` (a calm
   particle swarm) and `fragment_whorl.toml` (a slow warp field) are the
   friendliest bases:

   ```
   copy swarm_drift.toml   my_first.toml
   ```

3. **Edit the bindings.** Open `my_first.toml` and change an expression — for
   example make the beat kick harder:

   ```toml
   system = "swarm"
   name   = "My First"

   [params]
   force      = "1.2 + clamp(bass * 20, 0, 4)"
   spin       = "0.3 + clamp(mid * 6, 0, 1.2)"
   burst      = "beat * 9"          # was beat * 5 — a bigger blast on each beat
   hue        = "time * 0.02 + clamp(treb * 4, 0, 1)"
   brightness = "0.6 + clamp(bass * 6, 0, 0.8)"
   size       = "1.0 + beat * 0.6"
   ```

4. **Save and watch.** The standalone app polls the folder every ~150 ms and
   hot-reloads on any change (no restart). Press **Space** to cycle to your new
   preset, or **`Tab`** to pick it by name; the window title shows the active
   preset name and system. If the file has a typo, the app reports it and keeps
   the last good set — it never crashes on a bad preset.

   In the foobar2000 component the same loop is one click longer, because it has
   no watcher: right-click → **Reload presets**, then right-click → **Preset ▸**
   and pick yours. A file with a typo is simply absent from that list.

That is the whole loop: copy, edit an expression, save, pick.

---

## Anatomy of a preset file

A preset is a TOML file with a two-line header and a `[params]` table:

```toml
system = "fragment_field"   # required — which built-in system to drive
name   = "Aurora"           # optional — display name (defaults to the system name)

[params]                    # each key is a system parameter; each value is an expression string
warp  = "0.3 + clamp(bass * 14, 0, 1.8)"
hue   = "time * 0.03 + clamp(treb * 5, 0, 1)"
zoom  = "1.0 + bar * 0.25"
glow  = "0.4 + clamp((bass + mid) * 8, 0, 1.1)"
flash = "clamp(onset * 3, 0, 1)"
```

Rules:

- **`system`** must be one of the seven known system names (below). An unknown
  system rejects the whole file.
- **`name`** is free text shown in the standalone title bar. If omitted, the
  system name is used.
- **`[params]`** binds parameters by name to expression strings. Every value is
  a **string** (quote it), even a bare number: `warp = "0.4"`, not `warp = 0.4`.
- **Unbound parameters** fall back to the system's default, so you only need to
  write the parameters you want to drive. Order does not matter — bindings are
  sorted by name at load for determinism.
- **An unknown parameter name is a load-time warning**, not an error — see
  [When a preset is wrong](#when-a-preset-is-wrong).

Beyond `[params]`, a preset may carry optional tables — `[curve]` / `[generator]`
(structural config for the line systems), `[particles]` (attractor family and
sample density),
`[spectrum]` (the readout's element count, layout and per-element easing —
summarised [below](#the-spectrum-table)), `[feedback]` (how an accumulation reads
its own past — [below](#the-feedback-table)), `[smoothing]` (per-parameter
easing), `[palette]` / `[palette_b]` (colour), and `[layer]` (a second scene
composed under or over the main one — ADR-0090; the expression language inside
`[layer.params]` is exactly this document's). All are documented in
[`presets/README.md`](../presets/README.md) and
[`docs/preset-palettes.md`](preset-palettes.md).

Every `[params]` value is evaluated **once per frame** and applied to the system
before it renders. There is no per-frame state you can accumulate in a preset —
an expression is a pure function of the current analysis frame plus the clock.

The one exception is a binding that names [`index`](#index--one-binding-evaluated-once-per-element),
which the `spectrum` system evaluates once *per element*. Nothing else changes:
it is still the same pure expression over the same frame.

---

## The built-in systems

Every built-in system is addressable from a preset. Their **named parameters, defaults,
and per-system notes are tabulated in [`presets/README.md`](../presets/README.md#systems-and-their-named-parameters)** —
that table is maintained alongside the presets and is the authoritative list.

| `system = ` | What it draws |
|-------------|---------------|
| `fragment_field` | A fullscreen domain-warped light field (fragment shader). |
| `swarm` | ~10k CPU-simulated particles on an evolving flow field. |
| `parametric_curve` | A sampled line curve — the Maurer rose (ADR-0007). |
| `lsystem` | An L-system turtle figure, precomputed per depth (ADR-0007). |
| `star_pattern` | A Hankin star pattern over a regular tiling (ADR-0007). |
| `reaction_diffusion` | A Gray-Scott reaction-diffusion field (ADR-0012). |
| `attractor` | GPU compute particles iterating a strange attractor (ADR-0015). |
| `spectrum` | The log-spaced band array as N elements — bars, a contour, or a ring (ADR-0036). |
| `emitter` | Objects that spawn, ride their own parabola, and die — the only system whose population varies (ADR-0057). |
| `shape_field` | One mark silhouette drawn at frame scale as a signed-distance field, so banding the palette draws concentric offset contours (ADR-0105). |
| `warp_mesh` | The previous frame, resampled through a grid with **one transform per vertex** — the only system that draws nothing of its own (ADR-0113). |
| `shape_collage` | Flat opaque elements on their own off-white paper, composited in painter order — the only system in which one object is genuinely *in front of* another, and the only one that draws a graphic rather than light (ADR-0123). |

This table used to carry a per-system count of curated presets. It is gone rather than
corrected: at the Plan 0054 close six of its eight numbers were wrong (`parametric_curve`
read 11 against 6 actual, `swarm` 5 against 3, `attractor` 5 against 6), because a count
re-drifts every time a preset is added and nothing fails when it does. `presets/` is the
list; `ls presets/*.toml` is the count.

**The twelfth system added no expression grammar.** `shape_collage` (Plan 0113) is
entirely new *parameters* — every variable, constant, function, operator and error
message in [The expression language](#the-expression-language) is exactly what it
was before, and a collage preset is written with the same vocabulary as every other
preset. Recorded because the alternative to writing it down is a reader assuming a
new system must have brought new grammar with it.

Beyond a system's own parameters, **every** preset may also bind the engine-wide
compositing controls — the shared view transform (`zoom`, `pan_x`, `pan_y`), the
background pass (`bg_*`), feedback `trails`, the screen-space kaleidoscope
(`kaleido_*`), `bloom_*`, `occlude` (how much of the figure's coverage the
backdrop resolves against), the frame `exposure`, and the final ink-on-paper
remap (`ink_*` / `paper_*`). Those are documented under
[Engine-wide controls](../presets/README.md#engine-wide-controls-plan-0018).

They run in a fixed order, which is worth knowing when a look does not compose the
way you expect:

```
        |------------------ linear light, unbounded -------------------|  |-- 0..1 --|
scene -> post chain (trails -> kaleidoscope -> bloom) OVER background -> [transition blend] -> tonemap/exposure -> ink -> present
```

Everything up to and including the post chain is **per preset** — during a dissolve
each side composites its own backdrop and chain, independently. The blend, the
tonemap and the ink remap are **engine-wide**: one pass each, over the frame both
presets produced.

**Everything left of the tonemap is floating-point linear light** (Plan 0045), so
an additive accumulation is free to exceed 1.0 and no hand-off clips; the tonemap
is the single place the frame becomes a displayable picture. That is why
`bloom_threshold` can mean "brighter than the display could show" and why stacked
strokes roll off with their colour intact instead of flattening to white.

**The post chain renders at the render target's own resolution.** `trails` and
`kaleido_*` used to run through a fixed 1280x720 grid, so composing either one
upscaled the whole frame on any display above 720p — sharp geometry went in and soft
pixels came out. They follow the target now (quantized and capped), so a look no
longer costs sharpness.

**A stage changes softness, never shape.** The internal grid is a resolution, not a
proportion: your scene is drawn at the *window's* aspect and the stage's present
stretches it back by exactly the inverse, so the grid's own ratio cancels out and a
circle stays a circle whether `trails` is on or off (ADR-0037). Plan 0035 made that
true; before it, composing a stage stretched the whole frame by up to 28 % on common
window sizes, which is worth knowing if you are reading a preset comment written
earlier that blames a stage for a figure's shape.

Two things still follow from a stage being a *resample*:

- On a **line** scene, `mirror_order` / `mirror_reflect` replicate real geometry
  *before* rasterization, so they cost nothing in resolution, while `kaleido_*` folds
  finished pixels. Prefer the mirror when either would do — see
  [Mirror or kaleidoscope?](../presets/README.md#mirror-or-kaleidoscope-they-are-not-the-same-cost).
- **Reaction-diffusion is a special case in the other direction.** Its simulation grid
  is deliberately fixed and independent of the window (ADR-0012), but the field is
  **toroidal**, so `pan_*` is a seamless infinite scroll and `zoom > 1` tiles rather
  than running out of field.

### The `[spectrum]` table

The `spectrum` system is the one whose *figure* is the analysis rather than a
generator's geometry, so what it draws is chosen structurally rather than through
`[params]`:

```toml
system = "spectrum"

[spectrum]
elements  = 26                                  # 2..=64, default 24
layout    = "bars"                              # bars | polyline | radial_ring
smoothing = { attack = 0.025, release = 0.22 }  # seconds; default: instant
```

| Key | Values | Notes |
|-----|--------|-------|
| `elements` | integer `2..=64` | How many elements the frequency axis is divided into. Default 24. |
| `layout` | `bars`, `polyline`, `radial_ring` | Default `bars`. |
| `smoothing` | seconds, or `{ attack, release }` | Per-element easing, in the same vocabulary as `[smoothing]`. |

Every key is optional and so is the table itself. An out-of-range `elements` or
an unknown `layout` is a **surfaced load error** naming what it expected, like
every other structural table — never a silent fallback.

Element 0 is the bottom of the spectrum and the last is the top, so no expression
maps audio to position; that mapping is the scene. Three things follow that are
worth knowing before you tune one:

- **64 is the ceiling because 64 is the data.** The engine analyses 64 log-spaced
  bands and the scene averages each element's own contiguous slice of them — a
  real partition, nothing dropped or double-counted. A readout finer than its own
  data would be a lie rather than a feature.
- **The axis is only half logarithmic, and the low end is the coarse end.** Band
  edges follow a log curve, but each band is floored at one FFT bin (23.4 Hz at
  48 kHz), which binds all the way up to ~750 Hz — so the bottom **31 of the 64
  bands are linear**. Element 0 therefore covers about an octave while an element
  near the middle of the figure covers a semitone or two. This is the opposite of
  what a log axis suggests, and it is the same caveat
  [`bin(x)`](#binx--reaching-the-spectrum) carries — the measured mapping is
  tabulated there.
- **`smoothing` here is the one easing an expression cannot reach.** The element
  levels are scene state computed from the band array, not a binding, so
  `[smoothing]` has no name to attach to them. Asymmetric values earn their keep:
  the bands are the rawest signal in the engine, so a fast `attack` keeps a
  transient's shape while a slow `release` lets the elements fall like a meter
  instead of strobing on every analysis hop. It eases the level *after* the
  `curve` param has shaped it — the value you see — so a `release` means the same
  duration whatever `curve` is set to
  ([ADR-0040](adrs/0040-spectrum-level-curve-applies-before-the-easing.md)).

This is also the one system that reads **per-element bindings**: a `[params]`
expression naming [`index`](#index--one-binding-evaluated-once-per-element) is
evaluated once for each element, so the relationship between a frequency region
and what is drawn there is preset content rather than scene code.

Full parameter notes are in
[`presets/README.md`](../presets/README.md#spectrum--the-frequency-axis-readout-plan-0034),
including which composite controls this system honors, the layout-specific
parameters (`radius` on the ring; `span` and `baseline` on `bars`/`polyline`), and
the `curve`↔`scale` retune a level curve costs — a **5.8x** amplitude change at
`curve = 0.5` against measured typical band levels, which is the reason the
default is exactly linear.

### The `[feedback]` table

The engine has **two accumulation buffers** — the `trails` post stage, which every
scene composites through, and the `attractor` scene's own internal trail field —
and both read their past through the same transform
([ADR-0048](adrs/0048-transformed-feedback.md)). The moving parts of that
transform are ordinary bindables (`fb_zoom`, `fb_rotate`, `fb_dx`, `fb_dy`,
`fb_center_x`, `fb_center_y`, `fb_warp`, all documented in
[`presets/README.md`](../presets/README.md#transformed-feedback--fb_zoom-fb_rotate-fb_dx-fb_dy-fb_center_x-fb_center_y));
the two **structural** choices are this table:

```toml
[feedback]
warp  = "swirl"   # none (default) | swirl | ripple | fisheye
blend = "add"     # max (default)  | add
```

| Key | Values | Notes |
|-----|--------|-------|
| `warp` | `none`, `swirl`, `ripple`, `fisheye` | Which curated procedural distortion rides on top of the affine. Its *strength* is the bindable `fb_warp`. Default `none`. |
| `blend` | `max`, `add` | How this frame's light lands on the faded past. Default `max` — the engine's only blend until this table existed. |

Both keys are optional and so is the table. An unknown value is a **surfaced load
error** naming what it expected, like every other structural table — a warp kind
selects a shader path, and quietly falling back to `none` would render a look you
never asked for with nothing on screen to say so.

Two things about this table surprise people, and both are consequences of there
being two buffers rather than one:

- **One vocabulary, two sinks.** A single `fb_rotate` on an `attractor` preset
  that also binds `trails` turns **both** accumulations — the scene's field and
  the stage's — each about its own buffer, neither about the other's. That is
  deliberate (one thing to learn, and it transfers), but if you have both live and
  are attributing the motion to one of them, you are attributing half of it wrong.
  Turn `trails` off to see the scene's own.
- **`blend` reaches the trails stage only.** The attractor's deposit has been
  additive since the scene was written — its points draw through an additive
  pipeline over the decayed bed — so there is no `max` to select there. `warp`
  reaches both.

The transform applies to the **past**, never to the light being deposited this
frame: the fresh figure is always where the scene put it, and only the trail
behind it travels.

### Transitions between presets

A preset switch dissolves rather than cuts (ADR-0024 / ADR-0032). Nothing about it
is preset-authored today — duration and blend kind are engine policy in code — but
two consequences reach your file:

- **Your preset renders live from the dissolve's second frame.** It is composited
  through its own background + post chain the whole way, so a `trails` accumulation
  starts from empty at the switch, not from the outgoing preset's history.
- **`ink_*` / `paper_*` crossfade.** There is one ink pass for the blended frame, so
  its params travel from the outgoing preset's values to yours across the dissolve.
  A preset whose look *depends* on ink reaches it about a second after the switch.

Per-preset `[transition]` declarations are a deliberate follow-up, not a gap you can
work around from the TOML.

---

## The expression language

Each parameter value is a tiny arithmetic expression, compiled once when the
preset loads and evaluated every frame. It is deliberately small: pure,
allocation-free, and **total** (evaluation never panics), so it is safe to run
per parameter per frame during a live show.

### Grammar

```text
expr   := sum  (('>' | '<' | '>=' | '<=' | '==' | '!=') sum)*
sum    := term  (('+' | '-') term)*
term   := unary (('*' | '/') unary)*
unary  := ('-' | '+')? primary
primary:= number | ident | ident '(' expr (',' expr)* ')' | '(' expr ')'
```

- **Arithmetic:** `+` `-` `*` `/`, unary `-` / `+`, and parentheses `( )`, with
  the usual precedence.
- **Comparisons** sit at the **lowest** precedence, below `+`/`-`, so
  `1 + 1 > 3 - 2` reads as `(1 + 1) > (3 - 2)`.
- **Numbers:** decimal `f32` literals (`0.3`, `14`, `1.8`).

There is no way to read wall-clock time — the only clock is `time`, the
renderer's shared scene clock. Randomness exists, but only the **seeded** kind:
`hash(x)` and `noise(x)` are pure functions of their argument and the preset's
seed ([below](#hashx-and-noisex--seeded-randomness)), so a preset is still
reproducible given the same audio and the same seed.

### Variables

Eighteen read-only variables carry the live audio analysis into your expressions,
plus five that carry **position** rather than sound: `index` for a per-element
evaluation, and `x`/`y`/`rad`/`ang` for a per-vertex one.

| Variable | Meaning | Notes |
|----------|---------|-------|
| `bass` | Bass-band level (~20–250 Hz), as a fraction of its own recent peak. | **`0–1`.** `> 0.5` means "loud for this track". |
| `mid`  | Mid-band level (~250–4000 Hz), same basis. | `0–1`. |
| `treb` | Treble-band level (~4–18 kHz), same basis. | `0–1`; reads lowest of the three on most material. |
| `onset` | Onset/attack strength, as a fraction of its own recent peak. | `0–1`. A transient, not a level — spikes on hits. |
| `beat` | `1.0` on a hop where a beat fired, else `0.0`. | A gate: `beat * k` adds `k` only on beat frames. |
| `bar` | **Beat** phase in `[0, 1)`: `0` on each beat, ramping to the next. | A misnomer kept for compatibility — for real bar position use `bar_phase`. |
| `time` | The scene clock in seconds (monotonic). | Use `time * k` for slow drift; `k` sets the speed. |
| `tempo` | Tracked tempo in **BPM**. | **Not a `0–1` band** — see the warning below. |
| `novelty` | Spectral-change transient: ~`0` within a steady segment, spiking at a track/section boundary. | **Experimental** — see below. |
| `bass_raw` | Absolute bass magnitude — the pre-normalization value. | **Raw and small** (mean `0.040`): multiply up and clamp. |
| `mid_raw` | Absolute mid magnitude. | Raw and small (mean `0.006`). |
| `treb_raw` | Absolute treble magnitude. | Raw and small (mean `0.006`). |
| `onset_raw` | Absolute spectral flux. | Raw and very small (mean `0.002`). |
| `beat_index` | Monotone **onset** counter, `0` before the first detection. | Integer-valued, and **not a musical period** — `mod(beat_index, 4)` is not "every 4th beat". See [Musical time](#musical-time). |
| `time_since_beat` | Seconds since the last beat; exactly `0` on a beat hop. | A retriggered ramp — good for decays. |
| `beat_in_bar` | Which beat of the bar, `0`–`3`. | `beat_in_bar == 0` is the downbeat. |
| `bar_index` | Bar counter — monotone except across an alignment change. | `mod(bar_index, 8)` for an 8-bar arc, which a lock can repeat or drop one bar of. |
| `bar_phase` | Position across the whole bar, `[0, 1)`. | The genuine bar phase, unlike `bar`. |
| `index` | The element's own position in `[0, 1]` during a **per-element** evaluation. | Not audio. `0` everywhere else — see [below](#index--one-binding-evaluated-once-per-element). |
| `x` / `y` | The vertex's position in `[0, 1]` during a **per-vertex** evaluation; `y = 0` is the top. | Not audio. `0` outside a `[per_vertex]` table — see [below](#per_vertex--one-binding-evaluated-once-per-mesh-vertex). |
| `rad` | That vertex's distance from the centre, aspect-corrected against the render target. | `1.0` at the middle of the top and bottom edges on any display; further at the sides of a wide one. |
| `ang` | That vertex's angle from the centre, `[0, tau)`, counter-clockwise from +x as you look at the screen. | `0` outside a `[per_vertex]` table. |

**The four headline levels are normalized** (ADR-0049): each is divided by its own
slowly-decaying running peak, with a silence floor so a quiet room reads `0` rather
than amplified noise. That is what makes a threshold portable across tracks and
gain staging. The cost is deliberate — absolute dynamics are hidden, so a quiet
passage and a loud one read alike. When a look *should* scale with real loudness,
use the `*_raw` twin and expect the old tiny magnitudes.

The five musical-time variables come from the beat tracker, and the three
bar-position ones sit behind a confidence gate with a counter fallback, so they are
always periodic and never confidently wrong about the music (ADR-0050). 4/4 is
assumed. **None of the five is a dependable musical period.** `beat_index` counts
onset detections rather than beats (ADR-0109), and the bar trio is the counter
fallback most of the time — both measured, and diagnosed, in
[Musical time](#musical-time) below. Read that section before you build structure
on any of them.

> **`tempo` is the one variable that is not roughly `0–1`.** It is `0` until the
> tempo tracker warms up (the first several seconds of audio), then jumps to a
> real BPM in the ~`60–200` range. Using it raw will blow out any parameter.
> Either **scale** it (`tempo / 180`) or — better — **compare** it:
> ```
> select(tempo > 128, 2.5, 0.8)     # a fast look above 128 BPM, a calm one below
> ```

> **`novelty` is experimental.** It is a transient that spikes when the spectrum
> changes character — useful for gating an accent on a track or section change
> (`beat * (novelty > 0.5)`). Its DSP shape may change in a later release, or it
> may be withdrawn; do not build a preset that only works with today's exact
> values.

### Constants

| Constant | Value |
|----------|-------|
| `pi` | 3.14159265… |
| `tau` | 2π — a full turn, the natural unit for a cyclic `hue` or `rotation`. |

Constants resolve before the variable lookup, so they cannot be shadowed. A bare
identifier that is neither a constant nor a variable is a compile error.

### Functions

| Function | Args | Result |
|----------|------|--------|
| `sin(x)` | 1 | Sine of `x` (radians). |
| `cos(x)` | 1 | Cosine of `x` (radians). |
| `abs(x)` | 1 | Absolute value. |
| `floor(x)` | 1 | Largest integer ≤ `x`. |
| `sqrt(x)` | 1 | Square root. `sqrt` of a negative is `NaN` — guard it with `select` or `max(x, 0)`. |
| `log(x)` | 1 | **Natural** logarithm (base `e`). Same posture as `sqrt` at the edges: `log(0)` is `-inf` and `log(-1)` is `NaN` — guard with `select` or `max(x, tiny)`. There is **no `log10`**; divide by `ln(10)` = `2.302585`. See [Decibels](#decibels). |
| `min(a, b)` | 2 | Smaller of `a`, `b`. |
| `max(a, b)` | 2 | Larger of `a`, `b`. |
| `pow(base, exp)` | 2 | `base` raised to `exp`. Shape a response curve: `pow(bass * 8, 2)` is punchier, `pow(x, 0.5)` gentler. |
| `mod(a, b)` | 2 | **Floored** modulo, `a - b * floor(a / b)`. Divisor-signed, so `mod(-0.2, 1.0)` is `0.8` — it wraps cleanly for a cyclic hue or phase. |
| `clamp(x, lo, hi)` | 3 | `x` bounded to `[lo, hi]`. Total even if `lo > hi`. |
| `lerp(a, b, t)` | 3 | Linear blend `a + (b - a) * t`. |
| `smoothstep(e0, e1, x)` | 3 | Eased `0 → 1` ramp as `x` crosses `e0 → e1` (`0` below, `1` above). The easing primitive — smoother than `clamp` for a threshold. |
| `select(cond, x, y)` | 3 | `x` if `cond != 0.0`, else `y`. **Only the taken branch is evaluated.** |
| `bin(x)` | 1 | The **spectrum** at normalized position `x` (`0` = lowest frequency, `1` = highest), interpolated between adjacent bands. See below. |
| `hash(x)` | 1 | A **scattered** value in `[0, 1)`: neighbouring arguments give unrelated results. Seeded per preset — see below. |
| `noise(x)` | 1 | **Smooth** value noise in `[0, 1]`: a wander that changes over one unit of `x`. Seeded per preset — see below. |

Calling a function with the wrong number of arguments, or referencing an unknown
name, is a **compile error** — the preset is rejected at load and the app keeps
the previous good set (it does not crash). Division by zero yields `inf`/`NaN`
rather than panicking, but you should avoid it — a `NaN` parameter produces
undefined-looking visuals.

### `bin(x)` — reaching the spectrum

`bass`/`mid`/`treb` are three numbers for the whole audible range. `bin(x)` reads
the *same* analysis at whatever resolution you point it at: the engine's
log-spaced band array, sampled at a normalized position and interpolated between
the two adjacent bands. A preset never names the band count, so nothing here
breaks if the engine ever re-bands.

```toml
[params]
# Morph an attractor's shape from the low-mids rather than from a whole band.
a = "1.4 + bin(0.15) * 0.4"
# A treble-region shimmer, independent of what the bass is doing.
brightness = "0.6 + bin(0.85) * 0.8"
```

It is **total**: `bin(0)` is the lowest band, `bin(1)` the highest, anything
outside `0..1` clamps, and a `NaN` argument reads the lowest band. It never
errors and never rejects a preset at load.

> [!WARNING]
> **`bin(x)` is a narrow probe, not a region average.** One call sees about two
> of the 64 bands, so a handful of calls **spot-samples** a region rather than
> integrating it. That part has never changed. What did change is the axis
> underneath it — see the table, and note that **this page said the opposite
> until Plan 0048**.
>
> The band edges are laid out on a log curve from 35 Hz to 18 kHz
> (`core/src/dsp/fft.rs`, `edges_hz[k] = 35 × (18000/35)^(k/64)`), and since
> [ADR-0049](adrs/0049-analysis-v2-dual-resolution-axis-normalized-bands.md) a
> second, longer analysis window feeds every band below the **246 Hz crossover**,
> so no band is starved and **the curve is the truth end to end**. Before that,
> each band was floored at one FFT bin (23.4 Hz at 48 kHz), which bound the
> bottom half of the axis *linear* — the regime this page used to describe, and
> which no longer exists.
>
> Derived from `fft.rs`'s edge formula and `expr.rs`'s `bin()`, which places the
> probe at band-space position `x × 63` and interpolates — so what it listens to
> is the centre of that interpolation, not of one band:
>
> | `x` | lands on | band width there |
> |-----|----------|------------------|
> | `0.00` | ~37 Hz | 4 Hz (1.8 semitones) |
> | `0.10` | ~68 Hz | 7 Hz (1.8 semitones) |
> | `0.20` | ~126 Hz | 13 Hz (1.8 semitones) |
> | `0.31` | ~247 Hz | 25 Hz (1.8 semitones) — the crossover sits here |
> | `0.41` | ~457 Hz | 47 Hz (1.8 semitones) |
> | `0.50` | ~794 Hz | 81 Hz (1.8 semitones) |
> | `0.75` | ~3.7 kHz | 378 Hz (1.8 semitones) |
> | `0.84` | ~6.4 kHz | 657 Hz (1.8 semitones) |
> | `1.00` | ~17.1 kHz | 1.8 kHz (1.8 semitones) |
>
> - **Every band is 1.8 semitones wide, everywhere.** That uniformity *is* the
>   axis being genuinely logarithmic, and it is what the second window bought.
>   The old array's bottom was its coarsest region — band 0 spanned a full octave
>   in one number — and that is simply no longer the case.
> - **`35 × 514.3^x` is now accurate across the whole axis**, to within a few per
>   cent. The old warning that it is "up to 2.9× wrong below the crossover" was
>   true of the old layout and is false of this one. The residual is the `x × 63`
>   step and the half-band interpolation offset; if you need better than ~5 %,
>   read the table rather than the formula.
> - **The mapping no longer moves with the sample rate.** `hi = min(18 kHz,
>   sr × 0.45)`, so at 44.1, 48 and 96 kHz the edges are identical. (What *does*
>   still vary with rate is the resolution behind the low bands, since both
>   windows are sized in samples — [design-backlog 0032](design-backlog.md).)
> - **`bin(0.02)` is still not "the kick".** It is a ~1.7-semitone sliver near
>   40 Hz. Sweep the value while listening rather than assuming.
>
> **Averaging a few calls does not integrate a region — it spot-samples one.**
> Measured against a 6.5 kHz tone: `bin(0.84)` reads `0.094` while `bin(0.82)`
> and `bin(0.88)` both read **exactly zero**. Samples a few hundredths apart in
> `x` step *over* bands rather than across them, so a narrow peak between two
> probes is invisible to all of them. There is no range-integrating companion
> (`bin_range(lo, hi)`) yet — until there is, use `bin()` when you want
> **selectivity** and `bass`/`mid`/`treb` when you want a **region**. Combining
> both is usually right: a band scalar for the body, a `bin()` term for the
> detail, so the parameter never goes still on material that misses the probe.

Values come off the same normalization as the bands: a full-scale sine reads near
`1.0` in its band, and ordinary music reads **small**, so multiply up and clamp
exactly as you would with `bass`.

### `hash(x)` and `noise(x)` — seeded randomness

Two functions give a preset variety no arithmetic can fake. Both are **pure**:
the result depends on the argument and on the preset's seed, and on nothing else.
Call either twice with the same argument in the same frame and you get the same
number — they are dice you rolled once, not dice you keep rolling.

| | shape | reach for it when |
|---|---|---|
| `hash(x)` | a **scatter** in `[0, 1)` — neighbouring arguments are unrelated | you want something *different* per beat, per bar, per element |
| `noise(x)` | a **wander** in `[0, 1]` — smooth, changing over one unit of `x` | you want a parameter to drift organically instead of cycling |

```toml
[params]
# A wander that never repeats — one call replacing a sum of detuned sines.
# The coefficient is the speed: 0.3 moves noticeably, 0.03 is a slow tide.
hue = "noise(time * 0.3)"

# A different value twice a second: floor(time * 2) steps to a new integer
# every 0.5 s, and hash turns each of those into an unrelated number.
burst = "0.4 + hash(floor(time * 2)) * 0.6"

# Scatter a per-element readout so it stops looking combed (see `index`).
thickness = "2 + hash(index * 64) * 3"
```

**Layer `noise` for a richer wander.** One call is one octave — smooth and a bit
plain. Sum two or three at different rates and amplitudes and you get something
that reads as organic:

```
noise(time * 0.11) * 0.6 + noise(time * 0.43) * 0.3 + noise(time * 1.7) * 0.1
```

Give the calls **different arguments**, not just different multipliers of the
same one, if you want two parameters to wander independently — `noise(time * 0.2)`
and `noise(time * 0.2 + 50)` are two unrelated wanders at the same speed.

#### The seed

`[generator] seed` decides the scatter. It has been accepted (and ignored) since
the L-system landed; it now salts `hash`/`noise` for **any** system, so a preset
that is not an L-system can carry a `[generator]` table containing nothing else:

```toml
[generator]
seed = 12          # any non-negative integer: the same look, every time
# seed = "random"  # a different look every time the preset loads
```

- **No seed** means seed `0` — a perfectly good scatter, and what every preset
  had before this existed.
- **A number** makes the preset reproducible: same audio, same picture, forever.
  Two presets with the same expression and different seeds look different.
- **`"random"`** draws a fresh seed each time the preset **loads** — app start,
  and every hot reload of the preset folder, so it re-rolls on each save while
  you are editing. It is drawn once, at load — never per frame.

> **`"random"` and captures.** Every capture path — `shot`, the golden baselines,
> `--report`, the behavioral gates — forces the numeric seed (`0`) so its output
> stays reproducible. So a `seed = "random"` preset's captured frame is **not**
> the frame you saw live: same statistics, different instance. Tune with a
> number, switch to `"random"` at the end if you want the surprise.

### `index` — one binding, evaluated once per element

`bin(x)` lets an expression read *a* frequency region. `index` lets one
expression read *every* region — it is the only variable that is not audio. On a
system that draws N elements (today that is [`spectrum`](#the-spectrum-table)),
a binding whose text mentions `index` is evaluated **once per element**, with
`index` bound to that element's normalized position: `0` at the first element,
`1` at the last, evenly spaced between.

```toml
system = "spectrum"

[params]
# The stroke thickens where that element's own band is loud.
thickness = "0.01 + bin(index) * 0.05"
# A resting length that grows toward the top of the axis, where music has less
# energy, so the high elements read instead of sitting stubbed.
base      = "0.16 + index * 0.12"
# No `index` -> one evaluation, one value, every element the same.
brightness= "0.9 + bass * 3"
```

`index` is normalized rather than an element count on purpose: `bin(index)` maps
the axis onto itself, so the same preset is correct whether it draws 8 elements
or 64, and you never write the count into an expression.

Things worth knowing before you reach for it:

- **Outside a per-element evaluation, `index` reads `0`.** Naming it on any other
  system is not an error and not a warning — the binding simply evaluates once
  with `index = 0`, which is the value the first element would have seen.
- **`index` normalizes over the span, `hue_spread` over the count.** `index` is
  `i/(n-1)`, so the last element is exactly `1.0` — that is what makes `bin(index)`
  cover the whole spectrum end to end. The scene's own `hue_spread` walk is `i/n`,
  which is what keeps its steps even around a closed figure. So `hue = "index"` is
  **not** the same as `hue_spread = 1`: the former lands the last element on the
  same colour as the first. See
  [preset-palettes.md](preset-palettes.md#spectrum--colour-along-the-frequency-axis).
- **Not every parameter is per-element.** On `spectrum` the ones describing a
  single element vary per element — `base`, `scale`, `curve`, `thickness`,
  `brightness` and `hue`. The whole-figure ones — `span`, `baseline`, `radius`,
  `rotation`, `glow`, `hue_spread`, `palette_mix`, `saturation`, and the view
  transform / mirror — take the `index = 0` value of the series instead of
  silently dropping the binding.
- **`[smoothing]` cannot ease a per-element binding**, because the smoother holds
  one scalar and a series has no single value. Listing one is a surfaced load
  **warning**, not a silent no-op; the easing you want is
  [`[spectrum] smoothing`](#the-spectrum-table), which eases the element levels
  themselves.
- **Cost is bounded and small**: N × (per-element bindings) evaluations per frame,
  allocation-free — at the default 24 elements it is a low-microsecond fraction of
  a 60 Hz frame. A preset that names `index` nowhere costs exactly what it did
  before this existed.

### `[per_vertex]` — one binding, evaluated once per mesh vertex

`index` gives a binding one axis to vary along. `[per_vertex]` gives it **two**,
and it belongs to exactly one system: [`warp_mesh`](../presets/README.md#warp_mesh--the-past-resampled-through-a-per-vertex-grid-plan-0100),
which covers the frame with a grid and resamples the previous frame through it.
A binding in that table is evaluated once per grid **vertex** per frame, with
`x`, `y`, `rad` and `ang` bound to that vertex's own position.

```toml
system = "warp_mesh"

[mesh]
x = 32
y = 24

[per_vertex]
# The past expands harder at the rim than at the centre — a tunnel that opens
# out. No single `fb_zoom` can say this.
zoom = "1.9 + rad * 0.9"
# ...and turns at a rate that depends on which way round the frame you are.
rot  = "0.35 + sin(ang) * 0.30"
```

The nine outputs it accepts — `zoom`, `rot`, `cx`, `cy`, `dx`, `dy`, `sx`, `sy`,
`warp` — are the per-vertex generalization of the `fb_*` affine (ADR-0048), and
[`presets/README.md`](../presets/README.md#the-per_vertex-table) is the
authoritative table for what each does.

**Converted MilkDrop presets do not ship** (Plan 0100 Phase 8, user's call 2026-08-16: decided
later, nothing distributed now). The public `.milk` collections have no clear licensing, so no
converted preset enters this repository or a release; the `milkconv` converter plus a directory
you point `LMV_PRESET_DIR` at is the whole import path. The question is re-raised when the
conversion-fidelity backlog (design-backlog 0106–0108) is worked off.

> **If you already have a converted directory, re-run `milkconv` over it.** The feedback field now
> emulates the reference's 8-bit floor
> ([ADR-0118](adrs/0118-the-milkdrop-feedback-field-quantizes-in-the-encoded-domain.md)) — without
> it, every dim residual survives in the float field and integrates, which is what turned classic
> presets pastel, white-hot, or tonally inverted. The switch is a runtime uniform, so **an MD1-era
> preset — one with no `warp_shader` of its own — picks the fix up with no re-convert at all.** What
> needs re-running is a bundle that *does* carry a converted warp shader: its epilogue was emitted
> before the quantizer existed and never calls it, so the uniform reaches nothing. Nothing breaks in
> the meantime; that preset renders exactly as it does today.
>
> **A second reason to re-run, as of Plan 0109: the video echo now exists.** `fVideoEchoAlpha`,
> `fVideoEchoZoom` and `nVideoEchoOrientation` were warned about as unconsumed and are now read by
> the present pass — a second sampled copy of the frame, zoomed and flipped, summed over the first.
> This one *always* needs the re-convert, at every era: the three values are seeded into the bundle
> at conversion time, so a bundle emitted before this has no register for the scene to read. Only
> 2.4 % of the corpus sets a non-zero echo alpha, and where it does the preset is usually
> unrecognisable without it.

Things worth knowing before you reach for it:

- **Each of the nine is also an ordinary scalar param, and the table overrides
  it.** Bind `zoom` in `[params]` and the whole mesh takes that value; bind it in
  `[per_vertex]` as well and the table wins, vertex by vertex. So a preset starts
  from one shared transform and opts into a varying one **output at a time**.
- **Outside a `[per_vertex]` table, `x`/`y`/`rad`/`ang` read `0`** — the same rule
  `index` takes outside a per-element evaluation. Unlike `index`, naming one in
  `[params]` is a load **warning**, because a `rad` that silently reads zero is a
  much more surprising thing to debug than an `index` that does.
- **`rad` and `ang` are aspect-corrected against the render target**, never
  against the mesh (ADR-0037), so a `rad`-driven figure is round on a 16:9
  monitor and round on a 5:4 one.
- **`[smoothing]` cannot ease a per-vertex binding**, for the reason it cannot
  ease a per-element one: the smoother holds one scalar and a series has no single
  value. Listing one is a surfaced load warning.
- **Cost is the reason `[mesh]` is capped.** It is `(x+1) × (y+1) ×
  (per-vertex bindings)` evaluations per frame on the render thread — thousands,
  not dozens — so the grid is a **tier capacity** rather than an open number, and a
  preset asking for more than the tier carries gets the tier's ceiling without an
  error. The measured ladder behind those numbers is on `TierConfig::mesh_grid`.

### Comparisons and branching

The six comparison operators — `>` `<` `>=` `<=` `==` `!=` — each yield exactly
`1.0` (true) or `0.0` (false), so they compose with arithmetic:

```
0.4 + (bass > 0.2) * 0.3        # 0.4 normally, 0.7 once the bass crosses 0.2
```

`select(cond, x, y)` picks between two whole expressions. Because it evaluates
**only** the branch it takes, it is also the way to guard a partial function:

```
select(bass > 0.5, 3.0, 0.8)          # a threshold switch
select(x >= 0, sqrt(x), 0)            # safe — the untaken sqrt never runs
```

That last property is why `select` exists rather than a `lerp`-based blend: a
blend evaluates both sides, so an out-of-domain `sqrt` would poison the result
with `NaN` even on the branch you did not want.

**There are no boolean operators** (`&&`, `||`, `!`) — with clean `0`/`1`
comparison results they add nothing:

| You want | Write |
|----------|-------|
| `a AND b` | `min(a, b)` |
| `a OR b` | `max(a, b)` |
| `NOT c` | `1 - c` |

```
min(bass > 0.3, tempo > 120)          # loud AND fast
max(beat, onset > 0.6)                # on a beat OR a strong transient
```

**Chained comparisons are legal but rarely what you mean.** `a > b > c` parses
left-associatively as `(a > b) > c`, comparing a `0`/`1` against `c`. Write
`min(a > b, b > c)` instead.

#### Set the threshold from a measured level, not from `--set`

**This used to be the single most expensive mistake an author could make here**,
and ADR-0049 largely retired it. `bass`, `mid`, `treb` and `onset` are now each a
fraction of their own slowly-decaying recent peak, so they genuinely span `0..1`
and a threshold means the same thing on every track:

| variable | mean | max | so a live threshold sits… |
|---|---|---|---|
| `bass` | 0.661 | 1.000 | around `0.7`–`0.95` |
| `mid` | 0.575 | 1.000 | around `0.6`–`0.95` |
| `treb` | 0.281 | 1.000 | around `0.3`–`0.9` |
| `bass + mid + treb` | 1.517 | 3.000 | around `1.6`–`2.6` |
| `bin(x)` | 0.089 | 1.000 | around `0.15`–`0.6` |

Measured 2026-07-30 over `--signal dynamic:110`; re-measure any time with
`shot --signal dynamic:110 --out strip.png`, which prints the table.

Two traps remain, and they are the mirror image of the old one.

**A threshold can now be too LOW.** The old failure was a gate above anything
music produced, so it never fired. On the v2 scale the commoner failure is a gate
*below* the typical level, which fires always — the `else` branch becomes dead code
instead of the `then`. Nine bindings across the shipped library were in exactly
that state after ADR-0049 landed, every one a threshold written for raw levels
that normalized values then cleared constantly; Plan 0048 Phase 7 retuned them.

**`bin(x)` is not on the scalars' scale.** The band array normalizes against one
peak shared by all 64 bands — which is what keeps `bin(hi) - bin(lo)` a meaningful
contrast — so a single band only reaches `1.000` when it is the loudest in the
frame. Its typical value is `0.089`. A threshold tuned on `bass` is roughly 7×
too high for `bin()`.

`--set bass=1` writes the band straight onto the analysis frame, and since v2 that
is a *reachable peak* rather than a fiction — so calibrating against a `--set`
capture is now reasonable, remembering it is a held peak:

```
select(bass + mid + treb > 2.8, 24, 6)   # near the 3.0 ceiling: fires rarely
select(bass + mid + treb > 0.075, 24, 6) # below the 0.078 minimum: the constant 24
select(bass + mid + treb > 1.9, 24, 6)   # a real gate
```

Before v2, six shipped presets had their defining mechanism disabled by the
opposite error for months — `fragment_kaleido` never left 6 folds, `reaction_reef`
never folded at all — and all six scored **healthy** in `--report`, because its
stimuli were full-scale too. That whole class is what normalization removes.

#### Absolute level, when you actually want it

`bass_raw`, `mid_raw`, `treb_raw` and `onset_raw` carry the pre-v2 magnitudes,
unchanged: means of `0.040 / 0.006 / 0.006 / 0.002` against maxima of `0.106 /
0.019 / 0.032 / 0.016`. Normalization deliberately hides absolute dynamics — a
quiet track and a loud one read alike — so reach for a `*_raw` when a look *should*
scale with real loudness. Everything the old warnings above said about tiny levels
and unreachable thresholds applies to these in full.

#### Musical time

Five variables place you in the music rather than measuring it (ADR-0050):

| variable | range | meaning |
|---|---|---|
| `beat_index` | `0`, `1`, `2`, … | monotone **onset-detection** counter — see the trap below |
| `time_since_beat` | seconds | `0` exactly on a detection, climbing to the next |
| `beat_in_bar` | `0`–`3` | which beat of the bar this is |
| `bar_index` | `0`, `1`, `2`, … | bar counter (see the note below on monotonicity) |
| `bar_phase` | `0`–`1` | position across the whole bar |

Two naming traps live here, not one. `bar` is **beat** phase under a historical
name, kept because too much shipped content binds it; `bar_phase` is the real bar
position. And `beat_index` / `time_since_beat` are named for beats but driven by
the **onset detector**, which is the larger trap of the two — the section below it
is about exactly that.

These make phrase-scale structure a one-liner:

```
select(beat_in_bar == 0, 1.4, 1.0)        # accent every downbeat
0.5 + 0.5 * sin(bar_phase * tau)          # a sweep that breathes with the bar
select(mod(bar_index, 8) < 4, 0.2, 0.8)   # an 8-bar A/B alternation
1.0 - clamp(time_since_beat * 6, 0, 1)    # a decay retriggered by every detection
```

> **`beat_index` counts onsets, not beats, and no fixed multiplier converts
> between them.** It increments on the onset detector's flag —
> `flux > mean + 1.5 sigma` with a 96 ms refractory and no tempo gating — so a
> hi-hat, a snare rattle and a chord change each advance it. Measured against the
> real beat on live material: **1.35x–2.10x** detections per musical beat across
> Plan 0086's three genres and **1.20x / 1.22x / 2.28x** across Plan 0095's, and
> it wanders between 1x, 2x and 4x *within a single track*. That instability is
> the finding: there is no "about twice as often" to correct for.
>
> **So `mod(beat_index, N)` never means N beats.** `mod(beat_index, 16)` is not
> four bars of four — this document said it was, and that is retracted. What
> `beat_index` *is* good for is anything that only wants to change on activity
> without claiming a period: `hash(beat_index)` to re-roll a colour or a count on
> each hit, or a modulus chosen as a rough "every so often" and read as such.
> See [ADR-0109](adrs/0109-the-beat-clock-counts-onsets-not-beats.md).

`beat_in_bar`, `bar_index` and `bar_phase` come from a downbeat estimator that
publishes **only while it is confident**, and falls back to plain counters
otherwise. So they are always periodic and always usable, and never confidently
wrong about where the bar starts — you cannot see which mode is active,
deliberately, and you do not need to. 4/4 is assumed.

> **How often is it locked? Roughly 2–4 % of hops on material with bar-scale
> accents, and near zero on material without.** Since Plan 0095 the estimator
> folds accent history over a **tempo-driven bar grid** instead of over
> `beat_index`, so the four alignments it chooses between are alignments of a unit
> that is a **stable multiple of the beat** rather than a wandering one — a real
> bar when the tempo estimate is on the right octave, half or double one when it
> is not. Re-measured through the live app on three genres, each paired against a
> reconstruction of the pre-0095 fold on the same capture, the share of hops over
> the `0.25` confidence gate moved:
>
> | | rock/pop | hip-hop | techno |
> |---|---|---|---|
> | over the gate, old → new | 0.00 → **2.36 %** | 0.79 → **3.67 %** | 4.16 → **0.42 %** |
>
> **The hip-hop column carries that caveat.** Its capture read a `bpm` median of
> 165 on a track that counts at ~90 — an octave high, which
> [ADR-0109](adrs/0109-the-beat-clock-counts-onsets-not-beats.md) records as the
> half of the ambiguity the autocorrelation has no evidence to settle — so the
> grid's "bar" there spanned two musical beats. A stable two, not a wandering
> 1.35-2.10, which is the whole improvement; but not a bar.
>
> **Techno declining is the repair working, not a regression.** Four-on-the-floor
> puts a kick on every beat, so there is no bar-scale accent structure to find; the
> old fold's 4.16 % came from bucketing a counter that ran at 2.28 detections per
> beat, making its "bar" 1.75 musical beats long and aliasing its four buckets onto
> the kick/hat alternation — a real 4-periodicity that is not a bar. A gate that
> shuts on material with no bar accent is the honest outcome, and a confidently
> wrong downbeat is what ADR-0050 calls worse than none.
>
> The remaining ceiling is diagnosed, not mysterious: the accent the estimator
> folds is 70 % bass band, on the assumption that the kick marks the bar. In
> four-on-the-floor the kick marks *every beat*; in a backbeat it marks *1 and 3*,
> a half-bar. See
> [ADR-0082](adrs/0082-the-downbeat-gate-holds-and-the-estimator-is-diagnosed-first.md)'s
> `Outcome` and [ADR-0109](adrs/0109-the-beat-clock-counts-onsets-not-beats.md).
>
> **What this means when you write a preset:** the four one-liners above are all
> still correct and safe, but most of the time `beat_in_bar == 0` fires on a beat
> the counter chose rather than one the music did, and it will not reliably agree
> with where *you* hear the downbeat. If a look must land on the real bar line, it
> cannot today. If it only needs a periodic four-beat pulse, these deliver it —
> just do not read the names as a promise about the music. And do **not** fall back
> to `mod(beat_index, 16)`: the bar trio is the closest unit to a bar there is, and
> `beat_index` is not a musical period at all.

**`bar_index` is monotone except across an alignment change.** It is
`(beat count - alignment) / 4` — where the beat count is the **bar grid's**, and
`beat_index` only while the grid warms up — and `alignment` moves on the beat the
estimator locks, drops back to the counter, or is overtaken by a challenger, so at
that one beat the counter can repeat a bar or skip forward one. It never moves by
more than a bar and hysteresis makes it rare (a challenger has to lead for three
bars first), but if you write `mod(bar_index, 8)` for an 8-bar arc, know that a
lock landing mid-phrase can repeat or drop one bar of it. That is the deliberate
trade: a repeated bar is a much softer failure than a downbeat on the wrong beat,
which is the whole reason the gate exists.
#### Check the gate you just wrote

`--report`'s reachability check walks every expression and names any comparison
that only ever took one value, any `select()` whose condition never went both
ways, and any `clamp()` ceiling the value never reached
([capturing.md](capturing.md#reachability-gates-the-probe-never-drove-both-ways)).
Run it before you ship a gate.

That covers the bare-comparison form too, which is the one this page tells you to
write: `reseed = "onset > 0.55"` holds no `select()`, and a threshold nothing
crosses makes it a boolean param stuck at `0` forever. It reports as a `COMP`
line ([ADR-0043](adrs/0043-reachability-reports-comparison-nodes.md)).

**Reachability cannot see a gain — a second statistic does.** A comparison is a
fork the walker can watch; a `clamp(bass * 16, 0, 0.3)` is not, and after ADR-0049
a multiplier written for the old raw magnitudes drives its ceiling from just above
silence and holds it there — a binding that reads as a constant while every gate
stays green. The whole shipped library was in that state when Plan 0048 Phase 7
measured it, so this is the failure to expect, not a hypothetical.

Since Plan 0056 the same traversal also records **occupancy** — the fraction of
hops a `clamp()` spends *at* its upper bound — which is the mirror of the
`ceils` finding and the more serious of the two
([ADR-0062](adrs/0062-clamp-occupancy-is-the-saturation-instrument.md)).
`--report`'s `occ` column names the binding, and `core/tests/saturation.rs` is a
**HARD** gate at occupancy `0.9`; a clamp that is genuinely meant to pin declares
`[occupancy] exempt = [...]`, which silences the gate and not the diagnostic. The
arithmetic is still worth doing while you compose, because the gate is deliberately
high: a term reaches its cap at `ceiling / multiplier`, and if that number is below
the typical level in the table above the term is a constant long before occupancy
`0.9` convicts it.

### Decibels

`log(x)` is a **natural** logarithm, and it exists mainly so a preset can reach a
decibel scale — audio level is perceptually logarithmic, and the raw bands are
not. There is deliberately **no `log10`**, so convert with `ln(10)`:

```
20 * log(x) / 2.302585                 # x as decibels (2.302585 is ln(10))
```

Worked against a real measurement: a typical band level is around `0.03`, and
`log(0.03)` is `-3.5066`, so that expression gives **-30.5 dB**. Silence is the
edge to respect — `log(0)` is `-inf`, and silence produces it every time the
music stops — so floor the input rather than the output:

```
20 * log(max(bass, 0.0001)) / 2.302585  # floored at -80 dB instead of -inf
```

`log` follows `sqrt`'s posture exactly: mathematically honest at the edges
(`log(0)` = `-inf`, `log(-1)` = `NaN`), with `max` and `select` as the guards.

**What the engine does if you don't floor it.** A non-finite value reaching a
parameter **snaps** rather than easing: if that parameter is listed in
`[smoothing]`, the smoother passes the value straight through and keeps no state
for that frame, so the binding tracks the input again as soon as it is finite.
It used to be worse — the value poisoned the smoother permanently, and the
binding stayed dead until you switched presets — which is why flooring the input
is still the right thing to write. The guard is a floor under the failure, not a
reason to skip yours: a `-inf` still reaches the scene for as long as the input
is silent, and what that looks like is the scene's business.

**This does not reach the `spectrum` scene's own element levels.** An expression
only sees what the grammar exposes, and those levels are scene state computed
after every binding has been evaluated. To shape them, use that scene's `curve`
parameter instead (see [Systems](#systems)); `log` is for shaping a value you can
already name.

### Idioms (patterns from the curated set)

- **Gain-then-bound** — turn a small raw band into a usable range:
  ```
  clamp(bass * 14, 0, 1.8)
  ```
  Multiply the raw band up, then clamp so a loud passage can't blow the parameter
  out. Nearly every reactive binding is a variant of this.

  On a **luminance** parameter — `brightness`, `glow`, `flash` — the clamp used to
  be the difference between a peak and a flat white rectangle, because the frame
  clipped per channel at 1.0. Since Plan 0045 the composite carries light past
  1.0 and an engine tonemap rolls it off with the hue intact, so overshooting is a
  soft loss of contrast rather than a cliff. Keep the clamp anyway: what it buys
  now is that the peak stays *proportionate* to the rest of the frame, and it is
  still the difference between a beat that reads as the music and one that reads
  as a camera flash. See the additive-ceiling note in
  [`presets/README.md`](../presets/README.md#linear-light-and-exposure-plan-0045).

- **Baseline + reactive** — a resting value plus an audio-driven add:
  ```
  0.4 + clamp((bass + mid) * 8, 0, 1.1)
  ```
  The constant is what you see in silence; the clamped term is the reaction.

- **Slow drift** — a parameter that wanders on its own:
  ```
  time * 0.03
  ```
  Small coefficients (`0.008`–`0.08` in the library) set how fast a hue rotates.
  That is a *ramp*, though — it only ever goes one way. For a drift that
  genuinely wanders, [`noise`](#hashx-and-noisex--seeded-randomness) says it in
  one call:
  ```
  noise(time * 0.3)
  ```
  Presets in this library used to fake that with a **sum of detuned sines** —
  four of them, with periods chosen not to line up, in `attractor_dejong` alone.
  That idiom still works and there is no need to go and rewrite it, but write new
  ones with `noise`: it is one term instead of four, and it has no period at all
  rather than a very long one.

- **Per-beat variety** — something different on each beat, not merely louder:
  ```
  0.4 + hash(floor(time * 2)) * 0.6
  ```
  `hash` turns each integer into an unrelated number, which no sine sum can do.

- **Beat gate** — add something only on beat frames:
  ```
  0.5 + beat * 0.8
  ```
  `beat` is `0` most frames and `1` on a beat, so this jumps by `0.8` on each beat.

- **Beat-phase breathing** — a smooth ramp between beats:
  ```
  1.0 + bar * 0.25
  ```
  `bar` sweeps `0 → 1` between beats, so `zoom` eases up and resets each beat.

- **Soft threshold** — `smoothstep` where `clamp` would snap:
  ```
  smoothstep(0.15, 0.45, bass)
  ```
  A `0 → 1` ramp that eases in and out instead of cornering.

- **Cyclic wrap** — keep a rotating value in one turn:
  ```
  mod(time * 0.2, 1.0)
  ```
  Floored `mod` never returns a negative, so a hue driven this way never jumps.

- **Two-mode preset** — one file that behaves differently by energy or tempo:
  ```
  select(tempo > 130, 1 + bass * 3, 1 + bass * 0.6)
  ```

### What an expression cannot do: shape a value *over time*

`smoothstep` eases a **value** as its input crosses a threshold. It cannot ease a
**trajectory**, because expressions are pure and stateless by hard invariant —
there is no previous frame to ease away from. Anything time-shaped therefore
lives in `[smoothing]`, not in the expression:

```toml
[params]
thickness = "0.9 + beat * 0.6"           # the target: snaps on every beat

[smoothing]
thickness = { attack = 0.02, release = 0.7 }   # the trajectory: hit, then glide
```

`attack` is the time constant while the value is **rising**, `release` while it is
falling or level; a plain `thickness = 0.3` means the same constant both ways.
That is what makes a percussive accent land in a frame or two and then decay over
most of a second — a single constant slows the rise exactly as much as the fall,
so there is no value that gives both.

> **A two-constant entry stops being a low-pass and becomes a rectifier.**
> Because rise and fall are treated differently, a symmetric input comes out with
> a DC offset: the parameter **rides above its input's mean** under sustained
> material. On a percussive accent that is the point. On a continuous parameter
> it is a surprise — a fast-attack `hue` drifts upward instead of tracking. Use
> the pair where you want a hit; leave anything continuous symmetric. Full table
> reference: [`presets/README.md`](../presets/README.md).

---

## When a preset is wrong

The engine distinguishes mistakes that make a preset meaningless from mistakes
that merely waste a line. Neither ever crashes a running visual (NFR 10).

**Hard errors — the file is rejected, the last good set is kept:**

- Malformed TOML.
- An unknown `system` name.
- An expression that fails to compile — an unknown identifier, a bad number, a
  wrong argument count, an unbalanced parenthesis, a stray character.
- An invalid structural table (`[curve]`, `[generator]`, `[particles]`,
  `[spectrum]`, `[palette]`, `[smoothing]`).

**Warnings — the preset still loads and renders:**

- **An unknown parameter name.** A binding whose name no system or engine stage
  consumes is reported at load, naming the parameter and the system, and the rest
  of the preset applies normally (ADR-0020):

  ```
  preset ...\my_first.toml: warning: unknown parameter 'wrap' for system 'fragment_field' (binding kept, but nothing reads it)
  ```

  This is why a typo no longer fails silently. It is a warning rather than a
  rejection on purpose: one mistyped character should not blank a scene
  mid-show, so the good bindings keep working while the mistake is surfaced.
  The standalone prints both errors and warnings to stderr on every load and
  hot-reload.

---

## Where preset files live

There are three copies of the curated set, and understanding the flow explains
why "edit once, both frontends see it" works.

```
  presets/*.toml                 core/build.rs -> EMBEDDED             per-user preset dir
  (repo, source of truth)  ──>   (globbed + include_str!'d      ──>    seeded on first run,
                                  into the binary)                     then loaded + watched
```

1. **`presets/` at the repo root — the source of truth.** These `.toml` files are
   what a contributor edits. Nothing reads them at runtime directly.

2. **`EMBEDDED` — generated at build time.** `core/build.rs` globs `presets/*.toml`
   and emits an `EMBEDDED` slice of `(filename, contents)` tuples, each
   `include_str!`'d, so the compiled binary always carries the curated set
   ([ADR-0022](adrs/0022-build-time-preset-embedding.md)). `default_presets()`
   parses these as the fallback the C-ABI / foobar path renders even with no
   preset directory present. **Adding a preset is dropping a file** — there is no
   list to edit and no count to bump.

3. **The per-user directory — what actually gets loaded.** On first run each
   frontend **seeds** this directory (writes every embedded preset that isn't
   already there — **never overwriting** your edits) and then loads and watches
   it. The standalone and the foobar plugin resolve the **same** path, so a
   preset you edit shows up in both.

   | OS | Preset directory |
   |----|------------------|
   | Windows | `%APPDATA%\light-music-visualizer\presets` |
   | macOS | `~/Library/Application Support/light-music-visualizer/presets` |
   | Linux/other | `$XDG_DATA_HOME/light-music-visualizer/presets` (or `~/.local/share/light-music-visualizer/presets`) |

### A custom preset folder: `LMV_PRESET_DIR`

Set the **`LMV_PRESET_DIR`** environment variable to point the Rust frontends at
any folder instead of the per-user directory above
([ADR-0014](adrs/0014-preset-dir-override-for-dev-iteration.md)):

```bash
# Windows (PowerShell) — run the app against a folder you keep elsewhere
$env:LMV_PRESET_DIR = "D:\my-presets"; cargo run -p standalone --release

# macOS / Linux
LMV_PRESET_DIR=~/my-presets cargo run -p standalone --release
```

- Both the **standalone app** and the headless **`shot` CLI** honor it, through
  one shared resolver — they cannot disagree about which folder an edit lands in.
- The override folder is **yours**: the app loads and hot-reloads it but **never
  seeds** the curated set into it. An empty or missing folder simply falls back
  to the presets compiled into the binary, exactly like an empty per-user
  directory.
- Only the presets move. `diagnostics.log` and `config.toml` stay under the
  per-user app directory.
- The **foobar2000 plugin does not read it** — the C++ side keeps resolving the
  per-user directory (a followup, not a current behavior).

Pointing it at the repo's own `presets/` is the preset-authoring loop: edit a
version-controlled `.toml` and the running window follows within ~150 ms, with no
rebuild. See [`capturing.md`](capturing.md#editing-presets-live) for that loop and
for `shot`'s equivalent `--presets` / `--preset-file` flags.

### Loading, cycling, and hot-reload

- **Seeding is write-if-absent.** Your edits to a seeded preset survive
  re-seeding. The flip side: a curated preset changed in a **new release** does
  **not** replace the copy already on your disk — delete that file and relaunch
  to get the updated version (there is no "refresh curated" button yet).
- **Hot-reload (standalone).** The app polls the directory ~every 150 ms and
  reloads on any change. Errors and warnings are printed and the last good set is
  kept — a bad edit never crashes a running visual.
- **Cycling.** Standalone: the app **holds one scene by default** (ADR-0027) —
  **Space** cycles to the next preset (title bar shows the name), and **`A`**
  toggles auto-rotate on/off (auto is off out of the box; enable it per-run with
  `A` or persistently via `auto = true` under `[rotate]` in `config.toml`).
  foobar2000: **Space**, or right-click the visualization → **Next scene**.
- **Choosing by name.** Both frontends select directly rather than cycling to it.
  Standalone: the browse overlay (`Tab`). foobar2000: right-click → **Preset ▸**,
  a flat list of everything that loaded with a mark on the one showing; the
  choice persists across restarts **by name**, so a preset whose file you later
  delete degrades to the roster's default rather than to a stale position
  ([ADR-0117](adrs/0117-c-abi-v6-the-host-reads-the-roster-and-selects-a-preset.md)).
  The list is the **core's roster**, not a directory listing — a `.toml` the
  engine rejected is absent from it, which is how you tell a malformed file from
  a missed one.
- **foobar loads on init, and re-loads on demand.** The plugin calls the core's
  `lmv_load_presets` (C ABI v2, [ADR-0006](adrs/0006-c-abi-v2-preset-loading.md))
  against the shared directory when it starts, so it seeds and renders the same
  library — no loopback capture needed on that path. It has no file watcher: a
  file dropped in afterwards appears on right-click → **Reload presets**, which
  re-scans and keeps the preset you were watching selected. A reload re-seeds the
  running scene's simulation state, the same way the standalone's hot-reload
  does.
- **Two files may declare the same `name`.** The roster tolerates it, but
  "select by name" then means *first match wins* — for persistence, for the
  reload's keep-selection step, and for the standalone's browse overlay alike.
  Rename one if you want both reachable.

---

## Keeping this current

**Adding, renaming, or retiring a preset touches two places:**

1. **`presets/<name>.toml`** — the preset file itself. That is all the *shipping*
   takes: `core/build.rs` globs the directory, so the embedded list and the
   preset-count test follow automatically (ADR-0022). There is no array to extend
   and no count to bump.
2. **[`presets/README.md`](../presets/README.md)** — if the preset showcases a
   control worth pointing an author at.

**Adding a parameter to a system** touches the scene's `set_param` match, the
`PARAMS` const beside it (the two are guarded against drift by
`declared_params_match_set_param` in `core/tests/preset.rs`), and the table in
[`presets/README.md`](../presets/README.md).

**Adding an expression variable, function, or operator** touches
`core/src/preset/expr.rs` and [The expression language](#the-expression-language)
in this file — **and every other place that re-types the roster**, in the same
commit: the short list in [`presets/README.md`](../presets/README.md), and the
`preset-author` skill's `SKILL.md` and `references/grammar.md`. Four copies of a
list is four chances to drift, and a stale roster silently costs the content lane
a capability. A change to the grammar is **ADR territory** (ADR-0002 fixed the
model; ADR-0020 grew it to v2) — flag it rather than quietly widening the
vocabulary here.

> **Format stability:** the app is pre-1.0 and in active development, so the
> preset format may still change between releases. Preset-format stability begins
> at 1.0.0.

---

## Related documents

- [ADR-0002 — Layered preset architecture](adrs/0002-layered-preset-architecture.md):
  the data/expression/script model and why it is layered.
- [ADR-0020 — Preset expression grammar v2](adrs/0020-preset-grammar-v2-branching-functions-tempo.md):
  the math functions, branching, `tempo`/`novelty`, and warn-but-load typo handling.
- [`presets/README.md`](../presets/README.md): the per-system parameter tables,
  engine-wide controls, structural config, and `[smoothing]`.
- [`docs/preset-palettes.md`](preset-palettes.md): the palette surface — built-in
  names, custom stops, and the A/B crossfade.
- [ADR-0006 — C ABI v2 preset loading](adrs/0006-c-abi-v2-preset-loading.md):
  how the foobar plugin reaches the shared library.
- [`docs/capturing.md`](capturing.md): the headless `shot` CLI for rendering a
  preset to a PNG without launching the app.
