# ADR-0086 — The backdrop colours through the preset's palette

> **Status:** proposed
> **Date:** 2026-08-04
> **Related plan(s):** [0072](../plans/0072-the-backdrop-joins-the-palette.md)
> **Supplements:** [ADR-0021](0021-shared-palette-system.md) (the last surface it did not reach),
> [ADR-0018](0018-engine-wide-scene-compositing.md) (which introduced `bg_hue`)

## Context

[ADR-0021](0021-shared-palette-system.md) exists because the same iq cosine palette was duplicated
per scene and every future scene would re-duplicate it. Its fix was one shared module every scene
colours through: a gradient baked once into a 256-entry LUT, sampled by GPU and CPU alike. Since
then the surface has converged — reaction-diffusion was pulled onto `spectrum` in Plan 0020 Phase 5
(its baseline re-blessed, its default look deliberately changed), the attractor followed, and the
four line families joined at [ADR-0059](0059-line-scenes-colour-along-their-generator-axis.md) /
Plan 0054.

**The backdrop never joined, and it is now the only thing left outside.**
`core/src/render/background.rs:70` carries its own copy of the cosine inline in its WGSL —
`let d = vec3<f32>(0.10, 0.42, 0.62);` — and the pass binds one uniform and nothing else
(`@group(0) @binding(0) var<uniform> u: Bg`, carrying `bg_hue`, `bg_bright`, `bg_vignette`). That is
the third copy of the constant ADR-0021 was written to de-duplicate, and it shipped without being
noticed because the pass predates the module and nothing fails.

The consequences are not cosmetic:

- **`[palette]` does not reach the backdrop.** An `ember` preset draws an ember figure over a
  *spectrum-cosine* sky. `attractor_clifford`'s crimson→ember→white-hot custom gradient does not
  tint its own backdrop, and no value of `bg_hue` will make it.
- **`saturation` and `palette_mix` do not reach it either.** An A/B palette crossfade — the feature
  ADR-0021 added for audio-driven palette *selection* — moves the figure and leaves the sky behind.
