# ADR-0034 — Internal render resolutions follow the target (quantized and capped); the reaction-diffusion **simulation** grid does not

> **Status:** accepted (2026-07-26, at Plan 0033's close — see **Outcome** below, which corrects two
> claims this ADR made before the work was done)
> **Date:** 2026-07-26
> **Related plan(s):** [0033](../plans/done/0033-internal-resolution-and-preset-surface.md)
> **Supplements:** [ADR-0012](0012-stateful-feedback-render-system.md) (the fixed internal grid),
> [ADR-0018](0018-engine-wide-scene-compositing.md) (the composite),
> [ADR-0031](0031-post-stage-trait-instantiable-composite-chain.md) (the `PostStage` seam)

## Context

Three internal resolutions in the render path are compile-time constants, and every one of them is
now smaller than the display the project is actually used on:

| Constant | Value | Where | What it is |
|----------|-------|-------|------------|
| `TRAILS_W/H` | 1280x720 | `render/trails.rs:45` | a raster: the composited frame plus its feedback accumulation |
| `KALEIDO_W/H` | 1280x720 | `render/kaleidoscope.rs:40` | a raster: the fold's input |
| `GRID` | 256x256 | `render/scenes/reaction_diffusion.rs:48` | a **simulation** domain: the Gray-Scott U/V field |

On the 2048x1152 display the `preset-author` lane worked at, the two post stages are a 1.6x upscale
**of the whole frame** — including crisp line geometry that was rasterized at full resolution and
then thrown away — and reaction-diffusion is an 8x upscale. The lane's report is not speculative:
the user's words were "coral is broken", "fern grow... feels like it is upsized from something much
smaller", and "roses overall feels upscaled as well - quality is poor". The lane's mitigation was to
**remove `trails` from all 13 line presets** to recover sharpness, which cost those presets their
feedback and afterglow entirely. That is a direct trade between the look an author wants and
acceptable sharpness, forced by an engine constant rather than by any preset.

The fixed sizes were not arbitrary. `ADR-0012` made the simulation deliberately
resolution-independent, and `feedback.rs:45` still says so. The trails constant is documented as
"high enough that line trails stay crisp, cheap enough for the iGPU floor when a trails preset is
active" — a real budget claim against NFR §1 (>= 60 fps at 1080p on a ~2015+ integrated GPU) and
NFR §12 (a ~350 MB working-set soft ceiling on an iGPU, where GPU memory *is* system memory). The RD
constant carries its own reason at `reaction_diffusion.rs:44`: a 512² grid quadruples the per-step
fragment work the differential tests pay each warm-up frame **on the WARP software adapter**, and CI
runs there.

**The decisive fact is that these three constants are not the same kind of thing.** Trails and the
kaleidoscope are rasters: raising their resolution changes sharpness and nothing else. The RD grid
is a simulation domain, and raising it changes what the simulation *is*. At fixed `feed`/`kill`, the
Gray-Scott pattern wavelength is set in **texels**, so doubling the grid halves the pattern relative
to the frame — every shipped coral preset would look different. Worse, the obvious compensation does
not exist: the sim's 5-point Laplacian runs explicit Euler at `DIFFUSE_U = 0.16` against a stability
bound near 0.25, so the diffusion coefficient cannot simply be scaled up to restore the pattern
scale. Preserving the look at 2x resolution requires roughly 4x the sub-steps — quadratic in linear
resolution, on top of the 4x fragment cost.

There is also a memory number that decides the cap rather than being an afterthought. The trails
accumulation is a `PingPongField`, and `PingPongField::FORMAT` is `Rgba16Float` (8 bytes/texel,
chosen in ADR-0012 because `Rgba8Unorm` would band the slow gradients) — **two** textures. Sizing
the post stages to a 2560x1440 cap, the way Plan 0029 capped the attractor's trail grid, costs:

```
trails field pair   2560 x 1440 x 8 B x 2  =  59.0 MB
trails composited   2560 x 1440 x 4 B      =  14.7 MB
kaleidoscope src    2560 x 1440 x 4 B      =  14.7 MB
                                             --------
per chain                                     88.4 MB     (today, at 1280x720: 22.1 MB)
```

and a Plan 0023 dual-live dissolve holds **two** `PostChain`s with fully independent GPU state, so
the transient peak is **~177 MB** against a ~350 MB soft ceiling that is mostly driver floor
already. A 1920x1080 cap costs 49.8 MB per chain (~100 MB dual-live) and is a 1.07x downscale at the
display in question — visually indistinguishable from native, and less than half the memory.

