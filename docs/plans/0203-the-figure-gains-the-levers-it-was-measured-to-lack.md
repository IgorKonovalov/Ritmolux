# 0203 — The figure gains the levers it was measured to lack

> **Status:** approved
> **Created:** 2026-09-19
> **Approved:** 2026-09-19 (user)
> **Owner skill(s):** dev
> **Related ADRs:** [0226](../adrs/0226-the-mark-roster-travels-by-blending-its-fields-and-an-integer-index-is-an-identity.md)
> (proposed), [0225](../adrs/0225-the-backdrop-ramp-gets-an-angular-coordinate-and-the-floor-stays-out-of-the-chain.md)
> (proposed), [0084](../adrs/0084-a-particle-marks-silhouette-is-a-signed-distance-function.md),
> [0105](../adrs/0105-the-mark-roster-becomes-a-fullscreen-distance-field.md),
> [0111](../adrs/0111-the-shape-field-gains-a-scaled-copy-coordinate.md),
> [0090](../adrs/0090-a-preset-composes-two-scene-layers.md)
> **Closes:** design-backlog 0095, 0100, 0101

## TL;DR

Three look gates each passed a figure and named one thing it could not do: a roster that cuts
between silhouettes instead of travelling, a star whose only irregularity is spike length when
"hand-drawn" means edge wobble, and a backdrop that makes parallel stripes when the reference has a
fan. Each is a lever, each defaults to an exact identity, and each was filed with its constraint
already measured. The first visible behaviour is a figure that travels from a heart to a star across
a phrase.

## Context & problem

**The roster cuts.** `mark_shape` rounds, so a fractional index selects no arm and an eased binding
steps. Within an arm, morphing already works — `star_valley`, `star_curve` and `star_jitter` are
clamp-only — which is the answer to the question as asked and not to the one that was meant: the
user's *"can we morph the shape with music"*, at `presets/shape_facet.toml`'s authoring, asked for
travel between figures (backlog 0101).

**"Hand-drawn" is the wrong quantity.** Plan 0091 Phase 6 passed all five star silhouettes with one
soft exception — the hand-drawn six-pointer *"maybe"* reads as hand-drawn. `star_jitter` varies each
spike's tip radius, one scalar per spike from an index hash; a hand-drawn figure's spikes are
roughly the right length and its **edges wander**. Two smaller consequences of the same shape: the
jitter cannot be re-scattered without changing its amount, because the pattern is a pure function of
the spike index; and a jittered star's exterior is already the roster's least accurate field, up to
0.54 out, so anything built here is judged against that number rather than assumed free (backlog
0100).

**The backdrop cannot converge.** The ramp is swept and repeat-addressed, so parallel stripes ship;
an angular coordinate about a movable point turns the same stripes into a fan. Folding is not
available — the backdrop sits outside the chain's input — so it is a backdrop mode rather than a
reuse (backlog 0095).

## Decision

Per [ADR-0226](../adrs/0226-the-mark-roster-travels-by-blending-its-fields-and-an-integer-index-is-an-identity.md)
the roster travels by blending the neighbouring arms' distance fields and their boundary radii, with
an integer index an exact identity. Per
[ADR-0225](../adrs/0225-the-backdrop-ramp-gets-an-angular-coordinate-and-the-floor-stays-out-of-the-chain.md)
the backdrop gets an angular coordinate and the floor stays outside the chain — and because backlog
0095 instructs that the two routes be judged **by rendering rather than by argument**, Phase 3
renders both before Phase 4 builds either. The star gets a seed and an edge-displacement term, sized
against the measured exterior accuracy rather than against taste. Every default here is an
arithmetic identity, which is what lets three look-gate residues land in one plan without moving a
single shipped picture.

## Implementation phases

### Phase 1 — The roster travels
- **Owner skill:** dev
- **What:** a fractional `mark_shape` blends the two neighbouring arms' distance fields and their
  `r_boundary` values; an integer index produces exactly what the rounded selector produces today,
  on both the shape-field and the particle paths. The parameter's meaning is regenerated through
  `ParamSpec`, and every place that reads the field as a metric distance says what it assumes
  mid-travel.
- **Files touched:** `core/src/render/scenes/marks.rs`, the shape-field and particle shaders that
  read `mark_distance`, `presets/README.md` (generated), `presets/schema/*.schema.json` (generated),
  `docs/specs/player-schema.json` (generated), `docs/presets.md`
- **Done when:** at every integer index the rendered frame is **byte-identical** to the same preset
  before this phase, on a shape-field preset and on a `swarm`/`emitter` preset — so the whole golden
  roster and every card are unmoved without re-blessing; a fractional index draws a silhouette that
  is neither neighbour, shown by a measurement that separates it from both; and the shared-chunk
  consequence is discharged in words: which particle presets read this chunk, and that each is
  unchanged.

### Phase 2 — The star wobbles, and its scatter can be chosen
- **Owner skill:** dev
- **What:** a `star_seed` input re-scatters the existing tip jitter without changing its amount,
  keeping the integer-hash determinism the arm was built on (no `sin`-based hash — it differs
  between GPUs). An edge-displacement term wobbles the line between tip and valley along its length,
  evaluated on the folded edge parameter the arm already computes, with amplitude and frequency
  inputs and zero amplitude the default.
