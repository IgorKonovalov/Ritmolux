# ADR-0201 — A fullscreen scene presents premultiplied over the backdrop, and the chain's occlude value is the only switch

> **Status:** proposed
> **Date:** 2026-09-14
> **Related plan(s):** [0185](../plans/0185-a-fullscreen-field-lets-the-sky-through-with-no-post-stage.md)
> **Extends:** [0026](0026-full-composite-coverage-fullscreen-scenes.md) (full composite coverage for
> fullscreen scenes), [0085](0085-how-much-a-scene-occludes-the-backdrop-is-one-number.md) (how much a
> scene occludes the backdrop is one number)

## Context

ADR-0085 made `occlude` one engine-wide number: `out = scene + bg * (1 - alpha * occlude)`. Two passes
can land on the backdrop, and `composite_into` in `core/src/render/composite.rs` decides which one
owns the seam each frame. When a post stage is active, or an `over` layer is live, the scene draws
into a scratch target. It is handed `set_occlude(1.0)`, and the chain's last fold applies `occlude`.
When the chain is empty, the scene draws straight onto the backdrop, is handed the chain's `occlude`,
and must resolve the seam itself.

A scene resolves that seam through its present blend. The scenes that composite over the backdrop
correctly, `reaction_diffusion`, `cellular`, the attractor and `warp_mesh`, present with
`PREMULTIPLIED_ALPHA_BLENDING`. Four fullscreen scenes write `occlude` into their alpha and present
with `REPLACE`: `fragment_field`, `analytic_field`, `shape_field` and `shape_collage`. On the
no-stage path their alpha is written and never read, so `occlude = 0` lets no backdrop through
(backlog 0206). With a stage active they are correct, because the chain applies the seam. That is why
every existing test and preset missed it.

The decision is where the blend is chosen. The chain knows whether anything downstream will
composite the scene, and the scene does not. But the chain already passes that knowledge down: it
hands `1.0` whenever the scene is not the pass that resolves the seam.

## Decision

We will build every fullscreen scene's present pipeline with `PREMULTIPLIED_ALPHA_BLENDING` and let
the `occlude` value the renderer hands it be the only thing that differs between the two paths. On
the composite target (`Rgba16Float`), an alpha of exactly `1.0` gives a destination factor of exactly
`0.0`. So on the scratch path, and at the default `occlude` on the direct path, the blend reproduces
`REPLACE` to the bit. On the direct path with `occlude < 1` it resolves
`scene + bg * (1 - occlude)`, which is ADR-0085's formula with full coverage. A scene that writes
`occlude` into its alpha never presents with `REPLACE`.

## Consequences

### Positive

- **`occlude` means the same thing on every path and every scene that consumes it**, as its own
  parameter doc already claims.
- **No frame that renders correctly today changes.** Every shipped preset renders byte-identically,
  because each either takes the default `occlude` or binds `1.0` on the affected scenes.
- **The chain and the `Scene` trait are untouched.** The seam decision stays in `composite_into`,
  expressed as a value.

### Negative

- **Blending is enabled on four pipelines that did not blend**, which costs a little fill work on
  every fullscreen present. It is not measured here. It is the same state the other presenting scenes
  already pay for.
- **The target's alpha on the direct path changes from `occlude` to the composited value** (`1` over
  an opaque backdrop). Anything that read the old alpha, which was never meaningful, sees a different
  number.
- **The rule depends on authors.** A new fullscreen scene built with `REPLACE` reproduces the defect
  silently. The four-system behavioural test catches only the scenes it names.

## Alternatives considered

### Alternative A — The chain selects the scene's blend state

**Rejected because a pipeline's blend is fixed at construction.** Choosing it per frame means two
pipelines per scene, or a new method on the `Scene` trait (ADR-0002 keeps the trait thin). Either
duplicates a decision the chain already delivers through `occlude`, and the premultiplied blend is
exact on the path where the chain owns the seam.

### Alternative B — The field always renders into an offscreen and the chain composites it

**Rejected because it adds a target and a pass to the no-stage path**, the cheapest path the engine
has, to fix what is a one-line blend constant on four scenes.

### Alternative C — Document a `[post]` stage as a precondition of `occlude`

**Rejected because it leaves an engine-wide parameter inert on the simplest preset an author writes.**
It would also keep a parity test whose job is to pin both systems to the defect.
