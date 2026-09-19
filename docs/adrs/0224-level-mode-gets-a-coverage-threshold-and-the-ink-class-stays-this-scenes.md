# ADR-0224 — Level mode gets a coverage threshold, and the ink class stays this scene's

> **Status:** proposed
> **Date:** 2026-09-19
> **Related plan(s):** [0201](../plans/0201-the-warp-surface-stops-lying.md)
> **Extends:** [0197](0197-the-contour-can-be-an-ink-and-the-warp-field-can-be-coloured-by-its-level.md)
> (whose `Outcome` routed the question here); leaves
> [0138](0138-limited-ink-is-a-supported-palette-class-defined-at-the-draw-seam.md) where it is

## Context

[ADR-0197](0197-the-contour-can-be-an-ink-and-the-warp-field-can-be-coloured-by-its-level.md) gave
`warp_mesh` a second colour path: `color_source = "1"` deposits uncoloured light and the present pass
colours the field by its own accumulated level. As *structure* it does what was asked — the bands are
the feedback loop's own decay contours, and nothing else in this engine draws one.

What it does not produce is a limited-ink frame, and the reason is the last line of the pass rather
than the coordinate. The present writes `ink * coverage`: `ink` is one of the palette's own values
and coverage is the field's alpha, a continuum, so every pixel short of full coverage is a value the
palette never named. Measured on the shipped `presets/warp_ladder.toml` at 640x360 loud, a two-ink
palette renders **851 exact colours** with `palette_steps = "0"`. Quantizing the palette coordinate
quantizes the level that coverage is computed from as well and brings the same frame to **60** — which
is why the preset ships at twelve bands and states the count in its header instead of claiming a class
it does not have. Sixty is not two, and no switch in the engine gets there: `palette_contour` draws at
a band edge, so with no bands it is inert (851 colours with the key at full strength and 851 without,
measured both ways).

Plan 0184 Phase 4's look gate routed this here by its own stop condition: the fringe reads as shading
rather than as the ladder dissolving.

Two further facts shape the decision rather than the mechanism. **A soft outer edge is a look, not
only a defect** — `warp_ladder`'s header argues the fade reads as the ladder running out of ink at
the edge of the sheet, so two shipped worlds would want opposite defaults. And **this is a
present-time composite, while [ADR-0138](0138-limited-ink-is-a-supported-palette-class-defined-at-the-draw-seam.md)
defines the limited-ink guarantee at the draw seam**: the same residue exists in principle for every
scene whose output is premultiplied light, and a general repair would be that boundary moving.

## Decision

We will give level mode a **coverage threshold** parameter: coverage at or above it resolves to the
ink, below it to the paper, so the frame holds only the palette's values and the background. It is
**off by default**, which preserves every shipped look including `warp_ladder`'s argued fade, and the
edge it produces is **hard** — no smoothing, no derivative-based antialiasing — because an
antialiased ink boundary reintroduces exactly the intermediate values the threshold exists to remove.
The parameter belongs to `warp_mesh`. ADR-0138's draw-seam definition of limited ink does not move,
and this scene does not claim to satisfy it — it claims a frame whose colours all come from the
palette, which is the property a print needs and which this threshold delivers.

## Consequences

### Positive
- A print becomes reachable: a two-ink palette with the threshold on renders two inks and the paper,
  which no combination of today's switches reaches.
- Nothing shipped moves. Off by default, and `warp_ladder`'s fade is left as its header argues for.
- The question stops being open in three places at once — ADR-0197's `Outcome`, Plan 0184's stop
  condition and the backlog entry all point at one decision.

### Negative
- **The edge aliases, and will crawl when the field moves.** That is the honest cost of a hard alpha
  cutoff on a silhouette rather than a hairline, and it is the same trade the hard contour style
  already makes on a much thinner figure. An author who does not want it leaves the threshold off.
- **A second way to get a limited palette exists in this engine**, and it is not ADR-0138's. Anyone
  reading both will ask which one a scene satisfies; the answer is written here and in
  `docs/preset-palettes.md`, and it is a distinction a reader has to hold.
- **It is a per-scene answer to a class question.** If a second premultiplied-light scene asks for the
  same thing, this parameter is precedent for copying rather than for generalizing, and the right
  response then is to move ADR-0138's boundary rather than to add a third copy.

### Neutral
- The threshold interacts with `palette_steps`: quantized bands plus a threshold is a legal and
  probably common combination, and neither cancels the other.

## Alternatives considered

### Alternative A — Repair it at the draw seam, for every scene
Make premultiplied-light output satisfy ADR-0138's guarantee generally. It is the more complete
answer and it is a much larger decision: it touches every scene whose output is light rather than
colour, and it would be decided now on the evidence of exactly one world that wanted it. Rejected as
premature — ADR-0138's boundary moves when a second scene asks, and this ADR says so explicitly so
that the precedent is bounded.

### Alternative B — Antialias the threshold edge
Resolve the cutoff with a screen-space derivative so the silhouette is smooth. Rejected because the
smoothing produces intermediate colours at the boundary, which is the residue being removed; an
ink-class frame with a soft edge is the frame we already have.

### Alternative C — Do nothing, and record why
The mechanism does what ADR-0197 decided, the world that wanted it ships, and the residue is a class
the frame does not join rather than a picture that is wrong — backlog 0251's own summary. Rejected
because the ask is real and has arrived twice (backlog 0146, then Plan 0184's look gate), and the
parameter is small, default-off and reversible.

## Notes

Raised as backlog 0251, 2026-09-17, from Plan 0184 Phase 4's look gate. The measurements in the
Context are that entry's, taken on the shipped preset at 640x360.
