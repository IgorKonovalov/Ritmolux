# 0123 — A flat-graphic scene paints its own paper and composites opaque elements in one distance-field pass

> **Status:** accepted 2026-08-26 (Plan 0113) — see `Outcome`
> **Date:** 2026-08-25
> **Related plan(s):** [0113](../plans/done/0113-the-engine-paints-a-canvas.md)
> **Builds on:** [ADR-0056](0056-additive-scenes-emit-premultiplied-alpha.md) (the alpha model),
> [ADR-0085](0085-how-much-a-scene-occludes-the-backdrop-is-one-number.md) (`occlude` as coverage),
> [ADR-0046](0046-linear-light-hdr-composite-bloom-tonemap.md) (the tonemap this look lives under),
> [ADR-0105](0105-the-mark-roster-becomes-a-fullscreen-distance-field.md) (the fullscreen-SDF idiom),
> [ADR-0045](0045-quality-tiers-floor-and-rich.md) (where the element cap lives)
> **Relates to:** [design-backlog 0069](../design-backlog.md) — this delivers occlusion *within one
> scene* and does not discharge that entry; see Consequences.

## Context

The user asked for a Russian avant-garde world: flat opaque colour, hard edges, discrete objects on
a light ground — Malevich's suprematist canvases and Kandinsky's *On White II*. Every one of the
engine's eleven systems draws **light**. Scenes emit premultiplied additive colour into a
linear-light composite ([ADR-0018](0018-engine-wide-scene-compositing.md),
[ADR-0046](0046-linear-light-hdr-composite-bloom-tonemap.md),
[ADR-0056](0056-additive-scenes-emit-premultiplied-alpha.md)), and additive light cannot put a black
bar in front of a red one. `design-backlog 0069` states the surviving gap in exactly those words —
*"nothing in this engine decides what is in front of what; a shaped object that occludes another
figure is unbuilt"* — and prices it as a composite redesign.

That price is wrong for this ask, and three facts already in the tree say so.

