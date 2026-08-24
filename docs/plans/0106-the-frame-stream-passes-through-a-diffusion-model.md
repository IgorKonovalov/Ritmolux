# 0106 — The frame stream passes through a diffusion model

> **Status:** in-progress — Phases 1, 2, 2b and 3 are done (2026-08-20). The stop condition did not
> fire; **Phase 2b chose `native`**, so the filter diffuses at the stream's own aspect and every
> square s/frame reading in this document is superseded. **Phases 4-6 remain**, and Phase 4 is
> unblocked
> **Created:** 2026-08-16
> **Owner skill(s):** dev, human
> **Related ADRs:** [0121](../adrs/0121-the-diffusion-filter-is-an-offline-stage-with-profiles-and-it-interpolates-its-own-stride.md)
> — the deferred ADR, written 2026-08-20 between Phases 2 and 3 against the spike's evidence exactly
> as Phase 2 said it would be
> **Hard dependency:** [0101](done/0101-the-engine-renders-a-music-video.md) Phases 1–2, for every
> phase except the spike. *(The transitive dependency on
> [0099](done/0099-the-horizon-reaches-its-own-length.md) for renders past ~2 minutes is
> **discharged** — it closed 2026-08-16 and the long-run path now completes.)*

## TL;DR

The offline render stream gains an optional **stdin→stdout diffusion filter**: attractors and
mandalas go in, and an img2img pass with ControlNet holding their geometry turns them into
material — a canyon, a cathedral rose window, a creature — while the shape keeps tracking the
music. The filter is a **Python script in `tools/`**, not a bundled runtime, so `lmv.exe` and the
release zip do not change at all. **Phase 1 is a throwaway spike and Phase 2 is a stop condition**:
nobody has yet seen what this engine's output looks like through a diffusion pass, and if it boils,
the plan ends there having cost an afternoon.

## Context & problem

The user asked for TouchDesigner-plus-TouchDiffusion: take this engine's abstract output and let a
diffusion model reimagine it. The architecture turned out to be nearly free, because
[Plan 0101](done/0101-the-engine-renders-a-music-video.md) /
[ADR-0114](../adrs/0114-the-engine-renders-video-offline-and-delegates-encoding.md) already build
the pipe it needs: `shot --render clip.wav` walks a WAV at fixed injected `dt` and streams
self-describing frames to stdout for the user's own `ffmpeg`. **A diffusion stage is a filter
dropped in the middle of an existing pipe, and costs zero new Rust.**

So the architecture is not the risk. The picture is. Frame-independent img2img on moving abstract
content is notorious for **boiling** — a seething per-frame reinterpretation that reads as noise
rather than as a scene — and no amount of design removes that uncertainty. Two further unknowns
ride with it, and both are specific to this content rather than general:

- **An attractor is not a photograph.** It is thin bright filaments on dark ground. Canny edge
  detection on a filament produces a *double* edge, one line per side, so the model is conditioned
  on tubes where the render drew strands.
- **A mandala is radially symmetric, and diffusion is famously bad at preserving symmetry.**
  `star_rosewindow` either survives that or it does not, and the answer is not predictable from
  first principles.

The environment is measured rather than assumed (dev box, 2026-08-16): **RTX 3080 Laptop, 8 GB
VRAM**, driver 581.42, **Python 3.9.13**, **`ffmpeg` 8.1** already installed. That is comfortably
enough for SD1.5-class inference with ControlNet at fp16 (roughly 4–5 GB) and too tight for SDXL
plus ControlNet (roughly 7.5 GB) without offloading that would cost throughput over a
thousand-frame loop.

## Decision

