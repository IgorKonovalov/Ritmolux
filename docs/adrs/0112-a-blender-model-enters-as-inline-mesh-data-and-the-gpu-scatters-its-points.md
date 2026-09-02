# ADR-0112 — A Blender model enters as inline mesh data, and the GPU scatters its points

> **Status:** proposed
> **Date:** 2026-08-16
> **Related plan(s):** none — deliberately unscheduled, after the current roster clears
> (the [ADR-0102](0102-a-palette-coordinates-edge-is-a-per-preset-choice.md) precedent: the
> decision is recorded now, the work is taken when someone is ready to author a model)

## Context

The question arrived as a capability ask: *I have a 3D model I made in Blender — how hard would it
be to use it in the app, with shaders?* The engine's answer is counter-intuitive in both directions,
and both halves shaped this decision.

**There is no mesh path and no depth buffer anywhere.** Every render pass in `core/src/render/`
sets `depth_stencil: None` — twenty-odd of them, without exception. All geometry is either an
instanced quad (the line renderer, the emitter, the particle sprites) or a fullscreen triangle (the
field scenes, `shape_field`, every post stage). Nothing in the tree has ever consumed a vertex or
index buffer describing a surface.

**But the expensive half of putting 3D on screen is already shipped.** `particles/shaders.rs`
projects world-space points to the screen with perspective, a per-family basis and spin
([ADR-0068](0068-the-projection-basis-is-a-per-family-property.md)), and depth fade plus depth hue
([ADR-0076](0076-the-attractor-keeps-the-depth-it-already-computes.md)). Lorenz and Thomas are
genuine 3D attractors rendering today. Points-in-3D is solved; surfaces-in-3D is untouched.

**Everything this engine draws is emissive, and that is a deeper constraint than the missing depth
buffer.** Nothing computes a surface normal or evaluates a light
([backlog 0092](../design-backlog.md)).
The composite is additive, [ADR-0056](0056-additive-scenes-emit-premultiplied-alpha.md) makes alpha
*be* the falloff, [ADR-0046](0046-linear-light-hdr-composite-bloom-tonemap.md) orders the terminal
stages for additive accumulation, and
[ADR-0106](0106-two-tone-graphics-come-from-a-multiply-layer.md) had to introduce an entire multiply
layer to achieve something as modest as a dark outline. A lit, occluding object is not a feature
this engine lacks — it is a second rendering register that every terminal stage would have to learn.

An interview settled four forks, and each took the cheap side: **glowing points and wireframe** over
a lit surface, **inline preset data** over a runtime asset directory, **deformation eventually**
rather than never, and **both frontends** including the foobar plugin.

**The asset route was already decided once, against this feature's most obvious form.**
[ADR-0107](0107-an-authored-path-is-inline-svg-data-and-it-morphs-by-resampling.md) rejected
external `.svg` files (its Alternative D) because they need "a runtime asset path this project has
never had — file IO, missing-file handling, packaging" and because they make a preset stop being
self-contained. A `.glb` is that objection several orders of magnitude larger, and it lands hardest
exactly where the interview said the feature must work: the foobar plugin is a C++ shim over a
cdylib with no asset root of its own, so a file route there means a new C ABI entry point and an
`LMV_ABI_VERSION` bump ([ADR-0003](0003-c-abi-v1-surface.md)).

### The arithmetic is what makes this a decision rather than a restatement of ADR-0107

The obvious reading of "inline it, like a path" is to bake the model to a point cloud and paste the
points. **That does not fit, and the margin is not close.** The particle budget is 50,000 at the
floor tier and 150,000 at the rich tier (`tier.rs:261,280`). A point written as `"0.123,0.456,-0.789"`
costs roughly 20 bytes of TOML:

```
50,000 points x ~20 bytes   =  ~1 MB in a single TOML string
largest shipped preset      =  13.6 KB  (attractor_fern.toml)
                            -> roughly 75x the biggest file in the library
```

Inlining the **decimated mesh** instead, and scattering the points on the GPU, moves the same
picture into the size class presets already occupy:

```
~250 vertices x ~20 bytes  +  ~500 triangles x ~10 bytes  =  ~11 KB
                            -> the size of a preset that already ships
```

That is the whole decision: the inline budget constrains *how much geometry is described*, and it
turns out the description is three orders of magnitude cheaper than the picture drawn from it.

## Decision

