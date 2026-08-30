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
- **`frame_ms_p99` spikes on a GPU resource rebuild, and a governor reading it bare would demote
  a preset that is running fine.** Read this before designing the governor — it qualifies the
  instrument the design is specified to read. Measured over three minutes at Rich tier, 1080p, with
  preset switching and a fullscreen toggle (Plan 0046 Phase 5,
  [backlog 0082](design-backlog.md)):

  | | value |
  |---|---|
  | fps median / min | 165.0 / 114.3 |
  | rows below this section's 60 fps floor | **0 of 158** |
  | `frame_ms_avg` median / max | 6.061 / **8.749** ms |
  | `frame_ms_p99` median / max | 6.866 / **25.037** ms |
  | frames dropped | **0 of 28,698** |

  The budget holds with ~2.7x headroom and **nothing is dropped**, yet p99 passes 16.67 ms. The
  spikes coincide with the preset switches and the fullscreen toggle: they are the cost of
  *rebuilding* GPU resources, not of running the preset. A demotion fired on one of them would
  change what the audience sees during the event that is already the most visually disruptive.

  **Three candidate responses were named, and choosing between them is the governor's own design
  decision — this file deliberately does not choose.** Exclude the frames following a switch or
  surface reconfigure from the governor's window; require N consecutive bad windows rather than one;
  or read a separate steady-state statistic and leave `p99` the diagnostic it is today. Whichever is
  taken, **the measurement above is the test case.**

  **The shipped governor does not read `p99` at all, and that is worth knowing before anyone
  "fixes" it** (checked against `core/src/render/tier.rs` at Plan 0085 Phase 4, and it contradicts
  backlog 0082's own premise that the governor "is specified to read p99"). `sustained_miss` counts
  what fraction of the raw frame-time series exceeds `budget × MISS_FACTOR` and demotes only when
  **75 % of at least 180 samples** miss. A preset switch contributes a handful of slow frames to a
  240-sample ring, which cannot approach that fraction — so the governor as built already landed the
  second candidate response, in the form of a miss *fraction* rather than consecutive windows. What
  is still live is the **wording**: the roadmap item and the backlog entry both describe a governor
  that reads p99, and a future revisit starting from that description would reintroduce the hazard
  this measurement documents.

  The instrument for the third response exists anyway: `--soak` writes **`frame_ms_p99_steady`**
  beside the raw `frame_ms_p99`, the same statistic with the frames following a switch or
  reconfigure left out, alongside a monotone `switches` counter (Plan 0085 Phase 3,
  [ADR-0099](adrs/0099-the-show-length-horizon-is-a-spot-check-and-it-splits-in-two.md)). It is a
  **reading, not a gate** — nothing demotes on it today.
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
- **This budget binds the window, and the streamed picture sits outside it.**
  `lmv --stream` ([ADR-0125](adrs/0125-the-live-video-out-is-a-spout-sender-fed-by-a-frame-tap.md))
  publishes frames to another application, which then composites and presents them on its own
  schedule — so what a viewer of that composite sees is governed by the receiver's pipeline, not
  by anything measurable here. Our half adds a GPU readback and an upload to the numbers above;
  everything after the Spout sender is out of our hands and is deliberately not budgeted. A
  latency claim about the streamed path is a claim about two applications, and this document
  only speaks for one.

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
  **All of those carry `--workspace` since [ADR-0072](adrs/0072-the-c-abi-ships-from-its-own-crate.md)**,
  and it is load-bearing rather than stylistic: `lmv-core-cabi` is deliberately outside the workspace
  `default-members`, so the bare forms would silently stop testing and linting the C ABI entirely.
- Plus **eight** single-runner gates: `cargo deny check` (supply chain), Miri over `lmv-ring`'s
  `unsafe` (UB), the coverage ratchet below, and the five Node doc gates that share the `links`
  job — `check-doc-links.mjs` (every relative markdown link resolves — Plan 0061 Phase 2c),
  `check-index-rows.mjs` (every row inside a marked roster region stays a pointer under 320 bytes —
  [ADR-0116](adrs/0116-an-index-row-is-a-pointer-and-a-gate-holds-it-to-one.md)),
  `check-backlog-claims.mjs` (every live backlog entry's probe still holds —
  [ADR-0108](adrs/0108-a-backlog-claim-about-the-repo-carries-an-executable-probe.md)),
  `check-filter-figures.mjs` (the diffusion filter's cost figures live in one page —
  [ADR-0122](adrs/0122-a-sidecar-tool-documents-itself-in-one-place.md)), and
  `check-comment-hygiene.mjs` (no `.rs` comment carries a relative link or plan-relative narration —
  [ADR-0127](adrs/0127-a-comment-carries-the-mechanism-and-the-decision-record-stays-in-docs.md)).
- **The nine GPU-heavy suites run once per push, not twice**
  ([ADR-0073](adrs/0073-the-windows-ci-critical-path.md), Plan 0061 Phase 2b). They render the shipped
  preset library on WARP, and until that change ran uninstrumented in `check (windows-latest)` and
  instrumented in `coverage` at the same moment on two identical runners (≈ 1930 duplicated CPU-
  seconds). `check` now carries the same exclusion `.githooks/pre-push` does, which makes **`coverage`
  the only place they execute on Windows** — so that job is load-bearing for *correctness*, not only
  for the ratchet. Disabling it, skipping it, or letting `cargo-llvm-cov` fail to install takes the
  golden guard and every GPU behavioural suite with it.
- **Live audio** cannot run in CI. **GPU rendering partly can**: on Windows the DX12 **WARP**
  software adapter makes headless rendering deterministic, which is what the golden suite and
  the tier-4 chain test ride on. macOS has no software Metal fallback ([ADR-0016](adrs/0016-gpu-tests-opt-in-ci-scope.md)),
  so the GPU suites skip there with a printed reason. Real-GPU-vendor and live-loopback checks
  stay manual — see [`on-device-validation.md`](on-device-validation.md).
- **Coverage ratchet** ([ADR-0033](adrs/0033-testing-strategy-coverage-ratchet-and-pre-push-gate.md)):
  a Windows-only job runs `cargo llvm-cov nextest -p lmv-core --fail-under-lines $COVERAGE_FLOOR`,
  and since ADR-0072 a second, smaller gate beside it on `-p lmv-core-cabi` against
  `$CABI_COVERAGE_FLOOR` — without which the C ABI's coverage would silently stop being watched the
  moment it left `lmv-core`. Neither gates `standalone/`: it is a `winit` event loop plus two platform
  capture backends no runner can execute. Both floors live in exactly one place, the `env:` block in
  `ci.yml`, and are a **ratchet, not a target**: set from measurement, raised at a close ceremony when
  a plan improves coverage, lowered only with a note naming the plan and the reason. `COVERAGE_FLOOR`
  was **88** from Plan 0032's measured 90.13 %, and is **91** since Plan 0061 Phase 2 — a *moved
  denominator*, not better tests, since `ffi.rs` and its conformance suite left the gated crate.
  **That 91 was measured on the dev box, which has a hardware GPU where CI has WARP, so it is owed a
  re-derive from a cache-warm CI run** (Plan 0061 Phase 9, outstanding); the margin is ~3 points rather
  than the usual 2 for exactly that reason. `CABI_COVERAGE_FLOOR` is **54** against a measured 56.60 %,
  and it is low because most of `core-cabi` is error, null-handle and `catch_unwind` paths — recorded
  to catch a regression, not claimed as good coverage. A line-coverage floor is gameable by design — it
  is a backstop against silent erosion, not a quality measure. The Mode 4 review's
  "read the assertion body" step remains the actual quality gate.
- **Local pre-push gate** (opt-in, per clone): `.githooks/pre-push`, enabled with
  `git config core.hooksPath .githooks`. Runs the fast subset — the three Node doc gates, `fmt`,
  `clippy --workspace`, and a narrowed `nextest --workspace` — and prints the GPU-heavy suites it
  skipped.
  **Measured 48.6 s warm (2026-08-08, dev box), against the ~28 s recorded when ADR-0033 set it up.**
  The number drifted with the suite it runs, not with the gate's design; it is recorded here rather
  than targeted, and the README's developer section carries the per-step breakdown. If it grows past
  the point where people start reaching for `--no-verify`, that is the signal to narrow it further —
  ADR-0033's own argument is that a gate which hurts gets disabled.
  `cargo deny`, doctests, Miri, and coverage stay in CI. An uninstalled clone silently has no gate;
  see the README's developer section. All five Node steps (`check-doc-links.mjs` ~50 ms,
  `check-index-rows.mjs`, `check-backlog-claims.mjs`, `check-filter-figures.mjs`,
  `check-comment-hygiene.mjs`) also run as the CI `links` job
  (`ubuntu-latest`), so they are enforced for everyone rather than only where the hook is installed
  — and they skip together with a notice when `node` is absent, which is the [ADR-0016](adrs/0016-gpu-tests-opt-in-ci-scope.md)
  shape.

