# Non-functional requirements (v1)

Agreed in the 2026-07-21 architecture interview. These are the numbers behind every
"lightweight", "real-time", and "stable frame rate" in the plans. A done-when that
contradicts this file is a plan bug — surface it, don't guess.

## 1. Performance — adaptive quality

- **Model:** the engine ships two named **quality tiers**, `Floor` and `Rich`
  ([ADR-0045](adrs/0045-quality-tiers-floor-and-rich.md)), carried as a `TierConfig` of capacity
  values (particle counts, segment budget, internal-grid caps) resolved **at renderer
  construction** and, since [ADR-0054](adrs/0054-runtime-tier-switching-rebuilds-on-the-live-context.md),
  re-resolvable on the live context by an explicit `Renderer::set_tier` (the standalone's `[` / `]`
  and its settings menu). The values are capacities read at resource-construction time, so a change
  rebuilds GPU resources and costs one visible re-accumulation of trails and feedback; nothing
  branches on the tier per frame. A tier changes *how much* the engine draws, never *what* — so the
  same preset reads the same on both, at different budgets.
- **Selection:** unpinned resolves `Rich`, and a **frame-time governor** demotes it to `Floor` on a
  sustained miss of the display's refresh budget — **once per session, one way**, reported in the
  diagnostics overlay and on stderr, never silently. There is no auto-promotion: a demotion is
  predictable and testable, where an oscillating or continuously feature-shedding design is
  neither (ADR-0045 Alternatives A/B).
- **Pinning:** `--tier floor|rich`, `LMV_TIER`, or `config.toml`'s `[quality] tier`, in that
  precedence. A pin is honoured in both directions and the governor never touches it — which is
  the escape hatch for a capable machine that a transient stall demoted. An **in-app** change also
  pins, and clears the governor's demotion latch: ADR-0045's "the latch is never cleared" narrows
  to "never cleared *by the governor*" (ADR-0054). It writes `[quality] tier`, so the launch
  precedence above is unchanged.
- **Floor:** ≥ 60 fps at 1080p on the baseline hardware (below) at the `Floor` tier, whose values
  are exactly the pre-tier engine's. The floor commitment is unchanged by tiering: the governor
  means a mispredicted rich budget degrades to a known-good state instead of stuttering.
- **Rich:** calibrated against a midrange discrete GPU (RTX 3060 / RX 6600 class) **on device**,
  not asserted from a multiplier — Plan 0044 Phase 4.
- **Background cost:** when the window is minimized or fully occluded, rendering throttles to
  near-zero GPU; DSP may keep running so visuals resume in sync.
- **Captures pin `Floor`.** Headless capture is floor-tier by construction (`Renderer::new_headless`
  cannot produce another tier, and `set_tier` is a **no-op on a surface-less context** — the guard
  ADR-0054 adds so the runtime switch cannot reopen this), so every golden baseline stays
  byte-reproducible on the WARP
  software adapter and the suite's cost does not scale with the rich tier. `Rich` is covered by
  capture-level spot checks plus the on-device checklist — a real QA gap, named rather than solved
  (ADR-0045 Consequences). See [capturing.md](capturing.md).

## 2. Platform baseline

- **Windows:** Windows 10 1903+, any DX12-capable GPU **including integrated** (~2015+ Intel/AMD iGPU).
- **macOS:** macOS 13+ (ScreenCaptureKit floor), Metal via wgpu.
- **foobar2000:** current stable release, Windows only (per ADR-0001).
- Scene code never branches on backend or OS; the baseline constrains shader features globally.

## 3. Latency — audio to visual

- **Budget: < 60 ms end-to-end** from audible beat to visible reaction (~3 frames @ 60 Hz).
- Working allocation (rough, tune in Plan 0001 Phase 3-4): capture/delivery ≤ 15 ms,
  ring-buffer read-behind ≤ 20 ms, FFT hop ≤ ~11 ms (512 samples @ 48 kHz, window ≤ 2048),
  render + present ≤ 1-2 frames.
- The ring buffer may hold more than 60 ms of *capacity*; the requirement is that the DSP
  reads near the write head, not that the buffer is small.
- **The `window ≤ 2048` bound above governs the transient path, not the whole analyzer.**
  Since [ADR-0049](adrs/0049-analysis-v2-dual-resolution-axis-normalized-bands.md) a second
  **8192** window feeds only the bands below the ~246 Hz crossover, which a 23.4 Hz bin
  cannot resolve. The 60 ms budget is unaffected because onset, beat and tempo all still
  read the 2048 window — the beat-to-reaction path never touches the long one. What the
  long window does cost is **low-band level response: ~85 ms of Hann group delay**, accepted
  as physics rather than compensated away, and applying to `bass` and the sub-crossover
  `bin()` positions only. Measured per-hop analysis cost after the change is 31.5 µs
  (from 17.2 µs), against the ~11 ms allocated here — roughly 350x headroom. Cold start
  now publishes its first frame at ~171 ms instead of ~43 ms, once per stream.

