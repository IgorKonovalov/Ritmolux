# ADR-0031 — Post-composite stages behind a `PostStage` trait; the composite becomes an instantiable ordered chain

> **Status:** accepted (2026-07-25)
> **Date:** 2026-07-25
> **Related plan(s):** [0030](../plans/done/0030-composite-chain-and-scene-keying.md) (implemented
> this; closed 2026-07-25); unblocks [0023](../plans/done/0023-cross-preset-transitions.md)

## Context

[ADR-0018](0018-engine-wide-scene-compositing.md) fixed the engine composite as an ordered,
individually-skippable pipeline — background pre-pass → scene → feedback trails → screen-space
kaleidoscope → present — and deliberately rejected a render graph: the order is a product decision,
not a runtime one. [ADR-0028](0028-final-stage-ink-tone-remap.md) then appended the ink tone-remap as
the final stage. Each stage landed as its own struct in `core/src/render/`, and each independently
grew the same surface: `new` / `reset_params` / `set_param -> bool` / `reset_resources` / `active` /
`begin` / `resolve`, plus fixed-resolution accessors on two of the three. Nothing declares that
surface. `Kaleidoscope::begin(&mut self, _encoder)` carries an unused parameter purely to match
`Trails::begin`'s signature — the interface exists in fact and not in code.

`Renderer::draw_frame` composes them with a hand-written branch ladder over one boolean per stage
(`trailing` / `kaleidoing` / `inking`): roughly 70 lines that answer "where does the scene render"
and "which stage resolves into which", with `ink.begin()` appearing at three separate call sites and
the frame's `draw_calls` total assembled by hand arithmetic. The ladder enumerates orderings rather
than deriving them, so each new stage roughly doubles it. None of that routing has a unit test — it
is exercised only indirectly, through WARP captures of presets that happen to bind the right params.

Two forces make this a decision rather than a tidy-up. First, [Plan 0023](../plans/done/0023-cross-preset-transitions.md)
(status `approved`) appends a **fourth** stage, and it is two-input — it samples the outgoing and
incoming frames. Second, that plan's Phase 3 renders **two fully-composited frames in one frame**, and
its own risk list records the consequence: "in dual-live, each side needs its own feedback field". The
stages today are single-instance fields on `Renderer`, each owning a single offscreen and (for trails)
a single `PingPongField`. A second live composite is not expressible without duplicating every stage
field and writing the ladder twice.

The project deliberately keeps a small, countable set of extension seams — the C ABI and the `Scene`
trait — and [ADR-0030](0030-scene-target-size-hot-path-hook.md) set three conditions any widening of
`Scene` must meet. Declaring a *third* internal seam is therefore an ADR-worthy event, even though the
code it replaces is code we already have.

## Decision

We will declare a crate-internal `PostStage` trait and hold the post-composite stages in a `PostChain`
**value** that owns them in ADR-0018's fixed order. `draw_frame` asks the chain where the scene should
render, and then asks the chain to fold itself down to the surface; the chain walks its active stages
in order, each resolving into the next active stage's input and the last into the surface view. The
order stays a **compile-time constant array** constructed inside `PostChain::new` — this is not a
registration point, not a plugin API, and not a graph; ADR-0018's rejection of runtime-variable
ordering stands. Because the chain is a value rather than a set of `Renderer` fields, a second
instance is constructible with fully independent GPU state, which is precisely what Plan 0023's
dual-live path requires. The routing decision itself — which active stage resolves into which — is
factored out as a **pure function over the active flags**, so the contract is unit-testable with no
GPU. `Background` stays outside the trait: it is a pre-pass that owns the frame clear and never folds
a rendered frame down, so forcing it into the same shape would widen the trait for one non-member.

## Consequences

### Positive
- Adding a stage becomes one array element plus one trait impl. Plan 0023's blend stage stops being a
  ladder rewrite.
- `reset_params`, `set_param`, and `reset_resources` become loops over the chain instead of four
  hand-written fan-outs that a new stage must be added to (and can be forgotten from). `draw_calls`
  becomes a sum of what each stage reports rather than hand arithmetic.
- The routing contract gets its first real test coverage, GPU-free, because the adjacency computation
  is separated from the GPU calls.
