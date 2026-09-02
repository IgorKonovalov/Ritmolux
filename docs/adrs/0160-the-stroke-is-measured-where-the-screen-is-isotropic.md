# ADR-0160 — The stroke is measured where the screen is isotropic, not in NDC

> **Status:** accepted 2026-09-02 (Plan 0149)
> **Date:** 2026-09-01
> **Related plan(s):** [0149](../plans/done/0149-the-line-corners-stop-being-blunt.md) Phase 2a
> **Supersedes the metric half of:** [ADR-0041](0041-line-joins-are-per-endpoint-on-the-segment-instance.md)
> (nothing of its join design; what changes is the space the half-width is applied in)
> **Unblocks:** [ADR-0158](0158-a-joined-end-carries-its-own-miter-length.md) — whose Decision this
> makes correct as written, at zero instance cost
> **Supplements:** [ADR-0124](0124-the-line-stroke-carries-a-solid-core-and-a-pixel-wide-edge.md)
> (whose pixel calibration assumed this property), [ADR-0037](0037-internal-grid-is-a-resolution-not-a-shape.md)
> (the same failure shape, one level down), [ADR-0023](0023-golden-drift-guard-uses-frozen-fixtures.md)

## Context

The line renderer's vertex shader converts both endpoints into clip space, computes the segment's
direction and normal **there**, and offsets by the half-width **there**:

```wgsl
// core/src/render/scenes/lines/renderer.rs — Work in aspect-corrected space so the
// perpendicular offset is a uniform on-screen thickness whatever the segment's orientation.
let a_s = vec2<f32>(a_v.x * inv_aspect, a_v.y);
let b_s = vec2<f32>(b_v.x * inv_aspect, b_v.y);
...
let nrm = vec2<f32>(-dir.y, dir.x);
let pos = base + nrm * c.y * width;    // `pos` IS out.pos
```

**That comment states the intended property, and the code three lines below it does not hold.**
Clip space is the *anisotropic* space here. A clip displacement `(dx, dy)` lands on
`(dx·W/2, dy·H/2)` pixels, and `W/H` is the aspect — so one clip x unit is `aspect` times as many
pixels as one clip y unit. **World space is the isotropic one**: `world.x` becomes `world.x/aspect`
in clip and therefore `world.x·H/2` pixels, exactly as `world.y` does. Dividing x by the aspect is
what makes world space square on screen; the stroke is then applied *after* that divide, in the
space the divide just left.

The consequence is an on-screen half-thickness that varies with the segment's world angle `phi`:

```
width · (H/2) · sqrt(sin²phi · aspect² + cos²phi / aspect²) / sqrt(cos²phi / aspect² + sin²phi)
```

which is `width·H/2` at `phi = 0` and `width·H/2·aspect` at `phi = 90`. **A vertical stroke is
`aspect` times thicker on screen than a horizontal one**, in every line scene, today.

Measured 2026-09-01 on this plan's Phase 1 tree — flat `width`, no miter anywhere — at
`half_width = 0.10`, lit threshold `0.05`, on the hardware adapter, as the ratio of a vertical
stroke's thickness to a horizontal one's:

| target | aspect | vertical / horizontal |
|---|---|---|
| 1000x1000 | 1.0000 | **1.0000** |
| 1280x800 | 1.6000 | **1.5789** |
| 1920x1080 | 1.7778 | **1.7843** |
| 800x1280 | 0.6250 | **0.6333** |

The ratio tracks the aspect. Residuals are pixel quantization on counts of 76-182.

**Three documents in the tree describe this, and two of them describe it wrongly.**

- `renderer.rs`'s vertex comment, quoted above, states the property the code fails.
- `SegmentInstance::width`'s doc block: *"a half-width in NDC-y units (uniform on screen after the
  aspect divide)"*. The unit name is precise; the parenthetical is false.
- `ARC_SHADER`'s doc block is **accurate, and deferential rather than approving**: *"The stroke,
  though, is a half-width in NDC - that is what the segment path strokes (`nrm * width` is applied
  after the aspect divide), and this primitive has to draw the same picture as a densely sampled
  polyline of the same arc, **not a better one**."* The arc's whole `grad` conversion — dividing the
  exact world radial distance by the clip length of its own gradient — exists solely to reproduce
  the segment path's metric. It is a consistency decision taken downstream of the defect, not an
  independent endorsement of it.

**Nothing in this repo ever chose orientation-dependent thickness.** No ADR argues for it.
[ADR-0124](0124-the-line-stroke-carries-a-solid-core-and-a-pixel-wide-edge.md) calibrated the stroke
in pixels — *"`THIN = 0.0025` NDC-y is a **1.35 px** half-width at 1080p"* — which is
`0.0025 · 1080 / 2`, the **horizontal-segment** figure, quoted as though it were the thickness. The
threshold was set against one orientation without anyone noticing there were two.