## Decision

We will make the two `PostStage` internal sizes **follow the render target**, quantized and capped
by the policy Plan 0029 already established for the attractor: round each axis up to a 256 px step,
and apply a **single** scale factor to both axes when the cap binds, so aspect is preserved rather
than squashed. The cap for the post stages is **1920x1080**, not the attractor's 2560x1440, because
the trails `Rgba16Float` field pair is charged twice during a dual-live dissolve (arithmetic above);
it is one constant, and calibrating it is an on-device task.

We will **not** make the reaction-diffusion simulation grid follow the target. RD's blockiness is
addressed first at the **reconstruction** seam — how the present pass samples a finite field —
because the artifact is most likely bilinear faceting (bilinear interpolation is C0: its gradient is
discontinuous across texel edges, and RD's present pass runs iso-contours plus a central-difference
gradient over exactly that field, which turns a smooth upscale into angular facets). If and only if
reconstruction does not resolve it, the grid takes a single fixed step to 512² accepted as a look
change, with its sub-step and WARP-suite cost measured rather than assumed. The grid stays a
constant either way.

Separately, and as part of the same decision: RD's present sampler moves from
`AddressMode::ClampToEdge` to `Repeat`. The simulation is **already toroidal** — `ld()` wraps with
`((c % size) + size) % size` — so the field is seamless and only the present sampler refuses to
wrap. That single address mode is what makes `zoom > 1` and `pan_*` usable on the scene at all
instead of smearing the edge row outward into bars.

## Consequences

### Positive
- Line geometry composited through `trails` or `kaleido_*` is sharp at the display's own resolution,
  so an author no longer chooses between the look and the sharpness. The 13 line presets can take
  their feedback back.
- The RD reconstruction fix, if the hypothesis holds, costs approximately nothing and changes **no**
  chemistry: every shipped coral preset keeps its look, with no re-tune and no sub-step cost.
- `pan_*` on reaction-diffusion becomes an infinite seamless scroll over a torus — a better lever
  than the one `presets/README.md` currently promises, from a one-line change.
- The policy is one shared function rather than three constants, so the next stage that needs an
  internal grid inherits the answer instead of picking a new number.

### Negative
- **Goldens change.** The trails and kaleidoscope baselines stop rendering through a fixed 1280x720,
  and RD's change if the grid moves. Every re-bless must name its scope in the commit — `LMV_BLESS`
  rewrites *all* baselines, not the failing one.
- **Memory grows** even at the 1920x1080 cap: ~50 MB per chain against ~22 MB today, doubled during
  a dissolve. That is real headroom spent on an iGPU where GPU memory is system memory (NFR §12), and
  the number is a projection, not a measurement — it needs the on-device pass.
- **The iGPU frame budget is unverified.** Full-resolution trails means a full-resolution ping-pong
  read/write per frame. NFR §1's >= 60 fps at 1080p floor is exactly the claim at risk, and a WARP
  capture cannot speak to it.
- **A resize now rebuilds the trails field**, so the accumulated trail clears when the window
  changes size. The 256 px quantization makes this rare rather than continuous, but a slow drag
  across a step boundary will still blink the history away.
- `PostStage::internal_size` gains the surface as an argument, so the trait's shape changes. It is
  crate-internal (ADR-0031) and reachable from no preset and no C-ABI caller, so this is a refactor,
  not a widening of a public seam — but it is a seam change and is recorded as one.
- The kaleidoscope's fold aspect correction stops being the compile-time `KALEIDO_ASPECT` and has to
  travel in the uniform, because the internal grid is no longer always 16:9.
- **If the faceting hypothesis is wrong**, the cheap path evaporates and the plan falls back to the
  512² grid with its preset re-tune. The plan carries that branch explicitly rather than assuming.

### Neutral
- Headless captures stay a pure function of their inputs (NFR §6): the target size is an input, so a
  fixed-size capture remains byte-reproducible. What changes is *which* pixels a given size produces.
- RD stays "resolution-independent" in ADR-0012's original sense — the simulation is still decoupled
  from the surface. This ADR narrows that claim to say the decoupling is deliberate, not incidental.

## Alternatives considered

