# ADR-0119 — The video echo blends toward its copy rather than adding it

> **Status:** proposed
> **Date:** 2026-08-19
> **Related plan(s):** [0109](../plans/0109-the-milkdrop-import-gets-its-geometry-back.md) Phases 3 and 7

## Context

[Plan 0109](../plans/0109-the-milkdrop-import-gets-its-geometry-back.md) Phase 3 built the video-echo
stage the engine had never had: a second sampled copy of the finished frame, zoomed about the centre
and flipped per `echo_orient`, composited at `echo_alpha`. MilkDrop's own format stores those three
values and 2.4 % of the corpus sets a non-zero alpha, but **how** the second copy meets the first is
not in the format — it is in the reference renderer, which this project reads only by looking at it.

Phase 3 had to choose, and chose to **add**. Three things pointed that way at the time. Plan 0109's
own acceptance property said a flip-x at `alpha = 1` must render left-right symmetric, which is true
of a sum and false of a lerp. [Plan 0108](../plans/done/0108-the-milkdrop-import-gets-its-tone-back.md)'s
look gate described the reference's *Songflower (Moss Posy)* at `alpha = 1.000` as showing **two
families of bars** where this engine showed one, and two families reads as both copies. And the field
is premultiplied linear light whose seam is additive by construction ([ADR-0056](0056-additive-scenes-emit-premultiplied-alpha.md)).
The shader comment recorded the choice, named the knob, and said Phase 5 would settle it.

**Phase 5 settled it against the choice, and falsified the observation the choice rested on.** Two
conversions of *Songflower*, identical but for `fVideoEchoAlpha`, rendered at the same hop of the
same signal at the same size:

| | ground | lattice |
|---|---|---|
| `fVideoEchoAlpha = 0.000` | dark | crisp two-axis basket weave, strong bars |
| `fVideoEchoAlpha = 1.000` (as authored) | lifted | same weave, washed pale, bars barely readable |

Two findings, and the second is the load-bearing one. The echo as built **costs contrast rather than
adding structure** — the preset's per-frame code drives `echo_zoom` to roughly 1.75–2.0, so what is
summed is a large soft magnified duplicate. And **both families of bars are present with the echo
off**, so Plan 0108's "only one family survives" is not reproducible on today's engine. *Songflower*
sets `sx = sy = zoom = 1` with no negative scale anywhere, so Plan 0109 Phase 1 did not restore the
second family either; the lattice is the preset's own warp and always was.

That removes two of the three supports under `add`. The third — ADR-0056 — is about the *seam between
producers*, where two independent sources of light meet. An echo is not a second producer: it is the
same light, sampled twice.

## Decision

**The video echo blends toward its copy: `mix(base, echo, alpha)`, not `base + echo * alpha`.** The
composite of a frame with a transformed copy of itself never exceeds the dynamic range of the frame,
at any alpha, because a convex combination cannot. `alpha = 0` remains the exact identity it already
was, and `alpha = 1` is now the transformed copy alone rather than a doubling.

The acceptance property changes with it, and becomes **stronger rather than weaker**. Phase 3 asserted
that flip-x at `alpha = 1` renders left-right symmetric — a consequence so weak that a uniform grey
frame satisfies it. Under this decision the stage can be pinned exactly: because the echo samples the
same field the pass is already reading and **writes nothing back**, the field evolves identically in
both arms, so at `alpha = 1`, `echo_zoom = 1`, `echo_orient = 1` the rendered frame **is** the
horizontal mirror of the same preset at `alpha = 0`, frame for frame. That is the transform itself
under test, not a symptom of it.

## Consequences

### Positive

- **The echo cannot wash a preset out.** The one property a display-time composite of an image with
  itself must have — bounded by its own input — now holds by construction rather than by a preset
  happening to author a small alpha.
- **The acceptance test pins the transform.** Zoom, flip and alpha are all checkable against a mirror
  of the control instead of against a symmetry statistic that many wrong pictures also satisfy.
- **`alpha = 0` is still bit-identical**, so the 97.6 % of the corpus that sets no echo is untouched,
  and `core/tests/golden/warp_mesh_milk.png` does not move.

### Negative

- **This is still inference from a rendered picture, not from the reference's source.** A lerp is the
  standard alpha blend and matches what the gate saw, but nobody here has read `foo_vis_milk2`'s
  compositor. If a later gate finds a preset the lerp gets wrong, this ADR is the thing to supersede,
  and the evidence above is what it has to beat.
- **`alpha = 1` now hides the base frame entirely.** That is the correct reading of a lerp and it is
  a real behavioural cliff: a preset animating alpha through 1.0 crosses from "mostly itself" to
  "entirely its own echo". The reference presumably has the same cliff; we have not watched one do it.
- **Phase 3's shipped behaviour changes within one plan.** Any judgement recorded against the additive
  build — including Plan 0109's own Phase 5 verdict on *Songflower* — has to be re-taken.

### Neutral

- The stage stays one extra sample in the present pass, with no second target and no extra pass. The
  blend is a `mix` where it was an add; the cost is identical.

## Alternatives considered

### Alternative A — keep the additive blend, and rescale `alpha`

Keep `base + echo * alpha` and reinterpret `alpha` so that the authored `1.000` maps to something
tamer. Rejected because it does not fix the shape of the defect, only its magnitude: a sum is still
unbounded above, so some other preset with a larger authored alpha or a brighter field washes out
instead, and the remap would be a fudge factor with no reference behind it. It also leaves `alpha`
meaning something different here than in the format it was read from.

### Alternative B — average the two copies, `(base + echo * alpha) / (1 + alpha)`

Bounded, and preserves both copies at every alpha. Rejected because it makes `alpha = 0` a *scaling*
of the base rather than the identity — `1/(1+0)` is 1, so the identity survives, but any non-zero
alpha dims the base frame, which no reading of the reference supports. It is also a blend nothing
else in this engine speaks.

### Alternative C — revert the echo entirely and re-file it as unbuilt

Back Phase 3 out and return design-backlog 0115 to the queue. Rejected because the blend is one
expression: the converter roster, the `FrameOutputs` fields, `COMPOSITE_PARAMS`, the orientation
quantizer and the acceptance fixture are all correct and were all verified independently of it.
Throwing that away over a blend-mode choice costs more than making the choice again.

## Notes

The controlled A/B lives at `core/tests/fixtures/scratch-0109/` plus the gate's scratch renders; it is
reproducible with two conversions of `Aderrasi - Songflower (Moss Posy).milk` differing only in
`fVideoEchoAlpha`, captured with `shot --signal dynamic:110 --frame-at 180`.

The gate that produced this ADR also retracted a second Plan 0108 observation — *Cosmic Dust 2*'s
"hue magenta where the reference is green", which is three independent per-channel LFOs on `time` and
so measures only the phase difference between two renderers started at different moments. Recorded
here because both retractions came from the same sitting and neither is about this decision.
