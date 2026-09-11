# Proposed roster

One row per candidate. Appended by each authoring pass; nothing else writes here.

`what to look at` is the row that earns its keep — it names the thing the author was
going for, so a verdict is a comparison rather than a general impression.

| preset | system / family | pass | what to look at |
|--------|-----------------|------|-----------------|
| `fragment_auroraveil` | fragment_field | 2026-09-11 overnight | Built palette-first: green sheets with violet lit edges over indigo. Do the folds read as depth or as stripes? Ambient — it has to hold up at rest, not only loud. |
| `fragment_etchingplate` | fragment_field | 2026-09-11 overnight | Cream paper, indigo ink, seven hard bands with a hairline at each edge. Judge the paper: any grey cast in the empty regions and the print reading is gone. |
| `fragment_kaleidodome` | fragment_field | 2026-09-11 overnight | The only preset here whose mechanism is in the COMPOSITE — an eight-wedge kaleidoscope with a radius-growing spiral. Judge the seams: they must be hard lines. |
| `swarm_murmuration` | swarm | 2026-09-11 overnight | Cold flock, long trail, bass on `spin`. Does it read as one body with internal structure, or as noise? Judge a few beats in — `trails` restarts empty at a switch. |
| `swarm_emberstorm` | swarm | 2026-09-11 overnight | The hard counterpart: four-point sparks thrown outward on the hit by `burst` with a snap-attack/slow-release pair. Does the hit read as a THROW or as a zoom? |
| `curve_spindrift` | parametric_curve | 2026-09-11 overnight | `n` re-rolls twice a second off `hash(floor(time*2))`, so the figure is replaced rather than modulated. Do the re-rolls read as musical or as a glitch? |
| `lsystem_icecrystal` | lsystem | 2026-09-11 overnight | A bush grammar folded six ways. `visible_depth` on mid adds and removes the fine tips. The two long verticals through the centre are load-bearing and also the first thing that will look like a mistake. |
| `star_lanterncourt` | star_pattern | 2026-09-11 overnight | Twelve-fold Hankin interlace with four rings and a scalloped rim inside it. Does it read as ONE figure? Watch the rim deepen — that is `ring_scale`, not a separate param. |
| `star_orrery` | star_pattern | 2026-09-11 overnight | `tiling = "none"` — rings only, no interlace. Is the counter-rotation legible as two families going opposite ways, or just as shimmer? |
| `reaction_tidepool` | reaction_diffusion | 2026-09-11 overnight | Pale coral maze on dark water; the music drives `flow` (growth speed), not brightness. Judge the contour bands: a flat wash means the regime left the live band. |
| `attractor_emberdrift` | attractor | 2026-09-11 overnight | Clifford tuple 4 inside its own drifting smoke — the trail has a DIRECTION, given by `fb_dy`/`fb_zoom`. Watch a `reseed`: shake-and-recover, or glitch? |
| `attractor_lorenzknot` | attractor | 2026-09-11 overnight | Lorenz entry 1, the rho≈100 torus knot, shown as a solid via `perspective`/`depth_fade`/`depth_hue`. Can you see one strand pass BEHIND another? Judge it late — ~8 s emergence. |
| `spectrum_radialbloom` | spectrum | 2026-09-11 overnight | 48 spokes on a ring; `thickness` and `hue` both vary per element off `bin(index)`. Strongest reactivity in the batch (drive 0.173). Does it read as one object? |
| `spectrum_horizonline` | spectrum | 2026-09-11 overnight | The same data as scenery: a 40-point polyline ridge on a dusk horizon band. Does the ridge have relief across its whole width, or is the right half flat? |
| `emitter_confettidrop` | emitter | 2026-09-11 overnight | `spawn_rate` 55→900 on onset: a hit is a POPULATION change, not a flash. Does it read as a throw? And is the thin drizzle at rest still worth looking at? |
| `emitter_ashfall` | emitter | 2026-09-11 overnight | The ambient one, and the emitter upside down (`source_y` above the frame, `launch_angle` half a turn). Deliberately has no EVENT in it — that is the risk it takes. Weakest numbers in the batch: see the note below. |
| `shape_strataheart` | shape_field | 2026-09-11 overnight | Pure palette: `palette_steps` (bound, 6→11) is the picture and `coord_mode = 1` makes the bands scaled copies. Do the band edges stay HARD? A halo means bloom is fighting them. |
| `shape_crescentgate` | shape_field | 2026-09-11 overnight | An authored `[path]` crescent morphing to a disc, in `coord_mode = 0` so contours are offset curves rather than scaled copies. Judge the MIDDLE of the morph — nobody authored that shape. Best judged beside `shape_strataheart`. |
| `warp_plasmalamp` | warp_mesh | 2026-09-11 overnight | Cells drifting inward, edged by `solarize` riding the mids — the structure is a fold, not geometry. Are the cells soft at rest and rimmed when loud? |
| `collage_kandinskybeat` | shape_collage | 2026-09-11 overnight | Flat opaque shapes on off-white paper; `recompose` is edge-triggered on a strong onset with a 0.22 s blend. Palette is eight PLATEAUS under linear 0.6, so fills are unshaded and edges hard — check both. |
| `curve_phosphor` | parametric_curve / lissajous | 2026-09-11 curve families | Green scope trace; `phase` sweep turns it in depth, `n` re-tunes 3:4 ⇄ 5:4 on the bar THROUGH the open ratios. Does the re-tune read as deliberate, or as the trace coming apart? |
| `curve_lacework` | parametric_curve / lissajous | 2026-09-11 curve families | Cream thread, three reflected copies knit into one round body; `n:d` re-rolls per beat (3:4..5:6). Reads as a ball of thread, not the doily it was aimed at — judge it as that. |
| `curve_prismscope` | parametric_curve / lissajous | 2026-09-11 curve families | A Lissajous as kaleidoscope SOURCE: 6-wedge Droste tunnel, `d` 4 ⇄ 6 on the beat. The family is not legible through the fold. Keep for the tunnel or bin for hiding its figure. |
| `curve_cogwheel` | parametric_curve / hypotrochoid (epi, `n = -3.5`) | 2026-09-11 curve families | Brass seven-lobed epicycloid; bass drives `pen` 0.55 → 1.55 so scallops become cusps become loops. Watch a kick: seven knots should grow on the rim. |
| `curve_blueprint` | parametric_curve / hypotrochoid | 2026-09-11 curve families | White-on-blueprint drawing machine: `draw_progress` on `bar_phase`, re-geared each bar from four closing ratios. `--report` cannot see it (see notes). Judge the reveal's rhythm, not its sync. |
| `curve_rosewindow` | parametric_curve / hypotrochoid | 2026-09-11 curve families | 26-cusp tracery, opaque paint, 10 stepped panes along the trace; mids swell `pen`. Do the panes read as flat stained glass? Dim-ish by design — jewel palette under `stroke_blend = 1`. |
| `curve_turnabout` | parametric_curve / hypotrochoid (sign flip) | 2026-09-11 curve families | `n` flips +3 ⇄ −3 on a beat, un-eased: deltoid ⇄ trefoil, the same figure inside out, with the opposite sign ghosted UNDER. Does the ghost make it one object turning over? |
| `curve_urchin` | parametric_curve / superformula | 2026-09-11 curve families | Best reactivity-to-structure in the pass: bass drops `sharpness` and the star grows spines; `sym` walks 7/9/11 on the bar; rounded body layered under. Loud must read as spines, not tangle. |
| `curve_diatomplate` | parametric_curve / superformula | 2026-09-11 curve families | Haeckel plate: 18-point shell over 9-lobe core, ink on cream. Mids skew `d` so points lean. Lines read PENCIL-grey, not ink-black (gap 5). Is the skew visible between loud and quiet? |
| `curve_pinwheel` | parametric_curve / superformula | 2026-09-11 curve families | `d` wanders 1.4..3.4 so a flower grows hooked claws; bass throws `spin` and `trails` 0.8 smear the hooks. Must read as a spinning solid, not fog. Horizon: breathes, no pile-up. |
| `curve_inkpendulum` | parametric_curve / harmonograph | 2026-09-11 curve families | Victorian harmonograph, 2:3.012, ink on paper. `decay` INVERTED on loudness: quiet collapses to a tight knot, loud opens to the sheet. Watch the knot at rest — it is the quiet state's picture. |
| `curve_gyre` | parametric_curve / harmonograph | 2026-09-11 curve families | Near-unison damped circle, three-fold rotational mirror = three-armed galaxy; bass lowers `decay` and the arms unwind. Strongest quiet-vs-loud contrast of the four harmonographs. |
| `curve_tidechart` | parametric_curve / harmonograph | 2026-09-11 curve families | 4:3.006 opaque ribbon, palette stepped along the trace so the damping lands as nested colour shells — a bathymetric chart. Do the bands read as depth or as candy stripe? |

