# On-device validation — low-end Windows iGPU smoke

> **Status:** standing / mostly hardware-gated — **does not block plan closes.** (One item, the
> Plan 0044 `Rich` calibration, is runnable on the dev box today; it has its own section.)
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

1. Does the **NFR §1 perf floor** (≥ 60 fps @ 1080p at the `Floor` tier) hold on the weakest box?
   Since Plan 0044 the engine ships two tiers ([ADR-0045](adrs/0045-quality-tiers-floor-and-rich.md)),
   and `Floor` is byte-for-byte the constants that were measured here before — so every item below
   is a **`Floor`-tier** measurement unless it says otherwise. **Pin it: `lmv.exe --tier floor`.**
   Unpinned, the app starts on `Rich` and the frame-time governor may demote mid-run, which would
   put a tier change inside the very measurement being taken.
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
- [ ] **The layered heavy pair (Plan 0076), on the low-end box, 1080p.** A preset may now
      compose a second scene (`[layer]`, ADR-0090), and `Floor` renders **both** layers by
      design — a two-layer world must not lose half its content on weak hardware. The
      deliberately heavy pairing is an attractor main with a reaction-diffusion layer, at both
      joins (`under`, and `over` with a blend — the `over` join adds a surface-sized
      `Rgba16Float` offscreen pair and one blend pass). With the overlay on (`F3`), load such a
      preset — the Plan 0076 close notes carry probe TOMLs, or write one from
      `presets/README.md` — and report **(a)** whether fps holds ≥ 60 at 1080p for each join and
      **(b)** the p99 against the attractor preset alone. _(Plan 0076 Phase 4's measured numbers
      are dev-box hardware figures, not iGPU evidence; this line is where that evidence comes
      from. If it fails, the response is authoring guidance — heavy-plus-heavy pairings are an
      authoring responsibility per ADR-0090 — not a tier change.)_
- [ ] **Trails at native resolution, on the low-end box, 1080p.** Plan 0033 made the two post
      stages size their internal grid from the render target instead of a fixed 1280x720
      (ADR-0034). With `trails` active that is now a **full-resolution `Rgba16Float` ping-pong
      read *and* write every frame** where it used to be a 720p one — roughly 2.25x the feedback
      bandwidth at 1080p. **Plan 0045 raised it again:** the accumulation was always float, but the
      stage's *composited* output and the fold's source were the surface format and are now
      `Rgba16Float` too (ADR-0046), so every hand-off between stages moved from 4 to 8 bytes a
      texel. Composite bandwidth roughly doubled on top of the 2.25x. NFR §1's ≥ 60 fps @ 1080p
      floor is exactly the claim at risk, and no headless capture can speak to it: WARP timings say
      nothing about an iGPU's memory bandwidth.
      Load **`rose_trails`** — it binds `trails` around 0.78 and is the shipped preset that exercises
      this path (`fragment_kaleido` covers the fold). Let it settle with the
      overlay on (`F3`), and report **(a)** whether fps holds ≥ 60 and **(b)** the p99 against the
      same preset with `trails = 0`. _(Plan 0033's stated main exposure, widened by Plan 0045. If it
      fails, lower `TierConfig::FLOOR.post_cap` in `core/src/render/tier.rs` — the constant moved
      there in Plan 0044 — and do **not** re-fix the grids.)_
