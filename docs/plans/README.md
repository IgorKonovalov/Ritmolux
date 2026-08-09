# Plans index

The one-minute "what's in flight" view. Read this first each session instead of
re-deriving state from `git log`. Completed plans move to `done/`; their full
close write-ups move to [README-archive.md](README-archive.md).

**Next free number: 0075** (ADRs are a separate sequence — next free there is **0089**.)

## Active roster

Only plans still in `docs/plans/`. A closed plan leaves this table entirely —
`Recently closed` below and `done/` both already record it. Each row carries at
most two sentences of **live constraint**: what a reader needs to decide whether
to pick this plan up. Anything longer belongs in the plan file, which is where
someone who picked it up is reading.

| Plan | Title | Status | Owner | Live constraint |
|------|-------|--------|-------|-----------------|
| [0046](0046-transformed-feedback.md) | Transformed feedback: the past learns to move | **approved 2026-07-30** | dev, human | Unblocked — [0045] landed, so the linear-light pipeline exists. Cheaper after the attractor plans settled `particles/`. |
| [0053](0053-the-suite-stops-blessing-what-warp-gets-wrong.md) | The suite stops blessing what WARP gets wrong | **approved 2026-08-02** | dev, human | Phase 3 is `human` and gates Phase 4; it **is** runnable on this box (the gate is `device_type == Cpu`, not "discrete"). Read [0052]'s and [0055]'s archive entries first — each moved a bind-group layout this plan asserts on, and [0072] added one: **`background-lut-layout` now shares `[Texture, Texture, Sampler]` with `fragment-field-lut-layout`**, with its per-pair evidence already measured and recorded in `core/src/render/background.rs`'s `Background` doc comment. Phase 1's allowlist must be derived from the code and pick that entry up. |
| [0064](0064-the-symmetry-stage-and-the-banded-palette.md) | The symmetry stage and the banded palette | **approved 2026-08-04** | dev, human | Unblocked ([0055] landed), but Phase 4 is `human` and mid-plan, so it does not close in one session. The fold's default is now `tile`, not the disc it was drafted against. |
| [0067](0067-the-curation-route.md) | The curation route | **approved 2026-08-04** | dev, human | Phase 1 first makes the gate worth trusting — all five preset gates synthesize `AnalysisFrame` and none runs the analyzer. Its Phase 1 touches `core/tests/reactivity.rs`, which [0061] moved into the `coverage` job's sole ownership (ADR-0073) — a change there is now proved by one CI job, not two. |
| [0068](0068-why-the-downbeat-rarely-locks.md) | Why the downbeat rarely locks | **approved 2026-08-04** | dev, human | Ships a diagnosis and **no fix** on purpose; `CONFIDENCE_THRESHOLD` does not move. Shares no file with anything; Phase 3 is `human` and mid-plan. |
| [0071](0071-light-that-adds-without-covering.md) | Light that adds without covering (`occlude`) | **approved 2026-08-04** | dev, human | One scalar at the backdrop composite; default `1.0` is byte-identical. Phase 3 is `human` and mid-plan — the default is decided over a **lit** backdrop, because at `bg_bright = 0` the two models are identical. **[0072] has closed underneath it**: a lit backdrop is now the preset's own gradient, so the Phase 3 look decision is taken over sixteen skies that just moved. |

## Recommended execution sequence

Preference, not constraint: **the one hard ordering constraint has been
discharged** ([0055] preceded [0064] and [0055] has closed), so a session may take
any plan above. **No plan here closes in one session** — every remaining one
carries a `human` phase, so plan on bringing the user in rather than picking on
that criterion.

| # | Plan | Why here |
|---|------|----------|
| 1 | [0071] | Small engine change with a mid-plan `human` look; gated by nothing. Its Phase 5 retune wants to run as one pass with [backlog 0038] and [backlog 0058] — all three walk the same shipped set, which [0072] has just re-tinted. |
| 2 | [0064] | After the attractor work rather than concurrent with it: its Phase 3 sample grid renders on `attractor_lorenz`, and a look decision taken against a moving target has to be retaken. |
| 3 | [0053] | Protective rather than additive; moves no pixels. Now carries one more layout pair to allowlist — see its row above. |
| 4 | [0068] | Blocks nobody. Late because it ships no fix, not because it is unimportant. |
| 5 | [0067] | The curation route. `reactivity.rs` now runs only in `coverage` (ADR-0073), so a break there shows up in one job. |
| 6 | [0046] | Touches the attractor's feedback path, cheapest once that file has settled. |