**A `[mesh]` table carries inline vertex and triangle-index data for a decimated triangle mesh,
parsed once at load, and the GPU scatters the tier's particle budget across that surface by
area-weighted barycentric sampling seeded on the particle index.** The same data draws as a
wireframe by expanding its edges through the existing line renderer, which is a draw-mode selection
on one asset rather than a second feature. The figure is rigid at first and deforms in the compute
step later; nothing here forecloses that.

**Point density is a property of the tier, not of the asset.** One ~11 KB blob feeds 50,000 points
at the floor and 150,000 at the rich tier, and
[ADR-0069](0069-the-attractor-trades-sample-count-for-trace-length.md)'s existing `density` fraction
applies unchanged — the same relationship every attractor family already has with the budget.

**Sampling is uniform over surface area.** Curvature or edge weighting is a known and probably
necessary refinement (see the first Negative), and it is deliberately *not* taken here: uniform is
the honest starting point, and the decision to weight should be made against a rendered model rather
than in advance.

The mesh parser is written, not taken from a crate, and covers a flat numeric format the author
exports from Blender. It runs at load, never per frame.

## Consequences

### Positive

- **Density decouples from the asset**, which is the argument for the whole shape. A baked cloud
  would have to ship at rich density and discard two thirds of itself at the floor; this ships one
  description and draws whatever the machine can afford.
- **No C ABI change, no asset path, no file IO, no new dependency.** This is why "both frontends"
  costs nothing: the plugin gets the feature by linking the same core, with no asset root, no
  thirteenth function and no `LMV_ABI_VERSION` bump. It extends ADR-0107's self-containment
  precedent rather than superseding it.
- **The entire shipped chain is reused end to end** — projection, spin basis, depth fade and depth
  hue, the trail field, bloom, kaleidoscope, palette, `density`. The new surface area is a parser,
  an area CDF and a sampling function.
- **Deformation lands where it is cheapest.** The compute step is exactly where the attractor
  families already iterate per particle per frame, so displacing a sampled point is the same shape
  of work the scene already does — which is the concrete reason this route was preferred over a
  baked SDF for an author who wants deformation eventually.
- **Deterministic by construction.** The area CDF is built once on the CPU at load; the per-particle
  triangle choice and barycentric coordinates come from a hash of the particle index. No clock, no
  unseeded randomness — the project's determinism rule is satisfied without special handling.
- **The wireframe is free.** Edges are already in the index data, and the line renderer already
  strokes them.

### Negative

- **Uniform sampling produces flat density, and flat density is the opposite of what makes the
  shipped scenes beautiful.** The attractors read well because their density varies by orders of
  magnitude — that is an intrinsic property of a strange attractor's invariant measure, not
  something the renderer supplies. A uniformly sampled surface is a fuzzy shell with no internal
  structure, and **this is the single largest risk to the look**. It is not a tuning problem and no
  parameter fixes it; the answer, if one is needed, is curvature- or silhouette-weighted sampling.
- **The risk above is asymmetric between the two model kinds the author named, and worse for the
  hard-surface one.** An organic shape has curvature everywhere, so uniform area sampling roughly
  tracks where the eye expects detail. A hard-surface object is defined by its *edges and corners*,
  and area weighting gives a large flat face a uniform wash while giving the silhouette edge no
  emphasis whatever — so a cube reads as a fuzzy cube. Both kinds are in scope by the author's
  answer, and only one of them is well served by the sampling this ADR ships.
- **The author must decimate in Blender, and the ceiling is low.** ~500 triangles is what the preset
  size class affords. The workflow is Decimate modifier, export, paste — and decimation costs
  silhouette fidelity precisely on the hard-surface models that also sample worst.
- **A rigid figure saturates the trail field.** Points deposit into an accumulating trail; the
  attractors escape this only because their points move continuously. A motionless model deposits
  into the same pixels every frame and blows out. Spin (ADR-0068) is shipped and is expected to
  suffice, which means *a stationary model is not a supported look* — a real restriction, stated
  rather than discovered.
- **Occlusion is absent, and unlike everywhere else in this engine it will be noticed.** There is no
  depth buffer and this ADR does not add one, so the far surface of the model shows through the
  near one. On an attractor that transparency reads as the medium; on a recognizable object it reads
  as a **defect**, because the viewer knows what the shape is meant to occlude. Depth fade
  attenuates the far surface but does not hide it.
- **A preset stops being readable, again and further.** ADR-0107 accepted this price for a
  128-point path and named it plainly; an 11 KB mesh blob is the same price an order of magnitude
  larger, in a file format whose entire value has been that a human can read what a look does.
