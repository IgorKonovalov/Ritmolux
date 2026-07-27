# Plans index

The one-minute "what's in flight" view. Read this first each session instead of
re-deriving state from `git log`. Completed plans move to `done/`.

**Next free number: 0039**

## Active roster

| Plan | Title | Status | Owner skill(s) |
|------|-------|--------|----------------|
| [0037](0037-verifying-easing-transient-probe-and-dynamic-signal.md) | Verifying easing: a transient probe, a signal with dynamics, and the levels authors calibrate against | **approved 2026-07-26** — ready for `dev` | dev, human |
| [0038](0038-line-family-unreachable-levers.md) | The line family's unreachable levers: `glow`, the readout's geometry, a level curve, and `log` | **approved 2026-07-27** — ready for `dev`, **take this first** | dev, human |
| [0036](0036-macos-and-windows-release-artifacts.md) | macOS and Windows release artifacts: a tag-driven Release with a universal `.app` | **approved 2026-07-26** — ready for `dev` | dev, human |

## Recommended execution sequence

**[0035] has landed and closed** — the composite's aspect is the render target's
([ADR-0037](../adrs/0037-internal-grid-is-a-resolution-not-a-shape.md), now **accepted**), the two
post stages have their first capture-level pixel guard, and the two copy-pasted grid policies are one
function in `render/grid.rs`. See Recently closed. Every later stage inherits that rule, so backlog
0005's bloom stage now builds against it rather than against the defect.

**[0034] has landed and closed** — `bin(x)`, the eighth `SystemKind` (`spectrum`), the `[spectrum]`
table and per-element `index` are all live, and the curated set uses them (`037825d`). See Recently
closed. **Take [0037] next.**

The consequence accepted when [0037] was sequenced *after* [0034] (the user's explicit choice,
2026-07-26) **materialised and was caught at the close, not by the harness**: Phase 2 taught
`sanity`, `reactivity`, `golden` and `distinctness` to light the band array but left `shot` blind, so
the surface the content lane self-verifies through scored a spectrum preset on its scalar bindings
alone while `cargo test` passed it. Fixed in `ca99cb1`. That is the second time in three plans a
capability shipped without the content lane being able to see it, which is exactly [0037]'s subject.

**[0038] is approved and sequenced before [0037] at the user's explicit choice (2026-07-27)** —
design first, then implement. **Take it next.** It closes the whole 0016–0019 backlog batch: four levers that already
exist in the engine and are simply not reachable from a `.toml`. It is safe to take ahead of [0037]
because **every new parameter defaults to today's constant**, so Phases 1–5 are behaviour-preserving
and assert byte-identical goldens; the one claim [0037]'s probe would measure directly
([ADR-0040](../adrs/0040-spectrum-level-curve-applies-before-the-easing.md)'s curve-before-easing
ordering) is deliberately recorded as a *property* rather than a tuned number, so the probe can
confirm or refute it later without this plan having invented a threshold. [0038] Phase 6 is a
`preset-author` pass and is the only phase that changes how anything looks.

**[0033]'s Phase 8 (`human`) is open and independent** — the aesthetic re-tune on the user's
2048x1152 display: run the `preset-author` lane over the four `reaction_*` presets (now free of the
`zoom = 0.99` pin) and the 13 line presets, **restoring `trails` where it was removed purely for
sharpness**. That display is aspect-exact under the policy, so nothing blocked it; the stale
"both stages render at a fixed 1280x720" note in fourteen preset headers is preset content and rides
with this pass. (`66300d6` and `8b5b2e0` are that pass in progress.)

**[0034]** closes backlog 0002, the capability the user asked for twice. Three verifications shrank
it well below the feedback
note's estimate: the 64-band log-spaced spectrum **already exists** on `AnalysisFrame` (`dsp/mod.rs:32`,
commented "Log-frequency bands exposed to scenes"), **every scene already receives it** through
`Scene::update`, and `LineRenderer` **already draws arbitrary segment lists** — so there is no new DSP,
no new render idiom, and no `Scene`-trait or C-ABI change in the first three phases. `bin(x)` alone
answers the attractor-morphing half; per-element evaluation is sequenced last on purpose
([ADR-0036](../adrs/0036-preset-reachable-spectrum.md)).

**[0036] is orthogonal to all of the above** — it touches no `core/`, `standalone/` or shader code
at all (two new `packaging/` scripts, one new workflow, a docs sweep), so it neither blocks nor is
blocked by the render work and can be taken whenever the user wants artifacts. It exists because
nothing in this repo can currently be handed to another person, and it delivers the **standalone
half of roadmap item 5**; its Phase 4 (`human`) is gated on a friend with a Mac, which is also the
long-deferred Plan 0001 Phase 10 on-device validation finally being exercised. Note the practical
prerequisite: pushing anything under `.github/workflows/` needs the `workflow` OAuth scope on the
git credential.

**Deliberately sequenced after [0033]:** backlog 0005 (a bloom/glow post stage) — it now builds against
target-sized stages rather than inheriting the 720p problem it exists to answer, and it should follow
the aspect fix above so it does not inherit *that* instead. **Still undesigned:**
backlog 0007 (`star_pattern`, which the user chose to invest in rather than cut) and the waterfall
spectrogram, which [0034] leaves as a followup on the same spectrum surface.

### Prior sequencing notes

