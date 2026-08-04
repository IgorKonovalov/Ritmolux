# ADR-0070 — A feedback pass addresses its own target in framebuffer space

> **Status:** proposed
> **Date:** 2026-08-04
> **Related plan(s):** [0059](../plans/0059-lorenz-finds-its-plane.md) (Phase 1b)

## Context

`core/src/render/gpu.rs` offers three fullscreen-triangle vertex preludes. Two are
uncontroversial: `FULLSCREEN_VS_NDC` hands the fragment stage clip-space coordinates, and
`FULLSCREEN_VS_UV_FLIPPED` hands it texture coordinates with Y flipped, "clip space is Y-up, a
render target's texture space is Y-down, so a pass sampling what another pass rendered needs
this". The third, `FULLSCREEN_VS_UV`, hands over unflipped coordinates and justifies itself like
this (`gpu.rs:275-278`):

> For a ping-pong chain where every pass uses this convention, so the flips would cancel: reading
> and writing agree, and the field never appears inverted to itself.

**That premise is false, and not by a little — there is no chain arrangement in which those flips
cancel.** WebGPU clip space is Y-up while `@builtin(position)` and texture space are Y-down. A
fullscreen fragment at clip `p.y` writes framebuffer row `(1 - (p.y+1)/2) * H`; the unflipped
prelude gives it `uv.y = (p.y+1)/2`, which samples row `((p.y+1)/2) * H`. Those are opposite rows
for every `p.y` but the centre. A pass that samples the target it writes reads the vertically
mirrored texel — every pass, every frame, regardless of what its neighbours do. "Every pass uses
this convention" cannot repair a defect that is already complete within one pass.

**The attractor is where that lands.** `particles/mod.rs:780,786` compile the decay and present
passes with the unflipped prelude, and the accumulation target is *also* written by the draw pass
in clip space rather than through a fullscreen uv pass. So the decay pass re-reads its own trail
history mirrored, and the steady state of the feedback loop is `figure ∪ mirror(figure)`.

Measured rather than argued, 2026-08-04:

- With `pan_y = 0.35`, `attractor_lorenz` renders **two mirror copies about the screen
  centre-line** instead of one shifted figure. That is the diagnostic: a translation cannot
  produce a symmetric pair.
- Switching both call sites to `FULLSCREEN_VS_UV_FLIPPED` renders a **single** Lorenz butterfly —
  wings up and out, notch at the top centre, converging to a tail — and `pan_y` then moves one
  figure.
- That orientation is the correct one, settled off the particle buffer read back from the GPU
  rather than by eye: `|x| > 14` (the wing tips) has mean `z = 36.9`, `|x| < 2` (the crossover)
  has mean `z = 19.8`. High `z` sits at the wing tips, so with `+z` up the wings splay upward.
  A stdlib projection of the same 50 000 positions agrees; the shipped render is its exact
  vertical mirror (widest row 0.07 down the figure versus 0.95).

**Why nothing saw it.** The doubling makes the output mirror-symmetric, which makes the *second*
flip — the present pass, also unflipped — unobservable. The bug conceals its own symptom, so it
survives every gate this project owns: goldens were blessed with it, `sanity`'s coverage and
`quadrant_spread` are if anything flattered by a symmetric figure, and the reactivity gates
([ADR-0062](0062-clamp-occupancy-is-the-saturation-instrument.md),
[ADR-0067](0067-coverage-measures-the-scene-not-the-backdrop.md)) never look at shape. All six
`attractor_*.toml` presets bind a slow `pan_y` drift (`cos(time * 0.028) * 0.06` and friends)
which has been rendering as a symmetric breathe rather than as a drift, for the life of the scene.

**Reaction-diffusion uses the same prelude and is not affected**, for a reason worth recording
because it is the whole distinction: RD's sim reads the previous field with
`textureLoad(field, vec2<i32>(in.pos.xy))` (`reaction_diffusion.rs:148`) — framebuffer position,
never the prelude's `uv` — so it loads exactly the texel it writes. Verified empirically at both
the shipped even sub-step count (12/frame) and a forced-odd one (11/frame, `FIXED_STEP` temporarily
1/660): consecutive frames differ by mean `|dL|` 1.5 against a 53 mirror control at both parities.
RD's `uv` feeds only its injection stamp, its init blobs, and its present sample window, which
agree with each other.

So the tree contains one prelude whose stated contract is unachievable, one scene that is broken by
it, and one scene that escapes on a property of its own shader body.

## Decision

**We will address a sampled target the same way we address the target we write.** A fullscreen
pass that samples what another pass rendered uses `FULLSCREEN_VS_UV_FLIPPED`, whose `uv`
round-trips to the texel the fragment writes; a pass may instead address by `@builtin(position)`
through `textureLoad`, which is exact and is what RD's sim already does. **`FULLSCREEN_VS_UV` is
retired** — its documented precondition cannot hold, and after Plan 0059 Phase 1b it has no
callers.

Retiring it moves RD's three passes together, not just its present. RD's `uv`-addressed terms —
the init blob centres, the sim's injection stamp, the present's zoom/pan sample window — must keep
agreeing with one another, so they flip as a set or not at all. The user chose one convention
engine-wide over leaving RD on a per-scene proof (2026-08-04).

