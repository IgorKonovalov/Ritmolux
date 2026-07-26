# ADR-0037 — An internal grid is a **resolution**, not a **shape**: aspect comes from the render target

> **Status:** proposed
> **Date:** 2026-07-26
> **Related plan(s):** [0035](../plans/0035-composite-aspect-and-grid-policy.md)
> **Corrects:** [ADR-0034](0034-internal-resolution-follows-the-target.md) (whose "an ultrawide keeps
> its proportions" claim survives quantization only approximately)
> **Supplements:** [ADR-0030](0030-scene-target-size-hot-path-hook.md) (the target-size hook),
> [ADR-0031](0031-post-stage-trait-instantiable-composite-chain.md) (the `PostStage` seam),
> [ADR-0012](0012-stateful-feedback-render-system.md) (the first internal grid)

## Context

The engine has three places where something renders into an offscreen whose pixel dimensions are
**not** the surface's, and then blits that offscreen to the surface with a fullscreen triangle: the
attractor's trail accumulation (Plan 0029), and the two `PostStage` internal grids (Plan 0033). Every
one of those blits is a **plain normalized stretch** — output UV `0..1` maps to input UV `0..1`, the
aspect ratios of neither side considered.

That is a deliberate and good property: it means an internal grid can be *any* size without the
present pass caring, which is what lets the size be quantized, capped, and changed at runtime. The
cost is a rule that is easy to state and has now been violated twice: **if the grid's aspect differs
from the target's, whatever was drawn at the grid's aspect comes out stretched.**

Both violations came from the same quantization policy. Rounding each axis up to a 256 px step is
what keeps a live window drag from reallocating textures hundreds of times, but it also means the
grid's aspect is only approximately the target's:

| target | grid | grid aspect vs target's | present stretches by |
|---|---|---|---|
| 1280x720 | 1280x768 | 1.667 vs 1.778 | **1.067x wide** |
| 1366x768 | 1536x768 | 2.000 vs 1.779 | 0.889x |
| 1280x800 | 1280x1024 | 1.250 vs 1.600 | **1.280x wide** |
| 1600x900 | 1792x1024 | 1.750 vs 1.778 | 1.016x |
| 1920x1200 | 1792x1080 | 1.659 vs 1.600 | 0.964x |
| 2560x1080 | 1920x1024 | 1.875 vs 2.370 | 1.264x |
| 3440x1440 | 1920x1024 | 1.875 vs 2.389 | 1.274x |
| 1920x1080 · 2048x1152 · 2560x1440 · 3840x2160 | exact | — | 1.000x |

**The first violation was found and fixed.** Plan 0029 Phase 5: the attractor's draw uniform took its
`aspect` from the accumulation grid's ratio, so the cloud drew 11 % too wide at 1920x1080 and 33 %
too wide at 512x384. The fix was to project at the `aspect` argument `render` already received and
discarded — the render target's — and `particles/mod.rs:36-38` now carries the note in writing:
*"the field's own aspect is not the projection's… a point at field NDC `x` lands at target NDC `x`."*

**The second violation is the same code path, one layer up.** Plan 0033 made the post stages follow
the target, and `PostChain::begin` derives `SceneTarget::aspect` from the **internal grid**
(`post.rs:445`). Every scene renders through that one value (`render/mod.rs:421`), so with `trails`
or `kaleido_*` active the whole frame is drawn at the grid's aspect and then stretched. Reproduced
with `shot` on a trails-bound copy of `rose_web`, measuring the lit-pixel extent with the stage on
versus off: **1.278x wider at 1280x800** (predicted 1.280) and **1.069x at 1280x720** (predicted
1.067). Because the attractor reads that same value, an active post stage also re-breaks the fix
Plan 0029 shipped. The kaleidoscope has the same defect independently: it folds in its *output*
space but aspect-corrects by its *input grid's* ratio (`kaleidoscope.rs:298`).

Neither was caught because the sizes the project is developed and tested at — 1920x1080 and the
user's 2048x1152 — both come back from the policy exactly 16:9.

## Decision

We will treat an internal grid as a **resolution and nothing else**. Concretely: **any pass that
computes geometry, angles, or distances destined for the screen takes its aspect from the render
target, never from the grid it happens to be rasterizing into.** The grid's aspect is an
implementation detail of how many texels the work is sampled at, and it must cancel out of the
picture.

For the composite that means `SceneTarget::aspect` is derived from `surface`, and the kaleidoscope's
fold corrects by the aspect of the destination it is folding into. `Scene::set_target_size` keeps
receiving the **grid** — that one genuinely is a resolution, and a scene sizing an internal field
wants the texel count, not the shape.

The arithmetic works out because both stretches compose: a scene told the target's aspect draws
pre-squashed into a grid of a different shape, and the present's stretch is exactly the inverse. A
unit circle at aspect 1.6 rendered into a 1280x1024 grid occupies 400x512 texels, and the blit to a
1280x800 surface returns it to 400x400 — round. This is the same cancellation `particles/mod.rs`
already documents; ADR-0037 generalizes it from one scene to the rule.

We also fold the quantization policy into **one** implementation parameterized by cap and step.
`post.rs::internal_grid_size` is currently a line-for-line copy of
`particles/mod.rs::trail_grid_size`, which is how the two ended up with different aspect behavior in
the first place, and is the opposite of what ADR-0034 claimed the shared policy would buy.

## Consequences

### Positive
- Turning `trails` or `kaleido_*` on stops changing the **shape** of the picture. That was silently
  true of every non-16:9 window and of common 16:9 windows whose axes do not divide by 256, and it
  is a worse defect than the softness Plan 0033 set out to fix.
- Plan 0029 Phase 5's attractor fix stops being conditional on no post stage being active.
- The quantization policy is free to be as coarse as it likes. Once aspect is not carried by the
  grid, the 256 px step — and any future cap, per-stage fraction, or dynamic downscale — is a pure
  cost/quality knob with no geometric side effect. That is the real prize.
- The rule is checkable by inspection: any `aspect` computed from a grid size is a bug.

### Negative
- **A stage's grid is now genuinely non-uniform in sampling density.** At 1280x800 the composite
  rasterizes 1280x1024 texels to show 1280x800, so the short axis is supersampled ~1.28x and the
  long axis is not. That is wasted fill — bounded (worst case is one step, so under 256 px per axis)
  and strictly better than the alternative, but it is real work spent on texels that are averaged
  away.
- `PostStage::resolve` needs the destination size it is folding into, which `begin` already receives
  and `resolve` does not. That is a second signature change to a seam ADR-0031 introduced and
  Plan 0033 already touched once. It stays crate-internal and reaches no preset and no C-ABI caller.
- The fix is invisible at 1920x1080 and 2048x1152, so it cannot be confirmed on the development
  display. Its test has to pick a size where the grid and the target disagree, deliberately.

### Neutral
- No golden baseline moves: no fixture in `core/tests/fixtures/` binds `trails` or `kaleido_*`, so
  the changed path is not exercised by any current capture. That is *also* why the defect shipped —
  addressed as coverage in Plan 0035, not by this decision.
- Headless captures stay a pure function of `(preset, input, frame-count, size)` (NFR §6): nothing
  here reads a clock, and the aspect becomes a function of one input that was already in the tuple.

## Alternatives considered

### Alternative A — Make the grid's aspect match the target's instead of correcting at projection
Quantize the binding axis to the 256 px step and derive the other axis *exactly* from the target's
aspect, so grid aspect equals target aspect and the stretch is the identity. Superficially the
tighter fix, and it needs no signature change.

Rejected because it destroys what the quantization is for. The derived axis then changes whenever the
**aspect** changes, and a horizontal-only window drag changes the aspect continuously — so the grid
would be reallocated on nearly every `Resized` event, which is precisely the texture-pair
reallocation (and, for trails, the cleared accumulation) that the 256 px step exists to prevent. It
also leaves the rule unstated, so the next stage that picks a grid re-derives the trap.

### Alternative B — An aspect-preserving present: letterbox the grid into the target
Fit rather than fill, so a mismatched grid shows bars instead of stretching. Honest, and it makes the
mismatch visible rather than silent.

Rejected on product grounds: this is a fullscreen music visualizer, and black bars appearing because
of an internal resolution policy is a worse user-visible outcome than either the stretch or the fix.
It would also have to be threaded through three present passes that currently share a trivial
fullscreen blit.

### Alternative C — Snap the grid to the target's aspect only when the mismatch exceeds a threshold
A hybrid: tolerate small aspect error, re-derive when it is large. Rejected because it makes the
picture's shape a discontinuous function of window size — the exact regression Plan 0029's close
notes describe ("the shape changed discontinuously as the window crossed the cap") — and because the
threshold is a number nobody can defend.

### Alternative D — Leave it; document the stretch as a known limit of composing a post stage
What Plan 0033's Phase 6 commit effectively chose, calling it an "honest limit" and noting the
attractor policy shares the property. Rejected on measurement: 28 % at 1280x800 and 27 % on an
ultrawide is not a limit, it is a defect, and the attractor does **not** share it — Plan 0029 fixed
the outcome even though the grid keeps the property.

## Notes

- The measurements in Context are from `shot` captures at 1280x800 and 1280x720 against a
  trails-bound copy of `presets/rose_web.toml`, comparing the lit-pixel extent with the stage active
  and inactive. Predicted and measured agree to within 0.3 %.
- ADR-0034's Outcome section records this as the consequence it did not anticipate. Its decision —
  the stages follow the target, quantized and capped — is unchanged and correct; only the claim that
  a single scale factor preserves proportions needs the qualifier that quantization then coarsens
  them.
- The invariant is stated for `core/` in `CLAUDE.md`'s pitfalls, because it is the kind of rule a
  reviewer has to hold rather than a lint can catch.
