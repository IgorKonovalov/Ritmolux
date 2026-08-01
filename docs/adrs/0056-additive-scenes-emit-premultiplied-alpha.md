# ADR-0056 — A scene that draws into the chain emits premultiplied alpha equal to its own coverage, and the alpha blend saturates rather than sums

> **Status:** accepted (Plan 0051, closed 2026-08-01 — **carries an Outcome section**: the
> "provably moves no golden" claim was wrong, on a survey error rather than on the no-op argument,
> and the residual cost of coverage-as-alpha is now measured)
> **Date:** 2026-07-31
> **Related plan(s):** [0051](../plans/done/0051-the-scene-seam-emits-premultiplied-alpha.md)
> **Supplements:** [0026](0026-full-composite-coverage-fullscreen-scenes.md) (which made
> premultiplied alpha the scene→composite currency for the *present* passes),
> [0055](0055-backdrop-leaves-the-post-chain.md) (which extended that currency through the chain
> and whose first Negative bullet this ADR is the third instance of),
> [0018](0018-engine-wide-scene-compositing.md) / [0031](0031-post-stage-trait-instantiable-composite-chain.md)
> (the chain and its membership).

## Context

Plan 0045 Phase 2b moved the backdrop out of the post chain: stage inputs clear **transparent**,
every stage propagates alpha, and the last active stage resolves `PREMULTIPLIED_ALPHA_BLENDING`
over a backdrop painted into the chain's destination. ADR-0055's very first Negative bullet named
the price:

> **Every stage's alpha handling becomes load-bearing, and there is no existing test for it.**
> Today alpha after the scene present is ignored, so a stage that drops it costs nothing; after
> this it silently punches the backdrop through or holds it out.

That bullet has now come true three times. Twice inside the plan — the fold's falloff fading to
black (Phase 2b) and the bloom recombine's alpha exceeding 1 (Phase 4b) — and each time the fix
came with a lit-backdrop guard. The third instance is the one seam the plan never reached: the
**scene→chain seam**, where a scene draws directly into the chain's input rather than presenting
through an alpha-aware pass.

**Two draw pipelines emit a hard `1.0` alpha over their entire quad while colour carries a
falloff.** This is not a subtle rounding issue; it is a literal constant:

| seam | fragment output | quad geometry |
|---|---|---|
| `core/src/render/scenes/swarm.rs:186` | `vec4(in.color * g, 1.0)` | square sprite, `g = max(0, 1 - length(local))²` — **radial** |
| `core/src/render/scenes/lines/renderer.rs:150` | `vec4(in.color * g * u.v.y, 1.0)` | stroke quad, `g = max(0, 1 - abs(side))²` — across the stroke |

Both pipelines blend colour `One`/`One` (additive) with alpha `BlendComponent::OVER`. Since the
source alpha is always exactly 1, `OVER` saturates the destination alpha to 1 everywhere any quad
covers — including everywhere `g` is zero. The chain's resolve then computes
`src.rgb + backdrop * (1 - src.a)`, which at `src.a = 1` is `src.rgb + 0`. **Where the shader
wrote no colour, the frame writes black over the backdrop.**

The swarm case is the loud one because its falloff is radial over a square quad: everything
outside the inscribed disc — the four corners, about 21 % of every sprite's area — is exactly
`(0, 0, 0, 1)`. Reproduced on the unmodified preset at 1550x902, dozens of small black rectangular
notches punched into the backdrop beside every bright particle, and visible live in the app. The
line case is the quiet one — the falloff is one-dimensional, so only the stroke's long edges go
dark — but it is the same defect: at `bg_bright = 0.55` with a fat stroke, `lsystem_fern` renders
black rims and wedges over the whole figure.

**Attribution.** This is not preset content: it reproduces on the pre-retune files from history.
It is not a Plan 0045 regression in the sense of a bad edit either — the shaders are unchanged
since they were written. Phase 2b made an always-wrong alpha *observable* for the first time, by
putting something underneath it. Before that the chain forced alpha to 1 and the backdrop was
inside the chain's input, so nothing could be punched through.

**Why nothing caught it** is the part worth deciding about. Every swarm fixture and every golden
baseline for these scenes runs `bg_bright = 0`, where a black backdrop times any alpha is still
black — so the defect is invisible by construction in the entire regression suite, and invisible
again at contact-sheet scale. It bites roughly sixteen shipped presets (three `swarm_*`, thirteen
line presets) at exactly the setting the library uses least in test and most in use.

> **Correction (2026-08-01, at close).** That survey is wrong in one place, and the exception is
> the instructive one: `core/tests/fixtures/composite_kaleido.toml` is a line scene over a lit
> vignette at `bg_bright = 0.55`. It was the one baseline positioned to see this defect, it did
> not, and it moved when the fix landed. See the Outcome section.