## 4. Size and dependencies

- **Soft cap ~10 MB** for the standalone release exe; plugin DLL in the same ballpark.
  wgpu is the accepted fixed cost; little else is.
- Release profile: LTO on, symbols stripped, exact-version pins for direct deps.
- Gate: any new crate pulling > ~20 transitive deps needs a stated justification (comment in
  `Cargo.toml` or, if cross-cutting, an ADR).

## 5. Real-time safety (testable restatement)

- The audio callback (WASAPI / ScreenCaptureKit / `visualisation_stream` thread) performs
  **zero heap allocation, zero locks, zero logging, zero file I/O**. Seam is the lock-free
  SPSC ring buffer.
- No panics (`unwrap`/`expect`) on per-frame audio or render paths.

## 6. Determinism

- DSP outputs (spectrum bins, onset envelope, beat estimate) are pure functions of the input
  window — no wall clock, no unseeded randomness. Visual randomness is explicitly seeded.
- The grammar's `hash(x)`/`noise(x)` are that seeded randomness (Plan 0047 /
  [ADR-0051](adrs/0051-seeded-grammar-randomness-with-per-run-opt-in.md)): pure functions of
  their argument and a per-preset salt, never of a clock. A preset may set `seed = "random"`,
  which chooses **who supplies the seed** — OS entropy, once at load, in the live app — not
  whether one exists; every capture path forces the declared number, so the harness stays a pure
  function of its inputs.

## 7. CI

- GitHub Actions from the start (right after the workspace scaffold): Windows + macOS
  runners running `cargo build`, `cargo nextest run`, `cargo test --doc`,
  `cargo clippy --all-targets -- -D warnings`, `cargo fmt --all --check` on every push.
- Plus three single-runner gates: `cargo deny check` (supply chain), Miri over `lmv-ring`'s
  `unsafe` (UB), and the coverage ratchet below.
- **Live audio** cannot run in CI. **GPU rendering partly can**: on Windows the DX12 **WARP**
  software adapter makes headless rendering deterministic, which is what the golden suite and
  the tier-4 chain test ride on. macOS has no software Metal fallback ([ADR-0016](adrs/0016-gpu-tests-opt-in-ci-scope.md)),
  so the GPU suites skip there with a printed reason. Real-GPU-vendor and live-loopback checks
  stay manual — see [`on-device-validation.md`](on-device-validation.md).
- **Coverage ratchet** ([ADR-0033](adrs/0033-testing-strategy-coverage-ratchet-and-pre-push-gate.md)):
  a Windows-only job runs `cargo llvm-cov nextest -p lmv-core --fail-under-lines $COVERAGE_FLOOR`.
  It gates **`lmv-core` only** — `standalone/` is a `winit` event loop plus two platform capture
  backends no runner can execute. The floor lives in exactly one place, the `COVERAGE_FLOOR`
  `env:` key in `ci.yml`, and is a **ratchet, not a target**: set from measurement, raised at a
  close ceremony when a plan improves coverage, lowered only with a note naming the plan and the
  reason. Measured **90.13 %** at Plan 0032's close; floor set to **88** (a 2-point margin so an
  unrelated change does not trip on rounding). A line-coverage floor is gameable by design — it
  is a backstop against silent erosion, not a quality measure. The Mode 4 review's
  "read the assertion body" step remains the actual quality gate.
- **Local pre-push gate** (opt-in, per clone): `.githooks/pre-push`, enabled with
  `git config core.hooksPath .githooks`. Runs the fast subset — a doc-link check, `fmt`, `clippy`,
  and a narrowed `nextest` — in **~28 s** warm, and prints the GPU-heavy suites it skipped.
  `cargo deny`, doctests, Miri, and coverage stay in CI. An uninstalled clone silently has no gate;
  see the README's developer section. The doc-link step (`scripts/check-doc-links.mjs`, ~50 ms) is
  the one with **no CI counterpart**, so it is gated only here until that changes.

## 8. Distribution (v1)

- **GitHub release zip**: unsigned standalone exe + a packaged `.fb2k-component` for the
  plugin. No installer, no code signing in v1 (SmartScreen warning accepted). Signing, if
  ever, is a later plan + human task.

## 9. Test hardware matrix (what the user has)