[0045]: done/0045-linear-light-and-bloom.md
[0046]: 0046-transformed-feedback.md
[0052]: done/0052-the-emitter-objects-that-spawn-fall-and-die.md
[0053]: 0053-the-suite-stops-blessing-what-warp-gets-wrong.md
[0055]: done/0055-the-fold-edge-becomes-a-choice.md
[0062]: done/0062-the-chaos-game-grows-a-fern.md
[0065]: done/0065-the-mandala-interior.md
[0066]: done/0066-the-level-lever.md
[0069]: done/0069-the-instrument-that-sees-a-figure-leave-the-frame.md
[0061]: done/0061-the-build-stops-paying-for-what-it-is-not-building.md
[0064]: 0064-the-symmetry-stage-and-the-banded-palette.md
[0067]: 0067-the-curation-route.md
[0068]: 0068-why-the-downbeat-rarely-locks.md
[0071]: 0071-light-that-adds-without-covering.md
[0072]: done/0072-the-backdrop-joins-the-palette.md
[backlog 0038]: ../design-backlog.md
[backlog 0058]: ../design-backlog.md

### The six plans added 2026-08-04, and why they exist

They came from a **backlog sweep**, not from six separate requests: the user asked for the backlog
to be checked, the stale entries retired, and plans made from whatever was left that nothing else
already covered. What that produced is worth stating, because the shape of it is not obvious from
the rows above.

