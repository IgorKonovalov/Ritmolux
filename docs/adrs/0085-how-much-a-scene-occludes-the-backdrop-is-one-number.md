# ADR-0085 — How much a scene occludes the backdrop is one number, at one seam

> **Status:** proposed
> **Date:** 2026-08-04
> **Related plan(s):** [0071](../plans/0071-light-that-adds-without-covering.md)
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
