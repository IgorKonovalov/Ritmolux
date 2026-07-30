# ADR-0048 — Transformed feedback: the accumulation buffers resample the past through a bindable affine + curated-warp transform, with a selectable deposit blend

> **Status:** proposed
> **Date:** 2026-07-30
> **Related plan(s):** 0046-transformed-feedback (R2 of [docs/roadmap-visual-richness.md](../roadmap-visual-richness.md))
> **Supplements:** 0012 (the `PingPongField` seam this was always for), 0018/0031 (the trails stage), 0046 (the HDR headroom additive echoes need)

## Context

Both accumulation buffers in the engine can only let the past sit still and dim: the trails
stage is `accum = max(cur, prev * fade)` sampled at the identical uv (trails.rs:64-68), and
the attractor's trail blit is `prev * k`, same uv (particles/mod.rs:428-432). The
MilkDrop-class looks the visual-richness review targets — tunnels, spirals, radial streaks,
liquid smear — all come from one gene these buffers lack: **the previous frame resampled
through a small per-frame transform**. The seam expected this: feedback.rs:6-8 says "future
warp/feedback variants reuse it (ADR-0002 named it a deferred follow-up)", and ADR-0012
rejected warp advection only for the reaction-diffusion scene's target look, not for the
engine.

Two constraints shape the vocabulary. First, frame-rate independence is a delivered user
requirement (ADR-0019's injected real `dt`): a transform expressed per-frame would spin twice
as fast at 120 Hz. The trails stage already violates this for `fade` (applied per frame,
trails.rs:401, unlike the attractor's `fade^(dt*60)`), and this decision is where that gets
fixed rather than extended. Second, additive echo accumulation needs headroom — on the 8-bit
composite it would clip into flat white, which is why this lands after ADR-0046's linear-light
pipeline.

The 2026-07-30 interview settled scope: affine plus a curated warp family (not author-defined
per-pixel warp), max plus additive deposit (not the full blend family), a bindable centre, and
**both** buffers — the engine trails stage and the attractor's internal trail — in the same
design.

## Decision

We will make both accumulation reads sample the previous frame through an **inverse per-frame
transform** composed of: an affine part — `fb_zoom`, `fb_rotate`, `fb_dx`, `fb_dy` about a
bindable centre `fb_center_x`/`fb_center_y` (default screen centre) — and one **curated
procedural warp** — `warp = "none" | "swirl" | "ripple" | "fisheye"`, a load-time structural
key in a new `[feedback]` table (the `[curve] family` pattern), with its strength as the
bindable `fb_warp`. All rate-like quantities are **per-second** and scaled by the injected
`dt` (`zoom` as `rate^dt`, rotation as `rad/s * dt`, translation as `units/s * dt`), and the
trails `fade` is normalized to the same `fade^(dt*60)` form the attractor already uses — at
the fixed capture `dt` of 1/60 this is the identity, so goldens do not move from the
normalization. The deposit blend becomes selectable in the same `[feedback]` table:
`blend = "max"` (today's, the default) or `"add"` (`accum = cur + prev * fade`), whose
geometric series is bounded by `1/(1 - fade)` under the existing `MAX_FADE = 0.98` and rolls
off through ADR-0046's tonemap rather than clipping.

The same param names are consumed by **two** sinks: the engine trails stage (every scene
through the chain) and the attractor scene's internal trail blit (its `Scene::set_param`),
which may both be active — the contract is one vocabulary, two buffers, each transforming its
own accumulation. All `fb_*` params default to identity, so every existing preset and every
golden is byte-identical until a preset opts in.

## Consequences

### Positive
- The reference-imagery gap closes at its widest point: zoom feedback is a tunnel, rotation
  a spiral, swirl a vortex, additive echoes under HDR are neon streaks — all audio-drivable,
  since `fb_*` are ordinary bindables (a beat can kick the zoom).
- The standing frame-rate-dependence defect in trails `fade` is retired in the same change,
  with capture behavior provably unchanged.
- One vocabulary serves both buffers, so authoring knowledge transfers between the chain and
  the attractor family.

### Negative
- The trails stage stops being the cheapest pass in the chain: the read gains transform ALU
  and, in warp modes, non-uniform sampling. Still one pass; the cost is per-pixel math, not
  bandwidth.
- Off-frame reads become routine (a zoom-out pulls in from beyond the edge). The sampler's
  edge policy is now a *visible* design surface — the plan must pick and test it (transparent
  border vs clamp) at non-16:9 aspects, with ADR-0047's lesson applied.
- Two sinks for one param family is a routing subtlety the docs must state plainly, or an
  author with both active will attribute one buffer's motion to the other.
- A fourth structural table (`[feedback]`) grows the preset schema surface.

### Neutral
- `max`-blend feedback with a transform is no longer strictly non-increasing per pixel
  (moved copies land on new pixels), but remains bounded by the source maximum; only the
  `add` mode changes the energy story, and the tonemap owns that.

## Alternatives considered

### Alternative A — author-defined per-pixel warp (a UV expression per preset)
True MilkDrop warp. Rejected for now: it is the first expression evaluated on the GPU, which
means a grammar-to-WGSL translator, a per-preset pipeline compile, and a new QA surface — the
same order of leap as ADR-0002's deferred author-WGSL escape hatch, and it should be decided
as that, not smuggled in as a stage option. The curated family covers the target looks;
revisit only if the content lane exhausts it.

### Alternative B — the full deposit-blend family (screen, lerp, …)
Each blend is a pipeline permutation through one stage, multiplying against the documented
WARP software-adapter pipeline-count sensitivity, for looks the first two blends already
approximate. Two modes are testable and explainable; four are a matrix.

### Alternative C — a new dedicated feedback stage beside trails
Would duplicate the accumulation buffer (~17 MB at the float post cap) and put two
accumulators in sequence for no look the transformed trails cannot make. The trails stage
*is* the feedback stage; it gains the transform rather than gaining a sibling.

### Alternative D — engine trails only, attractor later
The recommendation the interview overrode, recorded honestly: it halves the blast radius.
The user chose consistency now; the cost is a second shader and test surface in Plan 0046,
and the plan phases the attractor last so the stage shape is proven before it is copied.

## Notes

The transform is a pure function of params and `dt` — determinism holds. The `[feedback]`
table's structural keys are load-time by the same reasoning as `[curve] family`: a warp kind
is a pipeline/shader-path choice, not a scalar, and ADR-0021 already rejected bindable
discrete indexes for the flicker/hard-cut class of reasons.