- **The documentation asserts the opposite.** `presets/README.md` reads "`bg_hue` offsets into the
  shared cosine palette". There is no sharing, and `docs/preset-palettes.md` — the document that
  owns the colour surface — does not contain the string `bg_hue` at all
  ([backlog 0059](../design-backlog.md#0059--the-backdrop-is-the-one-surface-left-that-does-not-colour-through-the-shared-palette-and-nothing-says-so)).

ADR-0021's Context named `bg_hue` once, to say it was a *background* colour and not a *scene*
palette — true at the time, and a statement about which problem that ADR was solving rather than a
decision that the backdrop should stay out. It has not been revisited since every other surface
converged.

**The scope was measured before this was written.** 26 of 37 shipped presets bind `bg_bright > 0`.
Eleven of those declare no `[palette]`, so their gradient already *is* `spectrum` and they cannot
move. Fifteen declare one and would re-tint.

## Decision

We will make the background pass sample the same baked palette LUT every other scene samples, and
delete its private cosine. `bg_hue` keeps its name and its type and becomes a **coordinate in the
preset's gradient** — cyclic, with exactly the semantics `color_center` and `hue_center` already
have. `saturation` and `palette_mix` apply to the backdrop as they apply everywhere else, through
the same group-1 (`lut_a`, `lut_b`, sampler) bind group and the same `desaturate` helper, so the
backdrop gains the A/B crossfade for free.

**A preset that declares no `[palette]` does not change, and the bound is arithmetic rather than
hopeful.** The built-in `spectrum` gradient is generated from the identical cosine
(`palette.rs:109`, `d = (0.10, 0.42, 0.62)`, pinned by `spectrum_reproduces_the_prior_cosine`), so
the only difference is LUT quantization and interpolation. Linear interpolation of that cosine over
a 1/256 step errs by at most `(h²/8)·max|f''|` = `1.9e-6 · 19.7` ≈ **3.8e-5**; `Rgba8Unorm` storage
adds at most half a step, **2.0e-3**. Both are then multiplied by `bg_bright`, and — this is the
part that makes it safe — the backdrop is *dim by construction*: the largest `bg_bright` anywhere in
the repo is `0.55` (a golden fixture; the shipped presets top out at `0.039`). Worst case at 0.55 is
**1.1e-3 of full scale, about a quarter of one 8-bit level**, before `grad` and `vig` shrink it
further. That is a sub-LSB change, and 20x under `golden.rs`'s own `0.02` rasterizer-drift floor.

**The fifteen that declare a palette do change, and that is the point rather than the price.**
`bg_hue` was tuned as a position in the cosine, and the same number means a different colour in a
custom gradient, so this is a re-tune. Plan 0072 owns it as its own phase, judged by looking.

## Consequences

**Positive**

- One colour language. A preset's backdrop, figure and crossfade come from the gradient the preset
  declared, which is what "the shared palette system" has claimed to mean since ADR-0021.
- The third copy of the cosine constant is deleted, not documented.
- `presets/README.md`'s sentence becomes true rather than needing a correction that explains an
  exception.
- The backdrop inherits `palette_mix` and `saturation` at no additional design cost, so an A/B
  crossfade moves the whole frame instead of half of it.

**Negative**

- **Fifteen shipped presets need a backdrop re-tune**, and nothing will fail if they do not get one:
  every one is a legal value producing a legal colour. This is the same shape as ADR-0061's `tile`
  default moving eleven presets, and it is accepted for the same reason — with the same obligation
  to follow it with a content pass rather than a note.
- **Two golden fixtures move visibly and must be re-blessed**: `emitter_lit_backdrop.toml` and
  `swarm_lit_backdrop.toml` both declare a flat `#ffcf80` two-stop palette *and* run
  `bg_bright = 0.35`, so their backdrop goes from cosine(0.55) to flat `#ffcf80`. (Arguably an
  improvement for what those fixtures test — they chose a constant palette precisely so the figure's
  colour is not a variable — but it is a baseline change either way.) `lines_lit_backdrop` and
  `composite_kaleido` declare no palette and are covered by the sub-LSB bound above.
- **The background pass gains a second bind group whose layout is shape-identical to the fragment
  field's group 1.** That is exactly the configuration
  [ADR-0058](0058-bind-group-layout-collisions-carry-evidence.md) was written about: on the DX12
  WARP software adapter a new pass whose layout matches a live pipeline's can receive *that*
  pipeline's resources, with no validation error and correct behaviour on real hardware — so the
  golden suite would bless garbage. The implementation carries ADR-0058's evidence requirement:
  compare adapters before blessing anything.
- **The backdrop can no longer be reasoned about without knowing the preset's palette.** Today
  `bg_hue = 0.30` is cornflower blue in every preset in the library; afterwards it is a position and
  nothing more. The authoring docs have to say so, which is the doc phase.
- A pass that was one uniform and no bindings becomes a pass that owns two textures and a sampler.
  The cost is real but bounded — they are the same 256x1 textures already resident for the scene,
  and the pass still does not run at all when `bg_bright <= 0`.

## Alternatives considered

- **Leave the private cosine and document it (the status quo, made honest).** Cheapest, and it was
  the option the interview put first. It loses because the thing being documented is an exception
  with no reason behind it: an author who wants a coherent colour scheme cannot have one, and the
  answer would be "the sky is a different palette, permanently". It also leaves ADR-0021's
  de-duplication argument standing against a live counterexample.
- **A `bg_palette` switch — an opt-in choosing the cosine or the preset's gradient.** Protects every
  shipped look at the cost of forking the colour language forever. Decisive: it would make the
  backdrop the only colour surface in the engine with a palette *source* selector, and the thing it
  protects is a tint at `bg_bright <= 0.039` across the entire shipped set — cheaper to re-tune once
  than to carry a permanent branch. Pre-1.0 this project owes no compatibility
  ([ADR-0005](0005-versioning-and-release-cadence.md)), and a param that exists only for continuity
  is exactly the shape that never gets removed.
- **A separate `[palette_bg]` table with its own stops and its own bake.** Strictly more expressive:
  a preset could give the sky a gradient unrelated to the figure. It loses on proportion — a second
  gradient config, a second bake and a third LUT pair, to serve a two-parameter wash that is
  multiplied by at most 0.04 — and on coherence, since the complaint being answered is that the
  backdrop does not match the figure. If a preset ever genuinely wants an unrelated sky, that is a
  new entry with a real look behind it.
- **Join halfway: `bg_hue` samples the LUT, but `saturation` / `palette_mix` stay off the
  backdrop.** Rejected because it reproduces the same confusion one level down — "why does the
  crossfade move the figure and not the sky" is the identical question this ADR is answering, and
  answering it partially guarantees it is asked again.
- **Fold the backdrop into a scene rather than keeping it a pre-pass.** Not seriously considered;
  [ADR-0055](0055-backdrop-leaves-the-post-chain.md) placed it deliberately as the plate underneath
  the post chain, and nothing here disturbs that. Recorded so a later reader does not mistake this
  for a re-opening of it.
