# ADR-0047 — The kaleidoscope folds a disc: radius clamped to the inscribed extent with a radial falloff, and a bindable fold centre

> **Status:** proposed (decision to be confirmed against rendered samples — see Notes)
> **Date:** 2026-07-30
> **Related plan(s):** 0045-linear-light-and-bloom, Phase 1
> **Resolves:** design-backlog 0010 and 0011. **Supplements:** 0018 (the stage), 0031 (the chain).

## Context

The kaleidoscope fold is a polar operation on a rectangular source: each output pixel keeps
its radius and takes a folded angle, then resamples. Any pixel whose radius exceeds the
source's extent in the folded direction samples outside `[0,1]`, and the sampler is
`ClampToEdge` — the border texel smears radially into hard streaks and chevron debris
(backlog 0010, user-reported three times). At 16:9 the corner radius is ~1.02 against a
0.889 short-extent, so the debris is corner-local; in a portrait window most of the frame is
out of range and the artifact becomes stripes across the whole picture. The defect has
already changed what ships: `swarm_dense` pins `kaleido_order = "1"` specifically to dodge
it. Separately (backlog 0011), the fold axis is hardcoded to screen centre, so `pan_*` and
`kaleido_*` are mutually exclusive — panning slides the rosette off its own axis.

A prerequisite chain runs through this: backlog 0005 says the bloom stage should be built
against the settled fold answer, and R1 (ADR-0046) carries bloom — so this decision gates R1.

One verification trap is on record from Plan 0035's close review: the pinned fixture
`composite_kaleido.png` stays **green** under the inscribed-disc fix at 94 % of its drift
budget, so the guard will not announce the fix — the baseline must be re-blessed by hand and
a direct guard added on the clamped-pixel statistic.

## Decision

We will fold a **disc**: the sample radius is clamped to the largest disc the source
contains (`r_max` = the inscribed half-extent along the short axis), and beyond `r_max` the
result takes a **radial falloff to the background** so the region outside the disc reads as
a deliberate vignette rather than either smeared texels (today) or a hard flat ring (plain
clamp). The fold centre becomes the bindable pair `kaleido_center_x` / `kaleido_center_y`
(default screen centre), answering backlog 0011 in its minimal form; an off-centre fold
shrinks the inscribed disc on one side, which the falloff absorbs by construction.

## Consequences

### Positive
- The debris class disappears at every aspect, including portrait, where today it is
  catastrophic. `swarm_dense` and the other eight fold-binding presets get the stage back.
- The vignette edge is a *designed* boundary — it composes with `bg_*` (the falloff lands on
  the backdrop) instead of fighting it.
- A moving fold centre plus `pan_*` stop being mutually exclusive.

### Negative
- Content outside the inscribed disc is discarded — the fold shows less of the source frame
  than the broken version pretended to. On field scenes the wrap alternative would have
  tiled *something* there instead of a falloff; we accept the loss for correctness on
  centred figures, which is what the fold is overwhelmingly used on.
- Every fold-bearing capture moves; `composite_kaleido.png` must be re-blessed **by hand**
  with the trap above in mind, and the new clamped-pixel guard is owed in the same change.

## Alternatives considered

### Alternative A — wrap or mirror the address mode
One line, and it fills the corners with content — but *unrelated* content: the frame's
opposite edge tiled into the corners. Plausible on a borderless field, visibly wrong on a
centred figure, which is the fold's main use. Rejected for figure scenes; not offered as a
per-preset mode because two address modes double the stage's pipelines against the
documented WARP pipeline-count sensitivity.

### Alternative B — plain clamp to the inscribed disc, no falloff
Defined and cheap, but the disc edge is a hard circle and the corners go flat — a letterbox
in disc form, which ADR-0037 already rejected in spirit for the present path ("black bars
are worse than the stretch for a fullscreen visualizer"). Rejected for the hard edge; it is
however the A/B control in the sample set below.

### Alternative C — status quo (`ClampToEdge`)
The defect. Named only because it shipped for months and the fixture pins it.

### Alternative D — the fold axis follows the `ViewTransform` instead of gaining params
Couples a `PostStage` to a scene-side transform the chain deliberately does not see, and
gives authors less (no independent fold-centre motion). The param pair is the minimal form.

## Notes

**Confirmation protocol (design-by-concrete-examples).** The user picks visual directions
from rendered side-by-side samples, not option lists. Plan 0045 Phase 1 renders the same two
scenes (a centred figure and a border-filling field) under B (hard disc), D-mode wrap, and
the chosen falloff-disc, at 16:9 **and** portrait, and the user confirms or flips this
decision from the captures before it is accepted. Any fix must be evaluated at a non-16:9
aspect — the 16:9 dev configuration is what hid the defect (same lesson as ADR-0037).