### Alternative A — Make the RD simulation target-sized too
The symmetric answer, and the one the feedback note assumed. Rejected on cost and on meaning. On
cost: preserving pattern scale at 2x linear resolution needs ~4x the sub-steps on top of 4x the
fragment work, and the WARP test budget that motivated the 256 is still live. On meaning: it would
make a preset's *appearance* a function of the window size — the same coral at 1080p and at 4K would
be different-looking patterns, not the same pattern at different sharpness. That is a worse property
than the blockiness it fixes.

### Alternative B — Keep all three fixed and expose a per-preset resolution scale param
Lets an author trade sharpness against cost per preset. Rejected because it pushes an engine cost
decision onto the content lane, multiplies the verification matrix (every preset now has a
resolution axis), and answers a question nobody asked: the lane wanted the geometry it already
rasterized at full resolution not to be thrown away, not a knob for how much to throw away.

### Alternative C — Uncapped, purely surface-sized post stages
The simplest rule, and it deletes the cap constant. Rejected on memory: an `Rgba16Float` ping-pong
pair at 4K is 265 MB before the dual-live doubling, which is not survivable inside NFR §12's
ceiling on an integrated GPU.

### Alternative D — Clamp `zoom` to <= 1 on reaction-diffusion instead of wrapping the sampler
The narrow fix for the edge-smear symptom, and what the report implies. Rejected because the field
is already toroidal: clamping would remove a lever the simulation has always been able to support,
to work around a sampler address mode that is one line. It would also leave `pan_*` broken, since
panning walks off the field at any zoom.

## Outcome (2026-07-26, at Plan 0033's close)

Recorded here rather than in a superseding ADR because the **decision held in full** — the post
stages follow the target, the RD simulation grid stayed a constant at 256, reconstruction was tried
first and resolved the artifact (so the Phase 4 grid step was **skipped** under the if-and-only-if
above), and the sampler wraps. What did not survive contact is two supporting claims, both of which
this ADR asserted before anything was built:

1. **"The RD reconstruction fix, if the hypothesis holds, costs approximately nothing"** (Positive
   consequences) is **false**. The shipped reconstruction is Catmull-Rom, called five times per
   fragment (value plus four gradient taps) at nine bilinear fetches each — roughly **45 texture
   fetches per fragment** on the RD present pass. The hypothesis itself was confirmed: the artifact
   is bilinear faceting, not pixel-stepping, with facet corners on the 256-texel lattice.
   The cost is **unmeasured on real hardware**: the +16 % WARP-suite figure first reported was
   retracted in the same plan's next commit once run-to-run variance on that machine turned out to
   dominate it (193.6 s / 224.2 s / 105.2 s across runs of the same suite, the fastest *after* the
   work landed). Treat the tap count as the real number and the wall time as unknown.
2. **"A quintic-smoothed fractional texel coordinate is the cheap form"** (Decision) is **wrong for
   this pass**. It was built and measured worse than the faceting it replaces: a smoothstep-family
   warp has zero derivative at both ends, so it pins the reconstruction's gradient to zero at every
   texel centre and the derivative then oscillates once per cell. Against a pass whose `line_d`
   divides by `fwidth`, that renders as one scalloped step per texel. The whole coordinate-warp
   class is unusable here — only a genuine higher-order filter has a smooth, non-degenerate
   derivative.

One consequence this ADR did not anticipate, found at the close review and routed to a followup
plan: the 256 px round-up makes the internal grid's **aspect** differ from the target's at most
sizes, and because the composite derives the scene's aspect from the grid while both stages present
with a plain normalized blit, the whole frame is geometrically stretched whenever a stage is active
(measured 1.28x wide at 1280x800, 1.07x at 1280x720; exact at 1920x1080 and 2048x1152). The single
scale factor preserves proportions only *before* quantization. The fix is to take the composite's
aspect from the surface rather than the grid — `Scene::set_target_size` keeps the grid, which is a
resolution and not a shape.

## Notes

- The faceting diagnosis in the Decision is a **hypothesis**, grounded in the code (`Linear` filter
  at `reaction_diffusion.rs:421` and the contour/gradient operators at `:231-232`) but **not
  reproduced**. Plan 0033 makes confirming it the first thing its RD phase does, and carries the
  fallback branch.
- The memory table is arithmetic from `PingPongField::FORMAT` and the texture descriptors, not a
  measurement. `docs/on-device-validation.md` is where it gets a real number.
- Plan 0029's `trail_grid_size` is the policy function this generalizes; its close notes record why
  a single scale factor beats a per-axis clamp (the per-axis version squashed a 3440x1440 ultrawide
  to 16:9, which the aspect-ignoring present then stretched back).
