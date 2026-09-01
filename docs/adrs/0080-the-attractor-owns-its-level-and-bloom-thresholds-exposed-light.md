# ADR-0080 — The attractor owns its level, and the bright-pass thresholds exposed light

> **Status:** accepted 2026-08-05
> **Date:** 2026-08-04
> **Related plan(s):** [0066](../plans/done/0066-the-level-lever.md)

## Context

`exposure` is the engine-wide camera stop: one scalar per frame, applied in `tonemap.rs` ahead of
the knee, defaulting to `1.0`. It arrived with [ADR-0046](0046-linear-light-hdr-composite-bloom-tonemap.md)
as a *correction* lever — the thing that lets a preset recover the ~8 % a mid-tone-dominated look
loses to the knee.

It is now being used as something else. `presets/attractor_lorenz.toml:74` ships `exposure = "0.03"`
and `presets/attractor_thomas.toml:60` ships `"0.10"`. **These are the first two shipped presets to
bind `exposure` at all** (`grep '^exposure' presets/*.toml` returns exactly those two), and they
bind it because the attractor scene has no other way to set how bright its figure is. Compare the
sibling particle scenes: `swarm.rs:473` and `emitter.rs:611` both carry a scene-local `brightness`.
`particles/mod.rs:1695` does not — its vocabulary is `a b c d size hue fade hue_spread hue_center
saturation palette_mix zoom pan_x pan_y reseed`. The one particle family without a level param is
the one whose presets reached for the engine-wide stop.

That substitution has two costs, and neither is hypothetical.

**`exposure` crossfades across a preset dissolve.** `tonemap.rs` holds the outgoing preset's value
and interpolates (`crossfade_from`, [ADR-0024](0024-cross-preset-transitions.md)'s
seam), so a preset sitting at `0.03` drags the ~1 s blend from *any* neighbour through a badly
exposed frame. Both presets' headers record buying as much of their level as possible from `size`
and `fade` first, precisely to keep `exposure` from going further — a workaround with a ceiling.

**The bloom bright-pass reads pre-exposure light.** The chain is scene → `PostChain` (trails, fold,
bloom) → tonemap, so `bloom.rs` thresholds the frame *before* `exposure` scales it, and
`MAX_THRESHOLD = 8.0` caps what a preset can ask for. At `exposure = 0.03` the entire figure sits
over every reachable threshold: rendered, `bloom_threshold` `0.95` and `8.0` on Lorenz are
near-indistinguishable. Lorenz therefore ships the threshold pinned at the ceiling with its header
saying to read the pair as *capped, not tuned*. **A threshold in pre-exposure linear units is only
meaningful while every preset sits near `exposure = 1.0`** — true until that commit, false now.

Recorded as [design-backlog 0057](../design-backlog.md).

## Decision

We will give the `attractor` scene a scene-local **`brightness`**, a multiplier on the per-particle
additive deposit ahead of the post chain, defaulting to `1.0` and named to match the identical param
`swarm` and `emitter` already carry — so a figure's level is set where the figure is drawn, blends
as pixels across a dissolve, and leaves `exposure` to be the whole-frame correction it was designed
as. And we will make the bloom bright-pass **threshold against post-exposure luminance**: the
frame's evaluated `exposure` is passed into the bloom stage's uniform and applied to the sampled
luminance before the threshold comparison, so `bloom_threshold` means the same thing at any
exposure and stays in the display-referred units the tonemap's knee already works in.

