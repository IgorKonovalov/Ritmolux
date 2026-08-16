# On-device validation — low-end Windows iGPU smoke

> **Status:** standing / mostly hardware-gated — **does not block plan closes.** (Two items, the
> Plan 0044 `Rich` calibration and the Plan 0102 foobar2000 component install, are runnable on the
> dev box today; each has its own section.)
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
- [ ] **The display-write dither, on a second display and on the low-end box, 1080p fullscreen.**
      Plan 0082 made the tonemap add ±1 encoded LSB of triangular noise before the 8-bit write
      ([ADR-0096](adrs/0096-the-display-write-dithers.md)) — always on, no param, so every frame
      the engine draws carries it. Two things were settled on **one machine and one panel** and
      neither generalizes for free. **(a) Does the grain read?** The dither is a *fixed* pattern
      by design (that is what keeps every byte-equality test working), and a fixed pattern on a
      long-held still frame can resolve as texture. The 2026-08-12 verdict was "looks fine" on
      the dev box's own display; a **6-bit + FRC panel**, which runs its own temporal dither over
      ours, is the case that verdict cannot speak to. Load the reference frame
      (`LMV_PRESET_DIR=core/tests/fixtures/scratch-0082 …`, the run line is in that directory's
      README), go fullscreen, and hold it. If the grain reads as texture the answer is ADR-0096
      **Alternative F** — an animated dither, one term — and it is an `architect` call, not a
      constant to lower. **(b) Does it cost anything?** The pass gained three `pow` calls per
      pixel (one per channel, in `srgb_slope`) on a fullscreen draw. Expected to be unmeasurable,
      never measured on weak hardware. Overlay on (`F3`), report the p99.
      _(Plan 0082 Phase 5 was a `human` verdict on one display; extracted at that plan's close.)_

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

## Runnable now — the foobar2000 component's clean-profile install (Plan 0102 Phase 5)

**Also not hardware-gated**, and it is the *only* functional check the component has.
[NFR §8](nfr.md#8-distribution-v1) names this file as where the answer lives, because no CI runner
can load foobar2000 — the same gap the macOS path has
([ADR-0115](adrs/0115-the-foobar-component-is-a-released-artifact-with-a-parameterized-sdk.md),
Negative).

Two things make this a real check rather than a formality. It must run against the **released**
zip, not `plugin-foobar/build.ps1 -Install` — an artifact that has only ever been installed over its
own build directory has never exercised the path a user takes, and the release route additionally
exercises the SDK fetch, the runner's MSVC and the three-zip count. And the dev box already carries
`%APPDATA%\foobar2000-v2\user-components-x64\foo_lmv\` from that inner loop, so **remove it first**,
or an older copy shadows the one under test and the version check means nothing.

- [x] **RUN 2026-08-16 — `v0.70.0`, foobar2000 v2.25.10, AMD iGPU dev box. Installs and renders;
      one new defect, worse than either expected failure.** Taken against the published release
      zip, into a profile with the `build.ps1 -Install` copy removed first.
      **(a) Pass** — Components list reads `Light Music Visualizer 0.70.0 / foo_lmv`. This is also
      the first evidence that the component archive's layout is right: nothing outside a real
      foobar2000 can confirm that an archive with an empty root and only `x64/foo_lmv.dll` installs,
      and `build-component.ps1` asserts that layout from documentation rather than observation.
      It loaded into a host **well past** the pinned 2025-03-07 SDK, which is one data point against
      the staleness risk `packaging/foobar/sdk-pin.ps1` admits nothing guards.
      **(b) Did NOT reproduce** — the docked panel was never black. It rendered a correct attractor
      at full panel size immediately.
      **(b') NEW, and it is the finding of this run** — the panel rendered at **6.5 fps / 154 ms per
      frame from the session's first sample**, pegging one thread and starving foobar2000's own UI:
      the status bar froze at `0:00` under playing audio and the playlist painted no rows, while
      `Responding` stayed `True`. Adding an album to the playing playlist took it to **17.6 ms at
      57 fps** — 8.7x — with preset, `draw_calls` and `gpu_bytes` byte-identical across the
      transition. That is [backlog 0102](design-backlog.md)'s named stream-format revival path,
      reached accidentally, with a symptom that entry does not predict. **Filed there; priority
      raised Medium -> High.** A follow-up run with a populated playlist showed only a brief slow
      patch at the first track, then correct: the bad state runs from panel creation until playback
      starts, so it is worst for a user who looks before pressing play.
      **(e) Confirmed failing, as expected** — [backlog 0103](design-backlog.md): the panel's
      right-click shadows foobar2000's layout-edit menu, so Remove is unreachable.
      **(f) Pass** — `%APPDATA%\light-music-visualizer\` is present and shared; the component wrote
      `plugin-diagnostics.log` there during the run. **Noted, not a component defect:** that
      library held **76** presets against the **40** the repo ships, because seeding is
      write-if-absent and never deletes. 36 retired presets from earlier cohorts are still live in
      it, including pre-rename `rose_*` files. Anyone judging the shipped set from a long-lived
      profile is judging the wrong set.

- [ ] **Install the released component into a clean profile and play something.** Download
      `light-music-visualizer-v<version>-foobar2000-component.zip` from the Releases page, unzip,
      and install via File → Preferences → Components. Then, in this order:
      **(a)** the Components list shows the released version, not the dev build's;
      **(b)** dock it as a Default UI panel **before playing anything** and record whether it comes
      up black — this is [backlog 0102](design-backlog.md), which says the panel renders without
      presenting and revives only at a track boundary, and one reporter's account is all the
      evidence there is;
      **(c)** open the pop-out from View → Light Music Visualizer;
      **(d)** play a track, confirm it reacts, change track, press `Space` a few times;
      **(e)** in layout-editing mode, right-click the panel and check whether Remove is reachable —
      this is [backlog 0103](design-backlog.md), expected to fail, and confirming it on a second
      machine is worth the ten seconds;
      **(f)** `%APPDATA%\light-music-visualizer\` exists and is the same folder the standalone uses.
      **Escalation:** a failure is a new backlog entry or a followup plan, never a re-opened plan —
      on-device checks do not gate closes here. (b) and (e) failing is the *expected* result and
      confirms two filed defects rather than finding new ones; anything else is new.
      _(Plan 0102 Phase 5, carried forward at that plan's close 2026-08-16.)_

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
  plus a `LOCK` / `FREE` row carrying the downbeat estimator's confidence — Plan 0049). Under the
  panel, an `audio` line naming the **capture verdict** — `live WASAPI 48000/2`, or `failed …` with
  the platform error (Plan 0083). **The corner preset name steps aside while the overlay is up**
  (Plan 0096) — the panel composites after the text layer and used to paint straight over it — so
  where a step below asks you to record *which* preset something happened on, read it from the
  browser (`Tab`) or the window title, not from the corner.
- **Check the `audio` line before recording anything.** Four flat band meters mean either "capture
  failed" or "nothing is playing", and every reactivity judgement below is worthless if it was the
  first. The line separates them in one glance, and it is the same value the log's `capture` column
  carries, so a screenshot and a log from one run cannot disagree.

The 1 Hz log lands at:

```
%APPDATA%\light-music-visualizer\diagnostics.log
```

Columns: `unix_ms  fps  frame_ms_avg  frame_ms_p99  frames_total  frames_dropped  gpu_bytes  rss_bytes
bass  mid  treb  onset  downbeat_confidence  downbeat_locked  capture`.
`rss_bytes` is the working set. For private commit too, run the throwaway floor spike or read
`PrivateMemorySize64` via `Get-Process lmv` (the ADR-0010 method).

The six analysis columns are the analysis snapshot (Plan 0049 / ADR-0052) — native-only, so the
plugin's `plugin-diagnostics.log` has no counterpart. `downbeat_locked` is `0`/`1`, so the
estimator's **lock rate** over a run is the mean of that column. A log written by an older build
is rotated to `.log.1` on the next launch rather than appended to, so a file never mixes row
widths.

`capture` is the trailing column (Plan 0083): the startup capture verdict, repeated on every row —
`live <backend> <rate>/<channels>`, or `failed <backend> <reason>` with the platform error, or
`unsupported` on a build with no capture path. It is what makes a log of flat band columns
self-diagnosing: it says whether audio ever reached the analyzer, and if not, why. It repeats per
row rather than appearing once at startup because this file rotates at 1 MiB, and because a
capture that dies mid-run is a thing a startup line could never show.

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
