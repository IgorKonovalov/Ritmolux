# ADR-0212 — A converted preset gets its own vertex module, and the pipeline is chosen rather than branched

> **Status:** accepted 2026-09-16 (Plan 0180), with an Outcome
> **Date:** 2026-09-16
> **Related plan(s):** [0180](../plans/done/0180-the-converted-picture-follows-the-source.md) (Phase 3, which
> parked `plan_wrong` on this question)
> **Related ADRs:** [0113](0113-milkdrop-presets-are-translated-ahead-of-time-onto-a-warp-mesh-idiom.md) (the converter),
> [0037](0037-internal-grid-is-a-resolution-not-a-shape.md) (whose aspect a stage takes),
> [0132](0132-a-rate-parameter-integrates-a-phase.md)

## Context

`warp_mesh`'s `vs_main` is **one vertex stage serving two constituencies**. Native presets bind
`warp`, `zoom`, `rot`, `cx`/`cy`, `dx`/`dy` and `sx`/`sy` through the preset grammar —
`core/tests/fixtures/warp_mesh.toml` and the shipped `warp_smoke`, `warp_sirocco` and `warp_cauldron`
all do. Converted MilkDrop presets reach the same stage through `milk::MilkRuntime`, because
`encode_warp` reuses the built-in vertex module for a converted preset's warp pipeline, and a bundle
carrying no warp shader (`ShaderSpec::warp` is `Option`) uses the built-in pipeline outright.

Plan 0180 Phase 1 read the reference and found the two do **not** agree.
`CPlugin::ComputeGridAlphaValues` applies `sx`/`sy`, the rotation centre, the procedural warp's
amplitude and `dx`/`dy` in MilkDrop's **aspect-corrected** space and undoes the correction afterwards;
`vs_main` applies them in raw uv. Each differs by `1/aspect` on the shorter axis — **1.78 on y at
16:9**. Zoom and the rotation angle itself already agree, because `vs_main` aspect-corrects those two
explicitly (ADR-0037).

So the converted picture is wrong in four stages, and the fix is a different arithmetic for the same
mesh transform. Phase 3 assumed that change could be confined to `shaders.rs`. It cannot: **every
native preset goes through those same lines**, so editing them changes the native vocabulary, which
Plan 0180 rules out, and moves `core/tests/goldens/warp_mesh.png`, which Plan 0180 makes a stop.
The phase parked before writing any code, which was correct.

What is left to decide is **how a converted preset gets different arithmetic from a native one**, and
the constraint that decides it is the golden: *no native golden may move.* An option that keeps the
native picture identical **by measurement** has to be re-measured on every adapter the project ever
runs; an option that keeps the native WGSL text identical **by construction** never does.

## Decision

**The converted preset gets its own vertex shader module, and `encode_warp` chooses a pipeline.
Nothing branches inside the per-vertex path, and the native module's source text does not change.**

Three parts:

- **One source, two preludes.** `WARP_SHADER` keeps its single copy of the stage chain — the stage
  order is MilkDrop's and duplicating it is how the two copies drift apart. The four differing stages
  call two tiny functions, `to_space(p, aspect)` and `from_space(p, aspect)`, which are the identity
  in the native prelude and the aspect correction in the converted one. The variants are built by
  substituting the prelude, so the native variant's text is **byte-identical to today's**.
- **Two built-in pipelines.** `Resources` gains a converted-vertex twin of `warp_pipeline`, because a
  converted bundle with no warp shader uses the built-in one. `MilkShaderResources::build` already
  takes `warp_vs: &wgpu::ShaderModule` as a parameter, so a converted bundle's custom warp pipeline
  needs only the other module passed at that call site.
- **The choice is per preset, at encode time**, from the same `res.milk_shaders` / `scene.milk`
  the pass already consults to pick the custom fragment shader. It is one more selection beside a
  selection that exists.

**`cx`/`cy` and `dx`/`dy` are not remapped CPU-side**, although they could be: the procedural warp's
per-axis amplitude cannot be, so a CPU-side half-fix would split one correction across two layers and
leave the shader still needing the other half.

## Consequences

### Positive

- **No native golden can move, and that is a property rather than a result.** The native pipeline is
  built from unchanged text through an unchanged path. `warp_mesh.png` and the seven baselines beside
  it are untouched without anyone blessing or re-measuring them — which also removes the blocker the
  park recorded, that this machine cannot bless without re-encoding all eight.
- **No branch in the hot path.** The per-vertex stage stays straight-line code. A uniform branch would
  be coherent and cheap, but "cheap" is a claim about a GPU rather than about the code.
- **The stage order stays single-sourced.** The thing most worth not duplicating — MilkDrop's order,
  which the comment in `vs_main` argues at length — has one copy, and the difference is isolated to
  two functions a reader can hold in their head.
- **The seam already exists.** `build(…, warp_vs, …)` is a parameter today, so the converted path is a
  different argument rather than a new abstraction.

### Negative

- **A second module and pipeline are built.** At startup and preset switch, never per frame, and the
  memory is one pipeline object. But the count of things `Resources` holds goes up, and that struct is
  already large.
