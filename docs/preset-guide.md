# Writing presets — the illustrated guide

Everything this app draws comes from a **preset**: a small TOML file that names one built-in
rendering system and binds that system's parameters to short expressions over the live audio
analysis. No Rust, no shaders, no rebuild — you edit a line, save, and the running window picks it
up within about 150 ms.

This is the entrance. It shows you what the built-in systems look like, which of the three reference
documents owns which surface, and the loop you actually work in. It deliberately **reproduces no
table** from those references: where you need a parameter, a function or a palette name, this page
says which document owns it and links there
([ADR-0101](adrs/0101-the-preset-docs-gain-a-tutorial-layer-rather-than-a-merge.md)).

Where to go from here:

| You want | Go to |
|---|---|
| The parameter reference: every parameter, its default, its range, what it does | [`presets/README.md`](../presets/README.md) |
| The expression grammar — variables, functions, operators | [`docs/presets.md`](presets.md) |
| Palettes, custom stops, the A/B crossfade | [`docs/preset-palettes.md`](preset-palettes.md) |
| One preset tuned step by step, with the numbers | [`docs/preset-tuning-walkthrough.md`](preset-tuning-walkthrough.md) |

> **Every picture on this page is a headless render of the engine**, captured by the `shot` CLI
> under a synthesized audio clip — not a screenshot of the application window. They are regenerated
> by `node scripts/docs-shots.mjs`, whose manifest records the preset, stimulus, hop, size and tier
> behind each one ([ADR-0100](adrs/0100-documentation-images-are-committed-headless-renders.md)).

---

## 1. A preset in ten lines

This is a complete, working preset. It is [`docs/examples/minimal.toml`](examples/minimal.toml), and
the picture below it is that exact file rendered:

```toml
system = "parametric_curve"
name = "Ten Lines"

[curve]
family = "maurer_rose"

[params]
n          = "6"
scale      = "0.55 + clamp(bass * 0.35, 0, 0.30)"
brightness = "0.80 + clamp(mid * 0.45, 0, 0.40)"
hue        = "0.55 + time * 0.02"
```

![A pale green Maurer rose: a dense fifteen-pointed star of interlaced straight lines on
black](images/preset-minimal.png)

Four things are worth naming, because they are the whole model:

- **`system` picks what gets drawn.** One of a closed list of names, in section 2. Everything else in the
  file is interpreted against that choice — `n` means something to a `parametric_curve` and nothing
  at all to a `swarm`.
- **`[params]` values are expressions, not numbers.** Each one is re-evaluated every frame against
  the current analysis. `"6"` is a constant that happens to be written as an expression; the other
  three move.
- **`bass`, `mid`, `time` are the vocabulary.** There is more of it — beat, bar, onset, tempo,
  counters, a 64-band spectrum accessor — and [`docs/presets.md`](presets.md) is the reference.
- **`clamp(x, lo, hi)` is doing the real work.** `bass * 0.35` is a *gain*: it decides how much of
  the music reaches the parameter. The clamp is a *limit*: it decides how far the parameter is
  allowed to travel. Getting the gain wrong is the single most common way a preset ends up looking
  dead, and step 3 of the [walkthrough](preset-tuning-walkthrough.md) is about exactly that.

Some systems also take a **structural table** — `[curve]` here, `[generator]` for the two generator
systems, `[particles]` for the attractor, `[spectrum]` for the readout. Those are declarative
configuration read once when the preset loads, **not** expressions: they choose *which figure*, and
the params then animate it.

---

## 2. The systems

One image each, captured under the same stimulus at the same moment in the clip, so they are
comparable. Each is a real shipped preset — the file name is under the picture. All twelve systems
have one, and `parametric_curve` adds four more, one per curve family no shipped preset draws yet,
rendered from teaching presets instead. The **parameter reference** — every parameter of every
system, with its default, the range that reads and what it does — is
[`../presets/README.md`](../presets/README.md).