The mitigation is a gate, because the defect's signature is exactly what no existing gate reads:
**a non-zero `pan_y` must move the figure's lit centroid off the frame's vertical centre.** Under
the defect that is impossible — the mirror doubling pins the centroid to the centre line whatever
`pan_y` says — so the assertion is non-vacuous, and it names the defect rather than a downstream
symptom. The margin is measured in the plan, not asserted here.

## Consequences

### Positive

- The attractor renders the figure it computes. Lorenz becomes the butterfly, which is the half of
  [ADR-0068](0068-the-projection-basis-is-a-per-family-property.md) that a basis alone could not
  buy: Phase 1 landed the correct plane and the picture was still an X.
- One rule, statable in a sentence, replaces a per-scene analysis of whether a chain's flips
  happen to cancel. The rule is checkable by reading a single pass in isolation.
- Every attractor preset's `pan_*` becomes a view control rather than a symmetric pulse — six
  presets get back a lever they were authored as if they had.

### Negative

- **Two of thirteen golden baselines move**: `attractor.png` and `reaction_diffusion.png`. The four
  `composite_*` fixtures run `parametric_curve` and are untouched, as is everything else.
- **All six attractor presets were authored against a doubled figure.** `fade`, `size` and the
  brightness balance were tuned to twice the geometry at half the meaning; this is not a re-aim but
  close to a first authoring. It lands in Plan 0059 Phase 4's content pass, whose budget grows —
  and that plan's own reason for being one plan is that the content pass must run *once*, after
  the figure stops changing shape.
- **RD's `pan_y` reverses direction**, so the four RD presets need a look check. Bought
  deliberately in exchange for a single convention.
- **The surviving prelude keeps a now-misleading name** — `FULLSCREEN_VS_UV_FLIPPED` will be the
  only uv prelude, so "flipped" names a contrast with something that no longer exists. Left as a
  separate rename rather than folded in, so the diff that changes behaviour is not also the diff
  that renames a symbol. Named here so it is a choice and not an oversight.

### Neutral

- No C ABI change, no new dependency, no hot-path cost — a prelude is a compile-time string, and
  the fix changes which one is concatenated.
- The attractor's present flip and its decay flip are one change but two independent corrections;
  either alone leaves a mirrored picture, which is why the pair has to land together.

## Alternatives considered

### Alternative A — Fix the attractor's two call sites, correct the unflipped prelude's doc, keep it

The minimal change: two symbols, plus replacing the false rationale with the real precondition.
Rejected because the corrected precondition is not a property of the prelude at all — it would read
"safe for a pass that never samples the target it writes through this `uv`", which is a property of
each *caller's shader body*. RD satisfies it by using `textureLoad`; nothing enforces that, and the
next scene to reach for the prelude inherits a trap whose documentation now describes a condition
it must verify by reading its own fragment shader. Retiring the symbol is what removes the trap.

### Alternative B — Normalize by making the draw pass write Y-down

The mirror is a disagreement between two passes, so it can be closed from either side. Rejected
because the draw pass writes clip space for a reason: it projects geometry, and every scene
parameter that means "up" — `pan_y`, and the projection basis ADR-0068 has just finished naming —
is expressed in that space. Inverting the space that carries the meaning, to accommodate the
sampling convention, puts the compensation at maximum distance from the cause.

### Alternative C — Correct the documentation and rely on review, with no gate

Rejected on this project's own record. ADR-0062, ADR-0063 and ADR-0067 each close a defect that
survived precisely because the rule protecting it lived in prose: an unsurfaced coupling plus a
doc rule is the combination that keeps failing here. Nothing in the suite can currently see a
mirror-symmetric figure, and "nothing could have caught it" is an argument for a gate, not for
better prose.

### Alternative D — Leave reaction-diffusion alone

RD is provably safe today, so the smallest correct change excludes it, moves one baseline instead
of two, and leaves four presets' `pan_y` untouched. Rejected by the user (2026-08-04) in favour of
one convention engine-wide: a second convention surviving on a per-scene proof is the thing this
ADR exists to remove, and the proof is a `textureLoad` call one tidy-up away from becoming a
`textureSampleLevel`. Accepted cost: the RD baseline and the `pan_y` direction.

## Notes

The evidence is a set of captures rather than a benchmark: `new_trail.png` (the shipped X),
`fix_trail.png` (the butterfly), `pan_trail.png` / `fix_pan.png` (the two-copies diagnostic and its
resolution), and a stdlib rasterization of the read-back particle buffer as an external reference.
The `pan_y` diagnostic is the reusable part — offset a figure and count how many copies come back.

This is the third time a defect here has hidden inside a coincidence of the development
configuration ([ADR-0037](0037-internal-grid-is-a-resolution-not-a-shape.md)'s 16:9 grid, WARP's
bind-group aliasing in [ADR-0058](0058-bind-group-layout-collisions-carry-evidence.md), and now a
symmetry the defect itself creates). The generalization in the architect review checklist —
*find the configuration where two sources disagree and ask whether anything probes it* — would have
caught this one at `pan_y != 0`, which is the gate this ADR adopts.