## Notes on this pass

**Twenty presets, all twelve systems, every one rendered at `--tier rich --signal
dynamic:110 --frame-at 300 --size 640x360` and looked at.** Several were re-cut after
the first render: `attractor_emberdrift` came back as a solid orange blob (the additive
ceiling, exactly as advertised — `brightness` 0.92 → 0.22 and `fade` 0.955 → 0.90 fixed
it), `swarm_murmuration` took four passes to stop being a dark speckle field, and one
world was abandoned and rewritten from its mechanism up (see gap 3).

**`--report` was run over the folder and it found three near-dead presets a still would
not have.** `emitter_ashfall`, `emitter_confettidrop` and `lsystem_icecrystal` all read
essentially zero reactivity or `anim 0.000` on the first pass. Ice Crystal was genuinely
frozen in silence — every binding it had was gated on a band — and now carries a slow
`rotation`. Both emitters gained a second reactive cue (a `hue_center` travel) because a
population change alone is diluted to nothing in a whole-frame differential.

**`emitter_ashfall` is the one to judge sceptically.** It reads `cover 0.056`,
`level 0.0003`, `drive 0.005` — the weakest row in the batch by a distance. Over its own
lit footprint the same differential is `bass 0.048`, a factor of ten higher, which is the
sparse-figure dilution ADR-0091 describes rather than a dead preset; the 640x360 render is
a full frame of falling ash. But it IS a dark, sparse, event-free world and it would not
survive the shipped `animation` gate comfortably. Keep it only if the ambient slot is
worth that.

