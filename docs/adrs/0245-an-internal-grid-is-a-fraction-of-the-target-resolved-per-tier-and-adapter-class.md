# ADR-0245 — An internal grid is a fraction of the render target, resolved per tier and adapter class, and the sample budget counts against the grid

> **Status:** accepted 2026-09-26 (Plan 0223), with an Outcome
> **Date:** 2026-09-22
> **Related plan(s):** [0223](../plans/done/0223-the-heavy-presets-fit-the-integrated-gpu.md)
> **Supplements:** [ADR-0034](0034-internal-resolution-follows-the-target.md) (the grids follow the
> target, capped), [ADR-0037](0037-internal-grid-is-a-resolution-not-a-shape.md) (a grid is a
> resolution), [ADR-0045](0045-quality-tiers-floor-and-rich.md) (the two tiers and the governor),
> [ADR-0140](0140-a-sample-budget-is-a-density-against-the-render-target.md) (the attractor's
> density law), [ADR-0240](0240-a-setting-lives-in-a-file-and-the-menu-edits-that-file.md) (a
> setting has a file key)

## Context

The per-preset bench (`scripts/bench/`, 2026-09-22) found the integrated GPU of the reference laptop
missing 60 fps on seven attractor-heavy presets, and the discrete GPU holding the 165 Hz cap on all
ten. Two facts about those readings were not in the bench's own notes. The headless `--stream` path
pins `Tier::Rich` by construction (`standalone/src/stream.rs`), so every iGPU column in
`scripts/bench/results/` is a Rich-tier number; and the live rows were Rich too, because the
operator's `config.toml` carries `[quality] tier = "rich"`, which disables the governor in both
directions. The Leviathan log rows on the iGPU hold 26 frames a second for all 800 frames with no
step, which is the governor never firing. So the lag was measured on the tier ADR-0045 calibrates
against a discrete GPU. It is still a real reading: the same laptop's Floor tier at 1080p was
already measured *on* the 16.67 ms budget at Plan 0128 Phase 1 (p99 16.854 ms), and the owner's
stated case is that both tiers should be usable on that machine.

**Where the frame goes was measured, not argued.** Leviathan on the iGPU (RADV RENOIR, Vulkan,
build `d4a1abb1`), headless at Rich and 1920x1080, 600 frames per run, in one session, with variant
presets loaded through `RLX_PRESET_DIR`:

| variant | ms |
|---|---|
| as shipped (600 000 particles) | 37.9 |
| `[particles] density = 0.5` (300 000) | 26.2 |
| `density = 0.16` (96 000) | 18.4 |
| `bloom_amount = 0` | 33.4 |
| `kaleido_order = 1` | 33.3 |
| `trails = 0` | 30.7 |
| all three post stages off | 24.7 |
| all three off, 96 000 | 10.3 |
| Meter Mono, the fixed floor | 4.2 |
| as shipped at 1280x720, still 600 000 | 18.1 |
| as shipped at 2560x1440 | 46.6 |
| as shipped, RTX 3080 Laptop | 5.7 |

Three readings. The additive sprite fill costs **about 3.9 ms per 100 000 particles**, linear, so
the 600 000-particle draw is roughly twenty of the thirty-eight milliseconds: each particle is a
six-vertex quad of about five texels a side, blended `One, One` into an `Rgba16Float` field, some
300 MB of scattered read-modify-write a frame. The three post stages cost about thirteen: the chain
runs twenty-eight full-screen `Rgba16Float` passes a frame, about 310 MB of traffic, on a grid that
`grid::quantize_axis` rounds **up** from 1920x1080 to 2048x1280, 26 % more texels than are ever
shown. And **the whole frame tracks the internal grid's area, not the particle count**: at 720p
the same 600 000 particles cost half, because a sprite's texel footprint scales with the grid it is
drawn into. On shared DDR4 that is a bandwidth-bound frame; the discrete part's tenfold bandwidth
is why the same frame is 5.7 ms there.

**The relief lever this project already named does not reach the case.** ADR-0046 and
`TierConfig::post_cap`'s docs both say: if the float chain misses NFR §1 on a floor-tier iGPU,
lower the cap. But the caps are absolute sizes — `post_cap` 1920x1080 and `attractor_trail_cap`
2560x1440 at Floor — and neither binds at 1080p, which is exactly where NFR §1 claims the floor. A
Floor window at 1080p draws its trails, kaleidoscope, bloom source and attractor field at the full
2048x1280 whatever the machine. The lever is inert where it is needed.