- **It widens what preset content is, for the second time.** ADR-0107 observed that geometry
  authored in a preset is a step past parameters; a mesh is a step past a path, and the content
  lane's boundary moves with it.

### Neutral

- **Shading is not delivered and not foreclosed.** Backlog 0092's ask stands untouched: these points
  are as emissive as everything else. If a lit register is ever built, a sampled mesh has true
  per-vertex normals available for free — better input than the distance-gradient normal that entry
  proposes — but nothing here depends on that and nothing here builds toward it.
- **The triangle ceiling is a size-class argument, not a fidelity one.** Nothing has established
  what triangle count a recognizable silhouette actually needs; ~500 is what the byte budget allows,
  which is a different claim.

## Alternatives considered

### Alternative A — inline a baked point cloud

The literal reading of "do what ADR-0107 did for paths". Rejected by arithmetic rather than
judgement: ~1 MB of TOML for one floor-tier figure, roughly 75x the largest preset in the library,
and it would additionally freeze density into the asset so the floor tier would ship data it
discards.

### Alternative B — a runtime asset directory (`.glb` / `.obj` from a folder)

The most flexible option and the closest to the literal ask — any model, no decimation ceiling, no
paste step. Rejected on ADR-0107's own reasoning (runtime file IO the project has never had,
missing-file handling, packaging, and a preset that is no longer self-contained), compounded by two
costs specific to this feature: the foobar cdylib has no asset root, so it would need a new C ABI
function and an ABI version bump, and a glTF parser is a substantial dependency against the standing
"lightweight is a feature" gate.

### Alternative C — rasterize the mesh as a lit surface

What the original question literally said — "with shaders and stuff". Rejected as the expensive
route that fights the architecture rather than using it: it needs the engine's first vertex/index
path, its first depth buffer, and its first non-emissive register, and the additive composite,
bloom's bright-pass, ADR-0056's alpha-as-falloff and the trail accumulator every one assume emission.
ADR-0106 needed a whole multiply layer to draw a dark outline; a shaded object whose dark regions are
*shape information rather than absent light* is a much larger version of that same problem.

### Alternative D — bake the model to a 3D signed distance field and raymarch it

Genuinely attractive, and the strongest rejected option. It lands directly on the machinery
[ADR-0105](0105-the-mark-roster-becomes-a-fullscreen-distance-field.md) already built — a fullscreen
distance field with palette banding and contours — needs no depth buffer and no topology, and would
deliver backlog 0092's shading almost incidentally, since the gradient of the field is the normal.
Rejected on two independent grounds. The asset cannot be inline (a 128³ field is ~2 MB, 256³ is
~16 MB), so it re-opens exactly the asset-path question Alternative B lost on. And **a baked SDF is
frozen geometry**: displacing it inside the march breaks the Lipschitz bound the march depends on
and the ray overshoots the surface, so the deformation the author asked for eventually is the one
thing this route makes hard.

### Alternative E — embed a model file at build time from the repo

`build.rs` converts a `.glb` checked into the tree, exactly as it already globs `presets/*.toml`.
This keeps every self-containment property, needs no runtime file IO, works identically in the
plugin, and **lifts the triangle ceiling entirely**, which makes it the near-miss. Rejected because
it turns content into code: a model would ship with the application, adding one would mean a
rebuild, and only a developer could do it. The ask was to use a model *the author* created, and this
route answers a different question.

## Notes

**Every number here is construction arithmetic, not measurement.** The ~11 KB figure is derived from
the preset size class backwards, and ~500 triangles is what that budget affords — neither came from
rendering anything. A plan taking this owes two measurements before the ceiling is fixed, and they
are the two that can invalidate the decision:

1. **What triangle count a decimated model needs to stay recognizable**, per model kind. If a
   hard-surface object needs 3,000 triangles to keep its silhouette, the inline route is back to
   arguing with the arithmetic and Alternative E becomes the live option.
2. **Whether uniform surface sampling reads at all.** This should be the walking skeleton — the
   author's own model on screen, untuned and ugly — placed first precisely because it is the
   cheapest moment to discover that weighted sampling is not optional.

Sequencing is deliberate: **this is future work, taken after the current plan roster clears.** It
carries no plan today, on the ADR-0102 precedent — the want is real, nobody is blocked, and the
decision is worth recording while the reasoning is fresh rather than re-derived later.