**Not run: `--horizon`.** Four of these accumulate (`attractor_emberdrift`,
`swarm_murmuration`, `swarm_emberstorm`, `warp_plasmalamp` — trails, `fade`, and a
feedback field respectively) and so meet ADR-0099's trigger for a horizon spot-check. It
was skipped for time; a ten-minute horizon on the two swarms and the plasma lamp is the
first thing to run on anything kept.

## Engine gaps this pass hit

Routed feedback for `architect`. Ordered by how much authoring time each one cost.

### 1. `warp_mesh` cannot deposit a crisp figure, and that killed a preset

**The deposit figure is intrinsically soft and nothing in the parameter surface sharpens
it.** Probed directly — `decay = 0`, `zoom = 1`, `warp = 0`, so the frame IS one deposit —
`deposit_arms = 6` at `deposit_radius = 0.32`, `deposit_width = 0.06` renders as six
Gaussian blobs with no edge anywhere. Narrowing to `deposit_width = 0.018` gives a blurred
ring, not a wire. Every downstream lever then compounds the blur rather than resolving it:
the per-vertex resample is bilinear, `warp` adds a ripple, and `decay` only controls how
long the blur persists.

**Cost: one whole preset.** `warp_vortexgate` — a six-armed rotor falling into its own
tunnel, meant to be the hardest-hitting look in the batch — went through five renders
before I accepted that a legible rotor is not reachable from the preset surface, deleted
it, and rewrote the world as `warp_plasmalamp`, whose entire structural mechanism is
`solarize` folding a smooth gradient into a contour. That is a genuinely good look, and it
is also the *only* edge-producing lever the system has.

**What would close it:** a `deposit_sharpness` (or a finer grid for the deposit pass
alone). The system currently has one aesthetic — soft — and a `warp_mesh` preset that
wants a hard edge has exactly one trick available to it.

### 2. `bg_hue` is a palette coordinate, so a wide-gamut palette has no dark ground

The background stage reads the preset's own LUT, so `bg_hue` can only name a colour the
scene is already made of. `emitter_confettidrop` carries a six-stop rainbow (that is the
confetti), and there is **no coordinate on that palette that is dark** — `bg_hue = "0.68"`
put a mid-green ground behind the whole piece on the first render. The only escape is to
drive `bg_bright` toward zero, which simultaneously removes the ramp, the shade pair, the
vignette gradient and the band — the entire background stage — rather than just changing
its colour.

**What would close it:** either a `bg_*` colour independent of the palette, or a third
`[palette_bg]` table beside `[palette]`/`[palette_b]`. Cost here was two renders and a
comment in the file warning the next author.

### 3. The line-scene mirror has no axis reflection

`mirror_order` / `mirror_reflect` repeat geometry **around the centre**. There is no
horizontal- or vertical-axis mirror, so a `spectrum` polyline at `mirror_order = 2,
mirror_reflect = 1` produces a Rorschach — the ridge answered by a 180°-rotated copy —
and never a reflection in water, which is the idiom a landscape readout actually wants.
`spectrum_horizonline` ships with `mirror_order = 1` because the alternative was wrong,
not because one copy was the design.