The diffusion stage is an **out-of-process stdio filter** speaking Plan 0101's own frame format.
The repository ships a **script and a `requirements.txt`; no model, no weights, and no Python
runtime**, so [NFR §4](../nfr.md#4-size-and-dependencies)'s ~10 MB soft cap is untouched and the
release artifact does not change. The AI stage **reimagines** rather than restyles — high denoise
with ControlNet holding the geometry — and **no audio data crosses the seam**: the image is the
whole signal.

We rejected **in-process Rust inference** (immature wgpu-native diffusion, no TensorRT path, and a
very large dependency against "lightweight is a feature"); **`shot` spawning and owning the
sidecar** (a three-process chain gives a broken pipe three candidate culprits, for ergonomics that
are one flag and a spawn to add later — it is a followup, not a rewrite); **the diffused frame
re-entering the renderer as a sampled texture** (genuinely the most interesting version, and it
needs a new `core` capability and its own ADR — it is the headline followup); and **publishing
frames to TouchDesigner over Spout** (gives away ownership of the loop, which the user explicitly
did not want).

## Architecture diagram

```mermaid
flowchart LR
    wav["track.wav"] --> shot

    subgraph repo["this repository"]
        shot["shot --render<br/>Plan 0101: fixed dt, Rich tier"]
        filt["tools/sd-filter/<br/>script + requirements.txt"]
    end

    subgraph ext["user-supplied, never in the release zip"]
        torch["torch + diffusers<br/>SD1.5 + ControlNet + weights"]
        ff["ffmpeg 8.1"]
    end

    shot -->|frame stream on stdout| filt
    filt <--> torch
    filt -->|frame stream on stdout| ff
    wav --> ff
    ff --> mp4["out.mp4"]
```

**`core/` does not appear in that diagram, and that is the point.** Nothing in this plan touches
the core, the C ABI, or `lmv.exe`.

## Implementation phases

### Phase 1 — the spike renders

- **Owner skill:** dev
- **What:** Produce the artifacts Phase 2 judges: a stills sweep and two or three short motion
  clips of a diffused attractor and a diffused mandala.
- **Files touched:** **none in the repository.** Everything lives under an untracked `spike/`.
  Note the `block-broad-git-add` hook will refuse a broad stage; do not add `spike/` to git.
- **Notes for the implementer:**

  **Frames come from tooling that already exists.** Plan 0101 is not built, so there is no stream
  yet — but `shot --frame-at <hop>` already writes one full-size frame under real audio *after the
  tonemap*, which is exactly one frame of what 0101 will stream. Loop it:

  ```powershell
  cargo run -p standalone --release --example shot -- `
    --preset-file presets/attractor_leviathan.toml --signal dynamic:110 `
    --frame-at $hop --size 768x768 --tier rich --out ("spike/frames/f{0:d4}.png" -f $i)
  ```

  Sixty frames at every third hop. The arithmetic: a hop is 512 samples at 48 kHz = 10.667 ms, so
  93.75 hops/s, so every third hop is **31.25 fps** and sixty frames is **1.92 s** — ample to see
  boiling. That quantization is precisely what 0101 Phase 1 removes; its note about the analysis
  hop and the frame rate being different clocks is this. `--signal dynamic:110` needs no asset and
  is the only synthesized kind with real dynamics. `--release` is not optional: this launches sixty
  processes and the cost is dominated by startup and shader compilation, not rendering — **expect
  10–15 minutes**, all of it an artifact of 0101 not existing yet.

  **Three subjects, because they fail differently:** `attractor_leviathan` (dense filaments, the
  hard ControlNet case), `star_rosewindow` (the radial-symmetry question), `attractor_ink`
  (black-on-white, inverted tonality, the easy control).

  **Models — SD1.5 class, not SDXL**, per the VRAM arithmetic above. `Lykon/dreamshaper-8` as the
  base: for reimagining into a scene a finetune is markedly better than stock SD1.5, which is the
  wrong default here. `lllyasviel/control_v11p_sd15_canny` to start, with `..._softedge` /
  `..._lineart` as the expected fallback for the double-edge problem named in Context.
  `StableDiffusionControlNetImg2ImgPipeline` is img2img and ControlNet in one call.

  **The coherence recipe is the design content of this phase.** Four lines, all load-bearing:

  ```python
  # illustrative — the algorithm, not the script
  for i, render in enumerate(frames):
      control = canny(render)                                            # STRUCTURE: this frame
      base    = render if i == 0 else lerp(render, prev_out, FEEDBACK)   # APPEARANCE: carried
      out     = pipe(prompt=PROMPT, image=base, control_image=control,
                     strength=STRENGTH, controlnet_conditioning_scale=CN,
                     num_inference_steps=STEPS,
                     generator=torch.Generator("cuda").manual_seed(SEED)).images[0]
      prev_out = out
  ```

  The control image comes from **this frame's render** and the img2img base carries the
  **previous output** — that split is the whole trick: geometry tracks the music frame by frame,
  material persists across frames. Taking the control from the previous output instead lets the
  shape drift off the audio. `SEED` is fixed for the entire render; a per-frame seed guarantees
  boiling whatever else is tuned.

  **Two passes, not twelve clips.** Stills cannot answer the boiling question and motion is too
  slow to sweep. Pass 1 (~1 minute): one mid-clip frame across `strength ∈ {0.45, 0.60, 0.75}` ×
  `cn_scale ∈ {0.6, 1.0}` × `control ∈ {canny, softedge}`, as one contact sheet. Pass 2
  (~5 minutes): all sixty frames through the two or three surviving cells, plus `FEEDBACK` at 0.0
  against 0.4 for the best one, assembled with `ffmpeg -r 31.25`.

  **Traps, each of which reads as "the model is bad" and is not:** in img2img the actual steps run
  are `steps × strength`, so `steps=4, strength=0.5` gives **two** steps and mud — use `steps=8`
  under LCM or `steps=20` without it. SD1.5 at 768² can duplicate or mirror content, which is its
  native-resolution artifact; drop to 512² rather than changing pipeline. Do not
  `enable_model_cpu_offload()` at 8 GB with SD1.5 — correct output, ruinous throughput over a
  thousand frames.

- **Done when:** a contact sheet and at least two motion clips exist, together with four measured
  numbers: **seconds per frame** at the chosen cell, **peak VRAM** during the run, the cell itself
  (strength, `cn_scale`, feedback, control type, prompt, steps, base model), and whether the
  radial symmetry of `star_rosewindow` survived. The timing figures quoted during the interview —
  roughly 0.1 s/frame for SD-Turbo at 512², 0.3 s/frame for SD1.5+LCM at 768² — are **rough
  estimates and unverified**; this phase replaces them with measurements, and every later phase's
  arithmetic is built on what it measures.

### Phase 2 — the look gate (STOP CONDITION)

- **Owner skill:** human
- **What:** Watch the clips and decide whether this is worth building.
- **Done when:** the user answers three questions. **Does it boil** — usable, usable only with
  feedback, or unusable? **Does the music still read** — after diffusion, does the visual still
  land on the beat, or has the AI stage flattened the dynamics into uniform busyness? And **is
  this something they would publish**?

  **If the answer to the first is "unusable" across every cell, the plan ends here** with a
  written diagnosis and nothing built. That is a good outcome for one afternoon, and it is the
  reason the spike precedes the architecture rather than sitting inside it.

  The second question is the one that is easy to forget and is the whole point of the application.
  A "no" there does not kill the plan; it **reopens the no-audio-conditioning decision**, and the
  repair would be denoise strength driven by the onset envelope — a protocol change carrying real
  data across the seam, not a tuning change. Phases 3–5 would need re-scoping before they start.

  **The ADR is written between this phase and the next**, by the architect, in a fresh session.
  The rejected alternatives are already enumerated in this plan's Decision; what they are missing
  is the evidence this phase produces, and an ADR written before it would be recording a guess as
  a decision.

### Phase 2b — the aspect measurement (GATE), added 2026-08-20

- **Owner skill:** dev
- **What:** Measure what a **16:9** frame costs and looks like through the banked cell, and ask the
  user which handling ships. Throwaway spike work under Phase 1's umbrella — same `spike/`, no
  repository file, no commitment.
- **Why this phase exists.** **Every measurement in this plan is square.** The spike rendered
  `--size 768x768`; the approved look, the 1.164 s/frame, the 2.7x for 768 and every `boil` figure
  are all square. The stream this filter actually sits in is **1920x1080**, and *Data shapes*
  promises the same geometry out. Nothing in this plan or
  [ADR-0121](../adrs/0121-the-diffusion-filter-is-an-offline-stage-with-profiles-and-it-interpolates-its-own-stride.md)
  says how a 16:9 frame becomes a diffusion and comes back. That is the failure this repo has a rule
  about: **the configuration we measured at and the configuration we ship at disagree, and no
  measurement covers the disagreement.** SD1.5 is trained at 512 square and is known to degrade off
  it, so the answer is not derivable from what we have — it has to be looked at.
- **Notes for the implementer:** re-render one subject at `--size 1920x1080` (the same
  `shot --frame-at <hop>` loop Phase 1 used), then run the **same cell** — LCM 8 steps `cfg 2.0`,
  the same prompt, the same fixed seed, feedback 0.4 — three ways, ~120 frames each:
  1. **native non-square**, 1024x576;
  2. **square-then-stretch**, 768x768 diffused and stretched to 16:9 on the way out;
  3. **pad-then-crop**, letterboxed into 768x768, diffused, bars cropped off.

  Record **s/frame for each** — cost tracks pixel count, so arm 1 is not free and the existing
  timings do not transfer — and note where the model puts material in arm 3's bars. Report `boil`
  if it is cheap, but do not lean on it: it is only interpretable when the source is moving, and
  comparing across arms of **different geometry** is not the same-kind comparison
  [ADR-0074](../adrs/0074-a-ratio-against-an-in-run-control-is-not-automatically-portable.md)
  requires. **The eye is the instrument here, as it was at Phase 2.**
- **Done when:** the three clips exist, their per-frame costs are recorded in the Implementation
  log with the machine named
  ([ADR-0071](../adrs/0071-a-numeric-test-contract-states-a-property-or-names-its-machine.md)), and
  **the user has said which arm ships**, recorded as given. If the answer is anything other than
  square-then-stretch, the log gains a line saying that every s/frame figure above it was measured
  square and does not carry over.
- **What this gates, and what it does not.** It blocks **Phase 4 only**. Phase 3 is byte-identical
  pass-through with no model in it and does not care about aspect — it can land first, or
  alongside. Unlike Phase 2 this is **not a stop condition**: no outcome here ends the plan, it
  chooses between three ways of doing the same thing.

### Phase 3 — the pass-through stub

- **Owner skill:** dev
- **What:** `tools/sd-filter/` reads Plan 0101's frame stream on stdin and writes it unchanged to
  stdout, with **no model involved**.
- **Files touched:** `tools/sd-filter/` (new), `docs/capturing.md`.
- **Notes for the implementer:** **read 0101 Phase 1's stream-format note first.** It records a
  measurement against `ffmpeg` 8.1 that decides this phase's difficulty: the Y4M muxer accepts
  `yuv444p, yuv422p, yuv420p, yuv411p, gray8` and **errors on `rgb24`**, so a Y4M stream costs
  this filter a YUV round-trip, while NUT with `rawvideo` carries `rgb24` unconverted. Whichever
  0101 chose, this filter speaks it and does not invent a second format.
- **Done when:** `shot --render | sd-filter | <sink>` produces bytes **identical** to
  `shot --render | <sink>`. That is an exact property needing no tolerance, it proves the whole
  plumbing with no GPU and no weights, and it is **the one part of this feature that can be a real
  CI gate** — everything downstream of it is a model whose output is not reproducible across
  machines.

### Phase 4 — the filter does the work

- **Owner skill:** dev
- **What:** The stub grows the Phase 1 recipe at the Phase 2 cell, with strength, ControlNet
  weight, feedback blend, prompt, seed and control type as command-line configuration.
- **Files touched:** `tools/sd-filter/`, `docs/capturing.md`.
- **Notes for the implementer:** the model loads **once**, before the first frame, and the
  previous output is held across frames — this is a stateful stream filter, not a
  request/response service, and reloading per frame would dominate the runtime completely. Report
  progress on **stderr**, never stdout, which carries frames.
- **Done when:** a sixty-frame clip through the real pipe reproduces the Phase 1 result at the
  same cell, and two runs on this machine with the same seed and arguments produce the same
  bytes. **Same-machine only**, and the done-when says so on purpose: fp16 reduction order and
  cuDNN autotuning make cross-machine equality false, so per
  [ADR-0071](../adrs/0071-a-numeric-test-contract-states-a-property-or-names-its-machine.md) this is a
  measurement that names its configuration rather than a property.
- **Amended 2026-08-20 by `architect`, after the Phase 2 gate, and carrying
  [ADR-0121](../adrs/0121-the-diffusion-filter-is-an-offline-stage-with-profiles-and-it-interpolates-its-own-stride.md).**
  The gate approved a strided render, which the ADR resolves in favour of the frame-count contract.
  Three additions, and the phase stays one commit:
  - **`--stride N` emits N frames per N consumed.** The filter diffuses every Nth frame and fills
    the gap itself, so `frames in == frames out` still holds and the canonical `ffmpeg` command
    does not change. **Implement the held frame first** — repeat the diffused frame N times — because
    it needs no dependency and is a complete, shippable behaviour. Then render the same clip both
    ways and **ask the user**: the gate saw only the `minterpolate`-interpolated version and has
    never compared it against held, so which one ships is a `human` call, not `dev`'s. If
    interpolation wins, it is a new dependency and the ADR says `minterpolate` cannot be it.
  - **`--profile <name>` sets a known-good combination of the existing flags**, and any flag passed
    explicitly overrides it. The **expanded flag list is echoed on stderr** at the start of every
    run, so a render is reproducible from what ran rather than from a profile name whose meaning may
    have moved. Two profiles are enough to start: `quality` (768, the 20-step UniPC cell, feedback
    0.6) and `fast` (512, the LCM 8-step `cfg 2.0` cell, stride 3). **Those sizes are square and
    provisional** — Phase 2b decides how 16:9 is handled, and its answer sets the real values.
  - **The quality profile diffuses natively at 768 or above.** No upscaler dependency is taken; the
    2.7x per-frame cost is the accepted price, and ADR-0121's Alternative C records the cheaper
    route with its measurement intact in case render cost later becomes binding.
  - **Done when**, in addition: `--stride N` is asserted to emit exactly as many frames as it
    consumed (an exact property, no tolerance, and it extends Phase 3's byte-count gate rather than
    replacing it); a profile's echoed expansion round-trips — passing the echoed flags without
    `--profile` produces the same bytes on this machine; and the held-versus-interpolated question
    has an answer from the user recorded in this log.

### Phase 5 — one command, and the documentation

- **Owner skill:** dev
- **What:** One canonical invocation end to end, and the setup written down.
- **Files touched:** `docs/capturing.md`, `tools/sd-filter/README.md`, `README.md`.
- **Notes for the implementer:** keep **exactly one** canonical command in `docs/capturing.md`, so
  there is one thing to fix rather than a wiki of incantations — the same posture 0101 Phase 2
  takes with `--ffmpeg`. State the prerequisites honestly: a CUDA GPU, a Python environment, and a
  first-run weight download of several gigabytes from Hugging Face.
- **Done when:** a reader who has never run this can go from a clean checkout to an MP4 by
  following `docs/capturing.md`, and the page states plainly that nothing here ships in the
  release zip.
- **Amended 2026-08-20 by `architect`, carrying
  [ADR-0121](../adrs/0121-the-diffusion-filter-is-an-offline-stage-with-profiles-and-it-interpolates-its-own-stride.md).**
  Two additions, and the phase stays one commit:
  - **The canonical command does not change, and the page says so.** Because `--stride` preserves
    the frame count, the `ffmpeg` invocation carries no rate that has to agree with a flag on
    another process. That is the property worth writing down, not the command's text.
  - **Document the profiles as the thing a user types**, with the flag list each expands to, and
    state that the expansion is echoed on stderr on every run. Name the render cost honestly
    alongside them, from the measurements in the log: a 4-minute track at 30 fps is 7 200 frames,
    which is ~6.3 h at the 768 cell and ~2.1 h at `--stride 3` **on the machine those numbers were
    measured on** ([ADR-0071](../adrs/0071-a-numeric-test-contract-states-a-property-or-names-its-machine.md)),
    not a portable figure.
  - **Done when**, in addition: `docs/capturing.md` names every profile and the flags it expands
    to, and states the measured cost with its configuration attached.

### Phase 6 — a real track

- **Owner skill:** human
- **What:** Render a full track with the chosen preset and cell, and watch it.
- **Done when:** the user says whether it is publishable. *(This done-when carried a two-minute
  ceiling — `shot --horizon` dying at ~3,601 frames — which **is gone**:
  [Plan 0099](done/0099-the-horizon-reaches-its-own-length.md) closed 2026-08-16, the wall was
  memory pressure from a capture path that never polled rather than a frame count, and the fix is
  one `poll` in `step_offscreen`. Track length is no longer a bound. **The one thing to carry
  forward:** a render mode that submits its own passes outside `step_offscreen` inherits the defect
  and none of the fix.)*

## Data shapes

The filter's contract with the pipe — illustrative, and deliberately minimal:

```
stdin   <- Plan 0101's frame stream (format fixed by 0101 Phase 1)
stdout  -> the same stream format, same geometry, same frame count
stderr  -> progress and diagnostics ONLY

--prompt <str>  --negative <str>  --strength <f>  --cn-scale <f>
--feedback <f>  --seed <int>      --steps <int>   --control canny|softedge|lineart
--model <hf-id> --controlnet <hf-id> --size <px>   --stride <int>
--profile <name>
```

Frame count in equals frame count out, always. A filter that drops or duplicates a frame
desynchronizes the audio mux downstream, and that failure is silent in the file.

**`--stride N` does not weaken that line, it pays for it.** Per
[ADR-0121](../adrs/0121-the-diffusion-filter-is-an-offline-stage-with-profiles-and-it-interpolates-its-own-stride.md)
the filter consumes N frames, diffuses one, and **emits N** — filling the gap itself rather than
handing a changed rate downstream. Emitting at 30/N and teaching `ffmpeg` the new rate was the
obvious cheaper route and is ADR-0121's rejected Alternative A: it trades a loud failure for a
silent one, in a pipeline whose output is judged by eye hours after it ran.

**`--profile <name>` is a preset of the flags above, never a separate surface.** Any flag passed
explicitly overrides the profile, and the **expanded flag list is echoed on stderr**, so a render is
reproducible from what ran rather than from a name whose meaning may since have moved.

## Risks & open questions

- **The whole plan is gated on Phase 2, by construction.** This is the intended shape, not a
  weakness — but it does mean Phases 3–6 should not be estimated or scheduled until Phase 1 has
  run. **Discharged 2026-08-20:** both phases ran, the gate returned *usable with feedback* and
  *the music reads*, and the stop condition did not fire.
- **An ADR is owed and does not exist.** Deferred deliberately to after Phase 2 (see that phase).
  If Phases 3+ begin without it, that is a Mode 4 blocker. **Discharged 2026-08-20 by
  [ADR-0121](../adrs/0121-the-diffusion-filter-is-an-offline-stage-with-profiles-and-it-interpolates-its-own-stride.md)**,
  which amended Phases 4 and 5 in the process.
- **Blocked on [0101](done/0101-the-engine-renders-a-music-video.md) Phases 1–2** for everything except
  the spike. The transitive block on [0099](done/0099-the-horizon-reaches-its-own-length.md) past
  ~2 minutes is **discharged** (closed 2026-08-16). Phase 1 is takeable **today** and depends on
  neither.
- **The stream format is settled: Y4M, `C444`, `XCOLORRANGE=FULL`, full-range BT.709**
  (0101 Phase 1; `docs/capturing.md`). Y4M *errors* on `rgb24`, so this filter owes a YUV round-trip
  the spike never paid — the spike read and wrote PNG files and has never seen a pipe. That is
  Phase 3's actual work.
- **Every spike measurement is square and the stream is 16:9.** Raised 2026-08-20; **Phase 2b
  exists to close it** and blocks Phase 4. Until it runs, the s/frame ladder, the 2.7x for 768 and
  the approved look are all statements about 768x768 and none of them is known to carry to 1920x1080.
- **Weights are a multi-gigabyte first-run download from a third party.** Hugging Face model IDs
  can move or be withdrawn — `runwayml/stable-diffusion-v1-5` already did. Pin what works and
  expect to re-pin.
- **Nothing here is reproducible across machines**, so nothing here can be a golden baseline. A
  diffused frame must never enter `core/tests/golden/`; Phase 3's stub is the only gateable part.
- **Python 3.9.13 is at the floor** of what current `torch`/`diffusers` target. If resolution goes
  badly, a 3.11 virtual environment is the fix rather than pinning old wheels.
- **Contention:** none. This plan touches `tools/` and `docs/` only, and no other roster entry is
  in either. It reads 0101's output but does not edit `standalone/src/shot/`.

## What this plan does NOT do

- **Nothing ships.** No model, no weights, no Python runtime in the release zip; `lmv.exe` and
  `foo_lmv.dll` do not change size, and there is no in-app Export button.
- **No `core/` change and no C ABI change.** The core never learns that a diffusion model exists.
- **No audio conditioning.** The image is the whole signal. Phase 2's second question is the
  measurement that would reopen this, not a hedge against it.
- **No real-time or live path.** This is offline creator tooling — and as of 2026-08-20 this bullet
  is **stronger than it was written**, not weaker. It used to say near-real-time preview at 512²
  *"looks reachable on this hardware"*; the Phase 2 followup spike measured that guess and it is
  wrong. The banked cell is **1.164 s/frame at 512²** against the 0.033 s/frame locked 30 fps needs,
  so **~35x remains**, against named levers worth ~2x (TensorRT) and ~1.3x (a T2I-Adapter). Worse,
  the two things that make it look right both cost throughput: the approved look **requires**
  classifier-free guidance (a second UNet evaluation per step, and two independent `cfg 1`
  configurations agree model choice will not remove it), and coherence is bought by the feedback
  blend, which measures 1.164 / 1.349 / 1.611 s/frame as it rises. Realtime is therefore its own
  plan with its own ADR, reopening two alternatives this plan rejected —
  [ADR-0121](../adrs/0121-the-diffusion-filter-is-an-offline-stage-with-profiles-and-it-interpolates-its-own-stride.md)
  records the evidence it starts from and decides nothing about its architecture.
- **No `shot`-owned child process.** The filter is a pipe stage the user composes; promoting it to
  a `--diffuse` flag is a followup and is one flag plus a spawn, not a rewrite.
- **No timeline, cuts, or prompt automation across a track.** One prompt per render, matching
  0101's one preset per render.

## Implementation log

### Phase 1 — the spike ran, 2026-08-20

By `dev`, in the lane `WORK/lmv-plan-0106` on branch `plan-0106-diffusion-filter`, branched from
`5cf592d` at v0.75.0. **No repository file changed** except this log and the `Status` line, as the
phase specifies; every artifact lives under an untracked `spike/`.

**The subject material.** Caribou — *Odessa* (Swim, 2010), 4 s from 0:45, 48 kHz 16-bit PCM. Three
subjects at 768x768, `--tier rich`, 120 frames each: `attractor_leviathan`, `star_rosewindow`,
`attractor_ink`.

**The phase's frame-generation recipe is superseded, and the saving is the whole point of 0101.**
The plan budgeted 10–15 minutes for sixty `shot --frame-at` processes and named that cost "an
artifact of 0101 not existing yet". 0101 closed 2026-08-17, so this ran through `shot --render`
piped to `ffmpeg`: **all three subjects, 360 frames, in under 30 seconds**, one process each.
`--fps` also takes an exact rational, so the 31.25 fps hop-quantization workaround is gone —
frames were generated at a flat 30. The cost of `--render` was **not** felt in this phase at all;
it is now three orders of magnitude below the diffusion pass beside it.

**The cell that survived pass 1** — 24 cells (strength x cn_scale x control x size) on
`attractor_leviathan`, contact sheet at `spike/out/sheet_leviathan.png`, plus the same sweep on the
other two subjects:

| | |
|---|---|
| base model | `Lykon/dreamshaper-8` (**no fp16 variant published** — full weights) |
| ControlNet | `lllyasviel/control_v11p_sd15_softedge` (canny for the ink control) |
| strength | **0.75** |
| `controlnet_conditioning_scale` | **0.6** |
| feedback | **0.4** |
| steps / guidance / scheduler | 20 / 7.0 / UniPC |
| seed | 1234, fixed for the whole render |

`cn_scale = 1.0` is dead across every strength: it pins so hard to the render that the output is
the tinted attractor back. Reimagining lives at `cn 0.6`, and only `strength 0.75` produces
material rather than tint.

**The four numbers the phase owes.**

| | 512x512 | 768x768 |
|---|---|---|
| seconds per frame | **3.49–3.75** | **9.43** |
| peak VRAM reserved | **3.87 GiB** | **5.04 GiB** |

Both are on the dev box (RTX 3080 Laptop 8 GB, driver 581.42, torch 2.6.0+cu124, Python 3.12) and
are **measurements naming their configuration**, not properties (ADR-0071). The interview's
estimates — ~0.1 s/frame SD-Turbo at 512, ~0.3 s/frame SD1.5+LCM at 768 — are **out by 12x to 30x**
against this cell, which runs 20 UniPC steps with neither Turbo nor LCM. The sweep peaked at
**5.68 GiB** because it holds two ControlNets; a single-net render is the 3.87/5.04 above. A
four-minute track at 30 fps is 7 200 frames, so this cell is **7.0 hours at 512** and **18.9 hours
at 768** — that arithmetic is what Phase 2 is deciding about, and it is the strongest argument for
the LCM/Turbo followup.

**Does it boil — measured, not argued.** `spike/boil.py` computes
`mean|out[i]-out[i-1]| / mean|src[i]-src[i-1]|`, mean absolute per-pixel difference in 8-bit sRGB
over the same frame pairs with the source resized to the output's size: same statistic, same units,
one run, dimensionless (ADR-0074). Ratio ~1 means the filter moves no faster than the render it was
handed; >>1 means per-frame reinterpretation on top of it.

| run | src MAD | out MAD | boil | p90 |
|---|---|---|---|---|
| `lev512_fb0` (feedback **0.0**) | 15.06 | 21.31 | **1.41** | 1.73 |
| `lev512_fb4` (feedback **0.4**) | 15.06 | 15.54 | **1.03** | 1.25 |
| `lev768_fb4` (feedback 0.4) | 15.82 | 16.21 | **1.03** | 1.24 |
| `rose512_fb4` (feedback 0.4) | 2.32 | 12.19 | 5.26 | 10.74 |
| `ink512_fb0` (feedback 0.0) | 2.61 | 6.85 | 2.63 | 3.06 |

**Feedback is the whole coherence lever, and it is worth 1.41 -> 1.03 on one arm-to-arm comparison**
— same seed, same cell, the blend the only difference. Frames 60/61 of the 0.0 arm re-roll the
material completely (teal rock strata become yellow lightning veins); the same pair of the 0.4 arm
share their lava veins and cyan streak while the geometry moves. **Resolution buys detail, not
stability**: 768 measures the same 1.03.

**The instrument's caveat, and it must travel with the number.** `boil` is only interpretable when
the source is moving. `star_rosewindow` reads 5.26 while looking calm, because its own motion
(`src_mad` 2.32) is a sixth of the attractor's — in *absolute* terms that clip moves less per frame
(12.19) than the accepted leviathan one (15.54). A near-static preset inflates the ratio without
the picture seething. Read `out_mad` alongside it, or the number will condemn the wrong content.

**Radial symmetry survives**, which was the plan's named unknown for `star_rosewindow`. Every one of
the 24 cells keeps the 12-point star intact, and at `strength 0.75` real stained glass fills the
gaps between the points. The ink control behaved as predicted — minimal reinterpretation, paper
grain and brush character, and at 512/0.75 the model signs it with a red seal.

**Artifacts** (untracked, in the lane): `spike/out/sheet_{leviathan,rosewindow,ink}.png`, five clips
`spike/out/{lev512_fb0,lev512_fb4,lev768_fb4,rose512_fb4,ink512_fb0}.mp4` at 30 fps with the audio
muxed, one `cell.json` per run recording its full configuration, and the logs beside them.

**Two environment traps, both of which belong in Phase 5's setup documentation because they produce
a plausible-looking environment that fails at the first frame:**

- `pip install diffusers transformers accelerate controlnet_aux` **replaced `torch 2.6.0+cu124`
  with `2.13.0+cpu` from PyPI** — `controlnet_aux` declares a bare `torch` — and the run then died
  at `pipe.to("cuda")` with *"Torch not compiled with CUDA enabled"* after a 13-minute weight
  download. The `requirements.txt` this plan ships must pin torch **with its `+cuXXX` local version
  and its index URL**, installed in a step of its own, or `pip install -r` silently produces a
  CPU-only environment.
- Reinstalling torch alone then broke `torchvision` (`operator torchvision::nms does not exist`) —
  the two are version-locked, so `torchvision 0.21.0+cu124` had to be pinned to match. Pin the pair.

**What Phase 1 did NOT establish.** Whether the music still reads through the filter — that is
Phase 2's second question and it needs a human watching the clips with the audio, which is
precisely why it is a `human` phase. Nothing here touches `core/`, `tools/`, `standalone/` or the
release artifact.

### Phase 2 — the look gate passed, 2026-08-20, and it did not stop the plan

The user watched the five clips with audio. Recorded as given, because a gate's value is the
verdict rather than the summary of it:

> *"everything besides first one `ink512_fb0` are very good and interesting. What can we do to make
> it happen realtime and increase resolution? music reads very well"*

Against the phase's three questions:

1. **Does it boil?** **Usable with feedback.** The verdict tracks the measurement exactly — the four
   clips judged good are the four at `feedback 0.4`, and the one singled out as weak is
   `ink512_fb0`, the `feedback 0.0` control. That agreement is worth stating: the eye and
   `boil` picked the same arm without being shown each other.
2. **Does the music still read?** **Yes** — *"music reads very well"*. So the no-audio-conditioning
   decision **stands and is not reopened**: the image is still the whole signal, and the risk this
   question existed to catch did not materialize.
3. **Would you publish it?** **Not asked in these terms and not answered**, so it stays open. The
   ask that came back instead — realtime, higher resolution — is a stronger signal than a yes, but
   it is not the same answer and is not recorded as one.

**What the verdict costs in scope, and it is not small.** Both requests are things this plan
explicitly excludes: *"No real-time or live path"* and *"Near-real-time preview ... is a followup,
not a scope."* Realtime in particular **reopens two of the Decision's rejected alternatives** — a
filter that must sit in the live loop cannot be a stdio stage downstream of `shot --render`, so
either inference moves in-process or frames are published to another process. That is the ADR's
problem, not a phase's. The measured gap is the thing to design against: **3.6 s/frame at 512² is
0.28 fps, and locked 30 fps is ~110x away.**

**The ADR this plan owes is now owed against a wider question**, and the honest ordering is that one
cheap measurement comes first — whether the look survives a 2-4 step schedule — because every
realtime architecture is built on that answer and an ADR written before it would be recording a
guess. That spike continues below under Phase 1's umbrella: same throwaway `spike/`, no repository
file, no commitment.

### Phase 2 followup spike — the speed ladder, and where it stops, 2026-08-20

Run under Phase 1's umbrella at the user's request, after the gate asked for realtime and higher
resolution. Same throwaway `spike/`, no repository file. Its product is four findings the ADR needs,
and it did **not** reach realtime.

**Finding 1 — the look rides on guidance, not on step count, and that is the wall.** LCM-LoRA
(`latent-consistency/lcm-lora-sdv1-5`) at the schedule it is sold on — 4 steps, `cfg 1.0` — returns
a smoothed, desaturated render with no material at all. 6 steps at `cfg 1.5` recovers monochrome
rock and nothing else. **8 steps at `cfg 1.0` is still flat at every strength tried (0.75, 0.90,
1.00); the same 8 steps at `cfg 2.0` is the canyon.** So the variable that carries the material is
classifier-free guidance, which costs a second UNet evaluation per step. The distilled schedule
gives back 15 effective steps -> 6; guidance takes back the 2x it was supposed to save.

**Finding 2 — the banked speedup is 3.1x, measured over 120 frames, with the look intact.**

| cell | s/frame | boil | look |
|---|---|---|---|
| 20 steps UniPC `cfg 7.0` (the gate's cell) | 3.60 | 1.03 | rich strata, lava veins |
| **LCM 8 steps `cfg 2.0`** | **1.164** | **1.09** | **holds** |
| LCM 4 steps `cfg 1.0` | ~0.50 | not run | lost |
| LCM 8 steps `cfg 1.0` | ~0.90 | not run | lost |

Locked 30 fps at 512x512 needs 0.033 s/frame. From 1.164 that is **another ~35x**, and the
remaining named levers are bounded: TensorRT ~2x, a T2I-Adapter in place of ControlNet ~1.3x. The
gap closes only if guidance stops costing 2x — a model distilled to run at `cfg 1`, or
StreamDiffusion's residual CFG, which is engineering rather than a setting.

**Finding 1b — buying a guidance-free model does not recover the look, and that is the negative
result the realtime question turns on.** `Lykon/dreamshaper-8-lcm`, a dedicated LCM finetune loaded
with the LCM schedule and no LoRA, at 8 steps and `cfg 1.0`, produces the **same flat, materialless
picture** as the LoRA did at `cfg 1.0` — at strength 0.75, 0.90 and 1.00 alike. Two independent
`cfg 1` configurations now agree. So the 2x is not an artifact of bolting distillation onto a base
model; **the material this content needs comes from guidance itself**, and the ways out are residual
CFG (approximates guidance at ~1.1x rather than 2x), an untested turbo-class model, or accepting a
flatter look in the realtime mode than in the render mode. That last option is cheap and should not
be dismissed: the realtime path is a preview, and a preview that reads differently from the final
render is a normal thing for a creator tool, provided the difference is stated rather than
discovered.

**Finding 3 — "it changes too fast" is a separate axis from boiling, and feedback is its brake.**
The gate's follow-up complaint was rate, not incoherence. Measured on the LCM cell, 120 frames each:

| feedback | out MAD | boil | s/frame |
|---|---|---|---|
| 0.40 | 16.49 | 1.09 | 1.164 |
| 0.60 | 13.95 | **0.93** | 1.349 |
| 0.75 | 12.13 | **0.81** | 1.611 |

Below 1.0 the output moves *less* than the render driving it: it persists rather than chases. Note
the cost direction — **feedback makes each frame more expensive, not less**, so it is a quality
lever and never a realtime one.

**Finding 4 — temporal stride is the only rate lever that is also a speed lever, and the user
accepted its look.** Diffusing every Nth source frame and interpolating back to 30 fps was judged
*"actually fine"* at N=3 (`lcm_stride3_interp.mp4`). The saving is exact arithmetic rather than a
measurement — per-diffused-frame cost is unchanged, so stride N is N x fewer frames. N=5 and N=8
were rendered as clips; **their per-frame timings are void**, having been measured against a
concurrent GPU job, and the honest bound on N is musical rather than computational: at ~118 bpm a
beat is ~15 frames, so N=8 leaves under two diffused frames per beat and the geometry stops
tracking the music before the picture stops looking smooth.

**What the gate asked for, and the one contract it collides with.** The ask is three modes — a
quality render (feedback high, steps high, 768), and a realtime path (stride plus interpolation).
**Stride contradicts this plan's Data shapes**, which state that frame count in equals frame count
out *always*, because a filter that drops frames desynchronizes the audio mux silently. Two shapes
resolve it, and choosing is ADR work: **interpolate inside the filter** (one frame out per frame
in, contract intact, `ffmpeg` unchanged, filter owns an interpolator) or **emit at 30/N fps** (a
simpler filter, but the invariant goes and the canonical `ffmpeg` command grows a rate). The first
is recommended. Note also that the interpolation in the judged clip was `ffmpeg`'s `minterpolate`,
which is CPU-bound and **not realtime** — a live path needs GPU interpolation (RIFE class), which
is a new dependency and not a phase's decision.

**The two modes are two architectures, not two flags.** The offline one is Phases 3-5 as written,
plus a stride/interpolation surface and one amendment to the frame-count contract. The realtime one
cannot sit downstream of `shot --render`, which walks a WAV offline — it needs the live loop, and
that reopens the Decision's rejected *in-process inference* and *publish to another process*
alternatives. It should be its own plan, and this plan should not grow it.

### Phase 3 — the pass-through stub, 2026-08-20

`tools/sd-filter/sd_filter.py` reads the Y4M stream on stdin and writes it unchanged to stdout.

**It parses rather than copies, deliberately.** `shutil.copyfileobj` satisfies the done-when
byte-for-byte while proving nothing, and Phase 4 replaces the middle of this loop with a diffusion
call — so the frame boundaries are real here or they are unwritten work wearing a passing test. The
header and each `FRAME` line are re-emitted as **the exact bytes they arrived as** rather than
reserialized from parsed fields, so a stream carrying a tag this parser does not model still
round-trips exactly.

**Geometry is read off the stream and never assumed** — one stage handles 320x180 and 1920x1080
with no flag, which is what the self-describing header is for. A truncated frame, an unknown colour
tag, or garbage where `FRAME` belongs is a named error rather than a short write.

**One Windows-specific hazard is closed in code.** A stdio handle inherited in text mode turns every
`0x0A` in a frame payload into `0x0D 0x0A`, which corrupts the picture and passes silently; both
handles are pinned to binary mode, and the test payloads are seeded with `0x0A` and `0x0D` on
purpose so that failure cannot hide behind a buffer of zeroes.

**Done-when: met.** `tools/sd-filter/test_sd_filter.py` runs on the standard library with no GPU —
28 checks, all green: the round-trip at four geometries and four colour spaces, an unmodelled
header/`FRAME` tag surviving verbatim, five malformed streams failing loudly, and the end-to-end
property against real `shot --render` bytes. It **skips** the end-to-end group with a printed notice
when no built `shot` is present rather than passing quietly (ADR-0016's shape). Also smoked as the
three-stage pipe a user actually types — `shot | sd_filter | ffmpeg` — 600 frames at 320x180 to MP4.

**CI wiring was deliberately not done**: `.github/` is not in this phase's file list. The stage is
gateable, which was the phase's point; wiring the gate is architect's call.

### Phase 2b — the aspect measurement, 2026-08-20

**The plan's cost estimate for the re-render was superseded before it was written.** Phase 2b's
notes say to re-render with the `shot --frame-at` loop; Phase 1's own log had already retired that
for `shot --render`. 120 frames of `attractor_leviathan` at **1920x1080** took **16 seconds**, not
the 25–35 minutes a `--frame-at` loop would have cost.

**The three arms are pixel-count matched by construction**, which was not planned and is worth
keeping: `1024x576` and `768x768` are both **589,824** pixels. Cost differences between arms are
therefore geometry, not resolution — a same-kind comparison in
[ADR-0074](../adrs/0074-a-ratio-against-an-in-run-control-is-not-automatically-portable.md)'s sense.
All three arms are judged at 1024x576.

**The cell is Phase 2's banked one, unchanged**: `Lykon/dreamshaper-8` + LCM-LoRA, softedge
ControlNet, strength 0.75, `cn_scale` 0.6, 8 steps, `cfg 2.0`, feedback 0.4, seed 1234, the canyon
prompt. `spike/aspect.py` execs `sd.py`'s definitions rather than copying them, so "the same cell"
means the same code.

**Measured on the dev box** — RTX 3080 Laptop 8 GB, torch 2.6.0+cu124, Python 3.12, 120 frames per
arm. These are measurements naming their configuration, not properties
([ADR-0071](../adrs/0071-a-numeric-test-contract-states-a-property-or-names-its-machine.md)):

| arm | diffused at | s/frame | peak VRAM | non-upscaled pixels delivered |
|---|---|---|---|---|
| **native** | 1024x576 | **2.721** | 5.49 GiB | 589,824 (**100 %**) |
| **stretch** | 768x768 | **2.871** | 5.49 GiB | 442,368 (75 %) |
| **pad** | 768x432 band in 768x768 | **2.913** | 5.49 GiB | 331,776 (**56 %**) |

**Native is the cheapest arm and also the only one that delivers every pixel it paid for.** That was
not the expected result — the square arms were the ones the measurements existed for — and it
inverts the intuition that SD1.5 wants a square. The two square arms are *slower* despite an
identical pixel count, and both spend part of their output on an upscale: `stretch` stretches 768
columns to 1024, `pad` upscales both axes from a 768x432 band.

**The pad arm's bars stay black.** Measured on the raw 768x768 output before the crop: bar mean
**8.3 / 10.5 / 7.8** against a picture mean of 47.3 / 44.7 / 50.5, with isolated bar pixels reaching
120–140. The model does not paint material into the letterbox and nothing bleeds across the seam —
so pad's cost is the wasted 44 %, not a corrupted edge.

**A directional statistic, reported with its confound rather than as a finding.** Mean `|d/dy|` over
mean `|d/dx|`, against the source resampled to the same 1024x576: source **0.994** (isotropic),
`pad` 1.103 (+11 %), `native` 1.580 (+59 %), `stretch` 1.749 (+76 %). The ordering matches the eye —
`stretch` shows pronounced horizontal banding, `pad` the least — **but the prompt asks for "rock
strata", which are horizontal**, so this number cannot separate material the model was asked to draw
from smear the geometry introduced. It is recorded as suggestive and is not the basis of the choice.
`boil` is 1.061 / 1.020 / 1.023 (native / stretch / pad) and separates nothing.

**Artifacts:** `spike/out/aspect_{native,stretch,pad}.mp4`, a stacked `aspect_compare.mp4`, contact
sheets `aspect_sheet.png` and `aspect_f100.png`, and the raw pre-crop squares under
`spike/out/aspect_raw/`. All untracked, as Phase 1's artifacts are.

**Verdict, 2026-08-20 — `native` ships.** Recorded as given:

> *"native is the best"*

So the filter diffuses at the stream's own aspect and neither squashes nor letterboxes. That is the
arm the measurements also favour — cheapest per frame and the only one delivering every pixel it
paid for — and, unusually for this plan, the eye and the numbers agreed without either being shown
the other.

**The conditional this phase's done-when attached to any answer other than square-then-stretch, now
owed and discharged: every s/frame figure recorded above this entry was measured square and does not
carry over.** Specifically, Phase 2's banked **1.164 s/frame** is a 512x512 reading, the **2.7x for
768** is square-to-square, and the realtime gap quoted as **~35x** is computed from the 512 square
figure. The shipping geometry is 1024x576, where the same cell measures **2.721 s/frame** — 2.34x
the banked number, for 2.25x the pixels. **Nothing about the realtime conclusion changes**: the gap
was already unbridgeable by the named levers, and it widens rather than narrows. What must not
happen is a later phase quoting 1.164 as though it described a shipping render.

**Two consequences for documents this phase does not own**, both routed to `architect`:

1. **[ADR-0121](../adrs/0121-the-diffusion-filter-is-an-offline-stage-with-profiles-and-it-interpolates-its-own-stride.md)
   is still `proposed`, and its resolution clause reads "768x768 or above"** — square, which this
   measurement retires. The honest restatement is a **pixel budget at the stream's own aspect**
   (589,824 px is the measured cell; 1024x576 is that budget at 16:9), plus the aspect Negative it
   already carries being closed. The ADR itself says this answer amends the bullet before
   acceptance.
2. **Phase 4's provisional profile sizes need the same edit** — `quality` and `fast` are written as
   768 and 512, which are square. They should name a pixel budget and derive the geometry from the
   stream.


## Followups (after this lands)

- **The diffused frame re-enters the renderer** as a texture the scene samples — the attractor
  drawing over its own hallucinated past, inside the engine. The genuinely novel version, needing
  a new `core` capability and its own ADR.
- **`shot --diffuse <filter>`** — 0101's `--ffmpeg` ergonomics applied to this stage, once the
  pipeline has proved itself.
- **Near-real-time preview — measured, and it is a plan rather than a followup.** This entry
  guessed ~0.1 s/frame at 512²; the Phase 2 followup spike measured **1.164 s/frame** for the cell
  that actually looks right, leaving ~35x to locked 30 fps. It is still the thing to want, and it
  is the first item the realtime plan starts from — see *What this plan does NOT do* and ADR-0121.
- **Audio-conditioned diffusion** — denoise from the onset envelope, prompt blend on bar
  boundaries — if Phase 2 finds the dynamics flattened, or simply as the next thing to want.
- **Demo material for [0103](0103-the-project-gets-an-audience.md)**, which needs moving images
  and currently has none.