**The density law has the same blind spot from the other side.** ADR-0140 counts the attractor's
sample budget against the render *target*, and its purpose is a constant density of samples per
texel of the field the deposit lands in. Once the field is smaller than the target, counting
against the target over-samples it and pays fill for samples the field cannot resolve.

Two constraints frame what a repair may look like. ADR-0045 chose two named tiers over any
load-dependent feature shedding, so the picture stays a function of a few named inputs and a
capture reproduces. ADR-0046 chose one float format for both tiers so shaders and pipelines stay
singular. And the adapter class is already an input the renderer holds: `RenderContext` records
whether the adapter is a software rasterizer, and the `# renderer adapter` line prints
`IntegratedGpu` or `DiscreteGpu` on every run.

## Decision

We will make every internal grid — the post chain's grid and the attractor's trail field — a
**fraction of the render target**, `grid = quantize(target * scale)`, capped as today, with the
scale resolved **once at renderer construction from the tier and the adapter class** and never
changed per frame. `wgpu::DeviceType` maps to a crate-local adapter class (integrated, discrete,
software, other) the way `is_software` already does; scene code never sees it. The scale table has
one row per tier and class, **every value 1.0 until Plan 0223 measures the two integrated rows on
the reference laptop**, and the discrete and software rows stay 1.0 by decision, so no golden
baseline and no discrete-GPU picture moves. A headless render or capture resolves 1.0 whatever the
adapter, the same way it pins `Floor`, so a `shot` still does not change between two machines of
different class; the explicit `--grid-scale` is how a headless run asks for less.

The quantizer rounds to the **nearest** 128-texel step with a floor of 256 on each axis, instead
of up to 256: a 1920x1080 target takes a 1920x1024 grid, and the 128x128 and 96x96 captures the
suites run still take 256x256. The attractor's sample budget (ADR-0140) is resolved against the
**grid's** texel count rather than the target's, so the reference density is what the field
actually holds; at scale 1.0 both clamps leave every count where it is today.

The scale is a setting with a file key, per ADR-0240: `[quality] grid_scale`, `"auto"` by default,
or a number in `0.25..=1.0`, with `--grid-scale` and `RLX_GRID_SCALE` as the one-run overrides at
the same precedence `tier` uses. The settings menu edits the key; the diagnostics overlay prints
the resolved scale beside the tier. The foobar2000 component resolves `auto` and gains no C ABI
field: a pin over the ABI is a later decision.

## Consequences

### Positive
- **Both terms of the measured frame fall together.** Sprite fill and the twenty-eight post passes
  both scale with grid area; a 0.5 scale is a quarter of both, and the 720p row above is the
  existing evidence that the frame follows.
- **The relief lever binds at 1080p**, which is where NFR §1's floor is claimed, and it binds by
  adapter class, so the discrete GPU pays nothing.
- **Nothing pinned moves.** The software row is 1.0, the discrete row is 1.0, headless resolves 1.0,
  and the nearest-128 quantizer returns 256 for every capture size the suites use. The change to
  the density law is a no-op wherever the grid equals the target.
- **ADR-0045's shape holds.** The scale is a capacity resolved at construction from named inputs,
  not a function of load history. A frame never reads it.

### Negative
- **A tier is no longer one set of numbers.** An iGPU Floor window and a dGPU Floor window draw the
  same scene at different internal resolutions, so "Floor" names a picture only together with the
  adapter class. Every frame-time figure now names its tier, its adapter **and** its scale.
- **The picture on an integrated GPU gets softer.** Trails, bloom and the attractor field are
  rasters (ADR-0034), so a fraction costs sharpness and nothing else — but at 0.5 it is visible, and
  ADR-0034's "1.07x downscale is indistinguishable" argument does not cover it. The plan's
  measurement phase is a look judgement as much as a frame-time one, which is why a human owns it.
- **The committed 640x360 documentation cards move once**, from a 768x512 grid to 640x384, because
  rounding to nearest changes every grid above 256 that is not already on a 128 step. The captures
  the suites run do not.
- **One more axis for the content lane to know about.** A preset authored on a discrete GPU is
  seen on an integrated one through a smaller field; `presets/README.md` gains a sentence.

