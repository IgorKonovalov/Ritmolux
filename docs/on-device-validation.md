# On-device validation — low-end Windows iGPU smoke

> **Status:** standing / hardware-gated — **does not block plan closes.**
> **Owner:** human (the user; only runnable on the target hardware).
> **Created:** 2026-07-22 (extracted from Plan 0012 Phase 3).

This is a **checklist, not a phased plan.** It exists so that on-device checks the user can
only run "much later" — when the low-end / older Windows iGPU test box is in hand — never sit
in the plan roster reading as stalled work and never gate a plan from closing. Development and
plan-close momentum stay unblocked; these items get ticked whenever the hardware is available.

Keep it lightweight: add an item when a plan produces an iGPU-hardware check it can't run
itself, tick items when the user runs them, and route any failure back to `dev`/`architect` as
its own follow-up. When every open item is ticked and nothing new is pending, this file can be
deleted.

## Why these live here (not in a plan)

The §9 test matrix names an "older Windows PC (iGPU)" the dev box is not. Everything measured so
far is one machine, one GPU vendor (AMD). Two questions can only be answered on that other
hardware, and the user won't have access to it until later:

1. Does the **NFR §1 perf floor** (≥ 60 fps @ 1080p at the shipped single fixed tier) hold on the
   weakest box?
2. Is the **~350 MB working-set soft ceiling** (NFR §12) AMD-specific, or does a second GPU vendor
   (Intel iGPU) land somewhere different?

Neither blocks shipping — they confirm portability of a floor and a ceiling already accepted on
the dev box (ADR-0010). So they wait here.

## Reference baseline (dev box — what the low-end box is compared against)

Release build, AMD iGPU dev box, post-cull (2-scene) standalone, 1080p, steady state
(measured 2026-07-22, `diagnostics.log`, ~5.5 min run):

| Metric | Dev-box value |
|--------|---------------|
| fps | ~165 (0 dropped frames over 51k) |
| frame_ms avg / p99 | ~6.06 / ~6.9 ms |
| working set (`rss_bytes`) | ~303 MB (run-to-run noisy; private commit ~338 MB is the stable figure) |
| gpu_bytes | ~16.6 MB |

The low-end box need not match these — it need only clear the **≥ 60 fps** floor and report *its*
footprint so the vendor spread is on record.

## Checklist

- [ ] **Low-end / older Windows iGPU box (§9), 1080p.** Run the current release standalone, let
      it reach steady state, capture `diagnostics.log`. Report **(a)** fps holds ≥ 60 @ 1080p, and
      **(b)** steady-state working set + private commit. _(This is Plan 0012 Phase 3, extracted; it
      also satisfies the identical Plan 0003 Phase 3 iGPU-60-fps carry-forward — same measurement.)_
- [ ] **Second GPU vendor — Intel iGPU, if a box is available**, 1080p. Same capture. The point is
      the footprint spread vs the AMD dev box — confirms whether the ~350 MB ceiling is AMD-specific.
