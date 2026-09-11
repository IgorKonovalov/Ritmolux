# Proposed roster

One row per candidate. Appended by each authoring pass; nothing else writes here.

`what to look at` is the row that earns its keep — it names the thing the author was
going for, so a verdict is a comparison rather than a general impression.

| preset | system / family | pass | what to look at |
|--------|-----------------|------|-----------------|

## Owner verdicts (2026-09-11)

The owner looked at all fifty-one. The twenty-two binned files are deleted and their rows
are gone from the table above; the notes below still mention some of them, because they are
the record of what each pass tried.

**Twenty-three have since shipped** (`eb5eab4`): the eleven kept as they were, the tuned
Echo Plate, Parabolic Dust, Seahorse, Lorenz Knot, Phosphor and Strata Heart, the
second-look Lace Grid, Multibrot, Cogwheel and Gyre, Ice Crystal, and Radial Bloom in
place of `spectrum_halo`. Their rows are gone from the table above; `git show eb5eab4^`
has them.

**Blueprint, Turnabout and Ink Pendulum were held back by the behavioral suite, not by
the owner.** Their reveals sat at an 8-12 % stub in every probe frame and failed sanity,
animation or reactivity. On 2026-09-11 all three were re-cut so a probe frame shows a
whole figure. Blueprint and Ink Pendulum now hold the finished drawing at the end of
each bar, Turnabout stands whole until the first onset, and Turnabout's stroke is
heavier. With the files temporarily embedded, every gate they had failed passes, and
distinctness passes too. Each header carries its numbers. They shipped the same day, each
with a gallery card, which leaves the table above empty until the next authoring pass.

- **Keep, as they are:** Julia Circuit, Pearl String, Ring Orbit, Searchlight, Stained
  Glass, Standing Wave, Two-Band Julia, Lacework, Prismscope, Etching Plate, Murmuration.
- **Tune:** Echo Plate (calmer), Parabolic Dust (reframe right, calmer zoom), Seahorse and
  Lorenz Knot (frame rate: 15 and 20 fps), Blueprint (draw different figures), Phosphor
  (the trace must close, it has a gap), Strata Heart (vertical). Open-ended: Lace Grid,
  Diatom Plate, Turnabout, Ice Crystal ("needs something").
- **Radial Bloom:** keep, and it REPLACES the shipped `spectrum_halo` - a curation act for
  `dev`/`architect`, not a content edit.
- **Meh, to tune:** Multibrot, Cogwheel, Gyre, Ink Pendulum - options to be rendered.
- **Strata Heart:** "vertical" meant dead upright - the sway is removed.
- **Second look (options rendered, owner picked):** Lace Grid A (shaded silk), Multibrot B
  (julia crown), Turnabout B (pen races in), Cogwheel C (saw blade), Gyre B (arms change per
  bar), Ink Pendulum C (drawn live) - each now IS the file of that name. Ice Crystal: keep the
  current cut as it is. Diatom Plate: binned.
- **Binned:** Engraving, Mode Sweep, Round Plate, Sand Plate, Ember Drift, Kandinsky Beat,
  Pinwheel, Rose Window (curve), Spindrift, Tide Chart, Urchin, Ashfall, Confetti Drop,
  Aurora Veil, Kaleido Dome, Tidepool, Crescent Gate, Horizon Line, Lantern Court, Orrery,
  Ember Storm, Plasma Lamp.

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

## Notes on the analytic-field pass (2026-09-11)

**Sixteen presets on `analytic_field`, the system Plan 0163 landed:** seven on `chladni`
and nine on `escape_time`. The Implementation log confirms both families, both `map`s and
all four trap shapes, and all of them are covered. Every file was rendered at
`--tier rich --signal dynamic:110 --frame-at 300 --size 640x360` after its last edit and
looked at.

**Filenames use `analytic_`**, the way `fragment_field` ships as `fragment_*`. No shipped
preset draws this system yet, so there is no precedent to follow. If the owner wants
`field_` instead, rename them all together.

**How the structural levers are spread:**

- **Chladni modes:** low (Echo Plate 1..3 × 4..7, Sand Plate 1..4 × 5..9), middle
  (Engraving 3..6 × 7..10, Round Plate 2..4 × 5..8), high (Standing Wave 7..10 × 11..15),
  the top (Lace Grid 12..13 × 14..16), and the whole range unheld (Mode Sweep 2..16).
  `plate_mix` runs from 0 through 0.25 to 1. Holds use `beat`, `bar` and none.
