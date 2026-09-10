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