**Why it survived, and why it is ADR-0037's failure one level down.** At aspect 1.0 the two spaces
coincide *identically* — `inv_aspect` is exactly `1.0` and every multiplication by it is exact in
`f32`. The line golden corpus renders at **128x128**, `line_joints` at **512x512**, `spectrum` and
`warp_mesh` at **96x96**: every one square. No capture in the line suite can distinguish the two
metrics, by construction. This is precisely ADR-0037's rule — *find the configuration where two
sources disagree and ask whether anything probes it* — applied to a space rather than to a size, and
this family has now shipped that shape four times.

**What makes it load-bearing now** is [ADR-0158](0158-a-joined-end-carries-its-own-miter-length.md).
A flat `width` is orientation-independent, so nothing before ever had to ask which space it lived
in. A miter is `width / sin(theta / 2)`: the producer measures `theta` in world coordinates and the
shader applies the result along a direction computed after the divide. Plan 0149's Phase 2 opened
with a stop gate on exactly that question, the gate ran, and it failed. Every repair that keeps the
clip-space metric changes what `SegmentInstance` carries — the neighbour direction (+16 B, the cost
ADR-0158 rejected a shader-side miter over), the per-endpoint bisector (+16 B), or the aspect handed
to producers that build once at `configure` and survive a resize.

## Decision

**The stroke — its half-width, its join extension and the direction both are taken along — is
applied in world space, and the aspect divide happens once, on the way out.** `dir`, `nrm`, the
endpoint extensions and the perpendicular offset all move ahead of the divide; only `out.pos`
carries the `/ aspect`.

`SegmentInstance.width` and `ext_a`/`ext_b` become **world half-widths**. That is a change of unit,
not of layout: no field is added, none moves, and the instance stays **44 bytes**.

`ARC_SHADER` moves with it. Its signed distance becomes the world radial distance directly, and the
`grad` conversion — which exists only to reproduce the segment path's clip metric — is deleted along
with the metric it was reproducing. Off the angular span, the endpoint distance loses its own aspect
divide for the same reason. The arc pipeline moves **unconditionally**, because `warp_mesh` emits no
`ArcInstance` and the four line families are the only producers of one.

**Three properties carry the decision:**

- **The comment becomes true rather than being deleted.** The intended property was written down
  before the code failed it; this ADR delivers the property, and the only prose that changes is the
  two blocks that assert it falsely.
- **[ADR-0158](0158-a-joined-end-carries-its-own-miter-length.md)'s Decision becomes correct as
  written, and costs nothing extra.** With `dir` in world space, the producer's world-space `theta`
  is the angle the shader extends along. No neighbour direction, no bisector, no aspect at
  `configure`, no byte on the instance. The stop gate's second branch is answered by removing the
  disagreement rather than by transporting a correction across it.
- **The two primitives keep one metric, stated once.** Today they agree because the arc replicates
  the segment; after this they agree because both measure where the screen is isotropic, and the
  replication machinery is gone rather than maintained.

### Scope: the metric is a per-draw parameter, and `warp_mesh` keeps the old one

`warp_mesh` is a `SegmentInstance` producer sharing the same vertex shader, so it cannot be scoped
out by touching only the line families. **The metric therefore becomes an explicit parameter of the
draw call**, carried in the segment uniform's `v.w` — a lane whose own comment reads `w: unused`, so
this costs no bytes anywhere, adds no bind-group entry and changes no resource (ADR-0058). The three
segment entry points (`draw`, `draw_split`, `draw_opaque`) take it; the four line families pass the
world metric and `warp_mesh` passes the clip metric.

That an engine carries two stroke metrics is a cost, and it is deliberate and dated. The reason is
the one [ADR-0158](0158-a-joined-end-carries-its-own-miter-length.md)'s Scope paragraph already
gives, with more force here because this changes *thickness* rather than corner length:
`warp_mesh` is judged against `foo_vis_milk2`, and
[Plan 0142](../plans/0142-the-milkdrop-import-earns-its-verdict.md) is approved and about to write
ADR-0113's third Outcome from readings taken on it. **Moving the instrument between the question and
the answer is not a tradeoff, it is a mistake.**

There is also a live possibility that the clip metric is *correct* for that surface and not merely
deferred: MilkDrop authors in a square `[-1,1]` space and its own stroke may be anisotropic on
screen too, in which case `warp_mesh` matching it is compatibility rather than debt. **That is
unverified** — nobody has measured it against the reference rig. The question is deferred to
Plan 0142's close, where the readings that answer it will already have been taken.

An explicit parameter is chosen over inferring the metric from which entry point a caller used: a
producer that passes the wrong one is then visible at its own call site, and the two surfaces'
disagreement is legible in the source rather than implicit in a pipeline.

## Consequences

**Positive.**

- **The property ADR-0124 calibrated against becomes the property that holds.** Its `1.35 px` at
  1080p and `1.0 px` at 1280x800 stop being the horizontal case quoted as the general one and become
  true at every orientation. `MIN_HALF_WIDTH` and the `0.167` dead zone
  ([backlog 0098](../design-backlog-archive.md), closed) were derived on those same figures and stay
  arithmetically valid — they were always the horizontal numbers, which is now the universal one.