A tactical ordering of the **active roster** (strategic themes live in the Roadmap below).
(**Plan 0014 has now landed and closed** — the `render::feedback::PingPongField` seam, the injected-`dt`
render clock (`Renderer::render(&frame, dt)`, `Scene::advance`, `SCENE_DT` retired to `FALLBACK_DT`),
C ABI **v4 `lmv_render_dt`**, and the `ReactionDiffusion` scene all exist for the plans below to build
against; see Recently closed. Plan 0013's capture/visual-QA harness landed before it.)

(**[0029] Attractor resize cost + ink-stage followups has now landed and closed** — the attractor's
GPU resources are split along grid-dependence so a size change rebuilds only the accumulation field,
the trail grid is quantized to a 256 px step with an aspect-preserving cap, the ink stage has its
first behavioral test, and the projection uses the **target's** aspect rather than the grid's; see
Recently closed. Its `PipelineResources`/`FieldResources` split is the pattern the deferred
"target-sized internal grid for RD/trails/kaleidoscope" work would follow.)

(**[0030] Composite chain + scene keying has now landed and closed** — the composite is an owned
`PostChain` holding `Trails`/`Kaleidoscope`/`Ink` behind a declared `PostStage` trait in ADR-0018/0028
order, its routing is a **pure, unit-tested** function over the active flags, and `system_slot`'s
magic-index scene lookup is gone (scenes are keyed by `SystemKind` via an exhaustive factory over the
single `SystemKind::ALL` roster). Every golden baseline is byte-identical. See Recently closed.
**A second `PostChain` with fully independent GPU state is constructible and proven so by a test** —
that is [0023]'s dual-live unblock. [0023] **has been revised against this** (2026-07-25): its Phase 3
now says "construct a second `PostChain`", and the revision surfaced and settled where the blend sits
([ADR-0032](../adrs/0032-ink-leaves-the-chain-blend-between-chain-and-ink.md) — ink leaves the chain).)

(**[0023] Cross-preset transitions has now landed and closed** — **every** preset switch dissolves
instead of cutting: `Space`, a browse-overlay pick, the director auto-rotate, and the C ABI
`lmv_cycle_scene`, over ~1 s, through a deterministic rotation of four blend kinds. The composite is
now `background -> scene -> PostChain (trails -> kaleidoscope) -> [blend] -> ink -> present`: ink left
the chain to become a terminal engine post-pass (ADR-0032) and the two-input blend sits between them,
outside the one-input `PostStage` trait. `Renderer::select_preset_now` is the instant-cut escape the
capture entry points use, so captures stay a pure function of their inputs. See Recently closed.
A later plan touching the render loop should pick up the five minors logged there — the switch-site
`from`/`to` ordering and the standalone's stale post-switch reads are the two with user-visible edges.)

(**[0032] Testing strategy has now landed and closed** — the suite gained a **tier 4** it did not
have: `core/tests/chain.rs` crosses the ring→analyzer→renderer seam for the first time, and
`standalone/tests/shot_cli.rs` runs the built `shot` binary as a subprocess. That second one is the
behavioral net [0031] Phase 1's `shot.rs` refactor should now be done under — its soft
"land Phase 2 before [0031]" preference **is satisfied**. Also live: an opt-in `.githooks/pre-push`
fast gate (~28 s) and a `COVERAGE_FLOOR` ratchet on `lmv-core` at **88** against a measured
**90.13 %** — [0031] deletes code, so watch that number at its close. See Recently closed.)

(**[0031] Cleanup pass has now landed and closed** — `standalone/src/shot/` holds the CLI's pure
helpers under real tests, `Renderer::from_context` is the one construction path, each binding's
easing `tau` and destination resolve **once at load** onto a render-layer route table, three pieces
of per-frame work are gone, `core/src/render/gpu.rs` is the single home for the repeated wgpu
boilerplate, and eight items of accumulated close-review debt are closed. Every golden baseline is
byte-identical. See Recently closed.)

(**[0016] GPU compute-particle scenes has now landed and closed** — the engine's first compute
pipeline + GPU-resident particle system (four strange-attractor families, data-driven `[particles]`
selection, trails on `PingPongField`) is available for later plans to build the effects layer on;
see Recently closed.)

(**[0010] Line-geometry scenes has now landed and closed** — the line-art category (roses,
L-systems, Hankin stars) built on the shared `LineRenderer` is available; see Recently closed.)

(**[0015] Preset-dir override has now landed and closed** — `LMV_PRESET_DIR` plus `shot`'s
`--presets` / `--preset-file`, both resolved through the single `standalone/src/lib.rs` resolver, so
editing a version-controlled `presets/*.toml` is live in the app (~150 ms) and in the next capture
with no rebuild; see Recently closed. That is the edit loop the scene and grammar plans below tune
their presets in.)

(**[0019] Preset grammar v2 has now landed and closed** — the expression language a preset binding is
written in is v2: `cos sqrt pow mod smoothstep` + `select`, the constants `pi`/`tau`, six comparison
operators, and the `tempo`/`novelty` variables; an unknown parameter name is a surfaced load-time
**warning** instead of a silent no-op. See Recently closed. Plans and presets below can assume the v2
vocabulary — notably **[0028]**'s `phase`/`radial_offset` rose morphing now has `tempo` and thresholds
to drive it. **Still outstanding at that close, user-gated:** the `preset-author` skill docs
(`SKILL.md`, `references/grammar.md`, `references/render-loop.md`) still describe the **v1** grammar
and still teach the pre-`LMV_PRESET_DIR` `%APPDATA%` copy-over — the assistant cannot edit
`.claude/skills/**`, so the lane runs on a stale reference until the user applies it.)

(**[0020] Shared palette system has now landed and closed** — the shared `core/src/render/palette.rs`
baked-LUT color surface (named + custom-stop palettes, bindable `saturation`/`hue`/`color_span`/
`color_center`/`hue_spread`/`hue_center`, and the A/B `palette_mix` crossfade) reaches **all four**
shader-colored scenes through the new off-hot-path `Scene::set_palette` hook; see Recently closed.
Plans below that build on the RD/attractor present/draw path now ride on its LUT-colored output.)

(**[0025] Full composite coverage has now landed and closed** — `bg_*` and `zoom`/`pan_*` reach the two
fullscreen/accumulating scenes (reaction-diffusion, attractor) via an alpha-present-over-backdrop and
named-param view transform; both present shaders now emit a premultiplied-alpha channel over the shared
backdrop (byte-identical over the default black). See Recently closed. Plans below that build on the
RD/attractor present path now ride on its alpha-composited output.)

(**[0027] Attractor ink-on-paper + crisp trails has now landed and closed** — the engine-wide final
composite stage `render/ink.rs` gives *every* scene a black-on-white / colored-duotone mode via the
bindable `ink_*`/`paper_*` params, and the attractor's trail grid follows its render target instead of
a fixed 640x360. See Recently closed. The ordering wiring it left **[0023]** — ink must remap the
*blended* frame, so the transition blend goes before it — was designed in
[ADR-0032](../adrs/0032-ink-leaves-the-chain-blend-between-chain-and-ink.md) and **has now landed**
as 0023's Phase 1: ink left the chain to become a terminal post-pass, byte-identical goldens, so the
blend sits ahead of it without widening the `PostStage` trait. A tidy-up candidate still
open in `render/scenes/reaction_diffusion.rs:1048`: correct the stale "fullscreen opaque pass" comment
on the RD present, left from before 0025's alpha switch — **carried by [0031] Phase 6**.)

## Standing (not a plan)

- **[On-device validation — low-end Windows iGPU smoke](../on-device-validation.md)** — a
  hardware-gated checklist, **not** a phased plan and **not** in the roster above: it never blocks a
  plan from closing. Holds the low-end / older Windows iGPU checks (fps floor ≥ 60 @ 1080p; footprint
  on a second GPU vendor) the user can only run once that box is in hand. Ticked when run; deleted when
  empty. Currently home to the extracted Plan 0012 Phase 3 (also covers the identical Plan 0003 Phase 3
  iGPU-fps carry-forward).

## Recently closed

- [0034 — Preset-reachable spectrum: `bin(x)`, a spectrum scene, and per-element
  evaluation](done/0034-preset-reachable-spectrum.md) — **done 2026-07-27**, passed Mode 4 review
  (**no blockers**; two majors, four minors, two nits, **all fixed in `ca99cb1`** rather than
  carried). Five `dev` phase commits (`a379b28` `bin(x)`, `2450c2a` the `spectrum` system, `a553b2e`
  the `[spectrum]` table, `6950c94` per-element `index`, `fe11659` the operator sweep) plus
  `ca99cb1` the review fixes and `4d41884` the band-axis documentation correction. Closes backlog
  0002, the capability the user asked for twice. The scoping claim **held**: no new DSP, no new
  render idiom, no `Scene`-trait change, **C ABI stays v4**, no new dependency — the 64-band array
  already existed on `AnalysisFrame`, every scene already received it, and `LineRenderer` already
  drew arbitrary segment lists, so an eighth `SystemKind` cost an exhaustive-match arm rather than a
  pipeline. `Variables` beat its own feared 264-byte per-binding copy by **borrowing with a
  lifetime**. **Major 1:** a gradient that *repeats* past its ends is not *continuous* there — all
  four stop-list palettes run dark to light, so a full `hue_spread` walk puts the sharpest transition
  on the ring; `Spectrum Corona` demonstrated the falsehood it was written to illustrate and its
  palettes were re-cut to return to their starting colour. **Major 2:** Phase 2 lit the band array in
  `sanity`/`reactivity`/`golden`/`distinctness` **but not in `shot`**, so the lane's own verification
  surface scored spectrum presets on their scalar bindings alone (`Spectrum Comb` bass 0.040 → 0.084,
  onset 0.000 → 0.119, coverage 0.664 → 0.913 once fixed). `--set` stays scalar-only **deliberately**,
  documented in `docs/capturing.md` as its third calibration trap. **The substantive postscript:**
  ADR-0036 and the plan both stated the band resolution profile **backwards**. The array is
  **35 Hz–18 kHz**, not 20 Hz–Nyquist, and `fft.rs` floors every band at one FFT bin *after* laying
  the log edges — 23.4 Hz at 2048 — which binds to **band 30 (~750 Hz)**, so **31 of the 64 bands are
  linear**. Band 0 spans 23–47 Hz, *a full octave in one number*; resolution peaks near 500–800 Hz and
  settles at ~1.7 semitones above 1 kHz, so **the low end is the coarsest region musically, not the
  finest** — and below the crossover the mapping moves with the sample rate. The error propagated once
  before it was caught (`037825d` annotated its probes from the log-edge curve, **up to 2.9x wrong**
  below the crossover; bindings were tuned by effect and unchanged, comments corrected).
  **[ADR-0036](../adrs/0036-preset-reachable-spectrum.md) accepted with an Outcome section.**
  **Verified at close:** `fmt --check` + `clippy --workspace --all-targets -D warnings` clean,
  `nextest --workspace` **251/251**, `core/tests/golden/` **byte-untouched**. **Two items routed to
  the backlog:** [0015](../design-backlog.md) whether the half-linear axis is a defect (**ADR-worthy
  if acted on**) and [0016](../design-backlog.md) the readout's missing `span`/`width` param. Version
  **minor 0.17.1 → 0.18.0**.

- [0035 — The composite's aspect is the target's: the grid-shape stretch, one grid policy, and a
  pixel guard for the post stages](done/0035-composite-aspect-and-grid-policy.md) — **done
  2026-07-26**, passed Mode 4 review (**no blockers, no majors**; four minors, one nit). Four `dev`
  phase commits (`d4f98f8` the aspect fix, `687621b` the two post-stage baselines, `bc11b23` one grid
  policy, `f9f9e79` the docs). Turning `trails` or `kaleido_*` on now changes the picture's
  **softness and nothing else**: `SceneTarget::aspect` comes from `surface`, the kaleidoscope folds
  about the render target's ratio, and `Scene::set_target_size` keeps the grid because that one
  genuinely is a texel count. The two stretches cancel — a scene told the target's aspect draws
  pre-squashed into a differently-shaped grid and the present's normalized blit is exactly the
  inverse. Plan [0029] Phase 5's attractor fix stops being conditional on no stage being active, and
  **[ADR-0037](../adrs/0037-internal-grid-is-a-resolution-not-a-shape.md)'s invariant is now
  checkable by inspection: no `aspect` anywhere in `core/src` is derived from a grid size**.
  `PostStage::resolve` takes **`surface`**, not the destination's own size — deliberately stronger
  than the plan's illustrative sketch, because every present down the chain is a stretch, so a stage
  added *after* the fold would otherwise re-plant the same trap. The composite's two stages have
  their **first capture-level coverage** (`core/tests/composite.rs`, its own test binary): one fixture
  composes `trails`, one composes `kaleido_*`, captured at **160x100** — a size whose grid is 256x256,
  so the defect was a 1.6x stretch there, stronger than the 1.28x at 1280x800 and at a sixty-fourth of
  the pixels. A square or 16:9 size comes back aspect-exact and would make the guard blind, which is
  exactly why the defect survived at 1920x1080; **do not "tidy" that size**. WARP was settled by
  measurement (both baselines byte-identical by SHA-256 across three consecutive runs), so they take
  the ordinary blessed posture rather than skip-on-software — chosen so the guard runs in CI, not only
  on developer machines. `composite_kaleido.png` **pins design-backlog 0010 on purpose** at order 6:
  there is no clean order (at order 2 a corner radius still overruns the rectangle along x, below 2
  the stage is skipped), and its header is candid that the baseline *looks* clean because a centred
  figure leaves the source border a smooth gradient. `post.rs::internal_grid_size` and
  `particles::trail_grid_size` are now thin wrappers over one `render/grid.rs::grid_size(surface, cap,
  step)`, **with their caps deliberately different** (1920x1080 for the stages under ADR-0034's
  dual-live arithmetic, 2560x1440 for the attractor under [0029]'s fill budget) and a test pinning
  that difference so an equalizing "cleanup" reads as the memory regression it would be — the
  duplication was not incidental to the defect, it was the mechanism. **Verified at review** rather
  than taken on trust: `fmt --check` + `clippy --workspace --all-targets -D warnings` clean; `nextest
  -p lmv-core` **171/171**; `git diff --stat` over the range confirms `core/tests/golden/` gained
  exactly the two new PNGs and no existing baseline moved; `ffi.rs`, `scenes/mod.rs` and both
  manifests untouched (**C ABI stays v4, `Scene` unchanged, no new dependency, no preset change**).
  **The non-vacuity was reproduced independently**: restoring the grid-derived aspect fails all three
  new guards, with the disc at 1280x800 reading 1.0000 skipped against **1.2800** through trails
  (predicted 1.280; `shot` measured 1.278), the 1920x1080 control unmoved under both, and the capture
  guard failing at `composite_trails` mean 0.0958 / outlier 255 and `composite_kaleido` 0.0356 / 135.
  The kaleido fixture's **5.9 % clamped-pixel figure was recomputed from the shader's arithmetic and
  is exact** (0.0590 at 160x100, order 6). **Minors:** (1) the kaleido fixture's claim that "a 0010
  fix must not pass silently" is **false as written** — applying backlog 0010's first candidate
  (`min(r, 0.5)`) leaves the guard green at mean 0.0189 against its 0.02 tolerance while consuming
  94 % of the drift budget, so the next unrelated fold change trips it with a misleading message
  (recorded in backlog 0010 with the number; a real guard needs a border-filling scene or a direct
  clamped-pixel assertion); (2) `render/grid.rs:22-26` and `post.rs:566-575` still justify the
  single-factor cap by *shape* while the same doc states ADR-0037's opposite conclusion — after this
  plan the surviving reason is **sampling density**, not shape; (3) `post.rs:1014-1018`'s aspect
  assertion is now a tautology whose message describes a property it no longer reads; (4)
  `README.md`'s "166 of 180 tests" went stale (the new `composite` binary is not in the pre-push skip
  list; it costs a measured **2.9 s**, so "~28 s" survives) — **fixed in this close commit**. **Nit,
  also fixed here:** backlog 0010's "fixing 0010 and closing major 3 belong in the same plan" — major
  3 closed here, 0010 still open. **Out of scope, correctly:** `trail_grid_size` keeps its `pub`
  (narrowing means moving `core/tests/attractor.rs` into the crate); `presets/swarm_dense.toml`'s
  false "six stays clean at 16:9" is `preset-author` content already in backlog 0010; and `cargo test
  -p lmv-core --lib render::post`'s `STATUS_ACCESS_VIOLATION` under in-process parallel GPU tests
  **predates this plan** (verified against a stashed tree at `a1e3e26`; `nextest` and
  `--test-threads=1` are clean). **⚠ On-device carry-forward:** the composite now rasterizes up to one
  256 px step of texels it does not show on one axis, on top of the full-resolution ping-pong [0033]'s
  item already watches; and Phase 4 put the RD reconstruction's ~45 texture fetches per fragment on
  `docs/on-device-validation.md`, with a failure routed to `architect` rather than an automatic revert
  because reverting costs the coral look. **[ADR-0037] accepted.** Version **patch 0.17.0 -> 0.17.1**
  (a fix/coverage/refactor/docs plan, no feature).

- [0033 — Internal resolution follows the target, plus the preset-surface and harness gaps behind
  it](done/0033-internal-resolution-and-preset-surface.md) — **done 2026-07-26**, passed Mode 4
  review (**no blockers**; three majors, four minors). Six `dev` phase commits (`978405a` `shot`
  reaches `tempo`/`novelty` + band levels, `cf65c4a` `{ attack, release }` smoothing, `8c0ff2b`
  Catmull-Rom reconstruction, `08714c7` the wrapped RD sampler, `3f3b652` target-sized post stages,
  `621fa7b` the operator sweep). **Phase 4 skipped** under ADR-0034's if-and-only-if — the
  reconstruction fix resolved the coral artifact, so the Gray-Scott grid stays 256, no coral preset
  takes a look change, and the ~4x sub-step cost is not spent. Takes four of the eight
  [design-backlog](../design-backlog.md) entries from the 2026-07-26 `preset-author` batch.
  `TRAILS_W/H` and `KALEIDO_W/H` are gone: one `internal_grid_size(surface)` policy (256 px step,
  single-scale-factor cap at **1920x1080** — not the attractor's 2560x1440, because the trails
  `Rgba16Float` field pair is charged twice during a dual-live dissolve) sizes both stages,
  `PostStage::internal_size` takes the surface, and `KALEIDO_ASPECT` moved into the uniform so the
  fold corrects by the live ratio. `lsystem_fern` with `trails = 0.75` at 2048x1152 now renders its
  finest fronds crisp with the feedback active — the combination the fixed 720p grid made impossible.
  RD's present pass reconstructs with **Catmull-Rom** (nine bilinear taps, every field read routed
  through `sample_v` — value, gradient, contour, hatch — because the gradient is where a C0
  discontinuity shows), and its sampler is `Repeat`, so `pan_*` is a seamless scroll over a torus the
  simulation has always been and `zoom > 1` tiles instead of smearing the edge row into bars.
  `[smoothing]` takes an `{ attack, release }` pair, hand-deserialized rather than `serde(untagged)`
  so a mistyped table is not harder to diagnose than a mistyped float. `shot --set` reaches
  `tempo`/`novelty` and every filmstrip now prints the min/mean/max the analyzer measured — a
  full-scale 60 Hz sine reads `bass 0.187` and a 120 BPM click peaks at `bass 0.011`, against the
  `--set bass=0.8` the lane had been calibrating against. **Verified at review** rather than taken
  on trust: `fmt --check` + `clippy --workspace --all-targets -D warnings` clean; `nextest -p
  lmv-core --lib` **97/97**; `golden` + `reaction_diffusion` **3/3**; `sanity` + `reactivity` +
  `animation` **3/3** (the floors that actually render through the changed stages, since
  `rose_trails` binds `trails` and two presets bind `kaleido_*`); `-p standalone` **58/58**; and
  `git diff --stat` over the range confirms `core/tests/golden/reaction_diffusion.png` is the **only**
  baseline touched, so both re-bless scope claims hold in fact (`LMV_BLESS` twice rewrote
  `fragment_field.png` / `swarm.png` on WARP variance and both were restored). **Three deviations
  accepted, each because the plan was wrong and the commit said so:** Phase 2's "90 % within two
  60 Hz frames" is unreachable for its own `attack = 0.02` (`alpha = 0.5654`, so two frames reach
  81.1 % and three reach 91.8 % — the test pins the arithmetic); Phase 6's "reports 2048x1152"
  contradicts the same plan's 1920x1080 cap, which ADR-0034 acknowledges as a 1.07x downscale; and
  Phase 3 shipped Catmull-Rom rather than the plan's named cheap quintic coordinate warp, which was
  built and measured **worse** (zero derivative at both ends pins the reconstruction gradient to zero
  at every texel centre, and `line_d`'s `fwidth` gain renders that as one scalloped step per texel).
  **Phase 3's done-when 3 is unmet and is not satisfiable as specified** — it asks a pixel-domain
  scanline second-difference statistic to detect a geometric property of a 1-D contour curve, and at
  8 bits the slope discontinuity over a smooth field is below one output quantum (five measured
  attempts in `8c0ff2b`). Accepted with cause; **no followup metric is owed**, since the RD golden is
  a real pixel guard for exactly this shader, it moved, and it passes. **Majors, all routed to a
  followup fix plan rather than reworked here:** (1) the 256 px round-up makes the grid's aspect
  differ from the target's, and because `post.rs:445` derives the scene's aspect from the **grid**
  while both stages present with a plain normalized blit, the whole frame is stretched whenever a
  stage is active — reproduced with `shot` on a trails-bound `rose_web`, **1.278x wider at 1280x800**
  (predicted 1.280) and **1.069x at 1280x720** (predicted 1.067, where the old fixed grid was
  aspect-exact); 1920x1080 / 2048x1152 / 2560x1440 / 3840x2160 are unaffected, which is why the
  plan's own display never showed it, and because the attractor reads this same `aspect` it also
  undoes [0029] Phase 5's fix on that path — the correction is one line, take the aspect from the
  surface and leave `Scene::set_target_size` on the grid; (2) `docs/on-device-validation.md`'s new
  trails item tells the tester no shipped preset binds `trails`, but `presets/rose_trails.toml:48`
  does, so the item guarding this plan's stated main risk sends them to build a needless workaround;
  (3) **no fixture in `core/tests/fixtures/` binds `trails` or `kaleido_*`**, so Phase 6's "goldens
  re-blessed" done-when referred to baselines that never existed and the headline change ships with
  zero pixel guard on the stages it rewired — which is how major 1 got through (not a blind fixture
  add: a feedback pipeline built mid-run resolves differently on WARP). **Minors:** fourteen preset
  headers plus `attractor_ink.toml:22` still teach the retired fixed-1280x720 rule (preset content —
  rides with Phase 8); `post.rs::internal_grid_size` is a line-for-line copy of
  `particles/mod.rs::trail_grid_size`, the opposite of ADR-0034's "one shared function" consequence;
  the RD reconstruction's real cost is **unmeasured** (the +16 % WARP figure was retracted in
  `3f3b652` as run-to-run noise — 193.6 / 224.2 / 105.2 s on the same suite — leaving 45 fetches per
  fragment as the only real number, and the on-device checklist gained trails items but no RD item);
  `presets/README.md`'s "an ultrawide keeps its proportions" overclaims under the round-up
  (3440x1440 comes back 1.88 against 2.39). **⚠ On-device carry-forward:** full-resolution trails
  against NFR §1's 60 fps @ 1080p floor, the working-set delta including mid-dissolve, and now the
  RD tap count — none reachable from WARP; the mitigation in all three cases is the one cap constant.
  **Core + standalone only; C ABI untouched (v4); `Scene` trait untouched; no new dependency; no
  preset re-tune** (Phase 8's job). [ADR-0034](../adrs/0034-internal-resolution-follows-the-target.md)
  **accepted with an Outcome section** correcting two claims implementation falsified, and
  [ADR-0035](../adrs/0035-asymmetric-attack-release-easing.md) **accepted**. **Phase 8 (`human`)
  remains open** — the aesthetic re-tune on the 2048x1152 display, restoring `trails` to the line
  presets. Version **minor 0.16.1 -> 0.17.0** at close (a feature plan).

- [0031 — Cleanup pass: testable `shot` helpers, one construction path, load-time param routing, and
  the accumulated close-review debt](done/0031-composite-cleanup-and-debt.md) — **done 2026-07-26**,
  passed Mode 4 review (**no blockers, no majors**; five minors, three nits). Six `dev` phase commits
  (`5244fd2` `shot`'s helpers into the lib, `83706a3` `from_context`, `6755014` load-time routes +
  `tau`, `64e7145` three per-frame stops, `fb024fc` `render/gpu.rs` + the attractor split, `609b9c9`
  the accumulated debt). Clears the non-blocking half of the 2026-07-25 codebase-health review **plus
  the minors four earlier closes logged and nobody returned for**. `standalone/examples/shot.rs` went
  1028 -> 803 lines: its pure helpers (the 16-bit WAV parse, `filmstrip_indices` + the strip layout,
  the arg helpers, the JSON emitter, the glyph table) live in `standalone/src/shot/` where `cargo
  test` actually reaches them — an `examples/` target's `#[test]` does not run — and `image` stays a
  dev-dependency because the *layout* arithmetic moved and the pixel blit did not (ADR-0011,
  ADR-0033 Alt E). `Renderer`'s three ~28-line constructors collapse onto `from_context`, with the
  `unsafe` boundary still wrapping exactly `RenderContext::new_unsafe`. **The per-frame binding loop
  no longer re-derives two load-time facts**: `tau` is folded out of the validated `[smoothing]`
  table onto each `Binding` at parse time (`Preset::smoothing` is gone), and a
  `ParamRoute { Background, Stage(usize), Ink, Scene, Unclaimed }` resolved by a pure
  `resolve_route(name, system)` replaces the chained `set_param(&str, ..)` fallthrough — so adding a
  composite stage costs an enum arm, not another link inside the hot loop. Three per-frame
  operations stop: the cap-overflow `format!` (an `OverflowContext` enum that formats only in
  `Display`, which is also what lets `CapOverflow` become `Copy`), the identity-mirror buffer copy
  (an O(1) `mem::swap` at `MirrorSpec::is_identity`), and the attractor's reseed on grid change.
  `core/src/render/gpu.rs` is one home for the bind-entry helpers, the single parameterized
  `fullscreen_pipeline`, **three** fullscreen-triangle vertex preludes (raw NDC / Y-flipped UV /
  un-flipped UV — the pasted stages genuinely disagreed, and a wrong flip is a vertically-mirrored
  effect) and a unit-tested `FixedStep`; `AttractorScene::render` went 228 -> 77 lines with no GPU
  call reordered. Phase 6 closed eight debt items: the frame's `Routing` is decided once in
  `PostChain::begin` and *consumed* by `resolve`, `GeneratorConfig`/`CapOverflow` moved up to
  `scenes/mod.rs` so **no module under `scenes/lines/` names `particles::`**, `cargo doc -p lmv-core
  --no-deps` is **warning-free**, `RoseParams` replaces eleven positional `f32`s, `Palette` drops
  `Copy` at 6 KB, and a **live** segment-cap overflow finally reports itself — edge-triggered on
  entry and once on recovery, because an unthrottled `eprintln!` at 130 fps is I/O on the render
  thread. **Verified at review** rather than taken on trust: **211/211** workspace tests green;
  **`core/tests/` byte-untouched across the whole range**, so "every golden baseline byte-identical,
  no re-bless" — the plan's central claim and the thing Phase 5 could have silently broken — is true
  in fact; `clippy --workspace --all-targets -D warnings` + `fmt --check` clean; `cargo doc`
  warning-free; Phase 5's grep proof re-run independently (one `fullscreen_pipeline`, zero local
  bind-entry helpers, the six surviving `BindGroupLayoutEntry` literals exactly the disclosed set);
  and **an independent non-vacuity check on Phase 3** — inducing `Stage(_) -> Unclaimed` fails three
  tests including `a_dual_live_dissolve_carries_the_outgoing_trail`, a *pixel-level* end-to-end, so
  the chain route is behaviorally covered and not merely unit-asserted. **`lmv-core` line coverage
  measured at 90.51 %**, up from Plan 0032's 90.13 %, so the `COVERAGE_FLOOR: 88` ratchet is safe
  despite the deletions. **Core + standalone only; C ABI untouched (v4); `Scene` trait untouched; no
  new dependency; no preset-visible change.** **Accepted deviations:** Phase 3's routes live on the
  render-layer `Roster` keyed by preset index rather than on `Binding` — a dissolve composites two
  presets in one frame and both need routes, and indexing by preset makes a side's routes
  structurally undriftable; Phase 3's plan note (unknown param "must keep today's behavior exactly:
  silently ignored… Plan 0019's job") was stale and correctly not followed, since 0019 landed and the
  load-time warning exists; Phase 1 shipped `filmstrip_layout` rather than the plan's
  `tile_filmstrip`, keeping the PNG codec out of `lmv.exe`; two approved scope expansions
  (`render/transition.rs` in Phase 5 and two `render/mod.rs` doc links in Phase 6, both postdating the
  plan's file lists and both required for the done-whens to hold literally). **Minors:** (1)
  `presets/README.md` still described the cap drop as surfaced "at load-time-style" — **fixed in this
  close commit**; (2) `render/mod.rs:1007`'s `cap_overflow()` doc still says the standalone surfaces
  it "at load", now false; (3) `poll_cap_overflow` edge-triggers on *presence*, and the configure-time
  overflow takes precedence, so a depth overflow masks a later mirror overflow; (4) `tile_filmstrip`
  itself stays untested; (5) `shot/json.rs`'s `num()` renders `NaN`/`inf` verbatim, which is invalid
  JSON (pre-existing, moved unchanged). **Nits:** `gpu::texture`/`gpu::sampler` hardcode
  `FRAGMENT` visibility while `gpu::uniform` takes it, which is why the attractor's vertex-visible LUT
  entries stayed inline; `render/mod.rs:56` still re-exports `CapOverflow` through `scenes::lines`;
  the lsystem early returns don't reset `mirror_overflow`. **⚠ On-device carry-forward:** Phase 2's
  `#[cfg(windows)]` HWND constructor is **build-checked only** (the plugin is not compiled here — the
  other two paths were verified live at 1592 frames / 165 fps and by the headless capture tests), and
  Phase 4's "resizing no longer restarts the point cloud" needs a real window. **No new ADR** — the
  plan carries out existing decisions. Version **patch 0.16.0 -> 0.16.1** at close (a fix/cleanup
  plan; its one behavior addition completes ADR-0007's never-a-silent-cut contract rather than adding
  a feature).

- [0032 — Testing strategy: full-chain e2e, `shot` CLI coverage, a core coverage ratchet, and a
  pre-push gate](done/0032-testing-strategy-e2e-coverage-and-pre-push.md) — **done 2026-07-26**,
  passed Mode 4 review (**no blockers, no majors**; four minors, two nits). Four `dev` phase commits
  (`332720f` the e2e chain suite, `108e21a` `shot` as a subprocess, `ee89905` the pre-push gate,
  `a4b7045` the coverage ratchet). Answers the three questions that opened it — coverage threshold,
  e2e tests, happy paths covered — with a number instead of an opinion. **`core/tests/chain.rs` is
  the first test that crosses the seam CLAUDE.md opens with**: synthetic PCM into a real
  `audio::intake` SPSC pair in 20 ms capture-callback-sized bursts, drained through `pop_samples`
  into a fixed scratch with the shell's own `pump_audio` policy, into a real `Analyzer`, into a real
  `Renderer`, to real pixels. Four claims: band routing survives the ring, determinism holds
  **byte-identically** across it, an oversized push is lossy-not-fatal (fills the ring exactly,
  never splits a frame, never blocks), and `intake` rejects 4 kHz / 0 ch / 9 ch with the exact
  `FormatError` and no panic. **`standalone/tests/shot_cli.rs` runs the built binary as a user
  does** — closing the fact that the CLI the `preset-author` lane self-verifies through had **no
  tests of any kind** (`#[test]` does not run in an `examples/` target). `shot` stayed an example
  rather than a `[[bin]]` (ADR-0033 Alternative E — a `[[bin]]` gets no dev-dependencies, so the
  `image` PNG codec would land in the shipped `lmv.exe`), located by walking `current_exe()`'s
  ancestors for the `examples/` sibling. Four GPU-free cases run everywhere; three rendering cases
  skip on an **adapter-error match rather than an OS check**, so an adapterless Windows runner is
  handled too. `.githooks/pre-push` runs `fmt` + `clippy` + a narrowed `nextest` in **~28 s**,
  opt-in per clone (`git config core.hooksPath .githooks`), printing the nine GPU-heavy suites it
  skipped. A `windows-latest` `coverage` job gates `lmv-core` line coverage behind
  **`COVERAGE_FLOOR: 88`** against a measured **90.13 %**, in one `env:` key with the ratchet rule
  in a comment. **Verified at review** rather than taken on trust: 166/166 green in the hook's
  narrowed set in 22 s wall and `nextest list` reporting 180 total (the README's "166 of 180" is
  exact); the coverage gate re-run locally at **exit 0 / 90.13 %**; `clippy --all-targets -D
  warnings` and `fmt --check` clean; **manifests untouched**, so "no new Cargo dependency" is true
  in fact (hand-rolled escape-aware JSON helpers bought it, and they carry their own negative-case
  unit test); the hook is mode **100755** in the index so it executes on a POSIX clone. **The
  band-routing test is non-vacuous, and more strongly than its own doc claims**: neutralizing the
  *treble* bindings drops the difference to 0.0243 and **fails** the 0.05 floor, while neutralizing
  the *bass* bindings still passes — so the treble half is load-bearing and the test cannot be
  satisfied by bass alone. **Core+standalone tests and CI only; no production code, no C ABI change
  (v4), no `Scene` change.** **Phase 3 deviated with the user's approval:** the plan's guessed
  narrowing list (`golden`, `attractor`, `reaction_diffusion`, `background_composite`, `ink`) is
  worth ~8 s of the ~98 s full gate and misses the real bottlenecks (`reactivity` 89 s, `animation`
  73 s, `sanity` 46 s, `distinctness` 41 s); the hook excludes the **measured** nine, and the plan
  text now says so. **Minors, three fixed in the close commit:** (1) the coverage job's summary step
  ran `cargo llvm-cov report` unscoped, which re-reports every object in the profile data —
  including `lmv-ring` — so the table under the "floor: 88 %" heading was not the number the gate
  enforces (confirmed locally; now `-p lmv-core`); (2) `ci.yml`'s own header comment still said
  "build, test, clippy, fmt" and "GPU rendering ... out of CI scope", the exact sentence this plan
  corrected in `docs/nfr.md` §7; (3) `CLAUDE.md`'s layout tree did not know `.githooks/` exists;
  (4) plan-text drift — Phase 1's done-when named `-E 'test(chain)'`, a *name* filter that matches
  three unrelated `render::post` tests and silently skips two of the four chain claims
  (`binary(chain)` is the suite selector) — corrected in the plan. **Nits:** `scratch()` never
  cleans up its `target/shot-cli-tests/<pid>-*` directories; the overflow claim asserts analyzer
  frames rather than *rendered* frames (which is what keeps it GPU-free — a net gain).
  **First-push discoveries:** the `coverage` job and the three GPU `shot_cli` tests have never run
  in CI, so whether `windows-latest` satisfies `shot`'s hardware-adapter request and how far
  instrumentation stretches the job are open (the suite degrades to a printed skip either way).
  **Phase 5 (`human`) is half done** — `core.hooksPath` is set in the user's clone; the refused-push
  half is observable on their next push. **Baseline for the followup coverage plan:**
  `render/overlay_font.rs` 0.00 %, `render/overlay.rs` 30.69 %, `render/context.rs` 34.71 %,
  `ffi.rs` 56.60 %, `diag/mod.rs` 65.75 %. [ADR-0033](../adrs/0033-testing-strategy-coverage-ratchet-and-pre-push-gate.md)
  **accepted**. **Version: no bump, deliberately** — every commit in the range is chore-class
  (`test`/`test`/`build`/`ci`), no production code shipped, so `docs/releasing.md`'s docs/chore-only
  rule applies; stays **0.16.0**.

- [0023 — Cross-preset visual transitions: MilkDrop-style dissolves between presets](done/0023-cross-preset-transitions.md) —
  **done 2026-07-26**, passed Mode 4 review (**no blockers, no majors**; five minors, one nit). Five
  `dev` phase commits (`2a40f83` ink leaves the chain, `4fefce3` the walking-skeleton controller +
  frozen crossfade, `918ae89` the blend library, `5ab441a` adaptive dual-live + the budget governor,
  `9c0d468` every switch path + re-entrancy + docs). A preset switch is no longer an index bump: an
  engine `Transition` drives it as a **dissolve** over ~1 s on the injected `dt` (no wall clock, so a
  captured show reproduces frame-for-frame), through a **deterministic rotation** over four blend
  kinds — crossfade, additive burn, luma-dissolve, wipe — each a variant of one two-input shader that
  **samples both sides** rather than alpha-compositing, which is what lets the additive line/particle
  families blend without colour corruption. Every switch path goes through it: `Space`, the browse
  overlay's pick, the director auto-rotate, and the C ABI `lmv_cycle_scene`. Policy is two code
  constants (`TRANSITION_KIND`, `TRANSITION_DURATION_SECS`); preset-declared `[transition]`,
  beat-quantized dissolves, and operator UI stay deliberate follow-ups.
  **The composite is now `background -> scene -> PostChain (trails -> kaleidoscope) -> [blend] -> ink
  -> present`** ([ADR-0032](../adrs/0032-ink-leaves-the-chain-blend-between-chain-and-ink.md), now
  **accepted**): ink left the chain to become a terminal engine post-pass symmetric with `Background`
  as the pre-pass, so the two-input blend can sit before it without widening ADR-0031's one-input
  `PostStage` trait. `STAGE_COUNT` is 2 and the chain is exactly the **per-preset** look; the blend and
  ink are the **engine-wide** passes the renderer drives. Ink's `ink_*`/`paper_*` crossfade by the same
  `t` — the poles interpolate in RGB inside the shader, feeding **one** remap of the blended frame
  rather than mixing two remapped frames (the non-linearity ADR-0032 rejects). **Adaptive fidelity**
  ([ADR-0024](../adrs/0024-cross-preset-transitions.md), now **accepted**): the outgoing side re-renders
  live through its own `CompositeSide` (background + a second `PostChain`) only when the two presets
  hold independent GPU state — `scenes::shares_resources`, which vetoes both the same `SystemKind` and
  any two of the three line scenes sharing one `LineRenderer` — **and** the smoothed frame time shows
  positive evidence of headroom under `DUAL_LIVE_BUDGET_MS = 18.0`; otherwise the frozen opening
  snapshot carries it. Demotion latches for the rest of the dissolve, so the mode cannot flicker. A
  zero frame-time reading (diagnostics off — every headless capture) is read as neither headroom nor
  overload, which is what keeps a capture free of any clock-dependent choice.
  `Renderer::select_preset_now` is the **instant-cut escape** (a path, not the code constant the plan
  suggested), used by the capture entry points so captures stay a pure function of their inputs
  (NFR §6). Re-entrancy: a switch mid-dissolve snap-finishes the one in flight and starts the new one;
  a hot-reload cancels cleanly to whatever `set_presets` resolves the active index to.
  **Core-only; C ABI untouched** (`lmv_cycle_scene` gains dissolves through its unchanged signature);
  **no new dependency**; `Scene` untouched. Verified cold at review: 137/137 `nextest -p lmv-core`,
  `clippy --workspace --all-targets -D warnings` clean, and **every golden baseline byte-identical with
  no re-bless** — Phase 1's behavior-preserving claim held in fact. The dual-live trail check
  **requests the default adapter and skips on software**: building the blend's pipeline + targets
  mid-run deterministically changes what the trails stage resolves to on WARP (the ADR-0016 posture
  `background_composite` already takes), and it was verified red against an induced restart bug.
  **Minors** (all carried, none blocking; full text in the plan's Close section): (1) `begin_transition`
  captures `from`/`to` **before** the snap-finish, so a second switch arriving inside one frame
  interval desynchronizes `from_index`, the snapshot and the roster, and `cycle_preset` absorbs the
  press; (2) the standalone's post-switch `warn_cap_overflow` reads the **outgoing** preset, so an
  oversized incoming L-system's truncation is announced one switch late (ADR-0007's "never a silent
  cut" slipping — the title's equivalent staleness self-corrects); (3) the stateful-incoming hitch was
  neither pre-warmed nor documented and `render/mod.rs:589` now claims the opposite; (4) `dissolve_mode`
  is untested (both its pure inputs are covered, the composition is not); (5)
  `docs/on-device-validation.md` gained no item for `DUAL_LIVE_BUDGET_MS`, which the code itself calls
  "the number to calibrate on a low-end rig". **Nit:** a resize mid-dissolve rebuilds the blend targets
  and discards the frozen snapshot, so that dissolve fades up from black. **⚠ On-device carry-forward:**
  the heavy attractor <-> reaction-diffusion pair holding 60 fps @ 1080p on a low-end iGPU, and the
  `DUAL_LIVE_BUDGET_MS` calibration — a WARP capture cannot speak to either. Version **minor
  0.15.1 -> 0.16.0** at close.

- [0030 — Composite chain + scene keying: a `PostStage` trait, an instantiable `PostChain`, and
  kind-keyed scenes](done/0030-composite-chain-and-scene-keying.md) — **done 2026-07-25**, passed
  Mode 4 review (**no blockers**; three minors, one nit). Four `dev` phase commits (`023777d` trait +
  pure routing + chain, `55c7109` the two-chain independence proof, `9c02953` kind-keyed scenes,
  `d711760` docs). `draw_frame`'s ~70-line composite branch ladder over `trailing`/`kaleidoing`/
  `inking` is gone: the three post stages sit behind a crate-internal `PostStage` trait in a
  `PostChain` array whose order is a **compile-time constant** (ADR-0018's feedback-then-fold plus
  ADR-0028's ink-last, pinned by `debug_assert`s so reordering the literal trips in a debug build).
  The routing adjacency is a **pure function** `route(&[bool]) -> Routing` with no GPU and no `self`,
  giving the composite its first real coverage — eight tests including the two invariants asserted
  over all eight flag combinations. `Routing` is fixed-size and `Copy`, so deciding a frame's routing
  allocates nothing. **The Plan 0023 unblock is proven, not assumed**: two chains built against one
  device accumulate independently (B comes back black after A's trail, then reproduces A's pixels
  byte-for-byte through the same history). `system_slot`'s magic-index lookup is deleted — scenes are
  keyed by `SystemKind` via an exhaustive `match` factory over a single `SystemKind::ALL` roster typed
  `[SystemKind; VARIANT_COUNT]`, so roster drift is a **compile error**, and `golden.rs`'s duplicate
  `SYSTEMS` list retired onto it. **Every golden baseline is byte-identical with no re-bless** — the
  plan's central claim, verified. 115 core tests green, clippy clean, C ABI untouched (v4), `Scene`
  trait untouched, no new dependency. ADR-0031 **accepted**. Version `0.15.0` → **`0.15.1`** (patch:
  production code ships, no feature, no behavior change). Open minors routed to [0031]: cache the
  `Routing` at `begin` so one routing decision per frame is structural; ten pre-existing `cargo doc`
  intra-doc-link warnings (all outside this plan's file list — `dev` correctly stopped at the scope
  boundary); the stale `schema.rs:137` routing narration. The debug overlay's p99 under the ~4 new
  vtable calls per frame is **unmeasured** — it needs a live window, so it goes on the on-device pass,
  not the close.

- [0019 — Preset expression grammar v2: branching, math functions, tempo, typo warnings](done/0019-preset-grammar-v2.md) —
  **done 2026-07-25**, passed Mode 4 review (**no blockers**; one major, three minors, one nit). Five
  `dev` phase commits (`c4f76fc` math functions + constants, `c33e996` comparisons + `select`,
  `b36a3de` `tempo`/`novelty`, `462422b` warn-but-load unknown params, `66b1abb` the `docs/presets.md`
  rewrite). The preset expression language roughly doubles, on the walls the `preset-author` lane
  actually hit (ADR-0020, now **accepted**): `cos`, `sqrt`, `pow`, floored `mod` (`mod(-0.2, 1.0)` is
  `0.8`, so a cyclic hue never jumps), `smoothstep`, the constants `pi`/`tau`, the six comparison
  operators at a **new lowest-precedence tier** each yielding a clean `1.0`/`0.0`, and
  `select(cond, x, y)` — which evaluates **only the taken branch**, so `select(x >= 0, sqrt(x), 0)` is
  safe where a `lerp` blend of both sides would poison the parameter with `NaN`. No boolean operators
  by design (`min` is and, `max` is or, `1 - c` is not). `VAR_NAMES` grows 7 → 9: `tempo` (the
  already-computed `AnalysisFrame.bpm`, which `render/mod.rs` simply never passed) and experimental
  `novelty` — **no new DSP**, both already pure functions of the input window, so determinism holds.
  Phase 4 kills the silent-typo footgun: each system declares a `PARAMS` const beside its `set_param`
  match, gathered by `SystemKind::param_names()`, and `is_known_param` unions in the four **global**
  compositing vocabularies (`bg_*`, `trails`, `kaleido_*`, `ink_*`/`paper_*`) so a curated preset
  setting a backdrop is not warned at. An unknown name is a **warning, not a rejection** (NFR 10) —
  the preset loads, the binding is kept, and `Preset::warnings` / `LoadReport::warnings` carry it to
  the standalone, which prints it on load and on every hot-reload. **Verified at review** rather than
  taken on trust: `cargo test -p lmv-core` green with the `preset` suite **22/22** and every new
  assertion body read — the math tests pin exact values (`smoothstep(0,1,0.25) == 0.15625`, the
  floored-`mod` wrap) *and* totality on degenerate input (`sqrt(-1)` NaN, `mod(1,0)` NaN, `e0 == e1`
  smoothstep stays bounded); the `select` test proves the untaken `sqrt(-1)` does not reach the
  result; the variable test binds a **distinct value per slot** so a mis-wired slot cannot
  coincidentally pass; `clippy --workspace --all-targets -D warnings` clean; the hot-path panic pragma
  on `expr.rs` intact (no new `unwrap`/indexing — the new `Call` arms use slice patterns and the eval
  falls back to `0.0`). The drift guard ADR-0020 asked for is real and **verified to fail on induced
  drift**: `declared_params_match_set_param` reads each `set_param`'s arms back out of the source, so
  it covers the GPU-backed scenes a headless test cannot instantiate, and a companion assert proves
  **every shipped preset is warning-free** so the check cannot cry wolf on the curated library.
  **Core-only; C ABI untouched (v4); no new dependency.** **Accepted deviation:** the plan's Phase 5
  was written against a 17-preset / 5-system library and 8 variables; `dev` documented the **current**
  35 presets / 7 systems and 9 variables, and restructured so the per-system parameter tables live
  once in `presets/README.md` with `docs/presets.md` as the authoritative *expression* reference —
  strictly better than the letter of the done-when. **Major (fixed in this close commit):** the
  required operator-doc sweep missed `README.md`, which still advertised "Ten curated presets ship
  across two systems" and pointed at `docs/presets.md` for "the two systems and their parameters" —
  now count-free and pointing at both docs. **Minors:** (1) `standalone/examples/shot.rs:267`'s
  `report_errors` prints `report.errors` but not `report.warnings`, so the headless CLI the
  `preset-author` lane self-verifies through is the one surface that still swallows a typo; (2) the
  drift guard hardcodes the four **global** stages' expected name lists inline instead of reading
  their `PARAMS` consts (those are `pub(crate)`, unreachable from an integration test), so a
  const-only edit to `background`/`trails`/`kaleidoscope`/`ink` can drift `is_known_param` away from
  `set_param` without failing — the seven per-system lists go through `param_names()` and are fully
  guarded; (3) [Plan 0031](0031-composite-cleanup-and-debt.md)'s Phase 3 note still tells its
  implementer that an unknown param "must keep today's behavior exactly: silently ignored" and that
  "making it a surfaced warning is Plan 0019's job" — true when drafted, stale now. **Nit:** the
  `preset-author` skill's own grammar reference is now a version behind (see the sequence note above);
  user-gated. Version **minor 0.14.0 -> 0.15.0** at close (a feature plan).

- [0015 — Preset-directory override + live iteration](done/0015-preset-dir-override-and-live-iteration.md) —
  **done 2026-07-25**, passed Mode 4 review (**no blockers**; one major, three minors, two nits).
  Three `dev` phase commits (`82d33dc` shared resolver + `LMV_PRESET_DIR`, `9e59211` `shot
  --presets` / `--preset-file`, `45bf613` docs). Editing a **version-controlled** `presets/*.toml`
  is now live in the running app within ~150 ms and in the next headless capture, with no rebuild
  and no relaunch. The per-OS resolver that was hand-copied into `main.rs` and `examples/shot.rs` is
  now one module, `standalone/src/lib.rs` (a new `[lib]` target beside the `[[bin]]`, per ADR-0014):
  `resolve_preset_dir() -> PresetDir::{Override, Default, Unresolved}` tells the caller **where the
  directory came from**, which is what lets the app seed write-if-absent into the per-user default
  and **never** into a user-owned override. `shot`'s precedence is `--preset-file` > `--presets` >
  `LMV_PRESET_DIR` > per-user dir > embedded; the two explicit flags **error** when they come up
  empty (an agent that named a folder wants exit 1, not a silent capture of some other library),
  while levels 3-5 keep degrading downward as the app does (NFR 10). The `[source]` label printed on
  every capture names the winner, so a PNG's provenance is never a guess. `main.rs` also stopped
  keeping its own per-OS copy for `diagnostics.log` / `config.toml` / `soak.log` — they call the
  lib's `preset_data_root` and, deliberately, do **not** move with the override. **Verified at
  review** rather than taken on trust: `cargo test -p standalone --lib` **3/3** with the assertion
  bodies read (they cover override-wins, override-survives-no-data-root, empty-is-unset, the per-OS
  default, and `Unresolved` — a superset of the Phase 1 done-when), and all of Phase 2's done-whens
  re-run live — `--presets presets --preset Aurora` exit 0 `[--presets presets]`, `--preset-file
  presets/fragment_aurora.toml` with **no** `--preset` exit 0, a bad `--presets` and a bad
  `--preset-file` both exit 1, and `--report` labelling `[LMV_PRESET_DIR ./presets]` set versus
  `[on-disk C:\Users\...\Roaming\...]` unset, which is the direct proof that app and `shot` share one
  resolver. Zero `core/` changes in the range; `plugin-foobar/foo_lmv.cpp` is **comment-only** (it
  now says in writing that it is the last independent copy of the path and does not honor the env
  var); **C ABI untouched (v4)**; no new dependency (polling, **no** `notify` — ADR-0014 C).
  [ADR-0014](../adrs/0014-preset-dir-override-for-dev-iteration.md) accepted at this close.
  **Accepted `dev` judgment calls:** the Edition-2024 `unsafe env::set_var` problem is handled by
  factoring the rule into a pure `resolve_preset_dir_from(env, root)` with two env-free tests plus
  **one** test holding both env halves (verified sound — the other tests never read env, and the
  bin's `director`/`overlay`/`capture_mac` tests are a separate process); `--preset` became optional
  for a one-entry roster (the plan's own done-when command needed it); malformed files in a loaded
  directory are now reported on stderr, matching the app. **Major (deferred by user call, rides with
  Plan 0019's close):** `.claude/skills/preset-author/SKILL.md` + `references/render-loop.md` still
  state this plan is unlanded and still teach the manual `%APPDATA%` copy-over dance — the lane this
  feature was built for. Architect cannot apply it (`.claude/skills/**` is denied); user-gated.
  **Minors fixed in this close commit:** `docs/presets.md` still said the poll was ~500 ms in a
  second place, `presets/README.md` still described the copy-into-`%APPDATA%` flow from inside the
  very folder the override targets, and `docs/capturing.md` still named the retired `SCENE_DT`
  (substance was right — headless steps at `scenes::FALLBACK_DT`). **Nits:** the lib's env-test
  SAFETY comment claims no test in the crate reads the environment, three lines above its own
  `preset_data_root()` read (reasoning holds, the sentence overstates); the plan and this index both
  labelled the untouched C ABI "v3" when it has been v4 since Plan 0014 — corrected. **Observation,
  unmeasured:** `poll_presets()` runs in the redraw path (`main.rs:311`) and does a synchronous
  `read_dir` + one `metadata()` per `.toml`, now 6.7x/s instead of 2x/s. Sub-millisecond on a warm
  local dir and pre-accepted by the plan; the uncovered case is a *slow* override folder (network
  share, sync-backed) eating a 16.6 ms frame. If it ever shows in the soak log the fix is an
  off-thread signature scan, **not** a looser interval — that would undo the feature. **Not covered
  by tests, by design:** "the aurora recolors on screen within ~150 ms" is the plan's stated
  on-device visual check. Version **minor 0.13.1 -> 0.14.0** at close (a feature plan).
- [0029 — Attractor resize cost + ink-stage followups](done/0029-attractor-resize-cost-and-ink-followups.md) —
  **done 2026-07-25**, passed Mode 4 review (**no blockers, no majors**; two minors, two nits). Five
  `dev` phase commits (`773d437` resource split, `9b927ea` quantize + aspect-preserving cap,
  `59aa298` ink golden, `dd74d41` rename + doc corrections, `e375c2e` project at the target aspect).
  Closes the two majors from the Plan 0027 review. `Resources` is split along the axis that actually
  varies: `PipelineResources` (four shader modules, every pipeline, the 50k-particle buffer, both LUT
  textures, the uniforms, the layouts and sampler) is built **once and survives every size change**,
  and only `FieldResources` (the `PingPongField` plus the four bind groups referencing its views) is
  rebuilt — so a live window drag costs a texture pair plus four bind groups instead of four WGSL
  compilations per frame. `trail_grid_size` became the whole size policy in one pure function:
  each axis rounds up to a **256 px step** (so most resize deltas cost two integer compares) and the
  cap is a **single scale factor** applied to both axes (the old per-axis clamp squashed a 3440x1440
  ultrawide target to 16:9, which the aspect-ignoring present stretched back — the shape changed
  discontinuously as the window crossed 2560 wide). `core/tests/ink.rs` is the **first behavioral test
  of the ink stage** (ADR-0028 had filed it optional): it asserts `ink_amount = 0` is byte-identical
  to the unbound fixture, then bands the ink-off frame by luminance and requires the band means to
  come back **strictly decreasing with the ends crossing** — the ADR-0028 property itself rather than
  a tuned constant. `Scene::resize` is renamed **`set_target_size`** with a doc that states what it
  actually carries (the trails/kaleidoscope internal grid when those stages are active, not the
  surface) and restates the ADR-0030 compare-first obligation; `presets/README.md` replaces the
  `"0.5"` recommendation with the honest note that a partial amount blends toward the near-black
  source and greys the paper (now agreeing with the warning in `presets/attractor_ink.toml`).
  **Phase 5 was added mid-plan by the Mode 4 review of Phases 1-4:** quantization broke the
  grid-equals-target identity the draw uniform's `aspect` silently relied on, so the attractor drew
  11% too wide at the default 1920x1080 window and 33% too wide at 512x384; the fix projects at the
  `aspect` argument `render` already received and discarded. **Verified at review:** `cargo nextest
  run -p lmv-core` **95/95 green**; `clippy -p lmv-core --all-targets -D warnings` + `cargo fmt
  --check` clean; both new tests read and confirmed non-vacuous, and the Phase 5 assertion confirmed
  to **fail before the fix** by temporarily restoring the grid-ratio projection (reported exactly the
  predicted 1.329 skew, 1.00 after) — it is the suite's first non-square capture, which is why every
  earlier square capture was blind to the defect. `core/tests/golden/attractor.png` is the **only**
  baseline that changed (the 128x128 capture now takes a 256x256 field), correctly scoped and noted
  in its commit. **Core-only; C ABI untouched (v4); no new dependency; no wall clock** (the debounce
  alternative was rejected for exactly that reason), so fixed-size captures stay byte-reproducible.
  The split is enforced by the types — `FieldResources::build` takes `&PipelineResources` and can
  reach only the layouts, sampler and decay uniform, so a pipeline cannot drift back into the rebuilt
  block. ADR-0028 and ADR-0030 were already **accepted** at Plan 0027's close. **Minors:** (1) the
  particle **re-seed** on a grid change (`particles/mod.rs:1403`) is no longer necessary — the buffer
  now survives the split — and is the surviving half of the "fullscreen toggle pops the cloud back to
  its seed scatter" symptom the plan opened with; determinism does not need it (a headless capture
  holds one target size, and a mid-capture stage flip stays a pure function of the frame index), so
  moving `needs_upload = true` into the first-build arm finishes the job; (2) the 256 px quantization
  **floors** at one full step, so every headless capture supersamples (a 128x128 target takes a
  256x256 field — that is why the golden changed) and no test exercises the grid-equals-target path
  any more. **Nits:** (3) `trail_grid_size` was made `pub`, widening `lmv-core`'s public API for a
  test's benefit (consistent with `AttractorFamily` already being public there, so not new drift);
  (4) this row's predecessor carried a stray fifth table column, fixed by this close. **Manual check
  (non-blocking):** the plan's user-visible payoff — a smooth live window drag that keeps the cloud,
  and a stall-free fullscreen toggle — needs a real window to confirm; the 256 px step is one
  constant if it reads wrong on device. Version **patch 0.13.0 -> 0.13.1** at close (a fix-only plan).

- [0027 — Attractor ink-on-paper (engine-wide final tone-remap) + crisp trails](done/0027-attractor-ink-and-crisp-trails.md) —
  **done 2026-07-25**, passed Mode 4 review (no blockers; two majors and four minors, all routed to
  [Plan 0029](0029-attractor-resize-cost-and-ink-followups.md) or [ADR-0030](../adrs/0030-scene-target-size-hot-path-hook.md)
  rather than reworked). Three `dev` phase commits (`0e3b84a` ink stage, `5f79dc6` surface-sized trail
  field, `5daddfa` curated preset + docs). Delivers the "ink on paper" look the `preset-author` lane
  could not reach: `render/ink.rs` is a final, skippable composite stage that reads each pixel's
  luminance as an *ink density* and repaints the finished frame to `mix(paper, ink, d)` — the
  **darkening** step the additive scene pipelines structurally cannot express. Default poles are
  white/black, so `ink_amount = 1` alone is a pure black-on-white invert, and `paper_*`/`ink_*` give an
  arbitrary duotone; it works on **every** scene because it operates on the composited frame, and it
  sits before the text/overlay passes so the HUD is never inverted. Phase 2 replaced the attractor's
  fixed 640x360 accumulation grid with one sized to its render target (capped 2560x1440), killing the
  soft upscale at 1080p+. **Verified at review:** `cargo nextest run -p lmv-core` 90/90 green
  (including `golden`, `sanity`, `animation`, `reactivity` over the new `attractor_ink` preset);
  `clippy -p lmv-core --all-targets -D warnings` clean; the Phase 1 passthrough claim confirmed —
  `core/tests/golden/attractor.png` is the only baseline the plan touched, so the `fragment_field`/
  `swarm` restore after an over-broad `LMV_BLESS=1` run was correct. **C ABI untouched**, no new
  dependency. The plan's "no `Scene`-trait change" line did **not** hold: Phase 2's ask for a
  resolution derived from the target size had no channel to travel on (`Box<dyn Scene>`,
  `Scene::render` receives only `aspect`), so a default-no-op per-frame target-size hook was added
  with the user's approval — the third widening and the first on the hot path, now recorded in
  ADR-0030, which replaces ADR-0007/0021's "this is the last one" countdown with three conditions any
  future widening must meet. The two majors: that undocumented widening, and a full GPU-resource
  rebuild (shaders and pipelines included) on every size change inside `render`. ADR-0028 accepted at
  close; version bumped 0.12.0 -> 0.13.0.

- [0025 — Full composite coverage: background + view transform for reaction-diffusion and attractor](done/0025-full-composite-coverage.md) —
  **done 2026-07-24**, passed Mode 4 review (no blockers, no majors; one minor, one nit). Five `dev`
  phase commits (`06b4007` RD alpha-present, `ae17d57` RD zoom/pan, `265045b` attractor alpha-present,
  `566fcf8` attractor zoom/pan, `6c570ec` docs) plus the pre-cleared `8d0e17a`
  (`Renderer::adapter_is_software()`). Finishes ADR-0018's engine-wide promise: both fullscreen/
  accumulating scenes switched their final present from opaque `REPLACE` to
  `PREMULTIPLIED_ALPHA_BLENDING` over the `bg_*` backdrop (RD alpha = the V-field `structure` term;
  attractor alpha = accumulated luminance), so coral voids / attractor negative space reveal the
  tintable gradient — and both now accept `zoom`/`pan_*` via `set_param` (RD in its present-pass sample
  UVs, the attractor in its world projection), exactly as `fragment_field` does. **Verified:** the
  `reaction_diffusion.png` / `attractor.png` golden fixtures are **byte-identical** (neither binds
  `bg_*`, so premultiplied-over-black equals the old opaque present) — the "re-bless" the plan
  anticipated was a confirmed no-op, pinned by the unchanged golden suite. View transform isolated +
  determinism-checked in the two scene tests (same field/seed, `zoom`/`pan` → `frame_diff > 0.02`,
  reproducible captures); the backdrop reveal asserted on real hardware (dual-hue differential,
  void-masked tint tracking) and skipped on the WARP software adapter in the new
  `core/tests/background_composite.rs` (the one added test file + the `adapter_is_software` accessor,
  both pre-cleared as outside the plan's file list). **No `Scene`-trait change; C ABI untouched;** no new
  dependency; hot-path-safe. `fragment_field` correctly stays fullscreen-opaque (bg_* no-op there, now
  documented); `mirror_*` stays line-only by design. **Minor:** stale "fullscreen opaque pass" comment
  on the RD present (`reaction_diffusion.rs:1048`) — swept into Plan 0027's tidy-up list. **Nit:** the
  plan's "goldens re-blessed" done-when wording diverges from the byte-identical reality (a
  strictly-better outcome). Unblocks Plan 0027 (hard-sequenced after it). [ADR-0026](../adrs/0026-full-composite-coverage-fullscreen-scenes.md)
  now **accepted**. Version **minor 0.11.0 -> 0.12.0** at close.

- [0028 — Parametric-curve shape params: radial offset + phase (audio-morphable rose geometry)](done/0028-parametric-curve-shape-params.md) —
  **done 2026-07-24**, passed Mode 4 review (no blockers, no majors; one minor, one nit). Two `dev`
  phase commits (`f37dde0` Phase 1 — core sampler + scene + tests; `20cd7f7` Phase 2 — docs). Added
  two named zero-defaulted per-frame **shape** params to `parametric_curve` (ADR-0029, now
  **accepted**, supplements ADR-0007): `phase` (radians inside the sine) and `radial_offset` (added
  to the radius), so the Maurer sampler becomes `r = sin(n*theta + phase) + radial_offset`. Threaded
  through the existing `reset_params`/`set_param`/`DEFAULT_*` machinery in `parametric.rs` into an
  extended `maurer_rose(...)` in `curves.rs` (new args grouped by role: `phase` by the frequency
  inputs, `radial_offset` by `scale`). Unlocks the reference's spiral/rosette/annular family and
  phase-morph as **audio-bindable** levers (bind to `bass`/`bar`/`beat`, and `tempo` once 0019 lands),
  not just color. Both **default 0.0** — a no-op reducing to the plain `sin(n*theta)` rose — so every
  shipped rose preset and the `parametric_curve` golden fixture stay **byte-identical** (no re-bless,
  the property a dedicated test pins). Verified: the 6 `curves` unit tests green — incl.
  `zero_phase_and_offset_reduce_to_the_plain_sine_rose` (the no-op pin), `radial_offset_shifts_the_
  radius_by_a_constant`, `phase_changes_the_geometry`, plus a `capture_preset` binding test in
  `render/mod.rs` (`shape_params_reach_the_parametric_scene`) proving both evaluated values thread
  into rendered geometry under a bass stimulus; `golden` **unchanged** (no re-bless, confirming the
  no-op); `hygiene` confirms the `curves.rs`/`parametric.rs` panic pragmas intact; `clippy -p lmv-core
  --all-targets -D warnings` clean. **Core-only; C ABI untouched; `Scene` trait untouched; no new
  dependency;** hot-path-safe (two pure `f32` adds, no indexing/division/allocation). **Minor:** Phase
  1 also edited `core/src/render/mod.rs` (+52) for the required Done-when-#3 binding test — expected
  (the capture harness lives there), just absent from the phase's "Files touched" list. **Nit:** Phase
  2 documented both `presets/README.md` (the live table) **and** `docs/presets.md` — the plan targeted
  only the latter, which was stale (no `parametric_curve` section); per the user both were updated now
  rather than deferring to Plan 0019's rewrite (which carries them forward). **`preset-author`
  followup (non-blocking):** revise the `rose_maurer_sweep`/`rose_overflow`/`rose_beat_bloom` drafts
  (untracked in the working tree) to use the new params and flag the strongest as a `dev` embed
  candidate. Pre-existing unrelated `every_preset_animates_over_time` Aurora failure (fragment_field,
  motion 0.0078) is not part of this plan. Version **minor 0.10.0 -> 0.11.0** at close.

- [0020 — Shared palette system: gradient LUT, named + custom palettes, bindable color (all four scenes)](done/0020-shared-palette-system.md) —
  **done 2026-07-24**, passed Mode 4 review (no blockers, no majors; two minor, two nits). Six `dev`
  phase commits (`e64908c` shared `core/src/render/palette.rs` + fragment through a 256-entry baked LUT,
  `b518130` custom gradient `stops`, `81ede9e` swarm through the LUT, `53c944e` A/B `palette_mix`
  crossfade, `9281c23` reaction-diffusion + attractor through the palette, `d00ce16` palette-surface
  docs). Landed the shared color axis (ADR-0021, now **accepted**, supplements ADR-0002): a preset
  declares an optional `[palette]` (built-in `name` — `spectrum`/`ember`/`ice`/`mono`/`aurora` — **or**
  custom `stops`, validated at the load boundary) baked once into a 256-entry RGB LUT that **all four**
  shader-colored scenes color through — a 256x1 1D texture for fragment/RD/attractor (attractor samples
  it in the **vertex** shader), the identical CPU array for the swarm. Color is fully bindable via
  layer-2 named params (`saturation`, `hue`, fragment/RD `color_span`/`color_center`, swarm/attractor
  `hue_spread`/`hue_center`) plus an A/B `palette_mix` crossfade against an optional `[palette_b]`.
  Delivered through one thin off-hot-path `Scene::set_palette` hook (the **second** trait widening after
  ADR-0007's `configure`; ADR-0021 flags a third should prompt re-examining the seam). Default
  `spectrum` = the exact prior cosine, so shipped presets are **visually unchanged**. **Scope expanded
  2026-07-23** to fold reaction-diffusion + attractor into Phase 5 (was a followup) on `preset-author`
  coral-trio evidence; docs became Phase 6. **User decision (Phase 5):** reaction-diffusion's present
  used a *different* cosine than fragment/swarm; it was unified onto `spectrum` (its golden re-blessed),
  so RD is the one scene whose default look intentionally shifted — re-authoring the four coral presets
  is the documented `preset-author` followup. Verified: **51 lib tests + `preset` integration green** —
  incl. `spectrum_reproduces_the_prior_cosine` (the no-regression proof: default LUT within 0.01 of the
  analytic cosine at 8 sampled `t`), the Phase 2 malformed-stops/selector-clash load-error suite (never
  a panic; loader keeps the prior preset), `narrow_spread_makes_colour_coherent` (variance-measured
  swarm distinctness), and `palette_mix_crossfades_a_to_b`; `clippy -p lmv-core --all-targets` clean
  with the hot-path panic pragma on `palette.rs`. Only `core/tests/golden/reaction_diffusion.png`
  changed — fragment/swarm/attractor goldens byte-identical, the drift-guard no-regression proof. A DX12
  WARP layout-aliasing fix kept each scene's LUT bind group structurally unique (fragment's LUT moved to
  `group(1)`; RD/attractor LUTs in unique-arity present groups). Palette docs landed as a sibling
  `docs/preset-palettes.md` (Plan 0019 owns the `presets.md` rewrite) + a `presets/README.md` colour
  section, with a worked custom-stops example. **Core-only; C ABI untouched; no new dependency;**
  determinism preserved (bake pure/off-hot-path, `sample` alloc-free). **Minor:** (1) a `[palette]`/
  `[palette_b]` declared on a non-colored line scene (lsystem/star/parametric) bakes and is silently
  ignored by the default `set_palette` no-op — no author feedback that colour config is inert there
  (consistent with `set_param`'s silent unknown-name drop, but a `preset-author` footgun); (2)
  `docs/presets.md` still describes `hue` as an "offset into the looping cosine palette" — now a LUT
  sample coordinate — but that file's rewrite is explicitly owned by Plan 0019, so it was deferred, not
  updated here. **Nits:** (3) the `Scene` trait is now two optional methods past its ADR-0002 shape
  (`configure` + `set_palette`) — a third widening should trigger the seam re-examination ADR-0021 calls
  for; (4) Phase 4's crossfade was demonstrated with a time ramp rather than the plan's `bar` driver
  (the synthetic click track barely sweeps `bar`), a reasonable substitution since the crossfade math is
  unit-tested. **`preset-author` followups (non-blocking):** re-author the field/flock + coral presets
  to exploit named/custom palettes and `hue_spread`/`color_span`; refresh the skill's `systems.md`/
  `grammar.md` colour snapshots; consider OKLab interpolation in the bake (ADR-0021 Alt E). Version
  **minor 0.9.0 -> 0.10.0** at close.

- [0026 — Calmer scene rotation: hold one scene by default, longer dwell, softened drop bias](done/0026-calmer-scene-rotation.md) —
  **done 2026-07-24**, passed Mode 4 review (no blockers, no majors; one minor, one nit). Three `dev`
  phase commits (`f3dab1c` hold-one-scene default, `49600a2` longer dwell + softened drop gate,
  `f4fd2c7` operator docs). Reverses Plan 0009's "lively unattended show" default (ADR-0027, now
  **accepted**): `Rotate::default().auto` flips **`true` -> `false`** so a fresh install (no
  `config.toml`) holds one scene until the operator opts in via the `A` hotkey (`toggle_auto`) or
  `auto = true`; manual `Space` next-scene works either way. When auto **is** on the cadence is calm:
  default dwell **8/40 -> 20/90 s**, and the energy-drop bias is **softened, not removed** — gated
  behind `DROP_GATE_FRACTION` (0.25) of the min->max span past the min dwell (`min + 0.25*(max-min)`
  ~37.5 s at the default), so a drop just after a rotation can't rapid-fire another; the max-dwell
  timer and the novelty nudge are unchanged. The gate is **proportional to the span** (the
  fraction-of-span option the plan's Phase 2 explicitly sanctions over a fixed `DROP_GATE_SECS`), so
  it scales sensibly for custom dwell configs. Every `Rotate` field is `#[serde(default)]`, so a
  pinned `config.toml` keeps its behaviour — only fresh installs get the revised defaults. Docs
  (config.rs module doc + README Controls) state hold-by-default, the opt-in path, and the new
  cadence. Verified: **13/13 `director` tests green** — incl. `default_config_holds_one_scene_but_
  manual_overrides_work` (default config never auto-rotates over 200 s + a drop, yet `force_next`
  returns `Manual` and `toggle_auto` enables it), `steady_passage_rotates_at_max_dwell` (holds to the
  90 s cap), `energy_drop_rotates_earlier_than_the_cap` (a drop past the ~37.5 s gate fires
  `AutoDrop`), and the new `drop_between_min_dwell_and_gate_is_held` (a drop past min 20 but before the
  gate is **held**, asserting the softened gate prevents rapid re-fire). **Standalone-only**
  (`director.rs` + `config.rs` + `README.md`); **core, DSP, and C ABI untouched**; determinism
  preserved (injected-`dt` EMA, no wall clock, no unseeded randomness). **Minor:** the novelty nudge
  (`AutoBoundary`) is still bounded only by the min dwell (can rotate at ~20 s on a strong boundary),
  not by the ~37.5 s drop gate — intended per the plan ("novelty rides the same dwell"), but worth an
  on-device look since a lively `novelty` could feel quicker than the calmed drop path; `track_change`
  is on-by-default and experimental. **Nit:** two pre-existing tests (`auto_off_never_auto_rotates_
  but_manual_still_works`, `toggle_auto_flips_and_reports_state`) still construct explicit `8, 40`
  configs — correct (they exercise behaviour, not defaults), just no longer echoing the shipped
  numbers. **On-device followup (non-blocking):** tune the 20/90 dwell + drop gate during the next
  live soak; optional scene-lock/on-screen rotation-state indicator. Version **minor 0.8.0 -> 0.9.0**
  at close.

- [0018 — Engine-wide visual enrichment: zoom, atmosphere, easing, mirrors](done/0018-engine-wide-visual-enrichment.md) —
  **done 2026-07-23**, passed Mode 4 review (no blockers, no majors; three minor, two nits). Eight
  `dev` phase commits (`bade3eb` shared `ViewTransform` + zoom/pan on line scenes, `0faa087`
  ViewTransform to fragment+swarm, `02b16e6` engine background pre-pass + scenes `Clear`->`Load`,
  `536b8c9` geometry mirror for line scenes, `822cc94` eased params via render-layer one-pole,
  `e67f217` feedback trails, `56d0460` screen-space kaleidoscope, `52673e0` curated presets + doc).
  Landed the **fixed-order engine composite** ([ADR-0018](../adrs/0018-engine-wide-scene-compositing.md),
  now **accepted**) — background pre-pass (owns the clear) -> active scene under a shared
  `ViewTransform` (zoom/pan, applied per family: line vertex shader, fragment sample coords, swarm
  particle positions) -> feedback trails (`PingPongField` max-decay) -> screen-space kaleidoscope
  (dihedral pixel-fold) -> present — every stage individually skippable to a **passthrough** (which
  also sidesteps the DX12-WARP multi-pipeline aliasing, like RD/attractor). Every scene switched
  `Clear`->`Load` so the background owns the frame. Plus the **render-layer easing seam**
  ([ADR-0019](../adrs/0019-eased-parameters.md), now **accepted**): an optional per-`(preset,param)`
  one-pole (`alpha = 1 - exp(-dt/tau)`) on Plan 0014's injected `dt`, between `eval` and `set_param`,
  reset on preset switch **and** capture rebuild — the expression layer stays pure/zero-alloc; a
  `[smoothing]` table (`param = seconds`, `tau = 0` default = today's instant behaviour) is validated
  non-negative/finite at load. The **geometry mirror** is line-only (segment replication under
  N-fold rotation + optional reflection *before* the cap, surfacing a per-frame `CapOverflow` through
  a new defaulted `Scene::mirror_overflow()` query); the **kaleidoscope** is the general pixel-fold —
  both ship, per the product decision. Six curated presets embedded (`rose_zoom`, `rose_atmosphere`,
  `rose_kaleidoscope`, `rose_trails`, `fragment_kaleido`, `fragment_smooth`) + a `presets/README.md`
  authoring note for every new param. **Core-only; C ABI untouched; no new dependency.** Verified on
  WARP: mirror `2*pi/n`-rotation invariance + exact cap-drop reporting, one-pole step/converge/reset,
  smoothed + trailed `capture_preset` **byte-identical recaptures** (NFR §6 determinism holds through
  the stateful stages), the 4 sparse-additive golden baselines re-blessed (lost their incidental clear
  tint, now owned by the default-black backdrop; fragment/RD/attractor byte-identical), embedded-parse
  + `every_preset_animates` green, hygiene pragma covers the three new `render/` files. **Minor:**
  (1) the mirror-overflow branch `format!`s a `String` per frame while an audio-driven `mirror_order`
  sits over the cap (normal fitting path allocates nothing — fix: store `order: u32`, format in
  `Display`); (2) a **live** mirror overflow never surfaces at the standalone (`warn_cap_overflow`
  reads only at load; the per-frame drop is exposed via `cap_overflow()` but nothing polls it live);
  (3) **zoom is semantically inverted** between the fragment field (`zoom>1` = out) and line/swarm
  (`zoom>1` = in) — deliberate, to honor the hard no-regression done-when (inverting the field would
  regress all 5 fragment presets + the golden fixture), documented in `presets/README.md`. **Nits:**
  (4) ADR-0018's "scenes gain no new *required* method" wording undersold the added (defaulted,
  ISP-clean) `mirror_overflow()` query; (5) `draw_calls` counts the passthrough background clear as a
  draw (diagnostic estimate only). **⚠ On-device carry-forward** (non-blocking, standing posture): the
  iGPU 60 fps @ 1080p floor under the passthrough/offscreen cost + active trails/kaleidoscope (NFR §1)
  — belongs on `docs/on-device-validation.md`. Trails/kaleidoscope run at a fixed 16:9 internal
  resolution presented stretched (same documented v1 limitation as the RD/attractor presents). Unblocks
  **Plan 0023** (cross-preset transitions append a blend stage to this composite). Version **minor
  0.7.1 -> 0.8.0** at close.

- [0024 — Single-source the foobar component version + refresh stale plugin descriptions](done/0024-foobar-component-version-single-source.md) —
  **done 2026-07-23**, passed Mode 4 review cold (**no blockers, no majors, no minors** — one nit).
  Two `dev` commits: `08df308` (Phase 1: `build.ps1` reads `[workspace.package].version` from root
  `Cargo.toml` and generates `build/foo_lmv_version.h`; `foo_lmv.cpp` includes it guarded with a
  `0.0.0-dev` fallback and feeds `FOO_LMV_VERSION` to `DECLARE_COMPONENT_VERSION`) and `a8effb9`
  (Phase 2: refreshed both stale scene-description strings). foobar's Components list stops showing a
  frozen `0.1.0` — the component version now **tracks the workspace version** through a build-time
  generated header (ADR-0025, now **accepted**). Verified: the `Cargo.toml` regex is anchored to
  `[workspace.package]` (a member/profile `version` can never match) and `throw`s on a miss;
  `/I "$build"` is on the `cl` line; the header is gitignored and untracked; **no `0.1.0` literal
  remains** and neither description mentions spectrum/pulse/starfield (both name the current families
  — fragment fields, particle swarm, line geometry, reaction-diffusion, attractors). **C ABI axis
  (`LMV_ABI_VERSION`) untouched** — no core/ffi/header change; plugin + build-script + docs only, no
  new tracked file. On-device confirmation that the Components list shows the current version is a
  user check (the plugin can't run headlessly). Version **patch 0.7.0 → 0.7.1** at close — which is
  now exactly the number the plugin will display, by design.

- [0021 — Decouple preset content from code: build-time embedding + single-source system names](done/0021-decouple-preset-content-from-code.md) —
  **done 2026-07-23**, passed Mode 4 review (**no blockers, no majors, no minors, no nits** — a clean
  landing). Three commits: `e1e4f1f` (Phase 1: `core/build.rs` generates `EMBEDDED` from
  `presets/*.toml`), `11798c3` (rustfmt of `build.rs`), `0241b7d` (Phase 2: single-source `SystemKind`
  name↔kind mapping). Shipping a preset stops being a code change: the project's **first build script**
  (zero-dependency std `read_dir` + sort + string emit) globs `presets/*.toml` at build time and emits
  `pub static EMBEDDED: &[(&str, &str)]` as filename-sorted `(name, include_str!(<abs path>))` tuples —
  rustc still embeds the bytes exactly as the old hand-written array did. Drop a `.toml` in `presets/`,
  rebuild, and it ships with **no Rust edit and no count to bump**. The path resolves from
  `CARGO_MANIFEST_DIR` (not CWD, so CI/rust-analyzer agree); `include_str!` paths are absolute and
  `{:?}`-escaped (Windows-safe); `rerun-if-changed` is registered for the directory **and** each file,
  covering add/remove **and** edit. The count assert is now **structural** (`core/tests/preset.rs`:
  every embedded preset parses, `default_presets().len() == EMBEDDED.len()`, `>= 8` floor) — never an
  exact number. Phase 2 is a behavior-preserving DRY refactor: `SystemKind::from_name` made `pub` and a
  new `as_str` added in `schema.rs` (the one place the mapping lives), so `shot.rs` deletes its two
  duplicate seven-arm matches and keeps only its friendly error text. Verified: the generated set
  reproduces the prior embedded set **exactly** — 22 filename-sorted entries, byte-identical file set to
  the old array (diff clean; `presets/README.md` correctly excluded by the `toml` filter);
  `cargo test -p lmv-core --test preset` 10/10 green (incl. `embedded_default_presets_all_parse`);
  `clippy -p lmv-core -p standalone --all-targets -D warnings` clean. Per ADR-0022 (now **accepted**):
  **no new dependency, C ABI frozen at v4** (no ffi/abi/header file touched; only `build.rs`, `mod.rs`,
  `schema.rs`, `preset.rs`, `shot.rs`). Simplifies the `preset-author → dev` curation handoff (embedding
  becomes "commit the `.toml`"). **Two documented followups** (neither blocks close): (1) update the
  `preset-author` skill's curation handoff (`references/api-feedback.md`) + ADR-0017's note so they stop
  instructing an `EMBEDDED` array + count-bump edit — **user-gated** (`.claude/skills/**` edits are
  blocked for the assistant); (2) when `docs/presets.md` is rewritten (**owned by Plan 0019**), describe
  the generated embedding instead of the hand-maintained array. Version **minor 0.6.0 → 0.7.0** at close.

- [0016 — GPU compute-particle scenes: strange attractors](done/0016-gpu-compute-particle-scenes.md) —
  **done 2026-07-23**, passed Mode 4 review (no blockers, no majors; two minor, three nits). Five
  `dev` phase commits (`79b6cf0` skeleton, `937fdfb` trails, `9acc415` audio params, `7ec850a`
  family set, `aa34d25` coverage guard + contract). Landed the engine's **first GPU compute
  pipeline** (ADR-0015 idiom B, now **accepted**): a 50k-particle `wgpu` storage buffer stepped
  through a strange-attractor map by a compute shader each frame (`STORAGE|VERTEX|COPY_DST`, read
  back as an instance vertex buffer — no CPU round-trip) and drawn as additive point-sprites with
  fading trails via Plan 0014's `render::feedback::PingPongField` (**no second feedback
  mechanism**). Four families — De Jong + Clifford (2D discrete maps), Thomas + Lorenz (3D
  continuous flows, Euler-integrated + orthographic-projected) — **selected data-driven** via an
  optional `[particles]` table through the existing ADR-0007 `configure` hook: the shared
  `GeneratorConfig` gained a `Particles` variant, so **no new `Scene` trait method**. All knobs
  (`a,b,c,d,size,hue,fade,reseed`) are ADR-0002 layer-2 named params defaulting to the active
  family's coefficients; init is `SeededRng`-seeded and the compute step reads no clock
  (frame-rate independence via the fixed-timestep accumulator, NFR §6). Four curated presets
  embedded (18→22). Phase 5 also **closed the Plan 0022 half-enforced-coverage followup**: a
  `SYSTEMS`-rosters-every-variant guard in `golden.rs` (`VARIANT_COUNT` + an exhaustive
  compile-time reminder), verified to fail when a system is dropped from the drift roster. **Core
  only; C ABI frozen at v4; no new dependency.** Verified: `nextest 68/68` — incl. a dedicated
  `attractor_contract` (byte-identical **seed reproducibility**, **beat perturbation**, animation,
  De Jong + 3D-Lorenz coverage/spread), the attractor golden baseline byte-identical on WARP, and
  preset count 22; `clippy -p lmv-core -p standalone --all-targets -D warnings` clean; `fmt` clean;
  all four families eyeballed distinct via `shot`. **Minor:** (1) the trail field is a fixed 16:9
  offscreen presented stretched (aspect ignored, like the reaction-diffusion present) — correct on
  a 16:9 display, distorted otherwise; (2) the shared `GeneratorConfig` still lives in
  `lines/mod.rs`, so `lines` now references `particles::AttractorFamily` (a future tidy could
  relocate the enum to `scenes/mod.rs`). **Nits:** the coverage guard is compile-*nudged* not
  airtight (`VARIANT_COUNT` is a literal; full rigor needs an enum-iteration dep, out of scope);
  the first cycle to an attractor preset hitches once (lazy GPU-resource build, same as Plan
  0014's cycle-to-Coral); the present pass keeps a redundant clear under a full-screen triangle.
  **⚠ On-device carry-forwards** (non-blocking, like prior plans): 60 fps @ 1080p + low-end iGPU
  compute/additive-fill smoke → `docs/on-device-validation.md` (ADR-0015 Risks); the four family
  presets run hot at high energy — `preset-author`-lane tuning, not engine work. Delivers idiom B
  of ADR-0015's four-idiom catalogue; curl-noise flow fields, fractal flames, and boids remain
  follow-ups on the same compute path. Version **minor 0.5.0 → 0.6.0** at close.

- [0022 — Decouple the golden drift guard from shipped presets (per-system frozen fixtures)](done/0022-golden-fixtures-decouple-content.md) —
  **done 2026-07-23**, passed Mode 4 review (no blockers, no majors; one minor, one nit). Two `dev`
  phase commits (`def9b24` per-system fixtures + repointed golden; `19e7123` engine-vs-content doc
  split). Golden (`core/tests/golden.rs`) previously pinned baselines to three **shipped, curated
  presets** (`Aurora`/`Warp Drive`/`Drift`), so every intended content tune tripped the engine-drift
  alarm and reds CI (concretely `76a2fb4`). Repointed it at six **test-only frozen fixtures** under
  `core/tests/fixtures/` — one per `SystemKind`, keyed by an **exhaustive `match`** with no wildcard
  arm so a new scene fails to compile until its fixture exists — loaded via `set_presets` and
  captured by name; baselines blessed on WARP. Closes the prior **zero** golden coverage of the three
  line-family systems (`parametric_curve`/`lsystem`/`star_pattern`, each feeding the shared line
  renderer through a different generator). The three shipped baselines are **deleted**; no test pins a
  shipped preset by name, and the shipped roster keeps its behavioral floors (`sanity`/`reactivity`/
  `animation`, all iterating `default_presets()`). **Landing this greened `main`** from the `76a2fb4`
  drift. Verified: `cargo test -p lmv-core --test golden` green on WARP with a **real comparison**
  (not an adapterless skip); all six variants have exactly one fixture + baseline. Per
  [ADR-0023](../adrs/0023-golden-drift-guard-uses-frozen-fixtures.md) (now **accepted**). **Test +
  docs only — no production, C ABI (still v4), or `ci.yml` change.** **Minor (non-blocking followup):**
  the `SYSTEMS` iteration list is hand-maintained separately from the exhaustive `fixture()` match, so
  a new variant is compiler-forced to add a fixture *arm* but **not** forced into `SYSTEMS` — coverage
  is only half self-enforced (fix: assert `SYSTEMS.len()` == variant count). **Nit:** `FRAMES = 60`
  warms the stateless line/fragment fixtures needlessly (harmless). **Version: no bump** — zero
  shipped-artifact change (chore-only per ADR-0005/`docs/releasing.md`, a deliberate call, not a miss).

- [0014 — Reaction-diffusion feedback scene + frame-rate-independent render clock](done/0014-reaction-diffusion-feedback-scene.md) —
  **done 2026-07-23**, passed Mode 4 review (no blockers, no majors; two minor, two nits). Six `dev`
  phase commits (`345be23`, `13148b7`, `39b6091`, `cb71057`, `9fcfc95`, `8a05cea`). Landed the
  engine's **first stateful feedback scene** — Gray-Scott reaction-diffusion on a reusable
  `render::feedback::PingPongField` (two `Rgba16Float` offscreen textures, fixed 256² grid) with an
  iso-contour + hatch + cosine-palette present look, driven by named params (`feed`/`kill`/`flow`/
  `inject` + look scalars, ADR-0002 layer 2) so bands steer the regime and beats stamp seeded growth;
  one embedded preset (`Coral`, roster **slot 5**). Threaded real **injected `dt`** through
  `Renderer::render(&frame, dt)` + a no-op-default `Scene::advance(dt)`, **retired `SCENE_DT`**
  (demoted to `FALLBACK_DT` for the ABI wrapper + capture), and made the CPU swarm
  frame-rate-independent (dt-scaled advection + `powf` damping, once/frame). Added **C ABI v4
  `lmv_render_dt`** ([ADR-0013](../adrs/0013-c-abi-v4-render-dt.md), now **accepted**; `lmv_render` =
  the exact `1/60` wrapper), header in lockstep, foobar shim QPC `measure_dt()`; new feedback render
  system per [ADR-0012](../adrs/0012-stateful-feedback-render-system.md) (now **accepted**). Both new
  scenes build GPU resources **lazily on first render** and beat injection folds into the sim shader
  (not a 4th pipeline) — documented DX12-WARP capture workarounds, real hardware unaffected. Verified:
  `fmt` / `clippy -D warnings` clean; `reaction_diffusion_contract` (sanity/animation/reactivity/
  **seed-reproducibility**), `ffi` v4, `preset` count 18, `hygiene` (both new `render/` files carry the
  panic pragma) all green. **Minor:** (1) the present pass ignores aspect (stretches the square grid) —
  an implicit choice the plan asked to document; (2) the lazy build makes the first cycle to `Coral`
  hitch once, against `cycle_preset`'s "never hitches" doc. **Nits:** stale `SCENE_DT` comment in
  `animation.rs:15`; the swarm "byte-identical at 60fps" claim is ULP-optimistic (moot — reproducibility
  holds, 0022 retires the swarm golden pin). **⚠ On-device carry-forwards** (like prior plans): Phase 2
  same-speed eyeball, Phase 4 "reads as the reference family" (dev verified via real-GPU PNGs), Phase 5
  live-foobar plugin `dt` (C++ shim not compiled here). **⚠ `main` stays red on `golden`** (pre-existing
  from `76a2fb4`, blessed cross-GPU; 0014's swarm `dt`-change also perturbs `Drift`) — **Plan 0022
  greens it**, not this close. Version **minor 0.4.0 → 0.5.0** at close.

- [0009 — Live performance features (standalone)](done/0009-live-performance-features.md) —
  **done 2026-07-23**, passed Mode 4 review (no blockers, no majors; two minor deviations, both
  pre-flagged and reconciled). Five `dev` phase commits (`6e048d0` per-user config + borderless-
  fullscreen on a chosen display, `3891272` line-in / audio-interface capture selection, `bb9a1e2`
  drop-biased scene director + hotkeys, `d693c69` experimental track-change novelty nudge, `d49f377`
  `--soak` long-run instrumentation). Made the standalone drive a live DJ show: `Fullscreen::
  Borderless` on the config-selected display (`F`/`D` hotkeys, name-over-index monitor match),
  WASAPI **capture**-endpoint enumeration for line-in alongside loopback (`--list-devices`, graceful
  default fallback), a clock-free **`director`** module (MilkDrop-style dwell timer, drop bias, `A`
  toggle, manual `Space`, all a pure function of injected `dt` + `AnalysisFrame`), and a coarse soak
  sampler (elapsed/fps/RSS/heartbeat every 5 s, off the per-frame path). Core gained **one
  deterministic scalar** — `AnalysisFrame.novelty` from a new `dsp/novelty.rs` (normalized spectral
  distance from a slow running mean; pure, seeded, hot-path pragma) — consumed via the native API as
  a director *nudge* that never triggers alone. Operator choices persist in a per-user `config.toml`.
  **C ABI untouched (still v3), no ADR** (standalone-only; ADR-0001 layering applied). Verified:
  `cargo test -p lmv-core` green (incl. `novelty_spikes_at_a_spectral_boundary` + determinism
  extended to `novelty` bits); 11 `director` unit tests green (timing/bias, drop/novelty-before-min
  holds, steady-never-rotates-on-novelty, disabled-nudge, inverted-dwell clamp); `cargo clippy
  -p lmv-core -p standalone --all-targets -D warnings` clean; hygiene guard covers `dsp/novelty.rs`
  (recursive `dsp/` scan + pragma); `ffi.rs`/`ffi` tests zero-diff. **Minor (reconciled):** (1)
  `config.rs` edited in Phase 2 though its file list omitted it — the `[input]` schema was a genuine
  prerequisite (flagged in the commit body); (2) novelty is **spectral-only**, not the plan's
  "spectral/tempo" — a deliberate narrowing (a beatmatched set holds tempo across the blend, so a
  tempo term would fire on exactly the case the nudge must stay soft on; rationale in `novelty.rs`),
  fully satisfying the distinct-spectra done-when. **⚠ Carry-forwards (on-device, non-blocking, like
  prior plans):** Phase 6 (≥4-hour projector-rig soak, human); Phase 1 multi-monitor fullscreen +
  `F`/`D` + persistence-across-restart + config-delete fallback; Phase 2 line-in reactivity with an
  interface connected (loopback + `--list-devices` smoke-verified live); auto-rotate "feel" tuning
  (`NOVELTY_REF`, dwell/drop constants intentionally in code/config for on-rig calibration).
  Delivers roadmap item 2 (live performance features). Version **minor 0.3.1 → 0.4.0** at close.

- [0017 — Green CI: reasoned ttf-parser advisory ignore + adapter-skip for headless GPU tests](done/0017-ci-green-advisory-and-gpu-tests.md) —
  **done 2026-07-23**, passed Mode 4 review (no blockers, no majors, no minors; two non-actionable
  nits). Two `dev` phase commits (`95bf510`, `134d4e3`) unbreaking `main` (CI run 29985131075) after
  two **environmental** failures. **Phase 1** silenced `RUSTSEC-2026-0192` — `ttf-parser` flagged
  **unmaintained** (not a vulnerability), load-bearing via the glyphon text stack (`ttf-parser →
  fontdb → cosmic-text → glyphon → lmv-core`, both shipped targets, ADR-0009's `text` feature),
  upstream-pinned so undroppable without reversing ADR-0009 or waiting on upstream — with a single
  **reasoned, tracked `advisories.ignore`** entry (id + unmaintained-not-vuln framing + load-bearing
  path + revisit trigger) and corrected the now-false `deny.toml` `[graph]` pruning comment. **Phase 2**
  routed the three headless GPU-capture tests through a shared `headless_or_skip` helper that **skips
  on `RenderError::RequestAdapter`** (macOS lost its Metal adapter; no software fallback) and panics on
  any other build error, per [ADR-0016](../adrs/0016-gpu-tests-opt-in-ci-scope.md) (now **accepted**) —
  keeping full assertions running on Windows WARP every push. Verified: `cargo deny check` exits 0
  (`advisories/bans/licenses/sources ok`); all three tests run their full assertions green on WARP under
  nextest; clippy `-D warnings --all-targets` clean with the production hot-path `#![deny(clippy::panic)]`
  intact (the `clippy::panic` allow is scoped to the `#[cfg(test)]` module only). **Config + test-only —
  no `ci.yml` change, C ABI untouched (still v3), no hot-path surface.** **Followup (standing):** remove
  the ignore once a glyphon/cosmic-text bump no longer pins the unmaintained ttf-parser. **Nits
  (non-actionable):** the `skipped:` notice is stderr-only (nextest hides it on pass) — the exact
  silent-no-op tradeoff ADR-0016 accepts; and the parallel session's untracked `skill-creator/` /
  `skills-lock.json` were surfaced-not-swept.

- [0010 — Line-geometry scenes: parametric curves, L-systems, star patterns](done/0010-line-geometry-scenes.md) —
  **done 2026-07-23**, passed Mode 4 review (no blockers, no majors; three minor, two nits). Five
  `dev` phase commits (`110eab7`, `cd0e518`, `4b9ea05`, `1cc7fa1`, `3e2dcc1`) implementing
  [ADR-0007](../adrs/0007-line-geometry-generators.md) (now **accepted**). Added a **line-art
  category** to the built-in vocabulary on one shared `LineRenderer` (segments → thick glowing
  instanced quads, additive blend, fixed 20k-segment buffer) under two build models: a **parametric**
  system (`ParametricCurveScene`, the Maurer rose sampled per frame, allocation-free into a
  preallocated buffer) and a **generator** system built + cached at preset load
  (`LSystemScene`: grammar rewrite + turtle-walk cached per depth `1..=max_depth`; `StarPatternScene`:
  Hankin contact-angle rosette with precomputed variants). The `Scene` trait grew by **exactly one**
  optional off-hot-path method (`configure(&GeneratorConfig)`, default no-op) — the single widening
  ADR-0007 sanctions — invoked once at preset load from `configure_active_scene`. New optional
  `[curve]`/`[generator]` TOML tables validated at the load boundary (`schema.rs`), 7 curated presets
  (4 roses, 2 L-systems, 1 star) embedded + seeded, and a `presets/README.md` authoring note.
  Verified: `cargo test -p lmv-core` green — grammar exact-string (incl. Koch depth-2), turtle
  cap-truncates-and-reports, Hankin segment-count + 2π/n rotational symmetry, zero-per-frame-alloc,
  and bad-`[curve]`/`[generator]`-config rejection, all present and non-tautological; clippy
  `-D warnings` clean; hygiene guard covers all nine new `lines/*.rs` panic pragmas. **C ABI untouched
  (still v3).** **⚠ Carry-forward (minor, non-blocking):** (1) `LSystemScene::overflow()` and the
  parametric `samples` clamp *track* the segment-cap drop but nothing *surfaces* it at load — the
  plan's "never a silent cut" is unmet in the surfacing half (latent: shipped fern peaks ~6k/20k).
  (2) `presets/README.md` says `max_depth` is "clamped to 1..=7" but schema rejects `>7` as a load
  error. (3) `parametric` `configure` is skipped when `[curve]` is omitted, so `family` doesn't reset
  (harmless with one family). (4) `lsystem_fern` `visible_depth` only bumps depth at `bass == 1.0`
  exactly — an on-device tuning nit. The iGPU 60 fps @ 1080p confirmation is the standing hardware
  carry-forward (`docs/on-device-validation.md`).

- [0013 — Headless scene capture + differential visual QA + golden images + shot CLI](done/0013-headless-scene-capture.md) —
  **done 2026-07-22**, passed Mode 4 review (no blockers, no majors; one minor, one nit). Eight
  `dev` phase commits (`ecc50e5`, `ba68026`, `d11a7f0`, `889f4e3`, `26a3180`, `4b54d1e`, `8152943`,
  `4364464`) plus the `assets/test` gitignore (`a16be92`). Gave the agent a **windowless
  visual-feedback + QA harness**: `RenderContext::new_headless` (a surface-less device+queue, `None`
  surface so the on-surface present path is byte-unchanged) + a shared `draw_frame` extracted from
  `render`, feeding an offscreen `render/capture.rs` (clear-to-black → draw → 256-byte-aligned
  `copy_texture_to_buffer` → blocking map → tight `CaptureImage`). Two pure primitives —
  `capture_preset(name, frame, N)` (constant synthetic frame) and `capture_audio(name, pcm, fmt,
  at[])` (feeds PCM hop-by-hop through the **real** `dsp::Analyzer`, format validated at the intake
  boundary — source-agnostic rule preserved), each rebuilding scenes to their seed + resetting the
  clock so a capture is a pure function of its inputs. A dependency-free `render/metrics.rs`
  (`frame_diff`, recolor-robust Sobel `struct_diff`, `coverage`, `quadrant_spread`) powers **hard
  `core` tests**: per-band `reactivity`, `animation` (N vs N+k), `sanity` (coverage+spread against
  each scene's own sampled background — not tautological), and `beat` (a `core::signal` 120 BPM
  click track through the real DSP; a zeroed-beat-binding probe stays below the floor). Plus an
  **advisory** dual-metric `distinctness` report, `golden`-drift regression (software adapter +
  mean/outlier tolerance + `LMV_BLESS`, three eyeballed baselines), and a `standalone/examples/shot.rs`
  CLI (`--preset/--set/--frames/--size/--out`, `--all` contact sheet, `--report [--json]`,
  `--signal`/`--audio --strip` filmstrips via a hand-rolled 16-bit-PCM WAV reader). `image` is a
  **dev-dependency** in both crates ([ADR-0011](../adrs/0011-image-crate-for-capture-tooling.md), now
  **accepted**); the audio path adds **no** dependency; **C ABI untouched (still v3)**. Verified:
  full `cargo test -p lmv-core` green (18 lib unit + 8 integration binaries), `cargo clippy
  -p lmv-core -p standalone --all-targets -D warnings` clean (lints the example), hygiene guard
  covers the new `render/` files' panic pragma + the `image` exact-pin (`=0.25.10`). **⚠ Phase 9
  (human) outstanding — non-blocking:** the CC0 demo clip. Dev implemented a **safer variant** —
  `assets/test/*` is **gitignored** (tracked README only), so a clip is supplied and used *locally,
  never committed*; the `--signal` path already validates the whole audio pipeline with no asset.
  **Minor:** that gitignore supersedes Phase 9's literal "commit a WAV under `assets/test/`"
  done-when (reconciled in the closed plan). **Nit:** `core/src/signal.rs` is a new top-level core
  module outside the hygiene panic-pragma scan set and carries no pragma — acceptable, since it runs
  at capture-setup time (not per-frame or in the audio callback) and is written slice-index-free, so
  it's within the plan's own "only if it carries per-frame indexing" guidance.

- [0008 — In-app preset browse overlay (standalone)](done/0008-preset-browse-overlay.md) —
  **done 2026-07-22**, passed Mode 4 review (no blockers, no majors). Four `dev` phase commits
  (`3bef1a8`, `b0bb95e`, `43f3b39`, `9cc3234`). Landed the codebase's first text rendering:
  **glyphon** behind a non-default core `text` feature ([ADR-0009](../adrs/0009-glyphon-text-rendering.md),
  now **accepted**) via a reusable `render::text::TextLayer` seam (a second load-pass compositing
  positioned `TextRun`s over the scene in one frame; Plan 0009's HUD reuses it). The standalone draws
  the active preset name on-canvas, plus a Tab-toggled browse overlay: pure window-free `OverlayState`
  (open/highlight/filter → `OverlayAction`) with arrow-nav, case-insensitive substring type-to-filter,
  Enter selecting the **absolute** roster index, Esc closing, and hot-reload re-clamp. Selection landed
  as `Renderer::preset_names`/`select_preset`, both delegating 1:1 to a new crate-private, unit-tested
  `Roster` (the surface-free selection state — mirrors the FrameStats-behind-Diag pattern, since a live
  `Renderer` can't be built headlessly). **C ABI untouched** (`LMV_ABI_VERSION` stays 2); the plugin
  stays cycle-only. Verified: 41/41 tests (3 `Roster` + 11 `OverlayState`, incl. the absolute-index
  and no-wrap asserts) green under nextest; clippy `-D warnings` clean on the `--features text` build;
  `cargo tree` confirms glyphon absent from the default/plugin graph, present only in the standalone;
  hygiene guard covers `text.rs`'s panic pragma and the glyphon exact-pin. **⚠ Carry-forward (human,
  on-box):** Phases 1/3/4 visual done-whens ("legible on canvas", "switches the visual", "narrows
  live") and the NFR 4 binary-size delta (release `lmv.exe` ≈ 7.6 MB with glyphon; the pre-glyphon
  delta wasn't isolated — the standalone hard-enables `text`) are GPU/on-device judgments.
  **Follow-up (not blocking):** a `--features text` core build check in CI (the two-shape build's green
  is a local gate only this session), alongside the standing FFI/Miri CI notes. **Minor:** the browse
  overlay's type-to-filter drops whitespace, so a preset name containing a space can't be fully matched
  (Plan 0007's "filenames only" makes this latent, not live).

- [0012 — Measure the driver-memory floor + cull dead scenes](done/0012-memory-floor-measure-and-scene-cull.md) —
  **done 2026-07-22**, passed Mode 4 review (no blockers, no majors). Two `dev` phase commits
  (`50a7ea0`, `3de5611`); the third phase (human, low-end iGPU) was **extracted** to the standing
  `docs/on-device-validation.md` checklist so the plan could close on completed work rather than wait on
  hardware. **Phase 1** culled the three dead legacy scenes (`spectrum`/`pulse`/`starfield` — built +
  driver-compiled at startup, addressed by no preset; closes the Plan 0003 carry-forward), leaving
  `scenes/mod.rs` at `fragment_field` + `swarm`; measured delta **WS −3.3 MB / private −2.0 MB**, first
  data point that pipeline count is a weak memory lever. **Phase 2** stood up `standalone/examples/floor.rs`,
  a throwaway scene-less wgpu-context spike (construct-only — `RenderContext` exposes only
  `new`/`resize`/`surface_format`, so the example measures at the configure boundary without widening
  core's surface), isolating the fixed driver floor: **~327 MB private commit vs ~338 MB post-cull
  standalone → our whole visual system is only ~11 MB (~3%)**. This resolves ADR-0010's two open items
  (floor-vs-overhead split; pipeline count as a real-but-weak lever) and gave NFR §12 the hard
  per-system denominator it lacked (folded in at close). Verified: `cargo test -p lmv-core` 9/9,
  `cargo clippy --workspace --all-targets -D warnings` clean (lints the example too), no dangling refs
  to the deleted modules, `floor.rs` links into no shipped binary and adds no dependency, dev-box smoke
  rendered all 10 presets at ~165 fps / 0 drops. Core-internal + throwaway example; **C ABI frozen, no
  new ADR.** **⚠ Carry-forward (human):** the low-end iGPU / second-vendor capture — see
  `docs/on-device-validation.md` (does not block anything).

- [0011 — Diagnostics harness + quick-win memory/perf trim](done/0011-diagnostics-and-memory-trim.md) —
  **done 2026-07-22**, passed Mode 4 review (no blockers, no majors; two nits). Seven phase commits
  (`7ad00df`, `166043f`, `5a9f67b`, `1ace817`, `82c7134`, `d266c08`) plus two post-review fixes
  (`10a4796`, `894a2fc`). Built the runtime diagnostics brain in `core`: a pure `FrameStats`
  accumulator (fps / frame-ms / p99 from a fixed 240-sample ring, unit-tested, no clock) wrapped by a
  `Diag` holding the **single gated `Instant::now()` read** — the only wall-clock read in `core`,
  quarantined behind `collecting` so NFR §6 determinism (fixed `SCENE_DT`) holds. A `render/overlay.rs`
  final pass paints a frame-time sparkline + GPU bar + a dependency-free 5x7 bitmap-digit readout as
  instanced quads (off by default, skipped when off). Standalone: F3 toggle, dependency-free per-OS RSS,
  1 Hz rotating `diagnostics.log` on the render thread. Foobar plugin reaches the same overlay + metrics
  over new **C ABI v3** (`lmv_set_debug` + `lmv_get_metrics` + size-guarded `LmvMetrics`,
  [ADR-0008](../adrs/0008-c-abi-v3-diagnostics.md), now **accepted**) — the v3 FFI test rides in with a
  `static_assert(sizeof == 56)` layout guard. Phase 6 landed the NFR §12 levers: wgpu gated to the per-OS
  backend only (DX12/Metal, default-features off, dropping the Vulkan/GL dead weight) and an explicit
  2-frame swapchain latency. `diag/` joined the hot-path panic-pragma guard + `hygiene.rs` scan set.
  **⚠ Phase 7 outcome (human smoke, 2026-07-22, Windows AMD iGPU):** fps unchanged (~165 @ 1080p — no
  §1 regression) and overlay/title parity verified, **but the §12 footprint win failed** — release
  `lmv.exe` measured ~300 MB WS / 343 MB private, *above* the 200 MB baseline. Measured root cause: the
  trim took effect (DX12-only verified, no Vulkan/GL mapped) but footprint is dominated by the DX12
  driver-stack private heap (`amdxc64.dll` 44.8 MB + `d3dcompiler`/`D3D12Core`), which the backend-trim
  can't touch. **Backend-trim retired as the memory lever; §12's <100 MB target likely unreachable on
  DX12/wgpu.** → **Follow-up (new work, does not reopen 0011):** measure the bare wgpu driver floor,
  then revise NFR §12 or profile pipeline/shader count as the real lever. Still-standing on-device
  checks: live-foobar overlay/log (like Plan 0004) and macOS RSS (`rss.rs`, pending a Mac — Plan 0001).
  **Nits (non-blocking):** (a)
  `LmvMetrics.draw_calls` counts render passes, not GPU draw calls — name slightly wider than the value;
  (b) `foo_lmv.cpp` adds a third hardcoded app-dir literal (the Plan 0007 shared-path minor, not new).

- [0007 — Curated preset library: robust loading + seed-on-first-run + C ABI v2](done/0007-curated-preset-library.md) —
  **done 2026-07-22**, passed Mode 4 review (no blockers, no majors). Four phase commits
  (`448b54b`, `ac5e7d0`, `cf8fb5b`, `ed67807`): `core::preset::seed_dir` (write-if-absent) +
  a hand-rolled per-OS data-root resolver in the standalone seed `%APPDATA%\light-music-visualizer\presets`
  on first run, then load + hot-reload it; the foobar shim resolves the **same** dir and calls
  the new `lmv_load_presets` after every `ensure_handle`, gated on an `lmv_abi_version()`
  handshake, so both frontends share one on-disk library. The C ABI grew by exactly one
  function and bumped to **v2** ([ADR-0006](../adrs/0006-c-abi-v2-preset-loading.md), now
  **accepted**) — the first automated FFI test rides in with it (create -> load_presets on a
  temp dir -> assert count + seeded + null-path error), closing the 0001/0002 zero-FFI-coverage
  gap. Curated set expanded 4 -> 10 (calm/warp/bright fragment + drift/dense/storm swarm
  variants). A `pending_presets` stash on `RenderState`, drained by `lmv_attach_window`, handles
  a load-before-attach call order (matching ADR-0006's "install" intent). Selection stays cycle +
  title-bar; the in-app browse overlay is **Plan 0008** (drafting next). Delivers roadmap item 1's
  preset-library thread + part of item 5's install-readiness.
  **⚠ Carry-forward (human):** (a) Phase 3 live foobar smoke — builds x64 Release against v2;
  seeding + Next-scene cycling in a running foobar2000 is an on-device check (Plan 0001 Phase 8
  posture). (b) Phase 4 visual quality — "visibly distinct/reactive" across the 10 presets is an
  on-box judgment. **Minor (non-blocking):** the shared preset-path convention is a string literal
  in both frontends (`standalone/src/main.rs`, `foo_lmv.cpp`) with no single source of truth — a
  rename silently un-shares them; a cross-referencing comment is the follow-up.
- [0004 — foo_lmv as an embeddable Default UI panel](done/0004-foobar-ui-element-panel.md) —
  **done 2026-07-21**, passed Mode 4 review (no blockers, no majors). All four phases landed in
  `plugin-foobar/foo_lmv.cpp` (commits `ef9193f`, `be3f90c`, `49ed225`, `855ccba`): the file-scope
  globals became one claimable `VizSession` (single `LmvHandle` + stream + pump + render timer); a
  Default UI `ui_element` panel and the View pop-out both host the core through one HWND, sharing
  the session so only one wgpu surface exists; ownership arbitration (400 ms poll) hands the session
  to a still-open host when the owner frees, with a GDI placeholder for non-owners; "Next scene" via
  right-click + Space; and a visibility/playback-driven cadence (full while playing+visible, ~6-7 fps
  idle, timer off when hidden). **Plugin-only, no ADR** — diff touches only `foo_lmv.cpp`, the C ABI
  is unchanged (`LMV_ABI_VERSION` still 1, only the pre-existing surface called), and the
  single-`lmv_create` invariant is owner-gated on both create paths. Relates to roadmap item 4 (UX).
  **⚠ Carry-forward:** all four done-whens are runtime checks in a live foobar2000 v2 — the code
  implements each; behavioral confirmation is pending an on-device run.
- [0005 — Extract the lock-free ring into a wgpu-free crate for Miri](done/0005-miri-ring-extraction.md) —
  **done 2026-07-21**, passed Mode 4 review (no blockers, no majors). Implements Plan 0002's
  deferred Phase 5. Phase 1 (`de0fe24`) pulled the SPSC ring — `RingShared`, `SampleProducer`,
  `SampleConsumer`, `spsc()`, and the four SPSC unit tests — out of `core/src/audio.rs` into a
  new zero-dependency `lmv-ring` crate, re-exported unchanged from `core::audio` (public API and
  the C ABI intact). The ring types carry a bare `channels: u16` instead of the core-owned
  `AudioFormat` (which stays at the `intake()` boundary with its validation), driving one
  documented `capture_win.rs` call-site edit — the plan's own Risks-section fallback.
  `hygiene.rs` guards extended to cover `lmv-ring` in both the exact-pin and hot-path-pragma
  checks. Phase 2 (`6af7865`) added the `miri` CI job (`cargo +nightly miri test -p lmv-ring`) —
  fast because no wgpu graph compiles; the probe (Release→Relaxed → data-race UB) confirmed the
  gate bites. No ADR (internal refactor; the rejected feature-gate-wgpu alternative is recorded
  in the plan). **⚠ Carry-forward:** the Miri job's green-in-CI is a runtime check pending the
  push (needs the `workflow` OAuth scope on the git credential). **Minor (non-blocking):**
  `spsc()` is now crate-public in a `publish=false` crate — a slightly wider surface than the
  former module-private constructor; the `channels`-validated-by-caller contract is documented.

- [0003 — Generative scenes + data-driven presets](done/0003-generative-scenes-and-presets.md) —
  **done 2026-07-21**, passed Mode 4 review (no blockers). Phases 0-5 landed (commits
  `ae2c035..df16c48`): scenes relocated under `render/` + brought under the panic-pragma guard
  (closing the 0002 review gap), a fragment-field system and a ~10k CPU particle swarm, DSP
  enriched with bass/mid/treb bands + a deterministic hop-clock tempo/BPM, a pure
  allocation-free expression evaluator, and TOML presets driving both systems with disk
  hot-reload. Implements **[ADR-0002](../adrs/0002-layered-preset-architecture.md) layers 1-2**
  (now **accepted**). Two review fixes at close (`6b7135b`): thread-isolated the zero-alloc test
  so both `cargo test` and nextest pass, and added `preset/expr.rs` to the hygiene guard.
  **⚠ Carry-forward (minor, non-blocking):**
  1. The three legacy scenes (spectrum/pulse/starfield) stay compiled and constructed but no
     preset addresses them - a cleanup candidate (delete, or expose via a `SystemKind`).
  2. Phase 3's iGPU 60 fps @ 1080p validation (NFR 1/9) and the Phases 1/3/5 "visibly flows and
     reacts" done-whens are runtime/hardware checks, not verifiable in review - confirm on the
     iGPU test PC when available.
  **Deferred follow-ups (tracked in the closed plan):** Rhai orchestration (layer 3),
  cross-preset blending, a compute-shader particle path for thousands-scale, additional built-in
  systems (feedback/warp, boids, walkers, 3D), and exposing preset selection across the C ABI.
- [0006 — Versioning: single source of truth + cargo-release + surfacing](done/0006-versioning-wiring.md) —
  **done 2026-07-21**, passed Mode 4 review (no blockers, no majors). Implements
  [ADR-0005](../adrs/0005-versioning-and-release-cadence.md) (now **accepted**): one
  `[workspace.package].version` inherited by both crates, `cargo-release` (`release.toml`:
  `shared-version`, tag `v{{version}}`, `push = false`, `publish = false`) as the single bump
  authority, version surfaced in the standalone title via `env!("CARGO_PKG_VERSION")`.
  Phase 4 (human) confirmed: `cargo-release 1.1.3` installed, dry-run clean. **First bump run
  at close: minor `0.1.0 -> 0.2.0`, tag `v0.2.0`, not pushed** (the user pushes). C-ABI version
  (`LMV_ABI_VERSION`) stays a separate axis; the foobar plugin version remains independent.
- [0002 — Rust enforcement tooling](done/0002-rust-enforcement-tooling.md) —
  **done 2026-07-21**, passed Mode 4 review (no blockers). Phases 0-4 landed and are green
  locally (fmt, clippy `-D warnings`, both hygiene guards, cargo-deny). Panic pragma on all 7
  core hot-path files with reasoned in-bounds escapes; no production hot-path panics.
  **⚠ Carried forward (both now tracked as their own work — no loose ends):**
  1. **Phase 5 (Miri CI job) was DEFERRED, not run** — `lmv-core`'s lib pulls the full
     wgpu/naga graph, so a full-crate Miri job is impractical (>10 min). The ring IS verified
     UB-clean locally (`cargo +nightly miri test -p lmv-core --lib`, all 5 ring tests incl. the
     cross-thread SPSC case, 95 s); only the CI automation was outstanding. **→ Now
     [Plan 0005](0005-miri-ring-extraction.md)** (draft): extract the ring into a zero-dep
     `lmv-ring` crate and run Miri against it.
  2. **Scenes were per-frame render code outside the hot-path pragma set / guard scan.** **→
     Folded into [Plan 0003](0003-generative-scenes-and-presets.md) Phase 0** (amendment):
     relocate scenes under `core/src/render/scenes/` so the guard's existing recursive `render/`
     scan covers them structurally, and add the panic pragma to each — done before 0003 fills
     `scenes/` with heavy per-frame indexing.
- [0001 — Core + standalone MVP, then foobar parity](done/0001-core-and-standalone-mvp.md) —
  **done 2026-07-21**, passed Mode 4 review (no blockers; C ABI recorded in
  [ADR-0003](../adrs/0003-c-abi-v1-surface.md)). Windows standalone + foobar2000 plugin
  smoke-tested; 9/9 tests green.
  **⚠ Carried forward: Phase 10 (macOS validation on real hardware, human) was DEFERRED, not
  run** — the plan was closed early on the user's request with the Mac path still unverified
  on-device (it compiles via CI only). When a Mac is available: run the standalone on macOS
  13+, grant the screen-recording permission, confirm live visuals; report results and route
  any fixes to `dev` (the `capture_mac` path). This is the one outstanding piece of v1.

**Open gap (from the 0001/0002 reviews):** the **C ABI has no automated test coverage** — the
C++ shim is not built in CI, and no in-crate FFI test exists. 0002 armed the pragma and
supply-chain gates but did not add an FFI test (it was never a 0002 phase). A minimal
`lmv_create`/`push`/`free` Rust-side test remains an unassigned candidate for a future plan.
Miri (the deferred 0002 Phase 5) now runs in CI via [Plan 0005](done/0005-miri-ring-extraction.md),
but **only against the ring** — the FFI `unsafe` in `core/src/ffi.rs` is renderer/window-coupled,
stays in `lmv-core`, and is out of the Miri job's scope, so the FFI pointer handling is still
uncovered (its C side remains the Plan 0001 Phase-6 smoke program's job, per ADR-0003).

## Roadmap (agreed 2026-07-21, revised same day for the live-show use case; numbers assigned when drafted)

Execution order after Plan 0001, per the NFR interviews ([docs/nfr.md](../nfr.md)):

1. **Preset / scripting engine** — layered presets per
   [ADR-0002](../adrs/0002-layered-preset-architecture.md): TOML data + expression language
   driving built-in systems (feedback/warp, boids, walkers/growth, 3D scene), with an
   optional budgeted Rhai script for staged per-track arcs (NFR §10). Replaces "scenes are
   Rust code" — Plan 0001's Scene trait becomes the rendering vocabulary presets drive, so
   keep it thin. **Delivered by [Plan 0003](done/0003-generative-scenes-and-presets.md)** (layers
   1-2: fragment-field + swarm systems, data + expression presets); Rhai (layer 3), blending, and
   compute-scale particles remain follow-ups tracked in 0003.
2. **Live performance features** — line-in/audio-interface capture, scene triggers
   (auto-rotate + hotkey/MIDI + experimental track-change detection), fullscreen on a
   chosen display/projector, 4-hour soak stability (NFR §10).
   **Delivered by [Plan 0009](done/0009-live-performance-features.md)** (standalone borderless-
   fullscreen on a chosen display, line-in capture selection, drop-biased scene director +
   hotkeys, spectral track-change novelty nudge on the native `Frame`, `--soak` instrumentation;
   C ABI frozen). **MIDI triggers and the ≥4-hour projector-rig soak run remain** — MIDI is its
   own ADR-backed follow-up; the soak run is a `human` on-device carry-forward.
3. **Adaptive quality + runtime-memory trim** — quality tiers + frame-time governor for the
   60 fps iGPU floor (NFR §1), plus cutting the standalone's ~200 MB working set (NFR §12).
   The memory trim's primary lever — compiling wgpu with only the per-OS backend feature
   (DX12/Metal), dropping the dead Vulkan/GL paths — is a cheap, low-risk win that can
   front-run the full tier system. Both validated on the older iGPU test PC (footprint stated
   before/after; the backend trim must not regress the §1 floor).
   **Front-run by [Plan 0011](done/0011-diagnostics-and-memory-trim.md)** (diagnostics harness +
   the cheap NFR §12 levers, all-three-frontend, C ABI v3 / [ADR-0008](../adrs/0008-c-abi-v3-diagnostics.md)):
   it builds the before/after measuring stick and lands the wgpu-backend + swapchain trims. The
   **adaptive-quality tiers + frame-time governor remain** for a later plan — 0011 explicitly
   does not do them.
4. **Remaining v1 UX** — always-on-top / mini mode, settings persistence (NFR §11;
   fullscreen/multi-monitor land earlier with live features).
5. **Packaging & release** — GitHub release zip: unsigned standalone exe +
   `.fb2k-component` (NFR §8).

Later, unordered: better tempo tracking, preset sharing/library, signed installer.

## Conventions

- **Numbering:** sequential, zero-padded 4 digits. Take the next free number above, then
  bump it here in the same session.
- **Phases:** ordered, each one commit, each tagged `**Owner skill:**` with one value from the
  vocabulary `dev` (all code) or `human` (a task only the user can do). The `dev` skill reads
  this tag at the start of each phase; a missing tag is a Mode 4 review blocker. An optional
  `**Area:**` note (`core` / `standalone` / `plugin`) orients the reader but is not the tag.
- **Skills:** `architect` designs and owns `docs/`; `dev` implements all code. `architect`
  writes and closes plans; `dev` flips `draft → in-progress` at "go" and nothing else in the file.
- **Lifecycle:** `draft` → `approved` (user/architect validated it; ready for `dev`) →
  `in-progress` → `done` (then `git mv` to `done/` and drop from this roster). Review
  happens at plan end, in a fresh `/architect` session — not by the session that wrote
  the code.
