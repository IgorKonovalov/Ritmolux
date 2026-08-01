# ADR-0030 — Third `Scene` widening: a per-frame target-size hook; hot-path notifications are now in scope

> **Status:** accepted
> **Date:** 2026-07-25
> **Related plan(s):** 0027-attractor-ink-and-crisp-trails (Phase 2)

## Context

Plan 0027 Phase 2 replaced the attractor's fixed 640x360 trail accumulation grid with one sized to
what the scene actually renders into. The scene therefore needs the pixel size of its render target,
and in this engine nothing carried it. `Scene::render` receives `queue`, `encoder`, `&TextureView`
and `aspect` — a `TextureView` does not expose its extent, and `aspect` is a ratio, so the size is
unrecoverable inside a scene. Scenes are `Box<dyn Scene>`, so there is no downcast path either.
Plan 0027 had written "no `Scene`-trait change" into its **What this plan does NOT do**, which made
the phase internally contradictory; the contradiction was surfaced during implementation and
resolved by the user in favor of the hook.

The trait has a documented widening budget and this spends the last of it.
[ADR-0007](0007-line-geometry-generators.md) added `configure` and called it "exactly one optional
method", naming further growth as a thing "the reviewer must watch".
[ADR-0021](0021-shared-palette-system.md) then added `set_palette`, described itself as "the second
and last thin **off-hot-path** widening", and closed with an explicit instruction: "a third widening
should prompt asking whether the seam is still the right one." This ADR is that ask, answered.

The new hook differs in kind from both predecessors. `configure` and `set_palette` fire **once at
preset load, off the hot path** — that off-hot-path property was load-bearing in how both ADRs
justified themselves. A target size is not a load-time fact: it changes with the window, and it also
changes with the *composite*, because the renderer routes the scene into the trails or kaleidoscope
stage's fixed 1280x720 grid when either is active and into the surface otherwise. The only correct
value is the one the renderer computes per frame, so the notification is per frame.

## Decision

We will widen the `Scene` trait with a third optional method — a per-frame notification carrying the
pixel size of the target the scene is about to render into (`core/src/render/scenes/mod.rs`, called
from `draw_frame` immediately before `Scene::render`) — and we will **retire ADR-0021's
"off-hot-path" qualifier on the widening budget**. Optional `Scene` methods may now fire per frame,
under three conditions that this hook satisfies and that any future one must:

1. **The renderer holds state no scene can otherwise reach.** The hook exists because the value is
   genuinely unavailable through the existing signature, not because it is convenient. If the datum
   can ride an existing channel — a named param, `configure`, `set_palette`, `render`'s arguments —
   it does.
2. **The implementor compares first and does nothing when unchanged.** The call is unconditional per
   frame, so all cost in the steady state must be a compare. The attractor records the requested
   grid and lets the next `render` notice the difference; it does no GPU work inside the hook.
3. **Default no-op.** Non-participating scenes stub nothing, so ISP holds and the trait stays
   substitutable.

We will **not** treat this as a general resize/lifecycle event channel. The method is named for what
it carries (the render target's size), not for the event that changed it, because the renderer calls
it every frame and the value depends on the active post-stages as much as on the window. The two
extension seams ADR-0002/ADR-0001 fixed — the C ABI and the thin `Scene` trait — are otherwise
unchanged: the C ABI is untouched by Plan 0027, and `Scene` gains no engine-lifecycle or GPU-backend
knowledge.

## Consequences

### Positive
- **A scene can size an internal field to its target.** That is the whole of Plan 0027 Phase 2: the
  attractor's accumulation grid follows the target up to a 2560x1440 cap instead of upscaling from a
  fixed 640x360, which is what read as soft on a 1080p+ display.
- **The value is correct under compositing, not just under resize.** Because the renderer computes
  it after choosing the composite chain, a scene drawing into the trails or kaleidoscope stage
  learns that stage's grid rather than the window size, and does not supersample into a smaller
  offscreen.
- **The budget is now stated as a rule instead of a countdown.** ADR-0007 and ADR-0021 each said
  "this is the last one" and each was overtaken. Three conditions that a candidate either meets or
  does not is a test a future reviewer can actually apply.

### Negative
- **The trait is three methods past its ADR-0002 shape**, and one of them is now unconditionally on
  the hot path. `Scene` is drifting from "the vocabulary the preset engine drives" toward "the
  interface the renderer drives", and each widening makes the next one easier to argue for. The
  three conditions above are the brake; they are a review obligation, not a compile-time one.
- **Condition 2 is unenforced.** Nothing stops an implementor from allocating or building GPU
  resources inside the hook. The panic-denial pragma does not cover cost, and no test asserts the
  unchanged case is free. It is caught in Mode 4 review or not at all.
- **The generalization is narrow.** Only the attractor implements the hook. The reaction-diffusion,
  trails, and kaleidoscope stages keep their own fixed internal resolutions, so the engine now has
  two resolution policies side by side — deliberately, per Plan 0027's scope, but it is a seam that
  will want unifying.

### Neutral
- The hook makes a resize semantically destructive for the attractor: the rebuilt field is undefined,
  so the trail clears and the particles re-seed. That is a scene-implementation consequence, not a
  trait one, and its cost is Plan 0029's subject.

## Alternatives considered

### Alternative A — Widen `Scene::render`'s signature to carry the size instead of `aspect`
Pass `(width, height)` to `render` and let a scene derive `aspect` itself. Rejected: it is a
*breaking* change to the one method every scene implements — five scenes edited to consume a datum
four of them ignore — where the optional hook is additive and costs non-participating scenes nothing.
It also conflates two things: `aspect` is a projection input every scene needs, the target size is an
allocation input one scene needs.

### Alternative B — Route the size through a named param
Have the renderer synthesize `target_width`/`target_height` into the existing `set_param` channel.
Rejected: named params are the *preset's* vocabulary (ADR-0002 layer 2) — engine-injected values in
that namespace would be indistinguishable from preset bindings, would collide with the author-facing
param surface, and would arrive as `f32` for what is a `u32` allocation size. It smuggles an engine
channel through a user-facing one to avoid admitting a trait change.

### Alternative C — Keep the fixed grid and raise the constant
Bump 640x360 to, say, 1920x1080 and leave the trait alone. Rejected: it pays the full fill bill of
the largest supported display on every machine including the iGPU floor (NFR §1), and it is still
wrong at both ends — soft above the constant, wasteful below it, and never matched to the
trails/kaleidoscope grids when those stages are active. The size has to be dynamic to be right; once
it is dynamic it has to reach the scene.

## Notes

- This ADR supersedes the "second and **last**" phrasing in ADR-0021's Decision and the "exactly one
  optional method" bound in ADR-0007. Neither ADR is edited (append-only); both remain accurate about
  the method they introduced.
- Plan 0027's "What this plan does NOT do" line claiming no `Scene`-trait change is corrected in the
  plan's close commit rather than left as drift.
- The cost of the attractor's *response* to a size change — a full `Resources` rebuild (shaders,
  pipelines, particle buffer, LUTs) inside `render`, which hitches badly during a live window drag —
  is a scene-implementation defect found in the Plan 0027 review and routed to
  [Plan 0029](../plans/done/0029-attractor-resize-cost-and-ink-followups.md). It does not change this
  decision.