The remaining scene seams are already correct and show what the invariant should be: the attractor
present (`particles/mod.rs:427`) and the reaction-diffusion present (`reaction_diffusion.rs:342`)
both emit a real alpha per ADR-0026, and `fragment_field.rs:130` emits `1.0` **correctly** — it is
a fullscreen opaque field that is supposed to cover the backdrop.

## Decision

**A scene that draws into the post chain's input emits premultiplied alpha equal to the coverage
that fragment actually has.** The same falloff term that already multiplies the colour becomes the
alpha: `vec4(color * g, g)`. A fragment that writes no light writes no coverage, and the backdrop
survives underneath it.

**The alpha blend component becomes `One` / `OneMinusSrcAlpha` — premultiplied OVER — while colour
stays `One` / `One` additive.** Stacked sprites therefore accumulate coverage as
`a_out = g + a_dst * (1 - g)`, which is monotone and **bounded in `[0, 1]` by construction**: it
approaches 1 as quads pile up and can never exceed it. Colour keeps summing without limit, which is
what the linear-light composite is for.

That bound is the reason for choosing this factor pair rather than the obvious `One`/`One` on alpha
too. Summing alpha additively is exactly what produced Phase 4b's defect one stage downstream — the
source alpha exceeded 1, `1 - src.a` went negative, and the frame *subtracted* the backdrop — and
it had to be repaired with an explicit clamp. Here the same saturation comes for free from the
blend state, so **an out-of-range alpha at this seam is unrepresentable rather than clamped after
the fact**. That is the same shape as Plan 0044's tier argument and Plan 0047's `SaltMode`: make
the configuration, not a reviewer, the thing that forbids the bug.

Note what the invariant is and is not. It is *"alpha equals this fragment's coverage"*, not
*"alpha is never 1"*. A fullscreen field scene covering every pixel correctly emits 1.

## Consequences

### Positive

- **The backdrop composites correctly under every scene**, which is what ADR-0055 promised and
  delivered everywhere except here. Sixteen shipped presets stop punching holes in their own
  atmosphere.
- **Over-range alpha becomes unrepresentable at this seam**, rather than being clamped downstream.
  The one place in the renderer that still needs an explicit alpha clamp is bloom's recombine,
  which genuinely sums two premultiplied sources.
- **Bloom's bright-pass stops seeing opaque black rectangles.** Today every sprite hands the stage
  a quad of `(0,0,0,1)` corners; they contribute nothing to the halo but they are noise in the one
  buffer the newest stage reads.
- **Existing goldens do not move — with one exception, found at implementation.** Colour is never
  a function of alpha anywhere in the chain — no pass un-premultiplies — so at `bg_bright = 0` the
  resolve's `src + dst * (1 - src.a)` reduces to `src` in every channel whatever the alpha is.
  That argument is already written down at `post.rs:265` and `kaleidoscope.rs:289`, and it held:
  every baseline at `bg_bright = 0` re-encoded byte-identical. What did not hold is the premise
  that *every* affected baseline runs `bg_bright = 0`. `composite_kaleido` does not, and it moved.
  See the Outcome section.
- **It closes an invariant that was stated in three places and enforced in none.** ADR-0026 set
  the convention at the present seam, ADR-0055 extended it through the chain, and neither reached
  a pipeline that draws direct.

### Negative

- **Nothing structurally forces a shader's colour and alpha to stay in step.** The decision above
  is a convention two fragment shaders must keep, and a third draw pipeline added later can break
  it exactly as these two did. The mitigation is a test, not the compiler: a lit-backdrop capture
  guard **per draw seam**, which is the third instance of the same guard shape Phase 2b and Phase 4b
  each installed. We accept the convention because the alternative — deriving coverage centrally —
  loses information only the scene has (see Alternative C).
- **The guard costs two new fixtures at a configuration the library does not otherwise test.**
  `bg_bright > 0` is deliberately absent from these scenes' baselines, and the reasoning for that
  is sound *for a baseline*. So the guard is additive test surface rather than a re-parameterized
  existing one.
- **A dense swarm now genuinely holds the backdrop out.** Where sprites pile up, alpha saturates
  toward 1 and the backdrop is correctly occluded — which is right, but it is a visible change from
  today's "the backdrop is occluded by the whole quad" only in the sense that the occlusion now
  follows the figure. Content tuned against the black rectangles (there should be none — they were
  never desirable) would notice.
- **It does not address the alpha *semantics* of an additive look.** An additive scene arguably
  wants no occlusion at all — light adds, it does not cover. Coverage-as-alpha is the conservative
  reading and matches every other seam; a deliberate "additive scenes are transparent" model is a
  different look decision and is left alone.

## Alternatives considered

- **Clamp the alpha at the chain's resolve.** One defensive edit in one place, mirroring Phase 4b.
  **Rejected because the alpha here is not out of range — it is the wrong number.** It is exactly
  1.0, perfectly legal, and a clamp does nothing to it. The notches would remain black.
