# ADR-0061 — What the fold does outside its disc is a per-preset choice, selected by one stepped param inside a single pipeline

> **Status:** proposed
> **Date:** 2026-08-02
> **Related plan(s):** 0055-the-fold-edge-becomes-a-choice
> **Supplements:** [0047](0047-kaleidoscope-fold-domain-disc-with-falloff.md) (the fold domain),
> [0018](0018-engine-wide-scene-compositing.md) (the stage), [0055](0055-backdrop-leaves-the-post-chain.md)
> (what the falloff fades *to*). **Resolves:** design-backlog 0037.

## Context

[ADR-0047](0047-kaleidoscope-fold-domain-disc-with-falloff.md) settled the fold's domain: the
sample radius is clamped to the inscribed disc and the region beyond it fades out. It was
confirmed from sixteen rendered stills and shipped in Plan 0045. Seen **in motion** on real
presets at that plan's close, the user rejected two of its consequences:

1. the residual rays around a centred figure (`attractor_leviathan`) read as leftovers rather
   than as the designed corona the ADR bet on, and
2. the disc **crops** a fullscreen field scene (`fragment_kaleido`) — a frame that used to be
   filled is now a disc with backdrop corners.

Both are recorded in ADR-0047 as *accepted cost*, so this is the bet not holding rather than a
defect, and neither is reachable from a preset: the fold is a polar operation on a rectangular
source, so no `zoom` / `scale` / `kaleido_*` value paints those corners.

**How much of the frame this decides is the fact that reframes it.** In aspect-corrected space a
centred fold has `r_max = 0.5` while the corner radius is `0.5 * sqrt(aspect² + 1)` — **2.04x
`r_max` at 16:9**, and the disc covers `π r_max²` of an `aspect x 1` frame. So **56 % of the
frame at 16:9 lies outside the inscribed disc** (the same 56 % at 9:16, by symmetry). ADR-0047
chose one treatment for the majority of the picture.

ADR-0047 declined to offer the choice per preset, on one stated ground: "not offered as a
per-preset mode because two address modes double the stage's pipelines against the documented
WARP pipeline-count sensitivity." That reasoning was about *address modes* — a sampler property,
fixed at pipeline creation. It does not cover the treatments that actually matter here. Three of
the four candidates are pure functions of the radius, differing only in how the shader maps `r`
to a sample radius and weight, which is a **uniform branch inside one fragment shader**: one
pipeline, one bind-group layout, one pass. The objection dissolves for everything except literal
address-mode changes.

ADR-0047's own Outcome already recorded that a fourth treatment (`vignette`) is "the cleanest of
the four on a border-filling field and the most costly on a figure". A treatment that is best on
fields and worst on figures, in an engine whose thirteen fold-binding presets are both, is the
classic argument for a choice rather than a default.

## Decision

We will make the fold's out-of-disc behaviour a **per-preset choice**: one new bindable named
param, `kaleido_edge`, selects a treatment from a small fixed roster, and every treatment lives
in the **same** shader behind a uniform branch on that selector — one pipeline, one bind-group
layout, one sample per pixel except where an address mode is genuinely involved.

`kaleido_edge` is **stepped**: the CPU side clamps it into the roster's range, rounds it to an
integer, and falls back to the default on a non-finite value — the same treatment `fold_order`
gives `kaleido_order`, and for the same reason. A selector is a discrete identity, and
`[smoothing]` plus preset dissolves both sweep a param continuously through the values between
two settings; rounding CPU-side keeps the shader's precondition visible in Rust and makes the
in-between values a snap rather than an undefined fifth treatment.

**The falloff-disc remains the default**, so ADR-0047 is supplemented rather than superseded and
no preset, fixture or golden baseline moves until one opts in.

Which treatments ship is decided by **rendered confirmation in the running app**, not here. Plan
0055 implements the full candidate roster behind the selector first, and a `human` phase A/Bs
them live — in motion, on a centred figure and a border-filling field, over a **lit** backdrop,
at 16:9 and at a non-16:9 window — then the losers are deleted. This ADR fixes the *mechanism*
and the *default*; the roster is its outcome. The candidates entering that A/B are:

| `kaleido_edge` | Name | What it does past `r_max` |
|---|---|---|
| 0 | `falloff` | Today's shipped behaviour: clamp the sample radius, fade out over `FALLOFF_BAND`. Rays, fading; corners empty. |
| 1 | `vignette` | The fade moves **inside** the disc, so nothing beyond `r_max` is ever painted and no ray is drawn. Crops a rim of real content. |
| 2 | `mirror` | **Reflect** the radius instead of clamping it — a triangle wave in `r` — so the region past the disc is a mirrored continuation of its interior. Fills the frame with related content, continuous at `r_max`, samples nothing outside `[0,1]`. |
| 3 | `tile` | ADR-0047's Alternative A in its mirrored form: leave the radius alone and let a `MirrorRepeat` sampler define the out-of-range reads. The only candidate needing a second sampler. |
| 4 | `squash` | Compress the radius asymptotically into the disc (`r_max * tanh(r / r_max)`): 1:1 at the fold axis, approaching `r_max` at the corners. No crop, no ray, at the cost of bending straight geometry near the frame edge. |

The `mirror` candidate is new — it was not in ADR-0047's alternatives and was not among the four
Plan 0045 Phase 1 rendered. It is named here because it is the one candidate that addresses both
rejections at once (no ray, because the corner content is real; no crop, because the corners are
filled) and because it is what a physical kaleidoscope does: the mirrors continue past the
aperture.

## Consequences

### Positive

- A field preset and a figure preset stop having to share one answer to a question whose right
  answer demonstrably differs between them. That is the whole content of the user's rejection.
