# Headless capture and video

Render a preset **with no window**. A surface-less GPU context draws into an offscreen texture and
hands back raw pixels, and three things are built on that: a `shot` CLI that writes PNGs and a
metrics report, an offline video renderer that walks a WAV end to end, and a live video-out that
publishes every frame to another application.

This is the page a preset author reaches for. The differential test harness that runs on the same
offscreen path is [Testing and visual QA](testing.md), and the MilkDrop converter is
[MilkDrop conversion](milkdrop-conversion.md).

A headless render is a **pure function** of `(preset, input, frame-count, size)`. Scenes are
reseeded per capture, every frame steps at a fixed `dt` where the live app injects its real one, and
every capture path pins the preset's **declared numeric seed** so the grammar's `hash()` and
`noise()` reproduce even where the preset asked for `seed = "random"`
([ADR-0051](adrs/0051-seeded-grammar-randomness-with-per-run-opt-in.md); see
[Seeded randomness](../presets/README.md#seeded-randomness--hash-noise-and-generator-seed)). The
analysis is deterministic too, so a render is reproducible and diff-able.

> The seed pin is the one place a capture deliberately shows you something other
> than the live app: a `seed = "random"` preset's filmstrip is *an* instance of it,
> not the instance a user will see. Tune with a number.

<!-- toc:begin depth=3 -->
- [Captures pin the floor tier](#captures-pin-the-floor-tier)
- [The `shot` CLI](#the-shot-cli)
  - [The horizon: does a world still look like itself after minutes?](#the-horizon-does-a-world-still-look-like-itself-after-minutes)
  - [`--render`: a music video from a track](#--render-a-music-video-from-a-track)
  - [The three calibration traps](#the-three-calibration-traps)
  - [Aiming a capture at a transient](#aiming-a-capture-at-a-transient)
  - [A full-size frame under real audio](#a-full-size-frame-under-real-audio)
  - [What the report's columns mean](#what-the-reports-columns-mean)
  - [Which preset library a shot uses](#which-preset-library-a-shot-uses)
  - [Editing presets live](#editing-presets-live)
  - [Examples](#examples)
  - [Held bindings: the scene may not be seeing this frame's value](#held-bindings-the-scene-may-not-be-seeing-this-frames-value)
- [The live video-out: `ritmolux --stream`](#the-live-video-out-ritmolux---stream)
  - [`--sink spout`: another application on the same machine](#--sink-spout-another-application-on-the-same-machine)
  - [`--sink stdout`: raw frames on a pipe](#--sink-stdout-raw-frames-on-a-pipe)
  - [The TouchDesigner side](#the-touchdesigner-side)
  - [Which GPU, and why it is not a preference](#which-gpu-and-why-it-is-not-a-preference)
  - [Presets, and stopping](#presets-and-stopping)
  - [What it costs, and what those numbers mean](#what-it-costs-and-what-those-numbers-mean)
  - [What it does not do](#what-it-does-not-do)
<!-- toc:end -->

## Captures pin the floor tier

**Every capture path renders at the `Floor` quality tier, and it cannot do
otherwise by accident.** `Renderer::new_headless` takes no tier argument and
resolves `Floor` by construction ([Plan 0044](plans/done/0044-quality-tiers.md) / [ADR-0045](adrs/0045-quality-tiers-floor-and-rich.md)),
so there is no field a test can forget and no environment variable that can change
what a baseline looks like. `shot` defaults to `floor` for the same reason and
deliberately does **not** read `RLX_TIER`.

Two reasons, and both are load-bearing:

- **Reproducibility.** A tier sets capacity — particle counts, the segment budget,
  the internal-grid caps — so a baseline blessed on a rich-tier run and compared
  against a floor-tier one differs for a reason that has nothing to do with the
  change under test. A capture is a pure function of its inputs (NFR §6), and the
  tier would otherwise be a hidden input.
- **Suite cost.** The golden and visual-QA suites run on the WARP software
  adapter, where fill and instance count translate directly into wall-clock. At
  rich values the same suite would draw 3x the attractor particles into a 4K-capped
  trail grid on a CPU rasterizer.

`--tier rich` is the deliberate opt-in, for spot-checking that the raised budgets
actually render.

**A tier is no longer one number per scene**, and the attractor is where that
started: its sample budget is a density against the render target, capped by one
ceiling in a window and a larger one under `--render`
([ADR-0140](adrs/0140-a-sample-budget-is-a-density-against-the-render-target.md)).
Every capture path — this page's stills, filmstrips and reports, and both test
suites — takes the **window's** ceiling, and at capture sizes the law's lower
clamp resolves exactly the tier's own count, so no baseline moves. See
[`--render`](#--render-a-music-video-from-a-track) for the path that does not.

> **A `Rich` capture is an instrument, and never a baseline**
> ([ADR-0064](adrs/0064-a-capture-may-pin-the-rich-tier.md)). Use it to *look*, not
> to bless: a rich capture must never be written into `core/tests/golden/`. The
> reason is dated rather than principled — `TierConfig::RICH`'s values are the
> provisional ones [Plan 0044](plans/done/0044-quality-tiers.md) shipped and its Phase 4 calibration has never run, so
> every `Rich` baseline would be a re-bless waiting on a number nobody has measured
> yet. Revisit after that calibration, not before.

Omitting the flag is exactly `--tier floor`, byte for byte — verified at [Plan 0057](plans/done/0057-the-attractors-compute-path.md)
Phase 1 rather than assumed, since "the default is the old behaviour" is the kind
of claim that quietly stops being true.

> The consequence [ADR-0045](adrs/0045-quality-tiers-floor-and-rich.md) names and accepts: rich-tier regressions are caught only
> by those spot checks and by on-device runs, not by the suite. That is a real hole,
> not a solved problem.

> `--size` is part of that tuple, and since [Plan 0033](plans/done/0033-internal-resolution-and-preset-surface.md) it does more than crop: the
> `trails` and `kaleido_*` stages size their internal grid from the render target
> ([ADR-0034](adrs/0034-internal-resolution-follows-the-target.md)), so a preset composing either one genuinely renders *differently* at
> 640x360 than at 1080p rather than merely smaller. A given size is still exactly
> reproducible; two sizes are no longer scaled versions of one picture. Capture at
> the size you are judging.

Everything here is **dev/agent tooling**. The `image` crate is a *dev-dependency*
only ([ADR-0011](adrs/0011-image-crate-for-capture-tooling.md)), so the shipped `ritmolux.exe` is untouched; the CLI is a
`cargo run --example`, not a subcommand of the app.

> Package name note: the standalone crate is `standalone`, so the invocation is
> `cargo run -p standalone --example shot -- …`.

## The `shot` CLI

Render one preset to a PNG (the agent then Reads the file):

```bash
cargo run -p standalone --example shot -- --preset "Whorl" --frames 120 --out shot.png
```

Flags:

| flag | meaning |
|------|---------|
| `--preset <name>` | preset to render (by name, as shown in the report / library); optional when the library holds exactly one preset |
| `--presets <dir>` | load the library from `<dir>` instead of the resolved preset directory |
| `--preset-file <path>` | load exactly one preset from `<path>` (beats `--presets`) |
| `--set k=v,...` | constant stimulus frame: `bass,mid,treb,onset,bar,novelty` (0..1), `tempo` (BPM), `beat` (non-zero = true). Keys are the **grammar's** names, so `tempo` is what a binding writes. It reaches the frame's scalars only — **not** the 64-band spectrum, so `bin(x)` reads `0` — see [the calibration traps](#the-three-calibration-traps) before trusting a value |
| `--frames <N>` | frames to advance before capture (default 120) |
| `--size <WxH>` | render size (default 1280x720) |
| `--out <path>` | output PNG (single shot) or dir/file (`--all`) |
| `--all` | contact sheet of every preset, labeled (needs `--out`) |
| `--report [family=<sys>]` | per-family metrics table — reactivity, animation, coverage and the [transient probe](#the-transient-columns); `family=` takes any `system` name — all twelve the scene registry carries: `attractor`, `emitter`, `fragment_field`, `lsystem`, `parametric_curve`, `reaction_diffusion`, `shape_collage`, `shape_field`, `spectrum`, `star_pattern`, `swarm`, `warp_mesh` |
| `--json` | emit the report as JSON instead of a text table |
| `--signal <kind:param>` | synth-audio filmstrip (see below) |
| `--audio <clip.wav>` | filmstrip from a 16-bit PCM WAV |
| `--strip <N>` | frames tiled along the audio (default 8) |
| `--at <hop>,...` | explicit filmstrip hops, beating `--strip`'s even spacing — [how to capture a transient](#aiming-a-capture-at-a-transient) |
| `--frame-at <hop>` | **one** frame at that hop, written at the full `--size` — no tile scaling, no border. Needs `--signal`/`--audio`; not combinable with `--at`. [Which flag takes a picture worth keeping](#a-full-size-frame-under-real-audio) |
| `--tier floor\|rich` | quality tier to capture at (default `floor` — see above) |
| `--horizon <minutes>` | [the long-run drift check](#the-horizon-does-a-world-still-look-like-itself-after-minutes) — N **simulated** minutes at capture cadence, one statistics row per interval. Minutes of wall clock; never a gate |
| `--interval <secs>` | simulated seconds between horizon rows (default 30) |
| `--render <clip.wav>` | [offline video](#--render-a-music-video-from-a-track) — walk the clip at `--fps` and stream every frame to stdout for an encoder to read. Deterministic and decoupled from real time |
| `--fps <n\|num/den>` | the render mode's frame rate (default 60). A decimal is rejected: write `30000/1001`, not `29.97` |
| `--ffmpeg <path>` | spawn this encoder and wire the pipe, so one command produces a file. Needs `--out <file>`. No encoder ships and there is no fallback |
| `--crf <0-51>` | the encoder's rate-quality setting (default 18, archival). Higher is smaller; `+6` is about half the size. Needs `--ffmpeg` — [the one argument you may move](#the-one-canonical-ffmpeg-invocation) |
| `--help`, `-h` | print the usage text and exit 0 |

Bad arguments and unknown presets exit non-zero with a message.

### The horizon: does a world still look like itself after minutes?

Everything else on this page measures the first seconds of a preset's life. The
four behavioral gates capture **30 frames** — half a second — and the reactivity
gate a few seconds of hops. A live set runs for hours, and a world whose
mechanism *accumulates* can drift across that gap with the whole suite green:
[Plan 0075](plans/done/0075-the-content-renaissance.md) cohort 4's Shatter piled onto its flow field's attractors over minutes
and collapsed live three times without a single test going red.

`--horizon` is the instrument for that class. It renders N simulated minutes at
the fixed 1/60 s capture step and prints one row per interval — coverage,
concentration (`peak/mean`), and motion over the figure's own footprint since the
previous row — plus a trend line per statistic.

```bash
# Ten simulated minutes of a swarm world, a row every 30 s
cargo run -p standalone --example shot -- \
  --preset-file presets/swarm_shatter.toml --horizon 10 --size 96x96 --set bass=0.7

# ...and the same run as JSON
cargo run -p standalone --example shot -- \
  --preset-file presets/swarm_shatter.toml --horizon 10 --interval 60 --json
```

**When to run it** ([ADR-0099](adrs/0099-the-show-length-horizon-is-a-spot-check-and-it-splits-in-two.md) states the trigger once, so it can be found): on a
world whose mechanism has an **accumulation axis** — sustained forces, feedback
with net gain, a population that can migrate or pile up. Record the verdict in
the world's own header, the way the fold-edge verdicts were recorded.

**Five things to know before reading a row.**

1. **It is never a gate.** Nothing here runs in CI or fails a build, and the mode
   applies no threshold. It prints a trend; the verdict is yours. Making it a
   gate was rejected on measured cost — the reactivity gate's move to real PCM
   alone cost 1.8x over 41 presets, and a minutes-long capture per preset is
   orders beyond that.
2. **Run a static control beside the subject.** The statistics are image-domain
   proxies for a simulation-domain event: particles piling onto attractors
   *reads* as coverage falling and concentration rising, and that correlation is
   strong without being identity. A world that renders one frame forever prints
   `delta 0.0000, monotone 0.00` on every statistic — that flat series is what
   makes a sloped one mean something.
3. **It is slow by construction** — 3,600 renders per simulated minute. At
   96x96 on a hardware adapter a ten-minute horizon measured **16 s** for a
   single-pass world (`Halo`) and **54 s** for a reaction-diffusion one
   (`Etching`, 13 passes a frame). Nobody will run it casually, which is the
   honest cost of refusing a gate.
4. **The verified length is the documented one, and here is what verified
   means** ([Plan 0099](plans/done/0099-the-horizon-reaches-its-own-length.md)). `--horizon 10` renders all 36,001 frames on **all three**
   shipped reaction-diffusion worlds — `Etching`, `Mitosis`, `Verdigris` — with
   the resident set **flat**: 324 MB reported before the run and 400 MB after,
   of which the render itself travels **0.8 MB** (398.8 MB at 4 s to 399.6 MB at
   54 s) — the 21 sampled images themselves. Measured on the Windows
   development box, hardware adapter, debug build, at 96x96 ([ADR-0071](adrs/0071-a-numeric-test-contract-states-a-property-or-names-its-machine.md) — a
   different machine or profile is a different measurement).

   It was not always true, and the shape of the old failure is worth keeping.
   The capture path submitted every non-sampled frame without ever polling the
   device, so it reclaimed nothing between two *sampled* frames — 1,800
   consecutive unpolled submits at the default interval. Retention was per
   **pass**, not per pixel: an RD world retained **950 KB a frame** against a
   36 KB captured frame, where single-pass worlds retained ~30 KB, so RD reached
   the allocator first at ~4.4 GB and died with an invalid readback buffer. That
   made it look like a property of the RD family; it was the poll cadence. The
   step now polls, which costs about **1.5x** wall clock on an RD world and
   bounds the memory of any run at any length.

   **A run that cannot reach its requested length says so on stdout**, in the
   table's place, with the wall clock and resident set it died at — not only as
   a stderr line. And a `--horizon` the `--interval` does not divide is rounded
   **down** to the last whole interval (rows are exact multiples, which is what
   makes them comparable between runs), so the header states the length the run
   actually reached and flags the shortfall when there is one.
5. **It cannot see most of the process.** GPU resource churn and the frame-time
   spike on a preset switch are not reproducible in a headless loop that never
   rebuilds a surface. [`--soak`](nfr.md) is the instrument for that half, and
   the two are deliberately separate ([ADR-0099](adrs/0099-the-show-length-horizon-is-a-spot-check-and-it-splits-in-two.md)).

   **One exception, and it is the one that mattered:** the resident set of the
   render loop itself. The cost block reports it, and reading that column is
   what found the ceiling in point 4 — a headless loop cannot reproduce a
   *switching* app's memory behaviour, but its own growth is exactly what a
   long run is in a position to see. This point used to name the resident set
   among the things a horizon cannot see; that was too broad.

The stimulus is `--set`, held for the whole run, which is what makes a row at
minute nine comparable with a row at minute one — so `--horizon` and
`--signal`/`--audio` are mutually exclusive rather than one silently winning.

Reading the trend block: `delta` is how far a statistic travelled end to end, and
`monotone` is what share of the steps went that way. A world grinding into a
corner reads a large `delta` at a `monotone` near `1.00`; a world breathing
around a stable mean reads a `delta` near zero whatever its `monotone`. Where the
line falls between *drifting* and *alive* is a judgement about the look, which is
why the tool declines to make it.

Two properties hold and are asserted rather than assumed
(`standalone/tests/shot_cli.rs`): the same world at the same horizon produces
**identical** rows across runs, and a row at interval *k* does not depend on how
far the run was asked to go — so a two-minute run and a ten-minute run agree on
every row they share. The wall clock and resident set in the cost block are the
exception: those are properties of the box, reported and never asserted
([ADR-0071](adrs/0071-a-numeric-test-contract-states-a-property-or-names-its-machine.md)).

### `--render`: a music video from a track

Every other mode on this page takes a **picture**. `--render` walks a WAV end to
end at a fixed frame step and writes a **video stream** to stdout, for a user's
own `ffmpeg` to encode ([ADR-0114](adrs/0114-the-engine-renders-video-offline-and-delegates-encoding.md)).

```bash
# One command: --ffmpeg spawns the encoder and wires the pipe.
cargo run -p standalone --example shot -- \
  --preset "Supernova" --render track.wav --fps 30 --size 1920x1080 \
  --ffmpeg ffmpeg --out track.mp4
```

**A `--preset` that names nothing costs nothing.** The name is checked against
the roster before the encoder is spawned and before a GPU device is built, so a
typo exits 1, lists the roster's keys, and **writes no file at all**. That check
sits where it does because `ffmpeg` exits 0 on a frame stream that never carried
a frame: rejecting the name any later left a valid, playable, audio-only MP4 at
the destination, a few hundred bytes that a glance cannot tell from a short
render. If you have such a file on disk from an older build, that is what it is.

**The roster is keyed on a preset's `name` field, not on its filename** —
`presets/attractor_leviathan.toml` is `--preset "Leviathan"`. That is the
confusion the key list in the error exists to settle.

**No encoder ships, and that is a decision rather than an omission.** A 1080p
RGBA frame is 8.29 MB and four minutes at 60 fps is 119 GB, so the frames can
never reach disk before the encoder — a pipe is the only viable shape, not an
optimization. A static encoder is in turn larger than this application's whole
[size budget](nfr.md#4-size-and-dependencies). So `ffmpeg` is a documented
prerequisite for this one feature, and `ritmolux.exe` does not change size.

**What makes this worth having is that it cannot drop a frame.** Every live
visualizer's render loop is welded to a real-time audio device, so its "export"
is a screen capture: bounded by the display's refresh and resolution, degraded
under load, and different every run. Nothing in this path races a display — `dt`
is injected ([ADR-0013](adrs/0013-c-abi-v4-render-dt.md)), the DSP is a pure function of its input window
([NFR §6](nfr.md#6-determinism)), and the grammar's randomness is pinned
([ADR-0051](adrs/0051-seeded-grammar-randomness-with-per-run-opt-in.md)). Two runs of the same command produce **byte-identical** streams, and
that is asserted in `standalone/tests/shot_cli.rs` rather than inferred.

**A render draws the attractor denser than a window does, deliberately**
([ADR-0140](adrs/0140-a-sample-budget-is-a-density-against-the-render-target.md)).
The attractor's sample budget is a *density* against the render target rather
than a flat tier constant —
`clamp(round(anchor * target_px / 230400), anchor, ceiling)`, anchored at
640x360, whose density was the accepted one. There are two ceilings, and
`--render` is the **only** path in this repo that takes the larger:

| target | `Rich` in a window | `Rich` through `--render` | samples per output pixel, rendered |
|---|---|---|---|
| 640x360 | 150,000 | 150,000 | 0.651 |
| 1280x720 | 600,000 | 600,000 | 0.651 |
| 1920x1080 | 600,000 | **1,350,000** | 0.651 |
| 2560x1440 | 600,000 | **2,400,000** | 0.651 |
| 3840x2160 | 600,000 | 2,700,000 | 0.326 |

The window column stops at 600,000 because a display has a frame to hit and the
buffer is paid for in every window; a render answers to memory instead, so a
1080p file reaches the reference density outright. **`--render` prints the number
it drew at** — `tier rich, attractor samples 1350000` in the header — which is
the only way to tell two files apart afterwards.

Three consequences worth knowing before you read a rendered file as evidence:

- **A rendered file is not the frames the app would have drawn at that size.**
  That is the one property `shot` otherwise works to keep, and `--render` gives
  it up on purpose. `--frame-at` and every other mode here stay on the window's
  ceiling, so a still and a render of the same instant differ at 1080p `Rich`.
- **`Floor` never moves in a window, at any size.** Its live ceiling *is* its
  anchor: 1080p at `Floor` on integrated hardware already sits on the 16.67 ms
  budget at today's 50,000 ([NFR §1](nfr.md#1-performance--adaptive-quality)),
  so the law is a no-op there. Through `--render` it scales like `Rich` does.
- **A render's resident set is ~950 MB whatever its size**, because the particle
  buffer is allocated once at the ceiling rather than at what the target asked
  for: measured 956 MB peak at 1920x1080 and 952 MB at 640x360, flat across 480
  frames either way (growth -7.9 MB and +0.1 MB). The offline ceiling is
  2,700,000 particles at 48 B, and the process holds it twice — once on the GPU
  and once as the CPU scatter it re-uploads from.

**The two clocks are different clocks.** Analysis hops arrive at
`sample_rate / HOP_SIZE` — 93.75 Hz for 48 kHz audio — and frames at `--fps`.
The loop advances whichever is due next, so most 60 fps frames take one new hop
and some take two. Rendering one frame per hop instead would run the picture at
64% speed against its own soundtrack. Above the hop rate (`--fps 240`) frames
repeat the last published analysis frame rather than interpolating one: the DSP
publishes on hop boundaries, and inventing values between them would put
something on screen the analyzer never derived.

**The wire format is Y4M** (`ffmpeg -f yuv4mpegpipe`), and the stream is
self-describing — `YUV4MPEG2 W1920 H1080 F60:1 Ip A1:1 C444 XCOLORRANGE=FULL` —
so a mistyped geometry cannot silently produce garbage and a non-`ffmpeg`
consumer needs nothing from the command line. Three parts of that header earn
their place:

- **`C444`** — chroma is not subsampled, so the conversion loses only rounding.
- **`XCOLORRANGE=FULL`** — the samples use 0–255, not the 16–235 studio swing.
  Omit it and every player expands the range again and the file is visibly
  washed out against the app, in a way that reads as an engine bug and is not.
- **`F60:1`** — an exact rational, which is why `--fps` refuses a decimal.
  `29.97` is 30000/1001; accepting it as 2997/100 would drift the picture against
  its own soundtrack by a frame every few minutes, and nothing in this harness
  would catch it.

Y4M cannot carry RGB — the muxer *errors* on `rgb24` — so `shot` owns the
RGB→YUV conversion (full-range BT.709). It is not bijective at 8 bits, which is
why the tap-placement guard is asserted on the RGB frame the writer is handed and
never on the wire bytes; a guard written against the wire would have to be
loosened to a tolerance until it passed, which is how a guard becomes decoration.

#### The one canonical `ffmpeg` invocation

`--ffmpeg <path>` spawns the encoder, wires the frame stream into its stdin, and
passes the source WAV through as a second input for muxing. **There is exactly
one command line and `--ffmpeg` generates it** — it is echoed on stderr at the
start of every encoded run, so adapting it by hand starts from what actually
ran rather than from a wiki of incantations:

```
ffmpeg -hide_banner -nostats -y -f yuv4mpegpipe -i pipe:0 -i track.wav \
  -map 0:v:0 -map 1:a:0 \
  -c:v libx264 -preset medium -crf 18 \
  -pix_fmt yuv420p -color_range pc -colorspace bt709 -color_primaries bt709 \
  -color_trc bt709 -x264-params colorprim=bt709:transfer=bt709 \
  -c:a aac -b:a 192k -shortest track.mp4
```

**`-crf` is the one argument you may move, and `--crf <0-51>` moves it.**
Everything else in that command line *describes the stream* — geometry, mapping,
colour — and a lever on any of those would be a way to mistype what is already on
the wire. The default of 18 is archival and deliberately not shareable: on a 30 s
slice of `attractor_leviathan` at 1080p60 rich it is **119 Mbit/s**, against
**60** at `-crf 23` and **27** at `-crf 28`, where a typical 1080p60 upload
recommendation is about 12. Lower is bigger and better; the scale is roughly
logarithmic, so `+6` is about half the size. The default does not move, because a
capture is evidence first — re-encoding down from an archival master is lossy but
possible, and the reverse is not.

```bash
# The same render at a shareable size.
cargo run -p standalone --example shot -- \
  --preset "Supernova" --render track.wav --fps 30 --size 1920x1080 \
  --ffmpeg ffmpeg --out track.mp4 --crf 23
```

`--crf` needs `--ffmpeg`, and says so rather than being ignored: without an
encoder there is no command line for it to appear in. **The other size-control
route is the raw-stream path** — omit `--ffmpeg`, redirect stdout, and run your
own encoder with whatever rate control, codec, or two-pass recipe you want. That
path is the one that has always been there and `--crf` does not replace it; what
it buys is that the common case stays one command line.

No `-s` or input `-pix_fmt`: the geometry is on the wire, which is the point of a
self-describing stream. The `-map` pair is explicit so a clip carrying album art
cannot displace the rendered picture. The four colour tags are the half most
likely to ship wrong — an untagged file is one the player expands from studio
swing and shows washed out. `-color_trc bt709` rather than `iec61966-2-1`: the
tap hands over sRGB-encoded samples and the two are close, but every player
assumes the former and some ignore the latter outright.

**Two of those four need saying twice, and that is why `-x264-params` is on the
line.** The `-colorspace` flag is honoured by the libx264 path; `-color_primaries`
and `-color_trc` are dropped by it. That was measured on **ffmpeg 8.1 (gyan.dev
full build)** and on no second build. A file written with all three reads back

```
color_range=pc  color_space=bt709  color_primaries=unknown  color_transfer=unknown
```

from `ffprobe -v error -select_streams v:0 -show_entries stream=color_range,\
color_space,color_primaries,color_transfer`. It is not a reporting convention:
asking the same command for `bt2020`/`smpte2084` moves the matrix and leaves the
other two `unknown`, so the loss is in the encoder wrapper. Setting them on x264
directly lands all four, and the flags stay beside it — they carry the same
values and are what a build that honours them reads. Nothing about the encoded
picture changes; this is metadata. `the_four_colour_tags_survive_into_the_container`
in `standalone/tests/shot_cli.rs` reads them back off a produced file wherever
`ffmpeg` and a GPU are both present, so the claim above is checked rather than
asserted.

**A dead encoder reports the encoder's failure, not our broken pipe.** The child's
stderr is drained on a thread — echoed line by line as it arrives, and kept — so
if it exits non-zero `shot` exits non-zero quoting its last words. Writing into a
full pipe blocks until the encoder drains it, which *is* the backpressure
handling: nothing is buffered on this side of it. Both halves are asserted in
`standalone/tests/shot_cli.rs`, the second against a stand-in encoder that dies
on its first argument.

**`ffmpeg` is a prerequisite, and its absence is a named error naming the flag** —
never a fallback to something else, because a quietly-substituted encoder is what
would make an exported file untrustworthy.

Practical notes:

- **stdout is the video** on the raw-stream path. Every human-readable line goes
  to stderr, so a summary printed the way the other modes print one would be
  eight bytes of garbage in the middle of the file. `--out` is therefore rejected
  *without* `--ffmpeg` and required *with* it.
- Nothing validates the container. That the frames and the stream are right is
  tested; whether `ffmpeg` made a good MP4 is outside this harness and [ADR-0114](adrs/0114-the-engine-renders-video-offline-and-delegates-encoding.md)
  accepts it.
- `--render` takes its own clip and is mutually exclusive with `--signal`,
  `--audio` and `--horizon` — any pair would mean silently ignoring one of two
  stimuli.
- The frame count is `ceil(clip_seconds x fps)`: the trailing partial frame is
  rendered rather than dropped, so the picture is never shorter than the audio.
- It renders at the **floor** tier like every other capture path. `--tier rich`
  is the opt-in, and it is the mode where it is most worth paying for — an
  offline render has no 60 Hz deadline, so the frame-time governor never fires.
- **Every run reports its resident set**, sampled across the render and printed
  on stderr at the end:

  ```
  render: resident set 434 MB, growth +0.1 MB across 600 frames after a
  +75.9 MB warm-up (peak 434 MB, 21 samples)
  ```

  A render that leaks is the same defect as a live session that leaks, so
  [NFR §12](nfr.md#12-runtime-memory)'s no-session-growth requirement applies
  here and is measured the same way. Read the **growth**, not the absolute: the
  latter is a ~327 MB vendor driver floor on the reference box and does not
  travel. Growth is charged from the *warm* reading rather than the baseline
  because the whole warm-up step lands at once on the first draw — pipelines
  compiled, GPU resources built — and charging it against the baseline would
  print a flat run as "+76 MB", which is exactly what the per-frame retention
  [Plan 0099](plans/done/0099-the-horizon-reaches-its-own-length.md) found looks
  like. The peak keeps both honest: a run that grew and was reclaimed reads flat
  end to end, and only an intermediate sample tells it apart.

  The full four minutes at 1080p/60 — 14,400 frames of `reaction_mitosis` at
  `--tier rich`, the thirteen-pass family that hit 0099's wall — measured
  **334 MB, growth -8.1 MB, peak 342 MB** on the Windows dev box (2026-08-17,
  hardware adapter, release). Where the old retention would have reached ~4.4 GB.

#### A filter stage between `shot` and the encoder

The frame stream is a pipe, so anything that speaks Y4M can sit in the middle of
it. `tools/sd-filter/` is the first such stage: an img2img diffusion pass with
ControlNet holding the render's geometry, so the attractor becomes canyon rock
while the shape keeps tracking the music. **It is documented in one place, and
that place is [`docs/diffusion-filter.md`](diffusion-filter.md)** — setup, the one
canonical command, the flags and what it costs. Nothing on that path ships in the
release zip.

`--ffmpeg` is **not** used there: it spawns the encoder itself, which leaves no
seam to insert a stage into. Composing the pipe by hand is the whole point of the
raw-stream path.

**The encoder line is invariant, and that is the pipe-level fact worth keeping
here.** A stage in the middle changes the picture and never the frame count, so
the `ffmpeg` invocation above never learns a rate that has to agree with a flag on
another process. There is no `-r` to keep in sync and no way to desynchronize the
audio silently — which is why the encoder half of that pipe is unchanged,
character for character, whether or not a stage is in it.

### The three calibration traps

`--set` is a **held** stimulus: it writes the analysis frame directly and that
same frame drives every captured frame. That makes it perfect for isolating one
binding and wrong for three things people reach for it anyway.

**Trap 1 — `--set beat=1` holds the beat gate high for the whole capture.** Real
beats are transient: `beat` fires on one hop and is false on the next. Held high,
every `beat`-driven accent in the preset is at full deflection in the still you
are looking at — a `+ beat * 0.155` thickness term that should flash for a frame
instead reads as the preset's baseline. The result is that a *working* preset
looks broken in a still (permanently blown-out, or so busy the geometry is
unreadable), and the natural response — turning the accent down — breaks it for
real on live audio. If you want a beat, capture one:

```bash
# Transient beats through the real onset detector, no asset needed
cargo run -p standalone --example shot -- --preset "Drift" \
  --signal click:120 --strip 8 --out click.png
```

**Trap 2 — `--set` band magnitudes are not real levels.** `--set bass=0.8` writes
`0.8` onto the frame; a band that arrives through the analyzer comes in through
the normalizer instead, and *what `0.8` means* differs between the two.

> This trap used to be stated as a magnitude gap — a full-scale 60 Hz sine reads
> `bass ≈ 0.19` and a 120 BPM click track peaks at `bass ≈ 0.011`, so `--set
> bass=0.8` is four to seventy times too hot. **Those figures are raw-scale and
> [ADR-0049](adrs/0049-analysis-v2-dual-resolution-axis-normalized-bands.md)
> retired them**; they now describe `bass_raw`, and the table in
> [what real material actually produces](#what-real-material-actually-produces)
> is where they live. On the normalized scale the same sine reads `1.000`, because
> a steady tone *is* its own recent peak.
>
> The trap survives the rescaling, in a different shape: a normalized band is a
> fraction of the material's own peak, so `--set bass=0.8` claims "80 % of peak,
> held forever", which no music does. The number is no longer wrong by two orders
> of magnitude — it is wrong about *time*.

So every `--signal` / `--audio` filmstrip **prints the levels it measured**, and
those are the numbers to calibrate a gain against (here, `--signal bass:60`):

```
audio levels over 355 analysis hops (past warm-up) — calibrate gains against these, not against --set magnitudes:
  signal      min     mean      max
  bass      1.000    1.000    1.000
  mid       0.000    0.000    0.000
  treb      0.000    0.000    0.000
  onset     0.006    0.405    1.000
  onset peaks at 1.000 on hop 20 — the shipped attractor reseed gates run 0.50 to 0.75
```

`--audio <clip.wav>` on real material is the one that answers "what does my music
actually produce"; `--signal` answers it for a known synthetic tone. Use `--set`
to ask "does this binding do anything at all", not to decide how much of it to
apply.

**`onset` is in that table since [Plan 0057](plans/done/0057-the-attractors-compute-path.md)**, and it is not a band. It is there
because a whole class of binding — every shipped attractor's `reseed`, every
beat-latched accent — is gated on it, and whether a given stimulus ever *crossed*
such a gate was not answerable from a capture. The peak hop is printed beside it
because that hop is the argument to [`--at`](#aiming-a-capture-at-a-transient).

### Aiming a capture at a transient

`--strip N` samples the clip at N **evenly spaced** hops. That is the right
question for "what does this look like over the clip" and the wrong one for "what
does the frame the gate fired on look like" — and the two get confused because a
strip *looks* thorough.

The arithmetic: under `--signal click:120`, `onset` crosses `0.75` on **7 hops out
of 375**. An evenly-spaced strip of 8 lands on one of them by luck, and when it
misses, the capture shows a preset whose reseed never fired — which is
indistinguishable from a preset whose reseed does not work.

So the level table names the peak hop and `--at` takes it:

```bash
# 1. What does this clip do, and where?  -> "onset peaks at 1.000 on hop 46"
cargo run -p standalone --example shot -- --preset-file presets/attractor_ink.toml \
  --signal click:120 --strip 8 --out over-the-clip.png

# 2. Now capture the transient itself, and the frames either side of it
cargo run -p standalone --example shot -- --preset-file presets/attractor_ink.toml \
  --signal click:120 --at 44,46,48,54 --tier rich --out the-reseed.png
```

Hops are indices from hop 0 of the clip — the same numbering the level table
reports. Order is yours (a before/after pair reads left to right), duplicates are
an error, and a hop past the end of the clip is an error rather than a silently
missing tile.

> **`--signal click:120` reaches every shipped reseed gate, and always did.**
> [ADR-0066](adrs/0066-a-reseed-disturbs-the-cloud-rather-than-replacing-it.md)
> and [design-backlog 0050](design-backlog.md) both state the opposite — that the
> synthesized clip's `onset` never clears `0.56`, let alone `attractor_clifford`'s
> `0.75`. That was true on the **raw** onset scale and was invalidated by
> [ADR-0049](adrs/0049-analysis-v2-dual-resolution-axis-normalized-bands.md)'s peak
> normalization, whose attack is *instant*: an isolated transient reads `1.000` on
> the hop it arrives, whatever its absolute magnitude. Measured at [Plan 0057](plans/done/0057-the-attractors-compute-path.md)
> Phase 1, `click:120` produces **7 clean rising edges over `0.75`**, one per beat.
> The gate was never the problem; **aiming** at it was.

Which kind to reach for, for a gate rather than a level:

| kind | `onset` min / mean / max | as a reseed stimulus |
|---|---|---|
| `click:<bpm>` | 0.000 / 0.033 / 1.000 | **the one to use** — 7 isolated edges over `0.75`, one per beat |
| `dynamic:<bpm>` | 0.001 / 0.153 / 1.000 | 12 edges, in amongst real band dynamics |
| `noise:<seed>` | 0.826 / 0.953 / 1.000 | **wrong** — pinned above every gate, so an edge-triggered binding fires once and never again, exactly as `--set onset=1` does |

**Trap 3 — `--set` leaves the 64-band spectrum silent, so `bin(x)` reads `0`.**
`--set` writes the analysis frame's *scalars*; there is no key for the log-band
array, and `AnalysisFrame::default()` leaves all 64 bands at zero. So under any
`--set` capture:

- every `bin(x)` call in an expression returns `0`, whatever `bass`/`mid`/`treb`
  say, and
- the whole **`spectrum`** system draws its `base` resting comb and nothing else —
  the readout is inert, which looks exactly like a broken preset.

This is not a bug in `--set` (it writes what you ask and nothing else); it is the
one part of the frame it cannot reach. **`bin(x)` and the `spectrum` system have
to be verified through `--signal` or `--audio`**, both of which run the real
analyzer over real samples and therefore populate the array:

```bash
# The band array through the real FFT - the readout actually moves
cargo run -p standalone --example shot -- --preset "Halo" \
  --signal chord --strip 3 --out comb.png
```

`--report` builds its stimulus frames in code rather than from `--set`, and those
**do** light the band array (each named band lights the slice of the log spectrum
it summarises), so the report's numbers are real for a spectrum preset.

### A full-size frame under real audio

`--frame-at <hop>` ([Plan 0088](plans/done/0088-the-docs-get-pictures.md)) captures **one** frame at the named hop and writes
it at the full `--size`. It is the flag every committed documentation image uses,
and the reason it had to exist is that neither of the other two paths produces a
picture worth keeping:

| what you run | what you get | why it is not a documentation image |
|---|---|---|
| `--frames 120 --out x.png` | a clean 1280x720 frame | it runs under **silence** — the capture path builds a default analysis frame, so a band-driven preset photographs at its resting state |
| `--at 340 --out x.png` | the right stimulus, through the real analyzer | the filmstrip scales every frame to a fixed tile height and draws a gutter round it: a single-hop `--at` at default size comes back **363x208 with a border** |
| `--set bass=0.8 --frames 120` | full size, and wrong three ways | [the three calibration traps](#the-three-calibration-traps) — a held `beat`, band magnitudes no music reaches, and a silent 64-band array |

`--frame-at` is the first two combined:

```bash
# The picture the docs commit: full size, real dynamics, on the loudest beat
cargo run -p standalone --example shot --release -- \
  --preset-file presets/attractor_leviathan.toml \
  --signal dynamic:110 --frame-at 300 --size 1280x720 --tier rich \
  --out docs/images/gallery/attractor.png
```

**Hop 300 is not arbitrary, and a later hop is worse.** `dynamic:110`'s phrase
builds for six beats and then rests for two at an amplitude of `0.04`, and at
110 BPM with a 512-sample hop that rest begins at hop 306 — so anything past that
photographs a reactive preset at its resting state. 300 is the last hop of the
loudest beat: maximum energy, and the most scene time an accumulating family can
have before the rest. The arithmetic is in
[`scripts/docs-shots.mjs`](../scripts/docs-shots.mjs)'s header, which is also
where a per-image deviation from 300 has to say why.

It shares everything with the strip except the write: the same hop numbering, the
same `capture_audio` call, and the same level table on stdout. A hop past the end
of the clip is an error, as `--at`'s is. Passing both `--frame-at` and `--at` is
an error — they answer the same question two ways — and `--frame-at` without
`--signal`/`--audio` is an error naming what is missing, since there is no clip to
advance through.

Two captures of the same `(preset, signal, hop, size, tier)` on **one machine and
binary** are byte-identical. That is a same-adapter claim only: the golden suite
treats a `0.02` mean channel difference as ordinary rasterizer drift, so
cross-machine byte equality does not hold and nothing here asserts it.

### What the report's columns mean

```
  preset           bass    mid   treb  onset  drive   anim    rate  cover  level  rise  fall
  Shatter         0.099  0.064  0.013  0.106  0.103  0.092 0.0357+  0.997 0.0316   11+    4+
  Stipple         0.050  0.013  0.001  0.011  0.057  0.010 0.0132+  0.222 0.5313    7+   26+
```

| column | question it answers |
|---|---|
| `bass` `mid` `treb` `onset` | how far the frame moves when that stimulus alone comes up, against silence — "does this preset respond to bass at all" |
| `drive` | how far the frame moves under the **combined** stimulus — silence against everything up at once, same depth and same size ([the two motion readings](#the-two-motion-readings)) |
| `anim` | how far the frame moves between two capture depths **under silence** — does it have a life of its own |
| `rate` | how far the frame moves **frame to frame** — the only column that walks consecutive frames, and the only one measured at 96x96 ([the two motion readings](#the-two-motion-readings)) |
| `cover` | fraction of the frame that differs from the corner background — [a low value is often correct](#a-low-cover-is-not-a-defect) |
| `level` | how much **light** the picture carries: mean linear light over the pixels `cover` counts as lit ([what the level column measures](#what-the-level-column-measures)) |
| `rise` `fall` | the **transient probe** (below) — frames to settle after a step up, and after the matching step down; a **`+` suffix** means the value is a *lower bound*, not a measurement (below); [read them as evidence, not a verdict](#what-the-transient-columns-cannot-see) |

#### What the `level` column measures

**Linear light, over the lit set** ([ADR-0150](adrs/0150-the-level-question-is-asked-in-linear-light.md)).
Two halves, and both matter:

- **Linear**, because the stored pixels are sRGB-encoded and that curve is
  concave: a mean over the bytes under-reports a brightness change by roughly
  half. That factor of two is the property, and it is what the statistic is for.
  **The move you see is not the trim you made**, and expecting it to be will
  make a working column look broken. Trimming `star_rosewindow`'s `brightness`
  by 30 % moves this column from `0.0532` to `0.0408` — **23 %**, measured
  2026-09-01 on the DX12 software adapter. Two things eat the rest, and both are
  the pipeline rather than the statistic: the tonemap is the identity only below
  its knee and compressive above it, so bright content moves less than its
  source; and the lit set itself shrinks as dimmed pixels fall under `cover`'s
  threshold, which lifts the mean of what stays.
- **Over the lit set**, the same pixels `cover` counts, so an authored
  background does not dilute the reading. The two columns read one picture from
  two directions: `cover` is how much of the frame is lit, `level` is how bright
  that lit part is. In the sample above `Shatter` fills the frame at a low level
  and `Stipple` lights a fifth of it, brightly.

**It is a comparison number and never a threshold.** There is no level a preset
ought to hit, and one row's cell says nothing on its own. What it is for is a
before/after on the same preset — *did this change make it brighter, and by how
much* — or an ordering across a family. The lit predicate is a threshold on the
stored bytes, so the statistic is linear light over a set chosen in code space;
[ADR-0150](adrs/0150-the-level-question-is-asked-in-linear-light.md) records why that seam is accepted rather than solved.

**The blind spot, by construction:** a preset that goes wrong by changing its
*background* is invisible here. That is the price of not being a background
detector, and `cover` is the column that sees it.

The name column is fourteen characters wide, and a longer name is **elided in
the middle**, not at the tail: `Tiled Rosette Mono` prints as `Tiled R~e Mono`.
The tail is what distinguishes a name in this library — `Mono`, `Gallery`,
`Bordered`, `Walk` — and a tail truncation threw it away, which is how two
presets came to print as one row label in all three tables (design-backlog 0131).
A `~` in a label means characters were dropped there.

Two extra labeled blocks print under the table (the table itself stays un-widened,
so every historical number keeps its place): the **realistic-levels** reading
(`reactivity_low` — the same bands at the levels real music reaches, [ADR-0042](adrs/0042-reachability-measured-on-the-expression-tree.md)) and,
since [Plan 0077](plans/done/0077-the-quiet-sky.md), the **footprint** reading (`reactivity_footprint`) — the same
differentials divided by the **union of lit pixels** instead of the whole frame
(`metrics::footprint_diff`, [ADR-0091](adrs/0091-the-animation-gate-scores-motion-against-the-figures-footprint.md)). Read the footprint block when a mean band
column shows ~0.000: reactivity concentrated in a small footprint — a `bloom_amount`
halo, a sparse figure — is diluted by the whole-frame mean, and before this reading
existed the house workaround was binding a `flash` lever just so the report had
something to see. On a backdrop-heavy preset the union mask approaches the whole
frame and the reading degrades toward the mean column — it never sits meaningfully
below it, so it fails toward the old behaviour rather than inventing reactivity.

Every one of those but the last two is a **settled** measurement: the capture
holds one stimulus for every frame it renders, so each smoother has converged
long before the pixels are read. That is the right question for "does it
respond", and it is exactly why those columns are **identical for any
`[smoothing]` constant**.

#### The two motion readings

`drive` and `rate` were added by [ADR-0134](adrs/0134-motion-is-two-readings-and-anchoring-is-why-neither-can-be-a-threshold.md),
which exists because the report could not see either axis. Every other statistic
on this page is a **settled differential** — two frames captured a fixed count
apart — so it cannot see *rate* at all, and the reactivity columns drive one band
at a time, so they cannot see combined drive either. A preset needed four live
passes with a person watching before the lane found its defect, and the harness
stayed green and unchanged through all four.

**`drive`** is the silent 48-frame capture differenced against the fully-driven
one: same frame count, same scene time, same `REPORT_SIZE`. It is the question
the listener asks — *does this change when the music does* — and it is not the
same question as the four band columns beside it, which drive one stimulus each.
A preset reading several bands together can measure low on every one of them and
still be strongly driven; one can read plausibly on all four and be driven by
none of them together. A preset with no audio binding at all reads `0.000`.

**`rate`** is the mean difference between **consecutive** frames over the settled
tail of the transient probe's loud plateau. It separates frozen from boiling,
which is the axis the report has never had.

Both ride captures the report already takes — no extra render pass, no extra
readback, no resize — so the full-library wall clock is unchanged.

Two things to know before reading either number:

- **`rate` is measured at `PROBE_SIZE` (96x96), not `REPORT_SIZE` (192x192) like
  every column printed beside it.** It rides the probe's frames, and the probe
  runs small for the reasons in [the transient columns](#the-transient-columns).
  `frame_diff` is a normalized mean so the two are broadly comparable, but
  "broadly" is not a property: never compare a `rate` against a number measured
  at another size. `--json` carries `measured_at_px` beside the value for exactly
  this reason.
- **A `rate` cell carries the same `+` mark the transient cells do**, taken from
  the probe's own `rise_settled`. An unsettled rise means the frames it averaged
  were still travelling toward the plateau, so the number describes the transient
  rather than the steady motion. The mark is **common, not exceptional** — see
  the transient section on why a great many presets never settle inside 48
  frames.

**Neither number is a gate, and `rate` does not sort the library.** This is the
finding the ADR turns on rather than a caveat bolted to it. Motion inside a
static repeating structure reads as *calm* — the eye reads a pattern updating in
place — while the same amount of motion in an unanchored world moves the whole
sheet bodily and has to be far quieter to watch. Measured: `fragment_tiledmono`
and `fragment_drostemono` both sit **higher** than a draft that was rejected for
shaking, and both are comfortable. No pixel statistic in this harness models
anchoring, so any threshold consistent with the rejected draft would fail two
shipped presets. Read both columns **against family neighbours**, the way `geom`
is read ([ADR-0083](adrs/0083-in-frame-geometry-is-measured-at-the-line-renderers-draw-seam.md)),
and never sort on them.

#### The transient columns

`rise` and `fall` are the one pair that is *not* settled. The probe drives a
step — silence, a held stimulus, silence again — reads back **every** frame, and
counts how many it takes for the frame to reach 90 % of its total change each
way ([ADR-0039](adrs/0039-verify-easing-with-a-transient-probe-not-a-committed-clip.md)).
That makes [ADR-0035](adrs/0035-asymmetric-attack-release-easing.md)'s
`{ attack, release }` pair visible: a scalar `[smoothing]` entry selects the same
constant both ways, so its two numbers match; a pair snaps up and glides down, so
`fall` runs well past `rise`.

Reading them:

- **`1` and `1`** — no easing on whatever the stimulus drives. The frame is fully
  there the frame after the step.
- **equal, both large** — a scalar `[smoothing]` entry, or an asymmetric one whose
  two constants are close.
- **`fall` much larger than `rise`** — an `{ attack, release }` pair doing its job.
- **`0` and `0`** — the step did not move the frame at all. Check the reactivity
  columns: this usually means the preset does not respond to the stimulus, not
  that its easing is instant.

#### What the transient columns cannot see

**The probe measures the frame, not the parameter.** It reads pixels, so it can
only see easing through whatever curve the scene puts between a bound value and
its output — and for most scenes that curve is neither linear nor even monotone.
Two consequences, both real and neither fixable by measuring harder:

- **A saturating response reads flat.** `rose_trails` is the worked example: its
  1.25 spin against a max-decay feedback drives the frame to the same place
  whatever `thickness` says, which is why the content lane once rendered five
  values from 1.10 to 2.30 — *including the untouched original* — and could not
  tell them apart. A preset like that will report a transient that has nothing to
  do with its `[smoothing]` table, and no column here will warn you.
- **The scene's own motion is measured too.** A fragment field's fold, a feedback
  trail and a particle cloud all keep changing while the parameter settles, and
  the probe cannot separate that from the response. Over the shipped library this
  shows up as presets reporting `fall` *below* `rise`, which is backwards for any
  easing.

Measured over the shipped set on 2026-07-27 — a snapshot of what the probe sees,
not a figure anyone maintains — presets carrying at least one
`{ attack, release }` entry had a median `fall / rise` of **1.02**, with
`fall > rise` in about half of them; presets with only scalar entries sat at
**0.60**, with barely any. So the columns separate the two populations
*directionally* and lose the magnitude almost entirely:
`Smooth Pulse`, the worked asymmetric example with a 0.60 s release,
reads `26 / 31` where a purpose-built near-linear fixture at a 0.5 s release reads
`3 / 61`.

> **Those numbers were taken before the `+` marker existed, and every one of them
> would carry it today** ([Plan 0038](plans/done/0038-line-family-unreachable-levers.md) Phase 8). They were produced by exactly the
> defect this section goes on to describe: at `PROBE_WINDOW` = 48 a 0.60 s release
> is 1.33 τ, leaving ~26 % of the travel undone, and the fixture's `3 / 61` is now
> known to be `3 / 69` when measured to settlement. Read the snapshot for its
> *shape* — two populations, separated directionally, magnitude lost — and not for
> its magnitudes, which is the same warning the rest of this section gives, now
> with the arithmetic behind it. It has not been re-taken: re-snapshotting is not
> what makes the columns trustworthy, marking them is.

**Read the columns as evidence, not as a verdict.** A wide `fall / rise` gap is
good evidence the easing is working. A narrow one is not evidence it is broken.
The place easing is proven is `core/tests/easing.rs`, against fixtures built to
have a near-linear response precisely so the measurement is of the easing and not
of a scene; everything else is a preset-shaped approximation of that.

One smaller limit, and it is sharper than it was first written: the probe's window
is **48 frames (0.8 s) each way**, so a release constant longer than about 0.35 s
does not fully settle inside it. This page used to say such a response "reads
*clamped*" — **it does not, and that word was the trap.** `frames_to_settle`
normalizes against *the segment's own last frame*, so when that frame is still
travelling the measured total is short and every threshold is crossed early. The
number that comes back is not pinned at the window length and is not obviously
wrong; it is a plausible, smaller frame count. Worse, the bias is uneven — the
0.9 threshold is pulled in harder than the 0.5 one — so a truncated fall also
reads as a *more even* fall than it is.

That is not hypothetical. [Plan 0038](plans/done/0038-line-family-unreachable-levers.md) Phase 3 measured two easing orderings, one of
which had an effective time constant of 1.0 s against a 1.6 s window, and read the
truncation as a difference in the shape of the two falls. Measured to settlement
the two shapes are identical and differ only in speed — 73 frames against 145,
where the truncated run said 61 against 78. See
[ADR-0040](adrs/0040-spectrum-level-curve-applies-before-the-easing.md)'s Outcome.

**The rule, and the function that enforces it.** `frames_to_settle` cannot detect
this about itself: normalizing against the last frame *guarantees* the threshold
is crossed inside the segment, so `frames_to_settle(seg, f) < seg.len()` is a
tautology rather than a check. Before trusting a frame count, gate it on
`metrics::segment_settled(segment, tol)`, which extrapolates the geometric tail
from three points spread across the segment and answers whether the last frame is
within `tol` of the asymptote. Sample widely rather than from the end: captures
are 8-bit, and a response slow enough to outrun its window moves by *less than one
code value per frame* near the end, so adjacent frames decode as identical and
read as settled exactly when they are not.

**So `--report` marks rather than pretends** ([Plan 0038](plans/done/0038-line-family-unreachable-levers.md) Phase 8). A transient cell
carries a **`+`** when `segment_settled` cannot certify the response arrived —
`61+` means *at least 61 frames*, never 61. Each family's table then names how
many of its presets marked. `--json` carries the same fact as `rise_settled` and
`fall_settled` booleans, so a consumer reading only the counts cannot mistake a
truncated response for a settled one.

**Expect most of the shipped set to mark, and for two different reasons the
suffix does not separate.** One is the window, above. The other is far more
common here and is not a defect in the probe at all: a scene whose own motion
never stops — a fragment field's fold, a feedback trail, a particle cloud — has
**no asymptote to settle to**, so `segment_settled` correctly declines to certify
one. That is the same limitation this page already describes as "the scene's own
motion is measured too"; the mark just moves it from a caveat you have to remember
into the cell itself. A preset whose cells are *unmarked* is the interesting case:
it means the number is a measurement.

Widening the `--report` window does not fix that table's *separation* problem,
which is a different thing — measured at 96 frames the scalar-only median got
**worse** (0.60 → 0.92) for double the wall clock. Scene saturation is what hides
the magnitude there. Window length is what corrupts a slow response's shape, and
the two are not the same defect.

#### A low `cover` is not a defect

`cover` counts pixels differing from the corner-sampled background by more than a
threshold, on any channel — a **symmetric** difference, so dark-on-light and
light-on-dark are measured identically. An ink-remapped look is not penalised by
construction, and a low reading is not evidence of one.

What a low `cover` means is that the frame is sparse, and sparse is often the
intent. `reaction_coral_bloom` reports **0.128**, about as low as the shipped set
goes, and is healthy — it is the family's ink-on-paper variant, a pale print whose
chaotic-branching regime genuinely covers an eighth of the frame. The number is
truthful; what it cannot tell you is "sparse on purpose" from "dead".

So the column **names suspects rather than convicting them**. A low `cover`
alongside a dead `anim` and flat reactivity columns is worth investigating; a low
`cover` on a preset that is deliberately a thin figure on a wide ground is the
report working.

#### The second reading: the same columns at realistic levels

Under each family's table is a second block
([ADR-0042](adrs/0042-reachability-measured-on-the-expression-tree.md)):

```
  at realistic levels (bass 0.661 mid 0.575 treb 0.281 onset 0.145) — read the *gap* ...
  preset            bass     mid    treb   onset   gates   ceils   occ
  Aurora           0.183   0.165   0.212   0.025       0       1     0
  Ember            0.078   0.053   0.032   0.035       0       0     0
```

> **The realistic levels changed meaning in [Plan 0048](plans/done/0048-analysis-v2-and-the-retune.md).** They are now fractions of
> each signal's own recent peak ([ADR-0049](adrs/0049-analysis-v2-dual-resolution-axis-normalized-bands.md)), not magnitudes, which is why they read
> `0.661` where they used to read `0.04`. Any `--report` number quoted in an older
> commit message, ADR Outcome or backlog entry was measured on the raw scale.

The four columns are measured exactly as the ones above, from stimuli set to
[what real material produces](#what-real-material-actually-produces) instead of
to `1.0`. **The gap between the two rows is the reading, not either number
alone**, and it has a direction:

| what you see | what it means |
|---|---|
| both healthy | the preset responds at levels it will actually meet |
| **full scale lively, realistic ~0** | gained or gated against a magnitude music never reaches. This is the defect that hid six presets for months |
| realistic close to full scale | a compressive `curve`/`smoothstep` doing its job, or a binding already saturating at low input |
| **realistic > full scale** | something inverted or saturating — a parameter past its useful range at full scale, reading *back* down |

The last row is why the full-scale columns stayed. They also keep every number
quoted in an older commit, ADR or backlog entry meaning what it said.

Two things this pair cannot see. **`beat` is an event, not a magnitude**, so it
is `true` in both readings — a beat-latched binding holds across the gap while an
`onset`-scaled one falls away, which is a distinction, not a fault. And the
**band array is on its own scale**: the low stimulus lights the `spectrum` slice
to the same level as the scalar, while real material's per-band mean is `0.020`
with peaks to `0.338`, so a `bin()`-reading preset reads *lower* here than the
scalar gap suggests.

#### Reachability: gates the probe never drove both ways

The `gates` and `ceils` counts are not measured from pixels at all. They come
from walking each preset's **expression trees** while evaluating them over 12 s
of `dynamic:110` through the real analyzer, recording which way every
**comparison** and every `select()` condition went, and how close every `clamp()`
came to its upper bound. A frame differential structurally cannot answer this —
`select(c, 6, 8)` and `select(c, 6, 6)` diff identically, and neither names
*which* gate.

Four kinds of finding come out of that walk. Three are named one per line
underneath the table; the ceilings are summarized:

- **`GATE`** — a `select()` whose condition never went both ways. One branch of
  the preset has never rendered. Named with its source text, so the threshold to
  re-gain is in front of you.
- **`COMP`** — a **comparison** (`> < >= <= == !=`) that only ever took one
  value, so it read as a constant `0` or `1`
  ([ADR-0043](adrs/0043-reachability-reports-comparison-nodes.md)). This catches
  two shapes a `GATE` line cannot. One is the bare comparison as a whole
  binding — `reseed = "onset > 0.55"`, the idiomatic boolean-param form, which
  holds no `select()` at all. The other is one **half of a composite condition**:
  in `select(min(tempo > 124, bass + treb > 0.38), 4, 1)` the `GATE` line names
  the whole `min(...)`, and since a `tempo` gate is legitimately one-sided here
  (below), a reader would dismiss it — so each half is also reported on its own,
  and the excusable one can no longer launder the other.
- **`SAT`** — a `clamp()` whose inner value sat **at** its upper bound for 90 %
  or more of the probe
  ([ADR-0062](adrs/0062-clamp-occupancy-is-the-saturation-instrument.md)). The
  binding is a gain that has stopped being a function of the audio: it reads as
  the constant its ceiling is, for anything above a whisper. Named one per line
  with its occupancy, because unlike a decorative ceiling this is a **HARD**
  failure — see [the saturation gate](#saturation-a-hard-gate-on-clamp-occupancy).
- **clamp ceilings** — a `clamp()` upper bound the value never approached. The
  bound is decorative and the parameter's real range is narrower than it reads.
  These are **not** printed one per line: a single summary line per family gives
  the count and names the furthest three. All of them are in `--json`.

A comparison that is the **direct condition** of a `select()` reports once, as
the `GATE` line only — that line already names it and says which branch never
ran, which a `COMP` line cannot. So `GATE` and `COMP` never double-report the
same finding.

The `gates` column counts `GATE` + `COMP` together: both say a branch of the
preset's behavior has never happened. `ceils` counts the ceilings, and `occ`
counts the saturated clamps.

**`ceils` and `occ` are opposite ends of one measurement**, taken on the same
traversal from the same two numbers — a `clamp()`'s inner value and its upper
bound. `ceils` asks how *close* the value ever came (its peak, as a fraction of
the bound) and fires when the answer is "never near": the ceiling is decorative.
`occ` asks how *long* the value stayed there (the fraction of hops at or above
the bound) and fires when the answer is "always": the ceiling never released. A
clamp can trip at most one of them, and the healthy state is neither — a bound
reached on peaks and released in between. `occ` is by far the more serious of
the two, which is why it is the one that is gated.

**A flag is a suspect, not a conviction.** It says *this* stimulus never drove
the gate both ways, which is a fact about the probe as much as about the preset.
The standing false positive is **`tempo`**: the probe runs at one BPM, so
`select(tempo > 132, ...)` is *correctly* one-sided and will flag forever. Check
those two by hand with `--set tempo=90` / `--set tempo=160` (see
[Examples](#examples)); a gate on a band is the one worth acting on.

The probe runs 12 s rather than the 4 s a `--signal` filmstrip synthesizes,
because the tempo tracker needs about 4 s to lock. Under a short clip `tempo`
reads a flat `0` and every `tempo` comparison flags for the wrong reason.

The `GATE` and `COMP` half of this is advisory output. It is **not** a CI gate,
and deliberately so — and as of [Plan 0048](plans/done/0048-analysis-v2-and-the-retune.md) **both** of the reasons are live again.
(The `SAT` half *is* gated, for reasons that do not apply to it — see
[below](#saturation-a-hard-gate-on-clamp-occupancy).)

The instrument is one of them, and that has not changed: the `tempo` single-BPM
false positive above accounts for 17 of the 26 flags the shipped set currently
produces, so a naive "fail if flags > 0" would fail CI permanently and a threshold
would be tuned to noise. The precondition remains a multi-BPM probe or an explicit
`tempo` exemption
([ADR-0043](adrs/0043-reachability-reports-comparison-nodes.md)).

The library is the other, and it regressed on purpose before being put back.
[Plan 0042](plans/done/0042-reachability-sees-every-comparison.md)'s re-audit measured **0 genuinely dead gates**, and that held until
[ADR-0049](adrs/0049-analysis-v2-dual-resolution-axis-normalized-bands.md) changed what a band level means: nine bindings written against raw
levels then compared against normalized ones and never went false, so their
`else` branches were dead. That was the priced cost of the one-time retune
[ADR-0049](adrs/0049-analysis-v2-dual-resolution-axis-normalized-bands.md) chose, and **[Plan 0048](plans/done/0048-analysis-v2-and-the-retune.md) Phase 7 cleared it** — the library is back to
zero genuinely dead gates, with the residual flags all `tempo` one-sidedness.

**The walk used to be unable to see a gain, and now it can.** A comparison is a
fork it can watch; `clamp(bass * 16, 0, 0.3)` is not — it has no `select()`, no
comparison, nothing two-valued, and it is simply an arithmetic expression that
has quietly become a constant. [Plan 0048](plans/done/0048-analysis-v2-and-the-retune.md) Phase 7 measured **263 of 332 clamped
band terms pinned at their ceiling**, and 14 presets with no live audio term at
all, none of it visible to any instrument this project had. The `occ` column and
the `SAT` lines are that instrument
([ADR-0062](adrs/0062-clamp-occupancy-is-the-saturation-instrument.md)), and
unlike the rest of the reachability block they are backed by a gate.

#### Saturation: a HARD gate on clamp occupancy

`core/tests/saturation.rs` runs the same walk over the embedded set and **fails
the build** on any `clamp()` whose occupancy reaches the threshold. It is the one
part of the reachability block that is not advisory, and the reason is the one
[Plan 0048](plans/done/0048-analysis-v2-and-the-retune.md) Phase 7 supplies: for the whole window between [ADR-0049](adrs/0049-analysis-v2-dual-resolution-axis-normalized-bands.md) landing and the
retune, every automated signal was green and nobody had cause to run a report. An
instrument that requires suspicion to fire does not address the failure that
there was nothing to be suspicious of.

The threshold is a **measured** constant (`SATURATED_OCCUPANCY`), taken from the
retuned library's own distribution rather than reasoned to, and it therefore has
a shelf life: re-measure it whenever the library changes materially.

A `clamp()` that is *supposed* to pin — a safety rail whose job is to bind at
peak — declares itself in the preset:

```toml
[occupancy]
exempt = ["fade"]   # this clamp is a rail, not a gain: pinning is the design
```

An exemption silences the **gate**, not the diagnostic: the binding still shows
up as a `SAT` line and in the `occ` count, so it stays visible in review. That is
deliberate — an exemption is a place to hide, and the mitigation is that it is
explicit, in the file, and still reported.

### Which preset library a shot uses

Highest precedence first:

1. `--preset-file <path>` — one preset, parsed from that file.
2. `--presets <dir>` — every `*.toml` in that directory.
3. **`RLX_PRESET_DIR`** — the environment override
   ([ADR-0014](adrs/0014-preset-dir-override-for-dev-iteration.md)).
4. The per-user preset directory (`%APPDATA%\Ritmolux\presets` on
   Windows; see [`presets.md`](presets.md#where-preset-files-live)).
5. The presets compiled into the binary.

The `[source]` label printed after every capture names the winner, so a PNG's
provenance is never a guess. The two flags are **errors** when they come up empty
— a missing file, unparseable TOML, or a directory with no valid presets exits
non-zero rather than quietly capturing some other library. Levels 3–5 degrade
downward instead, exactly as the app does.

`shot` resolves levels 3 and 4 through the **same** `standalone` library function
the app calls, so the two can never disagree about which folder your edit landed
in.

### Editing presets live

Point both surfaces at the repo's version-controlled `presets/` and edit a
`.toml` — no rebuild, no relaunch:

```bash
# Windows (PowerShell): the app reloads the edited file within ~150 ms
$env:RLX_PRESET_DIR = "./presets"; cargo run -p standalone --release

# ...and every shot in that shell reads the same folder
cargo run -p standalone --example shot -- --preset "Whorl" --out shot.png
```

For a one-off capture, the flags say it explicitly and need no environment:

```bash
# The whole repo library
cargo run -p standalone --example shot -- --presets presets --preset "Whorl" --out a.png

# A single file — --preset is unnecessary, the one-entry library names itself
cargo run -p standalone --example shot -- --preset-file presets/fragment_whorl.toml --out a.png

# Metrics for the repo library rather than the seeded per-user copy
RLX_PRESET_DIR=./presets cargo run -p standalone --example shot -- --report
```

The app hot-reloads an override folder but **never seeds** into it (it is yours,
not ours), and `diagnostics.log` / `config.toml` stay under the per-user app
directory. The foobar2000 plugin does not read `RLX_PRESET_DIR`.

### Examples

```bash
# Shot a preset under a loud beat, at a custom size
cargo run -p standalone --example shot -- --preset "Drift" \
  --set bass=1,onset=1,beat=1 --size 960x540 --out pulse.png

# Both sides of a gate, e.g. `select(treb > 0.55, 12, 8)` on kaleido_order.
# The subject is a docs/examples file rather than a shipped preset on purpose:
# NO preset in the library binds `tempo`, so the tempo-gate example this
# replaces named a file that had been deleted and a variable nothing uses.
cargo run -p standalone --example shot -- \
  --preset-file docs/examples/tuning/step-2-naive-bands.toml \
  --set treb=0.2 --out below-the-gate.png
cargo run -p standalone --example shot -- \
  --preset-file docs/examples/tuning/step-2-naive-bands.toml \
  --set treb=0.9 --out above-the-gate.png

# Labeled contact sheet of the whole library
cargo run -p standalone --example shot -- --all --out gallery/

# Metrics report as a text table, or JSON for parsing
cargo run -p standalone --example shot -- --report
cargo run -p standalone --example shot -- --report --json > report.json

# Beat filmstrip from a synthesized click track (no asset needed)
cargo run -p standalone --example shot -- --preset "Drift" \
  --signal click:120 --strip 8 --out click.png

# The frame a reseed actually fired on, at the tier the app starts in
cargo run -p standalone --example shot -- --preset-file presets/attractor_ink.toml \
  --signal click:120 --at 44,46,48,54 --tier rich --out reseed.png

# ...or from the one synthesized kind with dynamics
cargo run -p standalone --example shot -- --preset "Drift" \
  --signal dynamic:110 --strip 8 --out groove.png

# Filmstrip from a real clip (16-bit PCM WAV)
cargo run -p standalone --example shot -- --preset "Perseids" \
  --audio assets/test/clip.wav --strip 8 --out clip.png
```

**Three committed scripts drive `shot`**, all self-documenting in their headers — run them with
`node`, no arguments needed:

| Script | What it renders | Output |
|---|---|---|
| `scripts/tuple-sheets.mjs` | one labeled contact sheet per attractor family, a cell per roster entry — the menu a `tuple` curation judges | `target/`, not committed |
| `scripts/tuple-paths.mjs` | one filmstrip per candidate `tuple_from`/`tuple_to` pair, a cell per `morph` step; pairs the engine refuses a walk for are skipped rather than rendered as identical cells | `target/`, not committed |
| `scripts/docs-shots.mjs` | every documentation image, from an inline manifest naming the preset file, stimulus, hop, size and tier behind each one | **`docs/images/`, committed** |

The first two read the roster straight out of `core/src/render/scenes/particles/family.rs`, so a
roster edit needs no script edit, and their output is a scratch artifact — re-run them when a
judgement is owed.

`docs-shots.mjs` is the odd one: its output *is* committed, so the manifest is the provenance record
for every picture in the documentation, and swapping which preset represents a family is one line
plus a re-run ([ADR-0100](adrs/0100-documentation-images-are-committed-headless-renders.md)). It
writes only under `docs/images/` and refuses an entry pointing anywhere else. **It is not a CI gate
and must not become one** — renders are not byte-reproducible across machines, so freshness is a
close-ceremony sweep duty rather than a check.

`--signal` kinds: `click:<bpm>`, `bass:<hz>`, `treble:<hz>`, `noise:<seed>`,
`chord`, `dynamic:<bpm>`. The synth path needs no committed asset. `--audio`
reads uncompressed 16-bit PCM WAV only (a hand-rolled reader — no decoder
dependency); other encodings are a followup.

#### `dynamic:<bpm>` — the one kind that rises and falls

Every other kind is a **steady** tone or steady noise, and the band report says
so: `bass:60` reads min/mean/max `0.187 / 0.187 / 0.187` — zero variance — and
`chord` `0.058 / 0.059 / 0.060`. A filmstrip of those exercises the DSP with
material that never changes, which is not what any preset is authored against.
`click:<bpm>` has real transients but peaks at `bass ≈ 0.011`, far below anything
a shipped preset is gained for.

`dynamic:<bpm>` is three layers on a beat grid — a pitch-dropping kick every
beat (bass), eighth-note hats (treble), a harmonic pad that swells across each
beat (mid) — under an **8-beat phrase** that builds for six beats and rests for
two. Measured at 110 BPM through the real analyzer:

| band | min | mean | max | `max / mean` |
|---|---|---|---|---|
| bass | 0.0035 | 0.0399 | 0.1063 | **2.67** |
| mid | 0.0005 | 0.0062 | 0.0189 | **3.07** |
| treb | 0.0000 | 0.0059 | 0.0320 | **5.45** |

against `noise:<seed>`'s 1.78 / 1.15 / 1.07 — and it was the liveliest kind there
was. Like every generator here it is a pure function of its arguments, so a
filmstrip of it is reproducible.

Those are **raw** magnitudes, so since [ADR-0049](adrs/0049-analysis-v2-dual-resolution-axis-normalized-bands.md) they describe `bass_raw` /
`mid_raw` / `treb_raw`. Through the normalizers the same clip reads:

| variable | min | mean | max |
|---|---|---|---|
| `bass` | 0.035 | 0.661 | 1.000 |
| `mid` | 0.031 | 0.575 | 1.000 |
| `treb` | 0.002 | 0.281 | 1.000 |
| `onset` | 0.001 | 0.145 | 1.000 |

The crest factors above are what normalization *preserves* — it divides by a
slowly-moving peak, so a clip's dynamics survive while its absolute level does
not. Note every variable reaches `1.000`: full scale is a state real material
visits, not a corner.

The `waveform` trace is levelled the same way
([ADR-0139](adrs/0139-the-waveform-is-levelled-at-the-analyzer-and-publishes-its-gain.md))
and is absent from the table above for a different reason than the `*_raw` twins
are: it is not reachable from a preset expression at all, because the grammar is
scalar. What it has instead is an escape hatch the band variables lack —
`waveform_gain` is the divisor, so `waveform[i] * waveform_gain` is the amplitude
the analyzer actually read. That is what makes a capture of a `wave_mode` figure
independent of the fader it was taken at, and it is why the foobar component and
the standalone draw one picture from one track while tapping their streams on
opposite sides of the output volume.

> **It exercises dynamics. It is not evidence about real loopback levels.** A
> preset that looks right under `dynamic:110` is a preset that survives material
> which rises and falls — that is all this says. Nothing synthesized can tell you
> whether your gains match what your music actually produces; only `--audio` on
> real material does — see [the reference range](#what-real-material-actually-produces)
> below. Do not read a lively filmstrip as a calibration check.

#### What real material actually produces

Measured 2026-07-27 through `--audio` on three local clips, none committed (see
the note above about `assets/test/`). All three were peak-normalized to −1 dBFS
first, because two of them arrived 20–26 dB under-levelled and every band read
zero — a level problem in the file, not a fact about the music.

> **These are raw magnitudes, so they now describe `bass_raw` / `mid_raw` /
> `treb_raw`** ([ADR-0049](adrs/0049-analysis-v2-dual-resolution-axis-normalized-bands.md)). They used to be "the numbers to calibrate a gain
> against", and for the normalized `bass` / `mid` / `treb` they no longer are —
> that is the entire point of normalizing. Calibrate those against the `0–1` table
> in [presets.md](presets.md#set-the-threshold-from-a-measured-level-not-from---set)
> and they will hold on material like this without re-tuning. This section is now
> the reference for the **`*_raw`** variables, and for understanding *why* the
> normalized ones exist: look at how far apart the three rows below are.

| material | RMS | bass min / mean / max | mid | treb |
|---|---|---|---|---|
| electric-guitar loop, ~101 BPM, no drums | −17.7 dBFS | 0.000 / 0.000 / 0.004 | 0.000 / 0.002 / 0.013 | 0.000 / 0.000 / 0.000 |
| hi-hat percussion loop, ~102 BPM | −21.1 dBFS | 0.000 / 0.000 / 0.001 | 0.000 / 0.000 / 0.002 | 0.000 / 0.002 / 0.011 |
| trap with 808 sub, ~140 BPM | −19.9 dBFS | 0.000 / 0.007 / 0.190 | 0.000 / 0.001 / 0.026 | 0.000 / 0.001 / 0.006 |

**The shape of it matters more than any single number.** The 808's bass *peak*
(`0.190`) sits right on a full-scale 60 Hz sine (`0.187`) — the analyzer is not
quietly attenuating anything. Its *mean* is `0.007`, about 25× lower, because
real material is transient and spectrally sparse in a way no steady generator is.
Everything else here reads lower still: a guitar loop with no drums puts
essentially nothing in bass or treble, which is correct and is what most material
does in most bands most of the time.

So, in descending order of how far a stimulus is from real music — **on the raw
scale, i.e. what `bass_raw` sees**:

| stimulus | `bass_raw` it produces | vs. a real mean |
|---|---|---|
| `--set bass_raw=0.8` | `0.800` | **~100×** too hot |
| `--signal bass:60` (full-scale sine) | `0.187` | ~25× too hot |
| `--signal dynamic:110` | mean `0.040`, max `0.106` | ~6× too hot, right order for peaks |
| real music (above) | mean `0.000`–`0.007`, max up to `0.190` | — |

**This ladder is exactly what [ADR-0049](adrs/0049-analysis-v2-dual-resolution-axis-normalized-bands.md) abolished for the normalized variables.**
Its four rows span three orders of magnitude, and picking a threshold meant
knowing which rung you were standing on — which is why nine shipped mechanisms sat
dead for months. On the normalized scale every rung that carries real dynamics
lands in the same `0–1` range, so `bass > 0.8` means "near this material's own
peak" whether the material is an 808 at −1 dBFS or a quiet guitar loop. The ladder
survives here because `*_raw` still climbs it.

Two practical consequences, both of which still apply to `*_raw` and to `bin()`:

- **Calibrate against a mean, not a peak, for anything continuous** — a size, a
  zoom, a hue drift. Those spend their life near the mean, so a gain tuned to look
  right at `0.19` barely moves.
- **Calibrate against a peak for anything percussive** — a flash, a burst, a
  beat-latched accent. Those exist to fire on the hit, and the hit really does
  reach a full-scale tone's level.

The shipped library predates this measurement and is gained against the older
figures; whether it needs a re-gain pass is
[design-backlog 0020](design-backlog.md), not something to fix preset-by-preset.

> **Test audio is added manually and never committed.** Drop a 16-bit PCM WAV
> into [`assets/test/`](../assets/test/) — that folder is gitignored (only its
> README is tracked), so no licensed audio lands in the repo. Use your own or a
> royalty-free / CC0 clip; factory-library samples are fine to point at on disk
> but must not be committed. The `--signal` path needs no file, so the whole
> audio pipeline can be validated without adding anything.

The `--report --json` schema is a nested object of numbers keyed by
family/preset: per-band `reactivity`, `reactivity_low` and `reactivity_footprint`,
`animation`, `drive`, `rate`, `coverage`, `level`, `transient` (`rise_frames` /
`fall_frames` as integers plus their `ratio`), `reachability`, the pairwise
`pixel`/`shape` distinctness matrices, and `near_duplicates`.

`rate` is an **object**, not a bare number: `mean`, `settled`, and
`measured_at_px` — the size it was captured at, which is not the one the columns
beside it use ([the two motion readings](#the-two-motion-readings)). A consumer
that drops `measured_at_px` and compares a `rate` against a differently-sized run
is reading a different statistic.

`reachability` carries `dead_branches`, `unapproached_ceilings` and
`saturated_clamps` counts, the full `gates` list (each with `param`, `source`,
`kind`, and one of `always` / `peak_fraction_of_bound` / `occupancy`), a `holds`
list (each with `param` and `edge` — see [held bindings](#held-bindings-the-scene-may-not-be-seeing-this-frames-value)),
and a `probe` object naming the signal, BPM and duration they were observed
under. `holds` is the one member read off the compiled preset rather than
observed, so the `probe` provenance does not apply to it.
`kind` is `"select"`, `"compare"`, `"clamp"` or `"saturated"` — matching the
`GATE` / `COMP` / `CEIL` / `SAT` lines above — and `dead_branches` counts the
first two together. Keep the provenance when you consume it: a flag only ever
means *not observed under this stimulus*.

### Held bindings: the scene may not be seeing this frame's value

A preset may declare a [`[hold]`](../presets/README.md) table
([ADR-0180](adrs/0180-a-mathematical-world-joins-a-system-as-a-family-and-a-structural-parameter-is-held.md) rule 2): a binding
listed there is still evaluated every frame, and the value the scene receives
changes only on a named musical edge — `beat`, `bar`, or a period in seconds.

Every column on this page is a **rendered** measurement, so a hold is already
inside the numbers. What is not inside them is the reading a person does over
the same table: a binding that names `bass` is read as responding to bass, and a
held one responds to bass once a bar. So the report names them, one line each,
under the family they belong to:

```text
  held bindings: the scene sees the value the named edge last took, not this
  frame's — so read the columns above as the response the hold allows, not as
  the expression's own
  HELD: my_rose n on bar
  HELD: my_rose [layer] mix on 2.5 s
```

The block is **absent** when no preset in the family holds anything — no shipped
preset declares a `[hold]` table, so the whole library is in that case and the
lines above are from a hand-written fixture. `--json` carries the same pairs as
`reachability.holds`.

This is containment for `[hold]`, not a fix for the report's blindness to
`beat_index`-driven response — that is
[design-backlog 0192](design-backlog.md), and it is a larger hole.

## The live video-out: `ritmolux --stream`

Every other instrument on this page writes a **file**. This one writes a **live
video stream** into another program, with no window on our side, no codec
anywhere, and a latency of a frame or two
([ADR-0125](adrs/0125-the-live-video-out-is-a-spout-sender-fed-by-a-frame-tap.md)).

There are two sinks, and `--sink` picks between them.

```bash
# Spout, the default. Open a Syphon Spout In TOP in TouchDesigner and set its
# Sender Name to `ritmolux`.
ritmolux --stream --size 1280x720 --fps 60

# A pipe. Raw RGBA8 frames on stdout, for a parent process that spawned this.
ritmolux --stream --sink stdout --events | your-program
```

### `--sink spout`: another application on the same machine

TouchDesigner, Resolume, OBS — anything that receives
[Spout](https://spout.zeal.co/). **It exists only in a build with the `spout`
feature.** The shipped release `ritmolux.exe` has it; a plain `cargo build` does
not, and `--sink spout` there fails with a named error rather than starting and
publishing nowhere. To build it yourself you need the SDK staged first — it is
third-party, pinned by hash and never committed:

```bash
powershell -File packaging/spout/fetch-sdk.ps1
cargo run -p standalone --bin ritmolux --features spout --release -- --stream
```

### `--sink stdout`: raw frames on a pipe

**Every platform, no feature, nothing installed.** The frames go to standard
output as tight `width x height x 4` bytes of RGBA8, in order, with nothing
between them — no header, no length prefix, no padding. A reader cuts the stream
into frames by multiplying the geometry, which it learns from the `stream` event
on standard error before the first byte arrives:

```
{"v":1,"ev":"stream","width":640,"height":360,"fps":30,"format":"rgba8"}
```

That event needs `--events`, which is what turns the structured report on at all
([Configuration](configuration.md)). Without it the frames still flow and the
reader has to know the geometry some other way — which is why the studio always
passes both.

**The default is 640x360 at 30 fps**, not the Spout path's 1280x720 at 60: this
sink exists to feed a preview canvas, and asking the engine for four times the
pixels to shrink them into a panel costs the readback and the pipe for a picture
nobody sees at that size. `--size` and `--fps` override it in either direction.

**The writer blocks; it never drops.** A reader that stops reading fills the
pipe and stalls this process, and the deadline pacing absorbs that: frame `n`
stays due at `n * period` from the start of the run, so a stalled second costs
the frames that fell inside it and the run resumes at the frame index the wall
clock has reached rather than drifting behind it. That is the right policy for a
loop with no present deadline; a windowed preview has one and drops instead.

**A headless run is a whole show, and it is drivable.** It resolves and seeds
the same per-user preset directory a window does, watches it, and hot-reloads a
file you save into it; and it reports the same events — `roster` on every reload,
`preset` when the picture changes, `preset_error` and `preset_warning` with the
file and the line, `health` once a second. `--control` binds a listener here
exactly as it does for a window, and `hello` carries the port it actually got, so
a parent can move a parameter on the next frame. The window and the sink are what
differ between the two modes; nothing in this paragraph does
([ADR-0183](adrs/0183-the-studio-drives-one-player-and-the-show-loop-is-extracted.md)).

On a machine with no per-user data directory at all — a bare CI runner — the run
says so in one line and carries on with the presets built into the binary, which
is the same degrade every other reader of that directory performs.

**Standard output carries nothing else** while this sink is open — every
human-readable line goes to standard error, and a test holds the whole crate to
that.

### The TouchDesigner side

The operator is called **`Syphon Spout In`**, not "Spout In" — Derivative ships
the two app-to-app transports as one TOP, Spout on Windows and Syphon on macOS,
so that is the name in the OP Create Dialog. Set its **Sender Name** to whatever
the mode prints, which is not always what you asked for:

```
publishing 1280x720 at 60 fps as Spout sender 'Ritmolux'
```

`SetSenderName` **increments on collision** — a run that was force-killed leaves
its registration behind, and the next one comes up as `rlx_1`, then `rlx_2`. The
mode prints the name it actually got for exactly this reason. A TOP pointed at a
name nobody is publishing reports `No Active Sender Found`.

**No colour setting is needed on either side.** The engine reads back
display-referred sRGB bytes and Spout publishes them untouched
(`SetSenderFormat(DXGI_FORMAT_R8G8B8A8_UNORM)` matches the readback, so nothing
swaps red and blue and nothing re-encodes). A receiving TOP put beside a
`Movie File In` TOP of the same frame as a PNG is indistinguishable from it. If
your picture looks washed out or crushed, something in *your* network is
re-interpreting it, not the sender.

### Which GPU, and why it is not a preference

**On a machine with one GPU, skip this. On a hybrid laptop it is the difference
between a picture and nothing at all.**

A Spout sender shares a D3D11 texture *by handle*, and the receiver opens that
handle on its own device — which succeeds only when both devices are the same
physical GPU. Windows hands a plain console process the integrated GPU to save
power while TouchDesigner runs on the discrete one, and the receiver then
reports only `Unable to open shared Spout Texture`, naming neither adapter nor
the mismatch.

```bash
ritmolux --list-adapters              # both rosters, with their own indices
ritmolux --stream --gpu "RTX 3080"    # one name moves the renderer AND the sender
```

`--list-adapters` prints **two** lists because there are two enumerations, and
they are not assumed to agree on order — the renderer selects through `wgpu`,
the sender through the Spout SDK, and on a machine with a software rasterizer
installed the two lists are not even the same length. `--gpu` takes a name (a
substring is enough, case-insensitively) or an index, and **each side resolves it
against its own roster**; an ambiguous name is an error listing what it matched,
never an arbitrary pick.

Unset, the renderer asks for the high-performance adapter and the sender follows
it by name. Both resolved choices are printed at startup, always:

```
renderer : NVIDIA GeForce RTX 3080 Laptop GPU (Dx12, DiscreteGpu), driver 32.0.15.8142
sender   : adapter [1] NVIDIA GeForce RTX 3080 Laptop GPU
```

### Presets, and stopping

Presets rotate on the operator config's `[rotate]` dwell timer exactly as they
do in the window — **and rotation is on here even when `auto` is off**, because a
headless source has nobody to press `Space` and a four-hour set on one scene is
not what this mode is for. `--preset <name>` holds one scene and turns rotation
off. Rotations are announced:

```
rotate   : frame 5400, AutoTimer -> 'Clifford Gallery'
```

`Ctrl-C` stops the run through its own exit path, which is what makes it print
the three numbers a measurement needs — frames emitted, wall clock, scene clock.
`--frames N` bounds a run so it terminates and reports on its own.

### What it costs, and what those numbers mean

Every 30 s and at exit, the mode reports its per-stage cost and its resident set:

```
stream: render+readback 7.79 ms, spout send 0.79 ms, mean over 1800 frames
render: resident set 277 MB, growth +0.2 MB across 5400 frames ...
stream: 36000 frames, 600.00 s wall, 599.99 s scene clock, on NVIDIA GeForce RTX 3080 ...
```

**Two stages, not three.** `render+readback` is the engine drawing the frame
*and* pulling it back to the CPU: the readback blocks, so no CPU-visible instant
separates them and splitting them would need GPU timestamp queries. The second
stage is the sink's own, and it is **named for the sink** — `spout send` is the
upload into the sender's device, `pipe write` is the blocking write to standard
output — so a figure copied out of a log says which one produced it. The split
answers the question that matters, whether the sink is what limits the rate, and
on the development machine it is not, by an order of magnitude.

**Measured, on one machine, once** (RTX 3080 Laptop, 1280x720 at 60 fps, one
preset held, nothing else on the GPU): a **30-minute run emitted 108,000 frames
in 1800.00 s wall against 1799.99 s scene**, at 3.67-7.82 ms of render+readback
and 0.27-0.58 ms of Spout send per frame, with the resident set at a 280 MB peak
growing **2.0 MB across the whole run**. That is a reading from one box and one
driver, not a specification — a machine that cannot hold the rate reports it the
way the next paragraph describes.

**Wall clock against scene clock is the honest frame-rate reading.** They track
each other because `dt` is measured per frame rather than assumed, so a run that
cannot hold the requested rate renders in correct real time and simply delivers
fewer frames: the animation is never slow, the frame *count* is low. Compare the
frames emitted against `fps x wall` to see whether the rate was held.

### What it does not do

- **`--sink spout` is Windows only.** Spout has no macOS form; the analogue there
  is Syphon, a different SDK against a Metal/IOSurface seam. `--sink stdout` runs
  wherever the player does.
- **No audio, on either sink.** Both are video transports. A receiver takes audio
  from its own source.
- **Spout is same-machine only.** It shares GPU memory between processes on one
  box; there is nothing to send over a network. The pipe reaches whatever spawned
  the player and no further — a remote sink would be `--render`'s `ffmpeg` pipe
  pointed at SRT or RTSP, which is not built.
- **No golden covers the picture.** The mode is wall-clock paced, so its output
  is not reproducible and no baseline can assert on it. What *is* asserted, from
  outside the process, is the pipe's **shape**: a bounded run puts exactly one
  frame's bytes on stdout per frame, the geometry is announced before them, and a
  reader stalled for a second loses none. Whether the picture is right is still a
  byte-identity claim against a deterministic capture, or a human looking at a
  receiver.
