# Design backlog — archive of closed entries

Every entry from [`design-backlog.md`](design-backlog.md) whose question has been answered:
promoted and landed, answered by measurement, retired unfired, or retracted because its premise
turned out to be false. Split out on **2026-08-04**, when the live file had reached 3265 lines and
the open entries were under a fifth of it.

**Bodies are verbatim.** Nothing was summarized on the way in, because the value of a closed entry
is rarely its outcome — the outcome is in the ADR and the plan. The value is the *record of how the
diagnosis moved*, and this file is the only place several of those survive:

- **0010** — the fold's edge debris was filed as a Plan 0033 regression. It was not; a worktree at
  the prior commit reproduced it identically. The bug dated from Plan 0018 and Plan 0033 only made
  it legible.
- **0012** — filed as a metric bug (`cover` "penalises inverted-polarity presets"). The metric is
  symmetric; the reading was truthful and the gap was interpretation.
- **0014** — the conclusion held (the `hue` ramp is not a hue wheel) and every colour name in the
  entry was wrong by about 0.16 of the ramp.
- **0046** — filed by `architect`, retracted the same day by the lane it was handed to, before any
  preset was edited. The claim was false and the fix it specified would have made the family worse.
- **0052** — retired: the preset was never flat, and the statistic convicted the right preset for
  the wrong reason.

That is five out of forty-six entries whose causal claim inverted under verification, all filed in
good faith from real symptoms. The standing lesson, which is why the bodies are kept: **symptom
reports from any lane are reliable; causal claims want checking against code before they become
work.**

**This file is append-only and closed.** Entries arrive here when a live entry closes; nothing here
reopens. A question that comes back is a *new* entry in the live file, citing this one — the
distinction matters, because "0037 reopening" and "a new question ADR-0047 already priced as its
accepted cost" are different documents and only one of them is honest.

---

## 0001 — reaction_diffusion reaches only 2 of the 5 Plan-0018 composite levers

- **Raised:** 2026-07-24, from `preset-author` (authoring the "Chthonic Coral Oracle" coral preset).
- **Verified against code:** yes — see the per-lever notes below.
- **PROMOTED 2026-07-24 → [ADR-0026](adrs/0026-full-composite-coverage-fullscreen-scenes.md) +
  [Plan 0025](plans/done/0025-full-composite-coverage.md)** (full-audit scope: background + view transform
  for reaction-diffusion *and* attractor, via alpha-present-over-backdrop). Notes retained below as the
  origin record.

Plan 0018 shipped five engine-wide, audio-bindable composite controls (view zoom/pan, background
atmosphere, geometry mirror, feedback trails, screen-space kaleidoscope). The **reaction_diffusion**
(Gray-Scott coral) scene participates in only two of them. A preset author composing the coral
scene silently loses three families of named params — they parse fine (no `deny_unknown_fields`)
but do nothing.

| Lever | Params | Reaches RD? | Why |
|-------|--------|-------------|-----|
| Feedback trails | `trails` | **yes** | Post-pass over the composited frame (`render/trails.rs`) — scene-agnostic. |
| Screen-space kaleidoscope | `kaleido_order`, `kaleido_angle` | **yes** | Post-pass over the offscreen frame (`render/kaleidoscope.rs`) — scene-agnostic. |
| Background / atmosphere | `bg_hue`, `bg_bright`, `bg_vignette` | **no** | The `bg_*` pre-pass draws first, but RD's present is a **fullscreen opaque** pass (`reaction_diffusion.rs::render`, `LoadOp::Load` + `BlendState::REPLACE`, alpha 1 everywhere) — it overwrites the backdrop. |
| View transform | `zoom`, `pan_x`, `pan_y` | **no** | `ViewTransform` is consumed only by `fragment_field`, `swarm`, and the line scenes. RD's `render` takes no transform; its field samples 1:1 to screen. |
| Geometry mirror | `mirror_order`, `mirror_reflect` | **N/A by nature** | Line-segment replication before the segment cap — a fullscreen field has no segments. The screen-space kaleidoscope is the right tool for RD's symmetry and already works; **not a gap, just a clarification.** |

**So there are two genuine gaps (background, view transform) and one non-gap (geometry mirror).**

- **Highest value: background compositing.** The coral look is mostly dark — black voids between the
  contours. If RD composited over the `bg_*` atmosphere instead of overwriting it (present with
  alpha/blend so `V≈0` reads as transparent, or an explicit backdrop-aware present), those voids
  would fill with the tintable gradient. Big aesthetic upside for a small preset author, and the
  thing most likely to make the coral scene feel "finished."
- **Lower value: view zoom/pan on RD.** Wiring the `ViewTransform` into the present pass's sample UVs
  would let a preset zoom into the reef. Straightforward but less impactful than the backdrop, since
  the kaleidoscope already supplies large-scale motion.

**Likely also affected: the `attractor` scene** (also absent from the `ViewTransform` consumer list).
Confirm its lever coverage if this is promoted — the fix may want to be "audit composite coverage
across *all* scenes," not RD alone.

**ADR-worthy if pursued.** Touches ADR-0018's fixed-order composite (where/how a fullscreen scene
hands off to the background) and the `Scene` render seam (does every scene take a `ViewTransform`, or
does the composite own it?). The rejected alternative — "leave RD opaque; backgrounds are for the
line/particle scenes only" — is nameable, so a decision here earns an ADR, then a small plan.

---

## Entries 0002-0009 — the 2026-07-26 `preset-author` API-feedback batch

All eight below were raised together on **2026-07-26**, from the `preset-author` lane's rewrite of
all 35 shipped presets while iterating live with the user on a **2048x1152** fullscreen display.
Every one was **re-verified against the code by architect at intake** — findings and corrections are
recorded per entry. Ordered by how hard each blocked real authoring work, not by cost to fix.

---

## 0002 — No per-bin spectrum: the grammar sees three bands, and no scene draws N elements

- **Raised:** 2026-07-26, from `preset-author`. **Requested twice, unprompted, by the user**
  ("a full spectrogram in several lines... 20-30 points"; then "morph the attractor shape from a full
  spectrogram with a lot of bars").
- **Verified against code:** yes. `VAR_NAMES` (`core/src/preset/expr.rs:41`) is exactly nine scalars —
  `bass mid treb onset beat bar time tempo novelty`. The FFT exists in the analyzer; **none of it is
  reachable from a preset.**
- **PROMOTED 2026-07-26 → [ADR-0036](adrs/0036-preset-reachable-spectrum.md) +
  [Plan 0034](plans/done/0034-preset-reachable-spectrum.md).** **Three verifications shrank this well below
  the estimate below**, and they are why the plan is three separable steps rather than one big one:
  (1) the spectrum already exists as a **normalized, log-spaced 64-band array** on `AnalysisFrame`
  (`dsp/mod.rs:32`, commented "Log-frequency bands exposed to scenes"), already consumed by
  `novelty.rs` — **no new DSP**; (2) `Scene::update(&mut self, frame: &AnalysisFrame)` **already hands
  every scene all 64 bands every frame**, so a scene drawing the spectrum needs no new channel; (3)
  `LineRenderer::draw(&[SegmentInstance])` already draws arbitrary segment lists, so an N-element
  scene is a **fourth consumer of an existing idiom**, not a new render idiom. The attractor-morphing
  half is met by `bin(x)` driving its four shape scalars — no per-particle mechanism needed. Notes
  retained below as the origin record.

Three bands is not a spectrum. The lane's workaround was to map the three bands onto three
*separable structural* levers (Arrowhead: treble to subdivision depth, mid to mirror fold count,
bass to scale) so the figure at least represents something; the user's verdict on the single-band
version was "represents not sure what... feels very poor".

**This is two decisions, not one, and they are separable:**

1. **Grammar/analysis surface.** How does a preset name bin `i`? Nameable alternatives, all real:
   an indexing form (`spectrum[i]`) — which introduces the first non-scalar type into an expression
   language that is deliberately scalar-only; N flat variables (`band0`..`band31`) — no type change,
   but `VAR_COUNT` explodes and `Variables` grows a 32-float payload on the per-frame path; or a
   `bin(i)` call, which fits the existing `Call` node and keeps everything scalar.
2. **A scene that draws N elements.** This is the harder half and the one with no precedent. The
   binding model today is **one expression to one scalar to one `set_param`** — there is no channel
   on which a per-element value travels. Either the scene reads the spectrum directly from the
   analysis frame (engine-side; the preset selects the scene and styles it, but does not author the
   mapping), or the grammar gains an implicit per-element index that the scene evaluates the
   expression once per element against — which makes evaluation N-times-per-frame and is the first
   time an expression is not evaluated exactly once per frame.

**Largest item in the batch and the one needing the most interview.** ADR-worthy on both halves.
Note the determinism rule is not at risk either way: the FFT is already a pure function of the input
window.

---

## 0003 — Fixed internal resolutions: RD at 256x256, trails and kaleidoscope at 1280x720

- **Raised:** 2026-07-26, from `preset-author`. The user's own words: **"coral is broken"**,
  "fern grow... feels like it is upsized from something much smaller", "roses overall feels upscaled
  as well - quality is poor."
- **Verified against code:** yes — `const GRID: u32 = 256` (`render/scenes/reaction_diffusion.rs:48`),
  `TRAILS_W/H = 1280/720` (`render/trails.rs:45`), `KALEIDO_W/H = 1280/720`
  (`render/kaleidoscope.rs:40`). All three report through `PostStage::internal_size`.
- **PROMOTED 2026-07-26 → [ADR-0034](adrs/0034-internal-resolution-follows-the-target.md) +
  [Plan 0033](plans/done/0033-internal-resolution-and-preset-surface.md)** (Phases 3-4 the RD side,
  Phase 6 the post stages, Phase 7 the mirror-vs-kaleidoscope docs action). Notes retained below as
  the origin record.
- **Not chemistry:** the lane swept `flow` across 0.45 / 0.70 / 1.00 and the blockiness is identical
  in all three. No preset value removes it.

At 2048 wide, RD is an 8x upscale and the two post stages are a 1.6x upscale **of the whole frame**,
including crisp line geometry. The cost is not hypothetical: the lane **removed `trails` from all 13
line presets** to recover sharpness, which cost those presets their feedback/afterglow entirely — a
direct trade between "the look I want" and "acceptable sharpness".

**This is the already-deferred "target-sized internal grid for RD/trails/kaleidoscope" work**, and
[Plan 0029](plans/done/0029-attractor-resize-cost-and-ink-followups.md) already built the pattern it
should follow: the `PipelineResources`/`FieldResources` split (rebuild only what depends on size),
plus `trail_grid_size`'s pure policy function — round each axis up to a 256 px step, cap by a
**single** scale factor applied to both axes so aspect is preserved.

**Two things a plan here must not paper over:**

- **RD is different from the other two.** Its grid is a *simulation* domain, not a raster. Raising
  `GRID` changes the pattern's spatial scale relative to the frame at the same `feed`/`kill`/`flow`,
  so **every shipped RD preset's look shifts** and the RD goldens change. Trails and kaleidoscope are
  pure rasters — sizing them up changes sharpness, not content. Decide deliberately whether RD
  re-tunes its presets, scales its diffusion rates with the grid, or gets a smaller bump than the
  display would justify.
- **The 256 was chosen for test cost, and that reason is still live.** The doc comment at
  `reaction_diffusion.rs:44` says 512² quadruples the per-step fragment work the differential tests
  pay each warm-up frame **on the WARP software adapter**. A target-sized policy must keep the
  headless suite brisk — the existing quantize-and-cap shape does this naturally, since captures
  render small.

**ADR-worthy.** The nameable rejected alternative is real: keep the fixed grids (predictable cost,
a guaranteed iGPU floor, byte-reproducible captures independent of window size) and instead expose a
resolution *scale* param so the author trades sharpness for cost per preset.

**Docs action this carries (worth stating regardless of what gets built).** `mirror_*` replicates
real geometry **before** rasterisation and is therefore free of resolution cost, while `kaleido_*`
folds **finished pixels** at the stage's internal size. On line scenes, prefer the mirror. That
asymmetry is not stated anywhere in `presets/README.md` and it is exactly the guidance that would
have saved the lane the trades above.

---

## 0004 — `zoom`/`pan_*` on reaction-diffusion smear the edge: a toroidal sim behind a clamped sampler

- **Raised:** 2026-07-26, from `preset-author`. `zoom > 1` renders vertical bars and rectangular
  blocks (reproduced cleanly: 0.70 and 1.00 fine, 1.30 corrupt); `zoom < 1` magnifies the 256 px
  grid; any real `pan_*` at `zoom = 1.0` walks off the field the same way. All four RD presets are
  now pinned at `zoom = 0.99` with a whisper of pan, which costs the family its whole view-transform
  lever.
- **PROMOTED 2026-07-26 → [ADR-0034](adrs/0034-internal-resolution-follows-the-target.md) +
  [Plan 0033](plans/done/0033-internal-resolution-and-preset-surface.md) Phase 5.** Notes retained below
  as the origin record.
- **Verified against code — and the diagnosis is more specific than the report.** The present pass
  computes `uv = (in.uv - 0.5) * zoom + 0.5 + pan` (`reaction_diffusion.rs:226`), so `zoom > 1`
  samples outside `[0,1]`. The present sampler is **`AddressMode::ClampToEdge`** (`:418-420`) — so
  off-field reads repeat the edge row/column outward. That is precisely "vertical bars and
  rectangular blocks".
- **The sim it samples is already toroidal.** `ld()` in the sim shader (`:140-144`) wraps with
  `((c % size) + size) % size`. **The field is seamless; only the present sampler refuses to wrap.**
  So the likely fix is `AddressMode::Repeat` on the present sampler, not a clamp on `zoom` — and
  `pan_*` then becomes an infinite seamless scroll over the torus, which is a *better* lever than the
  one the docs promise. Unverified whether the gradient central-difference at `:231-232` wants the
  same treatment; it reads through the same sampler, so it should follow for free.
- **Correction to the report:** it states `presets/README.md` "documents the opposite". It does not.
  The README (`:124-127`) says a higher `zoom` shows *more* of the field, which is exactly what the
  shader computes. The gap is an **omission**, not an inversion: the README never says RD's field is
  finite, so "more of the field" past the edge is the edge smeared. `fragment_field` shares the
  formula and has no such problem because its domain is procedural and infinite.

**Small and high-value; not ADR-worthy on its own.** Sequence it with 0003 — the `zoom < 1` half of
the complaint is the fixed-grid problem, and only the `zoom > 1` half is this sampler.

---

## 0005 — No bloom / glow / halo stage

- **Raised:** 2026-07-26, from `preset-author`. The user, on Arrowhead and again on Fern: "can we add
  some kind of shadow or glow for arrowhead? or halo, still feels too 'naked'".
- **Verified against code:** yes. The composite is `background -> scene -> PostChain (trails ->
  kaleidoscope) -> [blend] -> ink -> present`; `core/src/render/` has no glow module. The line scenes
  offer `thickness` and `brightness` only.
- **PROMOTED 2026-07-30 → [ADR-0046](adrs/0046-linear-light-hdr-composite-bloom-tonemap.md) +
  [Plan 0045](plans/done/0045-linear-light-and-bloom.md). ~~CLOSED 2026-07-31~~** — shipped as a
  **screen-space `PostStage`** (the entry's first alternative, the universal one), third in the
  chain after the fold, with bindable `bloom_amount` / `bloom_threshold` / `bloom_radius` and
  `bloom_levels` per tier. Both of the entry's "two things to decide" were decided the way it
  hoped: the stage is universal rather than per-scene, and it is sized to the render target, so it
  inherits [0003](#0003--fixed-internal-resolutions-rd-at-256x256-trails-and-kaleidoscope-at-1280x720)'s
  fix rather than the 720p problem. Its sequencing note also held —
  [0010](#0010--the-kaleidoscope-fold-samples-outside-its-source-rectangle-and-clamps-leaving-edge-debris)
  was decided first, in the same plan.
  **What the entry did not anticipate, and it is the finding worth carrying:** a bloom stage is
  only half the answer. The **additive ceiling this entry's second half measured is what would have
  starved it** — a bright-pass over an already-clipped frame reads as haze, so the plan had to
  convert the whole composite to linear light *first* and only then add the stage. The
  consequence for authoring is that `bloom_threshold = 1.0` selects light that is genuinely over
  range, so a preset written to the old keep-it-under-1.0 habit gets **nothing** from bloom. That
  is now documented in `presets/README.md`'s bloom section, and `presets/star_lantern.toml` is the
  worked example. The entry's `thickness`-vs-`brightness` finding survives intact and is the reason
  `glow` is named as the cheapest fuel: it drives the core, not the width.

What both presets do now is a lit, loosely-vignetted **backdrop** reading as an aura. It is a
backdrop, not a halo: it does not follow the strokes. `trails` gives a real halo but costs the
downsample in 0003.

"Naked" thin-line art is a **recurring** aesthetic complaint, and a bloom stage would lift every line
preset in the set — 13 of the 35 shipped presets.

**Well-shaped, because the architecture for it now exists.** ADR-0031's `PostStage` trait and the
instantiable `PostChain` mean a bright-pass + separable blur + additive-combine stage is an added
array element and a `STAGE_COUNT` bump, not surgery. Two things to decide:

- **ADR-worthy, with a genuine rejected alternative:** a screen-space post stage (universal — every
  scene gets it, including RD and the attractor) versus per-scene glow in the line shaders (cheaper,
  follows the geometry exactly, no full-frame blur, but only the line family benefits and each scene
  reimplements it).
- **It interacts with 0003.** A bloom stage added at a fixed 720p inherits the same upscale
  complaint it is meant to answer. Either sequence it after 0003 or size it to the target from the
  start.

Cost on the iGPU floor is the live risk — a separable blur is two extra full-frame passes per blur
level. `docs/nfr.md` §7 is the budget it must answer to.

**RE-RAISED 2026-07-26 (second batch), with a new concrete finding.** The user asked for "much much
more glow" on Fern Grow, and getting it took four rendered variants to discover that glow on a line
scene is a **three-way** interaction whose decisive term is not a stroke param at all:

- `thickness` is the halo — the line primitive's quad has a *quadratic* falloff
  (`core/src/render/scenes/lines/renderer.rs:109`), so a wider stroke is a wider halo.
- `brightness` is the shader's glow multiplier onto that falloff.
- **`bg_bright` / `bg_vignette` decide whether either reads at all.** Fern's backdrop floor (0.085
  rising to 0.215, vignette 0.55) meant raising `thickness` made the fern *fatter but never
  brighter* — an additive halo falling off into a lifted floor is flat paint. Only after dropping the
  floor to 0.012 and deepening the vignette to 0.88 did the stroke params do anything.
- **And widening too far reads as OUT OF FOCUS rather than as glow** — user-reported at
  `thickness = 4.0`, because the quadratic falloff spans the whole quad, so widening spreads the core
  instead of adding halo around it. The working shape is a thin bright core (1.9) with `brightness`
  (1.55) carrying intensity.

That is four coupled hand-tuned params standing in for one missing capability, and the coupling is
undiscoverable — it cost a full sweep to find. Strengthens the case for the stage rather than
changing its shape. **Sequencing note:** 0033 is closed, so the "size it to the target" concern above
is satisfied; but a bloom stage inherits the same `PostStage` plumbing as [0010](#0010--the-kaleidoscope-fold-samples-outside-its-source-rectangle-and-clamps-leaving-edge-debris),
so decide 0010 first and let bloom be built against the settled answer.

---

## 0006 — `[smoothing]` is a one-pole low-pass: no attack/release split, no S-curve

- **Raised:** 2026-07-26, from `preset-author`. The user: "pulse field reaction are way too fast and
  jarring, we should smoothen it up a lot - use some qubic bezziere function or something."
- **PROMOTED 2026-07-26 → [ADR-0035](adrs/0035-asymmetric-attack-release-easing.md) +
  [Plan 0033](plans/done/0033-internal-resolution-and-preset-surface.md) Phase 2.** Notes retained below
  as the origin record.
- **Verified against code:** yes. `[smoothing]` is one time constant per param, folded onto
  `Binding::tau` at load (`preset/schema.rs:270`), applied by `Smoother::smooth`
  (`render/mod.rs:310-326`) as `alpha = 1 - exp(-dt/tau)`. One state slot per binding. No ease shape,
  no attack/release split.
- **Note on the ask:** `smoothstep` exists in the grammar but shapes a **value**, not a trajectory
  over time — expressions are stateless by hard invariant, so a cubic-bezier *ease* cannot be
  authored from the preset side today.

**The workaround does not work.** A longer `tau` does reduce the jarring, but it delays the attack
equally, so the preset gets mushy instead of getting the snap-then-glide a beat-driven param wants.
Symmetry is the actual defect, not the curve shape.

**The lane's suggested shape is the right one and is very cheap:** a two-constant `attack`/`release`
form (`param = { attack = 0.05, release = 0.6 }` beside today's scalar `param = 0.3`). It stays
stateless from the author's side, needs **no** new expression machinery, and lands in `Smoother` as
picking the constant by whether `raw` is above or below the held value — the state slot it needs
already exists. `Binding::tau` becomes a pair; the load-time fold is already the right seam.

**ADR-worthy as a short supplement to [ADR-0019](adrs/0019-eased-parameters.md)** — the nameable
rejected alternative is a full parametric ease curve (bezier control points per param), which is what
was literally asked for and which requires per-binding phase state, a notion of "a transition in
progress", and a rule for what happens when the target moves mid-ease. Asymmetric one-pole gets most
of the perceived benefit for a fraction of that.

**Best value-per-line-of-code item in the batch.**

---

## ~~0007 — `star_pattern` reads as a hollow ring, and discrete `variant` cannot be blended~~

- **CLOSED IN FULL 2026-08-06.** Both asks are now built, and the entry has no live half left.
- **Morph half — PROMOTED 2026-08-01 → [ADR-0060](adrs/0060-star-pattern-variants-interpolate.md) +
  [Plan 0054](plans/done/0054-the-line-scenes-catch-up.md)**: `variant` becomes a continuous contact
  angle with a hysteresis cache, so the three precomputed geometries and the `floor` between them are
  gone. ADR-0060's Notes carry the lane's `contact_angle_deg` sweep as the starting measurement.
- **Interior half — PROMOTED 2026-08-04 → [ADR-0079](adrs/0079-the-mandala-interior-is-rings-of-motifs-inside-star-pattern.md) +
  [Plan 0065](plans/done/0065-the-mandala-interior.md)**: `[generator] rings`, a closed seven-motif roster
  of concentric rings drawn through the same line renderer, plus three bindable levers
  (`ring_phase` / `ring_spread` / `ring_scale`). The "hollow ring" is a
  measurement rather than an opinion and the plan closed it as one: the bare rosette occupies
  **1 of 10** radial shells, a four-ring mandala **9 of 10**.
- **The plan also answered this entry's composition question** — "is the interlace worth keeping" —
  by shipping both: `star_mandala` was the ornament alone and `star_weave` the same roster inside
  the twelve-fold interlace.
- **All three of those presets were retired hours after the close, on 2026-08-06, and this entry
  stays closed anyway.** The live judgement Plan 0065 Phase 6 was to settle came back against the
  whole approach rather than against either composition: the motifs are parametric outlines sampled
  to straight segments, so at ornament scale the vertices show and a circle reads as a polygon
  ([0073](design-backlog.md)). **This entry asked for the interior to stop being hollow and it did**
  — the capability is shipped, tested and documented, and the 1-of-10 / 9-of-10 measurement is
  unaffected by whether a preset ships it. What the retirement establishes is narrower and belongs to
  0073: *placed outline geometry is the wrong mechanism for a mandala*. That look now ships as
  `reaction_gilt`, a folded analytic iso-contour field with no geometry in it at all.
- **Two findings the same work raised are live and are NOT this entry**: the coverage floor that
  pushes this scene toward washed-out tuning ([0072](design-backlog.md)), and the scalloped boundary
  the user chose as a real curve primitive ([0071](design-backlog.md)). Notes below retained as the
  origin record.

- **Raised:** 2026-07-26, from `preset-author`. The user's verdict: **"idea is interesting but looks
  poor"**, and separately "change between star rosette shapes should be smooth".
- **Verified by sweep, not by code read:** segments sit near the rim at every `contact_angle_deg`
  (swept 12 / 20 / 28 — no meaningful interior change). Because a Hankin rosette is rotationally
  symmetric about the frame centre, `mirror_*` rotates copies onto the original and is close to a
  no-op; only non-dividing fold orders (5, 7 against a 12-fold star) add anything.
- `variant` indexes one of three precomputed geometries, so there is nothing to interpolate.
  `[smoothing]` on it only spends time on fractional indices that `floor` collapses back to the same
  three shapes, which reads as a stutter. The only preset-side mitigation is making the cut rare
  (~50 s) and hiding it under a continuous redraw.

**Two asks, both engine work, and the first is a product call before it is a design one:** does this
scene earn its slot as-is? Options named by the lane are more tilings, an off-centre mirror, or
drawing the underlying tiling grid. The second ask — a real geometry lerp between variants — is a
generator-level change to `star_pattern`'s config surface (ADR-0007 territory).

**Lowest-confidence entry in the batch**, and the only one where "cut the scene" is a legitimate
answer. Do not promote without asking the user which way they lean.

**Decided 2026-07-26: invest, do not cut.** The user chose to make the scene earn its slot rather
than retire it. Still needs its own ADR and plan — both asks (a richer interior, and a real geometry
lerp between `variant`s) are generator-level changes to `star_pattern`'s config surface, which is
ADR-0007 territory. Not folded into [Plan 0033](plans/done/0033-internal-resolution-and-preset-surface.md),
which is a resolution/preset-surface plan and shares no files with this.

**RE-RAISED 2026-07-26 (second batch) — the user asked for the blocked capability directly.** Live
against the running app, unprompted: *"star rosette - very nice, but can we make morphing between
shapes easier, slower?"* That is exactly the second ask above, and it is unreachable from a preset —
`variant` indexes three precomputed contact-angle geometries, so smoothing it only stutters across
`floor` boundaries (`presets/star_rosette.toml:31-39` documents why). Note also the shift in the
verdict on the scene: the original entry recorded "idea is interesting but looks poor", and the same
user now opens with "very nice" — the preset-side mitigations (radial `draw_progress` motion, a rare
and hidden cut) did move it. What remains is the geometry lerp, which is engine work. This raises
confidence in the *invest* decision and narrows the ask to the second half.

---

## 0008 — `shot` harness gaps that cost the content lane real iterations

- **Raised:** 2026-07-26, from `preset-author`. All three verified.
- **PROMOTED 2026-07-26 → [Plan 0033](plans/done/0033-internal-resolution-and-preset-surface.md) Phase 1**
  (no ADR — no rejected alternative worth remembering). **One item was answered rather than built:**
  the pulsing `--set` form is *not* being added. `apply_set` is a pure per-frame function with no
  frame index, and `shot --audio <clip.wav>` plus `--signal click:120` already produce transient
  beats and realistic levels — so Phase 1 documents the trap and names those paths instead of
  duplicating them. Notes retained below as the origin record.

1. **`--set` cannot drive `tempo` or `novelty`.** `apply_set` (`standalone/src/shot/args.rs:35-43`)
   accepts exactly `bass mid treb onset bar beat`. The two variables Plan 0019 added are unreachable
   from the harness this lane self-verifies through — **which is a large part of why no shipped
   preset used them.** They can only be exercised via `--signal`, which cannot pin a BPM either side
   of a threshold. Adding two match arms has a direct effect on how testable the newest grammar
   features are.
2. **`--set beat=1` holds the gate high for all 120 captured frames.** That is unphysical — a real
   beat is transient — and it badly over-represents any `beat`-driven accent (`burst`,
   `mirror_reflect`). **It made several swarm and rose presets look broken in stills that were fine
   in motion.** Wants either a note in `docs/capturing.md` or a `--set` form that pulses.
3. **Band magnitudes in `--set` are not comparable to real loopback levels.** The lane calibrated
   against `bass=0.8`; the user's first live reaction was that presets barely reacted. Real material
   sits far lower, so gains tuned against a still are **systematically too weak**. Wants a documented
   "typical loudness" reference value, or a `--signal` preset matching real music levels.

**Cheapest item in the batch, no ADR needed, and it compounds** — every future preset the lane
authors is verified through this harness, so a mis-calibrated harness mis-tunes everything downstream
of it. Item 3 is the one with the widest blast radius and it is pure documentation plus a measurement.

**~~CLOSED 2026-07-27~~ — item 3, the last one open, is answered.**
[Plan 0037](plans/done/0037-verifying-easing-transient-probe-and-dynamic-signal.md) Phase 4 measured
real material through `--audio` and recorded the range in
[`capturing.md`](capturing.md#what-real-material-actually-produces): real bass **means** of
`0.000`–`0.007` against peaks up to `0.190`. The answer is not one number — it is that a *continuous*
binding must be gained against the **mean** and a *percussive* one against the **peak**, and that
`--set bass=0.8` is ~100x the former. Phase 3's `--signal dynamic:<bpm>` supplies the realistic-shape
stimulus the item also asked for. What that measurement then revealed about the shipped library is
**[0020](#0020--the-shipped-library-is-gained-against-stimuli-6-100x-hotter-than-real-music)**.

---

## Entries 0010-0014 — the 2026-07-26 `preset-author` API-feedback batch (second, post-Plan-0033)

Raised from the lane's Phase 8 preset pass (`a070f5a`) and the live-tuning session that followed
(`8b5b2e0`), on the user's 2048x1152 display. Two of these were filed by the lane as Plan 0033
regressions; **one of those was verified here and is not one** — see 0010.

---

## 0010 — the kaleidoscope fold samples outside its source rectangle and clamps, leaving edge debris

- **Raised:** 2026-07-26, from `preset-author`, user-reported from the running app on `swarm_dense`
  ("dense still has artifacts on corners", with a screenshot).
- **Verified against code AND against the pre-Plan-0033 engine.** This entry's diagnosis is
  **corrected** from the one the lane filed.
- **PROMOTED 2026-07-30 → [ADR-0047](adrs/0047-kaleidoscope-fold-domain-disc-with-falloff.md) +
  [Plan 0045](plans/done/0045-linear-light-and-bloom.md) Phases 1/2/2b. ~~CLOSED 2026-07-31~~** —
  the fold now covers a **disc with a radial falloff** (the entry's third alternative), chosen from
  a sixteen-image three-way sample set at 16:9 *and* portrait, per the user's
  concrete-examples workflow. The edge debris is gone at every aspect, and the direct guard this
  entry asked for exists — an assertion on the out-of-disc pixel statistic using a border-filling
  fixture, so a future fold change cannot pass by consuming the drift budget the way this entry
  warned `composite_kaleido.png` would. `swarm_dense`'s "pinned to dodge the defect" comment and
  the false "six is the highest that stays clean" claim are both gone.
  **Two things the sample set falsified, both worth reading before touching the fold again.**
  This entry's model of the clamp alternative was wrong: a plain clamp draws a *sunburst of rays*,
  not a flat ring, so the falloff's real job is fading rays a clamp still draws. And the falloff
  faded to **black** rather than to the backdrop, because the backdrop was rendered *into* the
  fold's own input — which is what [ADR-0055](adrs/0055-backdrop-leaves-the-post-chain.md) and
  Phase 2b exist to fix.
  **The disc itself then came back rejected in motion**, on grounds ADR-0047 already recorded as
  its accepted cost. That is a new question, not this one reopening — see
  [0037](#0037--the-fold-covers-a-disc-and-on-a-field-scene-that-reads-as-worse-than-the-defect-it-replaced).

**The symptom.** Hard-edged geometric streaks in the frame corners at `kaleido_order = 6`, chevron
debris on the left/right edges at `kaleido_order = 4`, clean only with the fold off (`order < 2`).
Lowering the order relocates the debris rather than removing it. Reproduced headlessly at 2048x1152.

**The cause, from the shader** (`core/src/render/kaleidoscope.rs:63-84`). The fold is a polar
operation on a **rectangular** source: each output pixel keeps its radius `r` and takes a folded
angle `a`, then samples `q = vec2(cos(a), sin(a)) * r`. In aspect-corrected space the source spans
`x` in +/-0.5*aspect and `y` in +/-0.5, so at 16:9 the corner radius is ~1.02 while the source only
reaches 0.889 along the `x` axis. Any output pixel whose radius exceeds the source's extent **in the
folded direction** produces `s_uv` outside `[0,1]`, and the sampler is `ClampToEdge` (`:125`), which
smears the border texel radially. Corners have the largest radius, so they are worst; a higher order
rotates more of that out-of-range region into view.

**It is NOT a Plan 0033 regression — this was tested, not assumed.** A worktree at `3f3b652~1` (the
commit before "the post stages follow the render target"), rendering the *same unmodified*
`swarm_dense.toml` at the same size, produces **the same corner debris**. The arithmetic says why:
the fold's aspect was a baked 1280/720 = 1.7778 before and is a live 1920/1080 = 1.7778 now, and the
shader works in normalized uv, so the fold geometry is the same decision at any 16:9 target. The bug
dates from Plan 0018 Phase 7, when the stage was written.

**What Plan 0033 changed is visibility.** The stage used to render at a fixed 1280x720 and upscale
1.6x to a 2048x1152 surface, which blurred the clamped streaks into something easy to miss; it now
renders at 1920x1080 and presents nearly 1:1, so the same debris is sharp and legible. Plan 0033
**revealed** this, it did not cause it. That distinction decides where `dev` looks: the fix is in the
fold shader, not in `internal_grid_size` or the grid policy.

**Also wrong, and worth correcting in place:** `presets/swarm_dense.toml:45-50` asserts "Six is the
highest that stays clean at 16:9". It is not clean at six, and was not before either. That comment
should go when this is fixed — it currently sends the next author hunting for a safe order that does
not exist.

**Impact:** nine shipped presets bind `kaleido_*` (`fragment_kaleido`, `fragment_glacier`,
`fragment_warp`, `attractor_dejong`, `attractor_lorenz`, `rose_kaleidoscope`, `reaction_reef`,
`swarm_dense`, `swarm_storm`). There is no preset-level workaround short of disabling the fold.

**RE-CONFIRMED 2026-07-28, from `preset-author`, with a cleaner reproduction and two new facts.**
The user reported it again from the running app, unprompted, on the same preset and then on a second
one (`reaction_reef`). Still open; the diagnosis above is unchanged and correct.

- **The A/B is now decisive and trivially repeatable.** `swarm_dense` at `kaleido_order = 6` against a
  byte-identical copy at `order = 1`, same `--signal click:110`, same size: hard bright bars along the
  left and right frame edges and wedges in the corners in **every** frame at 6, completely clean at 1.
  That is a two-command reproduction for whoever takes the fix, and it removes any remaining doubt
  that the swarm scene contributes.
- **Aspect makes it dramatically worse, and this is new.** The user hit it in a **portrait** window.
  The entry's arithmetic explains why and it is worth stating in the general form: the fold keeps each
  output pixel's radius and only changes its angle, so the failure is governed by the ratio between
  the frame's **corner** radius and its **shortest** half-extent. At 16:9 that is ~1.02 against 0.889.
  At the user's roughly 0.44:1 portrait window it is far larger, so most of the frame's area is
  out-of-range rather than just the corners — the artifact stops being corner debris and becomes long
  stripes over the whole picture. Any fix should be evaluated at a non-16:9 aspect, and any test
  pinning it should not be written at 16:9 only. (Same lesson as
  [ADR-0037](adrs/0037-internal-grid-is-a-resolution-not-a-shape.md): the configuration we develop at
  hides it.)
- **Impact list correction.** `rose_kaleidoscope` no longer exists (retired in the 2026-07-28 library
  pass, which cut the rose family from eleven presets to five), and `swarm_dense` now ships
  `kaleido_order = "1"` **specifically to dodge this defect** — the file says so. `lsystem_arrowhead`
  newly binds the fold. Current bindings: `fragment_kaleido`, `fragment_glacier`, `fragment_warp`,
  `attractor_dejong`, `attractor_lorenz`, `reaction_reef`, `swarm_storm`, `lsystem_arrowhead`. So the
  defect has now cost one preset its fold outright, which is the first time avoiding it has changed
  what ships.
- **A second, cheaper mitigation was found and is worth recording as a workaround, not a fix:**
  pinning `kaleido_angle` to a constant. A rotating fold drags the wedge seams through the corners
  continuously, so the debris sweeps and reads as motion; a fixed angle keeps each seam in one place
  where it is far less noticeable. `reaction_reef` ships this. It does not remove the artifact.

**ADR-worthy — a real choice with a real cost on each side:**

- **Clamp the fold radius** to the largest disc the source contains (0.5 in the short axis), so
  out-of-range pixels get a defined result. Cheapest; either letterboxes the fold to a disc or
  leaves the corners flat.
- **Wrap or mirror the address mode** instead of clamping. One line, but it tiles unrelated content
  into the corners — plausible on a field scene, wrong on a centred figure.
- **Fold as a disc and treat the corners deliberately** — sample at `min(r, r_max)` with a radial
  falloff, accepting the corners as a designed vignette rather than folded content.

**Connected to close-review major 3** ([Plan 0033](plans/done/0033-internal-resolution-and-preset-surface.md)):
no golden fixture bound `trails` or `kaleido_*`. **Major 3 is now closed and this is still open** —
[Plan 0035](plans/done/0035-composite-aspect-and-grid-policy.md) Phase 2 added
`core/tests/fixtures/composite_kaleido.toml` at order 6, which **pins this artifact on purpose** so a
fix does not read as a regression. Read that fixture's header before touching the fold.

**Re-bless `composite_kaleido.png` by hand when you fix this — the guard will not tell you to.**
Measured at Plan 0035's close review: the first candidate above (clamp the fold radius to the
inscribed disc, `min(r, 0.5)` in the shader) leaves the capture guard **green** at mean 0.0189 against
its 0.02 tolerance and outlier 22 against 48. It passes while consuming 94 % of the drift budget, so
the *next* unrelated fold change trips the guard with a message blaming the wrong thing. The fixture
header's claim that "a fix must not pass silently" is therefore false as written — a real guard for
this artifact needs a **border-filling** scene (`swarm`/`fragment`) where the clamp smears
high-frequency content, or a direct assertion on the clamped-pixel statistic. Whichever this fix
takes, do it in the same plan.

---

## 0011 — the kaleidoscope fold axis is screen-centred, so `pan_*` and `kaleido_*` are mutually exclusive

- **Raised:** 2026-07-26, from `preset-author`, while unpinning the reaction-diffusion view after
  Plan 0033 Phase 5 made `pan_*` genuinely usable there.
- **Verified against code:** yes. `kaleidoscope.rs:69` centres the fold on `in.uv - vec2(0.5, 0.5)`,
  a hard screen centre. The stage is a `PostStage` and never sees the `ViewTransform`.
- **PROMOTED 2026-07-30 → [ADR-0047](adrs/0047-kaleidoscope-fold-domain-disc-with-falloff.md) +
  [Plan 0045](plans/done/0045-linear-light-and-bloom.md) Phase 1. ~~CLOSED 2026-07-31~~** — shipped
  as the entry's **first** fix shape, `kaleido_center_x` / `kaleido_center_y`, rather than the
  second. Having the fold axis follow the `ViewTransform` was rejected explicitly in ADR-0047: it
  couples a `PostStage` to scene state, which is the wrong direction across that seam. A bindable
  pair gives the author the same result and more, since the centre can be driven independently of
  the pan. The interaction this entry flagged with
  [0010](#0010--the-kaleidoscope-fold-samples-outside-its-source-rectangle-and-clamps-leaving-edge-debris)
  was handled the way it asked — 0010 was decided first, in the same plan, and the off-centre
  case rides on the disc rather than on the old clamp.

A translating `pan_x`/`pan_y` slides the folded rosette off centre and costs the composition its
symmetry, because the source moves under a fold axis that does not. The lane shipped `reaction_reef`
deliberately un-scrolled for this reason and documented it in the file; the three RD presets without
a fold took the scroll happily. An oscillating pan is the only workaround, since it returns to centre.

**Impact:** narrow but sharp — it bites exactly where Plan 0033 just added value. Fix shape is either
a `kaleido_center_x` / `kaleido_center_y` pair, or having the fold axis follow the view transform.
**Note the interaction with 0010:** moving the fold centre away from screen centre makes the
out-of-range region asymmetric and probably worse, so 0010 should be decided first.

---

## 0012 — `--report`'s `cover` metric structurally penalises inverted-polarity (ink) presets

- **Raised:** 2026-07-26, from `preset-author`.
- **Verified by measurement:** `reaction_coral_bloom` reports `cover = 0.128`, the lowest in the
  library, and is healthy — it is the family's ink-on-paper variant (`ink_amount = 1`,
  `paper_bright = 0.965`), a pale botanical print.

**PREMISE CORRECTED 2026-07-26, at promotion time — this is not a metric bug.** The entry as filed
claimed `cover` "measures brightness above the background", so an inverted-polarity look is
"penalised by construction". The code says otherwise: `coverage`
(`core/src/render/metrics.rs:69`) counts pixels where `is_lit` is true, and `is_lit` (`:109`) is
`c.abs_diff(b) > eps` — a **symmetric** difference from the corner-sampled background, on any
channel. Dark-on-light and light-on-dark are measured identically.

So Coral Bloom's 0.128 is a **truthful** reading: only 12.8 % of its pixels differ from its paper.
The chaotic-branching regime genuinely is a sparse print. The number is right; what is missing is any
way for a reader to tell "sparse on purpose" from "dead", which is an **interpretation** gap, not a
measurement one.

**Impact:** downgraded from a measurement change to a documentation sentence — `docs/capturing.md`
should say that a low `cover` is expected and correct for a deliberately sparse or ink-remapped look,
and that the column names suspects rather than convicting them. Folded into
[Plan 0037](plans/done/0037-verifying-easing-transient-probe-and-dynamic-signal.md)'s doc phase
alongside 0014. No code change, no ADR.

**~~CLOSED 2026-07-27~~** — `bca1457` added
[the "A low `cover` is not a defect" section](capturing.md#a-low-cover-is-not-a-defect), naming
`reaction_coral_bloom` at 0.128 as the healthy worked example.

*(Second entry in this batch whose diagnosis inverted under verification — see 0010. Both were filed
in good faith from real symptoms; both attributed the symptom to the wrong mechanism. The lane's
symptom reports are reliable; its causal claims want checking against code before they become work.)*

---

## 0013 — no synthetic signal has transients, so a `[smoothing]` change cannot be verified at all

- **Raised:** 2026-07-26, from `preset-author`, after adopting `{ attack, release }` on 20 presets.
- **PROMOTED 2026-07-26 → [ADR-0039](adrs/0039-verify-easing-with-a-transient-probe-not-a-committed-clip.md) +
  [Plan 0037](plans/done/0037-verifying-easing-transient-probe-and-dynamic-signal.md). ~~CLOSED
  2026-07-27~~**, with its limitation measured rather than assumed: the probe proves easing on a
  purpose-built near-linear fixture (`fall/rise` 20.33 against a scalar entry's 1.03), but over the
  *shipped* set it separates the two populations only **directionally** — asymmetric median 1.02
  against scalar-only 0.61 — because it measures the frame, not the parameter. Neither the probe
  window nor its render resolution is the cause; both were tested. `--signal dynamic:<bpm>` closes
  the stimulus half, and 0008 item 3's calibration question is answered in
  [`capturing.md`](capturing.md#what-real-material-actually-produces), which routed
  **[0020](#0020--the-shipped-library-is-gained-against-stimuli-6-100x-hotter-than-real-music)**.
  What shipped is what was designed: a deterministic transient probe (the primary answer) plus one
  synthesized generator with musical dynamics, with a committed reference clip rejected and the
  calibration numbers taken from a `human` phase instead. **0012 and 0014 rode along as
  documentation** in that plan's Phase 5 and are closed too. Notes retained below as the origin
  record.
- **This is the unresolved half of 0008.** That entry's item 3 asked for a `--signal` matching real
  music levels; Plan 0033 Phase 1 shipped the *measurement* (the band-level report) and *documented*
  the trap, but added no such signal. The trap is now visible and still unavoidable.

Two independent blockers, both measured:

- **`--report` measures settled response**, so it is identical before and after any easing edit by
  construction. Kaleido Field: bass 0.228 / mid 0.153 / treb 0.131 before AND after a full smoothing
  rework.
- **Every `--signal` kind is effectively steady-state**, per the Phase 1 band report: `bass:60` gives
  0.187 / 0.187 / 0.187 (zero variance), `chord` 0.058 / 0.059 / 0.060, `noise:7`
  0.012 / 0.022 / 0.039. `click:120` is the only transient kind and peaks at **0.011** — roughly 50x
  below the levels the library is gained for.

**Impact: the widest blast radius in the batch.** ADR-0035's asymmetric easing is a capability whose
entire value lives in the transient, and the harness cannot see a transient. Every easing edit in
`a070f5a` and `8b5b2e0` rests solely on the user watching the running app, and the same hole will
make the next easing feature unverifiable too.

Cheapest credible fix is one short committed reference clip (`--audio` already exists and reads
16-bit PCM WAV) plus a transient-response measure — rise/fall time to a step. Harness work, but not
merely documentation this time.

---

## 0014 — the line scenes' cosine `hue` ramp is not a hue wheel, and nothing documents it

- **Raised:** 2026-07-26, from `preset-author`, choosing a glow colour for Fern Grow.
- **Verified by a rendered six-way sweep**, whose *conclusion* held and whose *colour names were
  wrong*. The sweep established that the ramp is not a hue wheel and that every prediction from the
  name (0.06 = amber, 0.17 = gold-green, 0.62 = violet) missed — which is the finding, and it stands.
  The six names it recorded (0.06 lavender, 0.17 turquoise, 0.30 cyan, 0.46 near-white/green,
  0.62 gold, 0.82 rose) **name the ramp roughly 0.16 further along than the shader produces** and are
  superseded.
- **CORRECTED 2026-07-27 at Plan 0037's close.** `palette(t)`
  (`core/src/render/scenes/lines/mod.rs:117`) is three cosines phased 0.10 / 0.42 / 0.62, giving
  **0.06 magenta, 0.17 orchid, 0.30 cornflower blue, 0.46 aqua, 0.62 mint, 0.82 amber**. Settled by a
  **15-point rendered sweep measuring the median chromaticity of each frame's unclipped lit pixels**,
  not by arithmetic alone: it tracks `palette(t)` at every point and is nowhere near
  `palette(t + 0.16)`. Re-derived independently at review. **The 20-row table now in
  [`preset-palettes.md`](preset-palettes.md#the-line-scenes-cosine-ramp--what-hue-actually-looks-like)
  is the verified ramp; read it rather than any figure in this entry.**

Three of the four line scenes (`parametric_curve`, `lsystem`, `star_pattern`) ignore `[palette]`
entirely and colour through their own cosine ramp, so `hue` is their *only* colour control — and its
mapping is undocumented and not the hue wheel the name implies. Picking a colour costs a render
round-trip every time. (`spectrum`, added by Plan 0034, does read `[palette]`, so it is not affected;
the swatch table below is still owed for the other three.)

**Impact:** small, recurring, purely documentation — a swatch table in `docs/preset-palettes.md` (or
a generated strip committed as an image) closes it. Bundle with any other doc sweep.

- **PROMOTED 2026-07-26 → [Plan 0037](plans/done/0037-verifying-easing-transient-probe-and-dynamic-signal.md)
  Phase 5. ~~CLOSED 2026-07-27~~** — the swatch table shipped in `bca1457`, with `presets/README.md`
  pointing at it from both places that mention `hue`. Retained above only because the correction is
  worth remembering.

---

## Entries 0015-0019 — the 2026-07-27 batch (third, post-Plan-0034)

Surfaced by the Plan 0034 **close review** and by the lane's first adoption pass (`037825d`), which
put `bin()` into five curated presets, then extended the same day by a second `preset-author` pass
over the `spectrum` scene itself. Most of the review batch was fixed in the close (`ca99cb1` the
`shot` report stimuli and the palette wrap seam, `4d41884` the band-axis documentation); these five
are what survived as open design questions.

**0016, 0017, 0018 and 0019 are one theme: the levers exist in the engine but not in the preset
surface.** Three are world-space constants that should be params (`SPAN_X`, `BASELINE_Y`, the level
curve) and one is a renderer argument already plumbed and hardcoded (`glow`). They want designing
**together as one plan** rather than four drive-by fixes — 0016 and 0018 both move scene geometry
and would otherwise be tuned twice, and 0017 carries the batch's only genuine ADR question. 0015 is
separate: it is DSP, not the preset surface.

---

## 0015 — the band axis is half linear, and it is undecided whether that is a defect

- **Raised:** 2026-07-27, from the Plan 0034 close review.
- **Verified against code and by independent computation** (`core/src/dsp/fft.rs:56-76`, replicated
  numerically at 48 kHz).

`SpectrumAnalyzer::new` lays the 64 band edges on a log curve from `BAND_LO_HZ = 35` to
`BAND_HI_HZ = 18_000`, then runs a fix-up loop guaranteeing every band **at least one FFT bin**. At a
2048-point window that floor is `sample_rate / 2048` = **23.4 Hz at 48 kHz**, and it binds from band
1 all the way to **band 30 (~750 Hz)**. So:

- **31 of the 64 bands are linear 23.4 Hz slices, not logarithmic.** The array has two regimes with a
  crossover at `x ~ 0.48`.
- **The low end is the array's musically coarsest region, not its finest.** Band 0 spans 23-47 Hz —
  **a full octave in one number**. Band 8 is 1.8 semitones, band 20 is 0.81, band 30 is 0.55.
  Resolution *peaks* around 500-800 Hz and settles at a constant ~1.7 semitones above ~1 kHz.
- **Below the crossover the mapping moves with the sample rate.** The log half is stable; the linear
  half is not, so the same `bin(x)` means a different frequency at 44.1 kHz than at 48 kHz.

**The documentation half is already closed** (`4d41884`: `docs/presets.md` and `presets/README.md`
carry a measured position table and both consequences). **What is open is whether the axis itself
should change**, and it is a real decision with real alternatives:

- **Leave it, documented.** Free. But a preset's low-end probes are sample-rate dependent, and the
  bottom two octaves — where kick and bass live, the most-reached-for region in this whole surface —
  are the least resolved part of a "log-spaced" array.
- **A longer analysis window** (4096 pushes the floor to 11.7 Hz, halving the linear span) costs
  latency and CPU on the hot path, and NFR budgets would have to be re-argued.
- **Let the edges respect the bin floor** — allocate the 64 bands over a range the window can
  actually resolve logarithmically, e.g. starting nearer 250 Hz, rather than pretending below it.
  Changes what every existing `bin()` position means, so it is breaking for the five presets in
  `037825d` and the three `spectrum_*` ones.

**Impact:** currently documentation-only, but it is load-bearing for the lane's most common reach
(bass-region probes) and it interacts with the deferred `bin_range(lo, hi)` followup — a range
integrator would paper over the resolution question without answering it. **ADR-worthy if acted on**;
the alternatives above are the ones to weigh.

**Checked empirically 2026-07-27** (Plan 0037 Phase 4, opportunistically, while real audio was
already on the meter). Drove `Spectrum Comb` from a trap clip with a prominent 808 sub, peak-normalized
to -1 dBFS. On every 808 hit the entire kick-and-sub region collapses into the **first one or two
elements** with no internal structure, while the rest of the array reads as a flat ridge — the
resolution table's arithmetic, visible. **The user's call: this is a real limitation, not a documented
curiosity.** The bottom two octaves are where a `bin()` binding is reached for most often, and they are
the one part of the array that cannot distinguish anything.

So this entry is **no longer documentation-only**. It should be promoted to an ADR weighing the three
alternatives above; the empirical half of its impact question is now answered.

**Routed at Plan 0037's close (2026-07-27): this is the repo's next ADR-worthy design item**, ahead
of the rest of the open backlog. Two things a design here inherits and must not rediscover. First,
it is **breaking**: the eight presets that reach `bin()` (`037825d`'s five plus the three
`spectrum_*`) encode positions against today's axis, and the third alternative — re-laying the edges
over a range the window can actually resolve — moves every one of them. Second, the
`bin_range(lo, hi)` followup deferred from [ADR-0036](adrs/0036-preset-reachable-spectrum.md) is
**not** an answer to this and should not be allowed to look like one: integrating over a range the
array cannot resolve returns the same undifferentiated number more smoothly. Interview before
drafting — the three alternatives trade latency, CPU on the hot path, and breakage against each
other, and only the user can price the last one.

---

## 0016 — the `spectrum` readout has no width control, and density makes it worse

- **Raised:** 2026-07-27, Plan 0034 close review minor 1; **re-raised and sharpened the same day**
  by the `preset-author` lane with the fix's binding constraint.
- **Verified against code:** `core/src/render/scenes/lines/spectrum.rs:78` — `SPAN_X = 1.0` is a
  **constant**, and the figure spans 2 world units, which the line renderer maps to the frame
  **height**. At 16:9 that is about **56 % of the width**, less on an ultrawide.

`zoom` is no substitute: it scales the whole figure about the frame centre, so widening the readout
also lifts its baseline off the bottom and grows the element lengths. There is no `span`/`width`
param in that scene's `PARAMS`, so a full-width bar readout — the single most conventional form this
scene has — is not authorable.

**It compounds with element count.** `MAX_ELEMENTS = SPECTRUM_BINS` (`spectrum.rs:73`) is the right
ceiling — a readout finer than its own data would be a lie — but **64 bars crammed into 56 % of the
width are hairs**. The width limit is what makes the top of the legal range unusable, so the two are
one problem, not two.

> **Binding constraint on any fix: the width must stay a WORLD quantity.** A scene that reads its
> render target's aspect to size itself is precisely the
> [ADR-0037](adrs/0037-internal-grid-is-a-resolution-not-a-shape.md) trap, which has already shipped
> twice in this codebase. The param sets a world span; the renderer's existing aspect handling maps
> it. Do not "fix" this by having the scene ask how wide the window is.

**Impact:** small and contained — one named param on one scene, no new idiom, no ABI or trait change.
Not ADR-worthy on its own, but see **0018**: it and this are the two halves of "the readout's shape
is pinned by constants", and they should be designed together.

- **PROMOTED 2026-07-27 → [Plan 0038](plans/done/0038-line-family-unreachable-levers.md) Phase 2
  (with 0018, as this entry asked). ~~CLOSED 2026-07-28~~** — `f3945be` made `span` a bound
  **world** half-width defaulting to exactly the old `SPAN_X = 1.0`, with a unit test asserting no
  aspect or target size is read anywhere in the scene. The binding constraint held: `span ≈ 1.78`
  fills a 16:9 frame and leaves an ultrawide short, and `presets/README.md` states that rather than
  offering a `fit` mode. `spectrum_comb` and `spectrum_ridge` now ship at `1.72`.

---

## 0017 — `[spectrum]` has no level curve, and the grammar has no `log`, so a dB readout is impossible

- **Raised:** 2026-07-27, from `preset-author`.
- **Verified against code:** yes, on all three legs.

`element_length` (`spectrum.rs:214`) is **`base + scale * level`** — strictly linear, with no shaping
lever. Audio levels are perceptually logarithmic, so a linear readout spends most of its range on the
loudest element and leaves everything else stubbed; a dB-like curve is the conventional answer and it
is **not reachable from a preset**:

- The grammar has `sqrt` and `pow` but **no `log`** (`Func::from_name`, `core/src/preset/expr.rs`) —
  confirmed absent, not overlooked.
- **The one reachable workaround silently breaks easing.** Driving `base` from `bin(index)` with
  `scale = 0` does shape the length — but `[spectrum] smoothing` eases the internal `levels`, which
  that formulation **discards**. So the author trades the scene's only temporal easing for the curve,
  and nothing warns. Since the bands are the rawest signal in the engine, that easing is exactly what
  keeps the readout from strobing.

**This must be engine work** — no preset-level composition reaches it. A `[spectrum] curve` key is
the obvious shape, and **the real design question is whether the curve applies before or after the
per-element easing**, which changes the behaviour materially: easing a curved value smooths what the
eye sees, while curving an eased value keeps the smoother operating in the linear domain the
`{ attack, release }` constants were reasoned about in. That is a genuine either/or with a
consequence worth recording — **ADR-worthy**.

Adding `log` to the expression grammar is the *other* candidate and is broader (it would serve every
system, not just this one), but it does not fix the easing-bypass leg on its own. Weigh both.

- **PROMOTED 2026-07-27 → [ADR-0040](adrs/0040-spectrum-level-curve-applies-before-the-easing.md) +
  [Plan 0038](plans/done/0038-line-family-unreachable-levers.md) Phases 3 and 4 — both candidates,
  as this entry asked. ~~CLOSED 2026-07-28~~** — `c9121fd` shipped `curve` as a bindable exponent
  applied **before** the easing, and `e31ae88` shipped `log(x)`. The either/or this entry called
  ADR-worthy was decided, then **measured and half-falsified**: the ordering stands, but not for the
  "perceptually even fall" reason — see the ADR's Outcome. Read the two consequences it left behind
  before quoting this entry's framing: an even fall is unreachable in any ordering
  ([0021](design-backlog.md#0021--an-even-fall-is-not-reachable-with-a-one-pole-in-any-ordering)), and `--report`
  cannot see a curve at all ([0022](#0022--reports-reactivity-columns-are-structurally-blind-to-a-level-curve)).

---

## 0018 — `BASELINE_Y` is a constant, so `mirror_reflect` throws the copy to the top of the frame

- **Raised:** 2026-07-27, from `preset-author`. **Rendered and confirmed**, not inferred.
- **Verified against code:** `spectrum.rs:81` — `BASELINE_Y = -0.85` is a **constant**. The geometry
  mirror reflects across the **x-axis** (`lines/mod.rs:227`: `let y = if reflected { -p[1] } else
  { p[1] }`).

Bars stand *upward* from `y = -0.85`, so a reflected copy stands *downward* from `y = +0.85` — it
lands against the **top edge of the frame** rather than mirroring about a shared centre line, which
is what `mirror_reflect` means on every other line scene. The symmetric "landscape and its
reflection" figure the ridge/polyline layouts want is therefore not authorable.

**`pan_y` cannot correct it**, and the reason is structural rather than a tuning miss: the mirror
runs in `update()` on **world** coordinates, while the view transform is applied later **in the
shader**. Panning moves the mirrored pair together; it cannot move the reflection axis relative to
the figure.

**Impact:** contained, and naturally paired with **0016** — both are "a world-space constant in this
scene should be a param", both are one named param, and a fix touching `BASELINE_Y` wants to think
about `SPAN_X` at the same time. Same ADR-0037 constraint applies: world quantities only.

- **PROMOTED 2026-07-27 → [Plan 0038](plans/done/0038-line-family-unreachable-levers.md) Phase 2
  (with 0016). ~~CLOSED 2026-07-28~~** — `f3945be` made `baseline` a bound world y, defaulting to
  exactly the old `-0.85`, and **fixed this by moving the figure rather than by special-casing the
  mirror**: no new mirror semantics, the reflection is still across the x-axis on every line scene
  alike. `baseline = 0` is the centre-mirrored readout, pinned by a test that counts the distinct
  foot lines (one at `0`, two at `-0.85`/`+0.85`). `spectrum_ridge` ships it, and the sentence its
  header had claimed aspirationally since it was written is now true.

---

## 0019 — `glow` is unreachable from a preset on all four line scenes

- **Raised:** 2026-07-27, from `preset-author`.
- **Verified against code:** `LineRenderer::draw(queue, encoder, view, aspect, glow, xform,
  segments)` already takes a `glow` argument, and **every one of the four call sites passes a
  hardcoded `1.0`** — `parametric.rs:291`, `lsystem.rs:288`, `star.rs:271`, `spectrum.rs:639`.

The plumbing exists end to end; only the binding is missing. This is the **cheapest item in the
batch by a wide margin**, and unlike the rest it is not spectrum-specific — it lands on the rose,
the L-system and the star as well, which is most of the line library.

**It is the renderer's per-segment falloff, not a post-process bloom.** Backlog **0005** (above) is
still the separate, larger stage. Worth deciding
deliberately whether this ships *ahead* of 0005 — it is nearly free and immediately useful — or
waits so the two are designed as one coherent luminance story. **Recommendation: ship ahead.** A
per-segment falloff param and a screen-space bloom are different tools an author would reach for
differently, and holding a one-line win behind an undesigned stage has no payoff.

- **PROMOTED 2026-07-27 → [Plan 0038](plans/done/0038-line-family-unreachable-levers.md) Phase 1,
  ahead of 0005 as recommended. ~~CLOSED 2026-07-28~~** — `a1c67f4` bound `glow` on all four line
  scenes at a default of exactly `1.0`, goldens byte-identical. The range question the entry did not
  ask was answered by the non-vacuity measurement: **downward has more range than upward** (0.25 vs
  0.17 per lit pixel on the rose), because strokes blend additively into an 8-bit target so `glow`
  above 1 saturates the core and only widens the skirt. `lsystem_arrowhead` ships `0.55` and
  `spectrum_comb` `0.75`. **0005 is untouched and still open** — this is the renderer's per-segment
  falloff, not a post-process bloom, and `presets/README.md` says so beside the param.

---

## Entry 0020 — from the Plan 0037 measurement phase

Not part of the 0015-0019 batch above. Raised by the one thing in this repo only the user can do:
play real music through the harness and read the meter.

---

## 0020 — the shipped library is gained against stimuli 6-100x hotter than real music

- **Raised:** 2026-07-27, from Plan 0037 Phase 4 (the `human` measurement phase), which routes this
  here rather than fixing it: re-gaining the set is a content-lane pass with its own scope.
- **Verified by measurement** through `--audio` on three local clips, peak-normalized to -1 dBFS.
  Numbers and material descriptions are in [`capturing.md`](capturing.md#what-real-material-actually-produces).

Real material produces bass **means** of `0.000`-`0.007` and **peaks** up to `0.190`. Every stimulus
an author has actually been able to reach sits above that:

| stimulus | bass | vs. a real mean |
|---|---|---|
| `--set bass=0.8` | `0.800` | ~100x |
| `--signal bass:60` | `0.187` | ~25x |
| `--signal dynamic:110` (Plan 0037 Phase 3) | mean `0.040` | ~6x |

**The peak is not the problem — the mean is.** An 808's bass peak (`0.190`) lands on a full-scale
60 Hz sine (`0.187`), so percussive bindings calibrated against a synthesized tone are roughly right.
Continuous bindings are not: a size, a zoom or a hue drift spends its life near the *mean*, which is
25x lower than the loudest thing an author could previously synthesize and 100x lower than the
`--set` magnitude most of the library was authored against.

**What is NOT yet known** and would gate the work: how much of the shipped set is actually
mis-gained. A binding reading `bass * 0.4` on a preset whose look tolerates a wide range is fine; one
gating a `select()` threshold is not. Nobody has audited which is which.

**Impact:** content-lane, potentially library-wide, and it wants evidence before it wants a plan. The
cheap first step is a pass with `--signal dynamic:110` over the library looking for presets that
barely move — that is now possible and was not before. No ADR: this is tuning, not architecture,
unless the audit turns up a *grammar* gap (e.g. no way to express "normalize this band against its
own recent range"), which would be its own entry.

### THE AUDIT IS DONE (2026-07-28, from `preset-author`) — and it found a second, worse failure mode

The gating question above — *"how much of the shipped set is actually mis-gained, and nobody has
audited which is which"* — is answered. The lane ran the `--signal dynamic:110` pass this entry asked
for while retuning the library against live user feedback. **Both halves of the prediction held, and a
third thing turned up that this entry did not anticipate.**

**The measurement it was calibrated against** (`shot --signal dynamic:110`, printed by the harness):

| band | min | mean | max |
|---|---|---|---|
| bass | 0.004 | **0.040** | 0.106 |
| mid  | 0.000 | **0.006** | 0.019 |
| treb | 0.000 | **0.006** | 0.032 |

**The new failure mode: a comparison gate that can never fire.** This entry framed the defect as
*gain* — a binding that moves too little. That is real, and widespread (`clamp(bass * 0.4, 0, 0.11)`
moves a param by 0.016 on real music, ~15 % of its own cap). But a `select()` threshold written
against `--set` magnitudes is not merely weak, it is **dead code**: the branch never evaluates, so
the mechanism the preset is *built around* has never run once in the program's life. The three-band
sum peaks near **0.157** on real audio; the shipped thresholds were written as if it reached 3.

Confirmed dead before this pass, each one the preset's headline mechanism:

| preset | gate as shipped | consequence |
|---|---|---|
| `fragment_kaleido` | `bass + mid + treb > 0.90 / 0.55 / 0.25` | frozen at 6 folds — the entire audio-driven-symmetry idea the preset exists to demonstrate had never run |
| `reaction_reef` | `bass + mid > 0.40` | never folded; the family's designated *figure* preset rendered as flat texture |
| `lsystem_arrowhead` | `mid + treb > 0.50 / 0.28 / 0.10`, `mid > 0.22 / 0.08` | never subdivided past its coarsest depth, stuck at 4 mirror copies — "boring" was wiring, not taste |
| `fragment_glacier` | `bass + mid > 0.42` | never folded |
| `rose_overflow` | `floor(2 + clamp(bass * 7, 0, 3.72))` | petal count never stepped off 2 |
| `swarm_storm` | `min(tempo > 132, bass + mid > 0.40)` | the conjunction's second term was permanently false |

**Still dead today** — the lane fixed only what the user flagged, and these were not in scope:
`attractor_dejong` (`bass + mid > 0.34`), `attractor_lorenz` (`bass + treb > 0.38`), `fragment_warp`
(`bass + treb > 0.55 / 0.30`). Three more folds that have never engaged. The wider un-swept set is
all five `attractor_*`, `fragment_aurora`, `fragment_pulse`, `fragment_warp`, `lsystem_fern`,
`star_rosette`.

**Why nothing caught this, which is the part that matters.** `--report` scored every one of these
presets as healthy while they were inert, and the reason is
[0022](#0022--reports-reactivity-columns-are-structurally-blind-to-a-level-curve)'s: **the report's
band stimuli drive their bands to full scale.** At `bass = 1.0` every gate above fires happily. The
harness the lane self-verifies through is the one instrument that could not see the defect — and the
same full-scale stimulus is 0022's root cause too. That is no longer a coincidence between two
entries; it is one property of the report causing two different classes of blindness.

**So the promotion condition 0022 named is now met.** That entry says it becomes ADR-worthy *"only if
bundled with 0020 into a decision about what level the report's stimuli should represent, which is a
real question with a real rejected alternative."* Bundled, the decision is:

- **What level should `--report`'s stimuli represent?** Full scale is reproducible and
  sample-rate-independent; realistic levels are neither, but they are the only ones that measure what
  a preset actually does. A second low-level column, so compression and dead gates both show as the
  *gap* between two readings, is the third option.
- **Should the harness detect an unreachable gate directly?** This is new and cheap and would have
  caught all nine presets mechanically: evaluate each `select()` condition across the run and flag any
  that never changes value. It is a property of the *expression*, not of the frame, so it is immune to
  whatever the stimulus level is — which arguably makes it the more robust half of the answer.

**Recommended split when this is promoted:** the harness/report decision is an ADR plus a small `dev`
plan; the library re-gain of the ~10 un-swept presets is a separate `preset-author` content pass that
should follow it, so the sweep can be verified by an instrument that can see the defect. Doing the
content pass first — as happened here — means re-verifying it later anyway.

**~~HARNESS HALF CLOSED 2026-07-29~~ by [Plan 0041](plans/done/0041-report-two-level-stimuli-and-expression-reachability.md)
+ [ADR-0042](adrs/0042-reachability-measured-on-the-expression-tree.md).** The split above was taken
exactly as recommended, and this is the first half. `--report` now reads at realistic levels beside
its full-scale ones, and walks every preset's expression tree to name any `select()` whose condition
never went both ways. Run over the shipped set it flags `attractor_dejong` (`bass + mid > 0.34`),
`attractor_lorenz` (`bass + treb > 0.38`) and `fragment_warp` (`bass + treb > 0.55`) — the three this
entry lists as *still dead today* — plus `lsystem_fern` and `star_rosette`, which nobody had named.
The instrument can now see the defect it was blind to. Authors are told the measured range where
they write a threshold: `presets/README.md` and `docs/presets.md` both carry the table.

**The content half stays open.** Re-gaining the ~10 un-swept presets (`attractor_*` x5,
`fragment_aurora`, `fragment_pulse`, `fragment_warp`, `lsystem_fern`, `star_rosette`) is a
`preset-author` pass and is now unblocked — and verifiable, which is the whole reason it was
sequenced second. `--report`'s new columns and flags are the acceptance check for it.

---

## 0022 — `--report`'s reactivity columns are structurally blind to a level `curve`

**Raised by `preset-author`, 2026-07-28, while verifying Plan 0038 Phase 6.** Not a wall this lane
hit while authoring — the presets landed fine — but a wall it hit while *proving* they landed fine,
which is worse in a different way: the measurement disagreed with the render and the render was right.

**What happened.** `spectrum_comb` adopted `curve = 0.62` with the `scale` retune it forces (2.6 →
1.75). Rendered against `--signal dynamic:110` the change is exactly what the curve is for: the shelf
of quiet elements carries visible shape in every frame where it used to be a run of near-identical
stubs. `--report` recorded the opposite — `bass` 0.084 → 0.068, `treb` 0.047 → 0.020 — reading as a
substantial loss of reactivity on a preset that had just become *more* legible.

> Those are the values at the moment this was raised; the preset was retuned again the same day and
> ships `curve = 0.85` / `scale = 10.0`. The numbers are left as the worked example because the
> defect is **structural** — the report is blind to a curve at *any* exponent, so nothing about the
> argument moves with the tuning.

**Why, and it is not a tuning accident.** The report's band stimuli drive their bands to **full
scale**, and `curve` is `level^curve`, so at a level of `1.0` it is the **identity** — `1^0.62 = 1`
at any exponent. The compression only exists below 1.0. So the reactivity columns see the `scale`
cut, which is the *price* of the curve, and are mathematically incapable of seeing the compression
that price bought. The stronger the curve, the bigger the apparent regression.

This is the same family as [0020](#0020--the-shipped-library-is-gained-against-stimuli-6-100x-hotter-than-real-music)
— stimuli hotter than real music — but it is not the same defect and 0020's fix would not fix it.
0020 is about *gain calibration* being wrong at full scale; this is about a whole parameter being
**invisible** at full scale. A preset could set `curve = 0.05` and the report would show nothing but
the scale cut.

**Consequences today.** Any future `curve` adoption will look like a regression in the table, and the
obvious "fix" — putting `scale` back — is precisely the wrong move and would send the readout off the
top of the frame. The numbers are in Plan 0038 Phase 6's commit with this caveat attached, so the
record is honest, but the next author has no reason to expect it.

**The shape of a fix, unranked** — this lane names the friction, `architect` picks:
- a reactivity stimulus at a **realistic level** rather than full scale (interacts with 0020, and
  arguably they should be decided together);
- or a second reactivity column measured at a low level, so compression is visible as the *gap*
  between the two;
- or simply document it in `docs/capturing.md` beside the existing "what the transient columns cannot
  see" section, which is the cheap honest option and costs nothing but a paragraph.

**Probably not ADR-worthy on its own** — the third option is a doc fix. It becomes ADR-worthy only if
bundled with 0020 into a decision about what level the report's stimuli should represent, which is a
real question with a real rejected alternative (full-scale stimuli are reproducible and
sample-rate-independent; realistic ones are neither).

**~~CLOSED 2026-07-29~~ by [Plan 0041](plans/done/0041-report-two-level-stimuli-and-expression-reachability.md)
+ [ADR-0042](adrs/0042-reachability-measured-on-the-expression-tree.md)** — the promotion condition
this entry named was met, it *was* bundled with 0020, and the second of the three unranked shapes
above is what shipped. `--report` keeps its full-scale columns and prints a realistic-level reading
in a second block under each family, so a level `curve` shows up as the **gap** between the two
readings rather than being mathematically invisible. `docs/capturing.md` carries a direction table
for reading that gap — including the case this entry hit, where full scale looks lively and the
realistic reading does not — plus what the pair still cannot see (`beat` is an event, and the band
array is on its own scale, hotter than the scalars). The third option, documenting it and nothing
else, was not taken: the first two were affordable together.

---

## 0023 — `LineRenderer` has no line joins, so every direction change leaves a notch

**Raised by `preset-author`, 2026-07-28, from a user looking at `spectrum_ridge` full-screen.** The
report was "there are gaps between lines, looks very strange" — visible as a thin dark tick across
the stroke at *every* vertex, on gentle slopes as well as sharp ones. It is not a preset defect and
no `[params]` value fixes it.

**The cause, from the shader** (`core/src/render/scenes/lines/renderer.rs`, `SHADER`):

```wgsl
let nrm  = vec2<f32>(-dir.y, dir.x);
let base = mix(a_s, b_s, c.x);
let pos  = base + nrm * c.y * width;
```

Each segment is an independent rectangle built from **its own** perpendicular, and nothing joins
consecutive ones. Where the direction changes by `theta`, the two rectangles share the centre point
but their outer corners diverge, leaving a wedge on the outside of the turn (and a double-covered
overlap on the inside). The gap's width goes as `width * tan(theta/2)`, so it is visible at any turn
once the stroke is thick enough — which is why it reads at every vertex rather than only at corners.

**Why it surfaced now, and why it will keep surfacing.** The four line scenes are affected equally,
but the three generator-driven ones (`parametric_curve`, `lsystem`, `star_pattern`) draw smooth
figures whose consecutive segments are nearly collinear, so `theta` is small and the notch is
sub-pixel. **`spectrum` with `layout = "polyline"` is the opposite**: consecutive points are adjacent
frequency bands, which are genuinely uncorrelated, so `theta` is large and arbitrary. Plan 0038 made
this much worse by design — the whole point of the `curve` lever is to give neighbouring elements
*more* height contrast, and height contrast on a polyline is exactly turn angle.

**The only preset-side lever is stroke width**, since the notch scales with it. `spectrum_ridge` now
carries `thickness = 4.2` — thinned from `5.0`, but not as far as the artifact wanted, because
thinning far enough to hide the notch drops the figure under the `animation` gate's motion floor.
That is a real cost paid to a renderer limitation. Raising `elements` does **not** help and slightly hurts: more points over a fixed `span`
shortens the x-step while the y-differences stay, which steepens every turn.

**The cheap fix, if it is wanted.** Extend each segment quad by `width` along its own direction
(`base = mix(a_s - dir*width, b_s + dir*width, c.x)`), so consecutive quads overlap by half a stroke
and the notch is covered. That is a two-line vertex-shader change, costs no extra geometry and no
extra draw, and is a decent approximation of a round join for a soft-falloff stroke. It does lengthen
every stroke by one width at each end, which is visible on a *short* isolated segment — the
`spectrum` `bars` layout is the case to check, since there each element is one short segment and the
bars would grow by a stroke width at both ends.

The fuller fixes, both more expensive: a real miter join (extra vertices per joint, needs a
miter-limit rule for near-180-degree turns), or a round join drawn as a disc per interior vertex
(one extra instanced quad per point, simplest to reason about and the usual choice for a glowing
stroke).

**ADR-worthy?** Probably not on its own — it is a defect fix inside an existing renderer, not a new
capability or a rejected-alternative decision. It wants a small plan with a golden-baseline
re-bless, because *every* line-scene golden moves. Worth noting the re-bless is the main cost here,
not the shader edit.

- **PROMOTED 2026-07-28 → [ADR-0041](adrs/0041-line-joins-are-per-endpoint-on-the-segment-instance.md)
  + [Plan 0039](plans/done/0039-line-joins.md). ~~CLOSED 2026-07-28~~** — and the "ADR-worthy?"
  reading above is the part this entry got wrong, which is why it turned into an ADR after all. The
  cheap fix it proposed (extend every quad unconditionally) is **incorrect**, for exactly the reason
  the entry half-spotted in its own `bars` caveat: at `spectrum_comb`'s `thickness = 13` an
  unconditional extend grows a bar ~60 % at rest and hangs it below `baseline`. The shipped shape is
  a **per-endpoint** flag on `SegmentInstance`, so a producer that flags nothing is byte-identical.
  `spectrum_ridge`'s `thickness` re-tune — the cost this entry priced — is Plan 0039's Phase 5 and is
  **still open**. One residue: the star rosette's contact points were mis-analysed as free ends and
  remain unjoined; that is **0024** below.

---

## 0024 — the star rosette is a closed chain, and half its joints are still unjoined

- **Raised:** 2026-07-28, from `architect`'s Mode 4 review of [Plan 0039](plans/done/0039-line-joins.md).
- **Verified against code:** yes — `core/src/render/scenes/lines/hankin.rs`, `star_rosette`.

[ADR-0041](adrs/0041-line-joins-are-per-endpoint-on-the-segment-instance.md)'s connectivity table
says the star produces "pairs meeting at a shared petal tip … `m0`/`m1` are free", and Plan 0039
Phase 3 implemented exactly that: both rays of a petal carry `JOINED_B` and both contact points stay
free. **The contact points are not free.** Petal `k` emits segments starting at `contact(k)` and
`contact(k + 1)`; petal `k + 1` emits one starting at `contact(k + 1)` again. So the figure is a
closed chain — `contact(0) -> tip(0) -> contact(1) -> tip(1) -> …` — and every one of its `2n`
vertices is a joint. Only the `n` tips were flagged.

**It is the sharper half that was missed.** The two rays leave a contact point `2 * contact_angle`
apart, so the through-turn is `pi - 2 * contact_angle` and the wedge is
`half_width / tan(contact_angle)` — bigger than a half-width for any star pointier than 45 degrees,
against a `contact_angle` clamped as low as 8 degrees (`star.rs`, `CONTACT_MIN_DEG`). The notch that
survives at a contact point is therefore wider than the one that was removed at the tip.

**The shape of a fix.** Two lines: both segments at a contact point take `JOINED_A` as well, so
`out.push(seg(m0, tip, JOINED_A | JOINED_B))` for both rays. It costs a `star_pattern` golden
re-bless and an amended test — the shipped
`the_star_joins_in_pairs_at_the_petal_tip` asserts only that the two contact points *within* a pair
are distinct and is silent about the sharing *across* pairs, so it would pass unchanged today. Worth
checking by eye first: a contact point is a near-reversal, which is the case ADR-0041 accepts as a
slightly bright bead rather than a gap, and on a pointy star that bead sits on the outer circle where
it may read as a deliberate stud or as a defect.

**Not ADR-worthy** — the mechanism is already decided and the per-endpoint flag expresses the closed
chain exactly. This is an unfinished application of it. Small enough to ride along with the next plan
that touches the line family, or with Plan 0039's open Phase 5.

- **CLOSED 2026-07-28 by [Plan 0040](plans/done/0040-line-joins-finish-the-job.md) Phase 3**
  (`0bc33a6`). Both segments at a contact point carry `JOINED_A | JOINED_B`, all `2n` vertices are
  flagged, the silent test was replaced with one that asserts the sharing *across* pairs, and
  `star_pattern.png` is the only baseline that moved. The "worth checking by eye first" caution was
  honoured and **came back the other way round**: the bead is more distinct at a *wide* turn than at
  the 8-degree floor, where the two extensions merge into the already-bright core and the point just
  ends in a point. Neither reads as a defect; no miter limit, no route-back. Recorded in
  [ADR-0041](adrs/0041-line-joins-are-per-endpoint-on-the-segment-instance.md)'s Outcome and in
  `presets/README.md`'s line-art notes.
- **PROMOTED 2026-07-28 → [Plan 0040](plans/done/0040-line-joins-finish-the-job.md) Phase 3**, the same
  day it was raised, alongside the review's two other code findings (the shader's untied bit
  literals, and the missing pixel pin under the reported defect). The plan takes this entry's
  proposed fix as written and adds the thing this entry only gestured at: the look. A joined contact
  point is a near-reversal, so it reads as a bright bead, and Phase 3 makes "capture it at the
  8-degree floor and say what it looks like" a done-when with a **stopping condition** rather than a
  note.

---

## Entries 0025-0027 — the 2026-07-28 `preset-author` batch (fourth), from the full library retune

Raised while retuning most of `presets/` against live user feedback — the same pass that answered
[0020](#0020--the-shipped-library-is-gained-against-stimuli-6-100x-hotter-than-real-music)'s audit
question and re-confirmed [0010](#0010--the-kaleidoscope-fold-samples-outside-its-source-rectangle-and-clamps-leaving-edge-debris).
Those two are updated in place above rather than duplicated here. These three are new.

---

## ~~0025 — `swarm` cannot express a flock: no depth, no cohesion, and its field frequency is a constant~~

- **Raised:** 2026-07-28, from `preset-author`. The user, on the whole swarm family: *"swarms still
  looks lame. they should look like floks of birds, swirling and dancing in 3d-like space"*.
- **Verified against code:** `core/src/render/scenes/swarm.rs`.
- **PROMOTED 2026-07-29 → [ADR-0044](adrs/0044-swarm-world-is-a-25d-torus-sized-from-the-target.md) +
  [Plan 0043](plans/done/0043-swarm-depth-and-domain.md); closed 2026-07-30.** Both open items are
  delivered, and **this entry's own recommendation set the order**: `field_freq` was taken alone
  first (Phase 2, defaulting to exactly the `2.3` it replaced), and it turned out to be the family's
  first *structural* separator — the three surviving presets now sit at ~1.9 / ~3.0 / ~5.2 of it. The
  depth half took the **2.5D fake this entry named as the rejected alternative** and ADR-0044 chose
  it on the reason this entry did not have: the scene blends **additively**, so there is no draw
  order to get right and the sort a 3D particle system pays buys occlusion that does not exist here.
  One `z` per particle drives sprite scale, an atmospheric fade, parallax against `zoom`/`pan_*`, and
  — the term that separates volume from a sprite sheet — *which current the particle rides*. **Boids
  stays rejected** (ADR-0044 Alternative B, on the O(n²) and per-frame-allocation grounds this entry
  anticipated); it needs its own ADR and a measured budget if ever wanted. The known limit is honest
  and recorded: no occlusion, so the illusion flattens as density rises.

**Two thirds of this was a preset defect and is fixed; the remaining third is real.** Recording the
fixed part too, because the mechanism was badly non-obvious and the next author will need it.

**What was authorable and was wrong.** `spin` is not vorticity — it is the flow field's **rate of
change** (`let field_t = self.time * self.spin`, then the field is sampled at `field_t`). The whole
family shipped `spin` between 0.34 and 2.2, which rewrites the streamlines faster than a particle can
cross them: every particle is steered somewhere new each frame by a field that has already moved on,
so ten thousand of them average into uniform shimmer. **That is what "lame" was.** Held near 0.1 the
field stands still long enough for particles to fall onto its streamlines and travel together, which
reads as flocking without any per-particle rule. The second half is that `force` cannot go with it:
at ~1.9 the steering overrides each particle's retained momentum (`DAMPING = 0.86`) and the entire
swarm collapses onto the field's attracting curve within seconds, leaving one bright ribbon in an
empty frame. Bracketed at 0.85 / 1.15 / 1.45, the flock lives near **1.2**. Small sprites and a long
`trails` then draw each bird as a directional dash. This is now documented in `swarm_drift.toml` and
cross-referenced from the other four.

**What is not authorable, and is the actual gap:**

- **No depth axis.** `Particle.pos` is `[f32; 2]` and the world is a 2D torus (`BOUND_X`/`BOUND_Y`).
  There is no z, no parallax, no perspective. The only depth cue in the scene is incidental:
  `bright = (0.25 + speed * 0.7) * p.bright`, so fast particles read as nearer. "3d-like space" is
  not reachable by any combination of existing params.
- **No flocking rules.** Motion is pure advection through a scalar-potential curl field. There is no
  cohesion, separation or alignment term — the apparent flocking above is entirely an artifact of
  neighbouring particles sharing a streamline, which is why it is fragile and needs the narrow
  `force` window to survive at all.
- **`FIELD_FREQ` is a `const 2.3`, not a param.** This is the cheapest and most interesting of the
  three: the field's spatial frequency sets how tight the vortices are, and therefore how many
  distinct streams fit in frame. One bindable value would let an author choose between a few broad
  currents and many tight swirls — which is most of the visual difference between "drifting cloud"
  and "murmuration" — at the cost of one `set_param` arm. **Recommend taking this one alone first**
  and seeing how far it gets before designing anything larger.

**ADR-worthy only if the depth/boids half is pursued** — that is a new simulation model for the
scene, with a genuine rejected alternative (a 2.5D fake: a per-particle z used only for sprite scale
and parallax offset, no z-sorting and no 3D field, which is far cheaper and might be
indistinguishable at 10 000 additive sprites). The `FIELD_FREQ` param on its own is not ADR-worthy —
it is [0019](#0019--glow-is-unreachable-from-a-preset-on-all-four-line-scenes)'s shape exactly: a
constant that should be a param, already plumbed, defaulting to the value it replaces.

---

## ~~0026 — `lsystem` has no per-segment colour, and the asymmetry with `spectrum` looks unintentional~~

- **PROMOTED 2026-08-01 → [ADR-0059](adrs/0059-line-scenes-colour-along-their-generator-axis.md) +
  [Plan 0054](plans/done/0054-the-line-scenes-catch-up.md)**, at wider scope than filed: reading the
  code showed `spectrum` is the *only* line scene reaching `[palette]` at all, so all four
  generators get the colour surface, each on its own axis (`lsystem` colours by **generation
  depth**). Notes below retained as the origin record.

- **Raised:** 2026-07-28, from `preset-author`. The user, on Arrowhead: *"we should introduce ether
  more lines, or glow or some more colors"*. The first two were authorable; the third was not.
- **Verified against code:** `lsystem`'s `PARAMS` has no `hue_spread`; `[palette]` is inert on the
  three generator-driven line scenes, which colour through their own cosine ramp
  ([0014](#0014--the-line-scenes-cosine-hue-ramp-is-not-a-hue-wheel-and-nothing-documents-it)). So
  `hue` is whole-figure and is the *only* colour control.

There is no way from a preset to give one branch of an L-system a different colour from another —
not by depth, not by position along the curve, not by anything. The only available answer to "more
colours" was to make the whole figure travel further and faster through the ramp, which is a
different thing and the user will notice it is a different thing.

**The asymmetry is the argument.** `spectrum` — added later, by Plan 0034 — *does* have `hue_spread`,
walking the palette across its elements, and on `radial_ring` that single param is most of what makes
`spectrum_corona` read as a designed object rather than a readout. The same lever on an L-system
(hue by recursion depth, or by distance along the turtle path) would do the same work for the whole
generator family. Nothing suggests the omission was decided; `spectrum` simply had a reason to need it
first.

**Impact:** moderate and permanent — it caps how rich any `lsystem` or `star_pattern` preset can look,
and those are 3 of the shipped set. Not urgent.

**Probably ADR-worthy, for one reason only:** *what* the spread should be indexed by is a real choice
with real alternatives, and unlike `spectrum` there is no obvious answer. `spectrum` has a natural
axis (frequency, `index` over elements). An L-system has at least three candidates — recursion depth
(structural, reads as "older growth is a different colour"), segment ordinal along the turtle path
(reads as a gradient sweeping through the figure as it draws), or world position — and they look
completely different. Picking one is the decision; the plumbing is trivial either way. Note the
`SegmentInstance` budget constraint from
[ADR-0041](adrs/0041-line-joins-are-per-endpoint-on-the-segment-instance.md) (8 floats, fixed
capacity, no-alloc) applies if this needs a per-segment value uploaded rather than derived in-shader.

---

## 0027 — two engine behaviours that are correct, non-obvious, and undocumented

- **Raised:** 2026-07-28, from `preset-author`. Neither is a defect. Both cost the lane multiple
  render round-trips in one session, and both are the kind of thing that will cost the *next* author
  exactly as much, because in each case the intuitive mental model is wrong in a way the render does
  not explain.

**1. `color_center` is CYCLIC.** The lane, wanting a dark render of a reaction-diffusion field,
reasoned that pushing `color_center` negative would slide the field's values toward the palette's
dark end. It does the opposite: the coordinate wraps, so a negative centre lands the bulk of the
field in the palette's *bright* stops and the picture gets brighter. Three rendered iterations were
spent tuning exposure, contour density and the palette ramp — all downstream of a cause that was
none of them — before the wrap was identified. `presets/README.md` and `docs/preset-palettes.md`
should say the coordinate is cyclic and that a negative centre is a wrap, not a clamp.

**2. The ink pass INTERPOLATES, so inverting its poles does not make a dark duotone.** The shader
(`core/src/render/ink.rs`) is `remapped = mix(paper, ink, d)` where `d` is the source's Rec.709
luminance. The intuitive reading of the two poles is that they are a *mapping* — dark input becomes
paper, bright input becomes ink — and therefore that swapping them (dark paper, bright ink) turns a
print into a glow. It does not, because a source sitting at **mid** luminance lands halfway between
the poles no matter how far apart they are set. Measured: `paper_bright = 0.055` against
`ink_bright = 0.94` on a developed Gray-Scott field rendered **flat slate grey**. The inversion only
works where most pixels are already near 0 or near 1 — a line scene against black, not a continuous
field. Worth a sentence beside the `ink_*` table in `presets/README.md`, because "make it dark" is an
obvious thing to want and this is the obvious way to try it.

**Impact:** pure documentation, cheap, and it compounds the way
[0014](#0014--the-line-scenes-cosine-hue-ramp-is-not-a-hue-wheel-and-nothing-documents-it) did —
every author who reaches for these pays the same round-trips. **Not ADR-worthy.** Bundle with the
next doc sweep, or with whichever plan next touches the palette or ink surface.

**~~CLOSED 2026-07-29~~ by [Plan 0041](plans/done/0041-report-two-level-stimuli-and-expression-reachability.md)
Phase 4**, folded into that plan's doc sweep exactly as this entry suggested. Both behaviours are now
written down where an author meets them:

- **`color_center` is cyclic** — `presets/README.md` says a negative centre wraps into the palette's
  *bright* stops rather than clamping toward the dark ones, and that `-0.1` and `0.9` are the same
  place. `docs/preset-palettes.md` — which this entry also named, and which the plan's Phase 4 file
  list omitted — carries the same note beside the `color_center` row and beside `hue_center`, which
  is cyclic for the same reason. Added at the Mode 4 close.
- **The ink pass interpolates** — `presets/README.md` says `mix(paper, ink, luminance)` is an
  interpolation, not a mapping, so a source at mid luminance lands halfway between the poles however
  far apart they are set. The measured `paper_bright = 0.055` / `ink_bright = 0.94` flat-slate-grey
  Gray-Scott result is quoted, along with where the inversion *does* work: a line scene against
  black, where most pixels already sit near 0 or 1.

---

## Entry 0028 — from the 2026-07-29 `preset-author` post-Plan-0041 library sweep

---

## ~~0028 — reachability only reports `select`/`clamp` nodes, so a bare comparison is invisible and a dead band gate can hide behind a live `tempo` one~~

- **Raised:** 2026-07-29, from `preset-author` (first library audit using Plan 0041's new
  reachability check).
- **Verified against code:** yes — `collect_flags` in `core/src/preset/expr.rs`.
- **PROMOTED 2026-07-29 → [ADR-0043](adrs/0043-reachability-reports-comparison-nodes.md) +
  [Plan 0042](plans/done/0042-reachability-sees-every-comparison.md); closed 2026-07-30.** Both
  shapes are now reported, as a `COMP` line. The re-audit the fix enabled found **0 genuinely dead
  gates** across the shipped set — so the second five this entry describes were the last of them,
  and the seven bare comparisons the old check could not see score clean. Notes retained below as
  the origin record; the mechanism section describes the *pre-fix* code.
- **Not a re-raise of [0022](#0022----reports-reactivity-columns-are-structurally-blind-to-a-level-curve).**
  0022 was about the *reactivity columns* being blind to a level `curve`, and Plan 0041 closed it.
  This is about the *reachability check itself*, which 0041 added.

Plan 0041 works: it found four presets whose headline mechanism had never run, all four are now
fixed, and every one of them would have stayed invisible without it. This entry is about the
**second five**, which the checker did not find and structurally cannot.

### The mechanism

`Node::probe` walks and records **every** node in the tree. But `collect_flags` only *emits* a
`GateFlag` for two node shapes:

```rust
(Node::Call(Func::Select, args), NodeObservation::Select { saw_true, saw_false })
    if saw_true != saw_false => …
(Node::Call(Func::Clamp, _), NodeObservation::Clamp { peak_fraction_of_bound })
    if peak_fraction_of_bound < 1.0 => …
```

`NodeObservation` has no variant for a comparison, so a `Node::Bin(Cmp, …)` that is not a
`select()` condition records `Untouched` and is never reported. Two consequences, both of which
shipped in the library for months and both found by hand:

**1. A bare comparison as the whole binding.** The idiomatic way to write a boolean param is
`reseed = "onset > 0.55"` — no `select` anywhere. Combined with `onset` being **raw spectral flux**
(peak `0.016`, not a `0..1` envelope), every attractor in the set had never reseeded once, and
`rose_web.mirror_reflect` had never reflected:

```toml
reseed         = "onset > 0.55"   # attractor_clifford — 34x unreachable
mirror_reflect = "onset > 0.18"   # rose_web           — 11x unreachable
```

All five scored a clean `gates 0`.

**2. A dead band gate behind a live `tempo` one — the worse case, because it reports as clean.**

```toml
kaleido_order = "select(min(tempo > 124, bass + treb > 0.38), 4, 1)"   # attractor_lorenz
```

The flag names the **whole `min(...)`** as the condition, and the report's own guidance says a
`tempo` gate is correctly one-sided under a single-BPM probe — so the reader dismisses it. The
`bass + treb > 0.38` half is separately dead (the sum peaks near `0.138`) and is never named. The
excusable half launders the inexcusable one. `swarm_storm` has the same shape with a *reachable*
band half, so the two are indistinguishable in today's output.

### Impact

This is the instrument all three lanes verify through, and the failure mode is a **false clean
reading**, which is worse than no reading. It cost this lane a full manual sweep
(`grep -rnoE '(bass|mid|treb|onset)[^"]*?[><]=? *[0-9.]+' presets/*.toml`, then every threshold
checked by hand against `LOW_LEVELS`) *after* `--report` had said the library was healthy.

Nine dead gates total were fixed in `e9a1c3c`; **five of the nine** were invisible to the check.

### What I am not deciding

Whether the fix is a `NodeObservation::Compare` variant reported like `Select`, or reporting the
innermost comparisons of a composite condition separately rather than the whole condition text, or
both. That is architect's call. The recording walk already visits every node, so the missing piece
looks like reporting rather than instrumentation — but that is an observation, not a design.

Worth deciding alongside it: ADR-0042 shipped this **advisory**, to be gated once the library is
clean. The library is clean *as measured today*; gating on a check with this blind spot would
freeze the false-clean reading into CI.

---

## ~~0029 — the swarm's toroidal wrap seam sits exactly on the frame edge, and feedback burns it into a bright bar~~

- **Raised:** 2026-07-29, from `preset-author` — reported by the user as a visible artifact in the
  running app, then reproduced headless.
- **Verified against code:** yes — `core/src/render/scenes/swarm.rs`.
- **PROMOTED 2026-07-29 → [ADR-0044](adrs/0044-swarm-world-is-a-25d-torus-sized-from-the-target.md) +
  [Plan 0043](plans/done/0043-swarm-depth-and-domain.md); closed 2026-07-30.** Fixed at the cause,
  which is the option this entry's "Not deciding" section listed first: the half-extents now follow
  the **render target's** aspect times a `MARGIN` of 1.25, so the seam projects outside the visible
  frame across the family's whole working `zoom`/`pan` range and the feedback stage has no fixed line
  to integrate. The 400-frame `dynamic:110` capture in the Reproduce block above is clean on all
  three surviving presets. Alpha-fading near the seam was rejected explicitly (ADR-0044 Alternative
  C — it keeps the 16:9 constant and trades a bright artifact for a dim one), as was respawning
  (Alternative D). This entry's closing note was taken: the replacement takes the target's aspect per
  [ADR-0037](adrs/0037-internal-grid-is-a-resolution-not-a-shape.md), and it is the first time that
  rule has been applied to a **simulation domain** rather than a render grid. Two consequences are
  priced in rather than hidden: `zoom` is usable down to ~0.84 (the wall moved, it did not vanish),
  and the margin puts about a quarter of the particles off-frame, which is why the surviving presets'
  `size`/`brightness` went up.
- **This is a bug report, not a capability request.** Unlike most entries here, nothing about it is
  a matter of taste, and **no preset lever fixes it.**

### Symptom

A hard, bright, near-horizontal bar along the top and bottom of the frame on every `swarm` preset,
growing brighter the longer the preset runs. Reproduce:

```sh
cargo run -p standalone --example shot -- --preset-file presets/swarm_drift.toml \
  --signal dynamic:110 --frames 400 --size 960x540 --out drift.png
```

By the last frames of that strip the bands are the brightest thing on screen, and the interior has
visibly *drained* — the picture is dimmer and flatter than it started. Present on **every** swarm
preset, and present before the 2026-07-29 retune (`714856a`), so it is not a content regression;
that commit's longer exposures and coarser sprites only made it more legible.

### Mechanism

```rust
const BOUND_X: f32 = 1.8;
const BOUND_Y: f32 = 1.0;
…
// Toroidal wrap keeps the field populated (no respawns/hitches).
if p.pos[1] > BOUND_Y { p.pos[1] -= 2.0 * BOUND_Y; }
else if p.pos[1] < -BOUND_Y { p.pos[1] += 2.0 * BOUND_Y; }
```

`BOUND_Y = 1.0` is the NDC frame edge. So the wrap seam is not somewhere off in the simulation's
margin — **it coincides with the top and bottom of the visible frame.** Every particle that leaves
the field teleports across at exactly that line, so the line is the one place on screen every
wrapping particle is guaranteed to paint. The feedback stage then integrates it: with `trails` in
the 0.7–0.9 range the whole family uses, a per-frame deposit at a fixed y accumulates into a
saturated bar over a few hundred frames.

The wrap comment is right about what it buys (no respawn hitches) — the defect is the seam's
*placement*, not the toroid.

### Why no preset can work around it

- **`trails` can only trade it against the look.** Shortening the exposure dims the bar and the
  figure equally; the bar wins because it is re-deposited every frame while the figure moves.
- **`zoom` cannot hide it.** Below `1.0` the camera pulls back far enough to expose the domain
  boundary as a hard rectangle inset from the frame (bracketed at 0.78 — unusable), so the family
  is pinned at or above 1.0, which is exactly where the seam is. There is no value that puts the
  seam off-screen without exposing the other edge.
- `BOUND_X`/`BOUND_Y` are private constants with no param binding.

### Impact

Five shipped presets, plus any future `swarm` content. It is the first thing the eye goes to on a
dark background, and it is what a user watching the app actually notices — this entry exists
because one did. It also interacts badly with ADR-0024's dissolves, since the add/burn kind sums
two frames and both may carry a bar.

### Not deciding

Whether the fix is pushing the bounds outside the view, fading particle alpha approaching the seam,
or resetting a particle's feedback contribution on wrap. That is architect's call. Worth noting
only that `BOUND_X = 1.8` vs `BOUND_Y = 1.0` already encodes a 16:9 assumption, so whatever
replaces it should take the render target's aspect rather than a constant
([ADR-0037](adrs/0037-internal-grid-is-a-resolution-not-a-shape.md)).

---

## 0030 — the library binds audio to luminance and colour far more than to the numbers that rebuild geometry

- **Raised:** 2026-07-29, from `preset-author`, prompted by user feedback on the shipped set:
  *"too safe, there is just nothing curious... I should see wonders and other worlds"*.
- **Verified against code:** n/a — this is an authoring principle with measured evidence, not a
  claim about the engine.
- **Not an engine gap.** Filed here because it is durable, measurable, and the thing a future
  content pass should be pointed at. It needs no ADR and no plan.

### The observation

Most of the shipped library binds its band terms to `brightness`, `glow`, `flash`, `thickness`,
`hue` and `palette_mix` — parameters that change how a figure is *lit or coloured*. Comparatively
few bind to the parameters that change what the figure *is*.

Every system has at least one of the latter, and they were largely unused:

| System | The numbers that rebuild the geometry |
|--------|----------------------------------------|
| `parametric_curve` | `n`, `d`, `radial_offset`, `phase` |
| `attractor` | `a`, `b`, `c`, `d` (the family's coefficients) |
| `reaction_diffusion` | `feed`, `kill` (which regime the chemistry is in) |
| `fragment_field` | `warp`, and `kaleido_order` as a *stepping* fold |
| `lsystem` | `visible_depth` |
| `star_pattern` | `variant`, `mirror_order` |

### The evidence

Four presets authored on 2026-07-29 (`a51c431`) bind audio to the geometry column instead, and
measure well outside the set they joined:

| preset | system | driven by audio | `anim` | reactivity at realistic levels |
|--------|--------|-----------------|--------|-------------------------------|
| Supernova | `fragment_field` | `warp` + a 3/8/16-fold stepping fold | **0.234** | 0.178 / 0.085 / 0.068 / 0.155 |
| Reliquary | `reaction_diffusion` | `feed`/`kill` inside the filament regime | 0.095 | 0.134 / 0.009 / 0.017 / 0.017 |
| Leviathan | `attractor` | all four de Jong coefficients | 0.087 | 0.144 / 0.085 / 0.093 / 0.036 |
| Cathedral | `parametric_curve` | the Maurer rose's `n` and `d` | 0.056 | 0.163 / 0.058 / 0.016 / 0.026 |

Supernova is the most animated preset in the library; the prior best outside the rose family was
`fragment_warp` at `0.183`. All four register on every band at realistic levels, where much of the
shipped set reads `0.000` on treble.

### Why it works, mechanically

> **Premise corrected 2026-07-31 (Plan 0045):** the frame no longer clips per channel — it rolls
> off through an engine tonemap. The paragraph stands per this file's append-only rule and its
> conclusion is unchanged (if anything stronger); the full correction is at the end of **0038**.

It is the same principle as the additive ceiling seen from the other side. Luminance is **bounded** —
the frame clips per channel, so past a point more energy on `brightness` produces less picture, not
more. Geometry is **not** bounded that way: a fold order stepping 3 -> 8 -> 16, or a rose's `n`
moving through fractional values, changes the image without ever running out of headroom. Peak
energy spent on structure has somewhere to go.

Two practical notes from authoring the four:

- **Ease the shape numbers.** A geometry parameter moving on a raw band term twitches; the four
  above use `[smoothing]` constants of 0.5-0.9 s on the shape numbers so a change reads as a morph.
  Fractional values are valid for `n`/`d`/`feed`/`kill`, so the morph is genuinely continuous.
- **Cutting luminance is usually the precondition.** `Cathedral` first rendered as a solid white
  disc, and only became legible after `thickness` 1.45 -> 0.42, `brightness` 0.92 -> 0.34 and
  `glow` 1.05 -> 0.55. Symmetry multiplies luminance: a six-fold mirror under an eight-fold screen
  fold stacks the same stroke dozens of times.

### Where this should end up

`references/craft.md` in the `preset-author` skill is the working home for the rule — it already
leads with the additive ceiling, and this belongs beside it as the second structural principle.
This entry is the record and the evidence; that file is where it changes behaviour.

---

## ~~0031 — the Rich tier's 3x particle count makes the attractor reseed transient opaque, and `clifford` blows out~~

- **CLOSED 2026-08-03 at [Plan 0057](plans/done/0057-the-attractors-compute-path.md)'s close, on
  measurement rather than on argument** — both halves, and the harness gap that kept it open. Half
  one (too bright) is fixed at its cause: [ADR-0065](adrs/0065-the-attractor-deposit-is-normalized-by-particle-count.md)
  divides the deposit by the particle count, and Clifford's mean display luminance at `Rich` went
  `17.37 -> 10.86` against `Floor`'s `10.34`, with Phase 6 re-verifying the invariance at the
  *raised* exposure the content pass restored. Half two (the hard-edged speckled slabs) is fixed by
  [ADR-0066](adrs/0066-a-reseed-disturbs-the-cloud-rather-than-replacing-it.md): the slabs were the
  seed box, re-filled on every reseed. This entry could not reproduce them from four captures, and
  the reason is now known — a reseed fires on 7 hops out of 375 under `click:120` and an
  evenly-spaced `--strip 8` lands on one by luck. `shot --at` is the instrument that aims at it, and
  `attractor_ink --tier rich --at 44,46,48,54` rendered the rectangle before the fix and does not
  after.
- **This entry's third bullet below was false when written, and the correction is the reusable
  part.** It says `shot` has "**no `--tier` flag**" and that settling either half "needs the running
  app". ~~True~~ — **false since [Plan 0044](plans/done/0044-quality-tiers.md) Phase 3**, which built
  `Renderer::new_headless_tiered` and the flag; it was missing only from `shot --help`, and four
  documents (this entry, [0047](design-backlog.md), ADR-0064 and the `preset-author` skill) reasoned
  from its absence. A flag's help text is part of the flag.
- **What it does *not* close: Plan 0044 Phase 4.** The `Rich` multipliers are still provisional, and
  the calibration in [`on-device-validation.md`](on-device-validation.md) is still owed — it is a
  frame-time measurement, and after ADR-0065 lowering `attractor_particles` no longer changes that
  family's exposure. Notes below retained as the origin record.

- **MECHANISM HALF TAKEN 2026-08-03 → [Plan 0057](plans/done/0057-the-attractors-compute-path.md)** —
  entry still **live** pending measurement, but both halves of its mechanism now have a fix
  designed. The 3x is removed at its cause by
  [ADR-0065](adrs/0065-the-attractor-deposit-is-normalized-by-particle-count.md) (Phase 2), which is
  the lever this entry's own re-check at Plan 0045's close identified — "the lever is the particle
  count, not the curve" — and the opaque *rectangle* is removed by
  [ADR-0066](adrs/0066-a-reseed-disturbs-the-cloud-rather-than-replacing-it.md) (Phase 3), whose
  entry [0050](design-backlog.md) supplied the reason it is a rectangle at all. Re-check this entry
  after Plan 0057 Phase 6, on a `--tier rich` capture rather than on argument — the flag that makes
  that possible is Phase 1.

- **Raised:** 2026-07-30, from the **user**, running the standalone at the `Rich` tier the day
  Plan 0044 landed: *"clifford is too bright and has artifacts"*, with a screenshot.
- **Verified against code:** partly — see the split below. One half is reproduced by capture, the
  other is a hypothesis with strong supporting evidence and no repro.

The screenshot shows `attractor_clifford` as a near-white/yellow saturated disc, overlaid with
**hard-edged, uniformly-speckled quadrilateral slabs** at odd angles, extending outside the disc.

### Half one — too bright. Reproduced, and expected.

Confirmed by capture: `shot --tier floor` vs `--tier rich` on this preset at 1280x800 and at
2048x1152 both show `Rich` visibly hotter, with the ribbon cores clipping to white where `Floor`
still holds tone. Nothing is wrong with the tier mechanism — this is `attractor_particles`
50 000 -> 150 000 depositing **3x the energy per texel** into an 8-bit additive composite with no
tonemap. That is roadmap [Wrong turn 3](roadmap-visual-richness.md) (the additive ceiling) meeting
R0's raised capacity, and it is the first field evidence that **the provisional 3x multiplier is
too high** — the number Plan 0044 Phase 4 was supposed to measure and did not.

Two independent fixes are already queued, and this entry is not asking for a third:

- **R1 / [Plan 0045](plans/done/0045-linear-light-and-bloom.md)** — the `Rgba16Float` linear composite
  with a real tonemap is the *structural* answer: with headroom, 3x the deposit stops clipping and
  starts reading as brightness. This preset is a good acceptance case for that plan.
- **Plan 0044 Phase 4** (carried to `on-device-validation.md`) — the *calibration* answer: measure,
  and bring `attractor_particles` down to the value that holds. Do this **after** R1, or the
  measurement is taken against a ceiling that is about to move.

### Half two — the slabs. Not reproduced; best-supported hypothesis is the seed box.

**Four captures did not reproduce it**: both tiers x {1280x800 static, 2048x1152 static}, plus an
onset-driven `--signal click:120` filmstrip at both tiers. So it needs something the headless
harness does not do — live loudness, a resize/fullscreen reallocation, a dissolve, or a transient
too short for a filmstrip's 8 sampled frames to land on.

The hypothesis, from the preset and the scene code rather than from a repro:

1. `attractor_clifford.toml` binds `reseed = "onset > 0.012"`, and its own comment records that
   `onset` peaks around 0.016 on music-like material — so **the reseed genuinely fires** on real
   audio (it is the most reluctant setting in the family, not an unreachable one).
2. A reseed re-uploads every particle to `AttractorScene::seed`, which scatters them **uniformly
   through `family.seed_box()` — a rectangle**. For a few frames after each reseed, the particle
   set *is* a uniform-density box, before the map contracts it onto the attractor.
3. This preset binds `trails = "0.62 + clamp((bass + mid) * 0.35, 0, 0.28)"` — a long exposure,
   rising with energy. The engine-wide feedback stage **holds that box in the accumulation** long
   after the particles have moved on.
4. At `Floor` that transient is 50 000 particles spread over a large rectangle — a faint wash.
   At `Rich` it is **150 000 in the same box**, three times the density, and the wash becomes an
   opaque slab. The view transform (`zoom`, `pan_x`, `pan_y`) rotates and offsets it, which matches
   the odd angles and the extent past the disc edge.

If that is right, the tier did not *create* the artifact — it made a pre-existing transient
visible, which is a fair description of a whole class of thing `Rich` will surface.

### What would settle it

- **A `--tier floor` run of the same preset on the same audio.** If the slabs vanish or go faint,
  it is density-amplified (hypothesis holds). If they persist identically, the seed box is not the
  mechanism and this needs a fresh look. *This is one command and it is the cheapest discriminator.*
- Failing that: a capture path that can hold a reseed transient — a filmstrip whose frame indices
  bracket a known onset rather than sampling evenly.

### Where it goes

`dev` investigation, not a preset fix — an author cannot see the seed box from the grammar, and
turning `reseed` off to hide it would cost the preset its intended behaviour. Route after R1
lands, since the linear composite changes what "too bright" means and may change the verdict on
the slabs' visibility too.

### Re-checked 2026-07-31 at Plan 0045's close. **Still open, and half one is not as closed as R1 promised.**

R1 has landed, so the routing note above is satisfied and this entry is now takeable. What the
re-check found, stated as what was and was not verified:

- **The tonemap is doing its job at `Floor`.** A capture of this preset at 1280x800 renders a
  saturated orange disc with its internal ribbon structure intact and **no white clipping** — the
  roll-off is holding tone where the 8-bit additive composite used to flatten it. That is the
  structural fix working.
- **But "with headroom, 3x the deposit stops clipping" is not established, and Plan 0045 measured a
  specific reason to doubt it.** Boundedness below 1.0 does not stop the sRGB byte rounding to
  **255**, which takes a linear value of about **35** at the shipped `KNEE = 0.6` — and Phase 4b
  measured that `attractor.toml` *already reaches that value at `Floor`* on the hardware adapter.
  `Rich` triples the deposit into the same texels. So the disc's core may still read as flat white
  at `Rich`, not because anything clips but because the display byte saturates. The tonemap moved
  the ceiling; it did not remove one.
- **This could not be settled from this session, and the reason is a gap worth naming.** The
  discriminator both this entry and the check above want is a `Rich`-tier capture, and `shot` has
  **no `--tier` flag** — headless capture is `Floor` by construction (ADR-0045), which is a
  deliberate property that keeps baselines reproducible. The `--tier floor` / `--tier rich`
  commands this entry calls "one command and the cheapest discriminator" **do not exist**; that
  half of the entry was written against a capability the harness does not have. Settling either
  half therefore needs the running app at `Rich`, which [Plan 0050](plans/done/0050-in-app-settings-and-a-browse-overlay-that-fits.md)'s
  `[` / `]` tier swap makes an A/B in one sitting.
- **Half two (the slabs) is untouched by Plan 0045.** It is a reseed-transient density problem in
  the seed box, and nothing in the linear-light work addresses it. A tonemapped slab is less
  opaque than a clipped one, so it may read as fainter, but the mechanism is unchanged.

**Where it goes now:** the calibration answer (Plan 0044 Phase 4 — bring `attractor_particles`
down to the value that holds) is **unblocked**, since the ceiling it was told to wait for has
moved and settled. Run that before treating half one as a defect, and use the same run to
discriminate half two.

---

## ~~0034 — nothing in the engine spawns, throws, ages or individuates an object~~

- **PROMOTED 2026-08-01 → [ADR-0057](adrs/0057-emitter-scene-analytic-ballistics-seeded-individuation.md) +
  [Plan 0052](plans/done/0052-the-emitter-objects-that-spawn-fall-and-die.md)** — a new
  `SystemKind::Emitter` with analytic ballistics and seeded per-object individuation. The user
  chose the new-scene shape over extending `swarm` or building a per-object expression facility.
  **[0033](design-backlog.md#0033) stays open** — this is the motion half; marks are still round additive blobs.
  Notes below retained as the origin record.

- **Raised:** 2026-07-30, from `preset-author`, by the Solitaire-cascade request.
- **Verified against code:** yes — `core/src/render/scenes/swarm.rs` (`Particle`, `bounds`),
  `core/src/render/scenes/particles/mod.rs` (`reseed`), `core/src/render/trails.rs`,
  `core/src/preset/expr.rs` (`INDEX_SLOT`).

Independent of what a mark *looks* like (0033), the engine has no model for an object with a life.
Four missing pieces, each verified:

- **No emitter and no lifetime.** `Particle` is `{ pos, vel, z }`. Nothing spawns, nothing dies.
  The swarm's world is a **torus** — `bounds(aspect)` wraps every particle back into the frame — so a
  particle categorically cannot fall out of shot, which is the entire motion of a cascade. The
  attractor's `reseed` is the nearest thing to an event, and it re-scatters the *whole* cloud at once.
- **No gravity and no ballistic integration.** Steering is a flow field plus a radial `burst`; there
  is no constant acceleration vector and no way to express one. A parabola is not approximable — a
  flow field can bend a path, but every particle in a region bends the same way, which is a current,
  not a throw.
- **No per-object state or per-object expressions.** A binding is evaluated **once per frame**; the
  only per-element evaluation in the engine is `index`, and `INDEX_SLOT` is fed only by `spectrum`.
  So `hash()` cannot give each object its own launch angle, spin, size or twinkle phase. This is what
  makes the starfield blink *as one sheet* rather than as individual stars — the user asked for
  "мигают" and got a field-wide flash, which is a different thing.
- **No stamped trail.** What makes the Solitaire cascade read is that each card leaves **hard,
  non-fading copies of itself** along its arc. `trails` is a fade-and-accumulate feedback over the
  whole finished frame (max-decay), i.e. a smear. It cannot stamp, and it decays.

**Relationship to 0033.** These are separable: an emitter that throws round blobs on parabolas is
buildable without touching the additive model, and would already read as a shower. Worth noting for
sequencing — 0034 alone is a smaller, safer piece of work than 0033, and it is the half that carries
the *motion* the user described ("падает красиво по параболе в разных направлениях"). 0033 alone
gives shaped marks that still cannot fall.

**Adjacent, already filed:** the "no stateful expressions / beat-latched state" gap in the
`preset-author` skill's own list is the grammar-side cousin of the per-object point here. If an
emitter is built, it likely subsumes the motivating case.

**Not deciding:** whether this is a new `SystemKind` (an emitter scene alongside `swarm`), an
extension of `swarm`, or a general per-object expression facility. All three have real costs and the
choice is architect's.

---

## ~~0035 — `presets/README.md` lists 10 expression variables; the code has 19~~

- **FIXED 2026-08-03, at Plan 0048's close** — the doc sweep this entry asked to ride on. The
  roster line now carries all 19 in `VAR_NAMES` order, with the `*_raw` escapes pointed at the
  scale section below it and the five musical-time variables split into ADR-0050's two layers,
  including the Phase 6 measurement that makes `beat_index` the one to build an arc on. Notes
  retained below as the origin record.

- **Raised:** 2026-07-30, from `preset-author`, while checking `tempo` for the BPM binding.
- **Verified against code:** yes — `VAR_NAMES` in `core/src/preset/expr.rs` has 19 entries.

`presets/README.md` names `bass mid treb onset beat bar time tempo novelty index`. Missing from that
line: the four `*_raw` escapes and — the ones that matter here — ADR-0050's beat clock
(`beat_index`, `time_since_beat`, `beat_in_bar`, `bar_index`, `bar_phase`). `docs/presets.md`
documents them correctly, so this is a one-line drift in the *roster* document, which is the one this
lane is pointed at first.

It cost a code read to establish that `beat_index` existed, and it is the difference between "a new
value every beat is impossible" and "`hash(beat_index)`". Small, but it sits on the most-read line of
the most-read authoring document. Flagging for the next close-ceremony doc sweep rather than
proposing anything.

---

## 0036 — the kaleidoscope stops folding the backdrop; is a folded backdrop a look worth keeping?

- **Raised:** 2026-07-31, from `architect`, while writing ADR-0055 for Plan 0045 Phase 2b.
- **Verified against code:** yes — `post.rs` routes background + scene into the first active
  stage's input, and `background.rs` paints a palette times a vertical gradient times a radial
  `bg_vignette`.

This is a **content question, not an open architectural one.** ADR-0055 is decided: the backdrop
leaves the post chain and composites underneath it, which is what makes the fold's falloff land on
`bg_*` instead of on black. A consequence of that decision is that the backdrop **stops being
folded** — today the kaleidoscope replicates `bg_vignette`'s radial darkening into the wedge
pattern, and after Phase 2b it will not.

Nobody chose that behaviour; it fell out of the routing. But it has been shipping for as long as
the fold and the backdrop have coexisted, so some presets in the library may be leaning on it
without anyone having named it — a folded gradient does put tinted structure in the wedges that a
flat underlay will not.

**What is actually being asked:** after Phase 2b lands, does any preset that binds both `bg_bright`
and `kaleido_*` look *worse*? The fold-binding presets are the population to check, and
`swarm_dense` (which pins `kaleido_order = 1` to dodge the old defect and is due an un-pin anyway)
is a natural first look.

**If the answer is "we lost something",** the way back is **not** reverting the alpha model. It
would be a bindable choice about *where* the backdrop composites — under the chain (the new
default) or into its input (today's behaviour) — which is a small named param on an already-settled
structure. Nobody should build that until a real preset is worse off.

**Not deciding:** anything. ADR-0055 stands regardless of the answer here.

---

## ~~0037 — the fold covers a disc, and on a field scene that reads as worse than the defect it replaced~~

- **Raised:** 2026-07-31, from `architect`, at Plan 0045's Mode 4 review, from the user's own
  screenshots of the running app.
- **Verified against code:** yes — `kaleidoscope.rs` clamps the sample radius to `r_max` and fades
  over `FALLOFF_BAND = 0.35`; nothing outside `1.35 r_max` is painted by the stage at any setting.
- **PROMOTED 2026-08-02 → [ADR-0061](adrs/0061-kaleidoscope-edge-treatment-is-a-per-preset-choice.md)
  + [Plan 0055](plans/done/0055-the-fold-edge-becomes-a-choice.md)** — the supplement takes the shape this
  entry argued for and one step further: rather than picking a better single treatment, **what
  happens outside the disc becomes a per-preset choice** (`kaleido_edge`), because the entry's own
  evidence is that a field and a figure want different answers. Five candidates ship behind one
  stepped selector in **one** pipeline and a `human` phase A/Bs them live.
  **Two things this entry got right and one it under-stated.** Right: the `vignette` reconsideration
  it asked for is in the roster, and ADR-0047's pipeline-count declination does not survive contact
  — that objection was about *address modes*, and three of four candidates are pure radius maps, so
  they are a uniform branch rather than a second pipeline. Under-stated: the scope. The corner sits
  at `0.5*sqrt(aspect²+1)` = **2.04x `r_max`** at 16:9, so **55.8 % of the frame** is what one
  treatment was deciding — this is not corner debris, it is most of the picture.
  **A candidate neither this entry nor ADR-0047 named is now the most interesting one:** `mirror`,
  which *reflects* the radius instead of clamping it, so past the disc the frame is a mirrored
  continuation of its interior — no ray (the content is real) and no crop (the corners are filled),
  which is what a physical kaleidoscope does. Notes retained below as the origin record.
- **CLOSED 2026-08-04 — [Plan 0055](plans/done/0055-the-fold-edge-becomes-a-choice.md) shipped it, and
  the live A/B settled the roster.** `kaleido_edge` is a bindable stepped param selecting one of
  three treatments inside one pipeline: `falloff` (0, ADR-0047's fade), `tile` (1) and `squash` (2).
  **`tile` is the default**, so this entry's second rejection — the disc cropping a field scene — is
  answered for every fold-binding preset without one of them having to opt in.
  **The A/B falsified two of the five candidates, and one of them was this entry's own bet.**
  `vignette` — the treatment this entry specifically asked the supplement to reconsider, on
  ADR-0047's Outcome calling it "the cleanest of the four on a border-filling field" — **lost on
  both scenes and was deleted.** So did `mirror`, the candidate the paragraph above calls the most
  interesting one: reflecting the radius puts the *centre* of the figure back into the corners (at
  16:9 a corner sits at `m = 2.04`, and `abs(2.04 - 2*round(1.02)) = 0.04`, so it samples from 0.04
  `r_max`) — arithmetic this entry did not have when it was written. Judging in motion is what
  separated them; neither would have been rejected from stills, which is the same lesson the entry's
  own last paragraph teaches.
  **What this entry got right stands:** a figure and a field want different answers. The verdict is
  `tile` for `attractor_leviathan` and `squash` for `fragment_kaleido` — two scenes, two treatments,
  which is the entire content of the argument for a choice.
  **The library retune this creates is [0058](design-backlog.md#0058)**, and one lesson from it belongs here:
  adopting a fill treatment on Leviathan took two edits, because that preset's `zoom` had been
  pinned under the inscribed radius *precisely to dodge the rays this entry reported*. Removing the
  cause removed the reason for the workaround, and other fold-binding presets carry similar pins.

ADR-0047 shipped the falloff-disc, confirmed at Plan 0045 Phase 2 from sixteen rendered stills.
Seen in motion on real presets at Plan 0045's close, the user rejected two consequences:

1. **The residual rays** around a centred figure (`attractor_leviathan`). ADR-0047's own Outcome
   predicts these exactly — a plain clamp replicates the disc rim outward as a sunburst, the
   falloff fades it, and on dense content the remainder "read as leftovers rather than as design".
   The ADR bet they would read as a corona *on centred figures*. On this one they did not.
2. **The disc's crop on a fullscreen field scene** (`fragment_kaleido`). The frame used to be
   filled; it is now a disc with backdrop corners. ADR-0047's Negative names this ("on field
   scenes the wrap alternative would have tiled *something* there") and accepts it for figures.

**Neither has a preset-side answer, and that is the point.** The fold is a polar operation on a
rectangular source, so the corners cannot be painted by it at any `zoom`/`scale`/`kaleido_*`. A lit
`bg_*` makes the corners not-black, which is a palliative. So this is not content-lane feedback
however it arrived.

**What the supplement should reconsider:** ADR-0047 declined a per-preset treatment mode on the
grounds that two address modes double the stage's pipelines against the documented WARP
pipeline-count sensitivity. That declination was taken before a fourth treatment existed. Phase 1
rendered `vignette` — the fade moved *inside* the disc, so no ray is ever drawn — and the Outcome
records it as "the cleanest of the four on a border-filling field and the most costly on a figure".
A treatment that is best on fields and worst on figures, in an engine whose presets are both, is
the classic argument for a choice rather than a default. Weigh that against the pipeline-count
cost, which Plan 0045 has since made more concrete: bloom added four pipelines and hit the WARP
aliasing hazard twice while doing it.

**Interview and render before deciding.** This project picks visual directions from side-by-side
samples ([ADR-0047](adrs/0047-kaleidoscope-fold-domain-disc-with-falloff.md)'s own confirmation
protocol), and the lesson from Phase 2 is that the sample set has to be taken **in the
configuration the complaint came from** — in motion, on a field scene and a figure, at a lit
backdrop, through the tonemap. Sixteen stills at `bg_bright = 0` confirmed a decision that two
screenshots of the running app then reversed.

---

## ~~0039 — four bind-group layouts in `core/src` are shared by pipelines that go live in the same frame, and only the tonemap's uniqueness is asserted~~

- **PROMOTED 2026-08-01 → [ADR-0058](adrs/0058-bind-group-layout-collisions-carry-evidence.md) +
  [Plan 0053](plans/done/0053-the-suite-stops-blessing-what-warp-gets-wrong.md)** — the collision
  property gets asserted, with an allowlist carrying dated hardware-vs-WARP evidence per pair.
  **One correction to the entry below:** it cites the hazard as "ADR-0021 / Plan 0020's
  documented hazard", and so do five code comments — ADR-0021 is the **shared palette system**.
  The hazard had no record until ADR-0058. Notes below retained as the origin record.

- **Raised:** 2026-07-31, from `architect`, at Plan 0045's second Mode 4 review (Phase 4b).
- **Verified against code:** yes — the numbers below are the printout of a test that ships,
  `the_tonemap_layout_is_a_shape_no_other_layout_in_core_has` (`core/src/render/tonemap.rs`).
- **For:** `architect` then `dev`. Not content-lane work and not a shipping defect.

The DX12 **WARP software adapter** hands a pipeline whose bind-group layout matches another live
one *the other pass's resources*. That is ADR-0021 / Plan 0020's documented hazard, and Plan 0045
reproduced it twice more, to the byte: the tonemap was fed the kaleidoscope's uniform (`exposure`
became `kaleido_order = 6.0`), then the backdrop's (`bg_hue`, `bg_bright`), and the bloom blur
passes behaved as though handed the vertical pass's buffer. Every one was invisible on hardware —
and **the whole golden suite captures on WARP**, so a mis-render there is blessed rather than
caught.

Phase 4b replaced the tonemap's prose uniqueness claim with an enumeration over every
`create_bind_group_layout` call in `core/src` (23 layouts; `standalone/` and the plugin add none).
It asserts on the tonemap alone, and **prints three collision groups it asserts nothing about**:

| shape | held by |
|---|---|
| `[Uniform, Texture, Sampler]` | `ink-bind-layout`, `kaleido-bind-layout` |
| `[Texture, Sampler]` | `attractor-present-layout`, `trails-present-bind-layout` |
| `[Uniform]` | `background`, `disc`, `fragment-field-uniform`, `renderer.rs` (per-scene), `rd-init`, `swarm` |

The test's docstring calls these "older and deliberate". **Deliberate is a claim with no record
behind it** — which is exactly the failure mode Phase 4b existed to retire, since the comment it
replaced made the same kind of claim and was false (`attractor-decay` had held the tonemap's
shipped shape all along).

**One pair is live together on shipped content.** `attractor_clifford.toml` and
`attractor_leviathan.toml` bind `trails` on the attractor, which puts `attractor-present` and
`trails-present` — both `[Texture, Sampler]`, both plain blits — in the same command buffer. No
golden fixture covers that combination (`core/tests/fixtures/attractor.toml` binds no trails), so
the only thing that renders it on WARP is the preset behavioural suite, whose floors are coarse.
`ink` + `kaleido_*` is the same shape of risk with no shipped preset binding both **today** —
nothing stops the content lane writing one tomorrow, and the tonemap incident is proof that WARP
aliases precisely that layout.

**Nothing is observed to be wrong**, and hardware is unaffected: this is a test-fidelity hazard,
not a shipping one. It is also pre-existing — Plan 0045 surfaced it, it did not cause it.

**What would settle it.** Extending the assertion from one layout to "no two layouts that can be
live in one frame share a shape" needs per-pair evidence, which is the design question rather than
the edit: render each colliding pair's configuration on the hardware adapter *and* on WARP and
compare, then either move a layout or record the pair on an explicit allowlist carrying that
evidence. The `[Uniform]` group is the awkward one — six single-uniform groups is the natural
shape for a fullscreen pass and reshuffling them all is a worse cure than the disease, so the
allowlist half is probably the answer there and the assertion half for the rest. Cheap adjacent
win while in the file: `bloom.rs`'s module docs make the same prose claim for its four layouts
(bright, blur, up, mix) — the enumeration's printout shows it holds today, so asserting it costs
four lines.

---

## ~~0041 — the line seam's lit-backdrop guard discriminates on ~5 pixels, and a stronger property is available~~

- **PROMOTED 2026-08-01 → [Plan 0053](plans/done/0053-the-suite-stops-blessing-what-warp-gets-wrong.md)
  Phase 4** — the `glow = 0` fourth capture sketched below is taken as written, plus the same
  shape at `brightness = 0` for the swarm guard. No ADR: the property is a better test, not a
  decision with a rejected alternative. Notes below retained as the origin record.

- **Raised:** 2026-08-01, from `architect`, at Plan 0051's Mode 4 review.
- **Verified against code:** yes — the guard was run against a reverted shader at review:
  15 channels differ at worst `|L - B| = 0.4944`, against 52 651 untouched pixels on the swarm's
  equivalent.
- **For:** `dev`, on `architect`'s say-so. Test fidelity, not a shipping risk.

`a_lit_backdrop_survives_where_the_strokes_drew_nothing` asserts over pixels where the scene wrote
**exactly** zero. For the swarm that is a 2-D region — a radial falloff over a square quad leaves
~21 % of every sprite identically zero. For the line the falloff is one-dimensional and *quadratic*,
so the exactly-zero band is the outermost sub-pixel sliver of the quad and only a handful of sample
points land in it. The magnitude is unambiguous and the guard is genuinely non-vacuous, but the
margin is 5 pixels, and no choice of `samples`/`scale`/`thickness` widens it — it is geometry. The
test reports `borders_geometry` (2 407) so the real discriminating region is at least visible.

**Do not take "a lit backdrop never darkens the frame"** as the stronger property: it is false
post-fix at the stroke core, where `g = 1` and the backdrop is correctly extinguished, so it needs a
fixture-brightness precondition and is fragile.

**The clean version is a fourth capture at `glow = 0`** — the stroke draws and emits nothing, so the
frame is exactly `bg * (1 - a)`. Pre-fix the fully-extinguished set is the whole quad footprint
(thousands of pixels); post-fix it is the centreline only (tens). Assert a count ratio against the
footprint measured from the lit capture. That is a 2-D region with a three-orders-of-magnitude
margin, no coverage recovery, and no tolerance — and it needs neither a shader change nor a new
fixture, only one more call to the harness the test already has. The same shape would strengthen the
swarm guard at `brightness = 0`.

Deferred rather than built at Plan 0051's close because the two seams share one blend constant, so
the swarm guard already covers the mechanism; this buys resolution at the quiet seam.

---

## Entries 0043-0045 — the 2026-08-02 `preset-author` batch (sixth), from Plan 0048 Phase 7 (the library retune)

---

## ~~0043 — every reactivity instrument we have diffs against **silence**, so a binding saturated just above the noise floor reads as maximally reactive~~

- **PROMOTED 2026-08-03 → [ADR-0062](adrs/0062-clamp-occupancy-is-the-saturation-instrument.md) +
  [Plan 0056](plans/done/0056-clamp-occupancy-and-the-axis-anchor.md)** (occupancy recorded on the
  existing walk, printed in `--report`, and enforced as a HARD gate). The ADR takes the entry's
  second option and **rejects its first**: a mid-scale rung in the reactivity gate names the preset
  where occupancy names the binding, and costs another render pass per band. Notes retained below
  as the origin record.

- **Raised:** 2026-08-02, from `preset-author`, running [Plan 0048](plans/done/0048-analysis-v2-and-the-retune.md)
  Phase 7.
- **Verified against code:** yes — `core/tests/reactivity.rs`, and the two per-band blocks in
  `--report` (`standalone/examples/shot.rs`).
- **Cost when it bit:** it hid ~79 % of the retune's actual work list behind a green suite and a
  clean report, and 14 shipped presets were audio-static for the whole window between ADR-0049
  landing and Phase 7 running.

**What happened.** ADR-0049 multiplied the four headline levels by 16-96x. Every gain in the
library was written against the old raw magnitudes, so `clamp(mid * 16.0, 0, 0.30)` — which used to
deliver a fifth of its cap — now reaches that cap at `mid = 0.019` and holds it for anything above
a whisper. Measured across the shipped set, **263 of 332 clamped band terms were pinned at the
real-music median**, and **14 presets had no live audio term at all**: Rose Web, Rose Zoom, Rose
Trails, Rose Overflow, all five `reaction_diffusion`, all three `spectrum`, Cathedral, Leviathan.

**Every gate stayed green through all of it**, and so did `--report`:

| instrument | what it compares | what a pinned binding looks like to it |
|---|---|---|
| `reactivity.rs` (HARD gate) | band driven vs **silence** | fully reactive |
| `--report` per-band columns | band driven vs **silence** | fully reactive |
| `--report` realistic block | realistic vs full scale | *identical to the block above* |
| `anim` | frame-to-frame **in silence** | unchanged — nothing audio-driven runs |
| reachability | which way a **comparison** went | blind; a gain holds no comparison |

The realistic block is the one designed to catch level problems, and it did fire — quietly. Its
documented reading, "realistic close to full scale = a binding already saturating at low input"
([capturing.md](capturing.md)), was true for most of the library at once, which reads as an
unremarkable table rather than as an alarm.

**The blind spot is structural, not a tuning miss.** A silence-relative differential answers "does
this preset respond to sound at all". It cannot answer "does it respond across the range music
actually occupies", and after normalization the second question is the whole game — the first one a
binary switch passes trivially.

**What found it instead**, and what a fix would formalize: the same contact sheet rendered at three
excitations (0.12 / 0.42 / 1.0) and compared *to each other*. Quiet and typical were pixel-identical
across all 14. That is a change to how existing frames are compared, not new machinery.

**What a design here would weigh:**

- **A mid-scale rung in the reactivity gate.** Drive each band to ~0.4 as well as 1.0 and require
  the two frames to differ *from each other*, not merely from silence. Catches saturation directly,
  and is the smallest change that would have caught this.
- **A ceiling-occupancy statistic in `--report`.** The walker already records how close each
  `clamp()` came to its bound — that is where `ceils` comes from. The mirror, what fraction of hops
  a clamp spent *at* its bound, falls out of the same walk, and a term pinned 90 % of the time is
  exactly this defect, named per binding instead of inferred from a table.
- **Leave it and rely on the ladder.** Cheapest; documents the three-level sheet as the standing
  audit move rather than automating it. Weakest, because nothing then runs in CI.

**Note the mirror-image history.** Plans 0041/0042 spent themselves closing the failure where a
*threshold* sat above anything music produced. This is the same class one level down — a *gain*
below anything music produces — and the instruments that closed the first were never extended to it.

---

## ~~0044 — Phase 1's axis rebuild silently re-pointed every sub-crossover `bin()` probe in the library, and three preset headers plus `docs/presets.md` still teach the axis it replaced~~

- **PROMOTED 2026-08-03 → [ADR-0063](adrs/0063-address-the-spectrum-by-frequency.md)** (`bin_hz` /
  `bin_range`, folding in the `bin_range` ADR-0036 deferred), with the external axis anchor landing
  as [Plan 0056](plans/done/0056-clamp-occupancy-and-the-axis-anchor.md) Phase 4. **The doc half is
  done** — `docs/presets.md`'s axis block was regenerated from `fft.rs` + `expr.rs` and the stale
  `onset` line removed. Two corrections the regeneration produced, both sharper than this entry:
  every band is now **1.8 semitones wide everywhere** (the old "two regimes" and "the bottom is the
  coarsest region" are both simply gone), and the mapping is now **sample-rate independent** at
  44.1 / 48 / 96 kHz, retiring the old "this moves with the sample rate" warning. The seven
  retuned probe positions check out at 0-5 % of their authors' targets. Notes retained below.

- **Raised:** 2026-08-02, from `preset-author`, running Plan 0048 Phase 7.
- **Verified against code:** yes — `core/src/dsp/fft.rs:94` lays every band edge on
  `BAND_LO_HZ * ratio^(k/64)`, with the long window feeding everything below the ~246 Hz crossover,
  so the axis is now genuinely logarithmic end to end.

**The content half is fixed in the Phase 7 commit; the doc half is not, and it is architect's.**

Before Plan 0048 every band was floored at one FFT bin (23.4 Hz at 48 kHz), which bound the bottom
half of the axis *linear* and made a log-curve fit up to **2.9x wrong** below ~750 Hz. That fact is
written, in capitals, into three preset headers and a `[!WARNING]` block in
[presets.md](presets.md) — all of them telling the next author **not** to compute a position from
`35 * 514.3^x`. Phase 1 made that formula exactly right, and every one of those warnings now says
the opposite of the truth.

The consequence for content was silent and real: **every probe below the crossover dropped about an
octave and a half.** `fragment_aurora`'s colour idea is a contrast between air and low-mid, chosen
explicitly so that *loudness* cannot move it. Its low probe was `bin(0.14)` for ~246 Hz; on the
rebuilt axis that position reads **~84 Hz** — a kick probe, which would have lurched the curtain
green on every bass hit. `attractor_dejong`'s header even names 65 Hz as the mistake a log fit once
made, and 65 Hz is precisely what its `bin(0.10)` now reads. Seven positions across three presets
were moved back onto the frequencies their authors named; the four above the crossover never moved.

**The general hazard, which is the part worth a decision:** a DSP change can re-point every `bin()`
in the library without failing anything. `bin(0.14)` is still a valid expression, still returns a
number, still renders. No gate compares a probe against the frequency its author intended, because
nothing records that intent anywhere a machine can read — it lives in a comment.

**What a design here would weigh:**

- **A frequency-addressed companion**, e.g. `bin_hz(246)`, so a preset states the intent it actually
  has and survives any future axis change. Interacts with the deferred `bin_range(lo, hi)` in Plan
  0048's followups — both are the same request: address the spectrum by what you mean.
- **A golden assertion on the axis map.** Pin the Hz of a handful of `x` positions in a test, so a
  layout change fails loudly and whoever makes it knows to sweep the library.
- **Nothing, plus a doc rule** that an axis change is a preset-sweep event, the way a C ABI change
  is an ADR event.

**Immediate, and small:** [presets.md](presets.md)'s axis `[!WARNING]` block and its measured
position table describe the pre-Phase-1 axis and want regenerating — Phase 5 regenerated the *level*
tables and did not reach this one. The same file still says `onset` "is raw spectral flux with a
peak near `0.016`" in its reachability section, which ADR-0049 retired.

---

## ~~0045 — `docs/analysis-v2-before-flags.md` has served its purpose and asks to be deleted~~

- **DONE 2026-08-03.** File removed and all inbound links rewritten. There were **three**, not two:
  `docs/presets.md`, `docs/capturing.md`, and `presets/README.md` — the third sits outside `docs/`
  and the entry's grep missed it.

- **Raised:** 2026-08-02, from `preset-author`, running Plan 0048 Phase 7.

The file's own closing line is "Delete this file once its Group 2 list is empty." It is: all nine
band-threshold flags are retuned, and `--report` is down from 26 to **17 flags, every one of them
the standing `tempo` single-BPM false positive** that the file's Group 1 documents as correct
behaviour rather than work.

Left in place by this lane rather than deleted, because two docs link to it
([presets.md](presets.md) and [capturing.md](capturing.md)) and this repo keeps its
cross-references resolving. Retiring it is a three-file edit and belongs with the Plan 0048 close.

---

## Entry 0046 — from the Plan 0048 close, and then corrected by the lane it was handed to

---

## ~~0046 — the retune's gain rule is direction-blind, so the six presets whose idiom *subtracts* light came out ~25 % darker~~

- **Raised:** 2026-08-03, from `architect`, after the user ran the app on the retuned library and
  reported "strange attractors are very dim".
- **RETRACTED the same day, by `preset-author`, before any preset was edited.** The claim is
  **false** and the fix it specified would have made the family worse. Kept in full rather than
  deleted: the *reason* it was wrong is a trap that will catch the next person who audits a
  semantic change, and it is the only durable thing here.

**What the entry claimed:** that Phase 7's `G' = C / P` rule is direction-blind, over-engaging the
22 subtractive terms, and that the six `attractor_*` presets consequently render ~25 % darker at
typical excitation. It carried a measured table (clifford 78.7 → 61.0, dejong 50.3 → 38.8, lorenz
49.9 → 37.0) and worked arithmetic through accumulation and point area.

**THE BASELINE WAS NOT THE AUTHORED LOOK.** Every "pre" figure came from rendering the
**pre-retune preset file on the post-retune engine**. On the v2 scale that file's gains are 16-96x
too hot, so *every* clamp in it pins at its cap — which for the additive luminance terms
(`trails`, `bg_bright`, `saturation`) means **maximum brightness**. The "regression" was the
distance between a correct render and a saturated one. It measured the defect the retune fixed and
read it as the standard to restore.

**Fed each file the values it was authored against, at the same physical stimulus** (v1 file at the
raw means `0.040 / 0.006 / 0.006 / 0.0016`, v2 file at the normalized means
`0.661 / 0.575 / 0.281 / 0.145` — the same `dynamic:110` hop, each side seeing its own scale):

| preset | authored (v1) | shipped (v2) | delta | coverage v1 → v2 |
|---|---|---|---|---|
| `attractor_clifford` | 50.8 | 68.5 | **+35 %** | 35.1 % → 44.4 % |
| `attractor_dejong` | 33.9 | 45.0 | **+33 %** | 21.3 % → 53.2 % |
| `attractor_lorenz` | 30.1 | 41.6 | **+38 %** | 25.8 % → 42.0 % |
| `attractor_leviathan` | 64.7 | 60.6 | −6 % | 48.6 % → 64.2 % |

The retune made this family **brighter and better-covered**, not darker.

**The mechanism in the entry was also backwards.** Restoring `fade` + `size` to their v1 gains makes
Clifford **darker by 4.6**, because on the v2 scale the old gain saturates the subtractive cap.
Attributing the (spurious) 17.6 gap by restoring one group at a time: `zoom` +6.1, colour +3.7,
`trails` +2.8, `bg_bright` +2.0, coefficients −0.9, and `fade` + `size` **−4.6**. The subtractive
terms were the one group moving the *other* way.

**And the family is not dim in context.** The whole shipped library at the typical stimulus puts the
four dark attractors at **41.6 / 45.0 / 60.6 / 68.5** against a median of ~57 — mid-pack. The genuinely
dim presets are `spectrum_corona` (9.1), `star_lantern` (14.0) and `swarm_storm` (16.5), none of
which the user flagged.

**The live cause is almost certainly the tier, which is why nothing here could see it.**
`TierConfig` (`core/src/render/tier.rs:219,237`) sets `attractor_particles` to **50 000 at `Floor`
and 150 000 at `Rich`** — exactly 3x. The attractor is the **only** family whose luminance *is* its
particle count, because it accumulates additive points; every other family's look is independent of
the capacity multipliers. The reporting session had the governor demote `rich → floor`, so the
attractors alone lost two thirds of their emitters while the rest of the library was untouched —
which is exactly the shape of the report ("more or less fine, but strange attractors are very dim").
So this is **[backlog 0031](design-backlog.md) and Plan 0044's never-run `Rich` calibration**, not
content work. **`shot` is `Floor` by construction (ADR-0045) and cannot test it** — the running app
under `--tier rich` is the only instrument.

**The durable lesson, and the reason this entry survives its own retraction:**

> **A pre-change preset file rendered on the post-change engine is not a baseline.** After any
> semantic change to what a variable *means*, the old content evaluates wrongly by construction —
> usually saturated — so "old file vs new file on today's build" measures the defect, not the
> intent. The authored look is only recoverable by feeding the old file the **values it was written
> against**. Both this entry and the Plan 0048 close review made this mistake, in the same session,
> having already written down that ADR-0049 changed what those variables mean.

Two smaller things worth keeping:

- **We have no instrument for "did this preset's look change".** Everything measures reactivity
  (against silence) or saturation. The absolute-luminance-at-matched-stimulus comparison used here
  is ad hoc and lived in a scratch directory. Worth a harness home if a content-wide pass ever
  happens again — it is what would have answered this in one command instead of a day.
- **The `Rich`/`Floor` split is invisible to every automated check we have**, and one family's
  headline look depends on it 3:1. That is a bigger gap than the calibration ticket implies.

---

## Entries 0047-0048 — the 2026-08-03 `preset-author` batch (seventh), from the attractor ceiling pass

---

## ~~0047 — `Rich` triples the attractor's light with nothing normalizing for it, so the tier is **not** look-neutral, and no automated check can see the difference~~

- **PROMOTED 2026-08-03 → [ADR-0064](adrs/0064-a-capture-may-pin-the-rich-tier.md) +
  [ADR-0065](adrs/0065-the-attractor-deposit-is-normalized-by-particle-count.md) +
  [Plan 0057](plans/done/0057-the-attractors-compute-path.md)** — both halves are taken, and they went to
  different places. The **deposit** is normalized by particle count (Plan 0057 Phase 2), which was
  this entry's first and cheapest option; the **`shot --tier` flag** is taken too rather than
  rejected as the entry expected, because a *pinned* tier is an input and ADR-0045 already named
  capture-level `Rich` spot checks as that tier's verification path (Plan 0057 Phase 1). The third
  option — document the 3x and leave the trap armed — is rejected in ADR-0065's Alternative A.
  **The second finding (the `sanity` gate passing a saturated frame) went to
  [Plan 0056](plans/done/0056-clamp-occupancy-and-the-axis-anchor.md) Phase 5**, as this entry suggested,
  where it is a general flat-frame check rather than an attractor-specific one — with the honest
  question attached: `sanity` renders at `Floor` and at silence, so the phase must state which of
  these four presets it would actually have caught. Notes below retained as the origin record.

- **Raised:** 2026-08-03, from `preset-author`, fixing four attractor presets the user reported as
  "very dim" at `Rich` (they were saturated, not dim).
- **Verified against code:** yes — `core/src/render/scenes/particles/mod.rs:387-393` and
  `core/src/render/tier.rs:219,237`.
- **Cost when it bit:** four shipped presets rendered as flat single-tone masses at the tier the
  app **starts on**, through a green suite, until a user said so.

**The mechanism.** `attractor_particles` is **50 000 at `Floor` and 150 000 at `Rich`**. The draw
blends `One, One` and the fragment emits `in.color * g` with **no division by particle count**, so
`Rich` deposits **three times the light** into the same accumulation texels. A preset authored at
`Floor` — which is the house rule, `presets/README.md` says so — is three stops hot at `Rich`.

**This contradicts a claim we make in two places.** ADR-0045 and `presets/README.md` both say
`Rich` raises **capacity, not behavior**, and that "no expression, param or structural field
changes meaning". For an *accumulating additive* scene, capacity **is** behavior: the same `fade`
and `size` produce a different picture. The claim holds for every other family and fails for this
one.

**Nothing can catch it.** ~~`shot` has no `--tier` and is `Floor` by construction, deliberately
(ADR-0045)~~ — **corrected 2026-08-03: `shot --tier floor|rich` exists and works**, built by Plan
0044 Phase 3 (`Renderer::new_headless_tiered`) and documented in `docs/capturing.md`; it is absent
only from `shot --help`, which is how four places across these documents came to claim otherwise.
What remains true is the half that matters here: **every golden baseline and every behavioral gate
still runs at `Floor`**, because `Renderer::new_headless` pins it and nothing opts those paths into
`Rich` — so they all describe a configuration the app does not start in, and a `Rich` capture is
available as an *instrument* but is not wired into any gate. The workaround found here is worth
writing down
wherever a future session will look: **multiplying `exposure` by 3 is equivalent to tripling the
particle count**, because accumulation is linear and the tonemap is terminal. It reproduced the
user's frame exactly.

**What a design here would weigh:**

- **Normalize the deposit by particle count** (scale by `FLOOR_PARTICLES / actual`). Makes the
  claim true: `Rich` then buys *smoothness* — less shot noise in the same picture — instead of
  brightness, which is what a capacity tier should buy. Cheapest and most honest. Note it would
  move the look of every attractor preset once, so it wants the same care as a retune.
- **Give `shot` a `--tier` flag.** Directly contradicts ADR-0045's reason for not having one
  (a capture must be a pure function of its inputs), but a *pinned* `--tier rich` is still pure.
  This is the only option that makes the difference **testable** rather than merely correct.
- **Stop claiming tier is look-neutral** and document the 3x, leaving authors to hold headroom.
  Free, and it is what the four presets fixed today already do — but it leaves the trap armed for
  the next author.

**A second, cheaper finding rides along.** The `sanity` gate renders **at silence** and asserts a
real shape exists. It passed a fully saturated single-tone frame for as long as these presets have
shipped. The statistic that exposed this — **the share of the lit figure sitting inside one narrow
luminance band** — is four lines of code over a frame the gate already renders, and it is a
general "the picture has no tonal structure" check, not an attractor-specific one. Worth folding
into whichever plan takes the occupancy gate ([ADR-0062](adrs/0062-clamp-occupancy-is-the-saturation-instrument.md)),
since both are "the frame looks alive to our instruments and is not".

---

## ~~0048 — the `lorenz` attractor family renders as a dust cloud, and no preset key reaches the scatter~~

- **DIAGNOSED AND RE-PROMOTED 2026-08-03 → [ADR-0068](adrs/0068-the-projection-basis-is-a-per-family-property.md)
  + [ADR-0069](adrs/0069-the-attractor-trades-sample-count-for-trace-length.md) +
  [Plan 0059](plans/done/0059-lorenz-finds-its-plane.md).** The diagnosis below was right and this entry's
  own three candidates were all wrong — [Plan 0057](plans/done/0057-the-attractors-compute-path.md)
  Phase 4 confirmed the shared 3-D **view basis** by discriminating capture and ruled out
  integration and an un-converged seed by measurement (the cloud fills 5.89 % of its own bounding
  volume, stable from 60 through 240 to 600 frames, where an un-converged seed box reads ~26 %). It
  then stopped, by its own instruction, because a shared convention changing for one family is a
  decision rather than a constant. **Two things the entry could not have known, both from doing the
  arithmetic at design time.** A basis fix alone leaves the figure reading as *stipple*, because the
  scene draws 50 000 independent samples of the invariant measure where the iconic plot follows one
  trajectory as a curve. And **this entry's ask — "no preset key reaches the scatter" — turns out to
  be half right for the wrong reason**: the key worth having is not a seed spread but a **`density`**
  that trades sample count for trace length, and it only became *safe* to expose one plan ago, when
  [ADR-0065](adrs/0065-the-attractor-deposit-is-normalized-by-particle-count.md) made the particle
  count stop being the frame's brightness. Notes below retained as the origin record.

- **PROMOTED 2026-08-03 → [Plan 0057](plans/done/0057-the-attractors-compute-path.md) Phases 4-5** — no
  ADR yet, deliberately. The phase is a **diagnosis before a fix**, because the leading hypothesis
  is not one of the three this entry named: the draw shader's 3-D branch uses **`y` as the
  vertical** and rotates `x` against `z` (`particles/mod.rs:355-360`), so the view is x-y at rest
  and z-y at a quarter turn — and the Lorenz butterfly lives in the **x-z** plane. The x-y
  projection of Lorenz is a dense core inside a diffuse cloud, which is this entry's description
  verbatim. Lorenz is the only family it can be wrong for: the discrete maps never take the branch,
  and Thomas is cyclically symmetric. If that is the cause, the fix is a shared-convention change
  and **Phase 4 routes back to `architect`** for the ADR. The entry's own candidates (integration
  step, settle iterations, a seed-spread key) are carried as secondary hypotheses with the estimates
  that make them unlikely. Notes below retained as the origin record.

- **Raised:** 2026-08-03, from `preset-author`, in the same pass.
- **Verified against code:** yes — `[particles]` accepts **only** `family`
  (`core/src/preset/schema.rs`, and the table in `presets/README.md`).
- **Cost when it bit:** one of the four attractor families cannot be made to read as its own
  shape, so `attractor_lorenz` is carried by colour rather than by geometry.

**What it looks like.** The Lorenz butterfly never resolves. A faint dense core sits inside a large
diffuse halo of scattered points that does not converge — after 240 frames with no reseed firing,
which is long past when the map should have settled onto the attractor. The other three families
(`de_jong`, `clifford`, `thomas`) all resolve their structure cleanly under the same treatment,
which is what makes this look like a property of this family's integration rather than of the
preset.

**Why the content lane cannot fix it.** Lowering density and backdrop (done today) improves the
*contrast* of the core against the halo but does not reduce the halo — the scatter is where the
particles **are**. `[particles]` exposes `family` and nothing else: no seed spread, no integration
step, no settle time, no cull. The four bindable coefficients are `sigma`/`rho`/`beta` and moving
them changes which attractor it is, not whether it has converged.

**What a design here would weigh:** whether the Lorenz integration needs a different step size or
a normalization the other three do not (it is the one *continuous* system in the set — the other
three are discrete maps, so "one iteration per frame" means something different for it); whether
the seed cloud should be given settle iterations before the first draw; or whether a `[particles]`
key for the seed spread is the right escape. Sizing this needs someone who can read the compute
shader, which is why it is here rather than fixed.

**Related:** [0031](design-backlog.md) (the same family's reseed transient at `Rich`) and
[0047](design-backlog.md) above — all three are the attractor's compute path being less controlled
than the scenes around it.

---

## 0049 — the fold's residual rays got a second rejection and a shipped instance, and they are avoidable from content

- **Raised:** 2026-08-03, from `preset-author`, after the user saw them on `attractor_leviathan`
  with the fold pinned on.
- **This is NOT a new design.** [Plan 0055](plans/done/0055-the-fold-edge-becomes-a-choice.md) /
  [ADR-0061](adrs/0061-kaleidoscope-edge-treatment-is-a-per-preset-choice.md) already design exactly
  what the user asked for — "remove them and make it an optional parameter" is that plan's
  `kaleido_edge` selector, verbatim. This entry exists to carry **three facts into its Phase 2**,
  the `human` A/B that decides which treatments ship and which is the default.

**1. The rays have now been rejected twice, independently.** ADR-0047 recorded them as an *accepted
cost*; the 2026-08-02 in-motion review rejected them (which is why Plan 0055 exists); and the
2026-08-03 session rejected them again on sight, unprompted, with the verdict that they "rarely
work with a preset". **Plan 0055 Phase 1 ships today's falloff-disc as `kaleido_edge = 0`, the
default, so that nothing moves on adoption.** That is the right way to land the phase, but Phase 2
should treat "the current default survives" as the *unlikely* outcome rather than the neutral one.
Phase 3 already has the machinery for a default change and its risk note prices it.

**2. There is now a shipped preset that shows them permanently.** `attractor_leviathan` had its
fold pinned on (the user's call: a leaf count that changes with the music reads as chaos), so what
used to appear only while `bass + mid > 1.12` is now the resting state. Plan 0055 Phase 2 asks for
the A/B to run on "a border-filling field" and one other scene — this preset is a *third* case
worth including, because it is the one where the artifact is now always on screen.

**3. The finding that changes how Phase 2 should be read: the rays are the DISC EDGE smeared
radially outward, so their brightness is set by what the figure puts at that edge — and a preset
can avoid them entirely.** Verified by rendering: at `zoom = 1.12` Leviathan's creature reached the
inscribed radius and the frame grew a permanent starburst; at `0.72` (peak 0.86, inside the disc)
there are **no rays at all**, with the rosette otherwise unchanged. Ruled out as causes on the way:
`trails = 0` and `bloom_amount = 0` both leave them identical.

Two consequences for the plan:

- **"How bad is the current default" is not a property of the treatment alone — it is a property of
  the treatment crossed with whether the scene fills its border.** A fullscreen field has no choice
  and gets the worst of it; a centred figure can duck it entirely. If Phase 2 A/Bs only
  border-filling scenes it will overstate the artifact, and if it A/Bs only centred ones it will
  understate it. It already names both, which is right — this is the reason *why* it is right, and
  it is worth stating in the verdict.
- **There is a content rule here that is not written anywhere**, and it is cheap: *on a preset with
  an active fold, keep the figure inside the inscribed disc or the edge smears outward as rays.*
  It belongs in `presets/README.md`'s kaleidoscope section whenever Plan 0055 Phase 4 does its doc
  pass — it is useful even after the edge becomes selectable, because it explains what the
  treatments are treating.

**4. A third shipped instance, 2026-08-04 (Plan 0059 Phase 4, `990fedc`) — and it is evidence for
how the content rule fails in practice.** `attractor_clifford` inherited `kaleido_order = 2` from
`f09f1fe`, where the fold was added as a symmetry A/B **with the unfolded framing still under it**.
The ribbon's tips therefore sat on the frame edge and smeared into a permanent starburst; pulling
the zoom peak from `1.42` to `0.94` removed them, the same lever and the same direction as
Leviathan's `1.12 → 0.72`. **Two data points now say the rays are avoidable from content.** But
both also say they are avoidable *only if someone remembers to re-frame when the fold arrives* —
in this case the person who added the fold and the person who found the rays were the same lane, one
commit apart, and it still shipped provisionally. That sharpens the entry rather than softening it:
a content rule nobody is prompted with is not a mitigation, which is an argument for Plan 0055's
`kaleido_edge` doing the work rather than for the rule doing it.

---

## ~~0050 — the attractor reseed scatters into an axis-aligned BOX, so every reseed flashes a speckled rectangle across the frame~~

- **PROMOTED 2026-08-03 → [ADR-0066](adrs/0066-a-reseed-disturbs-the-cloud-rather-than-replacing-it.md) +
  [Plan 0057](plans/done/0057-the-attractors-compute-path.md) Phase 3** — the first of this entry's four
  options is taken: a reseed **perturbs the cloud in place** rather than re-filling `seed_box`, so
  no rectangle exists at any tier and there is no convergence transient to wait out. Shaping the
  volume and fading the box in are both rejected in ADR-0066 for the same reason — they treat the
  hard edge and keep the wipe, which is the half `Rich` makes worse; the `[particles]` key is
  rejected because all six shipped presets want the same answer, so the default *is* the decision.
  **The second finding — that nothing in the harness can render the frame — is Plan 0057 Phase 1's
  second instrument**, a `--signal` kind whose onsets clear the highest shipped gate
  (`attractor_clifford`'s `onset > 0.75`). Notes below retained as the origin record.

- **Raised:** 2026-08-03, from the user, on `attractor_ink` at `Rich`: "a square artifact that breaks
  the flow... it's blinking sometimes". Seen on **all** attractors.
- **Verified against code:** yes — `core/src/render/scenes/particles/mod.rs:185-193` (`seed_box`)
  and `:1143-1155` (`seed`).
- **Sharpens [0031](design-backlog.md)**, which already records "the `Rich` tier's 3x particle
  count makes the attractor reseed transient opaque". This entry supplies the part that entry did
  not have: **why it is a rectangle**, and why it is getting worse.

**The mechanism.** `seed_box` returns axis-aligned half-extents per family — `±1.5` in x and y for
De Jong and Clifford, `±4.5` for Thomas, `±(20, 26)` for Lorenz — and `seed()` fills every particle
uniformly inside it. So a reseed does not "scatter" the cloud, it **replaces it with a uniform
rectangle**, which stays a rectangle until the map has iterated enough times to pull the points onto
the attractor. What the user sees is that rectangle's interior: flat random speckle, hard
axis-aligned edges, fading over some frames.

**The geometry corroborates it.** The box is *square* in world space and the vertex shader divides x
by the target's aspect, so on a 16:9 display the square must project **taller than wide** — which is
the proportion in the report. Nothing else in the pipeline produces an axis-aligned rectangle: the
trail grid rounds **up** and presents as a normalized stretch (ADR-0037), so it cannot leave an edge
inside the frame.

**Why it is newly visible, which is the part worth knowing.** The reseed gates were **dead** for most
of this project's life — every attractor shipped with `reseed` written against raw levels it could
not reach ("never fired once", `attractor_clifford.toml`'s own header). Plan 0041's content re-gain
(`e9a1c3c`, 2026-07-29) made them fire for the first time, and Plan 0048's retune rescaled them onto
the normalized axis. So the artifact is **as old as the scene and as new as the gate working**: the
user's instinct that it is unrelated to recent work is right about the mechanism and wrong about the
exposure. `Rich` then triples the particle count into the same rectangle, which is [0031](design-backlog.md).

**It has no headless reproduction, and that is a second finding.** `--set` holds a level constant, so
a held `onset = 1` reseeds *every* frame and averages into no visible box — the artifact is a
**transient** and `--set` cannot express one. A `--signal click:120` filmstrip does not catch it
either, because the synthesized clip's `onset` never clears the shipped `0.56` gate. So **nothing in
the harness can currently render the frame the user is complaining about**, which is why no gate or
baseline has ever seen it. Anything designed here should come with a way to capture a reseed frame —
a `--signal` whose onsets actually cross the shipped thresholds would do it, and would be useful well
beyond this entry.

**What a design here would weigh:**

- **Seed onto the attractor instead of into a box.** Keep a pool of on-attractor points (the current
  cloud is one) and reseed by jittering positions rather than replacing them, so a reseed reads as
  the figure being *disturbed* rather than erased. Most faithful to what the presets ask for — every
  header describes reseed as a percussive accent, not a wipe.
- **Fade the reseed in over N frames**, so the box is never fully opaque. Cheapest, and it directly
  targets [0031](design-backlog.md)'s "opaque at `Rich`" wording — but it leaves a rectangle,
  just a fainter one.
- **Shape the seed volume** (a disc/gaussian rather than a box). Removes the *hard edge*, which is
  what makes it read as an artifact rather than as texture, and is a few lines in `seed`. Does not
  fix the wipe.
- **Give the content lane control.** `[particles]` currently accepts only `family`, so a preset can
  choose *when* to reseed but nothing about what it looks like. Any of the above could be a key
  instead of a constant — though the defaults matter more here, since all six shipped presets would
  want the same answer.

---

## Entry 0051 — from the Plan 0054 Mode 4 review (2026-08-03)

---

## 0051 — `variant` can morph now, and neither shipped `star_*` preset does, because both drive it with a sawtooth

- **Raised:** 2026-08-03, from the [Plan 0054](plans/done/0054-the-line-scenes-catch-up.md) close
  review. Named in that plan's own Followups and in
  [ADR-0060](adrs/0060-star-pattern-variants-interpolate.md)'s Outcome; recorded here because this
  file is where the content lane looks, and a capability nothing demonstrates is indistinguishable
  from one that does not exist.
- **Verified against code:** yes — `presets/star_rosette.toml` and `presets/star_lantern.toml`.
- **Lane:** `preset-author`. No engine work.

ADR-0060 turned `star_pattern`'s `variant` from a `floor` into three cached rosettes into a
continuous contact angle, so a fractional value is a real intermediate figure and `[smoothing]` on it
morphs. The user's ask behind it was "change between star rosette shapes should be smooth", re-raised
as "can we make morphing between shapes easier, slower?".

**Both shipped presets still `floor` it, and Plan 0054 left them that way on purpose.** Each drives
`floor(mod(time * k, 3))` — a sawtooth — so removing the `floor` alone would replace one slow swap
with a *hard* `2 -> 0` snap at every wrap, which is worse than what it replaces. The composition that
actually delivers the morph is a **triangle wave over `0..2`** (up then back down, no wrap
discontinuity) with a smoothing constant on `variant`, and that is content, not engine.

Until someone writes it, the shipped library demonstrates none of the capability the plan was spent
building, and the engine's own tests are the only thing that exercises it. Both files carry a comment
at the binding pointing here.

**Worth pairing with the interior question.** The other half of [0007](design-backlog.md) — the
rosette leaves the inner 60 % of its disc empty at `star_rosette`'s angle and 87 % at
`star_lantern`'s, both now pinned as tests against `sin(a)/sin(pi/n + a)` — is the user's actual
"looks poor" verdict, and it is generator design work rather than content. A smooth morph between
three hollow rings is still three hollow rings.

---

## Entry 0052 — from the Plan 0056 close (2026-08-03), found by the gate the plan built

---

## ~~0052 — `Spectrum Ridge` has no tonal structure at all: it is the one shipped preset the new flat-frame gate convicts~~

- **RETIRED 2026-08-03 — the preset was never flat, and the statistic convicted the right preset
  for the wrong reason.** The `1.000` was not a saturated figure: `scale = 3.20` had been tuned
  before [ADR-0049](adrs/0049-analysis-v2-dual-resolution-axis-normalized-bands.md) normalized the
  bands, so the contour sat ~3.3 world units up against a visible half-height of `1.0` and was off
  frame **entirely**. What the gate measured was the lit `bg_vignette` left behind. The preset was
  repaired in `81190ac` (`3.20 -> 0.60`), `KNOWN_FLAT` emptied itself in `4d325fc` exactly as
  designed — a repaired preset fails its own exemption and tells you to delete the line — and
  [Plan 0058](plans/done/0058-the-gate-can-see-an-empty-frame.md) Phase 2 then re-measured it at
  **`0.1916`** once the backdrop stopped counting as a figure
  ([ADR-0067](adrs/0067-coverage-measures-the-scene-not-the-backdrop.md)). The mechanism this entry
  named — two mirrored haloed strokes adding at their convergence — is real and is Plan 0039
  Phase 5's finding, but it is not what produced this number.
- **The one number worth carrying forward is the last bullet, and it got worse rather than
  better.** Removing the backdrop cost the whole library a spread of mid-tones, so the flattest
  shipped preset is now **`Rose Web` at `0.8839`** (up from `0.7645`) against the same `0.90`
  ceiling — `0.0161` of margin, not `0.035`. Nothing about that preset changed; the number is worse
  because it is honest. A content pass that raises `trails` on a line preset still owes a `sanity`
  re-run, and the question if the top keeps climbing is whether a flat-spectrum stimulus can fairly
  judge these shapes — not whether to nudge `0.90`. Notes below retained as the origin record.

- **Raised:** 2026-08-03, by [Plan 0056](plans/done/0056-clamp-occupancy-and-the-axis-anchor.md)
  Phase 5 — the first thing the tonal-flatness statistic found when it was pointed at the shipped
  library.
- **Verified against code:** yes — measured, and the preset's own header already names the
  mechanism.
- **Lane:** `preset-author`. No engine work.

`core/tests/sanity.rs` now measures **tonal flatness**: the share of a frame's lit pixels sitting
inside one of 16 narrow luminance bands. `coverage` asks *is something there* and `quadrant_spread`
asks *is it more than a dot*; a fully saturated single-tone mass answers yes to both and is still a
blot, and this is the third question.

**`Spectrum Ridge` measures `1.000`** — every lit pixel in one luminance band. It is listed in
`KNOWN_FLAT` and tracked rather than gated on, because Plan 0056 was explicitly test-and-harness
only and repairing a preset is content work.

**It is not the fixture being degenerate**, which was the first thing checked: its two siblings draw
the *same* all-bands-at-1.0 data under the identical stimulus and read `0.31` and `0.44`. It is one
preset saturating, by the mechanism its own header already records — *two haloed strokes at the same
spot add on an additive renderer*, the mirrored-contour convergence Plan 0039's Phase 5 note warned
about when it brought `glow` down for the same reason.

**The `KNOWN_FLAT` entry is asserted to still be flat.** When this is repaired the test fails and
tells you to delete the line, rather than leaving a stale exemption behind — so this entry closes
itself once someone does the work.

**One number worth carrying past the fix:** the threshold is `0.90`, and past `Spectrum Ridge` the
library's highest is `0.830` (`Rose Trails`), then `0.765` (`Rose Web`) — both trails-heavy line
looks where most lit pixels are faint tail at one level. That is not much headroom, so a content
pass that raises `trails` on a line preset should re-run `sanity` rather than assume.

---

## Entry 0053 — from the `preset-author` pass on 0051/0052 (2026-08-03)

---

## ~~0053 — Plan 0048's retune rescaled the band GAINS but not the world-space params those bands multiply, and no instrument can see the result~~

- **PROMOTED 2026-08-03 → [ADR-0067](adrs/0067-coverage-measures-the-scene-not-the-backdrop.md) +
  [Plan 0058](plans/done/0058-the-gate-can-see-an-empty-frame.md)** — coverage is measured against
  black rather than a sampled corner, every floor is re-derived, and Phase 3 takes the general
  form of the second finding (more audio must not draw less picture) so no per-param audit is
  needed. The `spectrum_comb` / `spectrum_corona` re-scale is that plan's Phase 4. Notes below
  retained as the origin record.

- **Raised:** 2026-08-03, from `preset-author`, while repairing `spectrum_ridge` for
  [0052](design-backlog.md).
- **Verified against code and by rendering:** yes — measured under `--signal noise:7` and
  `--signal dynamic:110`, and against `core/tests/sanity.rs`'s own numbers.
- **Two findings, and the second is the durable one.**

### One — a shipped preset had been drawing itself off-screen since ADR-0049

`spectrum_ridge` ran `scale = 3.20`. `scale` multiplies the element level to produce a **world**
height, and every value in that file's long tuning history was chosen when a loud band read about
`0.1`. ADR-0049 normalized the bands to `0..1`, so the same constant was suddenly multiplying a
value roughly **five times larger**: a fully-driven element sat at about **3.3 world units against a
visible half-height of 1.0**.

Rendered, that is not a subtle mis-tune. Under `--signal noise:7` the frame came back **empty except
the vignette**. Under `--signal dynamic:110` only the near-vertical connecting segments crossed the
frame edges; the peaks were never visible at all. Repaired here to `0.60` (Plan 0054/0056 close
pass), which puts a fully-driven contour just inside the frame.

**Plan 0048 Phase 7's retune was a real and careful pass and it did not cover this class.** It
rescaled `clamp(band * G, 0, C)` **gains** — the terms whose job is to bound a band. A `scale` that
multiplies a band into a world coordinate is the same arithmetic exposure with none of the shape the
retune searched for: no `clamp`, no ceiling, nothing for occupancy (ADR-0062) or reachability to
observe. **`bin()`-driven and `index`-driven world quantities are in the same position.**

### Two — no instrument in this project can see a figure that has left the frame

This is the part worth designing against. `coverage` and `quadrant_spread` measure lit pixels
**against a sampled background**, and `bg_vignette` makes the backdrop a smooth bright-centred
gradient. So a frame containing **nothing but the vignette** scores as a large, well-spread, lit
figure. `spectrum_ridge` passed `sanity` for its whole broken life on exactly that, and its
`tonal_flatness = 1.000` — the number [0052](design-backlog.md) was raised about — **was the
vignette**, not the preset. Repairing the preset moved coverage to `0.63` of genuinely drawn figure.

So the flat-frame statistic did its job by accident: it convicted the right preset for the wrong
reason. Worth knowing before it is trusted as an attractor instrument.

**What a fix might be** (all cheap, none obviously best):
- **An in-frame fraction.** What share of the scene's own drawn geometry lands inside the render
  target. Line and spectrum scenes already build a CPU segment list, so this is measurable without a
  readback for exactly the scenes most exposed to it.
- **Sample the background from the frame's own corners *and* discount a radially symmetric
  gradient**, so a vignette cannot be mistaken for a figure.
- **Render one `sanity` frame at `bg_bright = 0`.** Crudest and probably most effective: with no
  backdrop, an off-frame figure scores a coverage of zero and the existing floor catches it.

### The two siblings are over-scaled by the same factor and their layout is hiding it

`spectrum_comb` still runs `scale = 3.80` and `spectrum_corona` `scale = 5.20`, and **both score
well** (coverage 0.76 / 0.80, flatness 0.31 / 0.44). Rendered, the reason is layout, not health: a
comb roots every bar on a baseline and a corona roots every spoke at a centre, so an overshoot
**clips the tips** and leaves the body of the figure in frame and legible. A `polyline` has no root —
every vertex sits at the level — so the identical overshoot removes the entire figure.

Measured under `dynamic:110`, the comb's tall bars run off the top edge on every peak, which means
the loudest part of the readout is the part you cannot see. That is milder than the ridge and it is
still the "loud reads as less information" failure. **Left unchanged deliberately** — re-tuning two
more shipped presets was outside the 0051/0052 handoff, and the right factor should be decided
together with whatever instrument comes out of finding two, so it can be verified rather than
eyeballed.

---

## Entry 0058 — from Plan 0055 Phase 4 (2026-08-04), the content half of a decision the engine has now made

**CLOSED 2026-08-04**, by content: `859ec66` (the eleven choose a treatment) and the Clifford
reframing inside it. All thirteen fold-binding presets now name a `kaleido_edge` explicitly —
`grep -l '^kaleido_edge' presets/*.toml` returns thirteen — and the verdicts are a genuine spread
(`falloff` 2, `tile` 6, `squash` 5, counting the two Plan 0055 already judged), which is the
evidence ADR-0061's premise was right: one treatment could not have served these.

**The entry named the wrong preset, and the body below is left uncorrected as the record.** Its
list of eleven says `attractor_dejong`. De Jong binds no `kaleido_*` param and never has; the
thirteenth is **`attractor_clifford`**. The error was inherited from
[Plan 0055](plans/done/0055-the-fold-edge-becomes-a-choice.md)'s own scope bullet, which carries the
same correction. The **count** was right and one **name** in it was wrong, which is exactly why it
survived: a wrong name in a list of the right length reads as correct until somebody opens the file.
`preset-author` found it by working the pass off the list and running `grep -l kaleido_order
presets/*.toml`.

**`swarm_dense` was decided, and the fold went back on** — `kaleido_order = "3"`, `kaleido_edge = 1`.
The pin at 1 (fold off) had been a mitigation for backlog 0010's clamped-edge smear, fixed engine-side
a plan earlier; the entry asked for this to be judged rather than assumed, and it was.

**Clifford is the second instance of the Leviathan pattern**, which is the pass's other finding and
outlived this entry: a framing pinned to dodge an engine defect, kept after the defect was fixed,
recoverable only by reading the preset's own header comment. That pattern is
[live entry 0060](design-backlog.md#0060--an-engine-fix-leaves-its-preset-side-workarounds-standing-and-only-a-header-comment-remembers-them).

---

## 0058 — thirteen presets bind the fold and eleven of them have not chosen an edge treatment, because until now there was nothing to choose

- **Raised:** 2026-08-04, at [Plan 0055](plans/done/0055-the-fold-edge-becomes-a-choice.md) Phase 4. Not
  a gap the content lane found — a gap the engine lane *created* on purpose and is handing over.
- **Verified against code:** yes. `grep -l kaleido_order presets/*.toml` returns thirteen files, and
  `grep -l '^kaleido_edge' presets/*.toml` returns two of them, so eleven ride the default. Anchor
  that second grep — `swarm_dense` mentions `kaleido_edge` in a header comment without binding it,
  so an unanchored match reports three.
- **For:** `preset-author`. No engine change, no ADR. The capability exists and is documented; what
  is missing is a per-preset judgement that only looking can supply.

**What changed under them.** [ADR-0061](adrs/0061-kaleidoscope-edge-treatment-is-a-per-preset-choice.md)
made the region outside the fold's inscribed disc a per-preset choice, `kaleido_edge`, with three
treatments: `falloff` (0, the fade ADR-0047 shipped), `tile` (1) and `squash` (2). Plan 0055 Phase
2's live A/B made **`tile` the default**, so every fold-binding preset that says nothing has already
moved from *cropping to a disc* to *filling its frame*. That is a real visual change to eleven
shipped presets, applied by a default rather than by an author, and it is the reason this entry
exists rather than being optional polish.

**Why the scope matters.** At 16:9 the frame's corner sits at 2.04x the disc radius, so **56 % of
the frame** is what the treatment decides. This is not a corner detail on any of the thirteen.

**The eleven that have not been looked at:** `attractor_dejong`, `attractor_lorenz`,
`curve_cathedral`, `fragment_glacier`, `fragment_supernova`, `fragment_warp`, `lsystem_arrowhead`,
`reaction_reef`, `reaction_reliquary`, `swarm_storm` — plus `swarm_dense`, which is a special case
below. All eleven currently ride the `tile` default without anyone having chosen it.

**The two that have been judged, and they are your reference pair.** Plan 0055 Phase 2 put the whole
roster in front of the user in the running app — in motion, over a lit backdrop, at 16:9 and at a
non-16:9 window — on exactly one centred figure and one border-filling field. Both verdicts are
**landed**, so they are shipped examples you can read rather than advice:

| preset | kind | verdict |
|---|---|---|
| `attractor_leviathan` | centred figure | **`tile`**, landed with a zoom raise (see below) |
| `fragment_kaleido` | border-filling field | **`squash`**, landed |

**That the two chose differently is the finding, not a detail.** It is the whole evidence for
`kaleido_edge` existing at all, and it is the first question to ask of each preset below: is this a
figure with space around it, or a field that fills its frame? The pair does not settle the other
eleven — nobody has watched those — but it tells you what the axis is.

**What Leviathan's change tells you about the others.** Adopting a fill treatment there was **two
edits, not one**. Its `zoom` had been pinned at base 0.72 with a header explaining that the pin was
"a fold constraint, not a taste" — the figure was held inside the inscribed disc so it could not
feed the fold's residual rays. A fill treatment removes that constraint entirely, and the preset
only benefits from one if there is content out past `r_max` for it to act on, so the zoom went to
1.80. **Expect the same shape elsewhere:** any preset whose scale, `zoom` or `glow` was tuned against
a disc that crops is now tuned against a premise that no longer holds. Grep the fold-binding headers
for language about the disc, the inscribed radius, or the rays before assuming a file only needs one
line added.

**`swarm_dense` is the odd one and worth doing first.** It pins `kaleido_order = "1"` — the fold off
— and its header documented that as a *mitigation for an engine artifact*: bright bars along the
frame edges, which was design-backlog 0010's clamped-edge smear. That artifact was fixed engine-side
by ADR-0047 a plan ago, so the dodge has been unnecessary since then and the comment was stale twice
over. Phase 4 corrected the comment and **deliberately did not turn the fold back on**, because
nobody has looked at this preset folded since the fix and that is a judgement for this lane. It is a
sparse figure over a dark field, which is the case where the three treatments differ most.

**Pairs with [0038](#0038) and [0040](#0040)**, and the pairing is the argument for doing them
together rather than in sequence: all three are retunes of the same shipped set against a composite
that moved underneath it. 0038 is the tonemap knee's ~8 % luminance loss, 0040 is coverage-as-alpha
making dim figures read as dark speckle over a lit backdrop — and a lit backdrop is exactly the
configuration this entry's treatments are judged in, since under `falloff` the corners *are* the
backdrop and under `tile`/`squash` they stop being it. Judging any one of the three at
`bg_bright = 0` is what produced the confirmation failure ADR-0061's Notes records.

**How to judge it.** In the running app, in motion, over a **lit** backdrop, at 16:9 and at a
window that is clearly not 16:9 — `LMV_PRESET_DIR` pointed at the repo's `presets/` makes an edit
live in about 150 ms, so walking a preset through `kaleido_edge = 0 .. 2` is changing one integer
and watching. The parameter roster and the per-treatment guidance are in
[`presets/README.md`](../presets/README.md#screen-space-kaleidoscope--kaleido_order-kaleido_angle-kaleido_center_x-kaleido_center_y-kaleido_edge).

**Not in scope.** Adding a fourth treatment. The roster is a closed set by ADR-0061; a look that
needs a new edge behaviour is engine work and routes back through `architect`.

---

## Entry 0057 — from the Plan 0059 Phase 4 content pass (2026-08-04)

---

## ~~0057 — a preset has no scene-local way to set a figure's level, so `exposure` gets used for it and two other stages disagree with that use~~

- **PROMOTED 2026-08-04 → [ADR-0080](adrs/0080-the-attractor-owns-its-level-and-bloom-thresholds-exposed-light.md) +
  [Plan 0066](plans/done/0066-the-level-lever.md)** — both halves, at the user's call. The attractor gains
  `brightness`, matching the param `swarm` and `emitter` already carry (it is the **only** particle
  family without one, which is why its two presets reached for `exposure`), and the bloom
  bright-pass thresholds **post-exposure** luminance. **The pixel cost turned out to be nil on the
  golden suite and the arithmetic is why:** no fixture binds `exposure`
  (`grep -l exposure core/tests/fixtures/*.toml` is empty across all 23), so the new factor is
  literal `1.0` and every baseline is byte-identical. The only looks that move are Lorenz and
  Thomas, which Phase 5 retunes because their headers document the retired model.
- **Raised:** 2026-08-04, from `preset-author` (Plan [0059](plans/done/0059-lorenz-finds-its-plane.md)
  Phase 4, `990fedc`). All three findings verified against code, with rendered evidence noted.
- **One entry, not three.** These are one gap seen from three sides: there is no per-scene deposit
  or intensity param, so a figure's level is spent on `exposure` — the one lever that is
  engine-wide, interpolated across a dissolve, and measured *after* the stage that would want to
  discriminate on it.
- **Why it has no history:** `attractor_lorenz` and `attractor_thomas` are the **first two shipped
  presets to bind `exposure` at all.** Nothing had a caller before, so nothing had a complaint.

**1. `density` is exposure-neutral in total light only (ADR-0065), and the docs said otherwise.**
Per texel it is not neutral: the same energy lands on `1/N` of the pixels, so a sparse preset needs
a cut on the order of `trail frames / density`. The shipped values are `exposure = 0.03` on Lorenz at
`density = 0.002` and `0.10` on Thomas at `0.02`, both picked off rendered ladders rather than
derived. `presets/README.md`'s `[particles]` section told authors they could re-aim `density`
*"without re-tuning `size`, `fade` or `exposure`"* — **true of the sum, false of the picture.**
*Corrected at this close* rather than left for the entry's promotion, because it is wrong today and
the next sparse preset would be misled by it.

**2. The ADR-shaped half: should a scene have a local deposit / intensity param?** `exposure` is
engine-wide and **crossfades across a preset dissolve** (`crossfade_from` in
`core/src/render/tonemap.rs`, ADR-0032's seam), so an extreme per-preset value drags the ~1 s blend
through a badly-exposed frame. Both new presets deliberately buy as much of their level as possible
with `size` and `fade` first *because of this* — those are scene-local and blend as pixels. That is
a workaround with a ceiling, and the question it poses is a real tradeoff with alternatives to
reject: a per-scene param, versus normalizing `exposure` per-preset at the crossfade, versus
declaring the current behaviour correct and documenting the workaround as the technique.

**3. `bloom_threshold` is measured in pre-exposure units, so at these values it cannot discriminate
at all.** Chain order is scene → post chain → tonemap, so the bright-pass reads the figure *before*
`exposure` scales it, and `bloom.rs` clamps the threshold at `MAX_THRESHOLD = 8.0`. At
`exposure = 0.03` the whole figure is over any threshold a preset can ask for. **Rendered: threshold
`0.95` against `8.0` on Lorenz are near-indistinguishable.** Lorenz therefore ships it pinned at the
ceiling, with its header saying to read the pair as *capped, not tuned*. **A threshold in
pre-exposure linear units is only meaningful while every preset sits near `exposure = 1.0`** — which
was true until this commit and is now not.

**What this entry is not.** It is not a bug report: nothing renders wrongly, both presets ship the
look they intend, and the workarounds are recorded in their headers. It is the observation that the
workarounds exist because one lever is doing a job it was not shaped for, and that the cost lands on
the *next* author rather than on these two.

---

## Entries 0033-0035 — the 2026-07-30 `preset-author` batch (fifth), from two figurative requests

The user asked for two looks that are **figurative** rather than generative, which is a class this
library has never been asked for before:

1. the Windows Solitaire win-cascade, with **hearts** instead of cards — red fill, black outline,
   falling at a rate set by the BPM, arcing off in different directions and leaving a trail of
   stamped copies;
2. **small seven-, eight- and nine-pointed stars**, white-gold on black, twinkling and flashing on
   bass and beat.

Both were rendered as far as the current surface reaches before being reported. The two requests
are independent in the user's mind and turned out to share **exactly one** root gap, which is why
they are filed together. Neither look shipped; the drafts were discarded on the user's instruction.

**What was already sufficient, and should be said first:** the *audio* half of "falls at a speed
that depends on the BPM, a new one on every beat" needs nothing. `tempo` is BPM, and ADR-0050's
clock — `beat_index`, `time_since_beat`, `beat_in_bar`, `bar_index`, `bar_phase` — supplies both the
per-beat event and the phase to drive an arc from. The gap in both requests is entirely in **what
can be drawn**, not in what can be heard.

---

## ~~0033 — every mark the engine can draw is a round additive blob or a stroked curve, so no *object* has a shape~~ (silhouette half; the fill-and-outline half is re-filed as [0069](design-backlog.md))

- **PROMOTED 2026-08-04 (the silhouette half only) → [ADR-0084](adrs/0084-a-particle-marks-silhouette-is-a-signed-distance-function.md) +
  [Plan 0070](plans/done/0070-shaped-marks.md)** — a `shape` param selecting a signed-distance function
  in the existing particle fragment shader, on `swarm` and `emitter`, keeping the additive model and
  the quadratic falloff. The user chose the SDF route over a fill-and-stroke path, a glyph atlas and
  author-supplied WGSL. **The fill-and-outline half of this entry stays open and is not promoted** —
  a heart in additive light is a heart-shaped glow, and the red-body-black-edge ask still reopens
  ADR-0018/ADR-0056. Re-file that half as its own entry when Plan 0070 lands, so the two stop being
  confused.
- **Raised:** 2026-07-30, from `preset-author`, by both requests above independently.
- **Verified against code:** yes — `core/src/render/scenes/swarm.rs` (`fs_main`),
  `core/src/render/scenes/particles/mod.rs`, `core/src/render/scenes/lines/*`,
  `core/src/render/ink.rs`.

The engine has **no shape vocabulary for an object**. There are exactly two mark-making models:

- **Particles are one hardcoded round blob.** The swarm's fragment shader is three lines —
  `let d = length(in.local); let falloff = max(0.0, 1.0 - d); let g = falloff * falloff;` — a radial
  falloff with no shape input at all. The attractor's compute points are the same idea. There is no
  glyph atlas, no SDF, no shape parameter, and nothing in `PARAMS` that could carry one.
- **Line scenes stroke a generator's path.** `maurer_rose`, the L-system turtle, the Hankin
  rosette, the spectrum comb. These *can* make a shape, but only one figure, centred, whole-frame,
  and only as a **stroke** — there is no fill.

**The second half is worse than the first: the pipeline is additive, so a dark mark cannot exist.**
Every scene blends additively (`swarm.rs`: *"Additive: overlapping particles bloom brighter"*), which
is a lightening model — black adds zero. A red-filled heart with a black outline is **three** tones
(light ground, red body, black edge) and the only dark-on-light route in the engine is the ink stage,
which is structurally **two**-poled: `mix(paper, ink, luminance)`.

**Measured, not assumed.** I drew the cardioid `r = 1 - sin(theta)` through `parametric_curve`
(`n = 1`, `phase = -pi`, `radial_offset = 1`) — a genuinely recognisable heart, and a useful data
point that the *outline* is reachable today. Running the same figure at `ink_amount = 1` with white
paper and black ink rendered the outline **grey, not black**: a thin anti-aliased stroke averages to
mid luminance, so it lands halfway down the paper→ink ramp. The ink stage cannot produce a crisp dark
contour around a light interior, because the contour is not where the luminance is.

**The star half lands on the same gap.** Small 7/8/9-pointed stars scattered across the frame are not
reachable: the swarm can put ten thousand small marks anywhere, but they are round; `parametric_curve`
with `radial_offset = 1` gives exactly `n` lobes and can flip the count every beat
(`n = "7 + floor(hash(beat_index) * 2.999)"`, which works and is rather nice) — but it is **one large
centred figure**, and `mirror_order` replicates about the origin, so the copies land on each other
rather than scattering. Rendered both; the starfield reads well as a starfield and not at all as
*stars with points*.

**Why this is not a preset problem.** There is no combination of existing params that gets closer.
Whatever the answer is — a shape enum on the particle sprite, an SDF glyph, an author-supplied WGSL
pass (already noted as absent in the skill's own gap list), a fill+stroke draw path outside the
additive model — it is a change to how the engine draws, and the fill/outline half re-opens the
additive-blending decision that everything else in the composite assumes.

**Impact.** First time it has been asked for, but it is not an exotic ask: "a shower of *things*" is
a mainstream visualizer idiom, and the request arrived twice in one session from one user. It is also
the gap that most limits what this lane can offer, because the library is entirely non-figurative and
nothing in the grammar hints that figurative is off the table.

**Not deciding:** whether the engine *should* draw figurative objects at all, or which of the four
routes above is right. Both are architect calls, and the additive-model question is ADR-shaped.

---

## Entry 0054 — from the Plan 0058 close (2026-08-03), and the plan's own measurement is the argument

---

## 0054 — pixel coverage cannot see a figure whose *tips* leave the frame, and an in-frame geometry fraction is the successor

- ~~**PROMOTED 2026-08-04**~~ → **CLOSED 2026-08-06 by
  [ADR-0083](adrs/0083-in-frame-geometry-is-measured-at-the-line-renderers-draw-seam.md) (accepted,
  with an Outcome section) + [Plan 0069](plans/done/0069-the-instrument-that-sees-a-figure-leave-the-frame.md)
  (done)** — and the "where does it live" question this entry names as the real decision was answered
  by **none of its three options**. It is measured inside `LineRenderer::draw`, which already receives
  every endpoint *and* the render target's aspect, so all four line families are covered by one
  implementation with no `Scene` accessor (ADR-0067's stated objection) and no harness re-derivation
  of generator math. The entry's per-family limit stands and is documented rather than papered over.
- **What shipped is narrower than what this entry asked for, and the difference is the finding.**
  The entry wanted a successor *gate*. The geometry fraction has **no separating absolute threshold
  either**: `Rose Zoom` (`0.3492`) and `Rose Overflow` (`0.3659`) bracket the frozen over-scaled comb
  (`0.3563`) and both are correct content, because a length fraction cannot tell "deliberately inside
  the figure" from "accidentally outside the frame". So `core/tests/geometry_extent.rs` convicts a
  configuration against **its own repair**, by name — which does work, and is what a content pass
  actually asks. Repairing the two frozen defects moves the measure `0.4975` and `0.7788`, against
  the `0.055` pixel coverage had and could not use. See `docs/capturing.md` for what it cannot see.
- **Raised:** 2026-08-03, at [Plan 0058](plans/done/0058-the-gate-can-see-an-empty-frame.md)'s
  close. This is not a fresh idea —
  [ADR-0067](adrs/0067-coverage-measures-the-scene-not-the-backdrop.md) named it in Alternatives,
  rejected it as the *primary* mechanism and kept it explicitly as the supplement. What is new is
  the evidence that it is now wanted, and the evidence is a measurement rather than an argument.
- **Verified against code:** yes — measured through the instrument Plan 0058 Phase 3 built, printed
  by `core/tests/sanity.rs` on every run.
- **Lane:** `architect` → `dev`. Engine/harness work, no preset content.

**What Plan 0058 established, and what it could not.** Phase 1 made `sanity` measure the scene
against black instead of against a sampled corner pixel, which catches the **total** case — a figure
so far out of frame that nothing is drawn. That is real and it is pinned by a frozen fixture. Phase 3
then tried to catch the **partial** case with a stimulus-relative check: capture at two excitations
and assert the louder frame does not draw less picture. It ships as a **report, not a gate**, and
the numbers are why:

```text
 ratio   cov@0.4  cov@1.0  preset
 0.8552   0.2878   0.2461  De Jong          <- lowest legitimate (correct content)
 0.9568   0.3164   0.3027  Leviathan        <- correct content
 1.0514   0.3866   0.4065  Spectrum Corona  <- OVER-SCALED, scale = 5.20
 1.0891   0.5088   0.5541  Spectrum Comb    <- OVER-SCALED, scale = 3.80
    inf   0.0000   0.0000  Spectrum Ridge (pre-repair)  <- 0/0, no denominator
```

**No threshold on this axis convicts anything it was built for.** The two over-scaled presets score
*above* 1.0 — they draw more when loud — because a comb roots every bar on a shared baseline and a
corona roots every spoke at a centre, so clipping the tips costs a rounding error of lit pixels
while the body stays exactly where it was. Meanwhile the only content anywhere near a plausible
threshold is correct: the attractor family's *peak buys structure* idiom, which
[ADR-0062](adrs/0062-clamp-occupancy-is-the-saturation-instrument.md) already records as real. A
gate at `0.80` would sit `0.055` from De Jong while catching none of the three known-defective
configurations.

**So the diagnosis is that the measure is wrong, not the threshold.** Tips are almost no pixels.
Asking a pixel-coverage statistic about a figure that overshoots its frame is asking the wrong
question, and no calibration of it will help.

**What a design here would weigh.**

- **The obvious mechanism, and its reach.** Line and spectrum scenes build a CPU segment list, so
  "what share of the drawn geometry lands inside the render target" is computable without a GPU
  readback for exactly the families most exposed. That is also its limit: `fragment_field`,
  `reaction_diffusion` and the attractor draw no such list, so this cannot be an engine-wide gate —
  it is a per-family instrument, which is a shape this project has not built before.
- **Where it lives.** ADR-0067 declined it partly because it "needs a `Scene`-adjacent accessor",
  and widening the `Scene` trait is ADR-0002 territory. Whether the fraction is computed inside the
  scene, exposed through a diagnostic seam, or derived by the harness from the same generator config
  a preset declares is the real decision, with rejected alternatives.
- **The confirmation half already works and is worth keeping either way.** Repaired, the same ratio
  moved `1.0891 -> 1.7196` (comb) and `1.0514 -> 1.6756` (corona). The check is blind as a
  conviction and sharp as a confirmation — useful to a content pass verifying its own repair even
  though it can never fail the build.
- **Non-vacuity is already available.** `core/tests/sanity.rs` carries `pre_repair_spectrum_ridge`
  as a frozen fixture, and `git show 2efb80e^:presets/spectrum_comb.toml` is the partial case. Any
  instrument proposed here can be tested against both before it is trusted.

---

## 0040 — additive light occludes by geometry, so a dim figure over a lit backdrop reads as dark speckle

- **CLOSED 2026-08-09** at [Plan 0071](plans/done/0071-light-that-adds-without-covering.md)'s close ceremony. `occlude` shipped as a bindable scalar
  at the backdrop composite and **the default stayed at `1.0`**, decided by the user in the running
  app over a lit backdrop at 0.35 and 0.60 rather than by the argument below. So the entry got its
  mechanism and its samples, and the answer to "is coverage the right model" was *yes, keep it, and
  make the exception reachable*. The entry's question therefore closes; what it opened does not —
  the retune it invites is Plan 0071 Phase 5, still outstanding and grouped with
  [0038](design-backlog.md#0038--mid-tone-dominated-presets-lost-8--luminance-to-the-tonemap-knee-and-the-library-has-not-been-retuned).
  Three claims in ADR-0085 were falsified by the implementation and are recorded in its Outcome.

- **PROMOTED 2026-08-04 → [ADR-0085](adrs/0085-how-much-a-scene-occludes-the-backdrop-is-one-number.md) +
  [Plan 0071](plans/done/0071-light-that-adds-without-covering.md)** — `occlude`, a bindable scalar at
  the backdrop composite, so the resolve is `scene + bg * (1 - alpha * occlude)`. The entry's
  "per-preset opt-in" shape was taken, as a **continuous** scalar rather than the two-valued enum it
  suggested: the scalar contains both endpoints at identical cost and avoids a fourth quantization
  seam. Default `1.0` is byte-identical to today; whether it stays there is Phase 3's `human` call
  from a rendered sample set over a **lit** backdrop, per this entry's own instruction that the
  question wants samples rather than an argument.
- **Raised:** 2026-08-01, from `architect`, at Plan 0051's Mode 4 review.
- **Verified against code:** yes — rendered. `swarm_storm` over `bg_bright = 0.35` at
  `brightness = 0.02` renders as black specks on the backdrop; at the shipped-value backdrop the
  same run's darkest pixel is (71,13,22) against a backdrop of (138,67,56).
- **For:** `architect` (a look decision, ADR-worthy if taken), informed by `preset-author`.

[ADR-0056](adrs/0056-additive-scenes-emit-premultiplied-alpha.md) made a scene emit alpha equal to
its **coverage**, which fixed the black notches and rims. Its last Negative bullet left one thing
open: coverage-as-alpha means a fragment occludes the backdrop **whatever light it emits**. The
resolve is `c * g + bg * (1 - g)`, so a fragment darkens the backdrop wherever `c < bg`.

At the shipped near-black floors this is unobservable — all sixteen affected presets sit between
`bg_bright` 0.009 and 0.070. It matters because **the fix invites raising them**: the black rim was
the reason the swarm and line families were floored (`lsystem_fern.toml:98-103` records the
symptom, misattributing it to the lifted floor washing out the additive halo — a real effect, but
the rim was contributing and the tradeoff has changed). An author acting on that invitation meets a
new ceiling: `bg_bright` can rise only to the **dimmest emitted luminance in the figure**. Past it,
the depth-parallaxed far particles and the `glow`-dimmed strokes stop fading out and start reading
as dark speckle. `presets/README.md` now states this.

**The open question is whether that is the right model.** An additive look arguably wants *no*
occlusion — light adds, it does not cover — which would mean deriving the seam's alpha from
something other than pure geometric coverage, or giving the scene a bindable choice between the two
semantics. ADR-0056 rejected deriving coverage centrally from luminance (a legitimately dark
covered pixel would go transparent, and it puts scene judgement in a shared pass); a per-scene or
per-preset *opt-in* is a different proposal and is not covered by that rejection. Nothing is broken
today — post-fix is brighter than pre-fix at every pixel — so this is a look decision, not a defect.
Settling it wants rendered samples of the same preset under both semantics at a raised backdrop,
per the concrete-examples workflow, not an argument.

---

# Archived 2026-08-13 — the second sweep

Nineteen entries, moved verbatim from the live file at the 2026-08-13 backlog sweep. Every one
had been discharged — most carried their own `CLOSED` / `DELIVERED` marker and had simply never
been moved. The live file's ledger indexes them; read the bodies here before acting on the same
surface.

---

## 0009 — the `animation.rs` gate penalizes two legitimate designs (informational)

- **Raised:** 2026-07-26, from `preset-author`, explicitly **not** as an argument that the gate should
  change.
- `core/tests/animation.rs` renders at 96x96 and `ANIM_FLOOR` is a whole-frame diff. Two legitimate
  looks fight it: a **rotationally symmetric** figure (Star Rosette's ring) is nearly invariant under
  rotation, so no amount of spin registers as animation and it must move radially instead; and a
  **thin-stroke** figure nearly vanishes at 96 px, so its motion measures near zero even when it is
  clearly animated at 2048.

Both are real preset-authoring constraints imposed by the **test resolution** rather than by the
look. The failure mode is non-obvious and cost several iterations to diagnose. **Captured so the next
author does not re-diagnose it** — the cheap resolution is a sentence in the authoring docs, not a
change to the gate.

### Update 2026-08-04 — the gate has now rejected a preset, and there is a number

- **Raised by:** `preset-author`, landing `emitter_squall` (`f6c56dc`). The entry above was filed
  explicitly *not* as an argument that the gate should change. This update does not make that
  argument either, but it does retire the phrase "informational": the gate is now shaping shipped
  content rather than only measuring it.
- **ROUTED 2026-08-04 → [Plan 0067](plans/done/0067-the-curation-route.md) Phase 1d**, as a bounded
  measurement rather than a redesign. The user's call, from the two options put to them: raise the
  gate's resolution and re-baseline the floor if the measurement supports it, rather than design a
  coverage-aware successor statistic.
- **MEASURED 2026-08-09, and the answer is negative — which sharpens this entry rather than closing
  it.** Phase 1d rendered both cases plus a static control at 96 / 192 / 384. **The ladder is flat.**
  `frame_diff` scores **occupancy**, and occupancy is scale-invariant, so no resolution separates
  *sparse but moving* from *static*. `ANIM_FLOOR` and `SIZE` did not move; the ladder ships
  `#[ignore]`d as a recorded measurement, at zero CI cost. The route the user chose is therefore
  **closed off**, and what remains is the alternative that was explicitly not taken: this gate needs
  a **coverage-aware statistic**, not a bigger render. That question is now *earned* rather than
  speculative, and it is the live form of this entry.

**The measurement, from the preset's own header.** Squall's second draft was the shipped geometry at
**a fifth of the density** — "far better looking, individual parabolas, lots of dark" — and it
failed: `anim` **0.005** against `ANIM_FLOOR = 0.01`, with three of four reactivity bands under
0.02. The shipped density is the lowest that clears both with margin, measured rather than guessed:
`anim` **0.018**, bands 0.025 / 0.018 / 0.032 / 0.023. So the gate did not reject a broken preset —
it rejected the better-looking one of two, and the author raised density by 5x to pass.

**One of the two cases is provably not a resolution problem, and the plan phase says so.** A figure
invariant under rotation by `2*pi/k` produces an **identical image** under that rotation, so its
whole-frame difference is zero at *every* resolution — Star Rosette's spinning ring cannot be
rescued by rendering it larger. The thin-stroke / sparse case is the one where resolution plausibly
helps, and even there it is not obvious: a mark smaller than a pixel at 96x96 is lost or aliased
rather than area-averaged, so whether the statistic separates a sparse-but-moving frame from a
static one is an empirical question. **That is why the phase measures a ladder before it moves a
constant.** The non-vacuity probe is free: the rejected draft is the shipped `emitter_squall` with
`spawn_rate` cut to a fifth.

**What this does not become.** An argument that `ANIM_FLOOR` should be lowered. A floor that a
genuinely static preset can clear is worth nothing, and the shipped Squall sits at 1.8x the current
floor, so the headroom is not large enough to give away blind.

### Update 2026-08-11 — the Plan 0075 cohorts add a second casualty, and this one was routed out rather than tuned up

- **Raised by:** `preset-author`, Plan 0075 cohort 4. A QUIET twinkling starfield — sparse marks,
  low coverage, slow shimmer (the "Perseids' quiet sky" look) — was **routed out of the cohort
  rather than shipped**, because the coverage/animation floors at 96x96 make a legitimately
  sparse, slow look ungateable. (The other half of that casualty — a swarm mark has no per-mark
  variation to twinkle — is
  [0068](#0068--a-swarm-mark-has-no-per-mark-variation-so-the-only-scene-that-can-hold-a-starfield-cannot-make-one-twinkle)'s,
  re-raised the same day.)
- The earned question above now has **two named casualties of two different severities**:
  `emitter_squall` shipped at 5x the density its author preferred (the gate shaped it), and
  Perseids did not ship at all (the gate priced the look out). The first was bent; the second
  was lost.
- **Handoff verdict (2026-08-11): promote** — the coverage-aware statistic, jointly with
  [0068](#0068--a-swarm-mark-has-no-per-mark-variation-so-the-only-scene-that-can-hold-a-starfield-cannot-make-one-twinkle)
  as the sparse-idiom pair: one look class, two walls, and fixing either without the other
  leaves the look unreachable.
- **PROMOTED same day → [ADR-0091](adrs/0091-the-animation-gate-scores-motion-against-the-figures-footprint.md) +
  [Plan 0077](plans/done/0077-the-quiet-sky.md) Phase 1** (queued after Plan 0076 + cohort 6).
  The rotational-symmetry case stays out of scope by arithmetic and remains this entry's
  documented authoring constraint — the promotion covers the sparse half only.
- **DELIVERED 2026-08-12 (Plan 0077 Phase 1, `698b734`).** The gate scores
  `metrics::footprint_diff` — motion over the union of lit pixels, backdrops stripped — and
  the entry's own casualty is the standing proof: the rejected fifth-density Squall draft
  **passes at 0.1049** (the whole-frame statistic read it 0.0057), the static control keeps
  failing on a zero numerator, both pinned as a non-vacuity test, and the whole-library
  re-sweep convicted nothing. The statistic half of this entry is closed;
  [ADR-0091's Outcome](adrs/0091-the-animation-gate-scores-motion-against-the-figures-footprint.md#outcome--2026-08-12-at-plan-0077s-close)
  carries the details. The rotational-symmetry case stays what it always was — a documented
  authoring constraint no image-domain statistic can lift.

---

---

## 0055 — the attractor's shape vocabulary is "breathe and bend", and the reference figures ask for more

- **Raised:** 2026-08-04, by the user, watching the attractor family in the app after Plan 0059
  Phases 1/1b/2/3 landed. Reference: a Google Images sweep for **`de jong strange attractor`**
  (the query, not the session URL — the URL carries per-session tokens and will not resolve later).
- **Verified against code:** yes — `core/src/render/scenes/particles/mod.rs`, and the four
  coefficient bindings in each of the six `presets/attractor_*.toml`.
- **Not a defect.** Everything below already works as designed. This is a capability question.

**What exists today.** The attractor's shape *is* programmatically drivable: `a`/`b`/`c`/`d` are
named bindable params carrying the family's coefficients, and all six shipped presets already
steer them — slow incommensurate sines for drift plus clamped band terms on top. Plan 0059 added
two more shape levers beside them: `[particles] density` (how many trajectories) and, on the
continuous families, the `prev -> pos` segment that turns a sparse cloud into legible curves.

**Where it stops.** These are **chaotic** maps, and `presets/README.md` already tells authors to
move the coefficients "slowly and by a little" for a concrete reason: a nearby coefficient can have
a *completely different* attractor, so past a small step the figure **cuts** rather than morphs.
The vocabulary is therefore "breathe and bend around one figure", and the reference images the user
is comparing against are a *gallery of different figures* — De Jong at widely separated coefficient
tuples, each its own shape. Nothing in the current surface walks between them without cutting.

**What a design here would weigh.**

- **Is the ask variety-over-time or morph-between-figures?** They are different features. Variety
  could be had by re-seeding to a new coefficient tuple on a section change and accepting the cut
  (cheap, no engine work, possibly hidden by the existing dissolve). A genuine *morph* needs a path
  through coefficient space along which the attractor stays recognisable, which is a real research
  question and may not exist in general.
- **A curated tuple roster is the cheap middle.** The reference galleries are, in effect, a list of
  known-good `(a,b,c,d)` per family. A preset-facing roster — pick tuple N, or walk a named path
  between two — buys most of the visual variety without solving continuous morphing. It also has a
  clear rejected alternative (free coefficient binding, which is what exists and cuts), so it is
  ADR-shaped.
- **Cross-fading two attractors is the other shape**, and it is expensive in the way this engine
  already understands: two particle buffers and two dispatches, against a scene that is already the
  heaviest in the library. ADR-0024's cross-preset transition may make this unnecessary — a
  dissolve between two attractor *presets* is a morph between two figures, at zero new engine cost.
  Worth measuring before building anything.
- **Watch the interaction with `density` and the streak.** Both landed in Plan 0059 and neither has
  had a content pass yet (Phase 4). The reference look the user is pointing at is *sparse curves*,
  which is exactly what those two levers exist to reach — so some of this gap may close in Phase 4
  without any new capability. **Re-check this entry after Phase 4 before designing against it.**

### Update 2026-08-11 — cohort 5 answers the re-check, and the concrete mechanism is per-tuple framing

Plan 0075's own Risks asked for this entry to be re-checked before the brief, because the
Plan 0059 Phase 4 levers might have closed the gap. Cohort 5 is the answer: the levers deliver
the breathe-and-bend vocabulary as designed, and the remaining wall is not coefficient freedom —
it is **framing**. A Lorenz at rho ≈ 100 (the torus-knot regime) was considered for the cohort
and is unreachable: `AttractorFamily::projection()` / `seed_box()` are **per-family constants
sized to the canonical tuple**, so a wild tuple renders off-centre and out of frame with no
preset-side recovery (`pan` cannot span it). Cohort 5 shipped unheld IFS figures instead.

This sharpens the entry's own "curated tuple roster is the cheap middle": the roster needs
**per-tuple projection and seed boxes**, not just a list of coefficients — a tuple is only
reachable with its framing carried beside it.

**Handoff verdict (2026-08-11): promote.** The entry already names the rejected alternative
(free coefficient binding, which exists and cuts), the mechanism is now concrete, and a cohort
demonstrably shipped around the gap. ADR-shaped and earned.

**PROMOTED same day → [ADR-0093](adrs/0093-attractor-tuples-are-content-with-per-tuple-framing.md) +
[Plan 0079](plans/done/0079-the-attractor-learns-new-figures.md)** — the roster **plus** measured
morph paths, the stronger form, user-decided by interview (zero surviving paths is a recorded
outcome, not a failure). Queued after Plan 0076 + cohort 6, last of the three handoff plans.

**CLOSED 2026-08-13, at Plan 0079's close, and closed in *full* — both halves.** The entry's own
framing was "is the ask variety-over-time or morph-between-figures? They are different features",
with the morph half called "a real research question [that] may not exist in general". Both
shipped. **Variety:** a curated roster per map family — `de_jong` 13, `clifford` 13, `thomas` 13,
`lorenz` 12 — each entry carrying its coefficients *and its measured framing*, selected by a
CPU-quantized `tuple` param. The rho ≈ 100 Lorenz that cohort 5 measured as physically unreachable
now renders centred and in frame, which is the wall this entry named coming down. **Morph:** four
measured paths ship, driven by `morph` along a structural `tuple_from`/`tuple_to` pair, after a
twenty-pair sweep in which four pairs were refused *by measurement* (a tuple partway between two
others can collapse to a fixed point) and four were judged in motion and kept. The research
question turned out to have an answer where a roster walks a **single** coefficient — Thomas's `a`,
Lorenz's `rho` — because there neighbouring entries are neighbouring *figures*; the discrete maps
are the harder case in both directions. The entry's own "cross-fading two attractors is the other
shape" stays rejected (ADR-0093 Alternative B), unmeasured and now unneeded.

---

---

## Entry 0056 — from the Plan 0050 close (2026-08-04), found while clearing a stale preset cache

---

---

## 0056 — a user-authored preset has been living outside the repo for six weeks, and it is a curation candidate the boundary has no route for

- **PROMOTED 2026-08-04 → [ADR-0081](adrs/0081-the-content-lane-lands-presets-and-architect-curates-the-set.md) +
  [Plan 0067](plans/done/0067-the-curation-route.md)** — both halves the entry asks for. The owed
  ADR-0017 supplement is written and the boundary moves: **`preset-author` lands presets directly,
  `architect` curates the set**, with the curation pass hooked to the close of any plan that touched
  `presets/` (a standing cadence was rejected on this project's own evidence — the version bump).
  The plan strengthens the gate that authorization rests on *before* walking this file through it,
  because all five preset gates synthesize `AnalysisFrame` and none runs the analyzer. Phase 3 is
  the Coral Oracle pass, and declining it is a successful outcome.
- **Raised:** 2026-08-04, by `architect`, checking whether a `%APPDATA%` preset cache was safe to
  delete before clearing it at the user's request.
- **Verified against code:** partly — the two defects below are read off the file against today's
  grammar and are **not** rendered. Render it before acting.
- **Not a defect in the engine.** This is a content-lifecycle gap plus two rot findings in one file.

**What was found.** `%APPDATA%\light-music-visualizer\presets\chthonic_coral_oracle.toml` — "Chthonic
Coral Oracle", a reaction-diffusion preset — has **never been tracked in git**. It survived the cache
clear deliberately; the other 43 files were retired or stale shipped copies, all recoverable from
`c11bbf9` / `de707cb`. It is the only non-shipped preset in the user's library.

**It is not a stray.** This is the preset that raised [backlog 0001](design-backlog-archive.md#0001--reaction_diffusion-reaches-only-2-of-the-5-plan-0018-composite-levers)
on 2026-07-24 — the entry that became [ADR-0026](adrs/0026-full-composite-coverage-fullscreen-scenes.md)
and [Plan 0025](plans/done/0025-full-composite-coverage.md), i.e. the reason `reaction_diffusion`
reaches the composite levers at all. The preset that motivated a whole plan then **never came back
into the repo**, and the levers it asked for landed without the look that asked for them ever being
shipped or re-checked.

**Why it is a candidate.** It composes four things into one coherent idea — Pearson-regime drift on
bass, beat-stamped `inject` growth, trails, and a bass-breathing kaleidoscopic fold — and documents
its own drive map in the header more thoroughly than several shipped presets do. Nothing in the
shipped set does regime drift.

**Two rot findings, both from the file rather than from a render.**

- **`bar` is no longer a variable.** `kaleido_angle = "time * 0.06 + bar * 0.5"` was written against
  the pre-[0048] grammar; since [ADR-0050](adrs/0050-downbeat-and-phrase-tracking-with-confidence-fallback.md) the trio is
  `bar_index` / `bar_phase` / `beat_in_bar`. Expect a load-time unknown-name **warning** and a dead
  term, not a failure. Note also what [0048] Phase 6 measured — the downbeat estimator locks on
  **3.1 %** of audible time — so whatever this term becomes should be built on `beat_index` /
  `time_since_beat` rather than on the bar trio.
- **`kaleido_order` is eased under `[smoothing]` at `tau = 2.0`.** That is the seam
  `presets/README.md` records: an eased param is continuous even where its math needs an integer, so
  a breathing wedge count sweeps through invalid values between them. Whether it reads as jitter or
  as intended breathing is a **render** question, and this entry does not answer it.

**What is actually being asked.** Not "ship this file". Two things:

1. **A `preset-author` pass** to refresh it against the v2 grammar, judge it in motion, and decide
   whether it earns a place — including whether the regime-drift idea is better carried by a new
   preset than by restoring this one.
2. **The route itself, which does not exist.** [ADR-0017](adrs/0017-preset-author-skill-lane.md) put
   curation at "`dev` embeds a curated preset" because embedding meant editing Rust in two coupled
   spots. [ADR-0022](adrs/0022-build-time-preset-embedding.md) removed that premise — `core/build.rs`
   globs `presets/*.toml`, so shipping a preset is committing a `.toml`. The architect skill already
   names this as an **owed ADR-0017 supplement**; this is the first concrete case waiting on it, and
   it is a good one to decide against, because the file is a real candidate with real rot rather
   than a hypothetical.

**Do not fold this into Plan 0059 Phase 4.** That pass is the attractor family, judged against a
figure that just changed shape; this is one reaction-diffusion preset and an unrelated boundary
question. They share only the lane.

---

---

## Entries 0059-0060 — from the `preset-author` handoff of 2026-08-04, after Squall and the fold-edge pass

Five findings were handed over; three were record corrections (the `attractor_dejong` misnaming, the
`distinctness` family list, the [0009](#0009--the-animationrs-gate-penalizes-two-legitimate-designs-informational)
update above) and are resolved where they belong. These two are design.

---

---

## 0059 — the backdrop is the one surface left that does not colour through the shared palette, and nothing says so

- **CLOSED 2026-08-09 — [Plan 0072](plans/done/0072-the-backdrop-joins-the-palette.md) landed.** The
  backdrop samples the preset's baked LUT, `background.rs` carries no cosine copy, and
  `saturation` / `palette_mix` reach the sky through one binding. `docs/preset-palettes.md` has its
  own backdrop section and the roster row; `presets/README.md`'s sentence is true rather than
  corrected. Three of the sixteen in-scope presets were re-tuned. **Everything below is the entry as
  raised** and describes the pre-plan engine — read it as history, not as behaviour.
- **PROMOTED 2026-08-04 → [ADR-0086](adrs/0086-the-backdrop-colours-through-the-preset-palette.md) +
  [Plan 0072](plans/done/0072-the-backdrop-joins-the-palette.md)** — same day it was raised, because the
  entry's own verification turned a documentation gap into a coherence gap. The doc half is not
  waiting for the plan: `docs/preset-palettes.md` gains the backdrop now, describing today's
  behaviour, and `presets/README.md`'s sentence is corrected.
- **Raised:** 2026-08-04, from `preset-author` — as "`bg_hue` is undocumented anywhere", with a
  measured six-row swatch table put in the preset's own header for want of a home.
- **Verified against code:** yes, and the finding got larger under verification.

**The reported half is true.** `docs/preset-palettes.md` — the document that owns the colour surface
and carries a twenty-row swatch table for the line scenes' ramp — does not contain the string
`bg_hue`. `presets/README.md` has one sentence.

**That sentence is wrong, which is the larger half.** It reads: *"`bg_hue` offsets into the shared
cosine palette."* There is no sharing. `core/src/render/background.rs:70` carries its **own copy** of
the iq cosine inline in its WGSL — `d = vec3(0.10, 0.42, 0.62)` — and the pass binds one uniform and
no LUT texture at all (`PARAMS` is `bg_hue`, `bg_bright`, `bg_vignette`; the bind group is
`@group(0) @binding(0) var<uniform>` and nothing else). So:

- **`[palette]` does not reach the backdrop.** An `ember` preset draws an ember figure over a
  *spectrum-cosine* backdrop. `attractor_clifford`'s crimson→ember→white-hot custom gradient does
  not tint its own sky.
- **Neither do `saturation` or `palette_mix`.** The A/B crossfade moves the scene and leaves the
  backdrop where it was.
- **The author's measured table is the ramp that is already documented.** `d = (0.10, 0.42, 0.62)`
  is byte-identical to the built-in `spectrum` gradient (`palette.rs:109`) and to the line scenes'
  hardcoded default (`lines/mod.rs:160`), so the six points measured for `bg_hue` land on the
  existing twenty-row table (0.30 blue = `#57ABF8` cornflower, 0.45 teal = `#2BECFA` aqua, 0.85
  amber = `#FCB118` amber). The right doc fix is therefore **one sentence pointing at that table**
  plus the exclusion above — not a second table that can drift from it, which is how backlog 0014
  got its colour names wrong.

**Why it is the last one.** ADR-0021 unified the fragment field and swarm; Plan 0020 Phase 5 pulled
reaction-diffusion onto `spectrum` and re-blessed its baseline; the attractor followed; Plan 0054 /
ADR-0059 brought the four line families in. The backdrop was named in ADR-0021's *Context* — "ADR-0018
adds a *background* colour (`bg_hue`), not a *scene* palette" — as a reason it was a different
feature, and it has never been revisited since every other surface converged.

**Scope, measured before the ADR was written.** 26 of 37 shipped presets bind `bg_bright > 0`. Eleven
declare no `[palette]`, so their gradient already *is* `spectrum` and they cannot move. Fifteen
declare one and would re-tint. Every one of those fifteen sits at `bg_bright <= 0.039` including its
audio term, so this is a dim wash rather than a repaint — but `bg_hue` values were picked as
positions in the cosine, and the same number means a different colour in a custom gradient, so it is
a re-tune and not a no-op. Plan 0072 owns that pass.

---

---

## 0060 — an engine fix leaves its preset-side workarounds standing, and only a header comment remembers them

- **Raised:** 2026-08-04, from `preset-author`, at the fold-edge content pass — as a *pattern*
  rather than a defect, with two instances and a specific ask.
- **ROUTED 2026-08-04 → [Plan 0067](plans/done/0067-the-curation-route.md) Phase 4**, which is already
  installing a close-ceremony duty hooked to "this plan touched `presets/`". This is a second
  trigger on the same step: "this plan fixed something a preset could have been framed around".
- **Verified against code:** yes — both instances are readable in the shipped files today.

**The pattern.** An engine defect gets worked around in **preset framing** — a zoom pinned, a fold
switched off, a density cut. The workaround is recorded honestly, in the preset's header comment,
because that is the only place it can be recorded. Then the engine defect is fixed by a later plan,
and the workaround stays: it is not a bug, nothing fails, no gate fires, and the file still renders.
The only artefact that knows the pin exists is a comment nobody has a reason to open.

**Instance 1 — `attractor_leviathan`.** `zoom` pinned at base 0.72, with a header saying the pin was
"a fold constraint, not a taste": the figure was held inside the fold's inscribed disc so it could not
feed the falloff's residual rays. ADR-0061 made the edge a per-preset choice; the pin lifted to 1.80.
**Two edits, not one** — the second only findable from the comment.

**Instance 2 — `attractor_clifford`.** The identical shape, found second and only because Leviathan
had taught the lane to look: framing cut from 1.10 base / 1.42 peak to 0.66 / 0.94 purely to keep the
ribbon's tips off the frame edge, with the note ending *"the general fix is a per-preset edge
treatment — Plan 0055, approved, not built."* It was built a plan later and the framing stayed cut,
so the user's report was that the preset was too small to show any difference between the treatments
— which was true, and was the pin talking.

**A third, already recorded, which is what makes it a pattern rather than a coincidence.**
`swarm_dense` pinned `kaleido_order = 1` (the fold off) to dodge backlog 0010's clamped-edge smear.
ADR-0047 fixed that smear; the pin outlived it by a plan, and its comment was *stale twice over* by
the time anyone read it. Three instances, three engine fixes, three files that kept paying.

**The ask, and it is cheap.** When a plan fixes an engine defect, grep `presets/` for headers citing
it before the plan closes. The workarounds are **greppable by construction** — this project's preset
headers name the ADR, the plan, or the backlog entry they are dodging (`design-backlog 0010`,
`Plan 0055, approved, not built`, `ADR-0065`), which is exactly what makes the sweep a one-line
search rather than a re-read of the library.

**What it is not.** A request that engine plans re-tune presets. The sweep's output is a *list* — the
judgement is content work and stays in the content lane. And the pins were right when they were
written: none of the three is an error, which is the point. A correct workaround for a defect that no
longer exists is invisible to every instrument this project has, because nothing is wrong.

---

---

## Entries 0061-0063 — from the Plan 0063 Phase 5 content pass (2026-08-04)

The pass that judged ADR-0076's four new attractor levers in motion and re-tuned `attractor_lorenz`
and `attractor_thomas` against them. The plan asked five questions; two of the answers needed no
follow-up and are now in the two presets' headers, and three findings are design signal.

**The two that needed nothing.** *Does perspective alone resolve the rotation, or does it need the
haze?* — **perspective alone, decisively.** Rendered as a five-way at fixed audio: with `depth_fade`
and `depth_hue` both on and `perspective` at 0, the butterfly is exactly as flat as it ever was —
both wings the same size, a symmetric bowtie, the haze reading as an uneven exposure rather than as
distance. Perspective alone reads as a solid at an angle immediately. The haze reinforces and cannot
substitute, which is worth knowing because it is the cue that *sounds* like the depth cue. And *at
what `density` does the no-occlusion limit bite?* — measured on the ladder 0.002 / 0.01 / 0.05 / 0.2
/ 1.0 with all three cues on: **reads, reads, markedly weaker, gone, gone.** The usable ceiling for a
preset that wants depth is about `0.01`; ADR-0044's warning is correct and the number is an order of
magnitude below the full budget. Both shipped 3-D presets (0.002 and 0.02) sit at or inside it.

---

---

## 0061 — `perspective` moves the figure far more than it enlarges it, so the documented way to recover the framing does not work

- **PROMOTED 2026-08-09 → [Plan 0075](plans/done/0075-the-content-renaissance.md) Phase 3** (with
  [ADR-0089](adrs/0089-the-library-renews-by-replacement-cohorts.md)'s renaissance) — the
  documentation option, which this entry's own Priority section says is cheap enough to do
  without a plan; it rides the renaissance's pre-flight because the rebuild multiplies the cost
  of every doc that points authors the wrong way. ADR-0076's Outcome section is added by
  `architect` at that plan's close.
- **Raised:** 2026-08-04, from `preset-author`, during the Plan 0063 Phase 5 re-tune.
- **Verified against code and by measurement:** yes, and the finding is larger than the documented
  behaviour it corrects.

[ADR-0076](adrs/0076-the-attractor-keeps-the-depth-it-already-computes.md) and `presets/README.md`
both describe one framing consequence of `perspective`: the magnification is applied before the view
transform, so pushing it up makes the figure larger as well as deeper, and *"recovering the framing
is a `zoom` edit"*.

**That is true, and it is the small half.** The dominant effect is not that the figure grows — it is
that the figure **moves**, by an amount that varies with the spin phase. The near side is magnified,
so the projected centroid shifts toward whichever side is currently near; as the figure turns, that
shift **orbits**. Measured peak-to-peak over four spin phases on a bare Lorenz at fixed audio,
600 px square:

| `perspective` | centre-x swing | widest span |
|---|---|---|
| 0.00 | 0.04 NDC | 522 px |
| 0.15 | 0.11 NDC | 525 px |
| 0.25 | 0.20 NDC | 529 px |
| 0.40 | 0.37 NDC | 542 px |
| 0.60 | 0.55 NDC | 555 px |

The swing runs about **0.9 x `perspective`** in NDC. The size growth across that entire sweep is
**6 %**. So the translation is roughly an order of magnitude more consequential for composition than
the enlargement the docs name — and **a `zoom` edit cannot recover it**, because a zoom is a static
scale and this is a phase-varying translation. All a zoom can do is shrink the figure until the orbit
fits inside the frame, which is what both re-tuned presets now pay: `attractor_lorenz` went from a
1.32 base to 1.16 and `attractor_thomas` from 1.14 to 1.02, and both lost real presence for it
(Lorenz's `sanity` coverage now reads 0.2747 against a 0.18 floor — still passing, with less room
than before).

**The practical consequence is a ceiling nobody documented.** The clamp sits at `0.8` and the
projection is optically fine there — it is a true perspective divide, straight lines stay straight,
and the butterfly at 0.8 reads as a strong wide angle rather than as a fisheye. But past about
**0.3** the figure visibly slides around the frame instead of turning in place, which is a worse
artifact than the flatness the parameter was bought to fix. The usable range is roughly the bottom
third of the legal one, and nothing says so.

### What a fix would be

Three options, in increasing cost, and this entry does not pick one:

1. **Documentation only.** Correct the ADR's and README's framing note to say the effect is chiefly
   translational, quote the ~0.9x law, and give the ~0.3 practical ceiling. Cheapest, and it stops
   the next author rediscovering this with a zoom ladder.
2. **Re-centre the projection on the figure's projected centroid.** Removes the orbit and keeps the
   depth. This is **not** ADR-0076's already-noted followup — normalizing `m` by its value at
   `d_n = 0` divides every magnification by `m(0) = 1` and so changes nothing at all about the orbit;
   it only addresses the size growth. Re-centring is a different, larger change, and it needs a
   decision about *what* to centre on: the particle centroid is a per-frame readback, which the frame
   loop must not do, and a fixed per-family offset would be another table.
3. **Accept and expose it** — a lever letting a preset trade orbit for off-centre framing.

### Priority

**Medium, and the documentation half is cheap enough to do without a plan.** Nothing is broken — the
levers work and both presets ship using them — but the guidance that exists points an author at a fix
that cannot work, and the parameter's legal range is three times its usable one.

---

---

## 0062 — `depth_hue` is a *lightness* cue on a lightness ramp, it wraps at the ends, and it is structurally dead under `ink_amount`

- **PROMOTED 2026-08-09 → [Plan 0075](plans/done/0075-the-content-renaissance.md) Phase 3** — the
  documentation fix (the three regimes, the `2 * min(hue_center, 1 - hue_center)` wrap bound,
  the duotone deadness), in `presets/README.md` and `docs/preset-palettes.md`. The optional
  wrap clamp stays undecided, per this entry's own reasoning that the repeat is the LUT's
  documented behaviour everywhere else.
- **Raised:** 2026-08-04, from `preset-author`, during the Plan 0063 Phase 5 re-tune.
- **Verified by measurement:** yes, all three parts.

ADR-0076 introduces `depth_hue` as the cue that makes distance read as **distance** rather than as
dimness: *"a hue shift is what makes it read as distance — real atmospheric perspective moves colour
as well as contrast."* That reasoning is sound and the parameter delivers it — **conditionally**, and
all three conditions are invisible from the roster.

**1. It needs a palette that travels in hue.** `depth_hue` shifts the per-particle *palette
coordinate*, so what it does is whatever moving along that preset's ramp does. Rendered side by side
at `perspective = 0.5`: against `attractor_lorenz`'s shipped night-blue -> teal -> mint ->
solar-white ramp, a `depth_hue` of 0.4 reads as the near material getting **brighter** — a second
contrast lever pointing the same way as `depth_fade`, not an independent cue. Against a
constant-lightness hue-travel ramp (blue -> cyan -> gold -> orange -> rose) the identical figure at
the identical value puts a clear **cool cyan on the far wing and warm gold on the near one**, which
is the atmospheric reading the ADR describes. Both shipped 3-D presets use lightness ramps, because
that is what an additive glow scene is normally tuned for — so the parameter is at its weakest on
exactly the two presets it shipped for.

**2. It wraps, and the wrap can make far material look near.** The offset is `+/- depth_hue/2` on a
coordinate the LUT sampler **repeats**. `attractor_lorenz`'s `hue_center` runs as low as 0.13, so a
`depth_hue` much above 0.26 sends the far end negative, wraps it to the top of the ramp, and lands
the far material on the same bright mint as the near — the cue inverts into a collision. At
`depth_hue = 1.0` with `hue_center = 0.20` both ends of the depth range sample the *same* coordinate
(0.70, and -0.30 which wraps to 0.70).

**3. It is dead under `ink_amount = 1`.** The terminal remap keys on luminance and discards hue, so a
depth *tint* is exactly the cue an ink preset cannot show. This is the same trap the same file
already records for `saturation` (`attractor_thomas`'s `a` binding carries that story). Not quite
inert — that preset uses the `mono` palette, so shifting the coordinate moves lightness and the remap
does see it — but measured at `depth_hue = 0.4` it moves 42 % of pixels by a **mean of 2/255 and a
max of 42**, a rounding error against what `depth_fade` does deliberately on the same frame. It is
left unbound there.

### What a fix would be

Documentation, chiefly — this is a parameter whose behaviour is entirely legitimate and entirely
undiscoverable. `presets/README.md`'s new depth section and
[`docs/preset-palettes.md`](preset-palettes.md) should say: `depth_hue` reads as a hue cue only on a
ramp with hue travel at roughly constant lightness; on a dark->light ramp it duplicates `depth_fade`;
keep it under `2 * min(hue_center, 1 - hue_center)` or it wraps; and it is inert under a duotone
remap, like `saturation`. A clamp on the wrap would also be defensible, but the repeat is the LUT's
documented behaviour everywhere else in the colour surface, so making this one parameter special is a
decision rather than a fix.

### Priority

**Low-medium.** Nothing misbehaves; an author simply cannot find out which of three regimes they are
in without rendering a ladder, which is what this pass did.

---

---

## 0063 — `spin`'s usable ceiling is set by `fade`, not by taste, and the pair is undocumented

- **PROMOTED 2026-08-09 → [Plan 0075](plans/done/0075-the-content-renaissance.md) Phase 3** — the
  documentation fix: `spin` and `fade` are one look, the two measured ceilings, and the stale
  "2-4 is where it becomes legible" advice goes.
- **Raised:** 2026-08-04, from `preset-author`, during the Plan 0063 Phase 5 re-tune.
- **Verified by measurement:** yes.

`presets/README.md` documents `spin` as a rate multiplier on 0.18 rad/s and suggests **2-4** as
"where the rotation starts being legible". On the two presets it exists for, 2-4 is at or past the
point where the figure stops being legible at all — and the bound is not the spin, it is the
**trail**.

A frame of trail drawn while the figure turns is a frame of **rotational smear**. The accumulation
stops being a trace of the trajectory and becomes a set of concentric arcs, which destroys precisely
the volume `perspective` was added to buy. The ceiling is therefore a function of `fade`: with
`fade = 0.932` (~15 frames) the rendered ladder 1 / 2 / 3 / 5 / 8 goes **crisp, crisp, softening,
smeared, scribble**, so the usable peak is about **1.9**. `attractor_thomas` runs `fade = 0.955`
(~22 frames) and its ceiling is correspondingly lower, about **1.3**. The arithmetic agrees: holding
the smear under ~5 degrees needs `rate < 0.087 / (frames / 60)` rad/s.

**The related half — a wide `spin` binding reads as instability, not as drive.** A `1 + bass * 5`
range through `--signal dynamic:110` swings the figure through most of a revolution between
transients and the depth reading never settles; it reads as tumbling. The *integration* ADR-0076
specified is working and is what makes any of this usable — the figure accelerates rather than
snapping to a new angle, which was the failure mode the ADR predicted for the multiply form — but the
integration fixes the discontinuity, not the range. Both re-tuned presets use a narrow modulation
(Lorenz 1.0-1.8, Thomas 1.0-1.3).

### What a fix would be

Documentation. `presets/README.md`'s depth section should say that `spin` and `fade` are one look:
the smear mechanism, the two measured ceilings, and the rule that the ceiling falls as `fade` rises.
The current "2-4 is where it becomes legible" should go — it is right for a trail-free scene and
wrong for every attractor preset that ships.

### Priority

**Low**, and it is one paragraph. But the advice currently in the README points the wrong way, which
is the same shape as 0061: guidance that actively costs an author a ladder.

---

## Entries 0064-0066 — from the Plan 0062 Phase 7 content pass (2026-08-04)

Three findings from judging the new IFS family against real audio. **0064 is the only one that is
an engine defect**; the other two are documentation gaps, and both matter more than "documentation"
usually does here because the current absence points an author the *opposite* way from what the
family wants.

---

## 0064 — an IFS preset switch shows a hard-edged rectangle of noise for two thirds of a second, which is the artifact ADR-0066 already removed once

- **PROMOTED 2026-08-05 → [ADR-0087](adrs/0087-the-ifs-particle-carries-its-age-and-its-last-map.md) +
  [Plan 0073](plans/done/0073-the-fern-unfurls-and-colours-by-what-made-it.md)** — and the route taken is
  the *second* of the two this entry proposed, folded into a change that was happening anyway. The
  entry's cheaper interim (seed at the figure's own fixed points) is Plan 0073 **Phase 2**; its
  Phase 3 then goes further and makes the respawn **continuous**, so the population is never a
  uniform box at any moment and the rectangle has no instant in which to form. The plan also catches
  something this entry did not: `jitter_extent` is *derived* from `seed_box`, so collapsing that
  spread — the obvious reading of "fix the seed box" — would have made `reseed` silently inert
  across the whole family. What changes is what `seed()` writes, not the box.
- **Raised:** 2026-08-04, from `preset-author`, during the Plan 0062 Phase 7 content pass.
- **Verified by measurement:** yes — captures at 2 / 6 / 12 / 24 / 40 / 90 frames.

Plan 0062 predicted this and called it a *haze*: "the initial fill scatters particles over the
figure's bounding box, so they converge onto it over ~23 steps — 0.39 s at the fixed step, with the
trail carrying the haze roughly a second at `fade = 0.94`". The timing is right and the description
is not. It is not a haze — it is a **legible, hard-edged, axis-aligned rectangle**, which is a
materially worse thing to show.

Rendered at `fade = 0.88`, `density = 1.0`:

| frame | what is on screen |
|---|---|
| 2-6 | a solid noise slab the shape of the seed box; the figure is a faint ghost inside it |
| 12-24 | the fern has resolved, and a **hard rectangular edge** frames it, brighter than the interior |
| 40 | the rectangle is down to a faint corner shadow |
| 90 | clean |

**Why this matters more than the plan's framing suggests.** It is the *same artifact class*
ADR-0066 was written to remove. That decision's own words are that re-uploading the seed array
"*replaced* the cloud with a uniform box rather than scattering it… a uniform fill of an
axis-aligned box" — which is what a `reseed` visibly was, and why it was changed. The rectangle is
now back: not on a beat, but on **every preset switch into an IFS**.

And the timing is the worst available. Presets dissolve into each other over ~1 s (ADR-0024), so the
**entire dissolve happens while the rectangle is on screen**. What a viewer sees a switch reveal is
a rectangle of noise resolving into a plant, rather than a plant.

**Not authorable around.** Nothing on the preset surface reaches the seed box. `fade` shortens how
long the trail *carries* it but not the slab itself; `density` thins it without removing the edge.

### What a fix would be

The successor plan's **staggered respawn** — the one Plan 0062's "What this plan does NOT do"
already names — is the fix, and it removes this as a side effect of doing something else.

A cheaper interim, if the successor plan is far off: seed every particle **at the figure's own fixed
point** instead of across its bounding box. ADR-0075's own Notes already establish that any
contractive map's fixed point `(I - M)^-1 t` lies *on* the attractor, so an orbit started there is
on-figure at step 0 and there is no convergence transient to hide at all. That is a change to what
calls `IfsFigure::seed_box`, not to the parameterization — and the ADR wrote the property down
precisely because it is a consequence of the parameterization rather than of the successor plan.

### Priority

**Medium**, and higher than the plan assumed. It is not a subtle quality issue: it is a visible
rectangle on every entry into the family, and the family now ships two presets.

### Architect note (2026-08-05, Plan 0062 close review)

**Accepted, and the read is right on both counts** — it is the ADR-0066 artifact class, and the
dissolve timing is the part that makes it matter rather than the duration. Two corrections to the
interim before anyone implements it, and one sequencing call.

**The interim must change `seed()`, not `seed_box()`, and the reason is a coupling the note does
not name.** `AttractorFamily::jitter_extent` (`particles/mod.rs:334`) is *derived* from
`seed_box()` — it is `JITTER_FRACTION` of that spread, which is how one constant serves a map
bounded in `[-2, 2]` and a flow spanning `±26`. So returning a fixed point from `seed_box` makes the
kick zero and **`reseed` goes inert on the whole IFS family**, silently, in a family whose
`reseed` response ADR-0075 lists as one of its free wins. The note is right that the change belongs
at the call site; it should say *why*, or the next person takes the shorter route and breaks the
lever instead.

**Seed at four fixed points, not one.** Putting every particle on a single fixed point trades a
rectangle for 50 000 particles on one pixel for frame 0 — a different artifact, not no artifact.
Same cost and same call site: seed particle `i` at the fixed point of map `i mod MAPS`, **restricted
to the maps with `p > 0`**. All of them lie on the attractor (`A = ⋃ fᵢ(A)` with `A` closed, so each
`fᵢ`'s fixed point is the limit of `fᵢⁿ(x)` for `x ∈ A` and therefore in `A`), they are already
spread across the figure, and the fill is on-figure at step 0 with nothing to converge. The `p > 0`
restriction matters because a padded slot's fixed point is only on the attractor when the pad
duplicates a drawn map — true of all five curated tables today, and exactly the kind of thing that
stops being true when a sixth figure is added.

**Sequencing: do not spend a plan on this.** The successor plan (unfurl + per-map colour) removes it
as a side effect of the staggered respawn, and this is two lines inside that plan's first phase. The
successor is not written yet, so the interim stays live as an option — if the successor slips more
than a session or two, fold the four-fixed-points change into whatever next touches
`particles/mod.rs`. What is *not* acceptable is closing this by widening `seed_box`'s box or by
shortening the trail: neither reaches the edge, and both were tried on the reseed path before
ADR-0066 replaced the mechanism.

---

## ~~0065 — `morph` is a travel knob whose visible rate is steepest near zero, and nothing says so~~

> **Discharged 2026-08-05** (Plan 0062 close). The fix was documentation and landed in the same
> commit that raised it, `cf977f9`: `presets/README.md`'s IFS section now carries the measured
> table, "`morph` is a TRAVEL knob, not a little-life knob", and the spiral-as-poor-target finding.
> Recorded in [ADR-0075](adrs/0075-ifs-family-morphs-in-singular-value-space.md)'s Outcome. **One
> thing is still open and is a review minor, not a backlog entry:** the param table's own `morph`
> row still reads "every value between is a real figure" ~40 lines above the correction, which is
> what an author scanning the table reads.

- **RESOLVED 2026-08-05, by documentation** — `presets/README.md`'s IFS section now carries the
  measured table, the travel-not-life sentence and the spiral-is-a-poor-target note (`cf977f9`). The
  `morph` row of the param table, which still read "every value between is a real figure" some
  thirty lines above its own correction, now points down to it (`Plan 0073` Phase 5 caught that
  inconsistency; it is done, so that item of its doc sweep is already satisfied). Kept here rather
  than archived because the *shape* of the finding — a parameter whose visible rate is front-loaded
  and whose description implies otherwise — is the reusable part.
- **Raised:** 2026-08-04, from `preset-author`, during the Plan 0062 Phase 7 content pass.
- **Verified by measurement:** yes.

`presets/README.md` documents `morph` as "position from `family` to `morph_to`… every value between
is a real figure". That is true, and it reads as an invitation to use a small `morph` for a little
life. Measured on fern → dragon at a neutral frame, the lit width of the figure as a fraction of the
frame:

| `morph` | 0.00 | 0.05 | 0.10 | 0.15 |
|---|---|---|---|---|
| lit width | 0.248 | 0.448 | 0.572 | 0.584 |

**By 0.05 the fern is half again as wide and already reads as a curl rather than as a plant.** The
cause is in the tables and is general: the dragon's two maps are `0.707 * R(45 deg)` and
`R(135 deg)` against the fern's near-zero rotations, and a few degrees of per-map rotation
**compounds through the recursion**. The rate of visible change is dominated by the *angle*
difference between the two tables and is therefore front-loaded — nothing like the linear read the
parameter's description invites. A cross that stays recognisably the named figure would have to live
under about `0.03`, which is not a lever.

The practical consequence: `morph` is for presets that **travel**, and a preset that wants to stay
one figure should not bind it at all and should use the four levers, which is exactly what they are
for. Both shipped IFS presets are now built that way — `attractor_fern` binds no `morph` at all, and
`attractor_dissolve` uses the full range.

**A second, independent finding on the same surface: the spiral is a poor morph TARGET.** Five
candidate pairs were swept end to end as filmstrips. Anything ending at the spiral thins into ragged
streaks with half the frame empty, because the spiral's dominant map contracts at only `0.93` — the
intermediate spends nearly every sample on a map that barely contracts, so the orbit spreads instead
of settling. `fern -> spiral` is the design's own showcase pair (ADR-0075) and came **last of the
five**. The best by a distance is `sierpinski -> fern` — and the figure ADR-0075 doubted would earn a
preset is exactly why it works: the Sierpinski's rigidity is what makes the dissolve legible.

### What a fix would be

Documentation, in `presets/README.md`'s IFS section: the measured table above, the sentence that
`morph` is travel rather than life, and a note that the spiral is a fine figure and a poor target.
No code change — the surface is behaving as designed and being described in a way that points
authors the wrong way.

### Priority

**Medium.** It is a paragraph, and without it the natural first thing an author tries — a small
`morph` for some life — produces a figure that is no longer the one they named.

---

## ~~0066 — the IFS figures are STILL, so the library's conventions about drift rates are wrong for them~~

> **Discharged 2026-08-05** (Plan 0062 close). Documentation, landed in the same commit that raised
> it, `cf977f9`: `presets/README.md`'s IFS section now says the figure is static so the levers carry
> all the motion and want ~30 s periods, and that `spin`'s default of a full revolution every ~35 s
> is wrong for a figure with an intrinsic "up". **The one part not discharged is the last sentence of
> its fix** — nothing yet notes, where the animation gate is described, that a passing `anim` is not
> evidence of a watchable preset on a still family. That is a real gap in `docs/capturing.md` and is
> the sort of thing that belongs in whatever plan next touches the gate, not in a plan of its own.

- **RESOLVED 2026-08-05, by documentation** — `presets/README.md`'s IFS section now says the figure
  is static so the levers carry all the motion and want ~30 s periods, and that `spin`'s default is
  wrong for a figure with an orientation (`cf977f9`). The optional half — a note wherever the
  animation gate is described, that a passing `anim` is not evidence of a *watchable* preset on this
  family — is **not** done and is the part still worth doing.
- **Raised:** 2026-08-04, from `preset-author`, during the Plan 0062 Phase 7 content pass.
- **Verified by measurement:** yes — `shot --report`.

Every attractor preset in the library drives its slow evolution from `time` sines with periods of
roughly **200-400 s**, and that is correct for a strange attractor: the attractor is *already*
churning, and the drift only stops it repeating. An IFS at fixed levers is a **photograph**. Nothing
in the figure moves on its own, so everything a viewer sees moving is a lever moving.

Copying the library's periods therefore produces a preset that is technically alive and visually
static. Measured `anim`, against the 0.01 gate floor:

| preset | inherited ~200 s periods | ~30 s periods |
|---|---|---|
| `attractor_fern` | 0.018 | 0.033 |
| `attractor_dissolve` | 0.016 | 0.025 |

Both **cleared the gate** at the slow periods. That is the part worth recording: the animation gate
does not catch this, because its floor is set for "is this frozen" and a still figure under a
slowly-panning view clears it comfortably.

**A related trap on the same family, and it cost a render to find.** `spin` **defaults to 1**, which
is a full revolution every ~35 s. On a chaotic cloud that is the shipped look, and leaving `spin`
unbound is correct. On a figure with an intrinsic "up" — the Sierpinski is an equilateral triangle,
the fern is a plant — the default means the figure spends half of every cycle **upside down**, which
reads as the frame being crooked rather than as the figure turning. An IFS preset almost always
wants `spin` bound to a rock instead. It had also been supplying most of `attractor_dissolve`'s
`anim` until it was bound, which is how the slow-period problem above stayed hidden.

### What a fix would be

Documentation in `presets/README.md`'s IFS section: one paragraph saying the figure is static so the
levers carry all the motion and want ~30 s periods, and that `spin`'s default is wrong for a figure
with an orientation. Optionally a line wherever the animation gate is described, noting that a
passing `anim` is not evidence of a *watchable* preset on this family.

### Priority

**Low-medium.** Purely advisory, but it is the difference between the family's first two presets
looking alive and looking like wallpaper, and neither the gates nor `--report`'s `anim` column
flags it.

---

## 0067 — `depth_fade` is a uniform dimmer on every flat family, where the other two depth cues are exact no-ops

- **PROMOTED 2026-08-09 → [Plan 0075](plans/done/0075-the-content-renaissance.md) Phase 2** — option
  2 (the true no-op: multiply the fade term by has-depth), restoring ADR-0076's stated
  invariant. Option 3 (redefine `depth01` so flat reads as nearest) stays rejected-for-now for
  the reason this entry gives: it changes what `dn = 0` means for the 3-D families too.
- **Raised:** 2026-08-05, from `preset-author`, while looking for depth on the IFS family.
- **Verified by measurement:** yes — per-parameter capture diffs on `attractor_dissolve`.

ADR-0076's design is that a 2-D family has an inverse depth extent of **exactly `0.0`**, so every
depth cue collapses to the identity "with no shader branch, no division and no way to reach a
`NaN`". Two of the three do. The third does not.

Isolated on `attractor_dissolve` (an IFS, so `dn ≡ 0`), each parameter set alone against an
otherwise identical capture:

| parameter | value | pixels differing | max channel delta |
|---|---|---|---|
| `perspective` | 0.7 | **0** of 921 600 | 0 |
| `depth_hue` | 0.6 | **0** of 921 600 | 0 |
| `depth_fade` | 0.9 | **184 989** (20.1 %) | **97** |

The arithmetic is straightforward once seen. `haze(dn) = 1 - depth_fade * (1 - depth01(dn))` and
`depth01(dn) = (dn + 1) * 0.5`, so at `dn = 0` the multiplier is `1 - depth_fade/2` — **not 1**. A
flat figure at `depth_fade = 0.9` is uniformly dimmed by 45 %, with no gradient, because "mid depth"
is the only depth it has.

It is arithmetically consistent — `dn = 0` genuinely is the middle of the depth range, and the middle
of a fade is half of it. It is not what the ADR's own summary claims, and it is not what
`presets/README.md` claimed either until this entry (that text has been corrected).

**Why it matters more than a doc slip.** It is a trap in both directions. An author binding
`depth_fade` on a 2-D preset expecting nothing gets a 45 % brightness cut that looks like the preset
being mysteriously dark; an author who *notices* the dimming may start using `depth_fade` as a
brightness trim, which works, is undocumented, and would break the moment anyone gave the IFS family
a third dimension. `exposure` is the parameter that means "dimmer" and says so.

**Note this is not IFS-specific** — `de_jong` and `clifford` have had it since Plan 0063 / ADR-0076
landed. Neither shipped preset binds `depth_fade`, which is why it went unnoticed.

### What a fix would be

Three options, in ascending cost.

1. **Document it** — done, in `presets/README.md`'s depth section. Cheapest, and leaves the
   asymmetry live.
2. **Make it a true no-op**, by multiplying the fade term by whether the family has depth at all:
   `1 - depth_fade * (1 - depth01(dn)) * has_depth`, where `has_depth` is `0` when the inverse extent
   is `0`. That restores the ADR's stated property — all three cues identical on a flat family — for
   one extra multiply and no branch, in the same style as the existing zero-extent trick. It would
   move any baseline of a preset that binds `depth_fade` on a flat family; none does.
3. **Redefine `depth01` so a flat family reads as nearest** rather than mid. Cleaner conceptually
   (a figure with no depth is not "half far away") but it changes the meaning of `dn = 0` for the 3-D
   families too, so it is the one that needs thought rather than a patch.

### Priority

**Low-medium.** Nothing renders wrong today and no shipped preset touches it. But it is a stated
invariant that is false, and the ADR leans on the "no branch, no division" phrasing as evidence the
design is clean — which is true for two thirds of it.

---

---

## Entry 0070 — from the Plan 0069 close (2026-08-06), and it is what that plan turned out not to buy

---

---

## 0070 — the in-frame geometry fraction cannot gate new content, and the number it computes for every line preset is not in the author's report

- **PROMOTED 2026-08-09 → [Plan 0075](plans/done/0075-the-content-renaissance.md) Phase 2** — the
  cheap half this entry names: the fraction becomes a `shot --report` column, where the
  over-scale defect is actually introduced. The distribution report in `sanity.rs`'s shape
  stays a candidate second step, not taken.
- **Raised:** 2026-08-06, at [Plan 0069](plans/done/0069-the-instrument-that-sees-a-figure-leave-the-frame.md)'s
  Mode 4 review. Successor to archived [0054](design-backlog-archive.md#0054--pixel-coverage-cannot-see-a-figure-whose-tips-leave-the-frame-and-an-in-frame-geometry-fraction-is-the-successor),
  which asked for a **gate** and got a **paired instrument**.
- **Verified by measurement:** yes — the numbers below are the printed report from
  `cargo nextest run -p lmv-core --test geometry_extent --no-capture`, reproduced on this box at the
  close.
- **Not a defect.** Everything Plan 0069 built works exactly as
  [ADR-0083](adrs/0083-in-frame-geometry-is-measured-at-the-line-renderers-draw-seam.md) designed it.
  This is the gap that remains after it.

**What landed, and the shape of what it left.** `LineRenderer::draw` now measures the share of drawn
segment length inside the render target, covering the four line families with one implementation.
Against the two frozen defects it is decisive: repairing them moves the measure `0.4975` (comb) and
`0.7788` (corona), where pixel coverage had `0.055` and scored both defects *above* the legitimate
content. That half is closed and closed well.

**But the instrument convicts only in pairs, and a new preset has no pair.** The gate compares a
configuration against a frozen *repair* of itself, matched by name. That is the right question for a
content pass verifying its own fix, and it is the only question available — no absolute threshold
orders the library, because `Rose Zoom` (`0.3492`) and `Rose Overflow` (`0.3659`) **bracket** the
over-scaled comb (`0.3563`) and both are working exactly as authored. A length fraction cannot tell
"deliberately inside the figure" from "accidentally outside the frame"; they are the same picture.

So the defect class Plan 0069 was built for **still ships undetected on new content**. Author a
`spectrum` preset tomorrow with the same over-scale that shipped in `2efb80e^` and nothing fails:
`sanity.rs` sweeps the library against per-system coverage floors, but `geometry_extent.rs` asserts
only on the two frozen pairs. The plan's own Followup — "if Phase 3 convicts a shipped preset, file
it" — could not fire for the same structural reason, and should be read as answered rather than
pending.

### What would close it

**Put the number where the author already looks, rather than inventing a threshold ADR-0083 twice
proves does not exist.** `shot --report` (`standalone/examples/shot.rs:28`) is the content lane's own
metrics table and does not carry this column. The fraction is computed on the CPU from segment
endpoints and an aspect, with no rasterizer in the loop — so it is cheap, machine-independent, and
already produced on every capture of a line-family preset. Surfacing it there turns "no gate" from a
hole into a number a human can read while tuning `scale`, which is the point at which the defect is
actually introduced.

A distribution *report* in `sanity.rs`'s shape (`report_coverage_distribution`, lowest-first, printed
not asserted) is the natural second step and is explicitly an ADR-0071 report rather than a gate.

**Not the answer:** an absolute floor over the library. It fails two shipped presets on day one, and
it is the identical mistake this whole line of work exists to stop making.

### Priority

**Medium-low.** Nothing is broken and no preset ships defective today — both known defects were
already repaired before the instrument existed. But the plan that built the instrument was scoped as
"the gate that convicts an over-scaled figure", and what is deployed cannot do that for content
nobody has already fixed. The cheap half (a `--report` column) is small enough that leaving it undone
is mostly a matter of nobody having filed it.

---

---

## 0072 — `sanity.rs`'s coverage floor forces dense thin-stroke line scenes into washed-out tuning, and it is measuring the halo

- **PROMOTED 2026-08-09 → [Plan 0075](plans/done/0075-the-content-renaissance.md) Phase 1** — first
  phase of the content renaissance, because a gate that selects for the rejected look must not
  examine a whole new library. The mechanism (per-family thin-stroke floor vs structural
  occupancy) is chosen at implementation from this entry's two candidates; the non-vacuity
  probes are the ones pinned below (the retired mandalas' honest tunings, and the
  renders-nothing case the current floor does catch).
- **Raised:** 2026-08-06, at Plan 0065 Phase 3, from the user's verdict on the shipped preset.
- **Verified by measurement:** yes, three ways, all inside Plan 0065's own lane.

`star_mandala`'s first draft ran `thickness = 1.15` / `glow = 0.85` / `trails = 0.26` — the author's
judgement that forty times as many segments want a lighter hand. It **fails** `core/tests/sanity.rs`,
which captures at 96x96 and enforces a `star_pattern` coverage floor of **0.34**: it measured
**0.2541**. A second pass at 2.05 / 1.20 / 0.30 read **0.3053**, still short. What ships reads
**0.4346** at 2.45 / 1.55 / 0.40 — and the user's first reaction to it in the running app was that
every line is half transparent.

**The gate cannot see the figure it is gating.** Three measurements say so:

- The mandala's coverage (0.4346) sits **below** bare `star_rosette`'s (0.7995) while drawing ~46x
  the geometry.
- In a controlled A/B at `star_rosette`'s own shipped tuning, the bare rosette and the four-ring
  mandala score **identically (0.403)** — 46x the segments, same number.
- A structural measure separates them cleanly: **9 of 10 radial shells occupied against 1**.

At 96x96 a hairline over a 46-fold ornament aliases to nothing, so `coverage` on this scene is a
measure of the **halo and the trail**, not of the figure. The only lever that moves it is inflating
glow and tail — which is exactly the look the user rejected. **The gate is selecting for the defect.**

### What Plan 0065 Phase 5 measured when it refused the trade (2026-08-06)

The phase was told not to buy coverage with `glow` and `trails`, and it did not. Three mandala
presets ship tuned by eye at 1280x720 with **`glow` at the engine's 1.0 and no `trails` binding at
all**, and all three **fail** this floor:

```text
star_pattern  floor 0.34  lowest 0.2442 (Star Mandala) - factor 0.72
  0.2442 Star Mandala  |  0.2505 Mandala Six  |  0.2544 Mandala Weave  |  0.6908 Star Lantern  |  0.7995 Star Rosette
```

Three things that sharpen the entry rather than repeating it:

- **`thickness` alone cannot reach the floor at a look anyone would ship.** Swept on the four-ring
  preset with `glow = 1.0` and no trail: `2.2 -> 0.198`, `3.1 -> 0.244`, `4.6 -> 0.247`,
  `6.5 -> 0.311`, `9.0 -> 0.366`. The floor is first cleared at a base thickness of about **9**,
  which is a **29-px-wide stroke at 1080p**; the figure has already started closing into a blot by
  4.6. The lever the entry hoped for is not enough on its own.
- **Density still does nothing, now measured within one tuning family.** `Mandala Six` draws 1 684
  segments against `Star Mandala`'s 1 092 — **54 % more geometry** — for a coverage difference of
  0.2442 vs 0.2505, i.e. **2.6 %**. That is the entry's central claim reproduced without changing
  anything else.
- **The three failures are the honest tuning, and the pictures are the evidence.** Compare
  `presets/star_mandala.toml` before and after this phase in `git log` — the pre-Phase-5 file carries
  `glow = 1.55` / `trails = 0.40` and its own comment block explaining the escalation, which is the
  tuning the user rejected in the running app.

**So the gate is red on `star_pattern` until this entry is taken.** That was accepted deliberately
rather than papered over; the alternative was to ship the look the user had already refused.

### Update 2026-08-06 — the floor moved to `0.12` and back to `0.34` the same day, and this entry survives both

Plan 0065 Phase 7 re-derived the floor to `0.12` at its close, which is exactly what
`coverage_floor`'s own rule prescribes when a family minimum moves. Hours later the user rejected all
three presets on sight for a reason that has nothing to do with this entry — the motifs are sampled
polylines and the vertices show ([0073](#0073--motif-outlines-show-their-vertices-and-a-sampled-polyline-does-not-read-as-a-curve))
— so they were retired and the floor reverted to `0.34`.

**Nothing about that answers this entry.** The three measurements above were taken on real content
and stand on their own: at 96x96 the bare rosette and the 46x-denser mandala score *identically*,
54 % more geometry moves coverage 2.6 %, and `thickness` alone first clears the floor at a 29-px
stroke. The next dense thin-stroke line preset meets the same wall. What changed is only that the
library currently ships no preset sitting against it, so the pressure is off the schedule rather than
off the problem — and a reader finding `0.34` in the code should not conclude the episode never
happened.

### Why this is not the same entry as 0054

[0054](design-backlog-archive.md#0054--pixel-coverage-cannot-see-a-figure-whose-tips-leave-the-frame-and-an-in-frame-geometry-fraction-is-the-successor)
and its successor [Plan 0069](plans/done/0069-the-instrument-that-sees-a-figure-leave-the-frame.md)
are about a figure leaving the frame; the in-frame geometry fraction that plan shipped does **not**
catch this one, because a mandala is entirely inside the frame and scores a clean 1.0. This is the
opposite failure: the figure is present, correct and dense, and the instrument reads it as sparse.
Both entries are the same root cause — a pixel statistic standing in for a structural one — and
anyone taking either should read the other.

### What a fix would be

Not a threshold change. Either a per-family floor that acknowledges what thin-stroke line scenes
render at 96x96, or a structural occupancy measure (the radial-shell count above is one candidate and
it already exists as a one-off in the lane). Whatever replaces it must be checked against the case
where it currently succeeds, which is catching a scene that renders **nothing**.

### Priority

**Medium-high, and it will recur on the next mandala.** Every future preset on this scene meets the
same floor and gets pushed toward the same washed-out tuning. It is the one item here that actively
degrades shipped content rather than merely failing to help.

---

## ~~Entry 0074 — from Plan 0073 Phase 6 (2026-08-06), the content pass on the two IFS colour channels~~ — BUILT

> **Closed 2026-08-08 by [Plan 0074](plans/done/0074-the-figure-colours-by-how-far-it-has-come.md) /
> [ADR-0088](adrs/0088-the-ifs-colours-by-distance-from-its-own-skeleton.md).** Route 2 was taken —
> the channel that IS spatial — and route 3 shipped with it: `root_tint` / `root_hue` measure
> distance to the nearest drawn fixed point directly, and `age_tint` / `age_hue` are retired, so the
> roster did not grow. Route 1 (`emergence`) shipped too, but on ADR-0087's independent
> `fade`-interaction merits rather than as the age channel's rescue.
>
> The plan gated on a rendered sample set after one implementation phase rather than at the end,
> because this entry's whole lesson was that the *previous* channel's reasoning sounded right and
> was not. It read, on all five figures. The gate also found something this entry did not predict:
> the palette coordinate is a **fixed budget**, so `attractor_fern` needs `map_tint` cut `0.46 -> 0.22`
> before `root_tint` improves on stock — stacked at full strength the plant washes out.
>
> Kept below unstruck as the record of the measurement that motivated it.

---

## ~~0074 — the age channel has nothing spatial to colour, because the emergence ramp hides the only steps where age correlates with position~~

- **Raised:** 2026-08-06, at [Plan 0073](plans/done/0073-the-fern-unfurls-and-colours-by-what-made-it.md)
  Phase 6 — the `preset-author` pass that phase exists to run. It is the phase's own
  "any channel or route that could not be made to read is written up here rather than quietly left
  bound to nothing".
- **Verified by measurement:** yes — rendered, and rendered in the configuration most favourable to
  the channel.
- **Half of the plan landed and landed well.** `map_tint` / `map_hue` work, are now bound in both
  shipped IFS presets, and survive the morph. This entry is about the other half only.

**What was built.** [ADR-0087](adrs/0087-the-ifs-particle-carries-its-age-and-its-last-map.md) gave
every particle two channels — which map last moved it, and how many steps since it last respawned —
each reaching the picture by a palette coordinate and a hue rotation. The ADR's stated reading of the
age channel is that "age is distance-from-the-fixed-points in disguise": a young particle has been
iterated only a few times, so it sits near one of the four points the figure contracts toward, and an
old one has spread across the whole figure.

**What it actually renders as: per-particle speckle with no gradient anywhere.** Measured on a bare
fern, `fade = 0` so no trail could average anything, `size = 0.9`, a three-stop ramp chosen for
maximum contrast, and `age_tint = 0.75` — the most favourable configuration the preset surface can
build. The figure comes out a uniform tint carrying fine multi-coloured noise. The same probe with
`map_tint = 0.75` instead, everything else identical, separates the fern into four legible regions:
cream body, teal and cyan fronds, a thin stem. Swept across `morph` at 0 / 0.25 / 0.5 / 0.75 / 1.0 on
`sierpinski -> fern` with `age_tint = 0.75` and a long `fade = 0.912`, no position shows a gradient.

**Why, and it is structural rather than a tuning miss.** The ADR's reading is true only for a
particle's first handful of steps. The family's probability-weighted per-step contraction is `0.742`,
so after about ten iterations a particle's position is decorrelated from its age — it can be
anywhere on the figure. And the first **eight** steps are exactly what
[`EMERGENCE_STEPS`](../core/src/render/scenes/particles/mod.rs) deliberately makes invisible, because
those are the steps where a particle sits on one of four points and the trail field would integrate
a thousand of them per frame into four bright dots.

So the two constants ADR-0087 treats as independent look knobs are in **direct opposition**:

- the emergence ramp exists to hide the four restart points;
- the restart points are the only place age correlates with position;
- therefore the ramp hides precisely the material the age channel exists to colour.

Lengthening the lifetime does not help — the problem is spatial, not temporal. The 180-step lifetime
was judged fine in the running app on 2026-08-06 ("looks ok really"), and the churn reads as life
rather than as twinkle, so the plan's *other* two open questions are answered and only this one is not.

**What the channel is still good for, and why that is not enough to bind it.** Because each
particle's colour cycles through the ramp over its own life, `age_tint` under a trail is a slow
per-particle shimmer. But `hue_spread` already gives per-particle colour variation from the fixed
seed, on the same coordinate, and reads as *grain* rather than as noise because it does not cycle.
Neither shipped preset binds `age_tint` or `age_hue`, and neither should until this is resolved.

### What would close it

Three candidates, and they are genuinely different decisions rather than a menu:

1. **Make the ramp length authorable, or shorten it, and accept some hot spots.** The cheapest, and
   the one the plan half-anticipated by calling both constants "the lever". It trades the artifact
   the ramp was built to remove against the gradient the age channel was built to show. Somebody has
   to look at that trade in motion; no capture decides it.
2. **Colour by a channel that IS spatial.** What the ADR wanted to show — distance from the fixed
   points — is computable directly and does not decay: the shader already has the four points on its
   step uniform. A `depth`-like "how far from the nearest restart point" channel would produce the
   gradient the age channel was reaching for, permanently, and would not fight the ramp. This is the
   honest successor and it is a new plan.
3. **Retire `age_tint` / `age_hue`.** Two params on the longest roster in the library that no
   shipped preset can use is a cost, and ADR-0087's Consequences already flagged "four new params on
   a scene that just took five" as a knowing risk. The two spare words in `Particle` stay spent
   either way — `age` still drives the emergence ramp, which is load-bearing.

**Not the answer:** tuning the lifetime, or binding the channel harder. Both were tried at the
extremes and the picture does not change in kind.

### Priority

**Medium.** Nothing is broken and nothing ships defective — the params default to the identity and
no preset binds them. But half of an accepted ADR is currently unusable content surface, and the
roster documents four channels where an author will find two that work. Whichever of the three
routes is taken, `presets/README.md` needs to stop presenting the four as peers.

---

---

## ~~0076 — the operator docs describe a fern tuning that the shipped fern does not carry~~ — REPAIRED

> **Closed 2026-08-08 at [Plan 0074](plans/done/0074-the-figure-colours-by-how-far-it-has-come.md)'s
> Mode 4 close**, in exactly the shape this entry proposed: the measurement and its attribution to the
> gate are kept in both files, and each is finished with what actually shipped, joined to the
> `*_hue`-is-the-escape paragraph already sitting below it.

- **Raised:** 2026-08-08, same pass as [0075](#0075--root_tint-earned-no-binding-on-either-shipped-ifs-preset-and-root_hue-earned-both). **This is drift, not a design gap** — filed here because
  it was found by the content lane and the fix is the architect's.
- **Verified:** yes, by reading both files against the shipped `.toml`.

Plan 0074 Phase 5 wrote the Phase 2 gate's fern finding into two operator docs as the worked example
of the palette-coordinate budget rule:

- `presets/README.md` — "Plan 0074 then found the same fern needs `map_tint` cut from `0.46` to
  `0.22` before `root_tint` improves the picture."
- `docs/preset-palettes.md` — the same numbers, as the budget rule's worked example.

Both sentences are **true as measurements** and were correct when written. But Phase 6 then did the
comparison the gate could not — against `root_hue`, which did not exist yet — and shipped the fern
with `map_tint` at its **full `0.46`** and no `root_tint` at all. So a reader who opens
`attractor_fern.toml` expecting the documented `0.22` finds `0.46`, and the worked example now
illustrates a tuning nothing in the repo carries.

**The rule itself survives intact** — the coordinate *is* a fixed budget, and the fern is still the
evidence for it. What changed is the conclusion drawn from it: the fern did not pay the budget, it
took the escape. That is arguably the *better* worked example, since it ends by showing what `*_hue`
is for.

### What a fix would be

Two sentences in each file at the plan's close: keep the measurement, attribute it to the gate, and
finish it with what shipped instead. `docs/preset-palettes.md`'s "**`*_hue` is the escape**"
paragraph is already immediately below the worked example in both files, so the correction is a
join, not a rewrite.

### Priority

**Medium** — it is small, it is in the two files an author reads *first* for exactly this decision,
and it is the kind of drift that reads as authoritative until someone diffs it against the preset.

---

## 0084 — the ink stage has no contrast lever, and three worlds in two cohorts paid for it

- ~~**PROMOTED 2026-08-11 → [ADR-0092](adrs/0092-the-ink-remap-gains-a-contrast-exponent.md) +
  [Plan 0078](plans/done/0078-the-ink-learns-to-bite.md)** — same day as filed; the response
  exponent, endpoints invariant so the paper never moves. Queued after Plan 0076 + cohort 6.~~
- **CLOSED 2026-08-12 at [Plan 0078](plans/done/0078-the-ink-learns-to-bite.md)'s close.**
  `ink_gamma` ships and `presets/README.md` carries the three-lever note. **The content half is
  standing, not closed**: that plan's Phase 3 (`human`) is the ink worlds re-judging onto the
  lever, carried in the [plans README](plans/README.md)'s Standing section with its two-header
  roster (Etching, Shatter). If a world turns out to need a toe *and* a shoulder, that is
  ADR-0092's named negative — file it **with the measurement** as a new entry rather than
  reopening this one.

**Raised by:** `preset-author`, three separate times across Plan 0075 cohorts 3 and 4.
**Owner if taken:** `architect` (a small ADR — where the response shape lives) then `dev`.

### The finding

The want, each time: a duotone whose dark pole bites harder **without moving the paper**. The
reach, each time: a contrast/gamma control on the terminal ink remap (`ink_*` / `paper_*`). The
remap keys on luminance with a fixed response, so the surface has exactly two workarounds and
both give something up: author the duotone into `[palette]` (the Etching world did this — it
works, and it spends the palette on what the remap should be doing), or juggle
`brightness`/`fade`, which trades away structure to buy contrast.

### Handoff verdict (2026-08-11): promote

Three measurements across two cohorts, and every ink-mode world pays it — the clearest
demonstrated want in the handoff. The obvious shape is a response exponent on the remap; the
nameable rejected alternative is palette-side authoring, which Etching proves possible and
which costs the palette. Where the lever lives and what it does to the existing `ink_*` roster
is the ADR.

---

## 0085 — `swarm` has no `reseed`, so a flow-field pile-up has no recovery lever

- **PROMOTED 2026-08-11 → [Plan 0077](plans/done/0077-the-quiet-sky.md) Phase 3** — same day as
  filed, riding the swarm-individuation plan exactly as the verdict below proposed; ADR-0066
  disturbance semantics, and the horizon caveat is carried by that plan's Phase 5.
- **DELIVERED 2026-08-12 (Plan 0077 Phase 3, `3bfc7c8`).** The swarm's `reseed` is the
  attractor's percussive accent with ADR-0066's semantics — a seeded ±6 % domain-relative kick
  on a rising edge past 0.5, never a box respawn — measured dispersing (frame-diff 0.153
  against control) and re-gathering (coverage gap 0.30 % three seconds on). The caveat this
  entry insisted on is honoured, not waved through: the *minutes-horizon* rescue is explicitly
  unclaimed by any test, and the one-off soak observation is Phase 5's rider, standing in the
  plans README with 0086's trigger intact.

**Raised by:** `preset-author`, Plan 0075 cohort 4 (the Shatter world). **Owner if taken:**
`dev`, inside whatever plan takes
[0068](#0068--a-swarm-mark-has-no-per-mark-variation-so-the-only-scene-that-can-hold-a-starfield-cannot-make-one-twinkle)'s
per-mark variation — same scene, same plan.

### The finding

Three live collapses before Shatter was rebuilt at engine-default dynamics: a swarm under
minutes of sustained flow-field force piles onto the field's attractors and stays there. The
author's reach was `reseed` — the attractor family has exactly that lever, since
[ADR-0066](adrs/0066-a-reseed-disturbs-the-cloud-rather-than-replacing-it.md), for exactly this
class of reason (disturb the cloud rather than let it degenerate). The swarm has no equivalent,
so the only recovery was rebuilding the world's dynamics from scratch.

Note what the suite said throughout the three collapses: **green**. The failure develops over
minutes and no capture path reaches that horizon — that half is
[0086](#0086--no-capture-path-reaches-the-minutes-long-horizon-so-a-slow-accumulation-failure-is-invisible-to-every-instrument),
and anyone taking this entry must read it first: a swarm `reseed` shipped without it is a lever
whose need the suite cannot demonstrate and whose effect it cannot verify.

### Handoff verdict (2026-08-11): promote, riding 0068's plan

One demonstrated want with three live failures behind it, and the marginal cost inside a
swarm-individuation plan is small. Not worth a plan of its own.

---

## 0088 — `shot --report`'s band columns cannot see reactivity spent on bloom

- **PROMOTED 2026-08-11 → [Plan 0077](plans/done/0077-the-quiet-sky.md) Phase 4** — same day as
  filed, riding the sparse-idiom plan as its small instrument phase; no ADR, per the verdict
  below.
- **DELIVERED 2026-08-12 (Plan 0077 Phase 4, `b1ca4e9`).** The mean columns stay untouched —
  every historical `--report` number keeps meaning what it said — and a **footprint reading**
  lands beside them (`metrics::footprint_diff` over the same capture pairs; text gets its own
  labeled block, JSON gains `reactivity_footprint`). The bloom-only fixture reads bass 0.161
  where the mean reads 0.004, unbound bands stay 0.000 in both readings, and the end-to-end
  claim is pinned in `shot_cli.rs`. The family is now three for three. **The house workaround
  is obsolete** — `fragment_vitrail.toml`'s header still cites it and is on the content lane's
  list (Plan 0077's Standing entry).

**Raised by:** `preset-author`, Plan 0075 cohort 4, as an instrument note. **Owner if taken:**
`dev` — `standalone/examples/shot.rs` and the report's band statistic.

### The finding

A preset whose reactivity is spent on `bloom_amount` reads **~0.000** in the report's band
columns — dead, in the very instrument the lane verifies with. Mechanism as the lane reports
it: sRGB→linear palette peaks times glow sit just under the report's threshold. House
workaround now in use: give onset a structural lever (e.g. `flash`) alongside bloom, so the
report has something to see.

This is the third member of a family the project has fixed twice:
[0022](design-backlog-archive.md) (the report blind to a level `curve`) and
[0028](design-backlog-archive.md) (reachability blind to bare comparisons) were the same
shape — the author's instrument silently under-reporting a legitimate binding class — and both
were promoted and closed.

### Handoff verdict (2026-08-11): promote, small

An instrument that misreports the library it verifies earns its fix, and the family precedent
is two for two. A single-phase plan, or ride the next plan that touches `shot --report`
(Plan 0075 Phase 2 is already adding a column there). Likely no ADR — unless the fix wants a
threshold, in which case ADR-0071's rule applies to wherever the number comes from.

---

## 0091 — there is no static, screen-anchored, oriented gradient, so a horizon cannot be drawn

- **PROMOTED 2026-08-12 → [ADR-0094](adrs/0094-the-backdrop-paints-a-directional-ramp.md) +
  [Plan 0080](plans/done/0080-the-sky-gets-a-horizon.md)** — the same day it was raised, because the
  entry arrived with the workaround already rejected by the user and the mechanism already
  concrete. The backdrop pre-pass gains one ramp axis and paints a *segment* of the preset's
  palette along it; the shape was chosen by user interview over a `gradient` scene and over an
  orientation lever on `fragment_field`.
- **Raised:** 2026-08-12, from `preset-author`, in a content session riding
  [Plan 0077](plans/done/0077-the-quiet-sky.md)'s new swarm params.
- **Verified against code:** yes, all three walls, at the lines cited below.
- **Not a defect.** Everything named here works as designed. This is a missing primitive.

**The look that failed.** A photoreal dusk — a bright orange horizon band whose light fades
smoothly upward through deep blue to near-black over roughly the lower third of the frame (the
user supplied a reference photo of exactly this), with a music-driven starfield above it. The best
approximation the shipped engine can express renders a **hard-edged static band with a thin glow
rim**, and the user's verdict on it is **"unacceptable"** — so this is a demonstrated want with a
rejected workaround, not speculation.

**Three walls, each structural rather than a tuning miss.**

1. **`fragment_field` cannot be it.** Its field is `0.5 + 0.5 * sin(p.x + p.y + t * 0.5)`
   (`core/src/render/scenes/fragment_field.rs:142`) — both axes weighted equally by construction,
   so its bands are diagonal and there is no orientation lever — **and** `t` enters the field phase
   directly, so even `warp = 0` cannot hold a frame still. There is no time-scale or freeze param
   on the scene.
2. **The backdrop pre-pass cannot be it.** It takes **one** palette sample at `bg_hue` and
   multiplies it by a *fixed* tilt, `mix(0.72, 1.0, 0.5 + 0.5 * ndc.y)`
   (`core/src/render/background.rs:124`), plus a radial vignette. The tilt is hardcoded, is 28 % of
   brightness rather than a colour ramp, and points **up** — brighter at the top, the wrong way
   round for a horizon.
3. **A line-scene fake cannot finish it.** A `spectrum` slab at `scale = 0` gives a static
   screen-anchored band — this worked, and is what the demo ships — but `glow` is a local stroke
   halo and the bloom pyramid's spatial radius is bounded (`bloom_radius = 3.0` measured, barely
   visible). **No stage downstream can turn a hard edge into a quarter-frame fade.**

**The layer budget is the second half, and it is why the ground cannot simply be another scene.**
This class of look needs three roles — ground gradient, reactive figure, star layer — and
[ADR-0090](adrs/0090-a-preset-composes-two-scene-layers.md) caps composition at a main scene plus
one `[layer]`. The dusk look already spends its layer on the star swarm.

**What made the smallest option also the strongest.** The demo's own `[palette]` already runs
`#060b24 → #1b2a5e → #c74b1d → #ff7a1f → #ffd06e` — near-black through deep blue to hot amber. That
*is* the dusk ramp, already authored, already baked into the LUT the backdrop has sampled since
[ADR-0086](adrs/0086-the-backdrop-colours-through-the-preset-palette.md). Sweeping the coordinate
along a screen axis makes the smooth fade fall out of the palette's own stops, and each stop's `at`
becomes the horizon's vertical placement — so no second colour language and no second placement
mechanism is needed.

**Evidence.** `quiet_sky_demo.toml` and `sun_v8..v10.png` in the content session's scratchpad; the
reference photo is the user's. The `v10` render is the finding in one image: a razor edge with no
upward light at all.

**One thing the promotion does not fix, recorded here because it is the same look's other wall.**
The backdrop is **invisible to every behavioral gate** — coverage measures the scene
([ADR-0067](adrs/0067-coverage-measures-the-scene-not-the-backdrop.md)) and the animation gate
strips `bg_*` ([ADR-0091](adrs/0091-the-animation-gate-scores-motion-against-the-figures-footprint.md)'s
Outcome). That is correct, and it is what stops a more capable backdrop being used to game the
gates, but it means a dusk world's whole gate burden falls on its figure and star layer even
though the frame looks full. If that prices the look out, the rule is the one
[0072](#0072--sanityrss-coverage-floor-forces-dense-thin-stroke-line-scenes-into-washed-out-tuning-and-it-is-measuring-the-halo)
established: **read the floor and re-derive by its own recorded rule, do not lower it to fit.**

## Entry 0077 — from the Plan 0061 close (2026-08-08), and it is a gate's blind spot rather than a preset gap

---

## 0077 — the doc-link gate is blind to reference-style links, so 85 of them rendered as bracket noise behind a green check

- **PROMOTED 2026-08-13 → [Plan 0084](plans/done/0084-two-gates-stop-lying-about-what-they-check.md)
  Phase 1**, with [0080](#0080--the-reactivity-gate-pays-18x-to-render-frames-it-throws-away-because-warm-up-and-measurement-share-one-capture-path)
  riding the same plan — two gates of the same size, sharing a session and nothing else. No ADR: the
  change restores a stated property and has no rejected alternative worth recording. **Re-verified
  against code 2026-08-13** — the regex at `scripts/check-doc-links.mjs:48` is unchanged and the
  script's own header still says *"not reference-style links"*.
- **CLOSED 2026-08-13** (Plan 0084 Phases 1-2, `33436f0` + `83cfb67`). Two new break classes beside
  the inline one — a use with no definition in its file, and a definition whose relative target does
  not resolve — reported through the same `file:line -> target` shape. **The narrowing that makes it
  usable was measured, not assumed:** a shortcut use is only reported when *some* file in the tree
  defines that label, without which the repo corpus yields 31 findings of which 24 are ordinary prose
  brackets; with it, 7 findings and no noise. It found exactly the seven breaks this entry and the
  plan predicted, all seven repaired by reading the target rather than pattern-matching it. **It
  proved itself again at this very close** — moving the plan into `done/` broke four inbound links
  and the checker named all four, one of them in the definition class it had just learned to see.
  **Known and deliberate blind spot:** the narrowing cannot see the mirror failure, where a
  definition block is *deleted* outright rather than left behind, since then no file in the tree
  defines the label and every use goes quiet. Documented in the script header. **Left undone:** the
  fixture tree the phase's done-when described was run ad-hoc and not committed, so the script's
  optional `root` argument has no caller in the repo and the bite check is unrepeatable — raised as
  a minor at the close, not repaired.

**Raised by:** `architect`, at Plan 0061's close ceremony. **Owner if taken:** `dev` — it is a
change to `scripts/check-doc-links.mjs`, which gates CI and the pre-push hook.

### The finding

Markdown has two link forms. `scripts/check-doc-links.mjs` validates one of them: its regex is
`/\]\((?!https?:|mailto:|#)([^)#\s]+)/g`, which matches `[text](target)` and nothing else. The
reference form — `[0044]` in the prose, resolved by a `[0044]: plans/done/0044-….md` definition
elsewhere in the file — is invisible to it, in **both** directions:

- a **use with no definition** renders as the literal characters `[0044]`, and the checker is silent;
- a **definition with a broken target** is never resolved at all, so it can rot freely.

Measured at Plan 0061's close: **85 undefined uses across 11 files**, every one of them behind a
green `links` job and a green pre-push hook.

### Why it accumulated, which is the part worth keeping

**62 of the 85 were created by Plan 0061's own Phase 7b**, and the phase did nothing wrong. It moved
~2,700 lines of link-dense prose from `docs/plans/README.md` into `README-archive.md` verbatim; the
reference *definitions* those lines depend on sit in a block at the bottom of `README.md` and stayed
behind. The archive shipped with **zero** definitions. The phase's done-when said, correctly, *"Run
it, do not inspect"* — and it ran clean.

This is the same shape as the 74 broken inline links Plan 0060 found: a rot that degrades only in a
browser, accumulates one close at a time, and has no natural moment of discovery. Plan 0060 built
the gate for the first form. This is the second form of the identical failure.

### What a fix would be

Collect each file's `^[label]: target` definitions and its `[label]` uses (excluding inline `[…](…)`
and fenced/inline code, which the checker already strips), then report two new classes beside the
existing one: a use with no definition, and a definition whose relative target does not resolve. The
existing per-line `file:line -> target` output shape carries both without a new format. Worth a
deliberately-broken-label check that it goes red naming the label, for the same reason Phase 2c owed
one: a link checker that silently passes is worse than none.

### Priority

**Medium.** The 85 uses were repaired at the close and all 68 definitions in the repo currently
resolve, so nothing is broken today — it is *unguarded*, and it re-accumulates on exactly the
close-ceremony `git mv` that Plan 0060 already proved nobody catches by eye.

---

## 0080 — the reactivity gate pays 1.8x to render frames it throws away, because warm-up and measurement share one capture path

- **PROMOTED 2026-08-13 → [Plan 0084](plans/done/0084-two-gates-stop-lying-about-what-they-check.md)
  Phases 3-4**, riding with [0077](#0077--the-doc-link-gate-is-blind-to-reference-style-links-so-85-of-them-rendered-as-bracket-noise-behind-a-green-check).
  The plan takes exactly the fix `core/tests/reactivity.rs:69` already names, and asserts the
  property that makes it safe: N warm-up hops **without** rendering must leave analyzer state whose
  next frame is **bit-for-bit** equal to N hops **with** rendering. `SIGNAL_HOPS` and `WARMUP_HOPS`
  do not move — the measured headroom is not being renegotiated, only the wasted work removed.
- **CLOSED 2026-08-13** (Plan 0084 Phases 3-4, `0e5216a` + `79b9b9b`). `Renderer::capture_audio_after_warmup`
  takes a count of leading hops to advance without drawing; `capture_audio` is that call with a
  warm-up of zero, unchanged. Measured on the Windows development box through the DX12 software
  adapter (ADR-0071 — a measurement, not a contract): **136.3 s -> 100.2 s over 36 presets**, two
  sequential runs in one session differing only by the change. **The 86 s -> 167 s figure this entry
  and the plan both quote is superseded and does not difference against it** — it was taken on a
  41-preset library. The ~15 % this entry attributed to the wider readback window does *not* come
  back: the readbacks are the measurement.
- **The premise was half wrong, and that is the finding worth more than the speedup.** The warm-up
  renders were not pure waste — they were also the *scene* warm-up, which `reactivity.rs` said in as
  many words. Removing them moved 35 of the 36 per-band vectors, the exception being `spectrum/Halo`,
  the only preset in the set with no accumulating state. Escalated rather than absorbed; the user
  accepted the recalibration. Nothing regressed and the direction is uniform — every maximum rose or
  held, and the lowest across the library went 0.0287 -> 0.0504 against the 0.020 floor. **Read any
  reactivity figure recorded before 2026-08-13 as a different measurement, not as drift.**
- **What still has no instrument, carried forward from the close review.** The byte-identity test in
  `core/tests/capture_advance.rs` guards the analyzer, which the render pass structurally cannot
  reach (`capture_api.rs:321` publishes before it skips). The thing that actually moves under this
  change — GPU-integrated scene state meeting the measured window `WARMUP_HOPS` steps colder — is
  documented in three places and asserted in none. It matters for the followup this entry enables:
  **any gate that copies this pattern onto an accumulating scene inherits the cold start silently.**
  `docs/capturing.md`'s gate section now says so where a future author will read it.

**Raised by:** `architect`, at [Plan 0067](plans/done/0067-the-curation-route.md)'s close.
**Owner if taken:** `dev` — `core/tests/reactivity.rs` and whatever `Renderer::capture_audio` needs
to grow to express "advance without rasterizing".

### The finding

Plan 0067 Phase 1 moved the reactivity gate onto real PCM through the real analyzer, which was the
right call and is the only reason a green suite now says anything about audio at all. It cost
**86 s → 167 s** over 41 presets, measured interleaved on one machine, and the lane declined to
absorb the number silently.

**About 85 % of that growth is warm-up.** Each capture runs `WARMUP_HOPS + SIGNAL_HOPS` hops, and
the warm-up exists so the analyzer's window is full before anything is measured — an FFT and an
onset envelope need history. But `capture_audio` **renders every hop**, warm-up included, and those
frames are discarded. The gate is paying for a full rasterization pass per warm-up hop to obtain a
DSP state that needs no pixels.

### Why it is worth an entry rather than a comment

The cheap fix was already tried and correctly rejected: `SIGNAL_HOPS = 16` cut the cost but dropped
`emitter_squall` to 10 % headroom, which is a gate that fails on someone else's machine rather than
a gate that is faster. So the *measured* budget is not negotiable downward — which leaves the
discarded work as the only slack, and makes this the one place the cost can come from.

It also compounds. This is the pattern Plan 0067 explicitly nominates as the one to copy if another
gate ever needs to answer an audio question, and each copy would inherit the same waste.

### What a fix would be

Split "advance the analyzer" from "capture a frame" in the capture path, so warm-up hops push
samples and step the DSP without a render pass, and only the measured window rasterizes. The
property to preserve is the one the plan already leans on: determinism. The analysis must remain a
pure function of its window, so a warm-up that skips rendering has to produce byte-identical
analyzer state to one that does not — which is testable directly, and is the assertion that would
make the change safe.

### Priority

**Medium.** Nothing is wrong; CI is simply slower than it needs to be on a job this project already
pays for more than once per push. Worth taking the next time anyone is in that file.

---

## ~~0090 — the Mac build's capture verdict is stderr-only, so a Finder-launched tester cannot tell us why it hears nothing~~

- **CLOSED 2026-08-13 by [Plan 0083](plans/done/0083-the-build-says-why-it-hears-nothing.md)** — the
  capture verdict is a value, and it lands in both artifacts a remote tester can send: a trailing
  `capture` column on every `diagnostics.log` row, and an `audio` line under the F3 panel, both read
  off one string built once at startup. The entry's claim — *we cannot tell why* — is discharged as a
  **capability**; the tester's own answer is Plan 0083's `human` Phase 5, still outstanding, and it
  is recorded here when it arrives.
- **PROMOTED 2026-08-13 → [Plan 0083](plans/done/0083-the-build-says-why-it-hears-nothing.md)** — no ADR;
  the shape follows the existing diagnostics surface. Two decisions the plan takes that this entry
  left open. The verdict lands as an **appended column**, not a startup line: this log rotates at
  1 MiB keeping one backup and the tester's spanned 6.5 days, so a line written once is exactly what
  rotation deletes — and a column also catches a capture that dies *mid-run*. And the **Windows arm
  is in scope**, not for symmetry but because nobody on this project can execute the macOS path, so
  building both is what gets the mechanism tested and reviewed on the development box.
  **Re-verified against code 2026-08-13** — `standalone/src/main.rs:1023` is still `eprintln!` only.

**Raised by:** `architect`, 2026-08-11, from the first external Mac tester report — "app
works, but does not react to music; permissions were granted". **Owner if taken:** `dev` —
`standalone/src/main.rs`'s capture startup, `diaglog.rs`, optionally `overlay.rs`.

### The finding

When `capture_mac::start()` fails, the app deliberately degrades: it prints one line to
**stderr** and renders without audio (`main.rs:1023` — same shape on Windows at `:996`).
That degradation is correct; what is wrong is that the reason exists **only on stderr**,
which a Finder launch discards. The two artifacts a remote tester can actually send carry no
capture verdict at all:

- **`diagnostics.log`** — the file `READ-ME-FIRST.md` step 6 asks testers to send — has the
  band columns (`bass`/`mid`/`treb`/`onset`), so it can show *whether* audio reached the
  analyzer (all ~0.000 under playing music = capture-side), but records nothing about
  whether capture started or which `CaptureError` it died with.
- **The F3 overlay** has no capture field (grep confirms: no capture/audio-state string in
  `overlay.rs`).

So the README's step 3 escape hatch — relaunch from Terminal to see the message — is the
*only* route to the reason, and it is the highest-friction ask in the whole tester loop.
The loop stalls exactly where this report stalled: "permissions were granted" is the
tester's entire observable, and it cannot distinguish the real candidates (not restarted
after granting; a **stale TCC grant** — each ad-hoc-signed build is a different app to
macOS, so the Privacy toggle can show an older build's entry as enabled while the new
binary is denied, the pile-up the README's update note already names; macOS below 13; a
Sequoia periodic re-approval lapse).

### What a fix would be

Cheap, and both halves off the audio thread. At startup, write one line into
`diagnostics.log`: capture started (path + negotiated format) or the `CaptureError`'s
`Display` — startup code, before the 1 Hz cadence. Optionally a capture field in the F3
overlay (`audio: SCK 48k stereo` / `audio: NONE — <reason>`) so a tester can read the
verdict off a screenshot. The sacred-callback rule is untouched — nothing here is on the
capture thread.

### Priority

**Medium-high while external Mac testing is active** — every remote round-trip that starts
with "it doesn't react" pays a Terminal-relaunch cycle this one log line would eliminate.
Drops to low once a capture-device picker (the live-performance plan's Mac half) surfaces
the state in-app anyway.

### Update 2026-08-11 — the tester's log arrived, and it is this entry demonstrated

The `diagnostics.log` came back: **1,249 rows spanning ~6.5 days and 12 app restarts, and
every row has all four band columns at exactly 0.0000.** The renderer is healthy throughout
(steady 60.0 fps, `frame_ms_avg` ~16.7, `gpu_bytes` constant at the 1080p float target; the
scattered 0.1-fps rows coincide with multi-minute timestamp gaps — sleep/background
throttling, not crashes). So the log *proves* capture never delivered one sample on any
launch — which rules out "forgot to restart after granting" (twelve restarts), quiet
music/gain (a live tap shows a noise floor eventually), and anything render-side — **and it
cannot say why**, which is this entry's exact claim. Thirteen launches produced thirteen
stderr lines naming the reason, and every one was discarded by Finder. The surviving
suspects (stale/mismatched TCC grant from the ad-hoc per-build identity, macOS below 13, an
SCK start error) are distinguishable only by the Terminal relaunch this entry exists to make
unnecessary.

---

<!-- Reference definitions for entries moved here by the 2026-08-13 sweep. The uses came
     across; their definitions lived in the live file and stayed behind — the Plan 0061
     Phase 7b failure mode, and the reason design-backlog 0077 exists. -->

[0048]: plans/done/0048-analysis-v2-and-the-retune.md


---

# Archived 2026-08-15 — at Plan 0089's close

Three entries, all discharged by [Plan 0089](plans/done/0089-the-framing-contract-stops-lying.md),
whose header named all three as its `**Closes:**`. Two of them (0078, 0081) are the pair the
2026-08-13 sweep found **falsified in their first halves** — both asserted an *absence* nobody had
checked against the file — and both survivors had named "the next plan that touches this file" as
their home with no such plan ever written. Plan 0089 was that carrier. The third (0089) is the
falsified `FRAME_FILL` invariant, whose own first suspect the sweep had already exonerated and
whose real mechanism (rotation) the plan then derived, pinned as a property test, and shipped
without moving a pixel.

**0078 and 0081 cross-link to each other by anchor.** Both bodies are here, so those links resolve
inside this file.

---

## ~~0078 — `kaleido_tile` is a discrete quantity that is not quantized, so it is the one term of the composed map an author cannot bind~~ — FALSIFIED

> **CORRECTED 2026-08-13, at the backlog sweep. The premise is false, and it was false when the
> entry was written.** `kaleido_tile` is not quantized **on purpose**, and the reasoning is a doc
> comment on the function itself (`core/src/render/kaleidoscope.rs:458`):
>
> > *Deliberately **not** rounded, unlike the fold order and the winding number. Those two are
> > integral because a fractional value is undefined or torn; a fractional cell count is neither.
> > `abs(fract(x·n/2)·2 − 1)` at `n = 2.5` is a perfectly continuous mirrored grid whose last cell is
> > cut off at the frame edge, so a smoothed `kaleido_tile` can ease between cell counts instead of
> > snapping — the one param on this stage where that is true.*
>
> That comment landed in `e648a02`, **Plan 0064 Phase 1** — five phases before this entry was filed
> at Phase 6 of the same plan. So the entry's central claim, that `tile` is "the same kind of
> quantity" as `kaleido_spiral` and `palette_steps`, is answered in place: those two are rounded
> because a fractional value is *meaningless*, and a fractional cell count is not.
>
> **What survives is smaller and is a look question, not a correctness one.** A fractional count
> genuinely does cut the last cell at the frame edge, so the wallpaper is seamless *within* the
> frame and clipped at its border. Whether that reads as a broken tiling or as an ordinary crop is
> a **render** judgement nobody has made — the entry asserts the first without having looked. If a
> content pass binds `kaleido_tile` to audio and the clipped edge reads badly, **that** is a fresh
> entry with a rendered pair behind it, and it would be arguing for a *clamp at the edge*, not for
> rounding.
>
> **The one thing worth acting on is a doc gap, and it is one line.** `fragment_tiled.toml` binds
> `kaleido_tile = "2"` as a constant and `presets/README.md` says nothing about whether it may be
> driven — so an author meets neither the capability nor its edge behaviour.
>
> **PROMOTED 2026-08-13 → [Plan 0089](plans/done/0089-the-framing-contract-stops-lying.md) Phase 2**, and
> the promotion is the point: "folded into the next plan that touches the symmetry stage's docs" was
> written here and **no such plan was ever written**, so the item sat with a named home and no carrier.
> The same was true of [0081](#0081--the-house-gain-rule-lives-only-in-preset-headers-the-first-half-is-falsified-the-exception-class-survives)'s
> survivor, which rides the same plan. Judging whether the clipped last cell *reads* badly is
> explicitly **not** in that phase's scope — nobody has looked, and a render is what would decide it.
>
> **The method note, which is why this is corrected in place rather than archived:** the entry is a
> claim about what the repo does *not* do, and it was checked against the param roster rather than
> against the function. "Nothing does X" rots exactly like "X is broken" does. Everything below is
> the entry as raised.

**Raised by:** `preset-author`, at [Plan 0064](plans/done/0064-the-symmetry-stage-and-the-banded-palette.md)
Phase 6. **Owner if taken:** `dev` — it is a CPU-side quantization in
`core/src/render/kaleidoscope.rs`, beside the one `kaleido_spiral` already has.

### The finding

The composed map ships three discrete quantities. Two of them are quantized to integers CPU-side
before the uniform is packed, for the reason this project has already written down twice — an eased
parameter is continuous even when its math needs integers, so the smoothing sweeps it through values
that are not merely wrong but meaningless. `kaleido_spiral` is quantized because a fractional winding
number draws a visible seam. `palette_steps` is quantized because a fractional band count is not a
band count.

**`kaleido_tile` is the same kind of quantity and did not get the same treatment.** It is cells
across the frame, with alternate cells mirrored. A fractional value splits a cell at the frame edge,
so the mirroring no longer meets and the wallpaper stops being seamless — which is the entire
property that makes the tile read as a pattern rather than a grid of stamps.

The practical consequence is narrow and complete: **a preset can only ever bind `kaleido_tile` to a
constant.** `fragment_tiled.toml` does exactly that and says so in its header. Every other term of
the composed map is audio-bindable; this one is decoration you set once.

### Why it is worth an entry rather than a comment

Phase 4 decided the tile **ships** — it was the term the plan named as most likely to drop, and the
rendered grid earned it a place. So this is now an author-facing param whose most natural binding
(a fold count that responds to the music, exactly as `kaleido_order` does) is unavailable, and
nothing warns: the param *is* known, so ADR-0020's unknown-parameter warning cannot catch it, and a
swept value renders a plausible-looking broken picture rather than failing.

### What a fix would be

Quantize `tile` where `spiral` is quantized, and say so in `presets/README.md` alongside the existing
note for the other two. The open question a fix has to answer is what a *transition* between two
integer tile counts should look like — `kaleido_order` has the same problem and solves it with a long
`[smoothing]` constant so the change is rare rather than smooth, which works because the fold count
changes the motif in place. A tile count changes the layout, so the same trick may read as a jump.

### Priority

**Low-medium.** Nothing is broken and the constant binding is honest. It is the gap between "the
stage has five new terms" and "the stage has four new terms you can drive with audio", which is the
difference an author meets on their first attempt.

---

## 0081 — the house gain rule lives only in preset headers *(the first half is FALSIFIED; the exception class survives)*

> **CORRECTED 2026-08-13, at the backlog sweep.** The entry's central claim — *"It is written down
> nowhere. It is not in `presets/README.md`, not in `docs/presets.md`"* — **is false, and was false
> when the entry was written.** `presets/README.md:203` carries it, inside the "A GAIN can be wrong
> the same way" bullet:
>
> > *The rule that came out of the retune: pick `G = C / 0.85` for `bass`/`mid` and `C / 0.60` for
> > `treb`/`onset`, which puts a typical passage near half the cap and a peak at it.*
>
> It landed in `fc698cd` on **2026-08-03** — six days before this entry was raised on 2026-08-09. So
> the rule is not folklore; it is documented, in the file the `preset-author` lane treats as the
> authority, in the paragraph an author reads when composing a clamped band term.
>
> **The second half of the entry stands, unchanged and unbuilt.** The *exception class* is nowhere:
> a param whose cap is a **failure state** rather than a maximum wants its range pulled in at both
> ends instead of gained to reach the cap. Gray-Scott `feed`/`kill` is the worked example — gains
> derived by the house rule put the field in the filled regime, where the gaps close and the picture
> becomes a flat wash with no contour left to draw, and the Coral Oracle's author found this by
> rendering it as flat mustard. Naming the **class** is what makes it useful beyond one preset, and
> it is unlikely to be the only member.
>
> **So this is now a one-paragraph doc item, not a rule-plus-exception item**, and it is half the
> size the entry claims.
>
> **PROMOTED 2026-08-13 → [Plan 0089](plans/done/0089-the-framing-contract-stops-lying.md) Phase 3.** "It
> goes into the next plan that touches `presets/README.md`'s reactivity section" was written here and
> no such plan followed, which is the same no-carrier failure as
> [0078](#0078--kaleido_tile-is-a-discrete-quantity-that-is-not-quantized-so-it-is-the-one-term-of-the-composed-map-an-author-cannot-bind)'s
> survivor; the two ride one plan. **Re-verified against code 2026-08-13** — neither
> `presets/README.md` nor `docs/presets.md` contains any "failure state" or "death state" language, so
> the exception class is still nowhere.
>
> Same method note as [0078](#0078--kaleido_tile-is-a-discrete-quantity-that-is-not-quantized-so-it-is-the-one-term-of-the-composed-map-an-author-cannot-bind): both entries assert an *absence*, and neither absence was
> checked. Everything below is the entry as raised.

**Raised by:** `architect`, at [Plan 0067](plans/done/0067-the-curation-route.md)'s close, from the
`chthonic_coral_oracle.toml` commit. **Owner if taken:** `architect` (the rule is a doc), with
`preset-author` review.

### The finding

This project applies a consistent convention when gaining a band term into a clamped range: derive
the gain from the cap so a typical passage reaches about half of it and a peak reaches it —
`cap / 0.85` for `bass` and `mid`, `cap / 0.60` for `treb` and `onset`. It is applied across the
library and it is the reason a clamp is meaningful rather than decorative.

**It is written down nowhere.** It is not in `presets/README.md`, not in `docs/presets.md`. It
propagates by being copied from one preset header to the next, which is how the Coral Oracle's
author found it and re-derived every gain from it.

And then that preset found a case where the rule is **actively wrong**. For a Gray-Scott regime the
cap is a **death state**: feed high and kill low is the filled regime, where the gaps close and the
picture becomes a flat wash with no contour left to draw. Gains derived by the rule (feed cap 0.059,
kill floor 0.0595) put the field there on a sustained loud passage and rendered as flat mustard. So
`feed` and `kill` are deliberately gentle, with caps pulled inside the labyrinth regime at **both**
ends — a small reactive span on purpose, a drift between two living states rather than a sweep to
the edge of the parameter space.

### Why it is worth an entry rather than a comment

The exception is already recorded, in the one file it applies to. The gap is structural: a
convention that exists only as folklore cannot carry an exception, because the next author meets
the *rule* (copied from a neighbouring preset) without ever meeting the *reason* it has limits.
Both halves are load-bearing for the `preset-author` lane, which keeps no catalogue of its own and
reads `presets/README.md` as the authority.

### What a fix would be

One short section in `presets/README.md`: the rule, the two constants and why they differ, and the
exception class the Oracle names — **a param whose cap is a failure state rather than a maximum**
gets its range pulled in at both ends instead. Gray-Scott `feed`/`kill` is the worked example; it is
unlikely to be the only member of that class, and naming the class is what makes the entry useful
beyond one preset.

### Priority

**Medium.** Cheap, and it converts a piece of folklore plus a one-file exception into a rule an
author can apply and know the limits of.

---

## 0089 — the dragon overruns the frame corner at the default view, and `FRAME_FILL = 0.88` promises it cannot

- **PROMOTED 2026-08-13 → [ADR-0103](adrs/0103-the-ifs-fit-frames-a-figure-that-does-not-turn.md) +
  [Plan 0089](plans/done/0089-the-framing-contract-stops-lying.md) Phase 1.** The correction below named
  **rotation** as the leading candidate; it is now **derived**, and the derivation makes this entry
  much larger than the dragon. `fit_scale` fits the axis-aligned half-extents; a centred AABB rotated
  by `θ` reaches `sqrt(hx²+hy²)`, so with `a = hx/hy` the guarantee holds at every angle only if
  `a <= sqrt(1/FRAME_FILL² − 1) = 0.5397` (vertical-binding) and is **unsatisfiable** horizontal-binding
  at any aspect >= 1. **A square figure overruns by 24.4 %**; only a figure 1.85x taller than wide is
  safe. The fern (`a ~ 0.48`) is the sole compliant shipped figure **and the one the fit was built
  on**, which is why nobody saw it — while **all three** 2D-IFS presets independently bind `spin` down
  *and* set base `zoom` below 1 (0.92 / 0.96 / 0.96), so the library has been paying in triplicate with
  only one header naming why.
- **The entry's own second candidate stays live as the plan's redirect.** If Phase 1's non-vacuity
  check finds the dragon *inside* the bound, rotation is not the mechanism and the finding is the
  preset's own `zoom` reaching `1.04` at a bass peak. The plan says so rather than tuning the test
  until the expected figure fails.
- **What the promotion does not do:** it does not buy the guarantee. The routes that would — fitting
  the rotation-invariant radius, or a per-figure measured fill — are **priced and deferred with a
  trigger** in ADR-0103, because both re-frame all three shipped worlds on top of compensating `zoom`
  values they already carry. What lands is a contract that is true, pinned as a property test, moving
  zero pixels.

> **CORRECTION 2026-08-13, at the backlog sweep — the entry's own first suspect is largely
> exonerated, and it would have sent `dev` hunting in the wrong file.** The entry says *"the
> suspicion to check first is `FitLut` against the fallback `frame()`."* But
> `core/src/render/scenes/particles/ifs/tests.rs:1064`
> (`the_fit_leaves_margin_for_what_it_under_measures`) **already asserts the property this entry
> says is falsified**, for every shipped figure and three morph points on two pairs: it compares the
> fit's sampled half-extent against a 200 000-iteration long run and requires the true figure to
> fill under `0.97` of the frame, with a non-vacuity check that the under-measure is real. That test
> is green, and it covers the dragon.
>
> **So the suspect should be what the fit does not model, and the leading candidate is rotation.**
> The fit measures an **axis-aligned** bounding box; the view transform then **rotates** the figure.
> An axis-aligned box that rotates sweeps its corners outside — which is a *corner* overrun
> specifically, matching the entry's own wording, and it would be invisible to a per-axis fill test
> by construction. `presets/attractor_dragon.toml:117` binds `spin = "sin(time * 0.24) * 0.26"`, so
> the shipped world does rotate. Its `zoom` line directly below is the `0.92` workaround.
>
> A second candidate worth eliminating in the same sitting: the same file's `zoom` reaches
> `0.92 + bar * 0.04 + clamp(bass * 0.094, 0, 0.080)` — **1.04 at a bass peak**, past 1.0 — so part
> of what was seen may be the preset zooming in rather than the fit mis-measuring.
>
> **This does not close the entry** — a stated invariant is still falsified and this project does not
> leave one standing. It redirects it, and it keeps the entry's own good instruction: do not close
> this by re-documenting the invariant away without knowing which mechanism breaks it.

**Raised by:** `preset-author`, Plan 0075 cohort 5, at 1280x720 — the reference aspect, not an
odd target. **Owner if taken:** `dev` — the suspicion to check first is `FitLut` against the
fallback `frame()` in the IFS fit path.

### The finding

`FRAME_FILL = 0.88` documents that the fitted figure stays inside the frame with margin. The
dragon (two maps at exactly 0.7071, space-filling) overruns the frame **corner** at the default
view; the shipped world works around it with `zoom = 0.92`. Small — but it contradicts a
documented invariant, and this project does not leave a falsified stated property standing.
The first question is which of the two fit sources mis-measures the dragon's extent: the
`FitLut` or the fallback `frame()`.

### Handoff verdict (2026-08-11): park, take opportunistically

Nothing ships broken — the workaround is one line and honest, in the world's own header. Take
it the next time anyone is in the IFS fit path, and do not close it by re-documenting the
invariant away without knowing which source mis-fits.

---

# Archived 2026-08-15 — at Plan 0085's close

Two entries, both discharged by [Plan 0085](plans/done/0085-the-show-length-horizon-gets-an-instrument.md).
Their sibling **[0083](design-backlog.md) stays live** — the plan gave it the `switches` instrument
it needs (Phase 3) and not the three paired runs it asks for (Phase 5, `human`, standing), so it is
half discharged and a half-discharged entry does not move.

---

## 0082 — the quality governor reads `frame_ms_p99`, and a preset switch spikes p99 to 25 ms while nothing is dropped

- **PROMOTED 2026-08-13 → [ADR-0099](adrs/0099-the-show-length-horizon-is-a-spot-check-and-it-splits-in-two.md) +
  [Plan 0085](plans/done/0085-the-show-length-horizon-gets-an-instrument.md) Phases 3-4**, jointly with
  [0083](#0083--rss-grew-385-to-663-mb-over-three-minutes-of-preset-switching-and-there-is-no-no-feedback-control-to-compare-it-against)
  and [0086](#0086--no-capture-path-reaches-the-minutes-long-horizon-so-a-slow-accumulation-failure-is-invisible-to-every-instrument):
  three entries about the show-length timescale nothing in this repo measures. **The plan does not
  choose among this entry's three candidate responses** — that stays R0's decision, exactly as
  written below. What it does is add a steady-state frame-time column beside `p99` and record the
  qualification where R0's designer will meet it before they meet the column, which is this entry's
  own "the cheapest time to know is before the governor exists".
- **CLOSED 2026-08-15** (Plan 0085 Phases 3-4, `97b7227` + `9514e2b`). The qualification is
  recorded in all three places a governor design starts from — `docs/nfr.md` §1 beside the frame
  budget, roadmap item 3 in the plans index, and R0's own entry in `docs/roadmap-visual-richness.md`
  — and **the three candidate responses below are named in each and deliberately not chosen**, which
  is what this entry asked for. `--soak` gained `frame_ms_p99_steady` (the same statistic with the
  frames following a switch or reconfigure left out) beside the raw `frame_ms_p99` and a monotone
  `switches` counter, so the third response can be measured rather than argued. It is a reading, not
  a gate; nothing demotes on it.
- **AND THIS ENTRY'S CENTRAL PREMISE WAS FALSE, WHICH IS THE MORE USEFUL HALF.** Two claims below
  did not survive being checked against the code at Plan 0085 Phase 4, and both had stood unchecked
  since 2026-08-09:
  - **"the adaptive-quality governor … (roadmap item 3 / R0, **not yet built**)" — it was built.**
    [Plan 0044](plans/done/0044-quality-tiers.md) / [ADR-0045](adrs/0045-quality-tiers-floor-and-rich.md)
    delivered tiers *and* the governor on **2026-07-30**, ten days before this entry was raised and
    two weeks before it was promoted. The plans index still said they "remain for a later plan" six
    weeks after they landed, which is where the belief came from.
  - **"is specified to read p99" — it does not read p99 at all.** `core/src/render/tier.rs`'s
    `sustained_miss` counts what fraction of the **raw frame-time series** exceeds
    `budget * MISS_FACTOR` (1.25) and demotes only when `MISS_FRACTION` (0.75) of at least
    `MIN_SAMPLES` (180) miss. A preset switch contributes a handful of slow frames to a 240-sample
    ring, which cannot approach that fraction — so **the shipped governor already landed this
    entry's second candidate response**, independently, in the form of a miss *fraction* rather than
    consecutive windows.

  The hazard the measurement documents is therefore real and lives in the **prose**, not the code:
  the roadmap item and this entry both described a governor that reads p99, and a revisit starting
  from those sentences would build the thing being warned about. That is what the three paragraphs
  now say. **Read this as method, not as outcome** — the entry's symptom half (the numbers) was
  right and is reproduced below unchanged; what rotted was its claim about *the repo*, which is the
  same failure mode the 2026-08-13 sweep found in 0078 and 0081.

**Raised by:** `architect`, at [Plan 0046](plans/done/0046-transformed-feedback.md)'s close, from
that plan's Phase 5 measurement. **Owner if taken:** `dev`, after an `architect` call on what the
governor should do about it.

### The finding

Three minutes of the standalone at Rich tier, 1080p, two feedback presets with switching and a
fullscreen toggle — 158 audible 1 Hz rows:

| | value |
|---|---|
| fps median / min | 165.0 / **114.3** |
| rows below the NFR §1 60 fps floor | **0 of 158** |
| `frame_ms_avg` median / max | 6.061 / **8.749** ms |
| `frame_ms_p99` median / max | 6.866 / **25.037** ms (p95 of that column: 18.1) |
| frames dropped | **0 of 28 698** |

So the frame budget holds with roughly 2.7x headroom and the transform costs nothing measurable.
**But `frame_ms_p99` exceeds the 16.67 ms budget while `frame_ms_avg` never passes 8.7 ms and no
frame is dropped.** The spikes coincide with preset switches and the fullscreen toggle — GPU
resource rebuilds, not steady-state cost.

### Why it is worth an entry

**The adaptive-quality governor is specified to read p99** (roadmap item 3 / R0, not yet built). A
demotion decision reading this column would see 25 ms during a preset switch and demote a preset
that is in fact running at 165 fps — and demotion changes what the audience sees, on the one event
that is already visually disruptive. The measurement is not a defect; the *instrument the governor
will read* is what needs the qualification, and the cheapest time to know is before the governor
exists.

### What a fix would be

One of three, and choosing is the design question: exclude the frames following a preset switch or
a surface reconfigure from the governor's window; make the governor require N consecutive bad
windows rather than one; or give it a separate steady-state statistic and leave p99 as the
diagnostic it is today. Whatever is chosen, this measurement is the test case.

### Priority

**Medium — but it must be read before R0's governor is designed**, not after.

---

## 0086 — no capture path reaches the minutes-long horizon, so a slow-accumulation failure is invisible to every instrument

- **PROMOTED 2026-08-13 → [ADR-0099](adrs/0099-the-show-length-horizon-is-a-spot-check-and-it-splits-in-two.md) +
  [Plan 0085](plans/done/0085-the-show-length-horizon-gets-an-instrument.md) Phases 1-2** — and promoted
  **as the shape this entry specified**, not as the gate it refused: a headless `shot` mode rendering
  N simulated minutes at capture cadence and reporting drift statistics, run by the lane on worlds
  with an accumulation axis, verdict in the world's header. The ADR adds one thing the entry did not
  separate: **this half is deterministic and reproducible headless** (forces integrate against
  injected `dt`), while [0083](#0083--rss-grew-385-to-663-mb-over-three-minutes-of-preset-switching-and-there-is-no-no-feedback-control-to-compare-it-against)'s
  RSS growth comes from live GPU resource churn no headless loop reproduces — so they share a
  motivation and **not** an instrument, and one harness for both would be blind to two of the three.
  The 2026-08-11 park-with-a-trigger below is discharged by the trigger having fired.
- **CLOSED 2026-08-15** (Plan 0085 Phases 1-2, `3280136` + `a1e62e5`). `shot --horizon <minutes>`
  renders N **simulated** minutes at the fixed capture step and prints one row per interval —
  coverage, `peak/mean` concentration, footprint motion — plus a `delta`/`monotone` trend per
  statistic, no threshold anywhere, and a line in its own output saying it is not a gate. Built as
  the shape this entry specified: run by the lane, verdict in the world's header. Both determinism
  properties are asserted on rendered pixels through the CLI (two runs byte-identical; a short run
  row-for-row prefixes a long one), with a static control world reading `delta 0.0000` on all three
  statistics as the non-vacuity half.
- **The named subject came back clean, and a world nobody suspected did not.** Seven worlds with an
  accumulation axis went through it (Phase 2). `swarm_shatter` — the world whose live collapse
  raised this entry — reads monotone 0.50 and wanders 0.197-0.384 across ten minutes with **no
  trend**: it was rebuilt at engine-default dynamics and [Plan 0077](plans/done/0077-the-quiet-sky.md)
  gave the swarm its `reseed`, so the known instance is repaired. What the instrument convicted
  instead is **`attractor_ink`**: coverage 0.199 -> 0.002 over ten minutes with the **silhouette
  intact and the density gone**, concentration climbing to 1161, reproduced at two capture sizes and
  under silence. Recorded in that preset's header, deliberately unrepaired — judging a look is
  content-lane work. That is this entry working exactly as it argued it would.
- **One caveat the entry could not have anticipated, and it bounds the instrument.** The headless
  capture path dies at 3,601 frames on both reaction-diffusion worlds after RSS reaches ~2.9 GB, so
  those two were measured at 0.5 simulated minutes rather than 10. **Pre-existing** — the shipped
  `capture_preset` fails identically at the same frame count, run as the control before it was
  called a finding — and filed as [0093](design-backlog.md#0093--the-headless-capture-path-dies-past-a-few-thousand-frames-so-the-horizon-cannot-reach-its-own-headline-length).
  Until that is fixed, "N simulated minutes" is bounded by world.

**Raised by:** `preset-author`, Plan 0075 cohort 4, watching Shatter collapse live three times
while the suite stayed green. **Owner if taken:** `architect` first — this is a methodology
question before it is code.

### The finding

`shot` strips and all five behavioral gates live in the first seconds of a preset's life (the
synthetic gates capture 30 frames at 1/60 s; even the PCM-driven reactivity gate measures a few
seconds of hops). Shatter's collapse developed over **minutes** of sustained force. Nothing
went red and nothing was flagged — the suite is structurally blind to any failure whose
timescale is the show rather than the capture. The class is real and now has a named member:
any slow-divergence look (accumulating forces, feedback with net gain, populations that
migrate) can ship green and die on stage.

Adjacent, not identical:
[0083](#0083--rss-grew-385-to-663-mb-over-three-minutes-of-preset-switching-and-there-is-no-no-feedback-control-to-compare-it-against)
wants a long-horizon RSS measurement for the same regime (hours-long live shows); the two would
share a soak recipe.

### What a fix would be

**Not a gate.** A minutes-long capture per preset is not a price this suite can pay — see
[0080](design-backlog-archive.md#0080--the-reactivity-gate-pays-18x-to-render-frames-it-throws-away-because-warm-up-and-measurement-share-one-capture-path)
for what the *seconds* already cost. The honest shape is a documented soak-style spot-check: a
`shot` mode or recipe that renders N minutes at capture cadence and reports drift statistics
(population spread, deposit concentration), run by the lane on worlds whose mechanism has an
accumulation axis, with the verdict recorded in the preset header the way the fold-edge
verdicts were.

### Handoff verdict (2026-08-11): park, with a trigger

Park until someone ships a look with a slow-accumulation axis — and that trigger explicitly
includes acting on
[0085](#0085--swarm-has-no-reseed-so-a-flow-field-pile-up-has-no-recovery-lever), which must
not ship its lever verified only at the horizon the suite can see.

**Trigger status (2026-08-11, same day):** [Plan 0077](plans/done/0077-the-quiet-sky.md) acts on
0085 and its Phase 5 carries the bounded check this entry's "What a fix would be" prescribes —
one minutes-horizon observation, verdict recorded in the world's header. The entry itself
stays **parked**: no instrument is built, and the trigger re-arms for the next
slow-accumulation look. (2026-08-12: the plan's `dev` scope closed; the bounded check travels
with its Phase 5, standing in the plans README — this entry's status is unchanged.)

---

## 0083 — RSS grew 385 to 663 MB over three minutes of preset switching, and there is no no-feedback control to compare it against

- **PROMOTED 2026-08-13 → [ADR-0099](adrs/0099-the-show-length-horizon-is-a-spot-check-and-it-splits-in-two.md) +
  [Plan 0085](plans/done/0085-the-show-length-horizon-gets-an-instrument.md) Phase 5** — the measurement
  this entry asks for, not a fix, and it stays a `human` phase because it needs the live app on a
  real machine for a real duration. One thing the plan adds that this entry did not ask for and
  needs: **`--soak` has no notion of a preset switch**, which is the axis the whole question turns
  on, so a `switches` column lands first (Phase 3) and the three runs read against it. **Either
  answer closes this entry** — the entry's complaint is the missing control, not the number.
- **HALF DISCHARGED 2026-08-15, at Plan 0085's close — and it stays live, which is the point.** The
  instrument landed and the measurement did not. `--soak` now writes a monotone `switches` counter
  (preset changes **and** surface reconfigures, so the difference between two rows is how many
  happened between them) beside the existing `rss_bytes` column, which is exactly the axis this
  entry said was missing. **The three paired runs are Phase 5, `human`, and have not been made** —
  they need the live app on a real machine for a real duration, so they carry forward under Standing
  in [`docs/plans/README.md`](plans/README.md) rather than holding the plan open. Its two sibling
  entries, [0082](design-backlog-archive.md#0082--the-quality-governor-reads-frame_ms_p99-and-a-preset-switch-spikes-p99-to-25-ms-while-nothing-is-dropped)
  and [0086](design-backlog-archive.md#0086--no-capture-path-reaches-the-minutes-long-horizon-so-a-slow-accumulation-failure-is-invisible-to-every-instrument),
  were discharged in full and are archived. **Nothing about the finding below has been re-measured**,
  so read it as the 2026-08-09 snapshot it is. One thing that *has* changed underneath it:
  [0093](#0093--the-headless-capture-path-dies-past-a-few-thousand-frames-so-the-horizon-cannot-reach-its-own-headline-length)
  found the **headless** capture path climbing to ~2.9 GB over 3,600 frames — a different process on
  a different path, and not evidence about this one, but a reason to run the control here rather
  than assume the growth is the driver's.
- **CLOSED 2026-08-15 — the three runs were made, and the answer is the bounded direction.**
  Plan 0085 Phase 5, run on windows x86_64 with a hardware adapter, release build, 165 Hz,
  windowed, `tier = "rich"`, four presets per set rotating on a fixed 20 s dwell:

  | run | presets | duration | switches | RSS first -> last | delta | range |
  |---|---|---|---|---|---|---|
  | 1 | feedback (`curve_ionwake`, `fragment_whorl`, `reaction_mitosis`, `reaction_verdigris`) | 1196 s | 62 | 382.6 -> 367.2 MB | **-15.4** | 359.6-390.1 |
  | 2 | no-feedback control (`fragment_strata`, `lsystem_vellum`, `spectrum_halo`, `swarm_shatter`) | 1196 s | 62 | 379.9 -> 380.1 MB | **+0.2** | 379.8-380.2 |
  | 3 | feedback, no switching (3 startup reconfigures only) | 1797 s | 3 | 379.7 -> 328.0 MB | **-51.7** | 328.0-389.1 |

  **No run grew.** The 385 -> 663 MB does not reproduce; run 3 *fell* 52 MB over half an hour. The
  control is what makes that readable: run 1 oscillates across a ~30 MB band while run 2 sits
  inside 0.4 MB, so **feedback presets do cost real per-switch memory churn and it is recovered
  every switch rather than accumulated** — which is precisely the "per-switch cost, bounded"
  direction this entry named as one of its two closing answers. NFR §12's hard requirement (no
  monotonic growth across a session) holds.

  **Read the caveats as bounding the claim, because they are real.** No audio was playing, so the
  band-driven half of the workload is absent and this is a *lighter* load than the one that
  produced the original number; the runs were windowed and never fullscreen, so the surface
  reconfigure that dominated the original observation never happened; the preset sets and the
  refresh rate both differ. What the runs establish is that **switching feedback content for 20
  minutes does not accumulate**, which is the question the missing control blocked. A fullscreen,
  audio-driven repeat would be a stronger claim and nobody is blocked on it.

  **One thing the runs found that this entry did not ask about, and it falsifies a claim in its
  archived sibling** — filed as
  [0094](design-backlog.md#0094--the-frame_ms_p99-tail-is-not-switch-correlated-so-the-steady-state-column-does-not-remove-it).

**Raised by:** `architect`, at [Plan 0046](plans/done/0046-transformed-feedback.md)'s close, from
that plan's Phase 5 measurement. **Owner if taken:** `dev` (a measurement first, not a fix).

### The finding

Over the same three-minute Phase 5 run, resident set grew **385 MB to 663 MB** (max 663), against
the ~327 MB driver-dominated floor [ADR-0010](adrs/0010-accept-gpu-driver-memory-floor.md)
established and the NFR §12 working-set target.

**This is not yet evidence of a leak, and it is important not to record it as one.** Three minutes
is short, the run switched presets repeatedly (each switch builds a side's GPU resources), and this
plan adds two accumulation buffers, so *some* growth is expected. What makes it worth keeping is
the other half: **it was never measured against a no-feedback control**, so nothing separates
"expected cost of what landed" from "growth that does not stop".

### Why it is worth an entry

R6 ([Plan 0075](plans/done/0075-the-content-renaissance.md)) will ship feedback presets, and the live-show
use case runs for hours — the exact regime three minutes cannot speak to. A number with no control
beside it is the kind of observation that gets quoted later as either a clean bill of health or a
known leak, depending on who is quoting it, and it supports neither.

### What a fix would be

The measurement, not a change: two runs of equal length and equal switching, one on feedback
presets and one on a no-feedback control, reading the same RSS column — and one longer run
(tens of minutes, no switching) to separate per-switch cost from monotone growth. Only if the
control run also climbs is there something to fix.

### Priority

**Medium.** Cheap to run, and it is owed *before* R6 ships long-running feedback content.

---

## 0068 — a swarm mark has no per-mark variation, so the only scene that can hold a starfield cannot make one twinkle

- **CLOSED 2026-08-15**, both options, at [Plan 0090](plans/done/0090-the-emitters-source-moves.md)'s
  close. Option 1 landed at [Plan 0077](plans/done/0077-the-quiet-sky.md) Phase 2; option 2 — the
  source — landed as Plan 0090 Phases 1-4. **The engine ask is discharged in full**: `source_y`,
  `source_width`, `spawn_fade` and `prewarm` are preset-reachable, every default an exact
  arithmetic identity (the golden suite passes against the committed baselines, not only re-blessed),
  and `presets/README.md`'s "no positionable source" paragraph is retired rather than reworded.
  **The entry named ONE warm-up and there were two**, which is the finding the plan added and then
  measured: moving the source kills the *travel* warm-up this entry measured (2.12 units against a
  0.5 s capture) and `prewarm` kills the *population* one. Measured on a slow drifting draft,
  `prewarm = 0` against `prewarm = 1` — `sanity` coverage `0.0074` (0 of 10 radial shells,
  convicted **blank**) against `0.1470` (10 of 10, structurally present); `animation` footprint
  motion `0.0629` against `0.1702`; `reactivity` best band `0.0002` against `0.0195`.
  **The entry's refusal was honoured**: no gate's capture length, floor or statistic moved.
- **What the measurement corrected in this entry's own reasoning.** The plan expected the *animation*
  gate to be the wall for a sparse slow look. It is not — ADR-0091's footprint statistic passes the
  draft **cold**, at `0.0629` against a `0.01` floor. The wall is `sanity`, and `prewarm` is
  what clears it. Reactivity is the one still short (`0.0195` against `0.02`, 97.5 % of it) on a
  draft nobody tuned for that gate, which reads as content tuning rather than an instrument problem.
- **What is NOT in this closure, deliberately: the world itself.** Plan 0090's `human` Phase 5 —
  a quiet drifting field and a point fountain, judged in the running app — stands under **Standing**
  in `docs/plans/README.md`. It is content work on a delivered surface, not an undischarged half of
  this entry: what this entry asked the engine for exists.
- **The probe this entry carried did not fire, and that is worth one line.** Its 2026-08-15
  verification bullet claimed `present: SOURCE_Y: f32 = -1\.12` was *"designed to go red the day
  that lands"*. Phase 1 renamed the constant to `DEFAULT_SOURCE_Y`, which still **contains** the
  substring, so `check-backlog-claims.mjs` stayed green through the exact delivery it was written
  to detect. **Anchor a probe on the line, not on a bare identifier** — otherwise a rename reads as
  continuity.
- **Raised:** 2026-08-05, from `preset-author`, during [Plan 0070](plans/done/0070-shaped-marks.md)
  Phase 6 — the starfield the whole plan was built for.
- **Verified by measurement:** yes. The emitter draft was rendered and gated before being discarded;
  the numbers below are from that run, not from reading the code.
- **Verified 2026-08-15** — option 2 is still open, and this entry's own dated correction still
  holds: `present: SOURCE_Y: f32 = -1\.12 in: core/src/render/scenes/emitter.rs`,
  `present: source_half_width in: core/src/render/scenes/emitter.rs`,
  `present: launch_speed = "2\.6" in: presets/emitter_perseids.toml`.
  [Plan 0090](plans/done/0090-the-emitters-source-moves.md) turns the first of these into an authorable
  scalar, so it is designed to go red the day that lands — at which point option 2 is delivered and
  the entry is the architect's to close.

**The scene with the right individuation cannot hold the look, and the scene that can hold it has no
individuation.** `emitter` carries `twinkle`, whose *rate and phase both* come off each object's own
seed, so a field shimmers while the frame's total light sits still — which is exactly what a sky of
stars does and exactly what a whole-field brightness term cannot fake. It also carries
`size_spread`. It is unusable for a starfield anyway, and the reason is geometry rather than taste:
its source line is fixed at `y = -1.12` and cannot be moved, so a star must travel 2.12 units to
cross the frame. A drift slow enough to read as a sky (~0.85 units/s) needs ~2.5 s to fill it, and
**every behavioral gate in the suite captures 30 frames at 1/60 s, which is 0.5 s** — so the gate
sees an empty sky. Measured: the emitter draft reported cover `0.013` and `0.000` on all four bands.
Speeding it to the ~4.3 units/s the geometry demands is a rising shower rather than a starfield, and
the twinkle stops reading at that speed regardless.

`swarm` has the opposite profile. Its population is fixed and present from frame one, so it has no
warm-up at all and the gates see it immediately — which is why `swarm_starfield` ships on it. What
it has no way to express is **per-mark** anything: a `brightness` or `size` binding moves the entire
field together, so the shipped preset's shimmer is a slow global breath plus a beat flash, and the
per-star life the look actually wants is simply absent.

### What would close it

Either half would; they are independent and the first is much smaller.

1. **Per-mark variation on the swarm** — a `twinkle` and a `size_spread` in the emitter's shape,
   driven off the existing per-particle seed. The swarm already draws a seeded per-particle `size`
   factor and a depth scale, so the machinery to individuate is there; nothing exposes a *bound*
   parameter through it.
2. **A movable or point source on the emitter** — the fixed line is recorded separately in
   [0060](#0060--an-engine-fix-leaves-its-preset-side-workarounds-standing-and-only-a-header-comment-remembers-them)'s
   neighbourhood as "no positionable source". Closing that would make the emitter reachable for
   slow-drift looks, and this entry is a second, independent reason to want it.

**Not the answer:** raising the gate's capture length. The gates are 0.5 s by design and a preset
that needs 2.5 s of warm-up to look like anything is also a preset that looks like nothing for the
first 2.5 s of a live show.

### Priority

**Medium.** One preset ships today with a documented compromise rather than a defect, so nothing is
broken. But the emitter's `twinkle` is the single most-cited example of per-object life in the whole
parameter surface, and the scene that most wants it cannot reach it.

### Update 2026-08-11 — a second, independent want, and this time it was the whole look

- **Raised by:** `preset-author`, Plan 0075 cohort 4: the quiet twinkling starfield (the
  Perseids look). Sparse marks, low coverage, slow shimmer — the shimmer half is exactly this
  entry's option 1 (per-mark variation on the swarm), and the look was **routed out of the
  cohort rather than shipped**. The gate half of the same casualty is recorded in
  [0009](#0009--the-animationrs-gate-penalizes-two-legitimate-designs-informational)'s update
  of the same date.
- **Handoff verdict (2026-08-11): promote** — option 1, jointly with 0009's coverage-aware
  statistic as the sparse-idiom pair (fixing either wall alone leaves the look unreachable),
  and with [0085](#0085--swarm-has-no-reseed-so-a-flow-field-pile-up-has-no-recovery-lever)
  (swarm `reseed`) riding the same plan.
- **PROMOTED same day → [Plan 0077](plans/done/0077-the-quiet-sky.md) Phase 2** (option 1; the
  gate half is that plan's Phase 1 via
  [ADR-0091](adrs/0091-the-animation-gate-scores-motion-against-the-figures-footprint.md)).
  Option 2 — the emitter's movable source — stays open in this entry, unpromoted.
- **DELIVERED 2026-08-12 (Plan 0077 Phase 2, `fae16e6`).** The swarm carries `twinkle` and
  `size_spread` with the emitter's names and semantics — **rate and phase both off the seed**,
  so the field shimmers while the whole-frame mean sits still, the exact property this entry
  measured the emitter for. Both default 0 and the goldens pass unblessed (byte-identity by
  arithmetic, not by bless). The quiet sky itself is Plan 0077 Phase 5, standing in the plans
  README.

### Option 2 — PROMOTED 2026-08-13, by interview

- **→ [ADR-0104](adrs/0104-the-emitters-source-is-authorable-geometry.md) +
  [Plan 0090](plans/done/0090-the-emitters-source-moves.md).** Four scalars, every default an exact
  arithmetic identity: `source_y` (which **may sit inside the frame** — the interview took that
  trade deliberately, because clamping it below is the decision that keeps the emitter unusable for
  any slow look), `source_width` (**fractional**, so `aspect * 1.0` is bit-for-bit today's value; `0`
  is a point source, which falls out rather than being its own concept), `spawn_fade`, and `prewarm`.
- **The internals were already the right shape**, which is why this is smaller than the entry implies:
  `source_half_width` is a real field on `Spawn` (`emitter.rs:357`) assigned `self.aspect`
  unconditionally, and the spawn site already multiplies a unit draw by it. One constant and one
  assignment.
- **This entry named ONE warm-up and there are TWO, which is the finding the promotion adds.** Moving
  the source into the frame removes the *travel* warm-up this entry measured (2.12 units against a
  0.5 s capture) and leaves the *population* warm-up untouched: the pool starts empty and fills at
  `spawn_rate`, so Perseids' own numbers put ~100 of ~560 objects on screen at 0.5 s — **about
  18 %** — wherever `source_y` is. `prewarm` is Plan 0090 Phase 3 and is the phase that actually
  makes a slow world gateable; it is also the plan's designed cut point, being beyond what the
  interview covered.
- **The entry's refusal is honoured**: no gate's capture length, floor or statistic moves. The
  warm-up is attacked instead of the instrument.
- **CORRECTION — this entry's own claim that the Perseids look was "routed out of the cohort rather
  than shipped" is stale.** `presets/emitter_perseids.toml` exists on `system = "emitter"` at
  `launch_speed = 2.6`: it shipped as exactly the fast-shower compromise this entry predicted would
  be the only reachable form. What did not ship is the quiet version, which is Plan 0090 Phase 5.
  The demonstrated want is therefore stronger than the entry records, not weaker.


---

## 0093 — the headless capture path dies past a few thousand frames, so the horizon cannot reach its own headline length

- **CLOSED 2026-08-16** at [Plan 0099](plans/done/0099-the-horizon-reaches-its-own-length.md)'s
  close, all three phases. **The candidate mechanism was right and the entry's own framing of the
  ceiling was wrong twice over**, which is why the body below is kept rather than summarized.
  `step_offscreen` never polled — that half held exactly, and the repair is the one line this entry
  named: `poll(wait_indefinitely)` per frame, at ~1.5x wall clock on an RD world, after which all
  three RD worlds render `--horizon 10`'s full 36,001 frames with the resident set flat
  (398.8 MB at 4 s to 399.6 MB at 54 s, against 419 MB to 4,402 MB and dead before).
- **It was never a frame ceiling.** This entry reports **3,601 frames** as the wall and the plan
  inherited that number. Re-measured, all three worlds clear **5,401** and fail at **7,201**, one
  unfixed run died at 4.4 GB and another reached 36,001 at 4.9 GB. The wall is memory pressure, so
  "3,601" was a reading of one machine's headroom on one afternoon.
- **And it was not the RD family's mechanism either — the discriminator this entry named answered a
  question neither of its two readings asked.** `reaction_etching` fails like its siblings, so the
  family reading holds; but retention is per **pass**, not per pixel or per world. Over one unpolled
  stretch an RD world (12 simulation sub-steps plus a present, 13 passes a frame) retained **950 KB
  a frame** against a 36 KB captured frame, where single-pass worlds retained ~30 KB. Every world
  grew. RD reached the allocator first, which is what made the family look responsible.
- **The `absent: poll` probe went red on delivery, exactly as this entry designed it to.** That is
  the [ADR-0108](adrs/0108-a-backlog-claim-about-the-repo-carries-an-executable-probe.md) grammar
  working: the probe was the hypothesis, and its own falsification is the signal that the entry is
  finished.
- **What the closure does not cover**, deliberately: the truncated horizon rows Plan 0085 recorded
  are still truncated. Re-running them is an instrument *use* and belongs to whoever next asks a
  show-length question.

> **PROMOTED 2026-08-16** — [Plan 0099](plans/done/0099-the-horizon-reaches-its-own-length.md), whose
> Phase 1 is exactly the `reaction_etching` run this entry names as the cheapest discriminator.

**Raised by:** `architect`, at [Plan 0085](plans/done/0085-the-show-length-horizon-gets-an-instrument.md)'s
close, from that plan's Phase 2 findings. **Owner if taken:** `dev` — but read the mechanism below
first, because the candidate cause is one line and the entry may be cheaper than it looks.

- **Verified 2026-08-15** — the candidate mechanism is still exactly as stated below, and still
  unfixed: `present: fn step_offscreen in: core/src/render/capture_api.rs`,
  `absent: poll in: core/src/render/capture_api.rs`. The second probe is the whole hypothesis in one
  line — the day anyone polls per frame in that file it goes red, which is the right moment to
  re-read this entry whether or not the fix worked.
- **Verified 2026-08-15, AND IT CONVICTED THIS ENTRY — reported by `dev` at Plan 0093 Phase 2,
  corrected in place by `architect` at that plan's close.** The finding said *"Both shipped
  reaction-diffusion worlds (`reaction_mitosis`, `reaction_verdigris`)"*. **There are three:**
  `present: system = "reaction_diffusion" in: presets/reaction_etching.toml`. `reaction_etching`
  landed in `6ebec33` on **2026-08-10**, five days before this entry was written, so this is a
  **birth defect** — the class
  [ADR-0108](adrs/0108-a-backlog-claim-about-the-repo-carries-an-executable-probe.md) exists for,
  found by the first pass that read the entry against the tree, by the instrument that pass was
  building. It is corrected rather than closed because the finding it distorts is untouched: the
  capture path still dies at 3,601 frames and the candidate mechanism is still one line. What the
  correction costs the entry is its **scope argument**, and that half is now stated as the open
  question it always was — see *Why it is worth an entry* below.

### The finding

**Two of the three shipped reaction-diffusion worlds** (`reaction_mitosis`, `reaction_verdigris`)
fail at **3,601 frames** with `Buffer with 'lmv-capture-readback' label is invalid`, after the
process's resident set climbs to **~2.9 GB**. The third, `reaction_etching`, **was never run** —
it was shipped before this entry was written and was simply missed.  Measured on the Windows development box, hardware adapter, debug build,
at 96x96 — the capture size is not the lever, since 2.9 GB is four orders above what 3,600 frames of
96x96 RGBA would be.

**It is pre-existing and not a defect in Plan 0085's new sampling primitive.** The shipped
`Renderer::capture_preset` fails identically at the same frame count on the same preset, which was
run as the control *before* this was called a finding. It is only visible now because nothing in
this repo had ever driven a world for thousands of frames: the four synthesized gates capture 30,
the reactivity gate a few hundred.

### Why it is worth an entry

**It bounds the instrument Plan 0085 just shipped.** `shot --horizon 10` is documented as ten
simulated minutes — 36,001 renders — and on two shipped worlds it cannot get past 3,601. Those two
rows in the plan's Phase 2 table are therefore a **0.5-minute** horizon, and their `monotone 1.00`
is a world still settling into its pattern rather than drifting: a horizon shorter than a world's
own warm-up reads settling *as* drift, which is precisely the misreading the instrument exists to
prevent.

**How wide the ceiling is, is open — and the entry originally overstated how well that was known.**
Every world in the *measured* set other than these two cleared 36,001 renders, which is why this
reads as a family ceiling rather than a general one. But the measured set was not the roster: it
omitted `reaction_etching`, the third RD world, so the family evidence is two of three and nobody
has run the member that would confirm it. Two readings survive that and the entry does not choose
between them — a mechanism ceiling specific to RD's heavy per-frame ping-pong, or a *cost* ceiling
that any sufficiently expensive world reaches and RD reaches first. **Running `reaction_etching`
separates them, and it is the cheapest thing anyone can do here** — it costs one `shot --horizon`
and it decides which of the two the fix has to answer.

It is also **adjacent to, and not the same as,**
[0083](design-backlog-archive.md#0083--rss-grew-385-to-663-mb-over-three-minutes-of-preset-switching-and-there-is-no-no-feedback-control-to-compare-it-against):
that is the *live app's* resident set under preset switching, this is a *headless offscreen loop*
that never rebuilds a surface. If they share a cause it would be worth knowing, and nothing
currently says they do.

### What a fix would be — with a candidate mechanism, stated as unverified

`core/src/render/capture_api.rs:481` — **`step_offscreen` creates a command encoder, submits it, and
never polls.** `capture::read_back` holds the only `device.poll` anywhere in the capture path, so
between two sampled frames wgpu has no opportunity to release the transient resources each
submission retains. At a 60-second interval that is **3,600 consecutive unpolled submits**. All
three capture entry points (`capture_preset`, `capture_preset_over`, `capture_preset_at`) funnel
through `step_offscreen`, which is exactly why the control failed the same way — and it would
explain why an RD world, whose per-frame ping-pong is the heaviest in the engine, hits it first.

**This is a hypothesis and nobody has run it.** The check is cheap: poll once per frame in
`step_offscreen` and re-run `shot --horizon 10` on `reaction_mitosis` — and on `reaction_etching`,
which has never been run at all and is what decides whether the ceiling is the family's or the
cost's. If the RSS trace flattens,
the fix is one line and the entry closes with a measurement; if it does not, the diagnosis is wrong
and the real one starts from a GPU memory report rather than from this paragraph. **Do not close
this by lowering a documented horizon** — the instrument's stated length is what makes a recorded
header verdict comparable across worlds.

### Priority

**Medium.** Nothing ships broken — this is a QA path, not a runtime one, and the live app polls
every frame through its own present. But it silently truncates the only instrument this project has
for show-length behaviour, on the family that most needs it, and the truncation reads as a *result*
(`monotone 1.00`) rather than as an error unless someone reads the run's stderr.

---

## 0106 — converted MilkDrop presets wash out or invert: the float feedback field never truncates

> **CLOSED 2026-08-17** — [ADR-0118](adrs/0118-the-milkdrop-feedback-field-quantizes-in-the-encoded-domain.md)
> + [Plan 0108](plans/done/0108-the-milkdrop-import-gets-its-tone-back.md). The fix shipped and it
> works: the field reaches exact zero, and under a dynamic signal a preset that dissolved to flat
> white now holds its shading. The re-judge ran and came back **still merely different**.
> **This entry's central claim is falsified in closing.** It said one mechanism with four
> presentations — pastel wash, white-hot glow, runaway with fringing, tonal inversion. On the five
> pairs with no video echo the background sits three orders of magnitude above the quantizer's floor
> (linear `3.03e-4`), so at most one of the four is truncation; the "inversion" is a mid-grey
> non-additive waveform drawn over an already-washed ground. **The residual wash is a new entry,
> 0113 — not this one reopened.**

> **CLOSED 2026-08-17** — [ADR-0118](adrs/0118-the-milkdrop-feedback-field-quantizes-in-the-encoded-domain.md)
> + [Plan 0108](plans/done/0108-the-milkdrop-import-gets-its-tone-back.md). The fix shipped and works;
> the re-judge ran and came back **still merely different**. **This entry's central claim is
> falsified in closing:** it said one mechanism with four presentations, and the look gate found the
> wash sits three orders of magnitude above the quantizer's floor on five of seven pairs, so at most
> one of the four is truncation. The residual wash is a **new** entry, 0113 — not this one reopened.

- **PROMOTED 2026-08-17 → [ADR-0118](adrs/0118-the-milkdrop-feedback-field-quantizes-in-the-encoded-domain.md) +
  [Plan 0108](plans/done/0108-the-milkdrop-import-gets-its-tone-back.md) Phases 1-2.** The entry's own
  design call — what "8-bit truncation" means in linear light — is answered with the arithmetic it
  asked for: one 8-bit sRGB step is `3.03e-4` in linear, so the literal `1/255` floor the entry warns
  against truncates everything below sRGB level ~13, **13x too aggressive**. The decision quantizes
  in the encoded domain instead, per bundle and driven by a uniform. The entry stays **live** until
  that plan lands, and its re-test trigger is that plan's Phase 2.
- **HALF DISCHARGED 2026-08-17 — the fix shipped, the verdict has not.** Plan 0108 Phase 1 landed
  (`b02cd45`): the feedback field quantizes in the sRGB-encoded domain at 255 steps, on by default for
  any `[milk]` bundle and an exact identity otherwise, reaching **both** warp epilogues (an MD1-era
  bundle has no custom warp shader and washed out through the built-in one). The field now settles to
  exact zero and stays black at a hundred times the brightness that shows the unquantized control
  still positive, so the *mechanism* this entry names is closed. **The entry's own reason for existing
  is not:** whether the HDR pipeline now reads *better* rather than *merely different* is Plan 0108
  Phase 2, a `human` look-gate that has not run. It stays **live** until it does, and its answer can
  route back into the code — ADR-0118's Alternative D is the recorded fallback if the banding reads
  worse than the wash did.

**Raised by:** `architect`, from Plan 0100 Phase 7's side-by-side judgment (2026-08-16).
**Owner if taken:** `architect` (the emulation is a design call) then `dev`. **The dominant
fidelity defect of the MilkDrop import, and the one gating the plan's central claim** — the HDR
"better or merely different" verdict came back *merely different* and cannot be fairly re-judged
until this is fixed.

- **Verified 2026-08-16** — the warp pass scales the previous frame by a decay term and writes the
  product back with no low-end floor: `present: decay in: core/src/render/scenes/warp_mesh/mod.rs`
- `unprobeable:` the defect itself is a rendered divergence against an external reference
  (`foo_vis_milk2` 0.2.0.0, DX11) and lives in no greppable line; the evidence is the seven
  side-by-side pairs recorded in Plan 0100's Phase 7 section.

### The finding

MilkDrop's feedback target is 8-bit: `decay` times a dim pixel **truncates to zero**, and that
quantization is what keeps a classic preset's background black and its trails finite. This engine's
field is `Rgba16Float` — nothing truncates, every dim residual survives and accumulates. Judged
over seven presets side by side (Plan 0100 Phase 7, 2026-08-16), one mechanism with four
presentations: pastel wash (*Songflower*, *Cosmic Dust 2*), white-hot glow (*Contortion*), runaway
to the clamp with per-channel fringing (*chasers 19 Portal*), and full tonal **inversion** — *Fog
Tunnel*, a dark preset rendered on a white plateau. The control ran the other way: *Blur Mix 3*,
whose blur chain actively darkens, kept its blacks and looked genuinely good — which is what
scopes the defect to the feedback path rather than the shader translation.

### What a fix would be

An opt-in (per-`[milk]`-bundle, not engine-wide) floor in the warp epilogue emulating the
reference's quantization — Butterchurn and projectM both carry an equivalent, so the shape is
known. The design call is what "8-bit truncation" means in linear light: the reference truncates
in its gamma-encoded target, so a literal `1/255` floor in linear is wrong at both ends. Measure
against the same seven pairs.

### Priority

**High within the import lane.** Plan 0100's Phase 7 verdict is provisionally negative on the
plan's own motivating claim; this entry is the re-test trigger.

---

## 0107 — the MilkDrop draw layer misplaces figures, and two warp-path defects mirror or unfold the frame

> **CLOSED 2026-08-17** — [Plan 0108](plans/done/0108-the-milkdrop-import-gets-its-tone-back.md)
> Phases 3-5. Two symptoms fixed: the `wave_usedots` beads (a mark carried no caps, so it was a
> sub-pixel dash) and `wave_mode 5` emitting a continuous stroke where the preset asked for dots.
> **Two were re-attributed by the look gate, and both of this entry's attributions were wrong.**
> Item 3's inert mirror is `per_pixel_3 = sx = -zm` clamped away by the rate conversion's
> `pow(max(v, 1e-4), dt)`, **not** the per-point carry Phase 5 fixed — that fix is real and reaches
> 3 368 corpus files, it is simply a different defect. Item 1's diagonal is `time * 0.05` in the
> mode 6/7 angle, suspected by Phase 4 and convicted by the gate. Item 2's seam is **still open**,
> with its reproduction fixture committed. Item 1's absent spiro was never reached. Successors:
> 0114, 0115, 0116.

> **THIS ENTRY'S OWN LEADING SUSPECT IS FALSIFIED — 2026-08-17. Do not start where item 2 says to.**
> It says to *"check `s_fw`'s address mode against the reference's toroidal wrap first"*.
> `core/src/render/scenes/warp_mesh/shader.rs:463` already builds `s_fw` with
> `wgpu::AddressMode::Repeat`, and Repeat produces a **shifted** copy — not the **reflected** one the
> entry's own fingerprint paragraph describes. The address mode cannot be the cause of a reflection.
>
> The replacement hypothesis, with arithmetic, is
> [Plan 0108](plans/done/0108-the-milkdrop-import-gets-its-tone-back.md) Phase 3: the polar pair is built
> with a y-negation (`emit.rs:1418`, `vec2<f32>(2.0, -2.0)`), so a preset reconstructing `uv` from
> `ang` recovers `uv_orig.y` reflected about 0.5 — a mirror about the horizontal midline with its
> seam on the fixed line. The negation is provably correct for the EEL per-vertex program; whether
> it is correct in the **pixel** shader is the open question, and it is answerable in one render.

- **PROMOTED 2026-08-17 → [Plan 0108](plans/done/0108-the-milkdrop-import-gets-its-tone-back.md)
  Phases 3-5, no ADR** — three unknown-cause hunts, one phase each, **each carrying a stop
  condition** (a phase that cannot reproduce its defect commits the fixture, records what it ruled
  out, and the plan continues). That shape was the user's call over committing to fix all three,
  which this entry's falsified suspect shows would have been dishonest.
- **PART DISCHARGED 2026-08-17 — two of the six symptoms fixed, three reproduced-but-open, one not
  reached.** Plan 0108 Phases 3-5 landed (`60674da`, `6e92eb3`, `a07b0c6`), and the entry stays
  **live** on what is left. Item by item:
  - **Item 1, `wave_usedots` beads — FIXED.** A mark carried no cap flags, so it was a hard-edged
    `DOT_LENGTH x 2*width` rectangle, 3.3x wider than it was long, with its long axis under a pixel:
    **2 of 512 marks lit at 320x180**. Both ends now flag joined. A second defect fell out of the
    same measurement — `wave_mode 5` emitted a polyline unconditionally in the first of its two
    passes, so a preset asking for dots got a stroke above `wave_y` and beads below it.
  - **Item 1, *Blur Mix 3*'s diagonal — OPEN, suspect named.** The mode 6/7 arm computes its angle as
    `mystery * PI + time * 0.05`, a full turn every ~126 s, so a trace authored horizontal is
    horizontal only at instants. It is the only use of `time` in the file and contradicts the sentence
    above it. **Not removed**, because doing so moves every mode-6 and mode-7 preset and whether the
    reference's line drifts is a question about the reference.
  - **Item 1, *Cauldron painterly 5*'s absent spiro — NOT REACHED.** Modes 2 and 3 are drawn here as
    a horizontal and a vertical scope line; if the reference's mode table calls either a spiro, this
    is a mapping question rather than a rendering one, and it needs the reference's own roster.
  - **Item 2, the reflection seam — REPRODUCED, DELIBERATELY NOT FIXED.** `emit.rs` builds the polar
    pair as `(uv_orig - 0.5) * (2, -2) * aspect.zw`, so a preset rebuilding `uv` from `ang` gets a
    mirror about the horizontal midline — the reported fingerprint, on eleven lines of committed HLSL
    (`core/tests/fixtures/scratch-0108/ang-roundtrip.milk`), 19x separation from its control. The fix
    is withheld on purpose: that pair works out to exactly what `run_vertex` computes, and in the
    reference `rad`/`ang` reach a pixel shader as the per-vertex program's own numbers, so flipping
    the fragment's sign alone would put the engine in a state the reference cannot be in. **The
    architect review endorsed withholding it.** Settling it needs `foo_vis_milk2` on screen.
  - **Item 3, *chasers 19 Portal*'s inert mirror — FIXED, and nowhere near where this entry pointed.**
    That preset is MD1 with no shader blocks and no per-vertex program; its fold is a `flip` counter
    in three custom waves' per-point code, and `run_point` restored a register snapshot before every
    point, so the counter computed a constant. **3 368 of the corpus's 6 347 custom-wave presets** read
    a per-point variable only carry-over can supply, so this reached far past one preset. One open
    half, from the review: the carry also crosses the **frame** boundary, which makes an odd-length
    trace invert every frame — believed faithful, pinned by test, unverified against the reference.

**Raised by:** `architect`, from Plan 0100 Phase 7's side-by-side judgment (2026-08-16).
**Owner if taken:** `dev` (mechanism hunts), `architect` if a fix needs a design call.

- **Verified 2026-08-16** — the wave placement code under suspicion is the warp-mesh draw layer:
  `present: wave_mystery in: core/src/render/scenes/warp_mesh/draw.rs`
- `unprobeable:` the three defects are rendered divergences against an external reference; the
  evidence is the pair record in Plan 0100's Phase 7 section.

### The finding

Three distinct defects, each seen in at least one pair against `foo_vis_milk2`:

1. **Waveform placement/orientation.** *Blur Mix 3* draws one steep diagonal stroke where the
   reference draws horizontal full-width traces — suspect the `wave_mystery` angle mapping or the
   mode geometry. *Cauldron painterly 5*'s centrepiece spiro scribble is entirely absent.
   *Cosmic Dust 2*'s beaded `wave_usedots` trails never appear. (The mono engine drawing one trace
   where stereo draws two is known and accepted — Plan 0100 Phase 4; these are beyond it.)
2. **A horizontal reflection seam in warp sampling.** Content mirrors across a horizontal line
   with a bright ragged boundary: *Contortion*'s split sphere, *Cauldron*'s flipped top band,
   *Cosmic Dust 2*'s full-width false horizon. The fingerprint (reflected copy, not shifted)
   points at the wrap path's sampler address mode or a v-axis flip — check `s_fw`'s address mode
   against the reference's toroidal wrap first, not the `ang` discontinuity first suspected.
3. **`chasers 19 Portal`'s mirror symmetry never takes effect** despite a clean conversion — the
   uv-fold that makes the portal is inert. One targeted reproduction wanted.

### Priority

**Medium-high within the import lane.** ~~Item 2 is likely one sampler descriptor~~ — **corrected
2026-08-17: it is not a descriptor at all.** It is a sign in the emitted warp epilogue's polar pair,
reproduced on a committed fixture, and fixing it is a *larger* call than this entry assumed because
the same convention is carried in three places. Item 1 decides whether waveform-led presets (the
whole *Blur Mix* / *Fog Tunnel* family) read as authored, and half of it is now fixed. **What is left
of this entry is mostly answerable in one sitting with `foo_vis_milk2` on screen** — Plan 0108's
Phase 6 — rather than by more code reading.

---

## 0114 — a negative scale is clamped away, so MilkDrop's standard mirror idiom collapses instead of mirroring

**Raised by:** `architect`, from [Plan 0108](plans/done/0108-the-milkdrop-import-gets-its-tone-back.md)'s
look gate (2026-08-17). **Owner if taken:** `dev`.

- **Verified 2026-08-17** — the rate conversion clamps the sign off before the power:
  `present: max\(in\.t1\.z, 1e-4\) in: core/src/render/scenes/warp_mesh/mod.rs`

### The finding

The mesh vertex stage converts MilkDrop's per-frame rates to per-second with
`pow(max(v, 1e-4), dt)` for `zoom`, `sx` and `sy`. The `max` is there because `pow()` of a negative
base is undefined — but **a negative scale is MilkDrop's standard mirror idiom**, so the guard does
not protect the mirror, it deletes it and substitutes a near-zero positive scale. The fold is not
*inert*; it is replaced by a collapse.

*chasers 19 Portal* is exactly this: `per_pixel_3 = sx = -zm`, and the reference renders a perfectly
bilaterally symmetric portal where this engine renders an asymmetric smear. **Archived 0107 item 3
attributed that preset's missing mirror to per-point code, and Plan 0108 Phase 5 fixed a real defect
there — the per-point carry, reaching 3 368 corpus files — which simply is not this preset's fold.**

**Corpus reach, 2026-08-17:** 363 of 10 347 files (3.5 %) assign a negative to `sx`, `sy` or `zoom`
in per-pixel/per-vertex code — 229, 172 and 155 respectively.

### What a fix would be

`sign(s) * pow(abs(s), dt)`. It is safe under rate conversion: a mirror composed with itself is the
identity, so the symmetric fixed point converges the same at 30 Hz and at 144 Hz even though the
intermediate frames differ. Needs a golden re-bless and a fixture pinning bilateral symmetry.

### Priority

**High, and cheap.** Roughly four lines with a test.
[Plan 0109](plans/done/0109-the-milkdrop-import-gets-its-geometry-back.md) Phase 1.

---

## 0115 — there is no video-echo stage, and one preset in seven is unrecognisable without it

**Raised by:** `architect`, from [Plan 0108](plans/done/0108-the-milkdrop-import-gets-its-tone-back.md)'s
look gate (2026-08-17). **Owner if taken:** `dev`.

- **Verified 2026-08-17** — the converter names the feature as unconsumed rather than dropping it
  silently, which is where the gap is recorded today:
  `present: echo_zoom in: milkconv/src`

### The finding

MilkDrop composites a **second sampled copy of the previous frame** — zoomed by `fVideoEchoZoom`,
flipped per `nVideoEchoOrientation` (0 none, 1 flip x, 2 flip y, 3 both), blended at
`fVideoEchoAlpha`. This engine has no such stage. All three values are already per-frame outputs the
runtime computes and then drops on the floor, and `milkconv` warns about them at conversion time.

*Songflower (Moss Posy)* sets `fVideoEchoAlpha = 1.000` with `nVideoEchoOrientation = 1` and animates
both per frame. **Its woven lattice IS the echo**: the reference draws a bold two-axis basket weave,
and without the echo only one family of bars survives — this engine renders pale horizontal ripples
that read as a different preset entirely. *Fog Tunnel* carries echo at `0.400`.

The converter's own census puts non-zero echo alpha at **2.4 %** of the corpus. Narrow, and
load-bearing where it appears.

### What a fix would be

A composite-stage pass sampling the previous frame with a flip and a zoom, blended at alpha. The one
open design question is whether the echo samples before or after the warp — MilkDrop composites from
the previous frame's final target, which decides whether the echo accumulates; confirm against the
reference rather than assume.

### Priority

**Medium-high.** Real new work, but bounded, and it is the difference between one of the seven
judged pairs being recognisable or not.
[Plan 0109](plans/done/0109-the-milkdrop-import-gets-its-geometry-back.md) Phase 3.

---

## 0116 — the mode 6/7 waveform rotates a full turn every two minutes, and the reference's does not

**Raised by:** `architect`, from [Plan 0108](plans/done/0108-the-milkdrop-import-gets-its-tone-back.md)'s
look gate (2026-08-17). **Owner if taken:** `dev`.

- **Verified 2026-08-17** — the term is still in the mode 6/7 arm, with Plan 0108 Phase 4's
  arithmetic recorded beside it:
  `present: time \* 0\.05 in: core/src/render/scenes/warp_mesh/draw.rs`

### The finding

`draw.rs`'s mode 6/7 arm computes its angle as `mystery * PI + time * 0.05` — a full turn every
~126 s, so a trace authored horizontal is horizontal only at instants. Three things made it a
suspect when Plan 0108 Phase 4 found it: it is the **only** use of `time` in the whole file, it
contradicts the sentence directly above it (which says the angle is set by `wave_mystery`), and it is
absent from the module header's list of stated approximations.

Phase 4 deliberately left it in, because removing it changes every mode-6 and mode-7 preset and
whether the reference's line drifts is a question about the reference. **The look gate answered it:
the reference's *Blur Mix 3* draws horizontal full-width traces and does not drift; this engine draws
one steep diagonal.** The term is convicted.

### What a fix would be

Remove the term; add its removal to the module header's stated-approximations list; pin that a
mode-6 figure's orientation is a pure function of `wave_mystery` by rendering the same geometry at
two well-separated `time` values. Needs a golden re-bless — it is *intended* to change what ships.

### Priority

**High, and cheap.** [Plan 0109](plans/done/0109-the-milkdrop-import-gets-its-geometry-back.md) Phase 2.

---

## 0121 — a bundle that never names `decay` reads MilkDrop's per-frame default as a per-second one

**Raised by:** `architect`, from [Plan 0109](plans/done/0109-the-milkdrop-import-gets-its-geometry-back.md)'s
close review (2026-08-19). **Owner if taken:** `dev`.

- **Verified 2026-08-19** — the unresolved-slot arm returns the seeded default without converting it:
  `present: None => d\.\$field in: core/src/milk/outputs.rs`

### The finding

`FrameOutputs`' table declares `decay: "decay" = 0.98, Factor`, and that one constant is used for two
different things. `seed` writes it into the register **before** the program runs, where `0.98` is
correctly MilkDrop's *per-frame* value. `read` returns it unchanged when the program never mentions
`decay` — but that arm feeds a field whose own doc says "**per second** here", and the converted
value is `per_second_factor(0.98) = 0.5455`. A bundle that never names `decay` therefore fades at
`0.98`/s (about `0.9997` per frame) instead of `0.5455`/s: effectively not at all.

`decay` is the only `Factor`-rate output, so nothing else is affected.

**Not currently reachable from a converted preset**, which is why this is an entry rather than a bug
report. `milkconv`'s prologue emits an assignment for **every** roster output carrying an
initial-condition key, unconditionally, at the top of every frame — so the slot always resolves and
the conversion always runs. The one shipped hand-written bundle
(`core/tests/fixtures/warp_mesh_milk.toml`) names `decay` too.

**It has already cost real time once**, which is the argument for fixing it rather than documenting
it: Plan 0109 Phase 4's field probe drives an empty bundle, silently ran its whole experiment at
`0.98`/s, and the phase's reported fade numbers could not be reproduced from the committed code until
the close review found this. A latent unit error that only bites instruments is exactly the kind that
bites the next instrument too.

### What a fix would be

Convert on the `None` arm as well, so the fallback is in the same vocabulary as the resolved value —
roughly `convert(d.$field, Rate::$rate, d.$field)`. **It moves goldens** for any fixture whose bundle
omits `decay`, so it wants a bless and a check of which fixtures those are, and it should not be
slipped into an unrelated plan.

### Priority

**Low-medium.** Latent for shipped content, but it is a units error sitting in the one table that
exists to stop units errors, and its own module header says four parallel lists were replaced by that
table precisely so a value could not silently read its neighbour.

### Closed 2026-08-19 by [Plan 0111](plans/done/0111-the-milkdrop-import-stops-washing-out.md) Phase 1 — the fix is as specified, and **one prediction above was wrong**

The `None` arm now calls the same `convert` the `Some` arm does, verified both ways: with the fix an
unnamed `decay` reads `0.54548466`, with the arm reverted it reads `0.98`.

**"It moves goldens" did not happen, and the reason is worth keeping.** The golden set carries exactly
two `[milk]` fixtures — `warp_mesh_milk` and `warp_mesh_shader` — and **both declare `decay` in their
`.regs` roster**, so both slots resolved to `Some` and both already took the converting arm. The only
fixture whose bundle omits `decay` is `warp_mesh_lit_backdrop.toml`, whose `[milk]` table is empty by
design, and it has no golden. No preset in `presets/` carries a `[milk]` block at all. So the entry's
own escalation — that this wants a bless and should not be slipped into an unrelated plan — was
sound reasoning from a premise that the fixture set did not support.

**What it did move was two tests, and not as collateral.** Plan 0109 Phase 4's two quantizer probes
passed `None` to `field_trace` and relied on the fallback being near-unity — which is what neutralizes
`decay` so the quantizer is the only bound, and is documented in the comment above their own call
site. The fix changed the meaning of a value they took for free, not their calibration. They now pass
an explicit `NEUTRALIZED_DECAY = Some(0.98)`, which reproduces every committed digit of both
doc-comment tables; names, claims, thresholds and tables are unchanged. The measurement that came out
of it — the same field under a realistic converted `decay` — is
[ADR-0118](adrs/0118-the-milkdrop-feedback-field-quantizes-in-the-encoded-domain.md)'s third
`Outcome`.

---
