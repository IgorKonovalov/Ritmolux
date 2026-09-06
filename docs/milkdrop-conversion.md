# MilkDrop conversion

`milkconv` turns a MilkDrop `.milk` file into a Ritmolux preset ahead of time
([ADR-0113](adrs/0113-milkdrop-presets-are-translated-ahead-of-time-onto-a-warp-mesh-idiom.md)).
This page is how you look at what it produced and decide whether the conversion is good.

It is a **developer tool and never ships** — it sits outside the workspace's
`default-members`, exactly as `rlx-core-cabi` does, so a bare `cargo build` does
not compile it:

```bash
cargo run -p milkconv -- "path/to/Some Preset.milk" --out converted.toml
```

The output is an ordinary preset, so everything on this page already applies to
it — `shot --preset-file converted.toml` renders it like any other:

```bash
cargo run -p standalone --example shot -- \
  --preset-file converted.toml --frames 300 --size 640x400 \
  --set bass=0.6,mid=0.5,treb=0.45 --out converted.png
```

### Read the warnings; they are the interesting output

The converter prints every name it could not carry across, to **stderr**, and
repeats them in the bundle's header comment. That is the plan's
no-silent-zero rule (Phase 3): a preset that reads a variable MilkDrop supplies
and this engine does not, or writes one MilkDrop draws with and this engine does
not, says so — in the shape ADR-0020's typo warning already takes.

```text
milkconv: Songflower: custom shape 1 is textured with the previous frame, which this engine has no stage for; it is drawn as a flat gradient fill
milkconv: Cauldron - painterly 5: reads `ang` in per-frame code, where it is a per-VERTEX variable ...
```

What is **not** warned about is a bare name the preset uses as its own
accumulator. In EEL2 an unset variable legitimately reads zero, presets rely on
that constantly, and warning on every one of them would bury the findings that
matter. The check is against MilkDrop's own roster, not against "every name we do
not know".

### What a conversion does and does not carry

| carried | not carried |
|---|---|
| the per-frame and per-vertex programs, whole | disk textures — deliberately out of scope, and priced at 19 % of the corpus. **Since Phase 6 this is a conversion *failure***: the shader that samples one is rejected by name rather than silently rendered without it |
| **the `warp` and `comp` HLSL blocks, translated to WGSL** (Phase 6) — the ~30 intrinsics, swizzles, `if`, bounded loops, `#define`s, helper functions, the noise samplers, `GetBlur1..3`, the `q`/roam/rand/`rot_*` input surface | HLSL arrays, computed `#if` conditions, structs — each a named rejection, together well under 2 % of the corpus |
| `zoom` `rot` `cx` `cy` `dx` `dy` `sx` `sy` `warp` `zoomexp` | a custom shape’s `textured` flag — the previous frame as a fill, which needs a stage this engine has not got |
| `decay` `gamma` `wrap` `darken_center` `brighten` `darken` `solarize` `invert`, and **the video echo** `echo_zoom` / `echo_alpha` / `echo_orient` (Plan 0109) — a bundle emitted before that plan carries no register for them and must be re-converted | the second audio channel, because this engine’s analysis is mono by construction |
| the waveform's eight `wave_mode` figures and their whole `wave_*` roster | |
| up to four custom waves and four custom shapes, each with its own programs | |
| the inner and outer borders, and the motion-vector grid | |
| the initial conditions, re-applied at the top of every frame | |
| `q1`–`q32`, `t1`–`t8`, `megabuf`, `gmegabuf` | |

**A converted preset draws its own light and the scene's deposit stays off.**
MilkDrop's light source *is* the waveform, and up to Phase 3 the converter emitted
a stand-in ring in its place so there was something for the mesh to move. That
stand-in is gone: the frames further down this section that share a radial-arm
character are from that era and are labelled as such.

### Rates are converted, and that is why a preset moves at the right speed

MilkDrop's `zoom`, `rot`, `dx`, `dy`, `warp` and `decay` are all **per rendered
frame**, so a preset drifts at half the speed on a machine running twice the rate.
This engine's vocabulary is per second throughout (ADR-0019). The runtime converts
at MilkDrop's nominal **30 fps** — a factor becomes `v^30`, a rate `v * 30` — which
is what makes a converted preset move at the speed its author saw, on any display.

Two consequences worth knowing before reading a number in a bundle:

- A `zoom` of `1.02` in a `.milk` file is `1.81` per second here. The numbers are
  not on the same scale as a hand-authored `[params]` binding beside them.