- **Escape time:** Julia and Mandelbrot. `power` runs 2, 3, and whole 3..7 on the bar.
  Iterations run from 20 (Stained Glass, deliberately low) to 40..400 on the bass
  (Seahorse). Zoom runs from 0.72 to ~330. `c` is placed inside the cardioid, on its
  edge, just outside it, in Seahorse Valley and past the cusp. Each trap shape is used
  once.

**Binned: `analytic_tornjulia`** (a fractional `power` wandering from 2.3 to 3.6, meant
to show the tear as a look). Two cuts at two different `c` values both came back with
the branch cut as a **dead-straight horizontal seam with a rectangular step**, cutting
through a black blob. It reads as a render defect, not a fracture. That answers the
risk Plan 0163 raised about the fractional seam. See gap 3.

**Re-cut after the first render:**

- Julia Circuit first toured the `|c| = 0.7885` circle and came back as dim dust. It
  moved to the cardioid edge, where the bass pulled `c` inside and the loud frame was a
  featureless black blob. The direction was then flipped: loud pushes `c` outside.
- Stained Glass took four cuts. The first three were a rainbow poster and then cream
  crackle glaze, because with 48 iterations every interior orbit grazes the cross (gap
  1). It needed `c` inside the cardioid, 20 iterations, and `color_span` at its maximum.
- Seahorse started 40x in, which was too shallow. Its first iteration range (140..380)
  showed no quiet/loud difference.
- Parabolic Dust was, at first, exactly one stop on Julia Circuit's tour in a different
  palette. It was re-framed as a close-up so that it is a separate candidate.
- Round Plate's first fold made chevron spokes instead of rings.
- Echo Plate's first trail was too short to read as echoes at all.

**`--report` (tier floor, `family=analytic_field`):** every preset reacts to at least
one band. There are no near-duplicates and no clamp over 90 % occupancy. It found one
real problem: **`anim` read 0.000 on eight presets.** The field is stateless, so with
no audio and no `time` term the picture is frozen. The shipped `animation` gate would
reject all eight. Five of them (Sand Plate, Standing Wave, Engraving, Mode Sweep,
Multibrot) now carry a slow pan, zoom or wash drift and read 0.004-0.009. Three still
read 0.000:

- **Two-Band Julia**, deliberately: every change on it is the music.
- **Julia Circuit** and **Ring Orbit**: they do drift on `time`, but too slowly for the
  probe's window.

A keeper among those three needs a faster drift before `dev` embeds it. Multibrot's row
(drive 0.058) is the weakest. Its main change is the bar re-roll, which the probe holds
still; an 8-frame strip confirms the re-roll works.

**Horizon:** only Echo Plate accumulates, since it is the only one with `trails`. I ran
five simulated minutes at 96x96 against a static `star_pattern` control, which read
`delta 0.0000 / monotone 0.00`. Echo Plate settles in the first interval and then
breathes (monotone 0.50-0.56). The verdict is in its header.

**Tier:** everything except Seahorse asks for at most 64 iterations (the Floor cap), so
it draws the same picture on both tiers. Seahorse asks for up to 400, and on Floor it is
clamped with an announcement.

**The best four, if the morning is short:** Standing Wave, Seahorse, Pearl String and
Searchlight. Two-Band Julia is the one to watch with real music, because it is the
plan's open question.

## Engine gaps this pass hit — analytic field

Routed feedback for `architect`, ordered by how much authoring time each one cost.

### 1. A trapped field cannot send its exterior to one colour

With a `trap`, a pixel's palette coordinate is the orbit's nearest approach. That value
is unbounded, it is largest out in the exterior, and **the LUT repeats past 1**. So
there is no palette that is bright near the trap and dark everywhere far from it: the
far field wraps back around through every stop. It shows in three presets:

- the striped fringes in Stained Glass's corners;
- the concentric rings round Pearl String;
- the chevrons behind Searchlight.

On Pearl String and Searchlight they happen to read as a design. On Stained Glass they
are the flaw. It also cost the most renders of anything in the pass. Stained Glass had
to be framed so that the exterior barely shows, and that constrains `c` and `zoom` for
a reason that has nothing to do with the look. On top of that, **an escaped pixel's
light is fixed at 1**, and `interior` has no exterior twin. So the exterior can only be
darkened through the palette, which is the wrapping coordinate itself.