### `fragment_field`

![Pastel ribbons — mint, lavender and cream — swirling in a wide vortex against near-black, with
fine parallel striations along each ribbon](images/gallery/fragment_field.png)

*`presets/fragment_whorl.toml`*

A fullscreen shader. Every pixel is computed from a domain-warped noise field and coloured through
the shared palette, so there is no geometry and no particle count — the whole frame is the subject.
It is the cheapest system to make *busy* and the hardest to make *sparse*.

**Reach for this when** you want an edge-to-edge look with no figure in it: auroras, marbling,
plasma, oil-on-water.

### `swarm`

![Thousands of small coloured marks — teal, amber, violet — banked into a broad wave crest across a
dark navy field, sparse above it and dense below](images/gallery/swarm.png)

*`presets/swarm_shatter.toml`*

Around ten thousand CPU-simulated particles drifting through a flow field, drawn as instanced
additive marks. Their world is a torus, so nothing ever leaves the frame — the field stays populated
without respawn hitches.

**Reach for this when** the look is a *motion*: murmuration, drift, a shoal turning. Be warned that
this is the system a still photograph serves worst, for exactly that reason.

### `parametric_curve`

![A pink and white star-web on a dark olive field: two nested many-pointed stars traced from
straight interlacing chords](images/gallery/parametric_curve.png)

*`presets/curve_nightbloom.toml`*

One continuous line, sampled every frame from a closed-form `t → (x, y)` curve and drawn as thick
glowing strokes. Because it is resampled per frame rather than cached, audio can sweep the *shape*
itself, not just its colour and scale.

**Reach for this when** you want precise line art whose geometry is the reaction.