**A fullscreen scene already holds the backdrop out.** `fragment_field.rs:165` returns alpha across
every pixel, and Plan 0091 Phase 1 measured the consequence: at `occlude = 1` the frame is
byte-identical over a lit backdrop and over a black one. The backdrop is not darkened, it is
**absent** — held out by coverage. A scene that covers every pixel therefore owns its own ground,
which is precisely what Plan 0091 concluded a light ground must do ("a light ground must come from
the CHAIN, not from `bg_*`").

**Flat colour survives the tonemap exactly.** ADR-0046's curve is the identity below `KNEE = 0.6`
(`tonemap.rs:127`), hue-preserving, applied as a scale on the brightest channel. An element whose
brightest channel is at or below 0.6 reaches the display byte-identical. Flatness does not have to
be argued for against this pipeline; below the knee the pipeline is a no-op.

**The same bound suppresses bloom.** Bloom's threshold sits above where sub-0.6 content lives — the
mechanism `design-backlog 0087` files as a *defect* for reaction-diffusion. Here it is the feature:
a canvas that stays under the knee gets no halo, so hard edges stay hard, at no cost and with no
new parameter. The one constraint buys the two properties this look is made of.

What the tonemap does not give is paper at pure white. The curve's own table records
`f(1.0) = 0.800`, and 1.0 is asymptotically unreachable — `f(x) = 1 - 0.16/(x - 0.2)` above the
knee, so 0.95 costs a linear emission of 3.4. Both reference grounds are off-white (Kandinsky's
canvas explicitly, Malevich's by age), so this is a property to state rather than a defect to fix.

The composite therefore holds opaque graphics **unchanged**. The decision that remains is where the
drawing loop lives.

## Decision

We will add a twelfth system, `shape_collage`, rendering **one fullscreen fragment pass** over a
storage buffer of flat elements. Each pixel initialises to the paper colour and walks the element
array **in array order**, rejecting any element whose precomputed axis-aligned bounding box does
not contain it, and compositing the survivors with `over` using analytically antialiased coverage
from the element's signed distance function. Array order is depth order, so occlusion is exact and
requires no depth buffer, no sort and no ordering infrastructure. The pass emits alpha 1, holding
the backdrop out and owning its ground. An element carries a **palette coordinate** rather than an
RGB triple, so ADR-0086's backdrop palette, ADR-0102's coordinate edge and the A/B crossfade all
apply with no special case. The maximum element count is a `TierConfig` capacity value
(ADR-0045), set from measurement rather than assumed.

## Consequences

### Positive
- **No composite change.** ADR-0018's chain, ADR-0056's alpha model and ADR-0085's `occlude` are
  untouched. The capability lands as a scene, which is the cheapest place in this architecture for
  a capability to land.
- **Occlusion is exact and free.** The painter's loop is the ordering mechanism; there is no depth
  buffer to size, no sort to keep stable, and no per-frame ordering state.
- **Hard edges without MSAA.** Coverage comes from the distance function against a pixel-width
  smoothstep, so edges are analytically antialiased at no memory cost and stay correct under
  arbitrary rotation.
- **The palette surface transfers whole.** Storing a coordinate rather than a colour means every
  palette, custom stop and crossfade already documented in `docs/preset-palettes.md` works here on
  arrival.
- **Flat colour and no bloom are the same constraint.** Staying under the knee delivers both, and
  neither needs a parameter.
- **The escape is cheap.** Only the draw path is at risk from the cost measurement: the SDF roster,
  the element struct, the layout generator and the whole preset surface survive a move to either
  alternative below.

### Negative
- **Cost is O(elements) per pixel, and the bounding-box reject saves the body, not the iteration.**
  A wavefront still walks every element; rejection removes the distance evaluation, not the loop
  step. At N elements and roughly six operations of rejection, a pixel pays about 6N before
  anything is drawn — so at 1080p and 60 Hz the rejection *alone* scales as N x 0.75 GFLOP/s.
  **The element cap is therefore load-bearing**, and it is what decides whether a Kandinsky-density
  canvas is affordable at the floor tier. Plan 0113 measures it and carries a stop gate rather than
  assuming.
- **Pure white paper is unreachable.** `f(1.0) = 0.800`; near-white costs linear emission well above
  1, which re-enters bloom's threshold and forfeits the free hard edge. Off-white is the affordable
  ground and presets should be authored knowing it.
- **A canvas under the knee gives up the engine's entire over-range vocabulary** — no bloom, no
  glow, no highlight modelling. Correct for this look, and it means a `shape_collage` layer sharing
  a chain with an additive layer will not share its glow in any meaningful way.
- **Occlusion stays per-scene.** Two `shape_collage` elements order correctly; a `shape_collage`
  element and a `swarm` particle still do not. `design-backlog 0069` stays live, and takes a dated
  update at this plan's close naming which half moved.
- **A twelfth system is another arm in every exhaustive match** — `SystemKind`, `ALL`,
  `VARIANT_COUNT`, the scene factory, `draws_through_shared_line_renderer`, the param roster, a
  golden fixture, and the five behavioural gates.

### Neutral
- The system cannot sit as an **upper** ADR-0090 layer while it emits a literal alpha 1; it composes
  as the lower layer with additive scenes over it. A `paper_alpha` parameter would lift that, and is
  deliberately out of Plan 0113's scope.
- `shape_collage` and `shape_field` are adjacent by name and by mechanism, and that parallel is
  intended: one figure's contour bands versus many discrete opaque elements.

## Alternatives considered

### Alternative A — Instanced quads with `over` blending
Draw each element as an instanced quad into an offscreen pre-cleared to paper, blending in
submission order. Cost scales with **covered pixels** rather than element count, so hundreds of
elements stay cheap. Rejected as the first delivery because it needs a new blend state and a
"scene owns its clear colour" concept — engine machinery for a capability the fullscreen route
delivers with none — while the same distance functions are still required inside each quad, so
none of the shape work is saved. **It is the named escape** if Plan 0113's measurement says element
count, not coverage, is the wall.

### Alternative B — Tile-binned distance field with a compute prepass
Bin elements into screen tiles in a compute pass, then have each pixel walk only its own tile's
short list. The best asymptotics of the three, and what a production GPU vector renderer does.
Rejected on machinery rather than merit: two passes, a bin buffer, an overflow policy and the
engine's first compute-shader scene, all committed before anything is known about whether the plain
loop suffices. It is this ADR's stated escalation, and Plan 0113 Phase 3 is the gate that can buy
it.

### Alternative C — Two layers with the `multiply` blend
Reuse shipped machinery — [ADR-0090](0090-a-preset-composes-two-scene-layers.md)'s layers and
[ADR-0106](0106-two-tone-graphics-come-from-a-multiply-layer.md)'s darkening blend — to get dark
figures on a light chain. Rejected on capability, not cost: **multiply is commutative, so it cannot
express occlusion at all.** A red bar crossing a blue bar multiplies toward black instead of
reading as one in front of the other, and the layer count is capped at two regardless. It answers
the "dark on light" half of backlog 0069 and none of the "in front of" half, which is the half this
ask is about.

### Alternative D — Extend `shape_field` rather than add a system
`shape_field` already draws signed-distance figures through the palette, and a fortieth figure looks
like a smaller change than a twelfth system. Rejected because the two draw opposite pictures:
`shape_field` renders one figure's **contour bands** as additive glow, and its
`palette_steps`/`color_span` surface exists to control that banding — exactly the machinery a flat
opaque element has to switch off. [Plan 0098](../plans/0098-the-figure-nests-properly.md) is
concurrently changing that file for nested contours, so the merge would also contend. Two systems
with parallel names and opposite fill models is the honest shape.

## Notes

The reference set the user supplied is six paintings across four problems, and only the first two
are in Plan 0113's scope: Malevich's suprematist canvases (floating flat quads, bars, circles and
triangles on white) and Kandinsky's *On White II* (the same vocabulary plus thin lines, arcs,
concentric rings, checkerboard patches and translucent crossings). Malevich's figurative
constructivism is authored bespoke geometry, adjacent to
[Plan 0092](../plans/0092-the-engine-draws-an-authored-path.md); Severini's fragmented collage is a
subdivision field and a different mechanism entirely.

**The element-count requirement is counted from those references, not estimated**: 14 elements in
*Suprematism* (the black disc canvas), roughly 20 in *Dynamic Suprematism*, roughly 35 in
*Suprematist Composition*, and above 40 in *On White II* once lines and arcs are included. That
count is what Plan 0113's stop gate is measured against.


## Outcome (added at Plan 0113's close, 2026-08-26)

**Every load-bearing claim in Context held, and one sentence in it was false.** The
decision itself is unchanged: the composite took no change at all, a fullscreen
`alpha = 1` scene owns its own ground, bloom's threshold sits above the knee so hard
edges came free, and the element cap landed as a `TierConfig` field. Phase 3's stop
gate read the cost and continued at Floor 40 / Rich 96 — the count this ADR's Notes
derive from *On White II*, arrived at from the reference paintings rather than from
the budget, because the fidelity ceiling turned out to bind first.

**The false sentence is in Context, and it propagated.** This ADR states that an
element whose brightest channel is at or below `0.6` *"reaches the display
byte-identical"*. It does not. A palette stop is a **linear coefficient with no sRGB
decode**, so the byte the display receives is that coefficient's sRGB *encoding*:
measured on the shipped preset, `#111111` presents as `#494949`, `#8a1420` as
`#BF5164`, and the paper `#d9d5c8` as `#E2E0DA`. Five downstream sites had copied the
claim — `docs/preset-palettes.md`, `presets/README.md`,
`presets/collage_suprematist.toml`, `core/tests/fixtures/shape_collage.toml` and
`shape_collage.rs`'s module docs — and Plan 0113 Phase 9 repaired all five; the second
close review found this ADR still carrying the original, which is the document those
five cite. Recorded here rather than edited into the body above, per this project's
append-only rule.

**What the knee actually buys is the property the look rests on**, and it is the
reason the decision survives the correction untouched: below `KNEE` the **tonemap** is
the identity, so every element's fill leaves the post chain **unshaded and halo-free**,
shifted by the same transfer curve as every other element. Flatness and hard edges both
follow from that; neither ever depended on the authored hex reaching the display.
`an_element_under_the_knee_arrives_at_the_value_it_was_authored_at` asserted the
correct thing from Phase 1 — `encoded(hex/255)`, not `hex` — so no test was ever wrong,
only the prose around it. Author a `shape_collage` palette by the rendered result.

**One consequence this ADR did not anticipate.** `coverage` in `core/tests/sanity.rs`
read `1.0000` for this family, which Phase 1 took as a structural property (*"its lit
fraction is 1.0 by construction"*). It was not: it was the old lens measuring painted
paper against black — the degeneracy
[ADR-0126](0126-the-sanity-lens-measures-departure-from-the-frames-own-ground.md) was
raised on. Against the derived ground the two shipped canvases read `0.3028` and
`0.2677`, and Phase 6b re-derived the floor to `0.13` from that distribution. A scene
that paints its own paper is exactly the case a black-reference lens cannot see, and
that is a property of **this ADR's fullscreen-paper decision**, not of the sanity gate.