**What would close it:** a clamp-addressed palette mode (for example a per-palette
`repeat = false`), or an `exterior` light parameter beside `interior`. Either one alone
would have saved Stained Glass two cuts.

### 2. The analytic field has no rotation

The scene has `zoom`, `pan_x` and `pan_y`, and nothing that turns it. A Julia or
Multibrot crown that slowly turns is the obvious ambient motion, and the one thing a
stateless field needs most (see the `anim` note above). The only rotation available is
the kaleidoscope's `kaleido_angle`, which brings a fold with it. Every drift in this
pass is therefore a pan or a zoom breath. **What would close it:** a `rotate` in turns,
like `trap_rotate` but applied to the field's coordinates.

### 3. The fractional-power seam reads as a bug (the plan's risk, answered)

Plan 0163's risks section asked to "check the seam before shipping a fractional
default". Checked: the `atan2` cut along the negative real axis draws a
**straight horizontal line with a rectangular step** where the two sides of the set
fail to meet. At `power` 2.3-3.6, with two different `c` values, it never read as a
fracture. It read as a tile that failed to render. Torn Julia was binned because of it.
`power` is safe as a whole number held on an edge, as Multibrot uses it. A fractional
`power` is not yet a look. The reference's line *"a fractional power tears along the
negative real axis"* is accurate, but it reads as an invitation, and it should read as a
warning.

### 4. The Chladni plate is square, and real cymatics is round

The `chladni` family is the square-plate formula. The imagery people know (a round
plate, water in a dish) is Bessel modes on a disc. Round Plate fakes the disc with a
six-fold kaleidoscope. It mostly works, but it leaves four stub artifacts at the rim,
and it needed three cuts to stop turning into chevron spokes. **What would close it:** a
`[field] plate = "square" | "circle"` choice, or a disc family placed beside the three
ADR-0180 already names. `mode_n` would become the angular order and `mode_m` the radial
order.

### 5. Smaller

- **`mode_n == mode_m` blanks the plate.** Every Chladni preset here keeps its two
  ranges disjoint by hand. Mode Sweep deliberately lets them cross, and gets a blank
  frame for it. The reference says so, but nothing warns at load when two bound ranges
  overlap.
- **The reference gives `zoom` as 0.25-4, but the scene does not clamp it.** Seahorse
  runs at ~150-330x and f32 holds cleanly there. For the Mandelbrot map the stated range
  undersells the system by two orders of magnitude, and an author reading the table
  would never try a deep zoom. That is a doc sentence, not an engine change.
- **The stateless field and the `animation` gate** (the eight `anim 0.000` rows above).
  This is not a defect, but every future `analytic_field` preset will hit it. A
  sentence in the reference ("the field has no state: without a `time` term it is
  frozen in silence") would save the next author a report run.
- **Short max-decay trails over a field read as ribbing.** Echo Plate's first cut, at
  `trails` 0.86 with a slow `fb_zoom`, drew each nodal line as a ribbed band of
  near-coincident copies instead of an echo. That is the trail behaving as documented,
  but together with a thin analytic line it looks like a sampling fault.

## Engine gaps the second-look pass hit (2026-09-11)

Routed feedback for `architect`, from rendering option sheets for the eight presets the owner
sent back for tuning.

### 1. `visible_depth` declares `0 - 1` and is read as a generation count

`lsystem.rs` declares `ParamSpec { name: "visible_depth", range: Some([0.0, 1.0]) }`, so the
generated `presets/README.md` row says `0 - 1`. The scene reads it as
`self.visible_depth.max(1.0) as usize`, a whole generation index, and the shipped L-systems bind
it 3-7. `lsystem_icecrystal` was authored against the reference at `0.72 - 1.0` and sat at
generation 1 forever, which is why the owner found it bare. The declaration is wrong, not the
scene, and every future author reading the generated table will make the same binding. A `dev`
fix to the `ParamSpec` range regenerates the row.

### 2. `shot --all` with `--signal` writes no contact sheet

With a directory `--out`, `--all --signal dynamic:110 --frame-at 300` fails with *image format
could not be determined*, and with a file `--out` it writes one frame (gap 5 of the first pass).
The command in this folder's own README is that combination. All four option passes fell back to
`--set` stills for the sheets and `--signal` strips per variant, which is two commands where the
README promises one.
