# ADR-0085 — How much a scene occludes the backdrop is one number, at one seam

> **Status:** **accepted** (Plan 0071, closed 2026-08-09) — with an
> [Outcome](#outcome-2026-08-09-after-plan-0071) recording three claims in the body that the
> implementation falsified. The body is unedited; read the Outcome before quoting it.
> **Date:** 2026-08-04
> **Related plan(s):** [0071](../plans/done/0071-light-that-adds-without-covering.md)
> **Supplements:** [ADR-0056](0056-additive-scenes-emit-premultiplied-alpha.md)

## Context

[ADR-0056](0056-additive-scenes-emit-premultiplied-alpha.md) made every additive scene emit alpha
equal to its **coverage**, which fixed the black notches and rims: a sprite's corners outside its
inscribed disc had been writing opaque black, and premultiplied coverage made them write nothing.
That was correct and it shipped.

Its last Negative bullet left one thing open, and it is the subject here. Coverage-as-alpha means a
fragment occludes the backdrop **whatever light it emits**. The resolve at the backdrop composite is
`scene + bg * (1 - alpha)` (`core/src/render/post.rs:585` — "the last active stage lands on the
backdrop and must blend", plus its no-stage counterpart), so a fragment darkens the backdrop
wherever what it emits is dimmer than what it covers.

At the shipped floors this is invisible. All sixteen affected presets sit between `bg_bright` 0.009
and 0.070, and against a near-black backdrop nothing the figure emits is dimmer. Rendered evidence:
`swarm_storm` over `bg_bright = 0.35` at `brightness = 0.02` renders as **black specks on the
backdrop**; at the shipped-value backdrop the same run's darkest pixel is `(71,13,22)` against a
backdrop of `(138,67,56)`.

**It matters because the ADR-0056 fix invites raising those floors.** The black rim is precisely why
the swarm and line families were floored in the first place — `lsystem_fern.toml:98-103` records the
symptom, misattributing it to the lifted floor washing out the additive halo, which is a real effect
but was not the whole story. An author acting on that invitation meets a new ceiling: `bg_bright`
can rise only as far as the **dimmest emitted luminance in the figure**. Past it, the
depth-parallaxed far particles and the `glow`-dimmed strokes stop fading out and start reading as
dark speckle. `presets/README.md` now states this, which documents the ceiling without removing it.

So the question is whether coverage is the right model at all for an additive look. An additive look
arguably wants *no* occlusion — light adds, it does not cover. Nothing is broken today; post-fix is
brighter than pre-fix at every pixel. This is a look decision.

Recorded as [design-backlog 0040](../design-backlog.md#0040--additive-light-occludes-by-geometry-so-a-dim-figure-over-a-lit-backdrop-reads-as-dark-speckle).

## Decision

We will add **`occlude`**, a bindable scalar in `[0, 1]` applied at the backdrop composite, scaling
the alpha the scene's frame presents with: the resolve becomes `scene + bg * (1 - alpha * occlude)`.
At `occlude = 1.0` — the default — the arithmetic is today's exactly. At `0.0` the scene never
darkens the backdrop and the composite is pure additive light over it. Values between are a
continuous blend of the two semantics.

It lives at the **backdrop composite**, not in each scene's fragment shader. That is one uniform and
one multiply in one place, and every scene inherits it without a single scene knowing it exists.

Whether the default stays at `1.0` is decided from a **rendered sample set** — the same
concrete-examples workflow [ADR-0061](0061-kaleidoscope-edge-treatment-is-a-per-preset-choice.md)
used for the fold edge — not from the argument above.

## Consequences

### Positive

- **The `bg_bright` ceiling becomes a choice rather than a property.** An author who wants a lit
  backdrop can have one; the cost moves from "dark speckle appears and nothing explains why" to "you
  set `occlude` and picked".
- **One seam, every scene.** No per-scene shader edit, no risk of the families drifting, and no
  widening of the `Scene` trait — which is what ADR-0056's rejection of a central luminance-derived
  alpha was protecting.
- **Continuous, so it eases.** Unlike `kaleido_edge` or a shape's `points`, `occlude` has no
  quantization seam: `[smoothing]` on it is a real blend between the two models, and a preset can
  drive it off audio if that turns out to be interesting.
- **It does not reopen what ADR-0056 decided.** That ADR rejected deriving coverage *centrally from
  luminance* — a legitimately dark covered pixel would go transparent, and it puts scene judgement
  in a shared pass. An explicit per-preset scalar is a different proposal and is not covered by that
  rejection, which the backlog entry says in as many words.

### Negative

- **Two models for one thing, and the wrong one is invisible until the backdrop is lit.** A preset
  authored at `bg_bright = 0.01` cannot tell which `occlude` it wants, because both look identical
  there. That is a real discoverability cost and it lands on whoever later raises the floor.
- **`occlude = 0` blows out over a bright backdrop.** Light that never covers is light that always
  adds; the tonemap's roll-off softens it
  ([ADR-0046](0046-linear-light-hdr-composite-bloom-tonemap.md)) but does not remove it. The dark
  speckle is traded for a lifted floor, not eliminated.
- **A figure at `occlude = 0` cannot sit in front of anything.** Depth cues built from *dimming* —
  the swarm's parallaxed far particles, a `glow`-dimmed stroke — stop reading as distance and start
  reading as transparency. On a scene whose depth model is luminance, that is a real loss and it is
  the thing the sample set has to be judged on.
- **If the default flips, every fold- and backdrop-bearing baseline moves.** The plan prices that
  explicitly rather than discovering it; at the default of `1.0` nothing moves at all, exactly.

### Neutral

- The name is about what it does to the *backdrop*, not about the scene's blend mode. The scene's
  colour blend is unchanged and stays additive; only how much of the backdrop survives underneath is
  what this scales.

## Alternatives considered

### Alternative A — A two-valued enum: `cover` or `add`

The shape the backlog entry proposed ("a bindable choice between the two semantics"). Rejected
because it is strictly less expressive at identical cost — the scalar contains both endpoints — and
because an enum introduces a quantization seam under `[smoothing]` for no benefit, which is a
failure mode this project has now hit three times (`kaleido_order`, `variant`, `points`).

### Alternative B — Derive the alpha from emitted luminance, centrally

Make the seam transparent wherever the figure is dark, so dim fragments stop occluding
automatically. Rejected by ADR-0056 and still rejected: a legitimately dark *covered* pixel would go
transparent, and deciding "is this pixel dark because it is far away or because it is meant to be
black" is scene judgement running in a shared pass.

### Alternative C — Per-scene occlusion semantics rather than per-preset

Let each scene declare whether it occludes, since the depth-cue cost falls unevenly (the swarm has a
luminance depth model; the line families do not). Rejected because it removes the choice from the
person who can see the frame — a preset is where a backdrop and a figure meet, and it is the only
level at which the question "does this look better" has an answer.

### Alternative D — Document the ceiling and change nothing

`presets/README.md` already states that `bg_bright` can rise only to the figure's dimmest emitted
luminance. Rejected because the ADR-0056 fix specifically invited raising those floors, and a
documented ceiling on an invited change is a trap with a sign on it rather than a resolution.

## Notes

- Rendered evidence at both ends is in
  [backlog 0040](../design-backlog.md#0040--additive-light-occludes-by-geometry-so-a-dim-figure-over-a-lit-backdrop-reads-as-dark-speckle):
  `swarm_storm` over `bg_bright = 0.35` at `brightness = 0.02` gives black specks; the same run at
  the shipped floor gives `(71,13,22)` against `(138,67,56)`.
- The sixteen presets whose floors sit between 0.009 and 0.070 are the population any sample set
  should draw from, and the judgement has to be made over a **lit** backdrop — at
  `bg_bright = 0` the two models are identical, which is the same confirmation failure
  ADR-0061's Notes records for the fold edge.
- `lsystem_fern.toml:98-103` carries a comment attributing its floor to the wrong cause. Whatever
  this decides, that comment should stop being wrong.

## Outcome (2026-08-09, after Plan 0071)

The decision held: `occlude` shipped as a bindable scalar in `[0, 1]` at the backdrop composite,
the resolve is `scene + bg * (1 - alpha * occlude)`, and **the default stayed at `1.0`** — decided
by the user in the running app over a sample set at `bg_bright` 0.35 and 0.60, on the grounds that
at shipped brightnesses the difference is almost negligible. No preset binds it; no golden baseline
moved, measured as byte-identity against a clean-`main` bless rather than argued from the literal
`1.0`.

**Three claims in the body above are false, and all three are in the same sentence-family — the one
that said this was cheap because it was central.**

1. **"One uniform and one multiply in one place"** (Decision) and the plan's "one seam in
   `post.rs:585`" are both wrong: there is no such seam. `post.rs` chooses a *load op*; the
   arithmetic is fixed-function blending in whichever pass lands on the backdrop, and the alpha that
   blend consumes is emitted by that pass's own shader. The factor is applied in **six** places —
   the three post stages through a new `Fold::Over { occlude }`, and the three premultiplied-present
   scenes through a new `Scene::set_occlude`. Four shader bodies were edited, not zero.

2. **"No widening of the `Scene` trait"** (Consequences → Positive) is false. The trait gained
   `set_occlude`, its **fourth** optional method and its **second** per-frame one. That is
   [ADR-0030](0030-scene-target-size-hot-path-hook.md)'s territory, and its three conditions were
   never applied during design. Applied at the Plan 0071 close, the hook **passes all three**: the
   effective value is renderer-held state no scene can reach (it is a literal `1.0` whenever a post
   stage is active, which depends on `Routing::scene_stage`, not on the preset); the implementors
   store an `f32` and do no GPU work; and the default is a no-op, so ISP holds and every scene stays
   substitutable. Recorded here rather than left implicit, because ADR-0030 says in as many words
   that the conditions are a review obligation and not a compile-time one — and because this ADR
   claimed the widening would not happen at all.

3. **"No risk of the families drifting"** (same bullet) is false, and the plan's matching assumption
   that the two composite paths do the same arithmetic is false with it. The additive families
   (swarm, lines, emitter) blend colour `One`/`One`, so **with an empty chain their backdrop already
   survives whole** — there is no occlusion at that seam for `occlude` to scale, and those presets
   behave as `occlude = 0` whatever they bind. It reaches them only through the chain's last stage,
   which every shipped preset in those families happens to have. So the engine has one documented
   default and two behaviours, and the asymmetry is now stated in `presets/README.md` and in
   `Scene::set_occlude`'s doc comment rather than discovered.

**One thing the ADR did not anticipate and the implementation had to buy: two bind-group layouts.**
The trails present and the attractor present carried no uniform, and `occlude` needed one. The first
attempt gave each a second group holding the uniform alone — `[Uniform]`, which is
`background-bind-layout`'s shape — and on the DX12 WARP software adapter the trails present then
read *the backdrop's* buffer: `occlude` moved 0 of 196 608 channels there while moving 3 307 on the
hardware adapter, with the whole capture suite green over it. That is
[ADR-0058](0058-bind-group-layout-collisions-carry-evidence.md)'s hazard, reproduced a fourth time.
Both layouts now use shapes the crate does not otherwise have (`[Sampler, Uniform, Texture]` for
trails; `[Texture, Sampler, Uniform, Sampler]` — the sampler bound twice — for the attractor), and
both are asserted against the crate-wide enumeration in `tonemap/tests.rs`. **A side effect worth
carrying forward: this empties ADR-0058's `[Texture, Sampler]` collision group entirely**, which was
the pair that ADR named as live on shipped content and made a Positive bullet of covering. Plan 0053
must derive its allowlist from the code.

**Still outstanding:** Plan 0071 Phase 5, the `preset-author` retune this unblocks, is deliberately
undone. The plan groups it with design-backlog 0038 **and 0058** as one pass over the shipped set;
0058 closed by content on 2026-08-04, before the plan reached that phase, so it is a two-way pass
with 0038 alone. Tracked in `docs/plans/README.md` → Standing.
