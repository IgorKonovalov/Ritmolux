# 0090 — A preset composes two scene layers: a per-preset join point, linear-light blend at the `over` join, per-layer scene instances

> **Status:** accepted (2026-08-09 — the four load-bearing choices were put to the user as an
> interview and each was decided explicitly; see Decision)
> **Date:** 2026-08-09
> **Related plan(s):** [0076](../plans/done/0076-the-second-layer.md)
> **Supersedes, in part:** [ADR-0018](0018-engine-wide-scene-compositing.md) Alternative A and
> [ADR-0031](0031-post-stage-trait-instantiable-composite-chain.md) Alternative B — both rejected
> a render graph, and both rejections **stand**; what this ADR revises is the "one scene per
> preset" premise underneath them, at minimum viable scope and with new evidence (below).
> **Builds on:** [ADR-0046](0046-linear-light-hdr-composite-bloom-tonemap.md) (the linear-light
> composite the blend modes require), [ADR-0024](0024-cross-preset-transitions.md) (dual-live —
> the in-tree proof two full composites are affordable), [ADR-0056](0056-additive-scenes-emit-premultiplied-alpha.md)
> (the alpha the `over` blend consumes)
> **Serves:** [roadmap-visual-richness R3](../roadmap-visual-richness.md)

## Context

A preset names exactly one `system` (`schema.rs`), and the composite runs one scene through one
fixed chain. The consequence, named by the visual-richness review as wrong turn 4: a layered
composition — a particle figure **over** a warped field, a crisp rose floating on a molten
kaleidoscoped ground — is not expressible by any preset, and the layered-collage reference look
is out of reach. The renaissance ([ADR-0089](0089-the-library-renews-by-replacement-cohorts.md) /
[Plan 0075](../plans/0075-the-content-renaissance.md)) is about to author a fresh library; every
cohort authored before this capability exists is a cohort that cannot contain a layered world.

The render graph was rejected twice (ADR-0018 Alt A, ADR-0031 Alt B), correctly, on YAGNI
grounds — and this ADR does not reopen it: the chain order stays a compile-time product
decision. What has changed since those rejections is **evidence**:

