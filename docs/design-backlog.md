# Design backlog — captured feedback, not yet promoted

Short, durable notes for design gaps surfaced during work but **not yet** decided into an ADR or
plan. Chiefly the `preset-author → architect` feedback handoff (a look wanting something the
preset grammar or engine can't express), plus any other "worth remembering, not worth acting on
yet" finding.

An entry here is **not** a commitment to build — it is a captured signal so the friction isn't
lost between sessions. When one is acted on, it graduates to an ADR (if it has a real rejected
alternative) and/or a plan, and the entry is struck through with a pointer to where it went.
Verify every entry against the code before acting on it — these are dated snapshots, and the
surface moves (same rule the lanes apply to their own references).

---

## 0001 — reaction_diffusion reaches only 2 of the 5 Plan-0018 composite levers

- **Raised:** 2026-07-24, from `preset-author` (authoring the "Chthonic Coral Oracle" coral preset).
- **Verified against code:** yes — see the per-lever notes below.
- **PROMOTED 2026-07-24 → [ADR-0026](adrs/0026-full-composite-coverage-fullscreen-scenes.md) +
  [Plan 0025](plans/0025-full-composite-coverage.md)** (full-audit scope: background + view transform
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
  [Plan 0034](plans/0034-preset-reachable-spectrum.md).** **Three verifications shrank this well below
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
  [Plan 0033](plans/0033-internal-resolution-and-preset-surface.md)** (Phases 3-4 the RD side,
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
  [Plan 0033](plans/0033-internal-resolution-and-preset-surface.md) Phase 5.** Notes retained below
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
  [Plan 0033](plans/0033-internal-resolution-and-preset-surface.md) Phase 2.** Notes retained below
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

## 0007 — `star_pattern` reads as a hollow ring, and discrete `variant` cannot be blended

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
- **PROMOTED 2026-07-26 → [Plan 0033](plans/0033-internal-resolution-and-preset-surface.md) Phase 1**
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
  ([0021](#0021--an-even-fall-is-not-reachable-with-a-one-pole-in-any-ordering)), and `--report`
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

---

## Entry 0021 — from the Plan 0038 / ADR-0040 ruling

Not from the content lane. Raised by an `architect` ruling that had to falsify a claim in order to
answer a `dev` finding, and left a real want with nowhere to live.

---

## 0021 — an "even fall" is not reachable with a one-pole, in any ordering

- **Raised:** 2026-07-28, from
  [ADR-0040's Outcome](adrs/0040-spectrum-level-curve-applies-before-the-easing.md#outcome-2026-07-28-after-plan-0038-phase-3s-measurement).
  ADR-0040 chose the spectrum level curve's position in the pipeline partly to buy "a perceptually
  even fall". Plan 0038 Phase 3 measured it, and the closed form settles it: **no ordering can deliver
  that**, because every `[smoothing]` response in this engine is a one-pole exponential and a power of
  an exponential is an exponential.
- **Verified against code:** yes. `Easing::step` (`core/src/preset/schema.rs:223`) is
  `held + (1 - exp(-dt/tau)) * (raw - held)`, one constant per direction (ADR-0035), no shape.
  `Smoother::smooth` (`core/src/render/mod.rs:317`) is the same arithmetic for bindings, and the
  spectrum scene's per-element easing calls the same method.

**The measurement, for the record.** An exponential spends **30 %** of its settling time covering the
first half of its travel (`ln2 / ln10` = 0.301); a linear ramp spends **56 %**. Both curve orderings
measure 0.301 when measured to settlement. So "even" is a ~1.8x gap from what the engine can currently
produce, in either ordering, at any exponent.

**The want is legitimate and has been asked for twice.** This is the half of
[0006](#0006--smoothing-is-a-one-pole-low-pass-no-attackrelease-split-no-s-curve) that
[ADR-0035](adrs/0035-asymmetric-attack-release-easing.md) deliberately did not take — 0006's origin
ask was literally "use some qubic bezziere function or something", and the asymmetric one-pole
answered the *symmetry* half of that defect while leaving the *shape* half untouched. A meter that
falls at a constant rate is the classic look this cannot make.

**The cheap shape, if it is wanted:** a **rate-limited (slew) release** rather than a curve —
`held += clamp(raw - held, -rate * dt, +rate * dt)` — which is a third `[smoothing]` form beside
today's scalar and `{ attack, release }`, needs **no** new per-binding state (the slot exists), stays
stateless from the author's side, and is frame-rate-independent for the same reason the one-pole is
(ADR-0019's injected real `dt`). A constant-rate fall is exactly evenness 0.556. The nameable rejected
alternative is a full parametric ease curve, which needs a notion of "a transition in progress" and a
rule for a target that moves mid-ease — the same reason it lost in 0006.

**Not the thing ADR-0035 already rejected.** Its Alternative C was a `slew(x, up, down)` **function in
the grammar**, refused outright because expressions are pure and stateless by hard invariant. The
proposal here is the opposite location: a `[smoothing]`-table *form*, where the state already lives and
where the asymmetric one-pole itself landed. That distinction is the whole reason this is a fresh entry
rather than a re-litigation, and any ADR must say so explicitly or it will read as reopening 0035.

**ADR-worthy** as a short supplement to [ADR-0019](adrs/0019-eased-parameters.md) /
[ADR-0035](adrs/0035-asymmetric-attack-release-easing.md) if acted on. **Not urgent**: nothing shipped
is broken, and unlike most entries here this one is a *new capability* rather than a wall the content
lane has already hit. It wants a preset-author "I want this look and cannot get it" before it wants a
plan — the evidence so far is an architect's arithmetic, not a frustrated author.

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