| Machine | Validates |
|---------|-----------|
| Primary Windows dev box | Standalone Windows path, plugin, day-to-day dev |
| Older Windows PC (iGPU) | The performance floor (§1) on baseline hardware (§2) |
| Mac, macOS 13+ | macOS standalone path (Metal + ScreenCaptureKit) — build/test is a human-in-the-loop step |
| foobar2000 (installed) | Plugin loading + `visualisation_stream` behavior |

## 10. Live performance (added in the 2026-07-21 follow-up interview)

The primary real-world use is **live DJ shows**: the app renders to a projector/LED screen
while a DJ mixes. This adds:

- **Session stability:** no crash, leak, or visual degradation over a ≥ 4-hour continuous
  session. A soak test becomes part of the live-features plan's done-when.
- **Inputs (all three, core stays source-agnostic):** loopback (DJ software on the same
  machine), **line-in via an audio interface** (cable from the mixer's booth/rec out — the
  robust stage setup; needs a capture-device path alongside loopback), and the foobar plugin.
- **Scene triggers, layered:** auto-rotate (MilkDrop-style timing, biased toward energy
  shifts/drops) as the baseline; **manual trigger** (hotkey, MIDI worth exploring) as the
  override; **best-effort track-change detection** (long-window spectral/tempo novelty) as an
  experimental extra — never the only mechanism, since beatmatched blends have no hard boundary.
- **Projector output is first-class:** fullscreen-on-chosen-display matters more than desk
  UX; it moves early in the roadmap.
- **Scenes are presets, not code (target state):** visualizations will be authored as
  lightweight MilkDrop-akin preset files with an optional scripting layer for staged,
  coherent per-track arcs and generative systems (walkers, flocks, 3D). Exact shape under
  exploration; the decision will land as an ADR before the preset-engine plan is drafted.
  Plan 0001's built-in Rust scenes remain the walking skeleton and later become the
  rendering vocabulary presets drive.

## 11. v1 UX scope (confirmed requirements, post-MVP plan)

All four are v1 requirements, delivered as their own plan after the Plan 0001 MVP:

- Fullscreen toggle (borderless, hotkey).
- Multi-monitor choice (pick the display to fullscreen on).
- Always-on-top / mini mode.
- Settings persistence (last scene, window size/position/mode — small config file; the quality
  tier already persists via `[quality] tier`).

## 12. Runtime memory (added 2026-07-21; retargeted 2026-07-22 per [ADR-0010](adrs/0010-accept-gpu-driver-memory-floor.md))

"Lightweight" (NFR §4) caps *binary* size but not *working set*. The original §12 target — "well under
~100 MB", to be hit primarily by compiling wgpu with only the per-OS backend — was **measured and
disproved** by Plan 0011 (Phase 6 landed the backend-trim; Phase 7 measured it). On the reference AMD
iGPU box, release build, the standalone sits at **~300 MB working set / 343 MB private commit** — the
trim took effect (verified DX12-only, no Vulkan/GL mapped) but footprint is dominated by the **DX12
driver stack's private heap** (`amdxc64.dll` + `d3dcompiler_47` + `D3D12Core` …), not by wgpu's
compiled backend code (mapped DLL code is only ~135 MB, and shared). The <100 MB absolute is not
reachable on a DX12/wgpu app; the backend-trim is retired as a *memory* lever (it stays as a binary-size
win under §4). See ADR-0010 for the decision and rejected alternatives.

Retargeted requirements — chosen to be enforceable by the Plan 0011 diagnostics harness
(`diagnostics.log`, `lmv_get_metrics`):

- **No session growth (the requirement that matters).** Working set / private commit stays flat over a
  session — no monotonic growth across the §10 ≥4-hour soak. A leak is the real live-show failure; the
  harness is the instrument. This is the hard requirement.
- **State the cost of what we add.** The GPU driver stack is a fixed, vendor-dependent floor we do not
  own; the actionable lever is **our** additions — render-pipeline / shader / resource count. A new
  built-in system states its working-set delta on the reference box (harness-measured), so growth is a
  recorded choice, not a surprise. (Footprint rose from ~200 MB to ~300 MB across Plans 0003/0010/0011,
  most plausibly from added pipelines — exactly this cost, previously untracked.) **Now quantified**
  (Plan 0012, reference AMD iGPU box, private commit): the fixed driver floor is **~327 MB** and our
  entire visual system (2 scene pipelines + overlay + DSP + audio + presets) adds only **~11 MB (~3%)**;
  culling 3 dead scene pipelines saved ~2 MB, so pipeline count is a real but **weak** lever
  (~1 MB/pipeline) against a floor that dominates.