**What would close it:** an axis mirror as a distinct control (a `mirror_axis` selector,
or an order-2 special case that reflects rather than rotates).

### 4. `--report`'s misleading table is printed first

Not a defect — ADR-0091 already names the dilution and prints the footprint-normalised
table to fix it. The problem is **ordering**. The first table an author reads shows
`Ashfall  bass 0.005`; the table thirty lines below shows `bass 0.048` for the same
binding. On a sparse-mark world the first number is wrong by an order of magnitude and it
is the one that gets acted on. Printing the footprint figure *beside* the whole-frame one,
or flagging rows where the two disagree by more than ~3x, would stop an author retuning a
preset that was never the problem.

### 5. `--all` with a file `--out` silently writes one frame

`shot --presets presets/proposed --all --out target/sheet.png` printed
`wrote target/sheet.png (400x225, preset Ember Drift, ...)` — a single preset's frame, no
contact sheet, no warning that `--all` wanted a directory. The command in
`presets/proposed/README.md` passes a directory and works; a file path produces a
plausible-looking wrong result instead of the error every other misuse in that CLI gets.

### 6. Second sighting: `bin(x)` addresses by position, not frequency

Already tracked ([ADR-0063](../../docs/adrs/0063-address-the-spectrum-by-frequency.md)),
recorded here as a second sighting rather than as news. Both spectrum presets wanted to
place a per-element colour and thickness **by frequency** — "warm above 2 kHz" — and had
to be written against `index` and normalized position instead, with the intended Hz in a
comment. A `bin_hz` would have made both files shorter and their intent legible.

### 7. Authoring trap, not an engine gap: `stroke > 0` and banding are mutually exclusive

On `shape_field`, `palette_steps` bands the figure coordinate — but with `stroke` above 0
only the outline is drawn, so the bands have a hairline to live in and the contour look
disappears entirely. `shape_crescentgate` was authored with both and rendered as a bare
outline on black. Neither parameter is inert and nothing warns; they simply do not
compose. This is a sentence in `presets/README.md`, not an engine change.

## Notes on the curve-families pass (2026-09-11)

**Thirteen presets, all `parametric_curve`, all on the four families Plan 0162 landed**
(its Implementation log confirms `lissajous`, `hypotrochoid`, `superformula`,
`harmonograph`; the epicycloid is the hypotrochoid at negative `n`, not a fifth name).
Three Lissajous, four hypotrochoid, three superformula, three harmonograph. Every one
was rendered at `--tier rich --signal dynamic:110 --frame-at 300 --size 640x360`
after its last edit and looked at, and all but Lacework, Cogwheel and Blueprint also got a
six-frame `--strip` to see motion. Every one binds its family's structural
lever to audio or to a held re-roll: `pen`, `sharpness`, `sym`, `lobe`, `d`-as-skew,
`decay`, the sign of `n`, or the ratio itself. None leaves it at the default.

**Binned: `curve_origami`** (superformula, `samples` 18 → 60 on bass, meant to fold the
star into flat facets). The arc fit smooths a coarse sample instead of faceting it, so the
star came back lopsided and kinked, like a broken render. See gap 2.

**Re-cut after the first render:** all three ink-on-paper presets (Diatom Plate, Ink
Pendulum, Blueprint) came back as near-invisible hairlines, and it took `brightness` at
its maximum plus `ink_gamma` 0.2 to get a legible line (gap 5). Gyre's first cut used a
reflected mirror, which clipped every ring into lens-shaped arcs, so it is now a plain
three-fold rotation. Prismscope's first cut rested on the 5:5 ratio, which is a bare
diagonal, and the fold threw it out of frame: a black screen on some beats. Pinwheel's
lobes were too round to hook, so the trail smeared them into a ring. Lacework went from 6:7
to 3:4 because the dense ratios read as a scribble.