## 8. Distribution (v1)

Delivered by `.github/workflows/release.yml` on a pushed `v*` tag
([ADR-0038](adrs/0038-tag-driven-release-unsigned-universal-mac-app.md),
[ADR-0115](adrs/0115-the-foobar-component-is-a-released-artifact-with-a-parameterized-sdk.md)).
**Three** zips, attached to a GitHub **prerelease** — a plain download URL, no account needed,
because the repository is public.

- **Windows**: `lmv.exe`, x64, **unsigned** (SmartScreen warning accepted).
- **macOS**: a **universal** (arm64 + Intel) `LightMusicVisualizer.app`, **ad-hoc signed and
  unnotarized**. Ad-hoc signing buys a stable code identity for the Screen Recording grant to
  bind to; it is *not* Developer ID, so Gatekeeper still quarantines the download and the grant
  does not survive a rebuild. Requires macOS 13+.
- **foobar2000 component**: `foo_lmv.fb2k-component`, **x64 only**, for foobar2000 v2. Built
  by `packaging/foobar/build-component.ps1` against a **pinned, checksummed** SDK release that
  the workflow fetches (`packaging/foobar/sdk-pin.ps1`). Unsigned, like the rest.
- All three zips carry a `READ-ME-FIRST.txt`; the two standalone ones also carry a reference
  copy of `presets/*.toml`.

