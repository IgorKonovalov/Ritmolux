# 0046 — Transformed feedback: the past learns to move

> **Status:** approved 2026-07-30 — ready for `dev` (runs after Plan 0045; same render files)
> **Created:** 2026-07-30
> **Owner skill(s):** dev, human
> **Related ADRs:** [0048](../adrs/0048-transformed-feedback.md).
> [docs/roadmap-visual-richness.md](../roadmap-visual-richness.md) R2.

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
  attractor's existing form). Pick and implement the off-frame edge policy (ADR-0048 names
  transparent-border vs clamp; evaluate at a portrait aspect per ADR-0047's lesson). The
  transform is aspect-corrected — a rotation must not shear on a non-square target
  (ADR-0037: aspect from the render target).
- **Files touched:** `core/src/render/trails.rs`, `core/src/render/scenes/mod.rs` (param
  routing), `core/tests/composite.rs`.
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
- **Files touched:** `core/src/render/scenes/particles/mod.rs`,
  `core/src/render/scenes/mod.rs`, `core/tests/golden.rs` fixtures.
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
