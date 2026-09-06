# Testing and visual QA

The differential harness that decides whether a change to the engine broke a picture, and the
instrument that shows what the beat estimator was thinking. Both run on the same offscreen render
path the [`shot` CLI](capturing.md) uses; this page is the contributor's half of it.

Nothing here is needed to write a preset. What a preset author reaches for is
[Headless capture and video](capturing.md).

<!-- toc:begin depth=3 -->
- [The `core/tests/` harness](#the-coretests-harness)
  - [The preset sweeps are one test per preset (ADR-0157)](#the-preset-sweeps-are-one-test-per-preset-adr-0157)
  - [What the five preset gates can and cannot see](#what-the-five-preset-gates-can-and-cannot-see)
  - [Golden baselines](#golden-baselines)
  - [The tonemap and pixel-level assertions (Plan 0045)](#the-tonemap-and-pixel-level-assertions-plan-0045)
  - [The display write dithers, and every baseline moved once (Plan 0082)](#the-display-write-dithers-and-every-baseline-moved-once-plan-0082)
  - [A lit backdrop is a distinct test configuration, not a variant (Plan 0051)](#a-lit-backdrop-is-a-distinct-test-configuration-not-a-variant-plan-0051)
  - [Asserting that something *moved* — the feedback fixtures (Plan 0046)](#asserting-that-something-moved--the-feedback-fixtures-plan-0046)
  - [The in-frame geometry fraction, and the four things it cannot see (Plan 0069)](#the-in-frame-geometry-fraction-and-the-four-things-it-cannot-see-plan-0069)
- [The habit for a new scene](#the-habit-for-a-new-scene)
- [`--downbeat-log`: the estimator's terms, one row per beat](#--downbeat-log-the-estimators-terms-one-row-per-beat)
<!-- toc:end -->

## The `core/tests/` harness

Most differential tests render on the **software adapter** (`prefer_software`) so
they hold on any GPU; the exceptions say so below. Run the whole suite:

```bash
cargo nextest run -p rlx-core     # what CI runs (per-test process isolation)
cargo test -p rlx-core            # single binaries only — see the two caveats below
```

> **Use `nextest` for the whole suite**, for two independent reasons.
>
> `preset`'s zero-allocation assertion
> counts allocations through a process-global allocator hook, so it is only
> reliable under nextest's per-test process isolation — under stock `cargo test`
> a concurrently-running test's allocations bleed into the count.
>
> And **stock `cargo test` runs a binary's tests as threads in one process, which
> the GPU tests do not survive.** Several of them build and drop a `Renderer` (and
> so a wgpu device) concurrently, and the driver aborts the process with
> `STATUS_ACCESS_VIOLATION` — `--test transition` every run on WARP, `--lib`
> intermittently at teardown, `--lib render::post` since Plan 0035. **This is a
> runner artifact, not a failing test**: the same binaries pass in full under
> `cargo nextest run`, which gives each test its own process. If a `cargo test`
> invocation aborts with `0xc0000005`, re-run it under nextest (or
> `-- --test-threads=1`) before concluding anything about coverage.
>
> Tests that need
> a real GPU (`background_composite`, and the in-crate dual-live dissolve check in
> `render/mod.rs`) **skip themselves** when only a software rasterizer is present,
> per [ADR-0016](adrs/0016-gpu-tests-opt-in-ci-scope.md). WARP mis-renders both:
> the fullscreen-scene + background pipeline set, and — once a dissolve allocates
> its blend targets mid-run — what the feedback `trails` stage resolves to.

### The preset sweeps are one test per preset (ADR-0157)

The four sweeps that make a claim about a single preset — `animation`,
`reactivity` and `sanity`'s loudness gate — are **generated, one `#[test]` per
shipped `.toml`**, by the same `core/build.rs` glob that embeds the library. A
new preset gets its tests by existing; no Rust is edited. `sanity`'s shape gate
and `distinctness` generate **one test per family** instead, because their claims
are about a family's distribution and its pairwise set and do not decompose
further.

Three things follow that matter when you are reading a red run:

- **A failure names the preset**, not a loop index inside a multi-minute test,
  and `-E 'test(animation_attractor_ink)'` re-runs exactly that one.
- **The per-preset reports print per preset.** `sanity`'s loudness ratio no
  longer arrives as one sorted table — sort a run's lines to rebuild it — and the
  shape sweep's flattest-preset ranking is scoped to the family whose test
  printed it.
- **`-P fast` renders a sample and the full run renders everything.** A preset
  may declare `representative = true` (see
  [`presets/README.md`](../presets/README.md)); `build.rs` marks its generated
  tests `<sweep>_rep_<stem>`, and the `fast` profile selects on that marker. So
  the `dev` lane's per-phase gate renders **24 of the 81** presets, while a bare
  `cargo nextest run --workspace` — what the plan close and CI's coverage job run
  — renders all 81 plus the per-family shape and distinctness tests, which are
  never sampled. ADR-0081's curation gate therefore still sees the whole library.

Individual tests (add `-- --nocapture` to see the printed diagnostics):

| test | kind | asserts |
|------|------|---------|
| `reactivity` | HARD | every preset moves for at least one band (bass/mid/treb/onset), driven by **PCM through the real analyzer** (Plan 0067 Phase 1 — see [what the gates can and cannot see](#what-the-five-preset-gates-can-and-cannot-see) below); prints the per-band vector so a dead single binding — e.g. treble — is visible |
| `animation` | HARD | every preset changes between frame N and N+k **on at least one of two readings** (not frozen). Since Plan 0123 Phase 1 ([ADR-0136](adrs/0136-the-animation-gate-asks-its-question-in-both-readings.md)) the verdict is a disjunction over this gate's own statistic: the **silent** reading below, or a **driven** one — the same `footprint_diff` between the silent capture at frame N+k and one taken against `AnalysisFrame::fully_driven()` at the same frame count, floored at `0.017` against the silent `0.01`. A preset frozen in *both* still fails, and the static control is pinned failing both. Each preset's own test prints which branch carried its pass, because that weakening is real and an unprinted property is one nobody re-reads. Since the sweep became one test per preset (ADR-0157) there is no end of a run to collect the **still images in silence** into one roster at; `shot --presets presets --report`'s `anim` column is where that set is read as a list now, and ADR-0136 carries the Outcome. Still no PCM: the driven capture is a synthesized frame, so the row in the stimulus table below is unchanged in kind. **Scores `metrics::footprint_diff` since Plan 0077 Phase 1** ([ADR-0091](adrs/0091-the-animation-gate-scores-motion-against-the-figures-footprint.md)) — motion over the **union of lit pixels**, with `bg_*` stripped, so a sparse figure is measured against its own footprint rather than diluted by the frame. The rejected fifth-density `emitter_squall` draft passes at 0.1049 where the whole-frame statistic priced it out at 0.0057; a static control still fails on a zero numerator, and both are pinned as a standing non-vacuity test. Two things it still cannot do, both by construction rather than by tuning: **a passing `anim` is not evidence of a *watchable* preset on a still family** — an IFS figure is a photograph, so a slow view pan clears the floor while nothing in the figure moves (design-backlog 0066); and **a rotationally symmetric figure cannot score its own spin** — a figure invariant under rotation by `2*pi/k` produces an *identical* image under that rotation, so its frame difference is zero at **every** resolution and no image-domain statistic lifts it (design-backlog 0009; the `#[ignore]`d resolution ladder from Plan 0067 Phase 1d is the recorded negative result, and it is why `SIZE` never moved). Such a figure must move radially, and that is an authoring constraint, not a gate defect |
| `sanity` | HARD | every preset lights a minimum coverage and spans ≥2 quadrants (not blank, not a dot), and is **not a blot**. **The blot check is a conjunction of two terms, and needs both to convict** ([ADR-0128](adrs/0128-a-tonally-flat-picture-is-a-blot-only-if-it-is-also-structureless.md), settled by [ADR-0130](adrs/0130-the-structural-term-is-boundary-density-and-conditioning-the-population-is-what-made-it-work.md), Plan 0119). Term one is `tonal_flatness` — the share of lit pixels inside one narrow luminance band, capped at `MAX_TONAL_FLATNESS` — asking *does the figure have any tonal structure*. Term two is `metrics::boundary_density` — the share of lit pixels touching an unlit 4-neighbour, perimeter over lit area, floored per system kind by `boundary_floor` — asking *does the lit set have any interior*. A preset fails only when it is flat **and** below its family's floor; either alone convicts legitimate content, which is why neither is a verdict. Term one alone convicted a two-ink print for being two-ink (`fragment_tiledmono`, held out of the set for two plans over it); term two alone would convict 22 of the 43 shipped presets, which the gate prints as a count on every run. The tonal term is Plan 0056 Phase 5: a saturated single-tone mass satisfies coverage and spread completely, which is how a run of attractor presets shipped flat. **What "lit" means moved twice, and neither reading is the preset's own backdrop.** [ADR-0067](adrs/0067-coverage-measures-the-scene-not-the-backdrop.md) strips every `bg_*` binding for this capture, because a `bg_vignette` made the frame's corner its darkest pixel and the backdrop read as a large, well-spread figure. [ADR-0126](adrs/0126-the-sanity-lens-measures-departure-from-the-frames-own-ground.md) then made the reference **the frame's own ground** — the mean tone of its most populous luminance band — rather than a hardcoded black, because a scene that paints its own paper reads full coverage whatever it drew, which made three of the four statistics constants for that content and left an emptying canvas indistinguishable from a broken one. So each statistic now answers *how far does this picture depart from the ground it is drawn on*: `coverage` how much of it does, `quadrant_spread` and `radial_shell_occupancy` where, `tonal_flatness` whether what departs has more than one tone, and `boundary_density` whether it has any interior. **Most floors are measured from the library's own distribution and printed on every run; `boundary_floor`'s default arm is not, and says so.** It is `0.31`, the midpoint of two frozen frames — the `Blown Out` blot at `0.2631` and `fragment_tiledmono` at `0.3602` — because a conjunction's second term is judged only over the frames that failed the first, and that population has two members, one of which is the preset the term exists to admit. Half-the-sparsest would be circular there. The `shape_collage` arm (`0.13`) *is* the ordinary ceremony, on a family [ADR-0123](adrs/0123-a-flat-graphic-scene-paints-its-own-paper-and-composites-opaque-elements-in-one-pass.md) holds under the tonemap knee so it has no additive path to the defect at all. `KNOWN_FLAT` is the exemption roster and it is **empty** — no shipped preset is excused from the blot check |
| `beat` | HARD | a 120 BPM click track through the **real** DSP makes a beat-accent preset render differently on-beat vs off-beat; a zeroed beat binding does not |
| `distinctness` | ADVISORY | prints per-family pixel + shape pairwise matrices and flags near-duplicate geometry; never asserts. Covers **nine of the twelve** shipped families, by a curated array in `core/tests/distinctness.rs`. The report’s unit is a pairwise matrix, so a family needs two or more presets before it can say anything — but that is no longer why the three absent ones are absent: `shape_field` (6), `warp_mesh` (4) and `shape_collage` (4) all ship enough and simply never joined the array, because a new `SystemKind` does not appear in it on its own and nothing fails when one is missing. **Split per family since ADR-0157** — one test each, never sampled, each asserting its own pair count against the shipped set |
| `golden` | HARD (tolerance) | one **frozen fixture per system**, plus the `EXTRA_FIXTURES` escape hatch below, matches its committed baseline PNG within a mean + max-outlier tolerance ([ADR-0023](adrs/0023-golden-drift-guard-uses-frozen-fixtures.md)) |
| `composite` | HARD (tolerance) | the **post stages**, one fixture each and never all at once — `trails`, `kaleido_*`, `bloom_*`, plus one that binds **no** stage and guards the composite's *arithmetic* (its assertion is that no channel of *that fixture* reaches 255 — a claim about the fixture, not a general property of the curve; see the re-bless note below). Captured at **160x100**, a size whose internal grid is *not* the target's shape, so an aspect error is visible ([ADR-0037](adrs/0037-internal-grid-is-a-resolution-not-a-shape.md)) |
| `bloom` | HARD (relative) | the bloom stage's behaviour, beside its baseline rather than in it: halo **energy** rises with `bloom_amount`, halo **extent** rises with `bloom_radius`, the rich tier's deeper pyramid reaches further than the floor's, and the halo is **round**. Captured at **256x256** — square, and load-bearing: the roundness guard is what catches a separable kernel whose two passes step in different units, and it reads 1.001 today against 7.05 under the defect it was written for. No magic numbers: every assertion compares two captures of one fixture differing in one bound param |
| `reaction_diffusion` | HARD | the first stateful-feedback scene: seed reproducibility, regime response ([ADR-0012](adrs/0012-stateful-feedback-render-system.md)) |
| `attractor` | HARD | the first compute-particle scene: seed reproducibility + beat perturbation ([ADR-0015](adrs/0015-gpu-compute-particle-idiom.md)) |
| `line_joints` | HARD (+ tolerance) | a **flagged joint stops leaving a hole** in the stroke ([ADR-0041](adrs/0041-line-joins-are-per-endpoint-on-the-segment-instance.md)): against a purpose-built zigzag `polyline`, a vertex is not a local luminance minimum relative to the segment interiors either side of it. Threshold-free, and captured at **512x512** because the wedge it measures is a fraction of a stroke-width across. The same capture is then pinned to a committed baseline (Plan 0040), since the reported defect had no pixel guard anywhere; the relative claim runs **first, even under `RLX_BLESS`**, so the notch cannot be blessed back in. Bless with `--test line_joints`, which cannot reach the golden roster |
| `attractor_trails` | HARD (tolerance) | the attractor with the engine `trails` stage bound — the attractor's four pipelines and the stage's two in **one command buffer**, which is the densest pipeline coexistence any shipped preset produces and the thing [ADR-0058](adrs/0058-bind-group-layout-collisions-carry-evidence.md)'s hazard keys on. `attractor.toml` binds no trails and every `composite_*` fixture is a line scene, so nothing pinned it before Plan 0053. Captured at **160x100** (a non-square, per ADR-0037) and, like every baseline here, blessed on WARP — so it is **coverage, not evidence of correctness**; ADR-0058's hardware-vs-WARP comparison is the check and this is the drift guard. Its own binary so `RLX_BLESS=1 … --test attractor_trails` can reach nothing else. A second, GPU-free test asserts the fixture still puts *both* accumulations live (`trails` > `fade`, `spin` non-zero), since at or below the scene's own tail the stage is a bit-for-bit passthrough |
| `ink` | HARD | the final tone-remap **inverts** tone, and `ink_amount = 0` is byte-identical to an unbound frame ([ADR-0028](adrs/0028-final-stage-ink-tone-remap.md)) |
| `geometry_extent` | HARD | the **in-frame geometry fraction**, for the four line families *only* ([ADR-0083](adrs/0083-in-frame-geometry-is-measured-at-the-line-renderers-draw-seam.md)): that the diagnostic is **byte-identical** to having it off, and that each of the two frozen over-scaled configurations measures below the shipped preset it was recovered from. **Neither engine-wide nor a threshold** — read the section below before using its numbers |
| lit-backdrop guards (**in-crate**, `--lib`) | HARD (exact) | one per **draw seam**, three of them: `swarm.rs`'s `a_lit_backdrop_survives_where_the_swarm_drew_nothing`, `lines/renderer.rs`'s `a_lit_backdrop_survives_where_the_strokes_drew_nothing`, and `emitter.rs`'s `a_lit_backdrop_survives_where_the_emitter_drew_nothing` ([ADR-0056](adrs/0056-additive-scenes-emit-premultiplied-alpha.md)). Each captures `swarm_lit_backdrop.toml` / `lines_lit_backdrop.toml` / `emitter_lit_backdrop.toml` three ways — lit backdrop, black backdrop, backdrop with the scene contributing nothing — and asserts that wherever the scene wrote no light the backdrop arrives **intact**. Bound **0** rather than a tolerance, because it reads the linear composite; see the section below. The swarm's and the lines' take a **fourth** capture at zero emitted light (Plan 0053 Phase 4), which turns the frame into a direct readout of alpha and widens the line guard's reach from 15 channels to the whole stroke footprint |
| emitter burst (**in-crate**, `--lib`) | HARD (relative) | the emitter is the first scene whose **population** varies, so `emitter.rs`'s `a_spawn_rate_on_onset_bursts_and_then_idles` drives `emitter_onset.toml` through `capture_preset_over` with a silent lead, a six-frame transient and a second of silence, and asserts the frame is dark before, lit after, and dark again by the end. `capture_preset` cannot ask this: it holds one analysis frame for every step, so it can show that a binding is live but never that the shower **empties** when the transient passes ([ADR-0057](adrs/0057-emitter-scene-analytic-ballistics-seeded-individuation.md)) |
| `background_composite` | HARD (**hardware only**) | RD / attractor presents alpha-blend over the `bg_*` backdrop; **skipped** on a software adapter, which mis-renders that pipeline set. **The stated cause of that mis-render was identified and fixed by Plan 0053 Phase 3** — it was `background-bind-layout` colliding with `rd-init-layout` / `fragment-field-uniform-layout` ([ADR-0058](adrs/0058-bind-group-layout-collisions-carry-evidence.md)), and an explicit `min_binding_size` moved WARP onto the hardware numbers for the RD half (`087.612 165.165 156.168` hardware, against a bare layout's `087.543 064.538 …`). **Whether the skip can now be lifted is open and unmeasured**: the attractor half of this test is a different layout group and nothing probed it. The module docs here and in `background_composite.rs` still assert the quirk as live — do not read that as evidence it is, and do not lift the gate without rendering both halves on both adapters |
| `transition` | HARD | every switch path (cycle **and** select) renders intermediate blended frames as a ramp, reproducibly from the injected `dt`; each blend kind shows its own signature; a switch arriving mid-dissolve lands on the last index requested; a hot-reload mid-dissolve cancels cleanly; the heavy attractor ↔ reaction-diffusion pair dissolves on the freeze fallback (set `RLX_TRANSITION_STRIP=<dir>` to also dump filmstrips) |
| `easing` | HARD | `[smoothing]` is observable: a scalar entry measures symmetric and an `{ attack, release }` pair does not, against purpose-built near-linear fixtures ([ADR-0039](adrs/0039-verify-easing-with-a-transient-probe-not-a-committed-clip.md)). Also measures the `spectrum` `curve`↔easing **ordering** both ways round through one renderer — **every** frame count in the suite is gated on `segment_settled` first — the shared probe's window is 180 frames (3 s, 6 τ) because at 96 its own asymmetric arm was truncated, reading 61 where the settled answer is 69 |
| `preset` | HARD | the expression evaluator and TOML schema: exact values, rejection without panic, **zero allocation** per eval, and the `PARAMS` ↔ `set_param` drift guard |
| `dsp` / `ffi` / `hygiene` | HARD | known-signal analysis fixtures; the C ABI across the boundary; the hot-path panic pragma + exact dependency pinning |

**Golden baselines pin frozen fixtures, not shipped presets.** `core/tests/fixtures/*.toml`
is a deliberately minimal preset per `SystemKind`, committed alongside
`core/tests/golden/*.png`; the shipped presets in `presets/` are guarded
*behaviorally* (`sanity` / `reactivity` / `animation`) so the `preset-author` lane can
tune them freely without re-blessing pixels. A new `SystemKind` variant fails
`golden.rs` to **compile** until its fixture exists (exhaustive match, no wildcard arm).

**One narrow exception: `EXTRA_FIXTURES`** (Plan 0063). A *second* fixture for a system
already in the roster, for when the rostered one structurally cannot reach the code under
test. `attractor_depth.toml` opened it, and the list has grown since: the rostered
`attractor.toml` is **De Jong**, and [ADR-0076](adrs/0076-the-attractor-keeps-the-depth-it-already-computes.md)
gives every 2-D family an inverse depth extent of exactly `0.0` — which is precisely what
makes the perspective divide, the distance haze and the depth tint the identity there, so
no edit to that fixture could execute a line of them. The newest, `warp_mesh_shader.toml`
(Plan 0110), earns its place the same way one level up: it is the only fixture anywhere in
the crate that carries WGSL, and `render/scenes/warp_mesh/shader.rs` is built **only** for a
bundle that declares a shader — so no edit to the rostered `warp_mesh.toml`, or to the
bytecode-driven `warp_mesh_milk.toml` beside it, could execute a line of that file either.
The list is **captured after the
roster loop, never interleaved with it**, so every pre-existing baseline renders from the
device state it always did (which matters on WARP, where building GPU resources mid-run
changes what a later capture resolves to). `systems_rosters_every_variant` holds it to the
roster's own two conditions plus one of its own: a stem colliding with a rostered system's
would have the two silently overwrite each other's baseline. The roster stays exhaustive —
ADR-0023 rests on that and this does not weaken it.

`core::signal` (pure, zero-dep) synthesizes the test audio; `core::render::metrics`
(pure) provides `frame_diff`, `struct_diff`, `coverage`, and `quadrant_spread`,
shared by the tests and the CLI report, plus the step-response pair
`frames_to_settle` / `step_response` and the `segment_settled` gate that says
whether either of those two is worth reading.

### What the five preset gates can and cannot see

A preset ships when the behavioral suite is green
([ADR-0081](adrs/0081-the-content-lane-lands-presets-and-architect-curates-the-set.md)), so
what "green" is evidence *of* is worth stating rather than inferring. Five gates
sweep the shipped set, and **one of them drives real audio**:

| gate | where its numbers come from | would it notice a preset that ignores the music? |
|------|------------------------------|--------------------------------------------------|
| `reactivity` | **PCM → the real analyzer** — four `core::signal` clips (60 Hz sine, a mid chord, a 12 kHz tone, a 240 BPM click track) pushed hop-by-hop through `Analyzer` via `Renderer::capture_audio_after_warmup`, which advances the analyzer's warm-up hops without pixels and rasterizes only the measured window | **Yes.** This is the only one. A preset that reads no band moves identically under all four clips and fails |
| `sanity` | one synthesized `AnalysisFrame` | No — it asks whether the frame is lit, spread and tonally structured |
| `animation` | a **zeroed** `AnalysisFrame` held constant, and a synthesized fully-driven one for the second reading — both held constant, neither from PCM | No, by design — it asks whether the picture moves at all, on the scene's own clock **or** against full drive. Neither reading passes a sample through the analyzer, so a preset whose bindings are wired to the wrong band still animates on both |
| `distinctness` | one synthesized `AnalysisFrame`, shared across a family | No — it asks whether two presets look alike |
| `golden` | frozen fixtures with constant params | No — it asks whether the renderer still draws what it drew |

**Four of the five are right to synthesize.** Their questions are about the
*frame* — is it lit, does it move, is it distinct, does it match its baseline —
and a made-up analysis frame answers those correctly and several times faster
than pushing samples would. Converting them would buy nothing and cost the sweep
a multiple of what it costs now. That is a decision, not an omission
([Plan 0067](plans/done/0067-the-curation-route.md), "What this plan does NOT do").
The **price of converting one has dropped** since that decision — Plan 0067 measured
it at ~1.8x when every hop rasterized, and
[Plan 0084](plans/done/0084-two-gates-stop-lying-about-what-they-check.md) removed
the warm-up renders from that figure — but the reasoning above does not turn on
the price, so the decision stands.

**So read a green suite as: the renderer produced a plausible, distinct, moving
frame, and the preset responds to at least one band of real audio.** It also says nothing about
whether the **library** wants another preset like this one; that is a curation judgement made at a
plan's close, not a property a gate can hold. What green
still does not say is that the preset responds *well* — `reactivity` compares a
driven band against silence, and against silence a binding that saturates just
above the noise floor is maximally responsive. That gap is `saturation`'s
(a CPU-only expression walk over a 12 s `dynamic:110` probe), and it is the
second gate that sees real signal even though it renders nothing. `beat`,
`chain` and `dsp` also push PCM, but they test fixtures and the DSP itself rather
than the shipped set.

**A fourth caveat, since Plan 0090: on an `emitter` world, a green gate means
something different at `prewarm = 0` than at `prewarm = 1`, and no gate can see
which.** Every behavioral gate captures 30 frames — half a second — and an
emitter's population *ramps* toward `spawn_rate * lifetime` over a whole lifetime
from an empty pool. `prewarm` back-dates that ramp so the first frame is already
the steady state, which means the gates score the world the author is designing
rather than the first 3 % of it. The same draft, changed in nothing else:

| statistic | `prewarm = 0` | `prewarm = 1` | floor |
|---|---|---|---|
| `sanity` coverage / radial shells | `0.0074`, 0 of 10 — convicted blank | `0.1470`, 10 of 10 — structurally present | `0.25` / 4 shells |
| `animation` footprint motion | `0.0629` | `0.1702` | `0.01` |
| `reactivity` best band | `0.0002` | `0.0195` | `0.02` |

Two things follow. A slow emitter world that fails `sanity` may be failing its
*warm-up* rather than its design — check `prewarm` before touching the look. And
a green row on a prewarmed world says nothing about what the first seconds of a
live set look like, which is the question `prewarm = 0` was answering all along.
Neither the capture length nor any floor moved to accommodate this
([ADR-0104](adrs/0104-the-emitters-source-is-authorable-geometry.md) rejected
that; the warm-up is what got attacked instead).

**A fifth caveat, since Plan 0119: `sanity`'s blot check convicts nothing in the
library today, and it is a landmine rather than a clean bill of health.** The
check is a conjunction — tonally flat **and** structureless — and no shipped
frame fails both, so a regression in its *wiring* would look exactly like a
healthy library. Two limits follow, and reading only the first will mis-price the
gate:

- **A raggeder blot passes the structural term.** `boundary_density` reads
  pixel-scale perimeter over lit area, so a particle field noisier than the
  frozen `Blown Out` fixture has *more* perimeter per lit pixel than a
  composition does and clears the floor while still being a mass of one tone.
  That is the known decay mode, it is the mechanism
  [ADR-0129](adrs/0129-the-structural-term-is-measured-at-composition-scale-not-pixel-scale.md)
  was written to escape, and ADR-0130 accepts it knowingly after measuring that
  the escape route was never open. The margin the term ships on is `1.37x`,
  inside the library's own `0.0440..0.9839` spread.
- **Over half the library is below the structural floor already and is held only
  by the tonal term.** 22 of 43 shipped presets read under their family's
  `boundary_floor` — the whole of `parametric_curve`, `shape_field`'s `Facet`,
  12 of 17 `attractor`, and 7 of 9 `fragment_field` including `Sumi` at `0.1008`.
  They pass because they have tonal structure, not because they have interior. So
  **converting any of them to a two-ink print flips them from passing to
  convicted**, since that raises `tonal_flatness` toward `1.0` and leaves
  `boundary_density` untouched. Each such conversion needs its own floor arm with
  its own derivation before it can ship; for `attractor` the ceremony-derived
  number is `0.0220`, `12x` below the blot and vacuous, so mono attractors and
  blot-catching cannot both be had on this term. The gate prints the full list
  and the count on every run, unasserted — there is no measured basis for a
  threshold on it, and it is expected to move.

If one of the other four ever needs to answer an audio question, `reactivity.rs`
is the pattern: synthesize with `core::signal`, drive
`Renderer::capture_audio_after_warmup`, and keep the clip only as long as the
analyzer's window needs to fill — `WARMUP_HOPS` of the ~40-hop clip publish
nothing at all, so they are fed as warm-up and never rasterized.

**Copying it carries one consequence that is easy to miss** (Plan 0084 Phase 4,
2026-08-13). Skipping the warm-up renders is safe for the *analyzer* — analysis
is a pure function of its window and the render pass never touches it, which
`core/tests/capture_advance.rs` asserts bit-for-bit — but it is **not** a no-op
for the scene. A scene that integrates on the GPU (trails, particles,
reaction-diffusion) now meets the measured window `WARMUP_HOPS` steps colder
than it would have, because those hops used to double as the scene warm-up.
Time-driven scenes are unaffected, since the clock advances either way. When
that change landed on `reactivity` it moved 35 of 36 per-band vectors — all
upward, none regressed, and the tightest headroom in the library roughly
doubled — but a gate copying the pattern should expect its own numbers to be a
fresh baseline rather than comparable to a rendered-warm-up run. Nothing asserts
this; it is documented here and in the two source docstrings because no
instrument in the repo can see it.

### Golden baselines

Golden baselines live in `core/tests/golden/*.png` and are ordinary PNGs
(viewable in the repo / PR diffs). To regenerate them after an intended visual
change:

```bash
RLX_BLESS=1 cargo test -p rlx-core --test golden
RLX_BLESS=1 cargo test -p rlx-core --test composite     # the post-stage baselines
RLX_BLESS=1 cargo test -p rlx-core --test line_joints   # the joined-polyline baseline
```

Only the first of those owns the per-`SystemKind` roster. Every
`composite_*.png` belongs to the `composite` test and `line_joint_zigzag.png` to
`line_joints`; blessing by binary is what keeps the scopes from rewriting each
other. `line_joints` additionally refuses to bless at all while its
local-minimum claim is failing, so a reopened notch cannot be baselined in.
`core/tests/feedback.rs` is deliberately absent from that list: it pins no
baseline at all — see
[Asserting that something *moved*](#asserting-that-something-moved--the-feedback-fixtures-plan-0046).

**Eyeball the regenerated PNGs before committing** — the first baseline is easy
to enshrine wrong. The compare tolerates minor cross-GPU rasterization drift; a
genuine change exceeds it.

> **Eyeballing the baseline is not enough on its own, and Plan 0045 is the
> record of why.** The whole suite captures on WARP, which is documented to hand
> a pipeline another live pipeline's resources
> ([ADR-0021](adrs/0021-shared-palette-system.md) / Plan 0020, the tonemap in
> Phase 3, the bloom blur in Phase 4). A mis-rendered frame at these capture
> sizes can look entirely plausible: Phase 4's bloom halo was 2:1 elongated in
> one draft and smeared into a column of copies in another, and the 160x100
> baseline looked like a reasonable glow under both. **Render the same fixture on
> the hardware adapter at a size large enough to see it** (`shot` uses the
> default adapter, so `cargo run -p standalone --example shot -- --preset-file
> <fixture> --size 512x512` is the check) **and confirm the two adapters agree
> before blessing.** Where they disagree, the hardware one is right and the
> baseline is about to enshrine a driver bug.

### The tonemap and pixel-level assertions (Plan 0045)

Every baseline in the repository was re-blessed once at Plan 0045 Phase 3, when
the composite became linear-light `Rgba16Float` with a tonemap at the surface
boundary. Two things follow for anything that reads pixels here:

- **A capture is downstream of a compressive curve.** The curve is the identity
  below ~0.6 and rolls off above it, so a low- or mid-luminance assertion reads
  what it always read, and a bright one reads lower than the linear value that
  produced it. `composite_overlap` pins that a frame of stacked additive strokes
  rolls off instead of flattening: no channel of *that fixture* reaches 255.
  **Do not read that as "255 is unreachable".** The curve is bounded strictly
  below 1 for every finite input, but the surface write encodes to sRGB and
  *rounds*, so `f(x)` crosses the last byte's midpoint at a linear input of about
  36 at the shipped knee — and `attractor.toml` reaches it on the hardware
  adapter. A suite-wide no-255 gate would fail on correct frames.
- **A backdrop makes a bright pixel's dim channels darker, and that is the curve
  working.** The roll-off scales all three channels by `f(m)/m` off the
  *brightest* one, so adding a red-dominant `bg_*` under a magenta stroke raises
  `m`, drops the scale, and takes blue down with it — measured at up to 15 bytes
  on `composite_bloom` with every post stage off. Any assertion of the form
  "compositing something underneath may only add light" therefore has to be made
  **upstream of the tonemap**, on the linear composite, where it is exact; see
  `a_backdrop_under_an_active_halo_only_ever_adds_light` in
  `core/src/render/bloom.rs` (Plan 0045 Phase 4b).
- **`--report` moved, slightly and measurably.** Re-run over the library at that
  change: reactivity max 0.060 / mean 0.012, animation max 0.042 / mean 0.006,
  coverage max 0.187 / mean 0.010, distinctness max 0.12, reachability
  identical everywhere and every floor still passing. Read that as the scale of
  drift a luminance-model change produces in these columns — not as noise, and
  not as something to re-derive without measuring.

### The display write dithers, and every baseline moved once (Plan 0082)

**All 27 baselines were re-blessed on 2026-08-12**, in one commit that contains
nothing else. If you find that commit in the history and wonder what happened:
the tonemap now adds ±1 **encoded** LSB of triangular noise before the 8-bit
write ([ADR-0096](adrs/0096-the-display-write-dithers.md)), hashed from the
fragment's integer coordinates and divided by the sRGB transfer function's local
slope. It is always on and it is not a param — correct quantization of the
display write is not a look. So every pixel in the engine can shift by one level,
and every baseline did.

**The re-bless is bounded, and that was asserted rather than trusted.**
`round(x + n)` with `|n| ≤ 1` differs from `round(x)` by at most one level, so
the whole change is provable rather than eyeballed. Measured **bless-to-bless**
across all 27 — a control set blessed from the pre-dither commit on the same box
first, because 8 of the 27 rewrite against their committed bytes on a clean local
bless and comparing against the repository would have charged that drift to the
dither:

| | |
|---|---|
| channels compared | 2 049 408 |
| max \|before − after\| | 2 (WARP) / 1 (hardware) |
| channels moved | 11.52 % |
| channels moved by 2 | 0.0103 % |

**The two-level moves are the blessing adapter's, not the amplitude's**, and this
is worth knowing before it is rediscovered. On the hardware adapter the bound is
exactly one: zero of 12 288 channels move by 2 on flat-sweep probes at either end
of the range. On WARP, 212 channels across the 27 baselines move by 2, 88 % of
them below byte 20 and every one skipping exactly one value. WARP is not missing
those code values — an undithered ramp there contains every byte from 6 to 18
with no gaps. DX12 permits tolerance in float-to-sRGB8, and in the steep dark
region WARP's approximation departs from the true transfer function, so a
perturbation sized by the true slope lands two levels away in some places and
fails to move the value at all in others. **Below ~byte 20 a WARP capture is not
a reliable instrument for one-level effects**; take those on hardware.

The guards live in `core/src/render/tonemap/tests.rs`:
`the_dither_is_one_encoded_level_at_both_ends_of_the_range` (the amplitude, which
is what a "tidied away" slope term breaks) and
`the_dither_dissolves_a_dark_ramps_plateaus` (the banding itself, stated as a
ratio against an undithered control resolved in the same run).

> **Two pixels per 8-bit level is the SAFE case, not the dangerous one.** Plan
> 0080 Phase 7 reasoned the other way — it called a quarter-frame fade at roughly
> two pixels per level "the classic Mach-band configuration" — and the arithmetic
> is inverted. Banding lives where one level lasts a *long* time, which is the
> **flattest** part of a curve, so a dense packing is the healthy state and a
> `bg_ramp_gamma` below 1 (a long dim tail) is where to look. That plan is closed
> and its own text is history; the correction belongs here, where someone reading
> about plateau widths will meet it. The reference frame and its before/after are
> in [`core/tests/fixtures/scratch-0082/`](../core/tests/fixtures/scratch-0082/README.md).

### A lit backdrop is a distinct test configuration, not a variant (Plan 0051)

**`bg_bright > 0` is its own coverage axis, and until Plan 0051 nothing in the
suite tested it deliberately.** Nearly every golden baseline and scene fixture
runs `bg_bright = 0` — the right call *for a baseline*, since on black every
lit pixel provably came from the scene rather than from the backdrop. It is also
a structural blind spot: **on a black backdrop, correctly compositing over the
backdrop and wrongly covering it are the same picture.** A stage or a scene that
mishandles alpha costs nothing there and punches a hole in the frame the moment a
preset turns `bg_bright` up — which is exactly what the shipped library does.

That blind spot has now produced four defects, all after
[ADR-0055](adrs/0055-backdrop-leaves-the-post-chain.md) moved the backdrop out of
the post chain and made every stage's alpha load-bearing: the fold fading to
black (Plan 0045 Phase 2b), the bloom recombine driving alpha past 1 and
*subtracting* the backdrop (Phase 4b), and both **draw seams** emitting a
constant alpha 1 over their whole quad (Plan 0051 / ADR-0056). Each was fixed
with a guard of the same shape, and the guards are per-seam rather than global
because nothing structurally forces a shader's colour and alpha to stay in step.

**Two golden baselines are lit, and they are the exception that proves the
rule** — both `EXTRA_FIXTURES` entries rather than rostered ones, so the
per-system roster is still uniformly dark and the sentence above still describes
what a *drift* baseline is for. Each exists because the backdrop pass is
**lazy**: it does not build its gradient pipeline at all below a visible
backdrop, so no dark baseline anywhere in the suite executes a line of it.

- `core/tests/fixtures/backdrop_ramp.toml` (Plan 0080, ADR-0094) runs
  `bg_bright = 0.6` — the directional ramp.
- `core/tests/fixtures/backdrop_band.toml` (Plan 0081, ADR-0095) runs
  `bg_bright = 0.5` **and all seven band params off their defaults** — the
  curved band, over a lit ramp so the baseline pins the two *added* rather than
  the band alone. It is not redundant with the one above: the band is an untaken
  `select` branch at `bg_band_amount = 0`, so `backdrop_ramp` — the suite's only
  other lit baseline — executes none of it, and `bg_band_curve` off `0` is the
  only thing in the crate's baselines that reads the along-band axis at all.

**Both were adapter-compared before blessing** (WARP against hardware, means
recorded in each fixture's header), because each plan grew this pass's uniform
and therefore moved its `min_binding_size` — a Plan 0053 fix against a *measured*
WARP mis-render, so a divergence there is a finding rather than something to
bless.

The fixtures below exist purely for this axis, and they are **additive test
surface** rather than re-parameterized existing files, for the reason above:

- **`core/tests/fixtures/swarm_lit_backdrop.toml`** — a sparse frozen swarm over
  a lit, un-vignetted backdrop. It guards the sprite pipeline, whose radial
  falloff over a *square* quad left four hard-edged black corners (~21 % of every
  sprite).
- **`core/tests/fixtures/emitter_lit_backdrop.toml`** — a sparse emitter shower
  over the same lit backdrop, guarding the **third** draw seam (Plan 0052). It is
  the one fixture of the three that **cannot be frozen**: an emitter whose objects
  do not move has no picture at all, because its source line sits below the frame.
  That costs nothing — the three captures vary only `bg_bright` and `size`, and
  neither touches spawning or the path, so the object positions are identical
  across all three. Demonstrated in both directions: reverting its fragment shader
  to a constant alpha gives `worst |L - B|` **0.3345** with 13 330 of 136 617
  compared channels violating, against **0.0002** and zero violations as shipped.
- **`core/tests/fixtures/lines_lit_backdrop.toml`** — a sparse frozen rose at
  **`thickness = 9`**, guarding the shared line renderer and therefore all four
  line scenes at once. The fat stroke is load-bearing: the line falloff is
  one-dimensional, so its dark region is a rim whose width scales with
  `thickness`, and at shipped widths (2–3) that rim is close to a hairline a
  capture cannot discriminate. Narrowing it leaves the test green and blind.
  **Its `softness = "1.0"` is pinned for the same reason and is deliberately
  *not* the shipped default** (Plan 0114): since
  [ADR-0124](adrs/0124-the-line-stroke-carries-a-solid-core-and-a-pixel-wide-edge.md)
  the profile is authorable and the library default is `0.25`, where a plateau
  reaches coverage `1` over a *region* — which is exactly what the fourth
  capture below reads as the defect. Normalising that line to the default
  retires the wide arm while leaving it green. The test asserts the pin.

All three bind a post stage (`trails`), and that is not decoration either: with an
empty chain the scene draws straight onto the backdrop and its additive colour
cannot remove light, so the defect is unrepresentable and the guard would prove
nothing. All three tests read those preconditions back out of the fixture before
they touch the GPU, and report the pixel counts either side of the property.

They live **in `core/src/render/`, not in `core/tests/`**, for the same reason
the bloom guard does: `capture::read_back_linear` is `pub(crate)`, and the
assertion has to be made upstream of the tonemap where it is exact (see the
previous section's second bullet for why a display-byte version cannot be
written). Follow that precedent for a **fourth** draw seam, if one is ever added.

#### The fourth capture, at zero emitted light — do not "simplify" it away

The swarm and line guards take a **fourth** capture (Plan 0053 Phase 4), and it
is not a variant of the other three. It renders the same scene over the same
backdrop with the stroke or sprite emitting **no light** — `glow = 0` for the
lines, `brightness = 0` for the swarm — so `src.rgb` is zero everywhere and the
composite reduces to exactly `bg * (1 - a)`. **The frame becomes a direct readout
of alpha**, which is the quantity these guards are actually about and the one the
lit capture can only reach indirectly.

It exists because the line guard's exact arm was nearly vacuous
([design-backlog 0041](design-backlog.md)). The line falloff is one-dimensional
and — **at the `softness = 1.0` this fixture pins** — quadratic. Since Plan 0114
that is a property of the fixture rather than of the renderer: the shipped
default is `0.25`, a solid core with a one-pixel edge, and every number in this
section was measured at `1.0` and stays valid because the fixture holds it
there. So the region where the falloff is *identically* zero is the outermost
sub-pixel sliver of the quad: reverting the shader moved that arm on **15
channels**, about five pixels, and no choice of `samples` / `scale` / `thickness`
widens it. The fourth capture changes the property instead of the fixture. A
pixel is fully extinguished exactly where `a = 1`, which pre-fix is the whole
quad footprint and post-fix is the centreline:

| guard | fixed shader | pre-fix shader | the exact arm, pre-fix |
|---|---|---|---|
| lines, `glow = 0` | 779 of 28 173 (2.77 %) | 28 178 (100.02 %) | 15 channels |
| swarm, `brightness = 0` | 1 of 12 880 (0.01 %) | 16 052 (124.63 %) | 9 594 channels |

Both arms stay, and neither replaces the other: the exact one says the backdrop
arrives **intact** where nothing was drawn, the wide one says alpha **is
coverage**. Both were confirmed in the reverted direction, and the wide arm's
count is measured *before* either assertion so a failing run prints both rather
than short-circuiting on the first — the comparison between the two regions is
the evidence, and hiding it would retire the improvement while keeping the code.

A ratio above 100 % is expected rather than a bug: the footprint is counted from
where the scene put *colour*, and the regions that draw no colour but still cover
a pixel — a sprite quad's corners, an anti-aliased stroke edge — belong to the
quad without registering as drawn. On the swarm that over-count *is* the defect's
signature.

**One existing baseline was positioned to see this and did not.**
`composite_kaleido.toml` is a line scene over a lit vignette — the exception to
the `bg_bright = 0` rule above — and it moved when Plan 0051 landed, at mean
0.0009 against a 0.02 tolerance, with only the outlier gate firing. A mean-drift
gate cannot see a hairline. That is the argument for asserting the property
directly rather than trusting a baseline to notice.

> **`RLX_BLESS=1` is not scoped to the scene you changed** — it rewrites **every**
> baseline the run touches. `git status` after blessing and `git checkout` the
> baselines your change had no business moving; committing an incidental re-bless
> silently retires the drift guard for that scene. (Learned the hard way in Plan
> 0027, where an over-broad bless moved `fragment_field` and `swarm`.)

### Asserting that something *moved* — the feedback fixtures (Plan 0046)

**A baseline cannot say a picture moved.** It says a picture is the picture it was
last time, which is the opposite question. When the thing under test is a motion —
ADR-0048's transformed feedback, where an accumulation is resampled through an
affine every frame — the guard has to compare **frames of one run against each
other**, and `core/tests/feedback.rs` is where that shape lives. The next author of
a motion test should copy its habits rather than reinvent them.

**Make the figure static, so the only thing that can move is the thing under
test.** Its motion fixtures are all one Maurer rose at `spin = 0`, parked
off-centre with `pan_x`. If the scene animated, a displacement measurement could
not attribute what it found.

**Measure in the coordinate the claim is about.** These transforms are radial and
tangential, so the guards convert every lit pixel to `(radius, angle)` about the
frame centre **in pixels** and compare *extents*: `fb_zoom` must grow the radial
span faster than the angular one, `fb_rotate` the reverse. Two details that cost
real time to rediscover — an angular span has to be computed as `2π` minus the
widest empty gap, or a set straddling the `atan2` branch cut reads as the whole
circle; and "lit" is a fraction of *that frame's own peak*, never an absolute
byte, because the tail of a decaying trail is dim by construction and how dim
depends on the tonemap, the palette and the adapter.

**Assert a ratio, not a number.** Every claim there is one run against itself —
late frame over early frame, or one axis' growth against the other's. There is no
pixel count to re-tune when a shader changes by a byte, and no threshold that
encodes the capture size.

**Say what a broken version would look like, then check the guard sees it.** The
shear guard rotates the figure into a closed ring and asserts its pixel bounding
box is square, because a ring about the frame centre is round *on screen* whatever
the target's shape. It was verified by breaking it: with the transform's aspect
forced to `1.0` the box goes from **45x46 to 44x71** at the file's 100x160 portrait
target. Which is also why that target is portrait — see the file's own header, and
ADR-0047.

Two more habits from the same file, for the deposit rather than the motion:

- **A convergence claim needs both ends.** "The frame stopped changing" is also
  what a stage that never accumulated would say, so the additive guard asserts
  that the *first* window moved a lot (97 bytes) and the last did not (0) — and
  compares them as a ratio, since an unbounded accumulation moves the late window
  by about what it moved the early one.
- **A one-key comparison is only controlled if the key does something.** Two of
  these fixtures measured `0.000000` apart before they were tuned, for two
  different reasons that both look like a passing test: `max(cur, prev * fade)`
  over a *stationary* figure is exactly `cur`, and so is a `trails` stage whose
  tail is shorter than the scene's own `fade`. Where a guard's premise is "this key
  changes the picture", assert that premise before asserting anything on top of it.

### The in-frame geometry fraction, and the four things it cannot see (Plan 0069)

**It covers four scene families and not the other five.** `parametric_curve`,
`lsystem`, `star_pattern` and `spectrum` build a CPU-side segment list and stroke
it through one shared `LineRenderer`, and the measurement is taken there — the
share of total drawn **line length, segments and arcs alike**, that lands inside
the render target's world rectangle `[-aspect, aspect] x [-1, 1]`, computed
inside `LineRenderer::draw` from the geometry and an aspect it already holds. An
arc contributes `|sweep| * radius`, clipped as 64 sub-arcs judged by their own
chords (Plan 0087 Phase 2) — which matters because since that plan a mandala's
whole figure is arcs, and a measure blind to them would report every arc-drawing
preset as better-framed than it is
([ADR-0083](adrs/0083-in-frame-geometry-is-measured-at-the-line-renderers-draw-seam.md)).
`fragment_field`, `reaction_diffusion`, `attractor`, `swarm` and `emitter` build
no segment list, are **not covered at all**, and keep pixel coverage. The split
follows whether a scene rasterizes a segment list — not a line an author would
guess — so **this is not an engine-wide gate and no number it prints says
anything about half the library.**

It exists because pixel coverage cannot see a figure whose tips leave the frame.
A comb roots every bar on a shared baseline and a corona roots every spoke at a
centre, so clipping the tips costs a rounding error of lit pixels: Plan 0058
measured two over-scaled presets scoring *above* the lowest legitimate content,
where no threshold ordering separates them. Repairing the same two moves this
measure by `0.4975` and `0.7788` — 9x and 14x the `0.055` that pixel coverage had
between its lowest legitimate preset and a plausible threshold.

Four things it does not see, each of which has its own answer:

- **It measures length, not area.** Stroke width and the joint extensions are not
  counted, so a **thick** stroke leaving the frame is under-counted relative to
  the picture it actually costs, and a hairline and a 24-px bar of the same
  length are weighted identically. It is the right measure for *overshoot* and a
  poor one for anything else; a stroke-width-weighted version is a different
  measure with a different failure mode, and it is deliberately not built.
- **A figure collapsed to a point scores a perfect `1.0`.** Zero-length segments
  contribute to neither sum, and a curve that has degenerated to a dot is
  entirely in frame, which is all this instrument is asked. *Is anything actually
  drawn* is `sanity.rs`'s question — coverage against the frame's own ground
  (ADR-0126) — and the two are complements rather than a progression. A figure drawing **nothing** reports no
  fraction at all (`None`) rather than a zero, for the same reason.
- **It cannot tell a deliberately zoomed-in figure from an over-scaled one**,
  because they are the same picture. `Rose Zoom` (`zoom` bound to `2.15..3.09`)
  measures `0.3492` and `Rose Overflow` (`scale` to `2.84`) `0.3659` — they
  **bracket** the frozen over-scaled comb's `0.3563`, one just below it and one
  just above, and both are working exactly as authored. No absolute threshold
  passes those two and fails the comb; that is why the gate is **paired**: it
  compares a
  configuration against *its own repair*, never against an absolute floor.
  Anyone adding `assert!(fraction > 0.5)` over the library would fail two shipped
  presets, which is precisely the mistake ADR-0083 catalogues pixel coverage
  making one axis over.
- **It is off in the shipped render path**, so it is not a runtime signal. The
  diagnostic is a thread-local switch (`set_extent_diagnostic` /
  `take_draw_extent`) and the first test in `geometry_extent.rs` asserts a capture
  with it on is **byte-identical** to one with it off. When on, it costs a CPU
  loop over the segment list per draw — bounded by the renderer's capacity, but
  real.

The particle families' equivalent, if it is ever wanted, is a genuinely different
design: they have no segment list and their "figure" is a statistical cloud. It
is not this measure with a different input.

## The habit for a new scene

When you add a new scene or preset:

1. **Eyeball it first** — `--preset <name> --out /tmp/new.png` and Read the PNG.
2. **Add the differential cases** — the `reactivity`, `animation`, and `sanity`
   tests iterate the embedded presets automatically, so a new *preset* is covered
   once it's in the default set; a new *system* may need its per-system floor in
   `sanity`. If it's beat-driven, extend `beat`.
3. **Check distinctness** — run `--report` (or the `distinctness` test) to see if
   the new preset is a near-duplicate of an existing one (advisory).
4. **A new *system* needs a golden fixture; a new *preset* does not.** Adding a
   `SystemKind` variant fails `golden.rs` to compile until you author
   `core/tests/fixtures/<system_name>.toml` and add its arm — then bless that one
   baseline after eyeballing it. Shipped presets are never pixel-pinned
   ([ADR-0023](adrs/0023-golden-drift-guard-uses-frozen-fixtures.md)).

## `--downbeat-log`: the estimator's terms, one row per beat

The one instrument on this page that runs the **live app** rather than a headless
capture, and that is the point of it (Plan 0086 Phase 1). The question it exists
for cannot be asked of synthesized audio: the downbeat estimator publishes on
**6.0 %** of audible time and on backbeat rock/pop **0.14 %**, and the cause is
still *inferred* — `diagnostics.log` samples at 1 Hz and records the estimator's
outcome, not its per-beat terms, so three different failures fit the same reading.

```bash
# Windows: a row per detected beat, alongside the 1 Hz diagnostics.log
ritmolux.exe --downbeat-log
ritmolux.exe --downbeat-log C:\path\to\downbeat.log     # or --downbeat-log=<path>
```

A bare flag writes `downbeat.log` beside `diagnostics.log` under the per-user app
dir — the same three argument shapes [`--soak`](nfr.md) takes. Off by default:
without the flag no logger exists and the frame loop is unchanged.

One tab-separated row per **beat**, header on a fresh file:

| column | what it is |
|---|---|
| `beat` | the beat clock's `beat_index` — gaps mean beats the render loop coalesced. **Not the counter the fold buckets by**; that is `fold_beat`, below |
| `s0`..`s3` | mean accent per candidate beat-1 alignment: the 4/4 fold's own output |
| `best` | the alignment the fold favours right now |
| `held` | the alignment actually held, which lags `best` by the hysteresis |
| `effect_raw` | between-alignment share of accent variance, **before** correction |
| `null_share` | what four groups would explain by chance at this history length |
| `effect_corrected` | what the gate compares — and the published `downbeat_confidence`, bit for bit |
| `beats_seen` | accents recorded, against the 8-beat floor, saturating at 32 |
| `locked` | `0`/`1`, so the publish **rate** over a run is the mean of the column |
| `bass` `mid` `treb` `onset` | the normalized levels for context |
| `bpm` | the tempo tracker's estimate — read the **row rate** against it (see below) |
| `time_since_beat` | how stale this row's band levels are: they come from the latest analysis hop, not necessarily the hop the beat fired on |
| `unix_ms` | the time axis. Deltas between rows are the inter-detection interval, and it lines a capture up against a `diagnostics.log` from the same session |
| `fold_beat` | **the counter `s0..s3`, `best` and `held` are indexed in** — the grid's tempo-driven beat count once the grid runs, `beat_index` before that. Bucket the accent by `fold_beat % 4`, never by `beat % 4` |
| `grid_bar_phase` | where `fold_beat` sits across the bar, `[0, 1)`, **ungated** — no alignment subtracted, so it says where the *grid* is rather than where the estimator thinks beat 1 is. **Only a grid reading where `bpm > 0`**; on a warmup row it is the tempo tracker's onset-reset phase, so do not average the column down the whole file |

> The last five are **appended, never interleaved** — the frozen-prefix rule
> `diagnostics.log` follows — so a capture taken before they existed stays
> parseable by column name.

**`beat` and `fold_beat` are two different counters, and only the second one
indexes the alignment block.** They were one number until Plan 0095 moved the fold
onto the bar grid ([ADR-0109](adrs/0109-the-beat-clock-counts-onsets-not-beats.md));
`beat` still means `beat_index`, unchanged, so that every capture taken before
that keeps parsing and stays comparable. Reading `s0..s3` against `beat % 4`
produces a plausible, wrong answer — on the synthesized 4/4 in
`standalone/src/downbeatlog.rs` it puts the accent on phase 0 while the fold
reports 3 — which is what these two columns exist to stop.

**Read the row rate against `bpm` before reading anything else.** The beat flag
that paces these rows comes from the onset detector — an adaptive threshold on
spectral flux with a 96 ms refractory — and it is **not tempo-gated**
(`core/src/dsp/onset.rs`); `beat_index` is a straight count of those events
(`core/src/dsp/tempo.rs`). So `rows / seconds` against `bpm / 60` is the number of
detections per musical beat, and it is **not** guaranteed to be 1. On a synthesized
clip with one transient per beat it measures exactly 1.00; on real material with
hats it does not — which is why the fold stopped bucketing by `beat_index` in
Plan 0095, and why the ratio is still worth reading: it is the size of the gap
between the `beat` column and `fold_beat`.

**Reading it is what tells the three stories apart** — the reason the plan spends
a phase capturing before choosing a repair:

- `s0..s3` flat and `effect_raw` low → the accent carries no bar-scale structure
  → the accent feature is the defect.
- two scores tied and high, the other two low → a kick on 1 and 3 is 2-periodic,
  so the fold is choosing between two equally good answers → the repair is a cue
  independent of the drum pattern, and a second *percussive* band is the same
  ambiguity phase-shifted.
- `effect_raw` healthy but `effect_corrected` near zero → the noise correction is
  eating a real effect at this history length → the window or the measure.

Three things to know before running one:

1. **It costs the estimator nothing.** `Analyzer::downbeat_terms` is `&self`,
   allocation-free and clock-free, so being observed cannot change what is
   observed. The write is on the render thread — never the audio callback — and
   event-paced: ~2 rows/s at 120 BPM, and a frame with no beat costs a bool test.
2. **A hidden window logs nothing.** Rows come off the frame path, which returns
   early while occluded or zero-sized. Keep the window up for a capture.
3. **Run matched material.** The measurement this file exists for is a *genre
   split* — unambiguous 4/4 backbeat rock/pop against a four-on-the-floor control,
   matched in duration — so the result is comparable with Plan 0068's 6.79 % /
   0.14 % baseline. And do not re-measure by ear: `locked` is the outcome
   instrument and the score columns are the decomposition.