- **Force alpha back to 1 at the resolve, or make the scene targets opaque again.** This is
  reverting ADR-0055. The fold's falloff would fade to black again and `bg_vignette` would be
  replicated into the wedges — the two defects Phase 2b exists to fix. Rejected on those grounds
  alone.
- **Derive coverage centrally from the emitted colour's luminance** (a shared "colour implies
  coverage" rule applied at the resolve or in a helper). Attractive because it needs no per-shader
  discipline and cannot drift. **Rejected because it makes a legitimately dark covered pixel
  indistinguishable from an uncovered one** — any scene that draws something genuinely dark would
  become transparent — and because it puts a scene-specific judgement into a shared pass, which is
  the coupling ADR-0055 rejected in its own first alternative.
- **Alpha additive (`One`/`One`) plus an explicit clamp**, mirroring bloom's recombine exactly.
  Behaviourally close, and it has the merit of consistency with the one place that does clamp.
  **Rejected because it reintroduces the failure mode by construction and then guards it** —
  `One`/`OneMinusSrcAlpha` yields a saturating accumulation with the same visual result and no
  reachable out-of-range state. Given that Phase 4b's clamp cost a whole extra phase to find, the
  version that cannot go wrong wins.
- **Fix the swarm only, and record the line seam as backlog.** The user-visible report was the
  swarm; the line rims are subtle at shipped stroke widths. **Rejected because it is the same
  defect, in the same class, found in the same hour** — deferring it guarantees the guard gets
  written once and the second seam gets discovered by a user later, which is precisely the history
  this ADR is documenting.

## Notes

The severity difference between the two seams is geometric and worth recording, because it explains
why one was reported and the other was not. The swarm's falloff uses a **radial** distance over a
**square** quad, so the zero-colour region is the four corners — a large, contiguous, hard-edged
area. The line renderer's falloff is one-dimensional across the stroke, so its zero-colour region
is the two long edges — a rim whose width scales with `thickness`. At shipped stroke widths that
rim is nearly a hairline; at `thickness = 9` it is unmistakable. Both are the same bug, and a guard
written only against the loud one would not have caught the quiet one.

## Outcome (Plan 0051, closed 2026-08-01)

The decision landed as written — `vec4(color * g, g)` at both seams, one shared
`gpu::ADDITIVE_LIGHT_SATURATING_COVERAGE`, one lit-backdrop guard per seam. Three things this ADR
asserted need correcting against what the implementation measured.

**One baseline moved, and this ADR said none would.** The no-op *argument* is sound; the **survey
around it** was not. `composite_kaleido.toml` runs `bg_bright = 0.55` — a line scene over a lit
vignette, and therefore the single baseline in the suite positioned to see this fix. It moved at
**mean 0.0009 against a 0.02 tolerance, with only the outlier gate firing (73 against 48)**. Every
other baseline re-encoded byte-identical under `LMV_BLESS`, which is independent confirmation that
the "colour never depends on alpha" premise holds. Re-blessed deliberately; the numbers and an
eyes-on description are in the fixture header.

The second-order lesson is the one worth keeping: **that baseline was positioned to catch the
defect when it shipped, and could not.** A mean-drift gate cannot see a hairline rim — 0.0009 is
22x under tolerance. Coverage at the right configuration is not the same thing as a gate that can
resolve the defect at that configuration, and only the outlier count separated them here.

**The line guard is structurally weaker than the swarm guard, and that is geometry.** The property's
domain is pixels where the scene wrote *exactly* zero. The swarm's radial falloff over a square quad
makes that a 2-D region (~21 % of every sprite, 52 651 pixels on the fixture); the line's falloff is
one-dimensional and quadratic, so its exactly-zero band is the outermost sub-pixel sliver and the
pre-fix shader violates on ~5 pixels — at an unambiguous magnitude of **0.4944**, but only 5. No
choice of `samples`/`scale`/`thickness` widens it. The guard is non-vacuous and was confirmed in
both directions; it is narrow. A stronger property is available and is recorded in
`docs/design-backlog.md` rather than built here.

**The Negative bullet about additive alpha semantics has a measured price now.** Coverage-as-alpha
means a fragment occludes by its geometry whatever it emits: the resolve is `c·g + B·(1 - g)`, so a
fragment **darkens the backdrop wherever `c < B`**. At the shipped near-black floors (0.009–0.070
across all sixteen affected presets) this is unobservable. At `bg_bright = 0.35` a swarm whose
emitted light is dim renders as **dark speckle** — verified by rendering `swarm_storm` at
`brightness = 0.02` over a lit backdrop. This is not a regression: post-fix is `>=` pre-fix at every
pixel, since pre-fix discarded the backdrop across the entire quad. It is a **ceiling on
`bg_bright`**, set by the dimmest emitted luminance in the figure rather than by the black rim that
used to set it. `presets/README.md` states it; the look question of whether additive light should
occlude at all is in `docs/design-backlog.md`.