### Neutral
- The C ABI does not change. The scale resolves inside the core from what the context already holds.
- The governor is untouched: it still demotes `Rich -> Floor` once, and a pin still disables it.
  A pinned `Rich` on an integrated GPU now takes the Rich-integrated row, which is the owner's case.

## Alternatives considered

### Alternative A — lower the absolute caps, as ADR-0046 prescribed
Bring `post_cap` and `attractor_trail_cap` down at Floor. Rejected because a cap is a ceiling and
1080p is under it: to bind at the floor's own resolution the Floor cap would have to be *below*
1080p, at which point it is a fraction of the target written as an absolute, and it would still
bind the discrete GPU's Floor the same way.

### Alternative B — a cheaper composite format on the integrated GPU
`Rg11b10Float` or an 8-bit chain halves every post pass's traffic. Rejected twice over: ADR-0046
decided formats do not fork by tier so pipelines stay singular, and the accumulation fields need
fp16's mantissa for the normalized deposit (ADR-0140's own precision worry). A format fork would
also touch every shader; a grid fraction touches one function and one constructor.

### Alternative C — a third tier between Floor and Rich
Rejected because the axis the measurement found is bandwidth, which is grid area, and a tier is a
set of capacities. A `Balanced` tier would need its own captures, its own calibration and a third
row in every table, to express what one fraction expresses. ADR-0045 chose two tiers deliberately.

### Alternative D — the governor lowers the scale before demoting
A two-step ladder, `Rich 1.0 -> Rich 0.5 -> Floor`. Rejected because a pinned tier is never
touched by the governor, so the pinned-Rich laptop that prompted this would not improve, and
because the picture would again depend on load history, which is what ADR-0045's Alternative B
refused.

### Alternative E — a setting only, no adapter class
`[quality] grid_scale` defaulting to 1.0 everywhere, set by hand on the laptop. Rejected for the
reason ADR-0045 rejected manual-only tiers: NFR §1's floor must hold by default, and an iGPU user's
first run would still miss it.

### Alternative F — a compute-shader scatter for the attractor
Splatting each particle into a density buffer with atomics instead of rasterizing 600 000 sprites
attacks the fill term itself, roughly tenfold, and is the only lever here that would make Rich at
scale 1.0 fit an integrated GPU. **Deferred, not rejected**: it changes the look (a per-particle
perspective magnification becomes a uniform post-blur), re-blesses every attractor golden, and is
not needed if the fraction alone reaches the floor. Recorded as design-backlog 0259.

## Outcome (2026-09-26)

Plan 0223 built the mechanism and measured the two integrated rows. Three things differ from the
Decision above, and the body stands as it was written.

- **The integrated rows are not 1.0.** On the reference laptop (AMD RADV RENOIR, Mesa 26.2.2), the
  largest scale holding a 60 fps median with no one-second sample under 60 across the ten bench
  presets at 1080p windowed is **1.0 at Floor** and **0.75 at Rich**, and the table carries those
  two values. At the panel's native 2560x1440, Floor holds at 1.0 and **no Rich scale holds**:
  Nebula reads 58.7 fps at 0.5. That result changes no row, and it is recorded in `tier.rs`'s table
  docstring and NFR §1. The discrete and software rows stay 1.0.
- **The scale reaches the attractor through a fourth `Scene` widening,** `Scene::set_grid_scale`,
  a default no-op that `composite.rs` calls every frame. The target `set_target_size` receives is
  already a post stage's grid when a stage is active, so scaling there too would square the scale.
  The hook carries only the part the target does not already hold. The close review graded it
  against ADR-0030's three conditions and found it meets them: the value is not reachable through an
  existing channel, the attractor compares before it acts, and every other scene takes the default
  no-op.
- **The sample budget counts `round(target_px * scale^2)`, not the quantized grid's texels.**
  Counting the quantized grid would move Rich's budget at 640x360 from 150 000 to 160 000, because
  that target's grid is 640x384, and the plan required every budget at scale 1.0 to stay where it
  was. The count is exact at 1.0, and quantization stays uncounted, as it was before this ADR.

## Notes

- The variant readings above are reproducible from the shipped `attractor_leviathan.toml` with the
  named edits; the scratch copies were not committed.
- The bench README's iGPU tables stay as taken. A reading at a scale other than 1.0 is a new dated
  file naming the scale, never an edit of an old one.