- [ ] **Working-set delta from the post stages, including mid-dissolve. Re-stated for the float
      composite (Plan 0045).** These numbers moved twice: Plan 0033 grew the composite from ~22 MB
      per chain to ~50 MB at the cap by sizing the grids from the target, and Plan 0045 took that to
      **~66 MB** by moving every intermediate to `Rgba16Float` (8 bytes a texel, not 4). A dual-live
      dissolve holds **two** chains, so the transient peak is **~133 MB**, and the worst case —
      dual-live, every stage on including bloom, ink on — is **~246 MB** against NFR §12's ~350 MB
      soft ceiling, which is mostly driver floor already. There is also a **genuinely new**
      surface-sized float buffer that exists on every frame regardless of preset: the tonemap's
      input, 16.6 MB at 1080p. The full before/after table is in [`nfr.md`](nfr.md) §12.
      **Those figures are arithmetic from the texture descriptors, not a measurement.** On the
      low-end box, report the steady-state working set with a `trails` preset active, and again
      *while holding down* preset switches so a dissolve is live, against the same numbers with
      `trails = 0`. **Read `rss_bytes`, not `gpu_bytes`** — the latter is a swapchain-only
      approximation (ADR-0008) that does not count the post stages' offscreens, so it reads
      identically either way. Measured on the dev box after Plan 0033 landed: `gpu_bytes` unchanged
      at 16,588,800 (= 1920x1080 x 4 B x 2 — the swapchain exactly), and `rss_bytes` up only ~3 MB,
      because that box renders on a **discrete** GPU where the textures sit in VRAM and never enter
      the working set. That is exactly why this item needs the iGPU, where GPU memory *is* system
      memory — and why the doubled float footprint is unmeasured rather than known-harmless.
      _(Plan 0033 Risks: "memory is a projection, not a measurement". Same mitigation as above — the
      cap is one constant.)_
- [ ] **Bloom on the low-end box, 1080p.** Plan 0045 Phase 4 added a bright-pass, a blur pyramid
      and a recombine as a third `PostStage`. It is **off by default** (`bloom_amount = 0` skips
      the stage entirely — no offscreens, no pyramid, no pipelines), so an ordinary run on most of
      the library measures none of it. **Load `star_lantern`** — it is the shipped preset built
      *for* this stage and binds all three params (`bloom_amount` around 0.95 rising on onset,
      `bloom_threshold = 1.0`, `bloom_radius` around 2.15), so no scratch `LMV_PRESET_DIR` is
      needed any more. When active the stage costs **4N passes** at `TierConfig::bloom_levels`
      (`Floor` = 4, `Rich` = 6) plus its own grid-sized `bloom-src` offscreen and the pyramid —
      **16.6 + ~11 ≈ 28 MB** at the floor cap (NFR §12). Report **(a)** whether fps holds ≥ 60 @
      1080p with bloom active on top of `trails` + the fold, and **(b)** the p99 against the same
      preset with `bloom_amount = 0`. **If it misses, the lever is
      `TierConfig::FLOOR.bloom_levels`** — a capacity, not a look, so it is a smaller decision than
      the other levers on this page, but the halo does visibly shorten.
      _(Plan 0045 Phase 6 measured the **Rich** side on the dev box's discrete GPU. Bloom is
      **not** what is expensive there: `star_lantern` runs **164 fps, p99 8.2 ms** windowed —
      comfortably inside a 60 Hz frame — against `attractor_clifford`'s p99 19.9 ms and
      `attractor_leviathan`'s 19.0 ms on the same run, neither of which binds bloom at all. So the
      cost that puts the heaviest shipped preset past a 60 Hz frame at Rich is the float composite
      plus the attractor, not this stage. **Still owed on this page:** the same `star_lantern` run
      at **native fullscreen**, the **`Floor`-pinned** sanity pass, and all of the low-end-box
      side.)_
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
- [ ] **The swarm's depth axis, on the low-end box, 1080p.** Plan 0043 Phase 3 added per-particle
      depth math and a fourth vertex attribute inside a loop that runs **10 000 times per frame**.
      Measured on the dev box over 5000 frames at 1920x1080: **1.03–1.09 ms/frame before,
      1.56–1.58 ms after — about +0.5 ms**. That is *not* fill rate (pinning the sprite scale flat
      still measures 1.60), so it is the depth math plus the wider instance, and it will not shrink
      on a slower box. Phase 3's done-when named NFR §1/§9's ≥ 60 fps @ 1080p floor as the
      acceptance criterion and **that floor was never measured** — it is defined on this box.
      Load **`swarm_dense.toml`** (the highest visible density of the three survivors, and the one
      whose `field_freq ~5.2` keeps the most particles resolving separately), let it settle with the
      overlay on (`F3`), and report **(a)** whether fps holds ≥ 60 and **(b)** the p99. **If it
      fails, the lever is `TierConfig::FLOOR.swarm_particles`** (`core/src/render/tier.rs` — the
      constant moved there in Plan 0044) — and per Plan 0043's own risk bullet that is a **look
      decision that routes back to `architect`**, not a constant to quietly lower. _(Plan 0043
      Phase 3's done-when, extracted at that plan's close.)_