- **Files touched:** `core/src/render/scenes/marks.rs`, `core/src/render/scenes/marks` shaders,
  `presets/README.md` (generated), `presets/schema/*.schema.json` (generated),
  `docs/specs/player-schema.json` (generated), `docs/presets.md`
- **Done when:** two `star_seed` values produce different arrangements with the same measured jitter
  amount; the wobble changes the edge and not the tip radius, shown by measuring both separately; the
  same figure renders identically on two runs and on two adapters, which is the determinism the hash
  choice exists for; and **the exterior field accuracy is re-measured** against Plan 0091 Phase 5's
  0.54 and reported — if the wobble makes it materially worse, say by how much and stop rather than
  tuning the number, because a worse field is a real cost to whatever reads it.

### Phase 3 — Judge the floor by rendering
- **Owner skill:** dev
- **What:** render the reference collage's floor both ways — as a backdrop fan (the coordinate mode
  ADR-0225 decides) and as a ground scene in the preset's layer slot — and put the two pictures
  beside each other, as backlog 0095 and Plan 0091's own Phase 7 done-when both instruct.
- **Files touched:** none necessarily; the renders and the reading go in the implementation log, and
  a throwaway preset under `docs/examples/` only if it is worth keeping.
- **Done when:** both renders exist and the log states which reads as the reference's floor and
  which does not, with the layer-slot cost named; and the log says plainly whether ADR-0225 survives
  the comparison — if the chain route wins the picture, the ADR is superseded rather than
  implemented and Phase 4 does not run.

### Phase 4 — The backdrop ramp converges
- **Owner skill:** dev
- **What:** a coordinate mode on the backdrop ramp plus a movable centre, so the existing swept,
  repeat-addressed stripes become a fan about a vanishing point. Every default an arithmetic
  identity.
- **Files touched:** `core/src/render/background.rs`, `presets/README.md` (generated),
  `presets/schema/*.schema.json` (generated), `docs/specs/player-schema.json` (generated),
  `docs/preset-palettes.md`
- **Done when:** the default renders every existing backdrop byte-identically, so no baseline moves;
  the angular mode produces stripes that converge on the named centre, measured as a decreasing
  spacing along a radius rather than judged by eye; and the documentation states ADR-0225's limit —
  that this buys the floor and not the collage, because a figure over a lit backdrop cannot be
  darkened into it.

## Risks & open questions

- **Phase 1's intermediate is not a shape anyone designed** (ADR-0226's first Negative). The
  done-when asserts identity at the integers and difference in between; whether a given pair looks
  good is a content judgement, and the answer may be that some pairings are simply avoided.
- **Phase 3 can falsify ADR-0225.** That is the phase's purpose, and it is why the ADR's own Notes
  say it is superseded rather than implemented if the rendering disagrees.
- **Phase 2's wobble may cost field accuracy**, and the done-when requires the number rather than an
  impression. A materially worse exterior is a stop, not a tuning problem.
- **Every phase here regenerates a parameter surface**, which a conductor session cannot do until
  [Plan 0197](done/0197-the-conductor-becomes-operable.md) Phase 4 lands (backlog 0250). Run this plan
  after 0197.
- **This is the weakest-justified of this round's plans, and deliberately so.** Each entry says in
  its own words to take it *when someone wants the thing* — a rough figure, a vanishing point — and
  only the roster morph has a want on record. Every default is an identity, so the cost of building
  ahead of the want is the lane's time and nothing else.

## What this plan does NOT do

- **It does not take backlog 0092 (lighting), and must not.** That entry's trigger fired and
  resolved negatively on 2026-08-16, and its own boldfaced instruction is that a future lighting
  plan needs a fresh want rather than that one. Nothing since has supplied one.
- **It does not take backlog 0069's surviving half.** What survives that entry is that nothing in
  this engine decides what is in front of what — an occlusion and ordering question in an additive
  pipeline, which is an architecture interview of its own and not a lever.
- It does not move the backdrop into the chain's input, and it does not spend a preset's layer slot
  on anything (ADR-0090). A world that needs a darkenable ground draws one, deliberately.
- It does not author the presets that would use these levers. That is `preset-author`'s, after this
  lands, and it is what will say whether any of the three was worth building.

## Implementation log

**Lane:** _(to be filled by `dev`)_

| phase | owner | state | commit |
|---|---|---|---|
| 1 — The roster travels | dev | not started | |
| 2 — The star wobbles, and its scatter can be chosen | dev | not started | |
| 3 — Judge the floor by rendering | dev | not started | |
| 4 — The backdrop ramp converges | dev | not started | |

### Notes

### Close triggers

- **`presets/` touched:**
- **Plan header `Closes:`** design-backlog 0095, 0100, 0101
- **What shipped:**
- **Operator docs touched:**
- **Backlog probes (`node scripts/check-backlog-claims.mjs`):**
- **Full suite:**
- **Outstanding `human` phases:**

## Followups (after this lands)

- A `preset-author` look for each lever, which is the only thing that can say whether it earned its
  place.