- Dual-live becomes wiring: construct a second `PostChain`, run both, hand two views to the blend
  stage. The trail-field-ownership risk Plan 0023 flags is answered by construction — each chain owns
  its own field.
- Ink's surface-sizing stops being a special case in the renderer: one `internal_size() ->
  Option<(u32, u32)>` where `None` means "size from the surface" replaces two per-stage `aspect()` /
  `size()` associated functions plus an ink-shaped branch.

### Negative
- **A third internal seam to maintain and document.** The countable-seams property was a feature; this
  spends it. The mitigation is that the seam is `pub(crate)`, has a fixed membership list in one
  function, and is not reachable from a preset or the C ABI — but it is still one more interface that
  can drift.
- **The trait must span genuinely different stages**, and the fourth one is the hard case. Plan 0023's
  blend stage takes *two* inputs; a one-input `begin` cannot express it, so it will need either a
  wider `begin` or a second trait method — exactly the ISP erosion this ADR exists to prevent. We
  accept that the trait will be revisited when Plan 0023 lands, and we state the bound now: a stage
  that needs a method no other stage implements is a signal the stage does not belong in the chain,
  not a licence to widen it.
- **~4 dynamic dispatches per frame** where there were direct calls. Unmeasurable against a render
  pass, but not zero, and it is on the hot path.
- `begin` returns an **owned** `wgpu::TextureView` rather than a borrow. That is an implementation
  concession to the borrow checker (the caller renders the scene between `begin` and `resolve`, so a
  `&self`-borrowed view cannot survive), paid for by `TextureView` being `Clone`/Arc-backed in wgpu 30
  — an atomic increment, not a resource copy. It is still a small per-frame cost that the current
  direct-field code does not pay.
- One more indirection between "read the preset param" and "see the pixel change", which costs a
  reader of the composite something even as it saves the reader of `draw_frame`.

### Neutral
- The three stage structs keep their files, their shaders, and their lazy-build discipline unchanged;
  only their surface is renamed into the trait and their composition moves out of `draw_frame`.
- Stage params keep their existing `bg_*` / `trails` / `kaleido_*` / `ink_*` namespaces and their
  first-owner-wins routing, so no preset changes and no golden re-bless.

## Alternatives considered

### Alternative A — Extend the branch ladder per stage (the status quo)
Keep each stage a `Renderer` field and add another boolean and another nesting level for each new
stage. Rejected because Plan 0023 breaks it twice over: the ladder would have to enumerate a fourth
stage's orderings, *and* be written a second time for the outgoing chain, with every stage field
duplicated beside it. The status quo is affordable at three stages and not at four-times-two.

### Alternative B — A general render graph (nodes, edges, runtime resolution)
Model the composite as a DAG resolved per frame. Rejected — again, and for ADR-0018's original reason:
the stage order is a fixed product decision, so runtime flexibility buys nothing we want and costs
per-frame resolution work plus a large amount of machinery. A fixed array walked in order is the graph
we actually need.

### Alternative C — Make each post stage a `Scene`
Reuse the existing seam instead of adding one. Rejected because the two things consume different
inputs: a `Scene` takes an `AnalysisFrame` and draws content; a post stage takes an already-rendered
frame and transforms it. Fusing them would add post-stage methods to the trait every content scene
implements (failing ADR-0030's condition 1) and would put post-effects into the preset `system`
vocabulary, where they do not belong — a preset selects one system and composes stages on top of it.

### Alternative D — Declare the trait but keep the ladder
Take the copy-paste win (loops for `reset_params` / `set_param` / `reset_resources`) and leave
`draw_frame`'s routing alone. Rejected because the routing is the half Plan 0023 cannot extend; this
would spend the ADR and leave the blocking problem in place.

## Notes

- `wgpu::TextureView` is `#[derive(Debug, Clone)]` in wgpu 30 (`wgpu-30.0.0/src/api/texture_view.rs:15`),
  which is what makes the owned-return `begin` cheap enough to prefer over fighting the borrow chain.
- Plan 0023's Phase 3 wording and its "a kaleidoscope or trail stage that assumes it is last may need
  the blend to sit outside it" risk bullet both become simpler once this lands; revising them is a
  followup on Plan 0030, not a change to the approved plan's scope.