- **Soft ceiling, for regressions only:** ~350 MB working set on the reference AMD iGPU box with the
  current built-in system set. A single-machine, vendor-dependent tripwire to catch a regression — not
  a portable absolute (a different GPU/driver has a different floor). Vendor spread (Intel iGPU) is a
  pending on-device capture — `docs/on-device-validation.md`.
- **Our own Rust state stays <~1 MB** (ring buffer ~340 ms of f32, fixed DSP buffers, a few uniform
  buffers) — unchanged; the target was never our allocations. The **emitter's object pool** (Plan
  0052 / [ADR-0057](adrs/0057-emitter-scene-analytic-ballistics-seeded-individuation.md)) is the
  newest entry here and it stays well inside that line. It is a **fixed** allocation, made once at
  scene construction and never grown — spawning past it drops the spawn rather than reallocating,
  which is the whole reason it is a pool — so this is a ceiling and not an average:

  | | floor (2 000 objects) | rich (6 000) |
  |---|---|---|
  | CPU pool (`Object`, 40 B incl. padding) | 80 KB | 240 KB |
  | free list (`u32`) | 8 KB | 24 KB |
  | CPU instance mirror (28 B) | 56 KB | 168 KB |
  | GPU instance buffer (28 B) | 56 KB | 168 KB |
  | **total** | **~200 KB** | **~600 KB** |

  Two orders of magnitude under the ~66 MB a single post chain costs, and the reason the tier's
  `emitter_objects` was sized for headroom rather than trimmed: the pool is bounded by cost of
  *drawing* the marks, not by the memory holding them. It adds **one** render pipeline, i.e. ~1 MB
  by the Plan 0012 measurement above, which is the number that actually moves.
- **The linear-light composite is the largest single addition since this section was written**
  (Plan 0045 / [ADR-0046](adrs/0046-linear-light-hdr-composite-bloom-tonemap.md)). Every intermediate
  upstream of the tonemap moved from the surface format to `Rgba16Float` — 8 bytes a texel, not 4 — so
  the offscreens that were charged at the surface format doubled. The trails accumulation
  (`PingPongField`, two textures) was already float and did not move. At the floor post cap
  (1920x1080, 16.6 MB a full-size float texture):

  | buffer | before | after |
  |---|---|---|
  | trails composited | 8.3 | 16.6 |
  | trails accumulation (x2) | 33.2 | 33.2 |
  | kaleidoscope source | 8.3 | 16.6 |
  | **per chain, both stages live** | **50** | **66** |
  | bloom source + pyramid (only when `bloom_amount > 0`) | — | 16.6 + ~11 |
  | tonemap input (surface-sized, genuinely new) | — | 16.6 |
  | transition snapshot + live, while a dissolve runs | 8.3 x2 | 16.6 x2 |
  | ink input (stays 8-bit — the tonemap hands it display-referred pixels) | 8.3 | 8.3 |

  Plan 0023's dual-live dissolve holds two whole chains, so the peak is ~133 MB rather than ~100, and
  the worst case — dual-live, every stage on including bloom, ink on — is **~246 MB** against the
  ~350 MB soft ceiling above, most of which is driver floor already. At the rich cap (2560x1440) the
  same arithmetic is ~118 MB per chain. **The post cap is the relief lever** if the float chain misses
  §1 on a floor-tier iGPU: lower it rather than re-fixing the grids, since bandwidth roughly doubled
  with the format and the grid policy is shared. **Rich-tier frame time on the target GPU is measured
  and bloom is not the expensive part:** windowed on the dev box's discrete GPU, `star_lantern` (the
  one shipped preset that binds `bloom_*`) runs 164 fps at p99 **8.2 ms**, against
  `attractor_clifford` at p99 19.9 ms and `attractor_leviathan` at 19.0 ms — neither of which
  switches the stage on. The float composite plus the attractor is what puts the heaviest preset past
  a 60 Hz frame at Rich. The fullscreen and `Floor`-pinned runs, and the whole real-iGPU side, stay
  with `docs/on-device-validation.md`.
- **Driver floor isolated (Plan 0012 Phase 2, resolved):** the once-optional dev spike ran —
  `standalone/examples/floor.rs`, a scene-less window standing up only the wgpu context — and put the
  hard **~327 MB private-commit** floor number on the split above. It confirms ADR-0010's diagnosis: the
  cost is the driver stack, not our code. Does not change ADR-0010.

Measurement method (repeatable): PowerShell `Get-Process lmv` → `WorkingSet64` vs `PrivateMemorySize64`,
`.Modules` by mapped size, and which backend loader DLLs are mapped. The private-vs-working-set split is
what proved the cost is driver heap, not our code.

Not a Plan 0001 blocker; the leak-guard folds into the §10 live-features soak, the per-system delta into
each scene-adding plan.
