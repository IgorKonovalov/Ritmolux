# 0092 — The engine draws an authored path

> **Status:** approved
> **Created:** 2026-08-13
> **Approved:** 2026-08-13 (user)
> **Owner skill(s):** dev, human
> **Related ADRs:** [0107](../adrs/0107-an-authored-path-is-inline-svg-data-and-it-morphs-by-resampling.md) (an authored path is inline SVG data, and it morphs by resampling)
> **Depends on:** [Plan 0091](0091-the-figure-fills-the-frame.md) (hard — the field scene this draws into). [Plan 0087](0087-the-line-renderer-draws-a-curve.md) is a **soft** dependency; see the sequencing note below.

## TL;DR

A `[path]` table takes inline SVG path data, parsed once at load into a normalized closed contour
and rendered as a per-pixel signed distance field — so a preset can author its own silhouette
instead of picking from a closed roster of five. Fill and stroke come from the *same* field
(`d < 0` and `abs(d) < w`), and two paths morph by resampling both to a common arity, aligning
winding and start point, and interpolating. The morph parameter is an ordinary bindable expression,
so a figure can become another figure on the beat.

## Context & problem

Every silhouette this engine can draw is one of five names (`marks.rs:63`), and a closed roster by
construction answers only the asks someone has already had. [ADR-0084](../adrs/0084-a-particle-marks-silhouette-is-a-signed-distance-function.md)
made that a deliberate consequence and [ADR-0105](../adrs/0105-the-mark-roster-becomes-a-fullscreen-distance-field.md)
restated it; this plan is the escape hatch, taken as a decision rather than by widening the roster
one name at a time.

**The six star references that raised the question do not motivate it**, and the plan says so up
front because it is the honest framing: five of them are the existing `star` arm wanting three
parameters, which is [Plan 0091](0091-the-figure-fills-the-frame.md) Phase 5. What motivates this is
the general capability — and the sixth reference, a cartoon star **with eyes**, which is the one
silhouette in the batch that no parameter reaches.

[ADR-0107](../adrs/0107-an-authored-path-is-inline-svg-data-and-it-morphs-by-resampling.md) settles
the four forks and carries the reasoning. Two of its findings shape every phase below:

- **Fill and stroke are one field, not two routes.** A signed distance gives both, which is why the
  interview's "both" answer costs a shader branch rather than a second renderer.
- **ADR-0098's vertex bead does not transfer.** That artifact belongs to the instanced-quad line
  renderer, where [ADR-0041](../adrs/0041-line-joins-are-per-endpoint-on-the-segment-instance.md)'s
  joins overlap and the additive composite sums them. A `min` over segment distances has no quads
  and is exactly correct at every join — so dense resampling costs ALU and compounds nothing.

### Sequencing, and a disagreement with it worth stating

This plan was sequenced after Plan 0087 so that paths would inherit its arc primitive instead of
inventing a second curve representation. **That instinct is right and the dependency is real, but it
is softer than the sequencing implies, and the difference matters because Plan 0087 carries two
gates that can end it early** into ADR-0098's Alternative C.

A polyline distance field is complete on its own. Arcs let *fewer segments* express a curve exactly,
which lowers `N` and therefore the per-pixel cost — a fidelity and performance gain, not a
prerequisite. So: take this after 0087 as intended, but if 0087 stalls or ends at its Alternative C,
**this plan is still takeable**, with the arity ceiling from Phase 2's measurement sitting lower than
it otherwise would. Phase 4 is written to consume arcs *if they exist* and to be complete without
them.

The hard dependency is Plan 0091, which builds the scene this draws into.

## Decision

Build the parser, the field and the morph in that order, and **set the arity ceiling from a
measurement rather than from ADR-0107's construction estimate**. The ADR's cost arithmetic (~2 % of
a nominal iGPU at `N = 32`) is explicitly not a measurement, and an arity ceiling chosen from an
unmeasured number is the kind of done-when this project has been burned by before.

We rejected a triangulated fill (its topology must be valid every frame a morph moves, and a shape
interpolating between two silhouettes self-intersects precisely mid-morph — a failure mode
concentrated in the feature this plan exists for), an SVG-parsing crate (the needed subset is small
enough to write; `lyon` also brings the tessellator already rejected), same-arity-only morphing
(pushes three alignment problems onto the author), and external `.svg` files (a runtime asset path
this project has never had, and a preset that stops being self-contained).

## Architecture diagram

```mermaid
flowchart LR
  subgraph load["load time — CPU, once per preset"]
    D["[path] d = \"M0,-1 L...\""] --> P["path parser<br/>(written, stated subset)"]
    P --> C["normalized closed contour"]
    C --> R["resample to N by arc length<br/>+ align winding + start point"]
    D2["[path] morph_to = \"...\""] --> P
  end
  subgraph frame["per frame — GPU"]
    R --> I["interpolate points<br/>(morph is a bound expression)"]
    I --> SDF["min over segment distances<br/>= signed distance"]
    SDF --> FILL["fill: d < 0"]
    SDF --> STROKE["stroke: abs(d) < w"]
    SDF --> BAND["palette_steps / palette_contour<br/>(free, from Plan 0091)"]
  end
```

