# ADR-0107 — An authored path is inline SVG data, and it morphs by resampling

> **Status:** accepted (2026-08-13, user approval)
> **Date:** 2026-08-13
> **Related plan(s):** [0092](../plans/0092-the-engine-draws-an-authored-path.md)

## Context

Six star reference images arrived with a question attached: *can we introduce paths and path
morphing, SVG-like?* Five of the six turned out to be the existing `star` arm wanting three
parameters ([Plan 0091](../plans/0091-the-figure-fills-the-frame.md) Phase 5), so the references
themselves do **not** motivate this. What motivates it is the general capability the question names:
every silhouette this engine can draw is one of a closed roster, and a roster answers only the asks
someone has already had.

Four forks were settled by interview, and the last of them changed the architecture:

- **Path data is inline SVG in the preset**, not a compiled roster and not an external file.
- **Morphing resamples both endpoints to a common arity automatically**, rather than requiring the
  author to hand-build correspondent shapes.
- **Both filled and stroked.**
- **Sequenced after [Plan 0087](../plans/0087-the-line-renderer-draws-a-curve.md)**, inheriting its
  arc primitive rather than inventing a second curve representation.

**The fill route is the load-bearing engineering question, and the arithmetic settles it.** A
per-pixel distance to an `N`-segment path is `O(N)` per pixel, which reads as disqualifying next to
a triangulated fill's `O(1)`. It is not. At 1920x1080 with `N = 32` that is ~66 M segment-distance
evaluations per frame, order 660 M ops — roughly 2 % of a current integrated GPU's throughput at
60 Hz, on a scene that draws nothing else. `N = 128` is still order 8 %. Meanwhile the triangulated
route needs a *valid topology every frame*, and a shape being morphed between two silhouettes can
self-intersect in the middle, which is exactly when ear clipping fails — so the cheap-per-pixel
route buys its speed with a failure mode that appears only during the feature this ADR exists for.

Two consequences fall out of choosing distance, and both were surprises worth recording:

**Fill and stroke stop being two routes.** A signed distance gives a fill as `d < 0` and a stroke as
`abs(d) < w`, from one field. The interview's "both" answer therefore costs one shader branch rather
than a second rendering path, and there is no second place for the path semantics to drift.

**The faceting objection does not transfer.** [ADR-0098](0098-the-line-renderer-draws-arcs-as-per-pixel-distance-fields.md)'s
vertex bead — the reason a polyline mandala looks wrong and three presets were retired — is an
artifact of the *instanced-quad* line renderer, where [ADR-0041](0041-line-joins-are-per-endpoint-on-the-segment-instance.md)'s
joins deliberately overlap and the additive composite sums the overlap. A `min` over segment
distances has no quads, no overlap and no bead: it is exactly correct at every join. So the
representation that is wrong for a stroked polyline is right for a filled one, and dense resampling
costs only ALU rather than compounding an artifact.

## Decision

**A `[path]` table carries inline SVG path data, parsed once at load into a normalized closed
contour, and rendered as a per-pixel signed distance field — filled at `d < 0`, stroked at
`abs(d) < w`, from the same field.** Morphing between two paths resamples both to a common arity by
arc length, aligns them for winding direction and start-point rotation, and interpolates the
resampled points; the morph parameter is an ordinary bindable expression.

The parser is **written, not taken from a crate**, and covers a stated subset of the path grammar.
It runs at load, never per frame.

## Consequences

### Positive

- **One field serves fill, stroke, banding and contours.** The path drops into the scene
  [ADR-0105](0105-the-mark-roster-becomes-a-fullscreen-distance-field.md) builds, so
  `palette_steps` bands an authored silhouette into offset contours and `palette_contour` outlines
  it — capabilities this ADR does not pay for.
- **The silhouette vocabulary stops being closed.** ADR-0084's roster answers only asks already had;
  a path answers the next one without an architect round trip. This is the actual argument for the
  feature, and it is why the references not needing it is not a reason against it.
- **Morphing is a pure function of the path data and the parameter**, so it satisfies the
  determinism rule without special handling — no clock, no RNG, same inputs same figure.
- **Resampling is load-time CPU work at a rate the author controls**, not hot-path work. The audio
  callback and the render loop are untouched by construction.

### Negative

