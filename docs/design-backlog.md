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
[Plan 0037](plans/0037-verifying-easing-transient-probe-and-dynamic-signal.md)'s doc phase alongside
0014. No code change, no ADR.

*(Second entry in this batch whose diagnosis inverted under verification — see 0010. Both were filed
in good faith from real symptoms; both attributed the symptom to the wrong mechanism. The lane's
symptom reports are reliable; its causal claims want checking against code before they become work.)*

---

## 0013 — no synthetic signal has transients, so a `[smoothing]` change cannot be verified at all

- **Raised:** 2026-07-26, from `preset-author`, after adopting `{ attack, release }` on 20 presets.
- **PROMOTED 2026-07-26 → [ADR-0039](adrs/0039-verify-easing-with-a-transient-probe-not-a-committed-clip.md) +
  [Plan 0037](plans/0037-verifying-easing-transient-probe-and-dynamic-signal.md)** — a deterministic
  transient probe (the primary answer) plus one synthesized generator with musical dynamics; a
  committed reference clip was rejected, and 0008's item 3 calibration question is closed by a
  `human` phase that measures the user's own audio and records the numbers. **0012 and 0014 ride
  along as documentation** in that plan's Phase 5. Notes retained below as the origin record.
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
- **Verified by a rendered six-way sweep.** Predicted 0.06 = amber, 0.17 = gold-green, 0.62 = violet.
  Measured: **0.06 lavender, 0.17 turquoise, 0.30 cyan, 0.46 near-white/green, 0.62 gold, 0.82 rose.**
  Every prediction was wrong.

The three line scenes ignore `[palette]` entirely and colour through their own cosine ramp, so `hue`
is their *only* colour control — and its mapping is undocumented and not the hue wheel the name
implies. Picking a colour costs a render round-trip every time.

**Impact:** small, recurring, purely documentation — a swatch table in `docs/preset-palettes.md` (or
a generated strip committed as an image) closes it. Bundle with any other doc sweep.

- **PROMOTED 2026-07-26 → [Plan 0037](plans/0037-verifying-easing-transient-probe-and-dynamic-signal.md)
  Phase 5**, which carries the measured swatch points to seed the table.
