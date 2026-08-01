# ADR-0059 — Every line scene honours the palette and colours along its own generator's axis

> **Status:** proposed
> **Date:** 2026-08-01
> **Related plan(s):** [0054](../plans/0054-the-line-scenes-catch-up.md)
> **Supplements:** [0021](0021-shared-palette-system.md) (the palette surface this extends to four
> more scenes), [0036](0036-preset-reachable-spectrum.md) (the per-element series channel and the
> precedent for colouring along an axis), [0007](0007-line-geometry-generators.md) (the generators
> gaining the axis).

## Context

`preset-author` filed [design-backlog 0026](../design-backlog.md) against `lsystem`: it has no
per-segment colour, and "the asymmetry with `spectrum` looks unintentional". The user's framing on
Arrowhead was that a single flat hue makes a branching figure read as wire rather than as growth.

Reading the code, the gap is wider than the entry states. `spectrum`'s module docs say it is **"the
first line scene to honor the palette; the others still colour from the built-in cosine"** — so
`parametric_curve`, `lsystem` and `star_pattern` do not reach `[palette]`, `[palette_b]`,
`palette_mix`, `hue_spread` or `saturation` at all. `lsystem::PARAMS` has thirteen entries and
`hue` is the only colour lever among them. Four scenes share one line renderer and one colour
model, and exactly one of them has the colour surface every other scene family got in ADR-0021.

The mechanism to fix it already exists and is narrow. `Scene::set_param_series`
([ADR-0036](0036-preset-reachable-spectrum.md)) carries one evaluation per element for a binding
naming `index`, and `spectrum` declares six per-element params (`base`, `scale`, `curve`,
`thickness`, `brightness`, `hue`) against a documented rule: a param describing the *whole figure*
degrades to its `index = 0` value. So the question is not "how do we get colour to vary" — it is
**what a line scene's element axis is**, which differs per generator and is a real decision:

- `spectrum` has an obvious axis: element `i` is frequency band `i`. That is what makes colouring
  along it turn a readout into a look.
- `parametric_curve` has a natural axis too: position along the traced path, `0..1`.
- `lsystem` has **two** candidate axes that mean different things — position along the turtle's
  path, or the branch's **generation depth**. Depth is what makes a fern read as growth; path
  position is what makes it read as a drawn stroke.
- `star_pattern` is rotationally symmetric about the frame centre, so a path-position axis paints
  an arbitrary seam across a figure that has no beginning.

## Decision

**Every line scene honours `[palette]`, `[palette_b]`, `palette_mix`, `hue_spread` and
`saturation`, sampled on the CPU exactly as `spectrum` does, and each generator declares the axis
its `hue_spread` walks.** The axis is a property of the generator, named in its module docs and in
`presets/README.md`, not a parameter the author picks:

- `parametric_curve` — normalized position along the traced path.
- `lsystem` — **generation depth**, normalized over `visible_depth`.
- `star_pattern` — normalized radius from the rosette centre.
- `spectrum` — band index (unchanged; this is the existing behaviour the others catch up to).

`hue_spread = 0` collapses every one of them to today's flat `hue`, so the change is a strict
superset and no shipped preset moves.

`lsystem` takes depth rather than path position because depth is the axis that carries the scene's
own structure — the L-system's whole subject is recursive generation, and colouring by it makes an
older branch read as older. Path position is available to `parametric_curve` for anyone who wants
the drawn-stroke reading.

## Consequences

### Positive

- **The colour surface stops being one scene's privilege.** Four line scenes gain `[palette]`,
  which is the surface ADR-0021 established for the engine and which the content lane reaches for
  first.
- **Each axis is the one that carries meaning for its generator**, rather than a generic
  "element index" that means something different in each scene and is documented nowhere.
- **No new mechanism.** `set_param_series` and the CPU palette sampling both exist and ship; this
  is four scenes adopting a pattern one scene already proves.
- **`hue_spread = 0` is exactly today**, so the change cannot move a golden and cannot surprise a
  shipped preset.

### Negative

- **Four scenes' colour paths grow, and three of them share a renderer that does not care.** The
  per-segment colour already exists on `SegmentInstance`; what grows is each generator's fill of
  it. That is four similar-but-not-identical loops, and they will drift unless the axis definition
  is asserted rather than commented.
- **The axis choice is baked, and one of them is genuinely arguable.** An author who wants the
  fern coloured along the drawn path rather than by depth cannot have it. Making the axis
  selectable is a param this ADR declines to add — two ways to say the same thing is how a preset
  surface becomes hard to learn — but it is the obvious thing to reopen if the lane asks twice.
- **`star_pattern`'s radial axis is the weakest of the four.** The backlog's own complaint is that
  the rosette reads as a hollow ring, i.e. its segments cluster near the rim — so a radial colour
  ramp has little radius to work with until
  [ADR-0060](0060-star-pattern-variants-interpolate.md)'s interior work lands. Sequencing, not a
  flaw in the axis.
- **More CPU work per frame per segment.** Palette sampling is a lookup into a baked table
  (ADR-0021), so the cost is per-segment table indexing rather than per-segment colour maths. It is
  still new work on a path that runs every frame and the segment counts are capped by
  `TierConfig::max_segments`.

## Alternatives considered

### Alternative A — give `lsystem` a `hue_spread` and leave the palette alone

The literal reading of design-backlog 0026: add the one missing param. **Rejected because it
entrenches the asymmetry it was filed about.** `hue_spread` without `[palette]` means the built-in
cosine ramp only, so `lsystem` would gain variation but still not reach the engine's colour system
— and `parametric_curve` and `star_pattern` would still have neither. The entry says the asymmetry
"looks unintentional"; fixing one param of it deliberately would make it intentional.

### Alternative B — one shared "element axis" defined by the line renderer

Have `LineRenderer` derive a normalized `0..1` from segment order and let every scene colour by it
uniformly. Simplest possible rule, one implementation, no per-generator decision. **Rejected
because segment order means something different in each generator, and in one case nothing at
all.** For the L-system it is turtle-traversal order, which interleaves branches of different
generations — colouring by it produces a pattern that tracks the *string rewriting*, not anything
visible in the figure. For the rotationally symmetric rosette it paints a seam across a figure with
no beginning.

### Alternative C — make the axis a preset-selectable param

`hue_axis = "depth" | "path" | "radius"`. Most expressive, and it defers the arguable choice to the
author. **Rejected as surface for its own sake.** It triples the documentation of each scene's
colour behaviour to serve a preference nobody has expressed yet, and a param whose legal values
differ per scene is exactly the kind of thing the loader's typo check (ADR-0020) cannot help with.
The axis is one line to change per generator if the lane asks.

## Notes

`spectrum`'s `SERIES_PARAMS` rule — whole-figure params degrade to their `index = 0` value — is the
precedent for how these scenes should treat a series aimed at `hue_spread` or `palette_mix`, and it
is already documented at `spectrum.rs:237`. The four scenes should share that wording rather than
each inventing its own.