- **A path grammar parser is new surface area in `core/`,** and it is the first thing this engine
  parses that is not an expression. It must be load-time only and it must fail loudly with an
  offset, because a silently mis-parsed path renders as a plausible wrong shape rather than as an
  error. The subset it declines (elliptical `A`, and multi-contour paths with holes) is a real
  capability gap that an author will hit while holding a file that a browser renders correctly.
- **Automatic correspondence has three alignment problems and the third has no clean answer.**
  Winding direction is fixable by signed area, start-point rotation by minimising total displacement
  over cyclic offsets — but two paths with *different subpath counts* have no natural correspondence
  at all, which is why the subset above excludes them rather than guessing.
- **A mid-morph shape is a shape nobody authored.** Plan 0079's tuple walk refused 4 of 20 swept
  pairs *by measurement* because intermediate states collapsed to zero extent, and
  [ADR-0075](0075-ifs-family-morphs-in-singular-value-space.md) exists because naive interpolation
  of the obvious representation was wrong. The same class applies here and there is no reason to
  expect this feature to be the one that escapes it.
- **`O(N)` per pixel is a real budget, and it is spent whether or not the path is on screen** — a
  fullscreen field evaluates everywhere. The arithmetic above says the cost is affordable at the
  arity these shapes need; it says nothing about a 500-point traced logo, and nothing prevents an
  author pasting one.
- **A preset stops being readable.** A 128-point path is one very long unreadable TOML line in a
  file format whose whole value has been that a human can read what a look does.
- **It widens what preset *content* is.** Structural tables already carry data (`[generator] rings`
  entries, `[particles]`), so this is a difference of degree — but geometry authored in a preset is
  a step past parameters, and the content lane's boundary moves with it.

### Neutral

- **The dependency on Plan 0087 turned out softer than the sequencing assumes**, and the plan says
  so rather than burying it. A polyline distance field is complete on its own; ADR-0098's arcs let
  fewer segments express a curve exactly, which lowers `N` and therefore cost. That is an
  optimisation and a fidelity gain, not a prerequisite — which matters because Plan 0087 carries two
  gates that can end it early into ADR-0098's Alternative C.

## Alternatives considered

### Alternative A — a triangulated fill

Tessellate the contour on the CPU and draw triangles: `O(1)` per pixel instead of `O(N)`, and the
obvious choice on cost alone. Rejected because the topology must be valid *every frame a morph
moves*, and a shape interpolating between two silhouettes can self-intersect precisely mid-morph —
so its failure mode is concentrated in the feature this ADR is for. The measured `O(N)` cost at the
arity these shapes actually need (~2 % of an iGPU at `N = 32`) does not justify buying speed with
that.

### Alternative B — parse with a crate (`svgtypes`, `lyon`)

Correct, complete, and free of the subset gaps named above. Rejected on this project's standing
rule that every dependency is a cost to be justified: the grammar subset needed here is small enough
to write and test directly, and `lyon` in particular brings a tessellator that Alternative A already
rejected.

### Alternative C — same-arity morphing only

Two paths morph if their command structure matches, otherwise a load error. Cheap, predictable, and
it makes the three alignment problems the author's. Rejected at the interview: it pushes the actual
difficulty onto the person least equipped to solve it, and hand-building correspondent shapes is
exactly the tedium a morph feature should remove.

### Alternative D — external `.svg` files

The most flexible and the closest to the literal ask. Rejected because it needs a runtime asset path
this project has never had — file IO, missing-file handling, packaging — and because it makes a
preset stop being self-contained, which every part of the preset system currently relies on.

### Alternative E — feed the resampled points to the existing line renderer for stroking

The natural reading of "inherit Plan 0087's primitive": strokes go through `LineRenderer`, fills
through the field. Rejected once the distance route was chosen, because `abs(d) < w` is a stroke
already — a second route would mean two implementations of one silhouette that can disagree, for a
capability the first one delivers in one branch.

## Notes

The cost arithmetic above is a **construction estimate, not a measurement**: 2.07 M pixels x `N`
segments x ~10 ops, against a nominal integrated-GPU throughput. Nothing has been rendered. Plan
0092 measures it against NFR §1's floor tier before the arity ceiling is set, and the ceiling is the
output of that measurement rather than an input to it.
