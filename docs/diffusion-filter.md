# The diffusion filter

`tools/sd-filter/` is a stage that sits between `shot --render` and `ffmpeg`.
Attractors and mandalas go in; an **img2img pass with ControlNet holding their
geometry** turns them into material — a canyon, a cathedral rose window, a
creature — while the shape keeps tracking the music, because the control map is
redrawn from every frame.

**This page is the only one.** Profiles, `--size`, `--stride`, the cost table and
the check are stated here and nowhere else; [`docs/capturing.md`](capturing.md),
[`tools/sd-filter/README.md`](../tools/sd-filter/README.md) and the top-level
[`README.md`](../README.md) are pointers that carry no figure of their own, and a
gate holds them to that
([ADR-0122](adrs/0122-a-sidecar-tool-documents-itself-in-one-place.md)). A
followup adds a section here rather than a fourth copy elsewhere.

**Nothing here ships.** No model, no weights and no Python runtime are in the
release zip; `lmv.exe` and `foo_lmv.dll` do not change size, and
[NFR §4](nfr.md#4-size-and-dependencies)'s size budget is untouched. The
repository carries a script and a `requirements.txt` — that is the whole of it.
This is creator tooling you build yourself from a source checkout.

## What you need

- **A CUDA GPU.** The filter refuses to start without one rather than falling back
  to a CPU path that would take a week. It was built and measured on an **RTX 3080
  Laptop, 8 GB**, which is comfortable for SD1.5-class inference with ControlNet at
  fp16 and too tight for SDXL plus ControlNet.
- **Python 3.10+.** 3.12 is what the measurements were taken on. 3.9 is at the
  floor of what current `torch`/`diffusers` target; if resolution goes badly, a
  newer virtual environment is the fix rather than pinning old wheels.
- **A first-run download of several gigabytes from Hugging Face** — the base model,
  the ControlNet, the LCM-LoRA and the HED annotator. They are cached in
  `~/.cache/huggingface` and downloaded once, not per render.
- **`ffmpeg`**, the same one `shot --render` already needs.

## Setup

```bash
python -m venv .venv
.venv/Scripts/python -m pip install -r tools/sd-filter/requirements.txt   # Windows
```

**Do not install `diffusers` and friends on top of an existing CUDA `torch`
without pinning it.** `controlnet_aux` declares a bare `torch`, and a plain
`pip install` will happily replace a working `2.6.0+cu124` with a CPU build from
PyPI — after which the run dies at the first frame, having first spent thirteen
minutes downloading weights. `requirements.txt` pins the `+cu124` build with its
index URL for exactly this reason, and pins `torchvision` alongside it because the
two are version-locked. The comments in that file record the failure in full.

Check the environment before spending a render on it:

```bash
.venv/Scripts/python -c "import torch; print(torch.__version__, torch.cuda.is_available())"
```

`2.6.0+cu124 True` is the answer. A bare version with `False` is the CPU trap.

## The one command

There is deliberately only one, and it is the canonical `--ffmpeg` invocation from
[`docs/capturing.md`](capturing.md#the-one-canonical-ffmpeg-invocation) with a
stage spliced into the middle — the encoder half is unchanged, character for
character:

```bash
cargo run -p standalone --release --example shot -- \
    --preset "Supernova" --render track.wav --fps 30 --size 1920x1080 --tier rich \
  | .venv/Scripts/python tools/sd-filter/sd_filter.py \
      --profile quality --prompt "a vast canyon of luminous glowing rock strata" \
  | ffmpeg -hide_banner -nostats -y -f yuv4mpegpipe -i pipe:0 -i track.wav \
      -map 0:v:0 -map 1:a:0 \
      -c:v libx264 -preset medium -crf 18 \
      -pix_fmt yuv420p -color_range pc -colorspace bt709 -color_primaries bt709 \
      -color_trc bt709 \
      -c:a aac -b:a 192k -shortest track.mp4
```

`--ffmpeg` is **not** used on this path: it spawns the encoder itself, which
leaves no seam to insert a stage into. Composing the pipe by hand is the whole
point of the raw-stream path.

**That command does not change when the filter's settings do**, and that is the
property worth remembering rather than the text of the command. Because
`--stride N` preserves the frame count, the encoder never learns a rate that has
to agree with a flag on another process — there is no `-r` to keep in sync and no
way to desynchronize the audio silently. Change the profile, the stride or the
prompt and the third line of the pipe is still the third line of the pipe.

## Profiles

A profile is a named set of the flags below and nothing else — never a second
surface with its own behaviour. **Any flag passed explicitly overrides the
profile**, and the expansion is echoed on stderr at the start of every run, so a
render is reproducible from what actually ran rather than from a profile name
whose meaning may since have moved. Pass the echoed flags back without
`--profile` and you get the same bytes on the same machine.

| | `--profile quality` | `--profile fast` |
|---|---|---|
| `--size` (pixel budget) | `589824` → 1024x576 at 16:9 | `262144` → 680x384 at 16:9 |
| `--scheduler` / `--steps` / `--cfg` | `lcm` / `8` / `2.0` | `lcm` / `8` / `2.0` |
| `--strength` / `--cn-scale` | `0.75` / `0.6` | `0.75` / `0.6` |
| `--feedback` | `0.6` | `0.4` |
| `--stride` | `1` | `3` |
| `--control` / `--model` | `softedge` / `Lykon/dreamshaper-8` | same |

Both are the same cell. They differ in what they spend and how much they persist,
not in what they draw.

**`--prompt` is required and no profile supplies one**, because the image is the
whole signal and there is no default worth having. With no prompt the filter exits
**2 before importing torch** — a missing prompt costs a second, not a
multi-gigabyte weight download.

## What it costs

<!-- figures:begin -->

Measured on the dev box — **RTX 3080 Laptop 8 GB, torch 2.6.0+cu124, Python 3.12,
Windows** — rendering `attractor_leviathan` at 1920x1080 in and out, on an
otherwise idle GPU. These are measurements naming their configuration, **not
portable figures**
([ADR-0071](adrs/0071-a-numeric-test-contract-states-a-property-or-names-its-machine.md)):

| profile | diffusion geometry | per diffused frame | per emitted frame | peak VRAM | a 4-minute track at 30 fps (7,200 frames) |
|---|---|---|---|---|---|
| `quality` | 1024x576 | 7.499 s | 7.781 s | 4.88 GiB | **~15.5 hours** |
| `fast` | 680x384 | 1.291 s | 0.693 s | 3.81 GiB | **~1.4 hours** |

**The two columns measure different things, and the difference is the point.**
*Per diffused frame* is the diffusion call alone. *Per emitted frame* is the wall
clock across the whole stream divided by the frames that came out, so it also
carries the colour decode, the downscale in, the upscale back to the stream's
geometry, the colour encode out, and the gap crossfades. Those run per **emitted**
frame, so at `fast`'s stride of 3 a single diffused frame pays three of them.
Dividing the first column by the stride does **not** give the second, and a figure
derived that way understates `fast` by 1.61x.

`fast` is not a different model: it is the same cell at 44 % of the pixels with
`--stride 3`, and the stride is where most of the saving comes from.

Each run prints both figures when it finishes, so nobody has to trust this table
for their own machine.

<!-- figures:end -->

## Flags

```
--profile quality|fast     a known-good combination of everything below
--prompt STR               required; what the render should become
--negative STR             default: text, watermark, blurry, low quality, frame, border
--size BUDGET|WxH          a pixel budget spent at the stream's aspect, or an explicit size
--strength F               how far from the render the image is allowed to travel (0..1]
--cn-scale F               how hard ControlNet pins it back to the geometry
--feedback F               how much of the previous output is carried into this one [0..1)
--stride N                 diffuse every Nth frame; N are still emitted
--gap blend|held           how the gap between diffused frames is filled
--steps N   --cfg F        sampler steps and classifier-free guidance
--scheduler unipc|lcm      the sampler; lcm also fuses --lcm-lora
--control canny|softedge|lineart      which control map is drawn from the render
--model HF-ID   --controlnet HF-ID    the weights, if you want different ones
--seed N                   fixed for the whole render, so motion comes from the render
--passthrough              no model, no GPU: emit the stream unchanged
```

### The three that decide what you get

- **`--strength`** is the reimagining dial. Below ~0.6 the output is the render
  with a tint; **0.75** is where material appears.
- **`--cn-scale`** is the leash. At `1.0` the model returns the attractor back to
  you, tinted — dead at every strength tried. Reimagining lives near **0.6**.
- **`--feedback`** is the coherence dial and it is the whole difference between a
  picture and a seething one. At `0.0` successive frames re-roll the material
  completely; at **0.4** they share their veins while the geometry moves; at `0.6`
  the output persists rather than chases. It also makes each frame *more*
  expensive, so it is a quality lever and never a speed one.

### `--size` is a budget, not a side length

The filter derives the diffusion geometry from the budget and the stream's own
header, rounds each axis to a multiple of 8, and **never squashes or letterboxes**.
At an identical pixel count the native arm was both the cheapest and the only one
delivering every pixel it paid for
([ADR-0121](adrs/0121-the-diffusion-filter-is-an-offline-stage-with-profiles-and-it-interpolates-its-own-stride.md));
a `--size WxH` that disagrees with the stream's aspect is therefore an error rather
than a silent squash.

The output is always the stream's own geometry, so a 1080p render stays 1080p. If
you want no resampling at all, render at the profile's own size — `shot --size
1024x576` for `quality` — and the filter will say so on stderr.

The geometry is read off the stream and never assumed, which is the reason the
header is self-describing: the same stage handles 320x180 and 1920x1080 with no
flag, and a truncated frame is an error rather than a short write.

### `--stride` keeps the frame count

`--stride N` consumes N frames, diffuses one, and **emits N**. Frames in equals
frames out, always, so the `ffmpeg` command downstream never learns a new rate and
no A/V desynchronization is representable — that failure would be silent in the
finished file. `--gap blend` crossfades between the diffused frames on either side
of a gap (the default); `--gap held` repeats the last one, for a deliberately
stepped 30/N look.

Its ceiling is musical rather than computational: at ~118 bpm a beat is ~15 frames
at 30 fps, so by N=8 there are under two diffused frames per beat and the geometry
stops tracking the music before the picture stops looking smooth.

## The check

```bash
python tools/sd-filter/test_sd_filter.py
```

**`--passthrough` is the stage with the model taken out** — it parses the header,
walks the `FRAME` chunks and re-emits them unchanged, with no weights and no GPU.
That is not a leftover: it is the only part of this feature whose output is
bit-exact and reproducible across machines, so it is the only part that can be a
real gate. Everything the model touches is not, which is why **no diffused frame
may ever enter `core/tests/golden/`**.

**215 checks**, standard library plus `numpy`, no GPU and no weights. It asserts
the pass-through round-trip at four geometries and four colour spaces, that an
unmodelled header or `FRAME` tag survives verbatim, that a malformed stream fails
loudly rather than emitting garbage, that the pixel budget lands within 0.5 % at
five aspects with both axes on a multiple of 8, that **frames in equals frames out
at every stride and both gap fillers** (with the model stubbed out, so the
accounting under test is the accounting that ships), that each profile round-trips
through its own echoed expansion, and that **this tool's YUV conversion agrees
with `standalone/src/shot/render.rs`'s** on a frozen table asserted from both
sides.

Two groups skip on their own terms, each with a printed notice rather than a quiet
pass: the colour table needs `numpy`, and the end-to-end group needs a built
`shot`. The counts are **215** with both, 196 without `numpy`, 210 without a built
`shot` — measured 2026-08-25 by running it three ways, not derived. CI runs the suite on `ubuntu-latest` and installs `numpy` so that group
runs there; `.githooks/pre-push` runs it behind a `python3` guard.

## Known sharp edges

- **Hugging Face model IDs move.** `runwayml/stable-diffusion-v1-5` already
  disappeared once. Expect to re-pin both the IDs in `sd_filter.py` and the
  versions in `requirements.txt` rather than to inherit them.
- **The first run is slow and looks stuck.** Weights download before the first
  frame is written and progress goes to stderr, not to the video.
- **`--gap blend` dissolves, it does not follow motion.** It is a crossfade, not
  optical flow. At large strides that reads as a soft double-exposure; if you want
  it crisper, lower the stride rather than reaching for an interpolator.
- **A long render wants sleep disabled.** A laptop sleeping mid-render suspends
  every stage of the pipe and they all resume on wake — but the supervising shell
  does not, and an `ffmpeg` output that was still being written has no `moov`
  atom, so `ffprobe` reports *"Invalid data found"* on a file that is merely
  unfinished. Neither is a defect; read it as "still writing", not "corrupt".