- [ ] **The dual-live dissolve budget, on the low-end box.** Plan 0023 made every preset switch a
      ~1 s dissolve, and its governor re-renders the *outgoing* preset live as well whenever the two
      presets hold independent GPU state **and** the smoothed frame time is under
      `DUAL_LIVE_BUDGET_MS` (`core/src/render/mod.rs`, currently **18.0** — the code calls it "the
      number to calibrate on a low-end rig", and no capture can speak to it because a headless run
      collects no frame times and so never upgrades). Two things to report, both with the overlay on
      (`F3`) so the p99 is visible: **(a)** press `Space` repeatedly between light presets — does the
      frame time hold ≥ 60 fps *through* the dissolves, or does the governor's threshold need to come
      down; **(b)** the **heavy pair** — dissolve an attractor preset into a reaction-diffusion one
      and back. That pair is the freeze fallback's reason for existing; on this box it should either
      stay frozen (fine) or, if it upgrades, still hold the floor. _(Plan 0023 Phase 4's done-when
      and its budget-tuning risk, extracted at that plan's close.)_
- [ ] **Trails at native resolution, on the low-end box, 1080p.** Plan 0033 made the two post
      stages size their internal grid from the render target instead of a fixed 1280x720
      (ADR-0034). With `trails` active that is now a **full-resolution `Rgba16Float` ping-pong
      read *and* write every frame** where it used to be a 720p one — roughly 2.25x the feedback
      bandwidth at 1080p. NFR §1's ≥ 60 fps @ 1080p floor is exactly the claim at risk, and no
      headless capture can speak to it: WARP timings say nothing about an iGPU's memory bandwidth.
      Load **`rose_trails`** — it binds `trails` around 0.78 and is the shipped preset that exercises
      this path (`rose_kaleidoscope` and `fragment_kaleido` cover the fold). Let it settle with the
      overlay on (`F3`), and report **(a)** whether fps holds ≥ 60 and **(b)** the p99 against the
      same preset with `trails = 0`. _(Plan 0033's stated main exposure. If it fails, lower the cap
      constant in `core/src/render/post.rs` — do **not** re-fix the grids.)_
- [ ] **Working-set delta from the target-sized post stages, including mid-dissolve.** The same
      change grows the composite's GPU memory from ~22 MB per chain to ~50 MB at the cap, and a
      dual-live dissolve holds **two** chains, so the transient peak is ~100 MB against NFR §12's
      ~350 MB soft ceiling — which is mostly driver floor already. **Those figures are arithmetic
      from the texture descriptors, not a measurement.** On the low-end box, report the steady-state
      working set with a `trails` preset active, and again *while holding down* preset switches so a
      dissolve is live, against the same numbers with `trails = 0`. **Read `rss_bytes`, not
      `gpu_bytes`** — the latter is a swapchain-only approximation (ADR-0008) that does not count the
      post stages' offscreens, so it reads identically either way. Measured on the dev box after this
      landed: `gpu_bytes` unchanged at 16,588,800 (= 1920x1080 x 4 B x 2 — the swapchain exactly),
      and `rss_bytes` up only ~3 MB, because that box renders on a **discrete** GPU where the
      textures sit in VRAM and never enter the working set. That is exactly why this item needs the
      iGPU, where GPU memory *is* system memory. _(Plan 0033 Risks: "memory is a projection, not a
      measurement". Same mitigation as above — the cap is one constant.)_
- [ ] **The reaction-diffusion present's reconstruction cost, on the low-end box, 1080p.** Plan 0033
      replaced the RD present's field sampling with a Catmull-Rom reconstruction to get the coral
      look. The present pass now calls `sample_v` **five** times per fragment, each at **nine**
      bilinear taps — roughly **45 texture fetches per fragment**, over the whole screen, every
      frame. It shipped **unmeasured on hardware**: the WARP figure first reported was retracted as
      run-to-run noise (the same suite timed 193.6 / 224.2 / 105.2 s across three runs), and a
      software rasterizer says nothing about an iGPU's texture-unit throughput anyway. Load
      **`reaction_reef`** (or any `reaction_*` preset), let it settle with the overlay on (`F3`), and
      report **(a)** whether fps holds ≥ 60 @ 1080p and **(b)** the p99. **If it fails, that is a
      user call, not an automatic revert.** The reconstruction is one function in the RD present
      shader and reverting it is cheap, but it costs the coral look outright — so route a failure to
      `architect` with the number, and let the look/perf tradeoff be decided rather than assumed.
      _(Plan 0033 shipped this and never measured it; extracted at Plan 0035's Phase 4.)_
- [ ] **Frame-time p99 with the debug overlay on, any box.** Plan 0030 put the three post stages
      behind a `PostStage` trait, so a rendered frame now costs ~4 vtable calls plus ~4 `TextureView`
      Arc bumps it did not before. Expected to be unmeasurable against a render pass, but it was
      **never measured** — the check needs a live window, so it could not run at that plan's close.
      Run the standalone with the overlay on, let it settle, and report whether p99 moved.
      _(Plan 0030's dynamic-dispatch risk bullet, extracted.)_

## How to run

From the repo root on the target box:

```
cargo build -p standalone --release --bin lmv
./target/release/lmv.exe
```

Play any audio (loopback capture feeds the visuals). Then, in the window:

- **`Space`** — cycle presets (step through the whole embedded set; each should render and react).
  Each switch **dissolves** over ~1 s rather than cutting, so the frame time during the dissolve is
  its own measurement — see the dual-live budget item above.
- **`F3`** — toggle the diagnostics overlay (frame-time sparkline + GPU bar + fps/p99 readout).

The 1 Hz log lands at:

```
%APPDATA%\light-music-visualizer\diagnostics.log
```

Columns: `unix_ms  fps  frame_ms_avg  frame_ms_p99  frames_total  frames_dropped  gpu_bytes  rss_bytes`.
`rss_bytes` is the working set. For private commit too, run the throwaway floor spike or read
`PrivateMemorySize64` via `Get-Process lmv` (the ADR-0010 method).

## Pass criteria & escalation

- **Pass:** fps ≥ 60 @ 1080p (NFR §1 floor holds) and a recorded working-set / private-commit
  figure for the box.
- **Fps below 60** → a §1 floor regression on the weakest box → route to `dev`/`architect` as a
  new follow-up (this is the trigger the adaptive-quality-tier plan waits on).
- **A wildly different vendor footprint** (e.g. Intel far above or below the AMD ~350 MB ceiling)
  → route to `architect` to widen the NFR §12 soft ceiling from one-vendor to a measured spread.

## Provenance

Extracted from Plan 0012 Phase 3 at that plan's close (2026-07-22) so Plan 0012 could close on its
two completed `dev` phases (scene cull + driver-floor spike) without waiting on hardware. See
`docs/plans/done/0012-memory-floor-measure-and-scene-cull.md` and
[ADR-0010](adrs/0010-accept-gpu-driver-memory-floor.md).