**`--report` (tier floor) found one real bug and two blind spots.** The bug: Blueprint's
first cut had `draw_progress` riding `bar_phase` from 0, so every probe frame drew
NOTHING (all columns 0.000, cover 0.000). It now floors at 0.12. The blind spots are in
gap 6. Weakest whole-frame rows: Turnabout (drive 0.013), Pinwheel (0.012) and Cogwheel
(0.014). All three spend their energy on small geometry (cusp loops, a spin that the
probe's still frames cannot see), and Cogwheel's footprint figure is not much higher.
Judge those three in motion, not from the number.

**Horizon run** on the three with `trails` (Phosphor, Pinwheel, Gyre): five simulated
minutes each at 96x96 against a static `star_pattern` control that read `delta 0.0000,
monotone 0.00`. All three read monotone 0.40-0.60 on every statistic, which is breathing,
not accumulation. The verdicts are in each header.

**The best four, if the morning is short:** Urchin, Cogwheel, Gyre, Ink Pendulum. Each
has a structural change on the music that reads in a single glance.

## Engine gaps this pass hit — curve-families pass

Routed feedback for `architect`, ordered by how much authoring time each one cost.

### 1. A family's `phase` is a position, and nothing integrates it

The classic oscilloscope Lissajous turns in depth at a rate, and the look wants that rate
on the music: faster on a busy passage. `spin` is a rate the scene integrates, but it turns
the figure in the plane. `phase`, the offset between the axes and the thing that makes a
Lissajous or harmonograph turn in depth, is a position, and the grammar has no accumulator
to integrate an audio-driven speed into it. So Phosphor and Prismscope bind
`time * k + mid * c`, and the mid term is only a jitter on top of a constant sweep. That
comment was corrected in both headers after drafting. **What would close it:** a
`phase_rate` beside `phase` (integrated like `spin`), or a general "integrate this binding"
marker.

### 2. `samples` cannot facet the arc-fitted families, and the reference says it can

`presets/README.md` says of `samples`: *"fewer reads as a polygon"*. That holds for the rose's
chord web. It does not hold for the four new families, which are fitted with arcs. At 18
samples the superformula came back as a smooth, lopsided, kinked star, not a polygon. That
cost one preset (Origami, binned). **What would close it:** a per-preset chord mode, or the
sentence in the reference qualified per family, which the Phase 5 per-family range cell
could carry.

### 3. Doc drift: the `[curve]` structural table still describes only the rose

`presets/README.md`'s `### [curve] — for parametric_curve` table lists `maurer_rose` as the
only value and calls `family` *Required*. `docs/presets.md` (Plan 0162 Phase 6) lists five
families and says an absent table draws the rose. The code accepted all four new names, so
the table is stale, and Phase 6's sweep missed it. Separately, `bg_angle`'s generated row
says *"fraction of a full turn"* and the hand-written backdrop table in the same file says
*radians*. I did not bind `bg_angle` in this pass, so I have not checked which is true.

### 4. A reflected line-scene mirror clips a figure into its wedge

At `mirror_order = 2, mirror_reflect = 1` a harmonograph came back as lens-shaped arcs, with
every ring cut at the wedge edge. That is presumably the mirror working as designed: it
takes one wedge and reflects it. But a centred, round figure is exactly what an author
reaches for a mirror with, and the result reads as a bug. This is a second sighting beside
the previous pass's gap 3 (no axis reflection). A sentence in the reference would have
saved a render.

### 5. The ink pass cannot make a hairline dark

On paper, a line scene's stroke only reaches the ink colour where the lit pixel is
bright, and a thin anti-aliased stroke never gets there. Getting a legible line needed
`brightness` at its maximum (`2`), `ink_gamma` 0.2 and `thickness` 2.2-2.6. The result
still reads as pencil grey rather than ink black, and at 2.6 px it is no longer a hairline.
I did not try the `multiply`-layer dark-on-light route, which the reference documents for
this. That is the next author's first move on Diatom Plate and Ink Pendulum, and it may
close this.

### 6. `--report` cannot see a `bar_phase`-driven reveal, or a figure on paper

Blueprint's whole motion is `draw_progress` on `bar_phase`, and the probe holds that
still, so the preset reads `drive 0.003, cover 0.006`. That is the weakest row in the
folder, for the preset with the most motion in the pass. The ink presets read `cover`
0.06-0.10 because their background is light paper. Neither is wrong arithmetic, and both
invite an author to retune the wrong thing. This is the same class as the previous
pass's gap 4 (the order the report prints its tables in).

### 7. Smaller: the harmonograph's length is a constant, and a pick needs nested selects

- `HARMONOGRAPH_TURNS = 4` fixes the trace length, so Gyre shows only four rings at low
  `decay`, and a deeper tunnel is not reachable. A `turns` lever would have been the
  second structural knob on the family.
- Re-rolling a ratio from a table of four needs two three-deep `select` chains over the
  same `hash(bar_index)`, one for `n` and one for its matching `d` (Blueprint), and the
  pairing is kept consistent by hand. A `pick(i, a, b, c, …)` function would make the
  intent one line per parameter.
