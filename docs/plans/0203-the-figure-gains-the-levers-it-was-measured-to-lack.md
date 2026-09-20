# 0203 — The figure gains the levers it was measured to lack

> **Status:** in-progress
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

**Lane:** `plan-0203-the-figure-gains-the-levers-it-was-measured-to-lack`, worktree
`C:\Users\Igor Konovalov\WORK\rlx-plan-0203`

| phase | owner | state | commit |
|---|---|---|---|
| 1 — The roster travels | dev | done | 56915750 + committed with this row |
| 2 — The star wobbles, and its scatter can be chosen | dev | not started | |
| 3 — Judge the floor by rendering | dev | not started | |
| 4 — The backdrop ramp converges | dev | not started | |

### Phase 1 — what the done-when measured

**The identity, at every integer index, byte for byte.** `shot` rendered ten frames at
128x128 over 60 frames — a `shape_field` preset and a `swarm` preset at each of the five whole
indices — first against this lane's sources and then against the same three files restored to
their pre-phase state (`git restore --source=56915750^`) and rebuilt. All ten PNG pairs compare
**byte-identical**, so the claim is measured on the rendered frame rather than inferred from the
CPU mirror.

**The fractional index draws a third figure.** The unit test prints coverage
(`max(0, 1 - d)^2`, what the shader emits) at each pair's midpoint against both endpoints, as a
fraction of the disc's mean coverage:

| travelling pair | from lower | from upper |
|---|---|---|
| disc -> ring | 1.110 | 0.936 |
| ring -> polygon | 1.006 | 1.067 |
| polygon -> star | 0.291 | 0.145 |
| star -> heart | 0.168 | 0.333 |

The smallest separation is `polygon -> star` at 0.145 from the star side, against the test's 0.02
floor.

**The shared chunk.** `marks::sdf_wgsl` is prepended by three scenes — `swarm`, `emitter` and
`shape_field` — and twenty shipped presets sit on those systems: `emitter_driftfield`,
`emitter_emberjet`, `emitter_heartfall`, `emitter_perseids`, `emitter_petalfall`,
`fragment_interferencemono` (whose second layer is a `shape_field`), `shape_aperture`,
`shape_contourmono`, `shape_facet`, `shape_heartmono`, `shape_lion`, `shape_maple`, `shape_pulse`,
`shape_ringmono`, `shape_strataheart`, `swarm_braid`, `swarm_drift`, `swarm_murmuration`,
`swarm_shatter` and `swarm_stipple`. Seventeen bind `shape` to a quoted whole constant; the other
three (`shape_lion`, `shape_maple`, `swarm_drift`) leave it at the default `0`. **None binds it to
an expression**, so every one of the twenty reaches the identity branch, and the ten renders above
cover the five values they use between them.

**One behaviour that does change where no preset is involved.** A `[smoothing]` ease or a preset
dissolve carrying `shape` from one whole setting to another now travels through the blend where it
used to step at the rounding boundary. No shipped preset can reach that path, and no baseline
renders one.

### Notes

- **Phase 1's code did not land in this session's commit.** A prior session was cut off by the
  usage window mid-phase and its edits were committed unfinished as `56915750`
  (`wip(marks): phase 1 as the usage limit left it, checks re-run`); this session verified that
  tree, took the two measurements the phase owed, and committed the log row. The phase's row names
  both commits.

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