- [ ] **Frame-time p99 with the debug overlay on, any box.** Plan 0030 put the three post stages
      behind a `PostStage` trait, so a rendered frame now costs ~4 vtable calls plus ~4 `TextureView`
      Arc bumps it did not before. Expected to be unmeasurable against a render pass, but it was
      **never measured** — the check needs a live window, so it could not run at that plan's close.
      Run the standalone with the overlay on, let it settle, and report whether p99 moved.
      _(Plan 0030's dynamic-dispatch risk bullet, extracted.)_

## Runnable now — the `Rich` tier calibration (Plan 0044 Phase 4)

**This one is not hardware-gated.** Every item above waits on a box the user does not have; this
waits only on the user sitting down at the machine they already own. It is here so it is not lost,
not because it is blocked.

Plan 0044 shipped `TierConfig::RICH` as **provisional multipliers, explicitly not measurements** —
the code says so in its own doc comment. Phase 4 was to replace them with measured values and was
not run at the plan's close, so the rich tier currently ships numbers nobody has timed.

- [ ] **Calibrate `Rich` on the midrange discrete GPU, native fullscreen.** Run
      `lmv.exe --tier rich` (the pin, so the governor cannot demote mid-measurement) with the
      overlay on (`F3`), across the heaviest preset of each family: an `attractor_*`, a dense line
      preset with mirror + fold (`fragment_kaleido`), `swarm_dense`, a `reaction_*`, and a
      `spectrum_*`. Report per preset **(a)** whether frame time holds the display's refresh rate
      and **(b)** the p99.
      **Escalation:** a miss is not a failure, it is the measurement — record which preset missed
      and by how much, and the specific `TierConfig::RICH` field that caused it comes down to the
      measured value. Route to `dev` with the numbers. The five fields and their provisional
      values: `post_cap` 2560x1440, `attractor_particles` 150 000, `attractor_trail_cap` 3840x2160,
      `swarm_particles` 30 000, `max_segments` 60 000 (`core/src/render/tier.rs`).
      **No number is invented upward to look good** — that was the phase's own rule, and it still
      binds. _(Plan 0044 Phase 4, carried forward at that plan's close 2026-07-30.)_
      **Two things changed under this item at [Plan 0057](plans/done/0057-the-attractors-compute-path.md)'s
      close (2026-08-03), and both make it cheaper rather than different.** It is still a frame-time
      measurement and every field above is still provisional. But `attractor_particles` is no longer
      a brightness: [ADR-0065](adrs/0065-the-attractor-deposit-is-normalized-by-particle-count.md)
      divides the additive deposit by the count, so bringing that field down now costs shot-noise
      smoothness and **not** exposure — a look consequence this escalation used to carry silently.
      And the "how does it read at `Rich`" half of the question no longer needs the running app:
      `shot --tier floor|rich` exists (it always did — Plan 0044 Phase 3 built it and only `--help`
      omitted it), so a *measurement* is a capture. Keep the app for the frame-time run and for any
      **judgement in motion**; that is what it is still better at.
      **And since [Plan 0050](plans/done/0050-in-app-settings-and-a-browse-overlay-that-fits.md) the
      frame-time run is an A/B in one sitting rather than two launches.** `[` and `]` swap the tier
      on the running renderer, and the settings menu (`S`) shows which tier is live and whether it
      is `(auto)`, `(pinned)` or `(demoted)`. So the loop per preset is: hold on the preset with the
      `F3` overlay up, read p99 at one tier, press the other bracket, let the trails re-accumulate,
      read p99 again. An in-app change **pins** the tier, so the governor cannot demote inside the
      measurement — the reason `--tier` was required for this before. It also writes
      `[quality] tier`, so remember to set it back (or pass `--tier`, which still wins at launch)
      before running anything that assumes the default. Plan 0050's own Phase 6 item 3 asks for
      exactly this measurement; whichever runs first satisfies both.

## How to run

From the repo root on the target box:

```
cargo build -p standalone --release --bin lmv
./target/release/lmv.exe --tier floor
```

**Pin the tier.** Every iGPU item above is a `Floor` measurement, and unpinned the app starts on
`Rich` with a governor that may demote partway through a run — which would land a tier change,
a full GPU-resource rebuild, and a one-frame trails blink inside the sample window. The pin also
survives a stall the governor would otherwise read as a verdict. (Use `--tier rich` for the `Rich`
calibration section instead.)

Play any audio (loopback capture feeds the visuals). Then, in the window:

- **`Space`** — cycle presets (step through the whole embedded set; each should render and react).
  Each switch **dissolves** over ~1 s rather than cutting, so the frame time during the dissolve is
  its own measurement — see the dual-live budget item above.
- **`F3`** — toggle the diagnostics overlay (frame-time sparkline + GPU bar + fps/p99 readout, and
  below them the analysis block: `BASS` / `MID` / `TREB` / `ONSET` as meters with their numbers,
  plus a `LOCK` / `FREE` row carrying the downbeat estimator's confidence — Plan 0049).

The 1 Hz log lands at:

```
%APPDATA%\light-music-visualizer\diagnostics.log
```

Columns: `unix_ms  fps  frame_ms_avg  frame_ms_p99  frames_total  frames_dropped  gpu_bytes  rss_bytes
bass  mid  treb  onset  downbeat_confidence  downbeat_locked`.
`rss_bytes` is the working set. For private commit too, run the throwaway floor spike or read
`PrivateMemorySize64` via `Get-Process lmv` (the ADR-0010 method).

The six trailing columns are the analysis snapshot (Plan 0049 / ADR-0052) — native-only, so the
plugin's `plugin-diagnostics.log` has no counterpart. `downbeat_locked` is `0`/`1`, so the
estimator's **lock rate** over a run is the mean of that column. A log written by an older build
is rotated to `.log.1` on the next launch rather than appended to, so a file never mixes row
widths.

## Pass criteria & escalation

- **Pass:** fps ≥ 60 @ 1080p (NFR §1 floor holds) and a recorded working-set / private-commit
  figure for the box.
- **Fps below 60** → a §1 floor regression on the weakest box → route to `dev`/`architect` as a
  new follow-up. The tier system that used to wait on this trigger **has landed** (Plan 0044 /
  ADR-0045), so the response is no longer "build tiers" but "lower the specific `TierConfig::FLOOR`
  value that missed" — and because `Floor` *is* the pre-tier engine, lowering one is a deliberate
  change to the floor commitment, which routes through `architect`.
- **A wildly different vendor footprint** (e.g. Intel far above or below the AMD ~350 MB ceiling)
  → route to `architect` to widen the NFR §12 soft ceiling from one-vendor to a measured spread.

## Provenance

Extracted from Plan 0012 Phase 3 at that plan's close (2026-07-22) so Plan 0012 could close on its
two completed `dev` phases (scene cull + driver-floor spike) without waiting on hardware. See
`docs/plans/done/0012-memory-floor-measure-and-scene-cull.md` and
[ADR-0010](adrs/0010-accept-gpu-driver-memory-floor.md).