- **Seven entries were retired**, none of which carried a marker saying it was dead — 0015 (the
  half-linear band axis, landed with Plan 0048's second analysis window), 0020's content half (Plan
  0048 Phase 7's 368-gain retune), 0030 (landed in the content lane's own `craft.md`), 0036 (retired
  unfired), 0049 (carried into Plan 0055's judged A/B), 0051 (both `star_*` presets now ship triangle
  waves) and 0007's interior half (specified at last by [0065]). The backlog had been accumulating
  answered questions faster than it was closing them.
- **Five entries stay parked deliberately** — 0009 (informational), 0021 (the slew release, awaiting
  an author who wants it), 0032 (96 kHz, awaiting a report), 0038 and 0058 (content-lane retunes,
  routed not planned), and 0055 (attractor variety, which [0062] partly covers and whose own
  re-check condition just landed).
- **Two of the six plans ship no capability at all.** [0068] ships a diagnosis and explicitly no
  fix; [0069] replaces a measure that was proved not to work. Both are here because the alternative
  — tuning a threshold, or calibrating a statistic that cannot separate the cases — is the move each
  one's ADR exists to refuse.
- **[0066] and [0071] each turned out to move zero pixels**, in both cases by *arithmetic* rather
  than by a chosen default: no golden fixture binds `exposure`, and `occlude` defaults to literal
  `1.0`. Neither was designed for that outcome; both plans check it as a phase failure rather than
  claiming it. **[0066] has since closed and the claim held exactly** — zero baselines modified
  across the whole plan, and it left behind the fixture that makes the premise false going forward
  (`composite_bloom_exposed.toml` is now the suite's only `exposure`-binding fixture), so the same
  reasoning cannot be reused unchecked.


## Standing (not a plan)

- **Plan [0061] Phase 9 — the one verification still outstanding** (2026-08-08). The plan is
  `done` and every `dev` phase landed. **Phase 8 ran and passed the same day**: the foobar plugin
  builds against the extracted `lmv-core-cabi` and `foo_lmv.dll` loads in foobar2000 v2 and
  renders. That closed the one risk [ADR-0072](../adrs/0072-the-c-abi-ships-from-its-own-crate.md)
  carried into C++ link time — the linked artifact renamed to `lmv_core_c.lib`, and CI has no
  plugin job that would have caught a stale path. What remains needs a CI run rather than this
  machine:
  - **Phase 9 — read a cache-warm CI run** (the *second* after the push; the first is a cold
    build, because a `[profile.dev]` edit and a new workspace member each invalidate `rust-cache`
    wholesale). It re-derives `COVERAGE_FLOOR` — currently **91**, measured on a hardware-GPU box
    where CI has WARP — and checks the one property
    [ADR-0073](../adrs/0073-the-windows-ci-critical-path.md) committed to: **`coverage` is the
    longest job**. If it is not, `check (windows-latest)` is build-dominated, and that is the
    single measurement that flips ADR-0073's Alternative A (merge the two Windows jobs) from
    rejected to worth taking — route it back to `architect` as a supplement rather than editing
    the job.
- **[On-device validation — low-end Windows iGPU smoke](../on-device-validation.md)** — a
  hardware-gated checklist, **not** a phased plan and **not** in the roster above: it never blocks a
  plan from closing. Holds the low-end / older Windows iGPU checks (fps floor ≥ 60 @ 1080p; footprint
  on a second GPU vendor) the user can only run once that box is in hand. Ticked when run; deleted when
  empty. Currently home to the extracted Plan 0012 Phase 3 (also covers the identical Plan 0003 Phase 3
  iGPU-fps carry-forward).


## Recently closed

One line per plan. **The full close write-ups — review verdicts, the findings
each close recorded, the properties that outlived the plan — moved verbatim to
[README-archive.md](README-archive.md)** (Plan 0061 Phase 7b). Nothing was
deleted; it simply stopped being loaded into every session's context.

- [0072 — The backdrop joins the palette](done/0072-the-backdrop-joins-the-palette.md) — closed 2026-08-09. Review: no blockers, no majors, four minors, three nits. **The last surface outside `[palette]` joined it**: no cosine copy remains in `background.rs`, and `saturation` / `palette_mix` fan out to the sky through one binding. Two of the plan's own claims were falsified and are recorded in [ADR-0086](../adrs/0086-the-backdrop-colours-through-the-preset-palette.md)'s `Outcome` — **the two fixtures it ordered re-blessed pin no pixels**, and its "fifteen" is 18 by its own grep (16 in scope, 3 moved). Zero golden baselines changed.
- [0061 — The build stops paying for what it is not building](done/0061-the-build-stops-paying-for-what-it-is-not-building.md) — closed 2026-08-08. Review: no blockers, two majors, three minors (all five doc drift, all repaired at the close). **Phases 8 and 9 are `human` and carried forward — see Standing.**
- [0074 — The figure colours by how far it has come](done/0074-the-figure-colours-by-how-far-it-has-come.md) — closed 2026-08-08. Review: no blockers, four minor items (three repaired at the close)
- [0073 — The fern unfurls and colours by what made it](done/0073-the-fern-unfurls-and-colours-by-what-made-it.md) — closed 2026-08-06. Review: no blockers, two minor doc items repaired at the close
- [0065 — The mandala interior: `star_pattern` stops being hollow](done/0065-the-mandala-interior.md) — closed 2026-08-06. Review: no blockers
- [0069 — The instrument that sees a figure leave the frame](done/0069-the-instrument-that-sees-a-figure-leave-the-frame.md) — closed 2026-08-06. Review: no blockers, three minor, one nit
- [0070 — Shaped marks](done/0070-shaped-marks.md) — closed 2026-08-05. Review: no blockers, one minor
- [0066 — The level lever](done/0066-the-level-lever.md) — closed 2026-08-05. Review: no blockers, one minor
- [0062 — The chaos game grows a fern](done/0062-the-chaos-game-grows-a-fern.md) — closed 2026-08-05. Review: no blockers, one major, four minor
- [0063 — The attractor keeps its depth](done/0063-the-attractor-keeps-its-depth.md) — closed 2026-08-04. Review: no blockers, one major, two minor
- [0036 — macOS and Windows release artifacts](done/0036-macos-and-windows-release-artifacts.md) — closed 2026-08-04. Review: no blockers, one minor, one nit
- [0055 — The fold edge becomes a choice](done/0055-the-fold-edge-becomes-a-choice.md) — closed 2026-08-04. Review: no blockers
- [0051 — the scene seam emits premultiplied alpha](done/0051-the-scene-seam-emits-premultiplied-alpha.md) — closed 2026-08-01. Review: no blockers, one major (an operator-doc gap, fixed at close), four minor
- [0039 — Line joins: the stroke stops coming apart at every vertex](done/0039-line-joins.md) — closed 2026-07-28. d Mode 4 review (no blockers; one major, three minors, two nits — nothing fixed in the close commit, the major is deliberately…)
- [0023 — Cross-preset visual transitions: MilkDrop-style dissolves between presets](done/0023-cross-preset-transitions.md) — closed 2026-07-26. Review: no blockers, no majors; five minors, one nit
- [0019 — Preset expression grammar v2: branching, math functions, tempo, typo warnings](done/0019-preset-grammar-v2.md) — closed 2026-07-25. Review: no blockers; one major, three minors, one nit
- [0015 — Preset-directory override + live iteration](done/0015-preset-dir-override-and-live-iteration.md) — closed 2026-07-25. Review: no blockers; one major, three minors, two nits
- [0029 — Attractor resize cost + ink-stage followups](done/0029-attractor-resize-cost-and-ink-followups.md) — closed 2026-07-25. Review: no blockers, no majors; two minors, two nits
- [0027 — Attractor ink-on-paper (engine-wide final tone-remap) + crisp trails](done/0027-attractor-ink-and-crisp-trails.md) — closed 2026-07-25. Plan 0029 or ADR-0030 rather than reworked
- [0025 — Full composite coverage: background + view transform for reaction-diffusion and attractor](done/0025-full-composite-coverage.md) — closed 2026-07-24. Review: no blockers, no majors; one minor, one nit
- [0028 — Parametric-curve shape params: radial offset + phase (audio-morphable rose geometry)](done/0028-parametric-curve-shape-params.md) — closed 2026-07-24. Review: no blockers, no majors; one minor, one nit
- [0020 — Shared palette system: gradient LUT, named + custom palettes, bindable color (all four scenes)](done/0020-shared-palette-system.md) — closed 2026-07-24. Review: no blockers, no majors; two minor, two nits
- [0026 — Calmer scene rotation: hold one scene by default, longer dwell, softened drop bias](done/0026-calmer-scene-rotation.md) — closed 2026-07-24. Review: no blockers, no majors; one minor, one nit
- [0018 — Engine-wide visual enrichment: zoom, atmosphere, easing, mirrors](done/0018-engine-wide-visual-enrichment.md) — closed 2026-07-23. Review: no blockers, no majors; three minor, two nits
- [0024 — Single-source the foobar component version + refresh stale plugin descriptions](done/0024-foobar-component-version-single-source.md) — closed 2026-07-23. passed Mode 4 review cold (no blockers, no majors, no minors — one nit)
- [0021 — Decouple preset content from code: build-time embedding + single-source system names](done/0021-decouple-preset-content-from-code.md) — closed 2026-07-23. Review: no blockers, no majors, no minors, no nits — a clean landing
- [0016 — GPU compute-particle scenes: strange attractors](done/0016-gpu-compute-particle-scenes.md) — closed 2026-07-23. Review: no blockers, no majors; two minor, three nits
- [0022 — Decouple the golden drift guard from shipped presets (per-system frozen fixtures)](done/0022-golden-fixtures-decouple-content.md) — closed 2026-07-23. Review: no blockers, no majors; one minor, one nit
- [0014 — Reaction-diffusion feedback scene + frame-rate-independent render clock](done/0014-reaction-diffusion-feedback-scene.md) — closed 2026-07-23. Review: no blockers, no majors; two minor, two nits
- [0009 — Live performance features (standalone)](done/0009-live-performance-features.md) — closed 2026-07-23. Review: no blockers, no majors; two minor deviations, both pre-flagged and reconciled
- [0017 — Green CI: reasoned ttf-parser advisory ignore + adapter-skip for headless GPU tests](done/0017-ci-green-advisory-and-gpu-tests.md) — closed 2026-07-23. Review: no blockers, no majors, no minors; two non-actionable nits
- [0010 — Line-geometry scenes: parametric curves, L-systems, star patterns](done/0010-line-geometry-scenes.md) — closed 2026-07-23. Review: no blockers, no majors; three minor, two nits
- [0013 — Headless scene capture + differential visual QA + golden images + shot CLI](done/0013-headless-scene-capture.md) — closed 2026-07-22. Review: no blockers, no majors; one minor, one nit
- [0008 — In-app preset browse overlay (standalone)](done/0008-preset-browse-overlay.md) — closed 2026-07-22. Review: no blockers, no majors
- [0012 — Measure the driver-memory floor + cull dead scenes](done/0012-memory-floor-measure-and-scene-cull.md) — closed 2026-07-22. Review: no blockers, no majors
- [0011 — Diagnostics harness + quick-win memory/perf trim](done/0011-diagnostics-and-memory-trim.md) — closed 2026-07-22. Review: no blockers, no majors; two nits
- [0007 — Curated preset library: robust loading + seed-on-first-run + C ABI v2](done/0007-curated-preset-library.md) — closed 2026-07-22. Review: no blockers, no majors
- [0004 — foo_lmv as an embeddable Default UI panel](done/0004-foobar-ui-element-panel.md) — closed 2026-07-21. Review: no blockers, no majors
- [0005 — Extract the lock-free ring into a wgpu-free crate for Miri](done/0005-miri-ring-extraction.md) — closed 2026-07-21. Review: no blockers, no majors
- [0003 — Generative scenes + data-driven presets](done/0003-generative-scenes-and-presets.md) — closed 2026-07-21. Review: no blockers
- [0006 — Versioning: single source of truth + cargo-release + surfacing](done/0006-versioning-wiring.md) — closed 2026-07-21. Review: no blockers, no majors
- [0002 — Rust enforcement tooling](done/0002-rust-enforcement-tooling.md) — closed 2026-07-21. Review: no blockers
- [0001 — Core + standalone MVP, then foobar parity](done/0001-core-and-standalone-mvp.md) — closed 2026-07-21. Review: no blockers; C ABI recorded in ADR-0003

## Roadmap (agreed 2026-07-21, revised same day for the live-show use case; numbers assigned when drafted)

> **2026-07-30: a second, strategic roadmap now exists —
> [docs/roadmap-visual-richness.md](../roadmap-visual-richness.md)** — from the user-requested
> "why is everything dull" architecture review. It diagnoses the five capability caps (single
> quality tier, decay-only feedback, 8-bit additive composite, one-scene/fixed-chain
> composition, starved grammar) and orders the themes R0-R6 that answer them. Item 3's
> remaining half below (quality tiers + governor) is that roadmap's R0. New visual-capability
> plans should cite it.

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