## Implementation phases

### Phase 1 — The parser, and what it refuses

- **Owner skill:** dev
- **What:** Inline SVG path data becomes a normalized contour at load. No rendering yet — this phase
  is pure CPU and is fully testable without a GPU, which is why it is first.
- **Files touched:** `core/src/preset/path.rs` (new), `core/src/preset/mod.rs`,
  `core/src/preset/` schema, its tests.
- **Done when:**
  - The supported subset is **stated in one place and enforced**: `M`/`m`, `L`/`l`, `H`/`h`, `V`/`v`,
    `C`/`c`, `S`/`s`, `Q`/`q`, `T`/`t`, `Z`/`z`. **`A`/`a` (elliptical arc) and multi-contour paths
    are refused**, each with its own error naming what it found and why the subset excludes it —
    an author will be holding a file a browser renders correctly, and "invalid path" would be a
    cruel thing to tell them.
  - **A malformed path is a load error carrying a character offset**, not a fallback shape. A
    silently mis-parsed path renders as a plausible wrong figure, which is worse than a red build:
    it looks like a design decision.
  - The contour is normalized to the fit-normalized world the rest of the engine uses, and the
    normalization is **recorded rather than inferred** — a path authored at any scale or offset
    lands in the same place, so swapping one path for another does not also move the figure.
  - Round-trip tests over hand-written paths, including the relative-command forms (`m`, `c`, `s`)
    that are the ones a real exported file actually uses, and the smooth-continuation commands
    (`S`, `T`) whose reflected control point is the classic place a hand-written parser is wrong.

### Phase 2 — The path becomes a field, and the arity ceiling is measured

- **Owner skill:** dev
- **What:** The contour renders — fill and stroke from one signed distance — inside Plan 0091's
  scene. This is the walking skeleton: at the end of this phase a preset draws its own silhouette.
- **Files touched:** `core/src/render/scenes/shape_field.rs`, its shader, `core/tests/`.
- **Done when:**
  - A `[path]` preset renders its silhouette filled, and `abs(d) < w` strokes the same contour, from
    **one** distance evaluation — verified as a property: the stroked outline lies on the fill's
    boundary at every sample, which is what "same field" means and what a second route could not
    guarantee.
  - **The arity ceiling comes out of a measurement against NFR §1's floor tier**, and it is the
    output of this phase rather than an input. ADR-0107's ~2 % estimate at `N = 32` is a construction
    from pixel count x segments x ops, and the ADR labels it as such; the number that ships is the
    measured one. **If the measurement disagrees with the estimate, the estimate is what was wrong.**
  - A path exceeding the ceiling is a **load error naming the ceiling and the path's own count**, not
    a silent decimation — an author who pastes a 500-point traced logo needs to be told, because the
    cost is paid on every pixel of every frame whether or not the figure is on screen.
  - The aspect comes from the render target (ADR-0037), on the same terms Plan 0091 Phase 3 states.
  - A golden fixture pins one filled path, adapters compared before blessing.

### Phase 3 — Two paths morph

- **Owner skill:** dev
- **What:** The correspondence machinery — the part ADR-0107 names as having three alignment
  problems, one of which is refused rather than solved.
- **Files touched:** `core/src/preset/path.rs`, its tests, `core/src/render/scenes/shape_field.rs`.
- **Done when:**
  - Both endpoints resample to a common arity **by arc length**, so points are distributed evenly
    along the outline rather than evenly per command — a shape whose commands are unevenly sized
    would otherwise bunch its correspondence where the author happened to click.
  - **Winding is normalized by signed area.** A path authored clockwise morphing into a
    counter-clockwise one turns inside out through the middle, and it is checkable directly: the
    interpolated contour's signed area does not pass through zero for an aligned pair, and provably
    does for a mis-aligned one. Assert the negative control, or the alignment is untested.
  - **Start-point rotation is chosen by minimising total displacement over cyclic offsets.** Without
    it, a star morphing to a star can unwind through a spiral — every intermediate frame valid, the
    whole motion wrong. `O(N^2)` at load for `N` in the low hundreds is thousands of operations, so
    the brute-force search is affordable and no cleverness is owed.
  - **Mid-morph states are inspected, not assumed.** Plan 0079 swept twenty tuple pairs and *four*
    were refused by measurement because intermediate states collapsed to zero extent;
    [ADR-0075](../adrs/0075-ifs-family-morphs-in-singular-value-space.md) exists because naive
    interpolation of the obvious representation was wrong. This phase renders a strip across the
    morph for each shipped pair and **records what it saw** — a degenerate interval is a finding to
    write down, not a bug to tune away.
  - Morph is a bindable expression under the existing grammar, with `[smoothing]` applying to it
    like any other param.

