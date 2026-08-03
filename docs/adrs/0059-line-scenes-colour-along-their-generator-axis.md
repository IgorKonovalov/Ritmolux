# ADR-0059 — Every line scene honours the palette and colours along its own generator's axis

> **Status:** **accepted** — implemented by [Plan 0054](../plans/done/0054-the-line-scenes-catch-up.md),
> closed 2026-08-03. **Carries an Outcome section**: the divisor and one axis's usefulness both
> moved under measurement.
> **Date:** 2026-08-01
> **Related plan(s):** [0054](../plans/done/0054-the-line-scenes-catch-up.md)
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

## Outcome (Plan 0054, closed 2026-08-03)

The decision holds — all four line scenes reach `[palette]` / `[palette_b]` / `palette_mix` /
`hue_spread` / `saturation`, each on the axis named above, and the colour half lives once in a
shared `ColorRamp` in `lines/mod.rs` rather than as the "four similar-but-not-identical loops"
this ADR's own Consequences warned would drift. Two things this ADR got wrong, both corrected by
measurement rather than by argument:

**The `lsystem` divisor is not `visible_depth`.** This ADR says the ramp normalizes over it; the
implementation normalizes over the **built figure's own deepest generation**, and the ADR is wrong
in both directions. A grammar can open more than one branch per rewrite — `lsystem_fern`'s
`X -> F+[[X]-X]-F[-FX]+X` opens two, so its deepest generation runs 1, 3, 5, 7, 9, **11** across
`visible_depth` 1..6, and dividing by 6 would clamp five sixths of the figure at the palette's far
end. A bracket-free grammar has deepest generation **0** at every depth, so there is no range for
`visible_depth` to describe at all. Normalizing over the figure's own maximum makes `hue_spread = 1`
span the palette exactly once whatever the branching factor, and it is a **load-time** quantity, so
an eased `visible_depth` cannot sweep the divisor through fractional values mid-fall — the
[smoothing-sweeps-through-invalid-values](0049-analysis-v2-dual-resolution-axis-normalized-bands.md)
hazard, avoided by construction.

**The `star_pattern` radial axis is not "narrow" — it is identically flat, and that is a stronger
result than this ADR anticipated.** A Hankin rosette is `2n` *congruent* segments about a centre
`normalize_fit` leaves at the origin (every tiling order the loader accepts — 4, 6, 8, 12 — is even,
so the bounding box is centred), so every segment occupies the *same* radial interval and one colour
per segment has nothing to tell them apart. Measured across both shipped presets and all three
variants of each: the spread of segment radii is **1.2e-7**, f32 noise rather than a range. The
normalization therefore collapses to `u = 0` for the whole figure, making `hue_spread` exactly the
identity on this scene rather than a hidden constant hue shift. The surface ships anyway — `[palette]`
/ `saturation` / `palette_mix` are real gains there, and the ramp comes alive on its own the day a
construction puts segments at different radii — with the inertness stated in the module docs, in
`presets/README.md`'s axis table, and in a test that **fails when the interior work lands**, which is
the good failure. The figure's own radial extent is a different quantity and is the one
[backlog 0007](../design-backlog.md) reported: `star_rosette`'s 12-fold/20-degree rosette lives
between radius 0.54 and 0.90, emptying the inner **60 %** of the disc, and `star_lantern`'s
55-degree variant empties **87 %** — both pinned against the closed form `sin(a)/sin(pi/n + a)`.
That is the interior question and no colour axis can answer it.

**`lsystem_arrowhead` gains nothing from `hue_spread`, and that is reported rather than papered
over.** Its rules contain no brackets, so all seven of its depths sit at generation 0 — a property
of a Sierpinski arrowhead, not a gap in the axis. It does gain the palette. Worth knowing because
[backlog 0026](../design-backlog.md) was raised against Arrowhead specifically.

**The superset claim holds behaviourally and is not bit-exact, which was stated rather than blessed
through.** The golden suite ran without `LMV_BLESS` and drifted nowhere; exact byte comparison of
512x512 90-frame captures across all shipped `lsystem_*` / `rose_*` / `curve_*` / `star_*` presets
bounds the difference at **one 8-bit level** on 0.021 % to 0.724 % of pixels. The cause is not this
change: it is ADR-0021's 256-entry LUT bake of the same iq cosine replacing the analytic call, the
identical approximation `spectrum`, `fragment_field` and `swarm` already ship.

## Notes

`spectrum`'s `SERIES_PARAMS` rule — whole-figure params degrade to their `index = 0` value — is the
precedent for how these scenes should treat a series aimed at `hue_spread` or `palette_mix`, and it
is already documented at `spectrum.rs:237`. The four scenes should share that wording rather than
each inventing its own.