- **Two preludes is a second thing to keep true.** A stage added to the chain later must be written in
  terms of `to_space`/`from_space` or it silently gets the native convention on both paths. A test
  that renders the same converted preset at 16:9 and 4:3 and asserts the corrected stages move with
  aspect is what catches that; the plan owes it.
- **It does not make the converted picture right, only differently wrong until the arithmetic lands.**
  This ADR decides the mechanism; the four stages' corrected forms are Plan 0180 Phase 3's work.
- **A native preset and a converted one can no longer be compared frame-to-frame through one pipeline**,
  which is a debugging convenience that existed by accident and is now gone.

## Alternatives considered

### Alternative A — A uniform lane and a branch in `vs_main`
Fill a spare lane in `wu.misc2` from `scene.milk.is_some()` and branch on it. The obvious fix, and the
one the park named first. Rejected on the golden: the native path's arithmetic is textually unchanged
but the **module is not**, so whether `warp_mesh.png` still matches is a question about what naga and
each backend's compiler do with a uniform branch around straight-line float code. That is answerable
only by rendering it, on every adapter, every time the shader changes. The whole reason Plan 0180
makes a moved golden a stop is that it cannot tell a deliberate change from a compiler's.

### Alternative B — Per-axis correction written into the vertex's unused `t2` lanes
`mesh.rs` writes `1.0` for native and `1/aspect` for converted, and the stages multiply. Rejected for
the same reason one step weaker: multiplying by exactly `1.0` is exact in IEEE 754, so the native
result is bit-identical *if nothing reassociates*, which is again a claim about a compiler. It also
spends per-vertex bandwidth on a value that is constant across the draw, and it puts a rendering
convention in the mesh builder, which is the wrong place to look for it.

### Alternative C — Remap what can be remapped on the CPU, in `milk/mod.rs`
`cx`/`cy` and `dx`/`dy` are plain values the runtime already computes, so the correction could be
folded in before they reach the uniform. Rejected as a half-fix: the procedural warp's per-axis
amplitude is applied inside the sinusoid chain and has no CPU-side value to fold into, so the shader
would still need a converted variant — and the correction would then live in two layers, with nothing
saying which stages are corrected where.

### Alternative D — Give converted presets their own scene
A `milk_mesh` scene, separate from `warp_mesh`. Rejected as far too large for the finding: the two
share the mesh, the ping-pong field, the deposit pass, the present and the whole resource set, and
they differ in four lines of a coordinate convention.

## Outcome — 2026-09-16, at Plan 0180's close

Accepted. **The decision held; the mechanism in the Decision's first bullet did not, and could not.**

**"One source, two preludes" is self-contradictory as written.** It asks for the four differing
stages to *call* `to_space(p, aspect)` / `from_space(p, aspect)` in the shared chain, **and** for the
native variant's text to be byte-identical to today's. Adding call sites to the shared chain changes
the very string the native module is built from, so the two halves cannot both hold. Plan 0180
Phase 3 kept the second, which is the one the whole ADR turns on.

**What landed instead: anchor substitution.** `WARP_SHADER` is **not edited**, and
`warp_module_source(WarpSpace::Native)` is the quantizer plus that constant verbatim — asserted
directly by `the_native_warp_module_is_built_from_the_unchanged_source`. The converted variant is the
same constant with `CONVERTED_PRELUDE` prepended and four anchor lines rewritten
(`WARP_ANCHORS` in `core/src/render/scenes/warp_mesh/shaders.rs`). The stage order still has one
copy, which is what the bullet was protecting.

**The tripwire the Negative section asks for exists, in the other form.** Where the ADR expects a
missing `to_space` call to be the tell, `the_converted_warp_variant_edits_four_anchors` holds each
anchor to **exactly one** match in the source, so a later edit that moves or duplicates a differing
stage is a red test rather than a silently native-convention converted preset. The rendered half is
there too: each of the four stages is asserted at **128x72 and 128x96**, on a converted scene and a
native one, and `at_a_square_target_the_two_warp_chains_agree` catches the two chains drifting apart.

Two arithmetic details the ADR does not state and the implementation had to settle:

- **The rotation's two aspect factors are removed for the converted variant, not wrapped.** Corrected
  space is already isotropic on screen, so keeping them would rotate in a sheared one.
- **Stage 1 (zoom) stays in raw uv, ahead of the mapping.** A uniform scale about the frame centre
  commutes with the diagonal map about the same centre, so the two orderings are the same arithmetic.
- **`Resources::build` takes a `converted` flag rather than deriving it from `shader_spec`.** A
  converted bundle carrying no WGSL has no spec, so `shader_key` stays `0` across a switch between
  two such presets and the staleness check would otherwise keep the wrong pipeline.

**The Positive section's central claim held: no native golden moved, in any phase.** What the ADR
could not foresee is that **no converted golden moved either.** Every fixture in
`core/tests/golden.rs` renders at `SIZE = 128` **square**, where MilkDrop's aspect pair is `(1, 1)`
and the two chains are identical by construction — the same agreement the square-target test asserts.
So the corrected-space chain has **no pixel baseline at all**, and its only coverage is the
non-square probes above. That is a drift-guard gap rather than a correctness one, and it is
[backlog 0245](../design-backlog.md).