The picture above is the **Maurer rose**, one of five **curve families** a `[curve] family` line
picks between. The other four are below, each rendered from a small teaching preset — the file under
the picture is the whole recipe. What `n`, `d` and `phase` mean on each family, and which levers
belong to which, is the [`[curve]` table](presets.md#the-curve-table) in the grammar reference.

![A 3:2 Lissajous figure: one closed looping line crossing itself seven times, shading from orange
through yellow to teal on black](images/curves/lissajous.png)

*`family = "lissajous"` — [`docs/examples/curves/lissajous.toml`](examples/curves/lissajous.toml)*

Two sine waves at right angles, one per axis. Whole frequencies close the figure; a fractional one
leaves it open and drifting.

![A spirograph pentagram: one looping line tracing a five-pointed star with rounded tips, magenta
shading to cyan, on black](images/curves/hypotrochoid.png)

*`family = "hypotrochoid"` — [`docs/examples/curves/hypotrochoid.toml`](examples/curves/hypotrochoid.toml)*

The spirograph: a circle rolling inside a fixed one, traced by a pen. `pen` moves the pen from
rounded loops through sharp cusps to overlapping petals, and a negative `n` rolls the circle outside.

![A superformula starfish: one closed outline with five pointed arms and softly curved waists,
orange shading to violet, on black](images/curves/superformula.png)

*`family = "superformula"` — [`docs/examples/curves/superformula.toml`](examples/curves/superformula.toml)*

Starfish, flowers, polygons and rounded shells from four numbers. `sym` is a whole count of lobes,
which makes it the family that most rewards a `[hold]` on the bar.

![A harmonograph trace: one line looping round a drifting figure of eight and spiralling inward over
several turns, green on the outside shading to pink at the centre](images/curves/harmonograph.png)

*`family = "harmonograph"` — [`docs/examples/curves/harmonograph.toml`](examples/curves/harmonograph.toml)*

Two damped pendulums: a Lissajous figure whose swing dies away along the trace, so it spirals inward
rather than closing. It is the family that best rewards `draw_progress`, which draws the trace on
from the outside in.

### `lsystem`

![A gold interlocking maze on deep navy: a dense space-filling curve of hexagonal turns forming a
rough diamond](images/gallery/lsystem.png)

*`presets/lsystem_vellum.toml`*

A turtle walking a string produced by rewriting an axiom with production rules. The expansion happens
once when the preset loads — one cached segment buffer per depth — so per frame the scene only picks
a visible depth and transforms it.

**Reach for this when** you want structure that *grows*: branching, botanical, or space-filling
figures, and a `draw_progress` that traces them on.

### `star_pattern`

![A bold gold rose window: concentric twelve-pointed star outlines nested inward on
black](images/gallery/star_pattern.png)

*`presets/star_rosewindow.toml`*

A Hankin star rosette built by the contact-angle method — n contact points on a circle, rays leaving
at a continuous contact angle, meeting at the petal tips. The rosette is cached and cheap to
animate, and the contact angle is a continuous parameter, so the interlacing can open and close.

**Reach for this when** you want hard radial geometry: rose windows, Islamic star patterns,
mandalas with real construction behind them.

### `reaction_diffusion`

![Verdigris-green heart-shaped cells arranged in rings around a small starburst, over a warm bronze
field](images/gallery/reaction_diffusion.png)

*`presets/reaction_verdigris.toml`*

A Gray-Scott simulation stepped on a ping-pong texture pair. It is *stateful*: each frame's field
depends on the last one, which is what produces the restructuring, growing, organic look that
stateless scenes cannot.

**Reach for this when** you want pattern that *evolves* rather than animates — spots, stripes,
coral, tissue.

### `attractor`

![A luminous sea-green rosette of fine particle filaments, eight-fold symmetric, dense at the centre
and feathering out to soft lobes on black](images/gallery/attractor.png)

*`presets/attractor_leviathan.toml`*

A very large number of points iterated through a chaotic map or an IFS and deposited into an
accumulating trail buffer that fades. The figure is not drawn so much as *exposed*: it builds up over
seconds, so this is the family that most rewards a late capture.

**Reach for this when** you want fine filamentary structure — smoke, ink, dust, strange-attractor
figures. It has by far the largest parameter surface, and much of it is family-specific.

### `spectrum`

![A radial spectrum readout: a ring of coloured spokes radiating outward, cyan at the top through
violet and pink to gold, over a dark blue-violet vignette](images/gallery/spectrum.png)

*`presets/spectrum_halo.toml`*

A direct readout of the analysis frame's 64-band log-spaced spectrum, drawn as bars, a polyline or a
radial ring of spokes. It is the one system where the audio is literally legible in the picture.

**Reach for this when** you want the music visible as data rather than as an impression. Note that
this system **cannot be verified with `--set`** — that path leaves the band array silent, so the
readout renders as its inert resting comb; use `--signal` or `--audio`.

### `emitter`

![A meteor shower: pale blue-white dashed streaks flying up and to the right in a broad fan across a
dark navy field](images/gallery/emitter.png)

*`presets/emitter_perseids.toml`*

Objects that spawn, follow an analytic ballistic path, age, and are retired. It is the only system
whose population is not fixed — which is precisely what the swarm's wrap-around torus cannot express.

**Reach for this when** the look is made of *discrete events*: fireworks, meteors, sparks, anything
that should be triggered by a beat rather than modulated by a band.

### `shape_field`

![Concentric heart-shaped bands in crimson, rose and peach on black, nested around a black heart at
the centre and drawing out into fine parallel striations at the corners](images/gallery/shape_field.png)

*`presets/shape_pulse.toml`*

It draws one
of five silhouettes — `disc`, `ring`, `polygon`, `star`, `heart` — as a **fullscreen distance
field**, which makes the palette coordinate a *distance*: turn `palette_steps` up and you get
concentric **offset contours of that shape**, not concentric circles. `palette_contour` then draws a
hairline at each band edge.

It is also the one system where the figure itself can be **authored rather than selected**. A
`[path]` table takes inline SVG path data — one closed contour, pasted out of a design tool and
rendered as the same distance field, so `palette_steps` and `palette_contour` band an outline nobody
put in the roster. `presets/shape_maple.toml` and `presets/shape_lion.toml` are that: a leaf and a
lion mask, each 54 commands. A second contour in `morph_to` gives the bindable `morph`, so a figure
can *become* another figure on the beat.

**Reach for this when** the subject is a **figure and its echoes**: nested outlines, a breathing
heart, rings travelling outward from a shape — or one specific emblem the five names do not reach.
The full parameter roster, the `[path]` subset and its arity ceiling, and three worked reactivity
recipes are in [`../presets/README.md`](../presets/README.md).

### `warp_mesh`

![A soft teal and steel-blue field with a black star-shaped aperture at its centre, ringed by a pale
glow that reads as depth receding into the hole](images/gallery/warp_mesh.png)

*`presets/warp_wellhead.toml`*

It covers the frame
with a grid of cells and resamples the **previous** frame through it, giving every grid vertex its
own zoom, rotation, stretch and drift — so the past can spiral in one corner and drift in another,
which no single whole-frame `fb_*` transform can express. It is also the only scene with nothing of
its own to draw: turn the deposit off and the frame goes black in about a second.

**Reach for this when** the subject is the **history of the image** rather than a figure in it:
tunnels, vortices, anything where what you are looking at is where the last frame went. A converted
MilkDrop preset brings its own draw layer and is the exception; see the `[milk]` table in
[`../presets/README.md`](../presets/README.md).

### `shape_collage`

![Flat opaque bars, squares, triangles and discs in charcoal, blue, red, ochre, green and mauve,
tilted and overlapping on a warm off-white ground](images/gallery/shape_collage.png)

*`presets/collage_suprematist.toml`*

The newest system. It is the engine's only **graphic** world rather than a luminous one.
Every other system emits additive light, which has no notion of one object being in front of
another — a black bar over a red one simply adds. This one *paints*: each pixel starts at the
**paper** colour and walks an array of flat elements in array order, compositing each with `over`,
so the array index **is** the depth. There is no glow, no bloom and no soft edge, and that is the
vocabulary rather than an omission.

**Reach for this when** the look is **flat colour and hard edges** on a light ground: suprematist
and constructivist canvases, poster shapes, anything where solid opaque forms overlap. The one
authoring fact is that the palette stays under linear `0.6` — see
[`preset-palettes.md`](preset-palettes.md#flat-colour-on-shape_collage--stay-under-the-knee).

---

## 3. The three surfaces

Whatever system you pick, a preset touches three surfaces. Knowing which is which tells you which
document to open.

**Expressions — what moves.** Every `[params]` value is a pure expression re-evaluated each frame
against the current analysis frame. The vocabulary is the audio (bands, onset, beat, bar, tempo,
novelty, the band array), a clock, arithmetic, comparisons, and a set of functions including
`clamp`, `select`, `sin`, `hash` and `noise`. Expressions are pure — no state, no memory of the last
frame — so anything that needs to *persist* is either a smoothing setting or a term built from a
counter. **The reference is [`docs/presets.md`](presets.md)**, which also documents how a malformed
expression is reported (the preset is rejected with a message; the app keeps the last good set and
never crashes).

**Structure — what is drawn.** The structural tables — `[curve]`, `[generator]`, `[particles]`,
`[spectrum]` — are declarative and are read **once, at load**. They pick the curve family, the
L-system's axiom and rules, the attractor family and tuple, the spectrum's element count and
arrangement. Nothing here can be an expression, because changing it means rebuilding geometry rather
than animating it. **The reference is [`presets/README.md`](../presets/README.md)**, which also
carries every named parameter per system and the engine-wide stages every system accepts —
background, trails, feedback, kaleidoscope, bloom, exposure and the ink remap.

**Colour — how it is tinted.** Palette-coloured systems address a shared lookup table rather than
naming colours per element, so `hue`, `saturation`, `color_span`/`hue_spread` and `color_center`
move a *window* over a gradient. You can name a built-in palette, write your own stops, quantize the
result into hard bands, or cross-fade between two palettes. **The reference is
[`docs/preset-palettes.md`](preset-palettes.md).**

There is a fourth, smaller surface worth knowing early: the **`[smoothing]` table**, which eases a
parameter's response over time and is the difference between an accent that snaps and glides and one
that flickers. It is documented in [`presets/README.md`](../presets/README.md), and step 4 of the
[walkthrough](preset-tuning-walkthrough.md) is a worked example of it.

---

## 4. Iterating

The loop is: point both the app and the capture tool at the same folder, edit, look.

**Live, in the window.** `RLX_PRESET_DIR` overrides the seeded per-user preset directory, and the
app polls it every ~150 ms:

```sh
RLX_PRESET_DIR=./presets cargo run -p standalone --release
```

```powershell
$env:RLX_PRESET_DIR = "./presets"; cargo run -p standalone --release   # PowerShell
```

Save a file and the change is on screen without a restart. **Editing a file in `presets/` without
this is invisible to the app** — seeding is write-if-absent, so a running frontend is reading its own
seeded copy and will not see your edit.

**Headless, as a picture.** `shot` renders one preset with no window. It takes a single file
directly, so an in-progress draft never has to enter the library:

```sh
cargo run -p standalone --example shot --release -- \
  --preset-file docs/examples/minimal.toml \
  --signal dynamic:110 --frame-at 300 --size 1280x720 --out draft.png
```

`--signal dynamic:110` is the stimulus to use. It is the only synthesized kind with real rise and
fall through the real analyzer; every other kind is a steady tone, and the `--set` path holds a
constant value forever and cannot reach the band array at all. `--frame-at <hop>` picks one moment
in that clip and writes it at full size. Every run prints the **band levels the clip actually
produced** — min, mean and max per band — and those numbers, not `--set` magnitudes, are what a gain
should be calibrated against.

To see the whole clip at once instead of one moment, swap `--frame-at 300` for `--strip 8` and you
get a filmstrip. [`docs/capturing.md`](capturing.md) is the reference for all of it.

---

## 5. Knowing it is good

Five gates in `core/tests/` sweep every preset in `presets/`, and **only one of them plays audio**.
That distinction is the most useful thing to know about them:

- **`reactivity`** drives real synthesized clips through the real FFT, band split and onset
  detector. It is the only gate that would notice a preset ignoring the music.
- **`sanity`**, **`animation`**, **`distinctness`** and **`golden`** construct an analysis frame
  directly, on purpose — their questions are about the *frame*, not the audio path. Each of them
  would pass a preset with every band binding deleted.

So a green suite means "this renders, moves, and is not a duplicate". It does not mean "this reacts
to music", and it certainly does not mean "this looks good". [`docs/capturing.md`](capturing.md) has
the full table of what each gate can and cannot see.

Two instruments are worth running by hand while you tune:

- **`shot --report`** prints per-preset reactivity, animation, coverage and transient columns, and
  accepts `--preset-file`, so you can measure a draft that has not shipped. The
  [walkthrough](preset-tuning-walkthrough.md) uses it
  at every step.
- **the band-level table** that every `--signal`/`--audio` capture prints. If your gain was chosen
  against a `--set` magnitude it will be wrong by roughly an order of magnitude, and this table is
  how you find out.

One thing no instrument here can judge: whether the picture is *good*. That stays a human call.

---

## 6. Next

[**`docs/preset-tuning-walkthrough.md`**](preset-tuning-walkthrough.md) takes one preset from
constants to a finished look over five numbered steps, and shows the picture **and the `--report`
row** that changed at each one — including the step where the numbers moved the wrong way.