**The component ships as of Plan 0102** (2026-08-16). This paragraph previously read
"Standalone only — CI does not ship a `.fb2k-component`", on the grounds that the SDK is
third-party, separately licensed and `.gitignore`'d, so no runner could build the shim. The
licence was then read rather than assumed: it is BSD-style, permits binary redistribution, and
puts a notice obligation only on redistributed *source* — so the workflow fetches the SDK and
the component is a released artifact. The SDK is still never committed
([ADR-0115](adrs/0115-the-foobar-component-is-a-released-artifact-with-a-parameterized-sdk.md)
Alternative A).

**Nothing in CI can test the component.** No runner loads foobar2000, so its installation and
`visualisation_stream` behaviour are checked by hand and recorded in
[`on-device-validation.md`](on-device-validation.md) — the same gap the macOS path has, named
rather than solved.

No installer, no Developer ID signing, no notarization, no DMG in v1. Signing, if ever, is a
later plan + human task.

## 9. Test hardware matrix (what the user has)

| Machine | Validates |
|---------|-----------|
| Primary Windows dev box | Standalone Windows path, plugin, day-to-day dev |
| Older Windows PC (iGPU) | The performance floor (§1) on baseline hardware (§2) |
| foobar2000 (installed) | Plugin loading + `visualisation_stream` behavior |

**There is no Mac in this matrix, and that is the point of §8's macOS artifact.** An earlier
revision of this table listed a "Mac, macOS 13+" as available hardware; it is not, which is why
the dev box cannot link a Mach-O binary and a macOS runner is the only build host
(ADR-0038). The macOS standalone path — Metal through wgpu, ScreenCaptureKit capture, glyphon's
font loading — is therefore validated by a **recipient**, not in-house, and until one reports
back it has never executed on Apple hardware at all.

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
- **A show configuration is a different workload, and it plateaus near ~800 MB (measured
  2026-08-29/30, the first full live set).** 8h08m, 3,505,083 frames, **zero dropped**, 120.0 fps
  flat end to end, on the show notebook — **not** the reference AMD iGPU box — at Rich tier with the
  operator console open and 18 presets rotating from an `LMV_PRESET_DIR`. Working set was 678 MB
  early, climbed to ~795 MB, then went **flat**: three consecutive half-hours at 795.4 MB, later
  settling 799.5 → 808.3 MB in discrete steps with flat stretches between. Handles pinned at exactly
  370 across six hours, threads at 5, `gpu_bytes` at 15.8 MB from first sample to last. The
  step-then-flat shape is a resource claimed on a preset's first pass through the rotation and then
  cached. **This does not move the ~350 MB soft ceiling above, which is scoped to the reference box
  and stays a single-machine tripwire** — it is a second point on the vendor/workload spread, and the
  first one taken at show length rather than in minutes.
- **Do not call a leak on a window shorter than two flat half-hours.** The same run was read
  mid-session as a linear leak at **+0.87 MB/min**, on a 20-minute window, and that reading was
  wrong: it was warm-up. This app's growth is step-then-flat, so any short window through a step
  fits a convincing line. The discipline is to hold the measurement until two consecutive
  half-hours agree.

Measurement method (repeatable): PowerShell `Get-Process lmv` → `WorkingSet64` vs `PrivateMemorySize64`,
`.Modules` by mapped size, and which backend loader DLLs are mapped. The private-vs-working-set split is
what proved the cost is driver heap, not our code.

Not a Plan 0001 blocker; the leak-guard folds into the §10 live-features soak, the per-system delta into
each scene-adding plan.