- Nothing moves on adoption. The default is today's behaviour, so every shipped preset, every
  golden baseline and `composite_kaleido.png` are byte-identical until an author opts in — which
  is also what makes the diff reviewable.
- The pipeline count is unchanged for four of the five candidates, so the WARP sensitivity
  documented in `render/post.rs` is not disturbed by the mechanism itself.
- `mirror` and `squash` sample strictly inside `[0,1]` by construction, so they keep ADR-0047's
  real guarantee — the smear of design-backlog 0010 comes from *reconstructing a coordinate
  outside the source*, and neither does.

### Negative

- **One more named param in a vocabulary the content lane must learn**, and a stepped one, which
  is the second such param on this stage. `presets/README.md` owes it a row and the stepped note.
- **A treatment change across a preset dissolve snaps rather than blends.** Two presets with
  different `kaleido_edge` values cross-fade their *frames* correctly (the dissolve is a blend of
  two rendered outputs), but a single preset easing the selector jumps at the midpoint. Documented,
  not fixed — the same shape as `kaleido_order`.
- **If `tile` survives the A/B it costs a second sampler in the kaleidoscope's bind group**, which
  changes that layout's shape and therefore interacts with
  [ADR-0058](0058-bind-group-layout-collisions-carry-evidence.md)'s allowlist. Whichever of Plan
  0053 and Plan 0055 lands second inherits the other's list. The change is more likely to *remove*
  a collision than create one — a two-sampler layout is more distinctive than the `[Uniform,
  Texture, Sampler]` shape several stages share — but it must be re-derived, not assumed.
- **The existing disc guard becomes treatment-scoped.** `core/tests/kaleidoscope.rs`'s
  `the_fold_paints_nothing_outside_its_disc` asserts an invariant that three of the five
  candidates deliberately break. It stays true of the default and must say so in its name or its
  docs, or a later reader will read it as a property of the fold rather than of one treatment.
- **The roster is a closed set.** An author cannot compose a new edge behaviour from the grammar;
  adding one is engine work and another row in this table. That is deliberate — an author-defined
  per-pixel radius map is the grammar-to-WGSL translator ADR-0048 already declined.

## Alternatives considered

### Alternative A — one new default for everyone, no choice

Pick whichever treatment loses least across both cases and reship it; no new param, no authoring
decision, smallest surface. Rejected because it is what ADR-0047 did, and the evidence in
design-backlog 0037 is precisely that one treatment cannot serve both cases: the same `vignette`
that is cleanest on a border-filling field is the one that crops the corona `star_rosette` was
chosen for. A new default re-accepts one of the two rejections it exists to answer.

### Alternative B — a continuous scalar instead of a discrete roster

One bindable number spanning "crop hard" to "fill completely", which would be smoothable and
dissolve-friendly, unlike a stepped selector. Rejected because the candidates differ
**structurally**, not by degree: reflecting a radius, fading a weight, and changing a sampler's
address mode are not three points on one axis. The only genuinely continuous quantity in the
family is the falloff band's width, which is already a constant nobody has asked to move and
which was never what the complaint was about.

### Alternative C — a separate pipeline per treatment

The cost model ADR-0047 assumed when it declined a per-preset mode. Rejected because it is
unnecessary for four of the five candidates and it is the expensive shape: N pipelines built
lazily, N bind groups, and a multiplied exposure to the WARP hazard that Plan 0045 hit twice
while adding bloom. A uniform branch costs one dynamic branch per fragment in a stage that only
runs when a fold is active at all.

### Alternative D — the engine picks the treatment from the scene kind

Let a fullscreen field scene get the fill treatment and a centred figure get the falloff, with no
param at all. Rejected on the same grounds as ADR-0047's own Alternative D: it couples a
`PostStage` to scene semantics the chain deliberately does not see, and it is the engine guessing
at authorial intent. A preset that draws a centred figure over a field, or one that means its
field to be cropped, has no way to say so.

### Alternative E — leave it, and treat design-backlog 0037 as an accepted cost already recorded

ADR-0047 does name both consequences in its Negative section, so a reading exists in which
nothing is owed. Rejected because the user reviewed the shipped result in the configuration the
stills could not show and rejected it; "we wrote the cost down" is not the same as "the cost was
acceptable once seen". This is the bet not holding.

## Notes

**Confirmation protocol (design-by-concrete-examples), corrected by Plan 0045's failure.** The
user picks visual directions from rendered candidates — but sixteen stills at `bg_bright = 0`
confirmed a decision that two screenshots of the running app then reversed. So the A/B here runs
in the **running app**, in motion, over a lit backdrop, through the tonemap, on both a figure and
a field, at 16:9 and at a non-16:9 window. It needs no throwaway debug seam: since Plan 0015,
`LMV_PRESET_DIR` makes an edit to a version-controlled `presets/*.toml` live in about 150 ms, so
changing one integer in the preset file *is* the A/B.

**Stopping condition.** If the live A/B finds no candidate better than today's default on either
scene, Plan 0055 stops and routes back to `architect` rather than shipping a selector with one
useful value — the same shape as Plan 0045 Phase 2's stopping condition, which did not fire.

**This ADR does not reopen the fold's domain.** ADR-0047's core finding stands and is load-bearing
for every candidate here: reconstructing a sample coordinate outside the source and handing it to
`ClampToEdge` is the design-backlog 0010 defect, and no treatment in the roster does that.
`tile` comes closest — it lets the coordinate leave `[0,1]` — and is the one candidate whose
address mode is *defined* for that, which is exactly why it needs the second sampler rather than
the existing one.
