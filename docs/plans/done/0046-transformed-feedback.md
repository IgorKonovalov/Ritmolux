# 0046 — Transformed feedback: the past learns to move

> **Status:** **done 2026-08-09** — all five phases landed as `f2e6ed6` (the affine + the `dt` fix) /
> `24f4bfc` (the `[feedback]` table) / `0816516` (the attractor joins) / `429396d` (the docs) /
> `16802ae` (the Phase 5 verdict, **run by the user on 2026-08-09**). Mode 4 review at the close:
> **no blockers, no majors, four minors, two nits.** Verified rather than taken on trust: the
> transform's aspect comes from the **render target** on both sinks (`trails.rs`'s `resolve` now
> takes `surface`, which it had been ignoring; the attractor's is `Scene::render`'s `aspect`
> parameter) — this is ADR-0037's third repeat, and the fix carries a negative control, 45x46
> shipped against 44x71 with the aspect forced to `1.0`; the identity is a CPU-computed `select` on
> the literal `in.uv` rather than an arithmetic round-trip; the `[feedback]` rosters are closed and
> reject at load; and **all 20 pre-existing golden baselines are hash-identical to a clean-`main`
> bless, with exactly three files added** (`composite_warp_swirl/_ripple/_fisheye.png`) —
> re-measured at this close, after the merge that first put this lane's code beside Plan 0068's.
> **Seven deviations, all self-reported in the phase commits; the two that changed a contract are
> corrected in the phase text below.**
> **Created:** 2026-07-30
> **Owner skill(s):** dev, human
> **Related ADRs:** [0048](../../adrs/0048-transformed-feedback.md) (**accepted** at this close,
> carrying a dated `Outcome` for the `blend` narrowing).
> [docs/roadmap-visual-richness.md](../../roadmap-visual-richness.md) R2.

## TL;DR

Both accumulation buffers — the engine trails stage and the attractor's internal trail —
resample the previous frame through a bindable transform: affine `fb_zoom`/`fb_rotate`/
`fb_dx`/`fb_dy` about `fb_center_x/y`, plus a curated `[feedback] warp` family
(`swirl`/`ripple`/`fisheye`) with bindable `fb_warp` strength, and a selectable deposit
blend (`max` default, `add` for summing echoes under the HDR headroom). All rates are
per-second on injected `dt`, and the trails `fade` frame-rate defect is fixed en route.
First user-visible behavior: a preset with `fb_zoom` bound to the beat renders a pulsing
light tunnel. Every `fb_*` default is identity, so all existing content and goldens are
untouched until a preset opts in.

## Context & problem

ADR-0048's Context carries the full case: decay-only feedback is the widest gap between the
engine and the reference imagery, the seam was built for this, and the HDR pipeline (Plan
0045) supplies the headroom the additive mode needs. **This plan runs strictly after Plan
0045** — it edits the same stage files and renders into the linear-light chain.

## Decision

Per ADR-0048. Rejected alternatives (author per-pixel warp, full blend family, a sibling
stage, engine-only scope) are recorded there; the attractor's inclusion is the user's call
over the smaller recommendation, so the attractor phase is deliberately last.

## Architecture diagram

```mermaid
flowchart LR
    subgraph "trails stage (Rgba16Float)"
        CUR[current frame] --> DEP{deposit blend<br/>max | add}
        PREV[previous accum] --> XF["inverse transform<br/>affine(fb_zoom, fb_rotate, fb_dx/dy @ fb_center)<br/>+ warp(kind, fb_warp)"] --> DEC[decay fade^(dt*60)] --> DEP
        DEP --> ACC[next accum]
    end
    subgraph "attractor scene"
        AXF["same vocabulary on its<br/>internal trail blit"]
    end
    PARAMS[fb_* bindables + feedback table] --> XF
    PARAMS --> AXF
```

## Implementation phases