- `fps` reports `30` to the program rather than the display's real rate. A preset
  that reads `fps` is compensating for a cadence, and telling it the truth about a
  144 Hz display would compensate twice.

### The moment of truth, captured

Plan 0100 Phase 3's done-when is a human judgement — does a converted preset move
the way its name says — so the evidence is frames rather than an assertion. All
four below are from `presets-milkdrop-original`, converted with no hand editing
and rendered at `bass=0.6,mid=0.5,treb=0.45` after 300 frames:

| | |
|---|---|
| ![LSD Zoomtunnel](images/milkconv/lsd-zoomtunnel.png) | ![A Million Miles from Earth](images/milkconv/a-million-miles.png) |
| *Goody — LSD Zoomtunnel*: rays converging on a dark centre, which is the preset's whole name. | *Krash & Rovastar — A Million Miles from Earth (Ripple Mix)*: the ripple's radial lobes. |
| ![Hypnotic Spiral](images/milkconv/hypnotic-spiral.png) | ![Escher's Tunnel](images/milkconv/eschers-tunnel.png) |
| *Idiot — Marphets Surreal Dream (Hypnotic Spiral Mix)*: a spiral, from `rot` and `zoom` composed per vertex. | *Aderrasi — Contortion (Escher's Tunnel Mix)*. |

And the same spiral over a clip, which is what "motion in the right direction, at
the right rate" actually looks like — six frames of `--signal click:120`:

![the spiral turning over a clip](images/milkconv/spiral-over-a-clip.png)

> **The radial-arm character these four share is the stand-in deposit, not the
> presets.** It is the same ring in all of them; what differs is what the mesh
> does to it. They are kept because they are what Phase 3's done-when was judged
> on — read the *motion* in them, not the figure.

### The same thing again with the draw layer (Phase 4)

Phase 4 replaced that stand-in with what MilkDrop actually draws. The figure is
now the preset's own, and the difference is not subtle:

| | |
|---|---|
| ![Escher's Tunnel, with the draw layer](images/milkconv/eschers-tunnel-drawn.png) | ![Songflower, with the draw layer](images/milkconv/songflower-drawn.png) |
| *Aderrasi — Contortion (Escher's Tunnel Mix)*, the same preset as the last frame of the block above. The tunnel and its dark core are the mesh, as before; the magenta lattice at the edges is the preset's own custom shapes and waveform, which the stand-in ring had nothing to say about. | *Aderrasi — Songflower (Hybrid Plant)*: four custom shapes and a waveform over a field with `fDecay = 1`, so nothing ever fades. This is the case that reads as a flat white frame under a purely additive draw seam — see the blend note in [`presets/README.md`](../presets/README.md#milk--the-table-you-do-not-write). |

### And with the shaders (Phase 6)

MilkDrop 2's `warp` and `comp` pixel shaders are translated to WGSL at
conversion time and run as the preset's own passes — the warp shader replaces
the built-in decay fragment, the comp shader replaces the built-in present.
Both frames below are corpus files converted with no hand editing, same
settings as above:

| | |
|---|---|
| ![Blur Mix 3, with its shaders](images/milkconv/blur-mix-3-shaded.png) | ![Myriad Mosaics, with its shaders](images/milkconv/myriad-mosaics-shaded.png) |
| *Geiss — Blur Mix 3*: the waveform stroke sharp, its history softened through the three-level blur chain the comp shader reads with `GetBlur1..3` — the look the preset is named for. | *Geiss — Myriad Mosaics*: the mosaic grid is the warp shader's `cos` tiling, the swirl inside each tile its 3D noise-volume lookup (`sampler_noisevol_hq`), the grain its `rand_frame` dither. |

### `--report`: how much of the corpus converts, and what happens to the rest

Point the converter at a directory instead of a file and it walks it, converts
everything, and prints what became of each — Plan 0100 Phase 5:

```bash
cargo run -p milkconv --release -- --report path/to/corpus
cargo run -p milkconv --release -- --report path/to/corpus --render   # slower, adds the third count
cargo run -p milkconv --release -- --report path/to/corpus --json     # for diffing two runs
```

Three counts, and the third is the interesting one:

| count | is |
|---|---|
| **parse** | the `.milk` section layout read without error |
| **compile** | every EEL2 program in it turned into bytecode |
| **render non-blank** | the emitted preset loaded into the engine and put light on screen |

Only the third catches a preset that converts *perfectly* and draws *nothing* —
the failure class the plan's Risks call the worst for reputation, because it looks
like our bug rather than like a refusal. It needs a device, which is why it is
behind `--render`; without an adapter the run reports the other two rather than
failing, which is [ADR-0016](adrs/0016-gpu-tests-opt-in-ci-scope.md)'s policy for
captures. Budget about 50 ms a preset.

> **It asserts no threshold and it exits zero however bad the numbers are.** Per
> [ADR-0071](adrs/0071-a-numeric-test-contract-states-a-property-or-names-its-machine.md)
> a coverage percentage is a property of one corpus and one converter at one
> moment, so the report names both and is never a gate. A non-zero exit would
> make it one the first time somebody put it in a script. **The ranked reasons
> are the output that matters** — they are the work list.

The report also prints the two predictions the census made *before* any of this
was built, beside what was measured, so a ranking that disagrees sharply with
either is read as evidence about the **converter** rather than about the corpus.
That is the whole reason the census came first, and it costs nothing to state the
prediction in advance.

#### What it said, 2026-08-16

Two readings of the same corpus, one plan phase apart — together they are the
record of what Phase 6 changed.

**Before the shaders (Phases 1–5).** Over all three collections at
`WORK/milkdrop-corpus`, on `windows x86_64`; 10 325 of 10 347 converted, but the
82 % `unconverted-shader` row is the honest caveat — four fifths of the corpus
rendered with its `warp`/`comp` HLSL missing. The full-corpus `--render` probe
(same day) counted **10 257 non-blank of 10 325**, with 60 of the 68 blanks
declaring MilkDrop 2 — the picture was in the shader nothing ran yet.

```text
  files seen                     10347  100.0 %
  parse                          10347  100.0 %
  compile                        10325   99.8 %
  render non-blank               10257   99.1 %

WHAT A CONVERSION COULD NOT CARRY (top rows):
    8482   82.0 %  unconverted-shader
    2251   21.8 %  disk-texture
                               predicted   measured
  reads a disk texture           19.0 %     21.8 %
  MilkDrop 1.x, no shaders       18.0 %     17.9 %
```

**After the shaders (Phase 6).** The translator ran; `unconverted-shader` is
gone, and the disk-texture class moved from a warning to a **named conversion
failure** — a preset whose shader samples a texture we refuse to ship no longer
loads without it and renders something its author never drew:

```text
  stage                          count  share
  ---------------------------- -------  -------
  files seen                     10347  100.0 %
  parse                          10347  100.0 %
  compile                         8289   80.1 %
  render non-blank                8063   77.9 %

WHY A FILE DID NOT CONVERT, ranked:
    1217   11.8 %  warp shader disk-texture
     609    5.9 %  comp shader disk-texture
      75    0.7 %  warp shader unsupported
      74    0.7 %  comp shader unsupported
      37    0.4 %  warp shader parse
      16    0.2 %  comp shader unknown-name
      15    0.1 %  per_frame
       6    0.1 %  warp shader unknown-name
       5    0.0 %  per_vertex
       2    0.0 %  comp shader parse
       2    0.0 %  per_frame_init
```

Read together: **8 289 presets (80.1 %) now convert whole, shaders included,
and 8 063 (77.9 %) render non-blank under the probe** — the converted set is
about 97 % of everything not bound to a disk texture. The `unsupported` and
`parse` rows (under 2 % combined) are HLSL arrays, computed `#if` conditions
and similar exotica, each rejected by name. Of the 226 converted-but-blank
presets, 218 declare MilkDrop 2 — their shaders *ran*, so those are
shader-fidelity findings — and the 8 shaderless blanks are the same 8 the
pre-Phase-6 probe found. On the 552-preset original pack, where disk textures
are rarer, the same run reads 476 converted (86.2 %) and 471 non-blank
(85.3 %).

**Both predictions held**, which is the result that says the ranking is about the
corpus. The shaderless share landed at 17.9 % against a predicted 18 % — as close
as a prior gets. The disk-texture class came in 2.8 points *above* its prediction,
and that direction is expected rather than surprising: the census counted a `grep`
for a texture name, and the converter also flags `sampler_pc`, so it sees a
slightly wider class than the census could.

"Converts" is still a **weaker** claim than "renders as authored" — 82 % of the
corpus carries HLSL that nothing translates yet, which is the number Phase 6 is
sized by.

The eleven files that do not compile are genuinely malformed — a stray backslash,
a `sin` with no arguments, a truncated `per_frame_init`. Two decisions in the
parser are what got the number that high, and both were measured rather than
assumed; they are on `milk::join_code` and `eel::Compiler::argument`.
