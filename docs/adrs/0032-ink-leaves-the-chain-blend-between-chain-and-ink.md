# ADR-0032 — Ink leaves the `PostChain`: a terminal engine post-pass, with the transition blend between chain and ink

> **Status:** proposed
> **Date:** 2026-07-25
> **Related ADRs:** revises [0031](0031-post-stage-trait-instantiable-composite-chain.md)'s chain
> membership (does **not** revise its trait, its fixed-order rule, or its instantiability claim);
> preserves [0028](0028-final-stage-ink-tone-remap.md)'s ink-after-blend ordering and
> [0018](0018-engine-wide-scene-compositing.md)'s fixed composite; enables
> [0024](0024-cross-preset-transitions.md)
> **Related plan(s):** [0023](../plans/0023-cross-preset-transitions.md) (implements this)

## Context

Two accepted decisions collide the moment Plan 0023's transition blend is wired.

[ADR-0028](0028-final-stage-ink-tone-remap.md) fixed ink **strictly after** the transition blend:
"ink remaps the *blended* result, not each side." [ADR-0031](0031-post-stage-trait-instantiable-composite-chain.md)
then landed the composite as a `PostChain` holding `Trails`, `Kaleidoscope`, and `Ink` behind a
one-input `PostStage` trait — and stated a bound in the same breath: *a stage that needs a method no
other stage implements is a signal the stage does not belong in the chain, not a licence to widen it.*

Plan 0023's blend stage takes **two** inputs. It cannot implement a one-input `begin` without widening
the trait for one implementor, which is the erosion ADR-0031 exists to prevent. So the blend does not
belong in the chain. But ink is *in* the chain, and ADR-0028 requires ink to run after the blend —
which means the two per-side chains a dissolve runs cannot each end in their own ink pass.

The collision is not incidental. It reflects a category difference that was already visible and went
unnamed: **trails and kaleidoscope are per-preset look**, and both sides of a dissolve legitimately
have their own. **Ink is engine-wide** — ADR-0028's own framing is "the engine-wide black-on-white /
colored-duotone mode" — and there is exactly one finished frame for it to remap.

Plan 0030's landed API makes the resolution unusually cheap. `PostChain::begin` and `PostChain::resolve`
already take the final destination as a **caller-supplied `&wgpu::TextureView`**, not the swapchain
view specifically. Whatever runs after the chain is already a parameter.

## Decision

**Ink leaves the chain and becomes a terminal engine post-pass the renderer drives directly**, exactly
as `Background` is the pre-pass it drives directly. `PostChain` holds `Trails` and `Kaleidoscope` — the
per-preset look — and `STAGE_COUNT` becomes 2. **The transition blend also stays outside the chain**,
between the chain(s) and ink, and never enters the `PostStage` trait.

The composite reads: `Background` (pre-pass, owns the clear) → scene → `PostChain` (per preset) →
blend (only while a transition runs) → ink → surface → text/overlay.

The chain's destination argument carries the change: it is the blend's input while a transition runs,
ink's input when ink is active, and the surface otherwise. `PostChain`'s signatures do **not** change —
only the name of that argument, which stops being `surface_view` and becomes what it always was, the
next thing downstream.

During a transition, ink runs once on the blended frame with its params **crossfaded by the same `t`**
that drives the blend: `ink_amount`, `paper_*`, and `ink_*` interpolate from the outgoing preset's
values to the incoming's, so `t = 0` is exactly the outgoing look and `t = 1` exactly the incoming one,
with no snap at either endpoint.

`PostStage` itself is unchanged — same seven methods, same fixed-order array, same instantiability. The
membership shrinks; the seam does not move.

## Consequences

### Positive
- **ADR-0031's bound is honored rather than spent.** No trait method exists for one implementor; the
  two-input stage stays out of a one-input trait, which is what the bound was written to produce.
- **ADR-0028's ordering is preserved unchanged**, and for the first time it is structural: ink cannot
  be composed before the blend, because it is not in the thing that composes.
- **`PostChain`'s API needs no change.** Its destination is already a caller-supplied view, so the
  relocation is an argument rename plus a shorter array — not a re-architecture of what Plan 0030 just
  landed. Plan 0030's two-independent-chains proof carries over intact.
- **The chain becomes conceptually clean**: it holds exactly the stages a *preset* composes, so "each
  side of a dissolve gets its own chain" needs no caveat about which stage must be suppressed.
- **Dual-live stays wiring.** Two chains, two views, one blend, one ink.
- The relocation is behavior-preserving on its own — same pixel path, so goldens stay byte-identical
  and it can land and be verified before any transition code exists.