### Phase 1 — Walking skeleton: the affine transform on trails, and the dt fix
- **Owner skill:** dev
- **What:** the trails read samples `prev` through the inverse affine (`fb_zoom`,
  `fb_rotate`, `fb_dx`, `fb_dy`, `fb_center_x`, `fb_center_y`), all rates per-second scaled
  by `dt`; `fade` normalized to `fade^(dt*60)` (identity at the capture `dt`, matching the
  attractor's existing form).
  **Shipped stricter than that, and the difference is worth knowing.** The trails exponent is
  written `dt / FALLBACK_DT` rather than `dt * 60`, so at the capture step it is `x / x` — exactly
  `1.0` in IEEE with no reliance on rounding — and the `exponent == 1.0` arm short-circuits `powf`,
  which is not required to return `x` for an exponent of one. **The attractor's decay keeps the
  older `powf((dt * 60.0).max(0.0))` with neither guard** (`particles/encode.rs`), so "matching the
  attractor's existing form" reads backwards: the new sink is the safer one. Nothing is wrong today
  — `(1/60f32) * 60.0` does round to exactly `1.0`, and every attractor baseline is hash-identical
  — but the guard the trails comment argues for is absent on the sink that shares the vocabulary,
  and a future change to `FALLBACK_DT` or to the capture step would break the two apart silently.
  Recorded, not repaired: it is a pre-existing line this plan did not introduce.
  Pick and implement the off-frame edge policy (ADR-0048 names
  transparent-border vs clamp; evaluate at a portrait aspect per ADR-0047's lesson). The
  transform is aspect-corrected — a rotation must not shear on a non-square target
  (ADR-0037: aspect from the render target).
- **Files touched:** `core/src/render/trails.rs`, ~~`core/src/render/scenes/mod.rs` (param
  routing)~~ — **corrected at the close: param routing lives in `core/src/render/mod.rs`**
  (`ParamRoute` / `resolve_route`) **and `core/src/render/post.rs`** (the `set_dt` / `set_feedback`
  one-way pushes). `scenes/mod.rs` was edited, but in Phase 3 and for the `Scene::set_feedback`
  trait method, not for routing. ~~`core/tests/composite.rs`~~ — **the motion guards landed in a new
  `core/tests/feedback.rs` instead**, and the reason is load-bearing rather than cosmetic: they need
  a portrait target and several consecutive multi-frame runs, and `composite.rs` is a separate test
  binary precisely because building GPU resources mid-run perturbs what the trails stage resolves to
  on WARP — the exact perturbation its own module docs warn about. The new file pins no baseline;
  every assertion in it is a ratio of one run against itself.
- **Done when:** all `fb_*` at defaults ⇒ every golden **byte-identical** (the identity
  claim, proven the Plan 0038 way); a fixture binding `fb_zoom` shows the accumulated
  streak displaced radially between two captured frames and a `fb_rotate` fixture shows it
  displaced tangentially (relative assertions between frames of one run, no magic
  thresholds); a rotation capture at a non-square size shows no shear (circle stays a
  circle, asserted on the accumulated ring's bounding box).

### Phase 2 — The `[feedback]` table: warp family and deposit blend
- **Owner skill:** dev
- **What:** the structural table `[feedback] warp = "none|swirl|ripple|fisheye"`,
  `blend = "max|add"` (load-time, `[curve] family` pattern, unknown values reject the
  preset at load like every other structural key); `fb_warp` strength bindable; the `add`
  deposit path. Each warp kind gets its own capture fixture; the `add` mode's test asserts
  the bounded-accumulation property (a static bright source under `add` at
  `fade = MAX_FADE` converges rather than growing without bound, read back over a long
  run).
- **Files touched:** `core/src/preset/schema.rs`, `core/src/render/trails.rs`,
  `core/tests/composite.rs`, new fixtures + baselines.
- **Done when:** `warp = "none"`, `blend = "max"` (the defaults) ⇒ goldens byte-identical;
  each named warp produces a distinct pinned baseline; the convergence property holds; a
  preset naming an unknown warp is rejected at load with a surfaced message.

### Phase 3 — The attractor's internal trail joins the vocabulary
- **Owner skill:** dev
- **What:** the attractor's accumulation blit consumes the same `fb_*` params and
  `[feedback]` table through `Scene::set_param`/`configure`, transforming its own field.
  The routing contract is stated in code and docs: one vocabulary, two buffers, both may be
  active, each transforms only its own accumulation.
  **Narrowed at the close, and the code is what stands, not this sentence.** The seven `fb_*` params
  and `[feedback] warp` reach both sinks; **`[feedback] blend` reaches the trails stage only.** The
  attractor's deposit has been additive since the scene was written — its points draw through an
  additive pipeline over the decayed bed, in one pass — so there is no `max` to select there without
  a **second draw pipeline**, which is exactly the coexisting-matching-bind-layout shape
  ([ADR-0058](../../adrs/0058-bind-group-layout-collisions-carry-evidence.md)) the warp family was
  kept to one shader to avoid. Paying a known WARP hazard to make one sentence literally true is the
  wrong trade; the asymmetry is instead **documented where an author meets it**
  (`presets/README.md`, "One vocabulary, two buffers"), and recorded in
  [ADR-0048](../../adrs/0048-transformed-feedback.md)'s `Outcome`.
- **Files touched:** `core/src/render/scenes/particles/mod.rs`,
  `core/src/render/scenes/mod.rs`, `core/tests/golden.rs` fixtures — **plus
  `core/src/render/feedback.rs`**, which is where the shared `Transform`, the `[feedback]` types and
  the one WGSL snippet both sinks compile actually live. **Not a new file:** it already existed as
  the `PingPongField` home — the seam [ADR-0012](../../adrs/0012-stateful-feedback-render-system.md) built
  and ADR-0048 says this was always for. That is a better home than either sink, and it is what
  makes "one vocabulary" structural rather than a convention two `set_param` matches have to keep
  agreeing on.
- **Done when:** attractor defaults ⇒ its goldens byte-identical; an attractor fixture with
  `fb_rotate` shows the trail field rotating while the freshly-deposited points do not lag
  (the transform applies to the past, not the present deposit); the shared-vocabulary
  contract has a test driving both sinks in one preset.

### Phase 4 — Docs sweep
- **Owner skill:** dev
- **What:** `presets/README.md` — the `fb_*` params, the `[feedback]` table, the
  two-sinks routing note, and per-second rate semantics; `docs/presets.md` — the new
  structural table in the schema section; `docs/capturing.md` — how the motion fixtures
  assert displacement (for the next author of a feedback test).
- **Files touched:** the three docs.
- **Done when:** all three describe the shipped surface; the trails `fade` per-second
  change is documented where `trails` is documented.

### Phase 5 — The look, on the wall
- **Owner skill:** human
- **What:** run the standalone (Rich tier) with two scratch presets `dev` leaves in a
  fixture directory — a beat-driven `fb_zoom` tunnel over a line scene, and a
  `swirl` + `add` echo over the fragment field — fullscreen on the target display. Judge:
  does the motion read as depth and flow (the reference looks), and does frame time hold?
- **Done when:** verdict recorded in this plan (impressions + frame time). No stopping
  condition — `fb_*` are continuous and content-tunable; a disappointing look routes to
  the content lane, not back to the ADR, unless a warp kind is fundamentally wrong.
- **VERDICT — run 2026-08-09**, `v0.48.0` lane build, Rich tier pinned, loopback audio,
  both scratch presets via `LMV_PRESET_DIR`. **Passed. No warp kind is fundamentally
  wrong and nothing routes back to [ADR-0048](../../adrs/0048-transformed-feedback.md).**

  **The look.** `swirl_add_echo` — *"very good."* The vortex reads as depth and flow, which
  is the reference look this plan exists to reach, and the `add` deposit did not wash out
  over live music. `tunnel_beat_zoom` — the rosette core *"looks more like a lit rosette"*
  in motion; **the black centre visible in a still is an artifact of capturing with `beat`
  pinned at 1**, not a defect, and a future reader comparing a headless still against this
  verdict should know that. Its one criticism is colour: *"maybe needs more colours and
  saturation."*

  **That criticism is content, and it routes to the content lane as this phase specifies.**
  These two presets are fixtures, not library content — Plan 0046's "What this plan does
  NOT do" reserves adoption for R6, and [Plan 0075](../0075-the-content-renaissance.md)'s
  cohorts are where a feedback preset gets a real palette. Nothing here asks for an engine
  change: `[palette]`, `saturation` and `palette_mix` already reach this surface
  ([ADR-0086](../../adrs/0086-the-backdrop-colours-through-the-preset-palette.md)), and the
  scratch preset's narrow band was chosen to keep `add` from flattening, which trades
  colour away deliberately. **The finding for R6 is that `blend = "add"` and a rich palette
  pull against each other, and no shipped preset has yet had to resolve that.**

  **Frame time — measured, not impressionistic.** 158 audible 1 Hz rows, ~3 minutes,
  1080p windowed and fullscreen, both presets, with preset switching:

  | | value |
  |---|---|
  | fps median / mean | **165.0** / 162.3 |
  | fps minimum | **114.3** |
  | rows below the NFR §1 60 fps floor | **0 of 158 (0.00 %)** |
  | `frame_ms_avg` median / max | 6.061 / 8.749 ms |
  | `frame_ms_p99` median / max | 6.866 / **25.037** ms |
  | frames dropped | **0 of 28 698 (0.000 %)** |

  So the §1 floor holds with roughly 2.7x headroom on this box, and the `add` +
  transform + tonemap path the Risks section called "new territory" costs nothing
  measurable. **Two observations that are not failures but should not be lost:**
  - **`frame_ms_p99` spikes to 25.0 ms** (p95 of the p99 column is 18.1 ms), above the
    16.67 ms budget, while `frame_ms_avg` never exceeds 8.7 ms and no frame is dropped.
    The spikes coincide with preset switches and the fullscreen toggle — GPU resource
    rebuilds, not steady-state cost. Worth a glance from whoever next touches the
    quality governor, since a demotion decision reading p99 would see these.
  - **RSS grew 385 MB → 663 MB over the three minutes** (max 663 MB), against the
    ~327 MB driver-dominated floor [ADR-0010](../../adrs/0010-accept-gpu-driver-memory-floor.md) established.
    Three minutes with repeated preset switching is too short and too atypical to call
    this a leak, and this plan adds two accumulation buffers so *some* growth is
    expected — but it is unmeasured against a no-feedback control and should be, before
    R6 ships feedback presets that run for hours. Not a blocker for this plan.

## Data shapes

No C-ABI or `Scene`-trait change expected. New named params: `fb_zoom`, `fb_rotate`,
`fb_dx`, `fb_dy`, `fb_center_x`, `fb_center_y`, `fb_warp`. New structural table:

```toml
# illustrative
[feedback]
warp  = "swirl"   # none | swirl | ripple | fisheye — load-time
blend = "add"     # max (default) | add            — load-time
```

## Risks & open questions

- **The edge policy is a visible design surface** (a zoom-out reveals what lies beyond the
  frame). Phase 1 must evaluate both candidates at portrait aspect before pinning fixtures;
  if neither reads acceptably, that is an ADR-0048 Outcome note, not a silent tweak.
- **`add` + transform + tonemap interaction is new territory** — the convergence test is
  the guard, but the *look* of near-`MAX_FADE` additive echoes is unproven until Phase 5.
- **Two sinks, one vocabulary** can surprise an author running chain-trails over the
  attractor; the routing note in Phase 4 is load-bearing, not optional.
- **WARP pipeline sensitivity**: warp kinds are shader variants of one stage — prefer one
  shader with a kind uniform over per-kind pipelines, and say so in the phase commit if the
  tradeoff bites.
- The trails `fade` dt-normalization changes live-app decay feel at non-60 Hz displays
  (correctly — that is the fix); captures are unaffected by construction.

## What this plan does NOT do

- No author-defined per-pixel warp (ADR-0048 Alternative A; ADR-0002's escape-hatch
  territory).
- No layer composition (R3) and no new scenes (R4).
- No shipped-preset adoption — the scratch presets are fixtures, not library content; the
  content pass is R6's.
- No change to the reaction-diffusion field (its feedback is chemistry, not echo — ADR-0012
  stands).

## Followups (after this lands)

- R3's interview (layer composition) — transformed feedback per layer is where the collage
  reference fully arrives.
- Content-lane adoption presets; retire the scratch fixtures if superseded.
- If the curated warp family proves too small, the per-pixel-warp ADR (Alternative A) gets
  its real hearing.
