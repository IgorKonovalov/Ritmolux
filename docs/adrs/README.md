# Architecture Decision Records

Numbered, append-only records of decisions that have a rejected alternative worth
remembering. Accepted ADRs are never edited in place — to change a decision, write a new
ADR that supersedes the old one and update the status here.

Rule of thumb: if you can't name an option you're *not* taking, you don't need an ADR —
you need a code comment.

**Next free number: 0038**

| ADR  | Title                                                      | Status   |
|------|------------------------------------------------------------|----------|
| [0001](0001-rust-core-wgpu-cabi-foobar-shim.md) | Rust core, wgpu rendering, C ABI with a C++ foobar shim | accepted |
| [0002](0002-layered-preset-architecture.md) | Layered preset architecture: data + expressions + optional script | accepted (supplemented by 0020) |
| [0003](0003-c-abi-v1-surface.md) | C ABI v1 surface (eight functions; frozen shape + rationale) | accepted (extended by 0006, 0008, 0013 — the shipped surface is **v4**) |
| [0004](0004-living-behavioral-spec-layer.md) | Living behavioral-spec layer: seed two contracts, no gate/ritual yet | accepted |
| [0005](0005-versioning-and-release-cadence.md) | App versioning: SemVer 0.x, one workspace version, cargo-release at plan close | accepted |
| [0006](0006-c-abi-v2-preset-loading.md) | C ABI v2: add lmv_load_presets (seed-then-load); bump to v2 | accepted |
| [0007](0007-line-geometry-generators.md) | Line-geometry generators: cached-build built-in category + instanced-quad line rendering | accepted |
| [0008](0008-c-abi-v3-diagnostics.md) | C ABI v3: diagnostics query (lmv_get_metrics) + debug-overlay toggle (lmv_set_debug); bump to v3 | accepted |
| [0009](0009-glyphon-text-rendering.md) | Adopt glyphon for standalone on-canvas text (feature-gated) | accepted |
| [0010](0010-accept-gpu-driver-memory-floor.md) | Accept the DX12/wgpu driver-stack memory floor; retarget the runtime-memory NFR (§12) | accepted |
| [0011](0011-image-crate-for-capture-tooling.md) | Use the `image` crate (dev-dependency only) for headless-capture PNG I/O and golden compare | accepted |
| [0012](0012-stateful-feedback-render-system.md) | Stateful feedback render system: ping-pong offscreen simulation + fixed-timestep accumulator (Gray-Scott first) | accepted |
| [0013](0013-c-abi-v4-render-dt.md) | C ABI v4: add lmv_render_dt (injected real dt); bump to v4 | accepted |
| [0014](0014-preset-dir-override-for-dev-iteration.md) | Preset-directory override (`LMV_PRESET_DIR`) with a shared resolver, polling over a watcher | accepted |
| [0015](0015-gpu-compute-particle-idiom.md) | GPU compute pipelines for particle scenes; the four render-idiom catalogue (attractors first) | accepted |
| [0016](0016-gpu-tests-opt-in-ci-scope.md) | Headless GPU-capture tests skip when no adapter is present (keep GPU out of the CI contract) | accepted |
| [0017](0017-preset-author-skill-lane.md) | A third skill lane: `preset-author` (preset content, not engine code); two-skill harness becomes three | accepted |
| [0018](0018-engine-wide-scene-compositing.md) | Engine-wide scene compositing: shared view transform + background pre-pass + feedback trails + screen-space post-effects (fixed order, not a render graph) | accepted |
| [0019](0019-eased-parameters.md) | Eased (smoothed) parameters: render-layer one-pole filtering on injected `dt`; expression layer stays pure | accepted |
| [0020](0020-preset-grammar-v2-branching-functions-tempo.md) | Preset expression grammar v2: branching (compares + `select`), math functions, `tempo` variable, soft typo warnings (supplements 0002) | accepted |
| [0021](0021-shared-palette-system.md) | Shared preset-controllable palette system: baked gradient LUT (named + custom stops), bindable color modulation + A/B crossfade (supplements 0002) | accepted |
| [0022](0022-build-time-preset-embedding.md) | Build-time embedding of the preset library: zero-dep `core/build.rs` generates `EMBEDDED` from `presets/*.toml` (drop a file, it ships; no code edit) | accepted |
| [0023](0023-golden-drift-guard-uses-frozen-fixtures.md) | Golden drift guard renders frozen per-system test fixtures (exhaustive `match SystemKind`), not shipped presets; shipped presets keep only behavioral floors | accepted |
| [0024](0024-cross-preset-transitions.md) | Cross-preset transitions: two-input blend stage over the engine composite, adaptive dual-live/freeze, engine-default policy (builds on 0018) | accepted |
| [0025](0025-foobar-component-version-single-sourced.md) | Single-source the foobar component version from the workspace version via a build-time generated header (revises Plan 0006's independent-plugin-version note); C ABI axis untouched | accepted |
| [0026](0026-full-composite-coverage-fullscreen-scenes.md) | Full composite coverage: background + view transform reach reaction-diffusion and attractor via alpha-present-over-backdrop and named-param zoom/pan (extends 0018); mirror stays line-only | accepted |
| [0027](0027-scene-rotation-constant-default-calmer-cadence.md) | Scene rotation: hold one scene by default (auto off), 20/90 dwell, softened-not-removed drop bias (revises Plan 0009 defaults); standalone-only | accepted |
| [0028](0028-final-stage-ink-tone-remap.md) | Final-stage duotone "ink" tone-remap (paper/ink colors) generalizing invert; engine-wide black-on-white via a skippable last composite stage, `ink_*` named params (extends 0018, coordinates with 0024) | accepted |
| [0029](0029-parametric-curve-shape-params.md) | Enrich the Maurer curve family via named shape params (radial offset + phase) so preset audio can morph the rose geometry; not new families or a superformula (supplements 0007) | accepted |
| [0030](0030-scene-target-size-hot-path-hook.md) | Third `Scene` widening: a per-frame target-size hook; hot-path optional methods are now in scope under three conditions (retires the "off-hot-path only" bound from 0007/0021) | accepted |
| [0031](0031-post-stage-trait-instantiable-composite-chain.md) | Post-composite stages behind a `PostStage` trait; the composite becomes an instantiable ordered `PostChain` (fixed order preserved, not a graph) — a third internal seam, unblocks 0024's dual-live | accepted (membership revised by 0032) |
| [0032](0032-ink-leaves-the-chain-blend-between-chain-and-ink.md) | Ink leaves the `PostChain` and becomes a terminal engine post-pass; the two-input transition blend sits between chain and ink, outside the `PostStage` trait (revises 0031's membership, preserves 0028's ordering) | accepted |
| [0033](0033-testing-strategy-coverage-ratchet-and-pre-push-gate.md) | Testing strategy: five named tiers, an end-to-end tier (ring->analyzer->renderer, plus `shot` as a subprocess), a line-coverage **ratchet on `lmv-core` only**, and an opt-in `.githooks/pre-push` fast gate; rejected workspace-wide coverage, a fixed 80% target, an inventory guard, promoting `shot` to a `[[bin]]`, and full-CI-parity in the hook | accepted |
| [0034](0034-internal-resolution-follows-the-target.md) | Internal render resolutions follow the target (256 px quantize, single-scale-factor cap at 1920x1080); the reaction-diffusion **simulation** grid deliberately does not — reconstruction first, one fixed step second — and RD's present sampler wraps a field that was already toroidal (supplements 0012/0018/0031) | accepted (Plan 0033; carries an **Outcome** section — "costs approximately nothing" and the named cheap reconstruction form were both falsified in implementation) |
| [0035](0035-asymmetric-attack-release-easing.md) | Asymmetric easing: `[smoothing]` accepts an `{ attack, release }` pair beside today's scalar, selected by direction in the existing render-layer smoother; rejected a parametric bezier ease, a second `[release]` table, and a stateful `slew()` in the grammar (supplements 0019) | accepted |
| [0037](0037-internal-grid-is-a-resolution-not-a-shape.md) | An internal grid is a **resolution, not a shape**: any pass computing screen-destined geometry takes its aspect from the render target, never from the grid it rasterizes into — the grid's aspect must cancel out of the picture; rejected matching the grid's aspect to the target (defeats the quantization step), letterboxing the present, and a mismatch threshold (corrects 0034, generalizes Plan 0029 Phase 5) | proposed |
| [0036](0036-preset-reachable-spectrum.md) | Preset-reachable spectrum in three separable steps: a scalar `bin(x)` over the **already-computed** 64-band log spectrum, an N-element `spectrum` line system on the existing `LineRenderer`, and per-element evaluation last; rejected `spectrum[i]` indexing, N flat `band*` variables, and a GPU spectrum texture (deferred, not refused) | proposed |