### Negative
- **It shrinks a decision that is one plan old.** ADR-0031 was accepted at Plan 0030's close and its
  membership claim is already revised here. The countable-seams cost it recorded is now paid for a
  two-stage chain, which is a thinner return on the same seam.
- **The renderer regains a stage it had just handed off.** Plan 0030's win was `draw_frame` no longer
  enumerating stages; it now drives blend and ink directly again — two calls, not a ladder, but the
  direction is backwards from that plan's.
- **Three things outside the chain, one inside-ish.** `Background`, blend, and ink are renderer-driven
  while trails and kaleidoscope are chain-driven, so "which stages are in the chain" becomes a fact to
  know rather than derive. The mitigation is that the split has a stateable rule — per-preset look in
  the chain, engine-wide passes outside it — and that rule is what a reader needs anyway.
- **Ink's param crossfade is new per-frame work and a new behavior** (~7 lerps per transition frame,
  and a visible one: a non-ink preset now fades *into* paper rather than cutting to it). It is only
  live while a transition runs, and it is the behavior the alternative would have snapped.
- A `PostChain` of two stages makes the routing tests' "all combinations" sweep smaller, so the
  routing contract gets marginally less exercise than the three-stage version it replaces.

### Neutral
- `Ink` keeps its file, shader, params, lazy-build discipline, and `PARAMS` const (which
  `preset::schema::GLOBAL_PARAMS` still reads); it reverts from a `PostStage` impl to inherent methods.
  No preset changes, no param renames, no golden re-bless.
- The C ABI is untouched, the `Scene` trait is untouched, and no dependency is added.

## Alternatives considered

### Alternative A — Put the blend in the chain and widen `PostStage` to two inputs
Make the blend stage index 2 in a four-stage chain (`trails -> kaleidoscope -> blend -> ink`), give
`PostStage::begin` a way to yield a second input, and teach `PostChain` to fold the stages *before* the
blend into a supplied view so two per-side prefixes feed one shared tail. Keeps one order in one place
and leaves ink where Plan 0030 put it. **Rejected** because it is precisely the case ADR-0031 named and
ruled out one plan ago: a method that only one of four implementors uses, with the other three stubbing
it. The "prefix + tail" machinery is also strictly more mechanism than a renderer that calls two passes
in order.

### Alternative B — Each side inks itself; blend two already-inked frames
Two complete chains, ink included, blended after both. The cheapest wiring by far — Plan 0030's
independence test already proves two full chains work untouched — and a preset with ink on dissolving
into one without it reads honestly. **Rejected** because it reverses ADR-0028's explicit ordering, and
that ordering has a reason: ink's remap is `mix(paper, ink, luminance)`, a **non-linear** function of
the frame. Blending two independently-remapped frames is not the remap of the blend, so mid-dissolve
would show a tone the operator never configured on either side. Reversing it would need a superseding
ADR against 0028, and the wrong-looking midpoint is the thing 0028 was protecting.

### Alternative C — Leave ink in the chain; restrict transitions to the frozen-snapshot path
Snapshot the outgoing *fully composited* (already-inked) frame and never run two live chains, so the
collision never arises. **Rejected** because it deletes Plan 0023's approved adaptive dual-live path
rather than implementing it, and dual-live is the half the interview specifically asked for. It also
only hides the problem: the snapshot is an inked frame blended against an inked live frame, which is
Alternative B's non-linearity with fewer options for fixing it later.

### Alternative D — Move ink out, but also move trails and kaleidoscope out (delete the chain)
If the renderer drives ink and the blend, let it drive all five passes and retire `PostChain`.
**Rejected** because it discards the property Plan 0023 needs most: two independently-instantiable
composites with their own feedback fields. The chain's value is per-preset multiplicity, and trails
(with its `PingPongField`) is the stage that has it. A renderer driving five passes would have to
duplicate the per-side ones by hand — ADR-0031's Alternative A, rediscovered.

## Notes

- The rule this ADR leaves behind, for the next stage that wants a home: **a pass that a preset
  composes belongs in the chain; a pass that applies to the finished frame belongs outside it.**
  `Background` (pre-pass, owns the clear), blend (two-input, transition-only), and ink (engine-wide
  remap) are all outside by that rule; trails and kaleidoscope are inside by it.
- ADR-0031's non-membership claims all survive: the `PostStage` trait shape, the compile-time-constant
  ordering, the pure `route` function, and the two-independent-instances property are unchanged. Only
  the array's contents shrink, so the `INK` position constant and the `ink_when_active_is_always_last`
  routing test retire with it — the ordering they asserted becomes structural instead.