We reject relocating the `exposure` multiply upstream to the scene→post seam (it would put every
stage behind it — including the trails accumulation — under an eased bindable, making trail
*history* depend on the exposure at deposit time), normalizing `exposure` per-preset at the
crossfade (it fixes the dissolve and not the threshold, and it silently discards an authored
value), and documenting the workaround as the technique (it leaves the cost on the next author,
which is the entry's whole complaint).

## Consequences

### Positive

- **The reason presets reach for extreme `exposure` disappears.** A level set at the scene is one
  multiply on a deposit that is already normalized by particle count
  ([ADR-0065](0065-the-attractor-deposit-is-normalized-by-particle-count.md)), so it composes with
  `density` instead of fighting it, and it does not touch the dissolve.
- **The particle families stop disagreeing.** Three scenes drawing additive marks now expose the
  same level param under the same name. An author moving between them carries the vocabulary.
- **`bloom_threshold` becomes honest at any exposure.** "Bloom what is over the display's ceiling"
  is a claim the parameter can now keep, rather than one true only near `exposure = 1.0`.
- **`exposure` recovers its designed meaning** — the whole-frame stop that answers the tonemap
  knee — which is what [backlog 0038](../design-backlog.md)
  is waiting to use it for.

### Negative

- **Every bloom-binding preset whose `exposure` is not 1.0 changes what its bright-pass selects.**
  Today that is exactly two presets, and the golden suite is untouched — **no fixture binds
  `exposure`** (`grep -l exposure core/tests/fixtures/*.toml` is empty across all 23), so the new
  factor is literal `1.0` there and every baseline is byte-identical. That is a fact about today's
  fixtures rather than a property of the design: the moment a fixture binds `exposure`, this becomes
  an ordinary re-bless, and it should be re-checked rather than assumed. The plan treats a moved
  baseline as a phase failure precisely so the assumption cannot rot silently.
- **Two params now scale the attractor's light**, and an author can reach either. The distinction —
  `brightness` is the figure, `exposure` is the frame — is real but has to be taught rather than
  discovered, and nothing in the engine prevents spending the same intent twice.
- **Lorenz and Thomas ship values that were tuned against the old arithmetic.** Their `exposure`
  and `bloom_threshold` pairs stop meaning what their headers say the moment this lands, so the
  plan owes them a retune rather than leaving two presets documented against a retired model.
- **One more uniform field on a hot per-frame path.** Trivially cheap, but it is a widening of the
  bloom stage's contract with the composite: the stage now needs to know something the tonemap
  owns, which is a coupling that did not exist.

### Neutral

- `MAX_THRESHOLD = 8.0` keeps its value and changes its meaning — it now caps a display-referred
  threshold rather than a linear-light one. Whether 8.0 is still the right ceiling is a question
  the plan measures rather than the ADR asserting.

## Alternatives considered

### Alternative A — Relocate the `exposure` multiply to the scene→post seam

Conceptually the cleanest: apply the stop to the light *before* anything downstream reasons about
light levels, so bloom needs no new knowledge and the tonemap does only the knee. Rejected because
the trails stage sits in that chain and accumulates over frames — exposure is an eased bindable, so
a preset that drifts it would make the trail buffer's *history* a function of the exposure at each
deposit, and a later exposure change could not correct light already accumulated. That is a worse
coupling than the one it removes, and it moves far more pixels.

### Alternative B — Normalize `exposure` per preset at the crossfade

Divide out each preset's exposure when blending so an extreme value stops dragging the dissolve.
Rejected because it addresses one of the two costs and not the other — `bloom_threshold` stays
meaningless — and because it makes the engine silently disregard a number the author wrote, which
is the kind of helpfulness that is indistinguishable from a bug when it surprises someone.

### Alternative C — Declare the current behaviour correct and document the workaround

Cheapest: write down that a sparse particle preset buys its level with `size` and `fade`, that
`exposure` is the last resort, and that `bloom_threshold` is capped rather than tuned below
`exposure ≈ 1.0`. Rejected because the workaround has a measured ceiling (both presets hit it and
say so in their headers), and because the asymmetry it documents — one particle scene without the
level param its two siblings have — reads as an oversight to every author who meets it, which is
what it is.

### Alternative D — Name the new param `intensity` rather than `brightness`

The attractor's deposit is an accumulation into a trail field rather than a direct emission, so a
distinct name would signal that it behaves differently. Rejected because the difference is one an
author discovers in a second of rendering and the naming cost is permanent: three particle scenes
with two names for the same lever is exactly the sort of surface a content lane mis-remembers, and
`presets/README.md` would have to explain the distinction on every mention.

## Outcome (2026-08-05, at Plan 0066's close)

Accepted as written. Three things the implementation and the Phase 5 retune settled, one of which
this ADR did not anticipate and which every future preset making the same swap will meet.

**The swap is level-neutral on the figure and NOT on the backdrop, because the background pre-pass
is upstream of the tonemap.** `exposure` scaled the backdrop; `brightness` is scene-local and does
not reach it. So a preset moving a number from one to the other keeps its figure and multiplies its
sky by `1 / old_exposure`. On `attractor_lorenz` that was 33x — a `bg_bright` authored at ~1/255
(black) would have become a visible grey wash, putting a floor under the figure the whole look is
composed against. Phase 5 caught it and divided the backdrop terms by the same 0.03
(`presets/attractor_lorenz.toml`, `bg_bright = "0.0003 + …"`). `attractor_thomas` was unaffected
because it binds no `bg_*` at all, and its `paper_*`/`ink_*` sit *downstream* of the tonemap. The
rule worth carrying: **this ADR's "level-neutral" claim covers the scene, not the frame** — check
every upstream-of-tonemap term a preset binds before moving a number across.

**The Neutral note about `MAX_THRESHOLD = 8.0` is answered, and the ceiling is not a constraint.**
Lorenz's threshold moved from `8` — where its header called the value *capped, not tuned* — to a
measured `0.4`, chosen off a coverage sweep (`3.0 / 1.5 / 0.8 / 0.5 / 0.4 / 0.3 / 0.2` reading
`0.164 / 0.165 / 0.177 / 0.207 / 0.224 / 0.245 / 0.269`) that simply did not exist before: at the
old ceiling every value in that list rendered the same picture. A real preset is now an order of
magnitude *below* 8.0, so nothing suggests the ceiling should move.

**The byte-identical claim held exactly, and it is now guarded rather than assumed.** No existing
baseline moved across the whole plan. The Negative bullet's warning — that this is a fact about
today's fixtures rather than a property — was answered by adding the fixture that makes it false:
`core/tests/fixtures/composite_bloom_exposed.toml` is the suite's first and only `exposure`-binding
fixture, and it was sized by measurement (against a build with the multiply forced back to 1.0 it
reads a max outlier of 209 against a tolerance of 48; three weaker configurations all passed within
tolerance, because the tonemap shoulder flattens a difference between two already-blown frames).

## Notes

- Deposit seam: `deposit_scale(active_count)` (`particles/mod.rs:369`) packs into `draw.w.w`
  (`:2069`) and is applied at `:734`. A `brightness` multiply composes there.
- Measurement backing the bloom half: rendered on Lorenz at `exposure = 0.03`, `bloom_threshold`
  `0.95` against `8.0`, near-indistinguishable. Recorded in
  [backlog 0057](../design-backlog.md)
  item 3.
- The `density`/`exposure` interaction this sits next to is
  [ADR-0065](0065-the-attractor-deposit-is-normalized-by-particle-count.md): density is neutral in
  *total* light and not per texel, so a sparse preset needs a cut on the order of
  `trail frames / density`. `brightness` is the lever that cut should be spent on.