- **ADR-0158 ships without growing the instance.** The alternative repairs cost 16 bytes or break
  the cached producers; this costs zero and deletes a question instead of answering it.
- **The softness ramp stops varying with orientation too.** The fragment reads `fwidth(side)` off a
  quad whose screen width currently changes with `phi`, so the pixel-wide edge ADR-0124 specifies is
  a pixel only where the stroke is horizontal. It becomes one pixel everywhere.
- **The arc's `grad` conversion and its explanatory paragraph are deleted, not maintained.** The
  correct arc distance is the world radial distance, which is what the primitive already computes
  exactly before converting it away.
- **A non-square fixture becomes able to see the line renderer at all.** The suite gains its first
  probe of a property the whole square corpus is structurally blind to.

**Negative — these are the price.**

- **Every line preset gets thinner, and by more than the extreme case suggests.** All orientations
  converge on today's *horizontal* thickness, which is the thinnest one. The factor lost is the
  formula above: `1.00` at horizontal, `1.45` at 30 degrees, `1.63` at 45, `1.78` at vertical, and
  its mean over a uniform distribution of orientations is **1.52** at 16:9. An orientation-uniform
  figure — which is most of the roster — loses about half again of its stroke weight. **Whether
  `WIDTH_SCALE` compensates is deliberately not decided here**: it is a look question, this project
  picks look questions from side-by-side renders, and Plan 0149 Phase 2a carries it as a judged step
  before the constant is set.
- **`spectrum`'s `Bars` take the full `aspect` factor**, because a bar is vertical by construction.
  ADR-0158 promised those baselines would stay still; that promise was about the *miter*, it is
  scoped to it, and this ADR moves them for an unrelated reason. Restated: the miter must not move
  `Bars` and `RadialRing`; this metric change does, on any non-square target.
- **The line golden corpus cannot see the change and will not move** — which is a cost, not a
  saving. `128x128`, `512x512` and `96x96` are all aspect 1.0, where the transformation is exactly
  the identity in `f32`, so a green suite is **no evidence at all** that this landed correctly.
  The one baseline set that does move is `composite_*` at **160x100**, whose fixtures are
  `parametric_curve`. A phase implementing this must bring its own non-square measurement; the
  drift guard is silent here by construction.
- **The engine carries two stroke metrics until Plan 0142 closes**, and a reader of `warp_mesh/draw.rs`
  has to be told why. That is one comment and a dated deferral, and it is the cost of not perturbing
  a live measurement.
- **Every producer's `width` and extensions change meaning together.** The rescale in
  `LineInstance::styled` is a ratio and is unaffected, but a producer that mixes a world extension
  with a clip half-width renders a wrong-length stroke. The units move as a set or not at all.

## Alternatives considered

**Keep the clip metric and hand the miter what it needs.** Pass the neighbour direction (+16 B, the
exact cost ADR-0158 rejected a shader-side miter over), the per-endpoint bisector (+16 B, with the
shader finishing it as `w / |cross(bisector, dir)|`), or the aspect to the producers. Lost on paying
real bytes — or breaking the cached producers, which build at `configure` and survive a resize — to
correct *half* of a defect while leaving two doc comments false and the arc's replication machinery
in place. It also transports a correction across a disagreement this ADR simply removes.

**Keep the clip metric and correct the three comments to match it** — declare orientation-dependent
thickness the intended behaviour. Genuinely the cheapest option: no golden moves, no preset changes
weight, and the tree becomes self-consistent for the price of three paragraphs. Lost because
**nothing ever chose it**. The vertex comment states the opposite intent, ADR-0124's calibration
assumed the opposite, and the arc shader's own doc says it reproduces the segment metric so as to
draw *"not a better one"* — deference, not endorsement. A property that no ADR argues for, that
three documents contradict, and that a user-visible defect report now depends on, is a defect.

**Pre-correct `width` per instance on the CPU.** The producer multiplies the half-width by the
orientation factor for that segment, and no shader changes at all. It works, and it is wrong here
for three reasons: every producer must then know the aspect, which is exactly the cached-producer
break this decision avoids; the correction is duplicated in five places where one shader line
suffices; and it does nothing for the arc, whose stroke has a different orientation at every
fragment along it.

**Fix it engine-wide, `warp_mesh` included.** The consistent-looking choice and the one a reader of
the Decision would assume. Lost on Plan 0142's live measurement, per the Scope paragraph — and more
sharply than the same argument lost for ADR-0158, because this moves the stroke's *weight* against
the reference renderer rather than the shape of its corners.

**Take the metric fix as its own plan, ahead of Plan 0149.** Cleanly separates two golden-moving
changes and lets Plan 0149's Phase 2 land untouched on a corrected tree. Lost on sequencing cost:
Plan 0149 is open with its lane live and a `dev` session's context already on these two files, the
metric fix is what unblocks that plan's reason for existing, and splitting it strands the miter
behind a second plan's whole cycle for no engineering benefit. It lands as Plan 0149 Phase 2a
instead, ahead of the phase it unblocks.