- The dissolve engine runs **two fully-composited frames with independent `PostChain`s in one
  frame** today (ADR-0024's dual-live, tested by Plan 0030). A second scene layer is a
  *generalization of shipped machinery*, not new invention.
- ADR-0046 moved the composite to **linear light**, which is the precondition for blend modes
  other than `add` meaning anything (`screen`/`multiply`/`overlay` on 8-bit gamma were never
  worth building).
- The roadmap's reference table shows an entire look family ("layered translucent collage")
  whose only gap is this.

Four sub-decisions were genuinely open; all four were put to the user on 2026-08-09 and
decided. The one decided **against** the architect's recommendation is recorded as such.

## Decision

A preset may declare **one optional second scene layer** in a `[layer]` table. Concretely:

1. **A per-preset join point** (`join = "under" | "over"`, default `under`) decides which
   post stages apply to the layer:
   - **`under`** — the layer draws into the same scene target as the main scene, before the
     post chain. Both layers share trails, kaleidoscope and bloom and fuse into one substance.
     Accumulation is the scene target's existing additive/premultiplied seam; no blend mode
     applies. Cost: one extra scene draw.
   - **`over`** — the layer renders into its own offscreen and is blended into the chain
     **between the kaleidoscope and bloom**: exempt from the *resampling* stages (its geometry
     is never smeared by trails nor folded by the kaleidoscope) but participating in the
     *luminous* stages (it blooms and tonemaps with the frame, in linear light, before ink).
     Cost: one offscreen target plus one blend pass.
2. **Blend modes at the `over` join**: `add`, `screen`, `multiply`, `overlay`, fixed at load
   (`blend = "screen"`), operating in linear light within the layer's premultiplied-alpha
   footprint (ADR-0056's coverage is what bounds where a darkening mode may darken). The
   layer's **mix amount is a bindable scalar** (`mix` in the layer table), so audio can surge
   the second layer on a drop and a preset can breathe between one world and two.
3. **Full authoring surface, shared palette.** The `[layer]` table carries its own `system`,
   params, bindings and `[layer.smoothing]` — everything a top-level preset has — but the
   preset's single `[palette]` serves both layers, so a world keeps one colour language and
   one baked LUT. Layer params are **namespaced to the layer**, not merged into the main
   scene's first-owner-wins routing.
4. **Per-layer scene instances, from the start** (the user's explicit call, against the
   architect's different-systems-only recommendation). The layer's scene is **constructed for
   the preset**, not borrowed from the prebuilt one-instance-per-system roster, so
   **same-system pairs are legal** — two fragment fields at different zooms, two curve figures
   in counterpoint. This is the same singleton problem ADR-0024 dodged with its same-slot
   freeze rule, now solved rather than dodged; the decisive argument was that two-of-one-kind
   is wanted *content*, and deferring hardens the singleton assumption further into the
   registry.
5. **Both tiers render the layer.** A layer is content, not garnish: `Floor` renders both
   layers at its existing per-scene budgets, the frame-time governor handles sustained misses
   as it does today, and the behavioral gates (which pin `Floor`) see the same picture an
   iGPU user gets. Heavy-plus-heavy pairings are an authoring responsibility.

**What stays fixed:** the chain order (compile-time, ADR-0031), the two extension seams (C ABI
untouched — the layer arrives entirely through preset data; `Scene` trait unwidened — a layer
scene is an ordinary `Scene` driven twice per frame by the engine), and the one-`[palette]`
colour model. In v1 the `over` layer carries **no post stages of its own** — no private trails
or fold; a full second `PostChain` for the layer is the named deferred alternative, waiting on
a cohort that demonstrates the want.

## Consequences

### Positive

- The layered-collage reference look becomes reachable, and the renaissance cohorts can
  contain layered worlds — the acceptance test is that look, rendered.
- Both joins are cheap relative to what dual-live already pays: `under` is one scene draw,
  `over` is one offscreen and one blend pass. The 2x-composite cost the roadmap priced R3 at
  is *not* incurred, because the v1 `over` layer has no chain of its own.
- Same-system layering ends the registry's one-instance-per-system assumption deliberately,
  with the migration paid once, instead of the assumption hardening under more code.
- The blend lands where ADR-0046 prepared the ground: linear light, pre-tonemap, so `screen`
  and `overlay` behave like their definitions rather than like gamma-space approximations.

### Negative (the price we pay)

- **The scene registry grows a second construction path.** Per-layer instances mean the
  stateful families duplicate GPU state when layered — a second RD ping-pong field, a second
  particle buffer (~7 MB at the attractor's ceiling per ADR-0087's arithmetic). Bounded and
  paid only by presets that declare a layer, but it is the largest engine change in the plan
  and it touches the line families' shared `LineRenderer` idiom.
- **The dissolve's eligibility rule extends.** Dual-live between two layered presets is up to
  four composited scenes in flight; the existing budget gate plus a shared-anything check must
  decide freeze more often. The freeze fallback already exists, so this is a rule extension,
  not new machinery — but transitions will freeze more often on layered content.
- **The gates' per-preset cost rises** for layered presets (two scenes per capture), on a
  suite CI already pays for more than once per push. Measured at implementation; if material,
  said rather than absorbed.
- **The author surface grows a whole table**, and the operator docs (`presets/README.md`,
  `docs/presets.md`, `docs/preset-palettes.md`) must present the layer as a first-class
  citizen or the content lane authors against a surface it half-knows.
- A preset can now be **wrong in a new way** — a layer whose system matches nothing, a `blend`
  on an `under` join (ignored, warned), a heavy pair on Floor. Load-time validation and the
  gates absorb what they can; the rest is authoring judgement.

### Neutral

- The C ABI stays v4; the foobar frontend gets layered presets for free through the same
  render call.
- Presets that declare no `[layer]` are byte-identical in cost and pixels; no golden baseline
  moves at the capability's landing.

## Alternatives considered

- **A general render graph** (re-rejected, third time). The join point is one junction with
  two positions, not an edge list; ADR-0018's reasoning still holds at this scope, and the
  fixed order remains a product decision.
- **A fixed join — always `under`** (cheapest) or **always a full second chain** (dual-live
  shape). Rejected by the interview: the first makes the collage look impossible, the second
  makes every layered preset pay for expressiveness most will not use. The per-preset join
  puts the cost on the presets that want it.
- **Different-systems-only in v1** (the architect's recommendation). Rejected by the user:
  same-system pairs are wanted content, and the singleton workaround would harden. Recorded
  honestly — this is the decision that turns a schema-plus-routing plan into one with a real
  registry migration in the middle.
- **Per-layer `[palette]`.** Rejected: two colour languages per preset reads as two presets
  stacked, and it doubles the LUT bake for a coherence loss, not a gain. The shared palette is
  the thing that makes two layers one *world*.
- **Rich-only layers** (Floor drops layer 2), or **Floor forcing `under`**. Rejected: a
  two-layer world losing half its content — or its structure — on weak hardware doubles the
  authoring burden and splits the gates' evidence from the iGPU user's picture.
- **An `over` layer with its own `PostStage` chain** (private trails/fold). Deferred, not
  refused: it is the dual-live shape and the machinery exists, but no look has asked for it
  yet, and the renaissance's cohorts are exactly the instrument that will say if one does.

## Notes

- The `over` join's position (between kaleidoscope and bloom) is the one place the chain
  order is touched, and it is still compile-time: the blend is a junction the `PostChain`
  walk knows about, not a reorderable stage.
- Determinism holds by construction: a layer scene is seeded like any scene, the blend is a
  pure function of two textures and a bound scalar, and captures pin `Floor` as always.
- The reachability instrument (ADR-0042/0043) must walk layer bindings' expression trees the
  same as top-level ones — they are the same `Binding` machinery, so this is a property to
  verify at implementation, not new design.
