# ADR-0048 — Transformed feedback: the accumulation buffers resample the past through a bindable affine + curated-warp transform, with a selectable deposit blend

> **Status:** accepted
> **Date:** 2026-07-30
> **Related plan(s):** [0046](../plans/done/0046-transformed-feedback.md) (R2 of [docs/roadmap-visual-richness.md](../roadmap-visual-richness.md))
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

## Outcome — 2026-08-09, at Plan 0046's close

The design shipped as decided and the look passed on the wall (Phase 5: *"very good"* on the
`swirl` + `add` echo, which is the reference look this ADR exists to reach; no warp kind is
fundamentally wrong, and nothing routed back here). **Every `fb_*` default is the identity and no
golden moved** — all 20 pre-existing baselines hash-identical to a clean-`main` bless, three added.
Five things this ADR asserted came back different, and one of them narrows the Decision.

### The Decision's "one vocabulary, two buffers" is true of everything except `blend`

The seven `fb_*` params and `[feedback] warp` reach **both** sinks. **`[feedback] blend` reaches
the trails stage only.** The attractor's deposit has been additive since the scene was written —
its points draw through an additive pipeline over the decayed bed, in one pass — so there is no
`max` to select there without a **second draw pipeline**, which is precisely the
coexisting-pipelines-with-matching-bind-layouts shape
([ADR-0058](0058-bind-group-layout-collisions-carry-evidence.md)) that Alternative B's rejection and
the one-shader warp family both exist to avoid. Paying a known WARP hazard to make one sentence
literally true is the wrong trade, so the sentence is what changes. The asymmetry is documented
where an author meets it (`presets/README.md`, "One vocabulary, two buffers"), alongside the second
one measurement turned up: **the trails stage is invisible unless its tail outlasts the scene's own
`fade`** — over a `fade = 0.95` attractor, `trails = 0.9` is a bit-for-bit passthrough.

### Alternative D's price was overestimated, and that is the more interesting half

Including the attractor was accepted at a stated cost of "a second shader and test surface". It
cost rather less. The transform's arithmetic, the seven param names, the identity predicate and
the aspect correction all live **once**, in `core/src/render/feedback.rs` — the existing
`PingPongField` module, i.e. the ADR-0012 seam this ADR's own Supplements line says it was always
for — and the WGSL is **one snippet concatenated into both shaders**. Both sinks delegate
`set_param` to the shared `Transform` rather than matching seven names each. So the two buffers
cannot drift apart on what `fb_rotate` means by construction rather than by discipline, which is
the outcome Alternative D was worried about not getting.

### The `Scene` seam widened again, and the routing grew a second fan-out

The Decision says the attractor consumes the vocabulary through "its `Scene::set_param`". That is
true of the *params* and not of the *table*: the structural `[feedback]` choice needed a new
`Scene::set_feedback`, a default-no-op fourth optional trait method, and `PostStage` needed two —
`set_feedback` and `set_dt`, both one-way pushes on `set_exposure`'s established route. Routing
grew `ParamRoute::StageAndScene(usize)`, the enum's second fan-out and the mirror of
`SceneAndBackdrop`: there the name belongs to the system and reaches the sky as a courtesy, here it
belongs to a stage and reaches the scene because the scene declared it. Recorded because
[ADR-0085](0085-how-much-a-scene-occludes-the-backdrop-is-one-number.md)'s `Outcome` had to record
the same kind of widening one plan earlier, and "the `Scene` trait stays thin" (ADR-0002) is a claim
that is now being paid for in small instalments.

### ADR-0037 was caught here for the third time, and the fix carries a negative control

`Trails::resolve` had been **ignoring its `surface` argument** on a documented and, until this
change, correct premise: neither of its passes computed geometry. A rotation does. The transform
now takes its aspect from the render target, and the guard is not a claim but a measurement — a
rotation spun into a closed ring at a **portrait 100x160** target has a pixel bounding box of
**45x46**, against **44x71** with the aspect deliberately forced to `1.0`. That is the shape this
rule needs every time: a value sourced from two places that agree at 16:9 cannot be tested at 16:9.

### The edge policy the Negative left open: transparent border, by shader rather than by sampler

Evaluated at portrait per ADR-0047's lesson. `ClampToEdge` re-deposits the border texel every frame
and compounds it into a permanent bar of colour along the two long edges — worst exactly where a
tunnel wants empty space to travel into. Off-frame reads therefore contribute **nothing**, and it is
implemented as a **shader test** rather than `AddressMode::ClampToBorder`, which is an optional wgpu
feature this project cannot require on every adapter it ships to.

### Two smaller corrections

The `fade` normalization shipped as `fade^(dt / FALLBACK_DT)` with an `exponent == 1.0`
short-circuit, not the `fade^(dt*60)` written above: `x / x` is exactly `1.0` in IEEE, and `powf` is
not required to return `x` for an exponent of one. **The attractor's decay keeps the older
`powf(dt * 60.0)` with neither guard** — harmless today and hash-identical, but the two sinks are
not in fact using the same form, which is the opposite of what the Decision says. And the
`add`-mode convergence property held as specified: 360 frames of a static bright source move the
frame by 97 bytes over the first 60 and **0** over the last 60.

### What Phase 5 measured that is not this ADR's business but should not be lost

`frame_ms_p99` spikes to 25.0 ms on preset switches while `frame_ms_avg` never passes 8.7 ms and no
frame drops — [backlog 0082](../design-backlog.md), because the not-yet-built quality governor is
specified to read that column. And RSS grew 385 to 663 MB over three minutes of switching, with **no
no-feedback control beside it** — [backlog 0083](../design-backlog.md). Neither blocks this ADR;
both are owed before R6 ships long-running feedback content.