### Phase 4 — Arcs, if Plan 0087 delivered them

- **Owner skill:** dev
- **What:** The soft dependency, consumed. **This phase may be empty, and that is a legitimate
  outcome** rather than a failure — it is written so the plan is complete without it.
- **Files touched:** `core/src/preset/path.rs`, `core/src/render/scenes/shape_field.rs`.
- **Done when:**
  - **If Plan 0087 landed its arc primitive:** the parser's cubic and quadratic segments are fitted
    to biarc chains through 0087's own fitter rather than a second one, and the field evaluates arc
    distance where an arc exists. The win is stated as a measurement — the arity needed for a given
    fidelity drops, and by how much — not as an assertion that curves are now exact.
  - **If Plan 0087 ended at ADR-0098's Alternative C or has not run:** this phase records that in one
    paragraph and closes. Nothing below it depends on arcs, and the Phase 2 ceiling already reflects
    the polyline cost.
  - Either way, **no second curve representation enters the engine.** If arcs exist, they come from
    0087's seam; if they do not, paths stay polylines. What this phase must not do is grow its own.

### Phase 5 — The authoring surface is documented

- **Owner skill:** dev
- **Files touched:** `presets/README.md`, `docs/presets.md`, `.claude/skills/preset-author/`
  reference sweep.
- **Done when:**
  - `presets/README.md` carries the `[path]` table, the supported subset **and the refused
    commands**, the arity ceiling with the measurement behind it, and the morph alignment rules —
    specifically that a pair morphs well when both are single closed contours of comparable
    complexity, since that is the thing an author can act on.
  - It says plainly that **a preset carrying a long path stops being readable**, which is a real cost
    of this feature and not something to discover in review.
  - The `preset-author` lane's references are swept in this commit. That lane keeps no catalogue of
    its own precisely so these stay the one copy, and the identical minor has been raised at four
    consecutive closes.

### Phase 6 — The look gate

- **Owner skill:** human
- **Done when:**
  - A verdict on the morph **in motion** — the question no test answers is whether a figure becoming
    another figure on the beat reads as transformation or as mush, and a strip of stills cannot
    settle it.
  - A verdict on whether the authoring loop is actually usable: paste a path from a design tool,
    see it render, adjust. If that loop is painful the feature does not land, whatever the tests say.
  - **May carry forward** to `docs/content-brief.md` under the rule Plan 0083's and Plan 0088's
    Phase 7 both followed. It gates nothing.

## Data shapes

```toml
# illustrative — not the final interface
[path]
d        = "M 0,-1 L 0.22,-0.31 L 0.95,-0.31 L 0.36,0.12 ..."
morph_to = "M 0,-1 C 0.3,-0.4 0.9,-0.35 0.95,-0.31 ..."   # optional
samples  = 64        # resample arity; ceiling set by Phase 2's measurement

[params]
morph  = "beat"      # ordinary bindable expression
stroke = "0.0"       # 0 = filled; > 0 strokes at abs(d) < w
```

## Risks & open questions

- **The cost estimate is unmeasured and the plan is built on it.** ADR-0107 says so in its own
  Notes. If the floor tier comes back materially worse than the construction suggests, the arity
  ceiling drops and the fidelity of a curved path drops with it — which is the scenario where Plan
  0087's arcs stop being an optimisation and become the thing that makes this viable.
- **The refused subset may be the common case.** Real exported SVGs use `A` for rounded corners and
  carry multiple subpaths routinely (any letterform with a counter). If Phase 1's refusals fire on
  most files an author reaches for, the subset is the wrong cut and that is a Phase 1 finding, not a
  Phase 5 documentation problem.
- **Morph degeneracy is expected, not hypothetical.** The recorded fallback if a wanted pair morphs
  badly is ADR-0075's: change the interpolated *representation*, not the endpoints. What this plan
  will not do is tune a bad pair until one still frame looks acceptable.
- **This is the first geometry authored in a preset.** Structural tables already carry data, so it is
  a difference of degree — but the content lane's boundary moves, and nobody has judged whether
  authoring shapes is work that lane wants.

## What this plan does NOT do

- **It does not add a runtime asset path.** No `.svg` file loading, no external references; the path
  lives in the preset.
- **It does not tessellate**, and therefore never needs a valid triangulation of a self-intersecting
  mid-morph contour.
- **It does not build a second curve representation.** Either it inherits Plan 0087's arcs or it
  stays polylines (Phase 4).
- **It does not compose multiple shapes.** The cartoon star's *eyes* — two discs on a star — are
  multi-shape composition, which is neither a path nor a parameter, and it stays unbuilt after this
  plan as it was before it.
- **It does not close the roster question.** `marks`' five shapes stay exactly as they are; a path
  is an alternative source of a silhouette, not a replacement for them.
- **It does not shade.** The chrome register is [backlog 0092](../design-backlog.md), gated on Plan
  0091 and independent of this.
