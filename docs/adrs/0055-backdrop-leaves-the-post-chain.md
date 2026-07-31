# ADR-0055 — The backdrop leaves the post chain: the composite carries premultiplied alpha and the backdrop is composited underneath

> **Status:** proposed (accepted at Plan 0045's close)
> **Date:** 2026-07-31
> **Related plan(s):** 0045-linear-light-and-bloom, Phase 2b
> **Supplements:** [0026](0026-full-composite-coverage-fullscreen-scenes.md) (the
> premultiplied-alpha present convention this generalizes), [0031](0031-post-stage-trait-instantiable-composite-chain.md)
> / [0032](0032-ink-leaves-the-chain-blend-between-chain-and-ink.md) (the chain and its
> membership), [0018](0018-engine-wide-scene-compositing.md) (the stage order).
> **Corrects:** [0047](0047-kaleidoscope-fold-domain-disc-with-falloff.md)'s second Positive
> bullet. **Gates:** Plan 0045 Phase 3.

## Context

ADR-0047 shipped the fold's radial falloff with this claim in its Positive section:

> The vignette edge is a *designed* boundary — it composes with `bg_*` (the falloff lands on
> the backdrop) instead of fighting it.

That is not what shipped, and the sample set built for Plan 0045 Phase 2 is where it became
visible. The fold's shader ends with

```wgsl
return vec4<f32>(textureSample(t_src, samp, s_uv).rgb * w, 1.0);
```

— a **multiply**, so the falloff drives the result toward **black**, not toward the backdrop.
Both sample presets use a near-black backdrop, which is why all sixteen captures look correct
and none of them show the defect. `core/tests/golden/composite_kaleido.png` uses
`bg_bright = 0.55`, and that is where a lit backdrop darkens toward black across the falloff
band.

The obvious repair — hand the stage a backdrop colour — runs into the reason the claim was
wrong in the first place, and the reason is structural rather than a missing uniform.
`post.rs`'s routing renders **background + scene [+ trails] into the first active stage's
input**, so the backdrop is *inside* the texture the kaleidoscope folds. Three consequences
follow, and only the first was noticed before:

1. There is no backdrop colour to fade *to*, because the backdrop is already folded into the
   thing being faded.
2. The backdrop is not a colour. `background.rs` paints an iq-cosine palette times a vertical
   gradient times a radial vignette (`bg_hue`, `bg_bright`, `bg_vignette`) — a **field**, so any
   single RGB handed to the stage is an approximation of it, not a match.
3. **The fold folds the backdrop.** `bg_vignette`'s radial darkening is centred on the frame,
   and the kaleidoscope replicates it into the wedge pattern — now, after Plan 0045 Phase 1,
   around a fold axis (`kaleido_center_x/y`) that need not be the vignette's centre at all.

The third item is pre-existing and was not introduced by Phase 1; it has simply never been
written down. It is also the one that makes the cheap repairs unattractive, because each of
them leaves the backdrop being folded and merely paints a better colour outside the disc.

The convention needed to fix all three properly **already exists in this codebase**. ADR-0026
gave the fullscreen scenes a premultiplied-alpha present over the backdrop, and the emissive
scenes already run *additive colour with `BlendComponent::OVER` alpha* — so scene alpha is
already meaningful, already premultiplied, and already bounded in `[0, 1]` (`OVER` is
`SrcAlpha`/`OneMinusSrcAlpha`; the additive factors are on the colour component only). What is
missing is that the **post chain** discards it: every `PostStage` resolve is
`BlendState::REPLACE`, and the chain's inputs are cleared opaque.

## Decision

We will **take the backdrop out of the post chain's input and composite it underneath the
chain's output**, carrying premultiplied alpha through the chain.

Concretely, four changes that together are one decision:

- The chain's stage inputs clear to **transparent**, and the scene draws into them with its
  existing blend states — unchanged, because they already write meaningful alpha.
- Each stage propagates alpha rather than forcing `1.0`. For the kaleidoscope this is exactly
  the fix: `w` multiplies **colour and alpha together**, so the falloff fades to *transparent*
  instead of to black.
- The `Background` pass renders into the chain's **destination** instead of into the first
  active stage's input, and the **last active stage's resolve** blends
  `PREMULTIPLIED_ALPHA_BLENDING` over it instead of `REPLACE`-ing it. Intermediate resolves stay
  `REPLACE` — they overwrite their own offscreen and the alpha travels with the colour.
- The **trails** accumulation decays alpha on the same schedule as colour, so a pixel the trail
  once touched does not hold the backdrop out forever.

The no-active-stage path is untouched: background and scene already render straight to the
destination there, and that path was never wrong.

Because each live chain composites its own backdrop before the transition blend sees it, a
dissolve keeps each side's own `bg_*` — which is what the dual-live path (ADR-0032) already
intends.

## Consequences

### Positive

- **ADR-0047's second Positive bullet becomes true.** The falloff lands on the backdrop
  because the backdrop is genuinely underneath it, at every `bg_bright`, with no colour passed
  anywhere and no backdrop maths duplicated into a second shader.
- **The backdrop stops being folded.** `bg_vignette` keeps its own frame-centred geometry
  instead of being replicated into the wedges around an unrelated fold axis. The same holds for
  every future stage: bloom will bright-pass the *scene*, not the scene plus a lit backdrop,
  which is the behaviour Plan 0045 Phase 4 would otherwise have had to discover on its own.
- **It generalizes a convention rather than inventing one.** ADR-0026 already made
  premultiplied alpha the scene→composite currency; this extends the same currency through the
  chain, so there is one alpha model in the renderer instead of one at the scene seam and an
  opaque assumption after it.
- **No new named param, no C ABI change, no preset-visible surface.** The fix is invisible in
  the authoring vocabulary; presets get correct behaviour from the params they already bind.

### Negative

- **Every stage's alpha handling becomes load-bearing, and there is no existing test for it.**
  Today alpha after the scene present is ignored, so a stage that drops it costs nothing; after
  this it silently punches the backdrop through or holds it out. The mitigation is a capture
  assertion at a **lit** backdrop (`bg_bright > 0`) — the one configuration in which the whole
  class of alpha bugs is visible, and precisely the configuration the sixteen Phase 1 samples
  did not have. This is the ADR-0037 lesson in another costume: near-black backdrops are the
  configuration we happen to author at, and no capture taken there can tell you whether alpha
  was handled.
- **Trails gains an alpha decay policy that is a look decision, not just a correctness one.**
  Decaying alpha with colour is the choice here; a slower alpha decay would keep the trail
  holding the backdrop out longer, which is a different (and defensible) look. We take the
  simple coupling and leave the split as a followup only if the content lane asks.
- **Goldens move a second time.** The fixtures with a lit backdrop move in Phase 2b, and then
  every fixture moves again in Phase 3's format conversion. The overlapping subset is re-blessed
  twice. We accept this to keep the alpha restructure separately reviewable from the float
  conversion — see Notes.
- **The claim being corrected was in an accepted ADR's Positive section.** ADR-0047 is not
  edited; it gains an Outcome pointing here. Anyone reading 0047 alone still reads the wrong
  sentence unless they reach the Outcome.

## Alternatives considered

### Alternative A — re-evaluate the backdrop function inside the fold shader
Duplicate `background.rs`'s palette/gradient/vignette maths into `kaleidoscope.rs`, driven by
the same `bg_*` uniforms, and `mix` toward it. Pixel-exact against what the background pass
would have painted. **Rejected:** it puts the backdrop's definition in two shaders that must be
edited together forever, with nothing to catch a divergence — and it repairs only the fold,
leaving bloom and every future stage to duplicate the same block a third and fourth time. It
also leaves the backdrop being folded, so `bg_vignette` stays kaleidoscoped.

### Alternative B — pass one flat backdrop colour into the stage
One extra uniform, `mix(backdrop, sampled, w)`, no duplicated maths. The cheapest option by a
wide margin. **Rejected:** the backdrop is a gradient times a vignette, not a colour, so this
is an approximation that reads as wrong exactly where the backdrop is most visible (high
`bg_bright`) — the case it exists to fix. Same residual defect as A: the backdrop is still
folded.

### Alternative C — fade to the unfolded frame
Outside `r_max`, sample the source at the original coordinate instead of the folded one, so the
region shows the composite as it would have been, backdrop and all. No uniform, no duplication,
exact by construction. **Rejected:** it paints unfolded scene content outside the disc, which is
Alternative A of ADR-0047 (wrap) wearing better clothes — the objection that killed wrap was
that a centred figure should not have unrelated content in its corners, and correct-but-unfolded
content is still unrelated content.

### Alternative D — accept the black fade and correct ADR-0047's wording
Change nothing in code; strike the "lands on the backdrop" claim from 0047 and document that
the falloff goes to black. Honest and free. **Rejected by the user**, who declined both this and
deferring it to the backlog. It is worth recording *why* it is not merely lazy: a fade to black
is a defensible look on a dark preset and indefensible on a lit one, so accepting it would make
`bg_bright` and `kaleido_*` quietly incompatible — which is the same class of "these two
features cannot be used together" defect that `kaleido_center_x/y` was just added to remove
(backlog 0011).

## Notes

**Why Phase 2b runs before Phase 3, not after.** Both orderings re-bless the overlapping
fixtures twice, so re-bless economy does not decide it. Risk isolation does: Phase 3 adds a
tonemap, and a tonemap sitting on top of an alpha bug makes the alpha bug much harder to read —
"the backdrop is too dark here" and "the curve rolled it off" look alike in a capture. Settling
the alpha model first also matches this plan's own founding logic, which put the fold before
bloom so that bloom built against settled resampling; bloom equally wants to build against a
settled *coverage* model, and Phase 4 follows Phase 3.

**What this ADR does not decide.** Whether the folded backdrop was ever a look worth keeping is
a content question, not an architectural one — the restructure removes it either way. It is
filed in `docs/design-backlog.md` for `preset-author` to answer against real presets, and if the
answer is "we lost something", the way back is a bindable choice about *where the backdrop
composites*, not a reversal of the alpha model.
