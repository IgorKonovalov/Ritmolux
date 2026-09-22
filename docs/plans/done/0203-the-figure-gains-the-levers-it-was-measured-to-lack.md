# 0203 — The figure gains the levers it was measured to lack

> **Status:** done — closed 2026-09-20. Phases 1-4 landed as `56915750` + `7f8ca5dc`, `b8f00cd8`,
> `0dd39295` and `869fbbde`; round 1's major was repaired in `5174d516` and round 2's two minors in
> `6e6dbc08`. Close review round 2: **no blockers, no majors**, three minors and one nit, two of
> them repaired here. Verified: the full suite green on the tagged tree through the conductor's
> suite ledger, rustdoc clean under `-D warnings`, the roster travel reachable from a preset
> binding, every whole index an exact identity, and the backdrop's default byte-identical.
> **Created:** 2026-09-19
> **Approved:** 2026-09-19 (user)
> **Owner skill(s):** dev
> **Related ADRs:** [0226](../../adrs/0226-the-mark-roster-travels-by-blending-its-fields-and-an-integer-index-is-an-identity.md)
> (proposed), [0225](../../adrs/0225-the-backdrop-ramp-gets-an-angular-coordinate-and-the-floor-stays-out-of-the-chain.md)
> (proposed), [0084](../../adrs/0084-a-particle-marks-silhouette-is-a-signed-distance-function.md),
> [0105](../../adrs/0105-the-mark-roster-becomes-a-fullscreen-distance-field.md),
> [0111](../../adrs/0111-the-shape-field-gains-a-scaled-copy-coordinate.md),
> [0090](../../adrs/0090-a-preset-composes-two-scene-layers.md)
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

Per [ADR-0226](../../adrs/0226-the-mark-roster-travels-by-blending-its-fields-and-an-integer-index-is-an-identity.md)
the roster travels by blending the neighbouring arms' distance fields and their boundary radii, with
an integer index an exact identity. Per
[ADR-0225](../../adrs/0225-the-backdrop-ramp-gets-an-angular-coordinate-and-the-floor-stays-out-of-the-chain.md)
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
  [Plan 0197](0197-the-conductor-becomes-operable.md) Phase 4 lands (backlog 0250). Run this plan
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
| 1 — The roster travels | dev | done | 56915750 + 7f8ca5dc |
| 2 — The star wobbles, and its scatter can be chosen | dev | done | b8f00cd8 |
| 3 — Judge the floor by rendering | dev | done | 0dd39295 |
| 4 — The backdrop ramp converges | dev | done | committed with this row |

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

### Phase 2 — what the done-when measured

Three parameters landed: `star_seed` (0..255, rounded), `star_wobble` (0..1, amplitude) and
`star_wobble_freq` (0.5..2.5). All three default to an identity and all three went into padding the
two uniforms already carried, so no bind-group layout changed.

**Two seeds, same amount.** Per-spike tip radii at sixteen seeds, 7 points, `star_jitter = 0.5`
(band 1.0): no seed spans more than the band, none collapses, and the **mean range is 0.7364**
against the 0.7500 a uniform draw of seven predicts. No seed's arrangement equals another's under
any of the seven rotations — the rotation check is what the multiplicative stride buys, since a seed
merely *added* to the hash's input would produce the same star turned by one spike. Seed 0's hash is
bit-identical to the unseeded one.

**The wobble moves the edge, not the tip**, measured separately on the arm's own outline:

| wobble | tip moved by | edge moved by |
|---|---|---|
| amp 0.25, freq 1.0 | 0.0000002 | 0.04144 |
| amp 0.5, freq 1.0 | 0.0000004 | 0.08282 |
| amp 1.0, freq 1.0 | 0.0000009 | 0.16543 |
| amp 1.0, freq 0.5 | 0.0000005 | 0.09127 |
| amp 1.0, freq 2.5 | 0.0000011 | 0.13486 |

Five orders apart, and linear in the amplitude at fixed frequency. The tip is a tolerance rather
than a bit-equality: the probe names the angle `seg * spike` and the arm recovers it through
`atan2`, which does not round-trip it — spike 4 of 7 lands about `1e-7` off its own axis, where the
first sub-segment is wobbled. The window is algebraically exactly 0 at both ends.

**Determinism, rendered on this machine's three adapters** at 160x160, on a figure carrying seed 11,
`star_jitter` 0.5 and `star_wobble` 1.0. Two runs on the software adapter are **byte-identical**.
Across adapters the spike order is the same list `[4, 6, 3, 0, 2, 1, 5]` on all three:

| adapter | frame_diff against software |
|---|---|
| AMD Radeon(TM) Graphics | 0.00018 |
| NVIDIA GeForce RTX 3080 Laptop GPU | 0.00021 |
| Microsoft Basic Render Driver | 0.00000 |

**The exterior field accuracy, re-measured against Plan 0091 Phase 5's 0.540.** Worst exterior error
in sprite-local units, ground truth sampled 16x finer than the shader:

| configuration | 5 points | 7 points |
|---|---|---|
| wobble 0.5, freq 1.0 | 0.06259 | 0.04667 |
| wobble 1.0, freq 1.0 | 0.09332 | 0.07480 |
| wobble 1.0, freq 2.5 | 0.07144 | 0.07032 |
| wobble 1.0, freq 0.5 | 0.12123 | 0.09408 |
| wobble 1.0, seed 9 | 0.09955 | 0.09006 |
| wobble 1.0 + jitter 0.4 | 0.24931 | 0.54985 |

**`star_wobble` alone costs at most 0.12123, about a quarter of the jitter's 0.540**, so the field is
not materially worse and the phase did not hit its stop condition. On top of a `star_jitter` of 0.4
the worst reading is 0.54985 against that configuration's own 0.54019 without the wobble — the
jitter dominates and the wobble adds 0.0097. Interior error under the wobble alone is 0.004 to 0.053.

### Phase 3 — the two floors, rendered

Three throwaway presets at 320x200, all sharing one construction so the only
variable is **where the floor lives**. None was kept: they are minimal
demonstrations of a decision, not teaching files, and `docs/examples/` is
referenced by the guide rather than a scrapbook.

**Route B — the floor as a scene in the chain.** A flat white ground as the base
scene, the figure multiplied over it through the one `[layer]` slot. It draws the
reference's treatment exactly: a flat red heart on white paper, the figure a
uniform **97.3–99.0** luma down its middle against a ground of **230.0–232.0**.
Dark-on-light, 133 luma points of separation, and the figure is flat because
nothing is added to it.

**Route A — the floor on the backdrop.** The same preset, with the striped floor
moved to `[background]` and `occlude = 0`. The backdrop is added after the
chain's junction, so the floor's light lands on the figure as well: the heart
runs **109.7–210.1** down the same column — striped through, spanning a hundred
luma points — while the floor runs 209.1–245.0. **The figure's brightest part
(210.1) is brighter than the floor's darker stripes (209.1)**, so figure and
floor are no longer separable by tone at all. This is Plan 0091 Phase 1's
measurement taken from the other side, and it is exactly the limit
[ADR-0225](../../adrs/0225-the-backdrop-ramp-gets-an-angular-coordinate-and-the-floor-stays-out-of-the-chain.md)
states in its own first Negative.

**Route B's other half — the fan in the layer slot instead of the figure.** A
white ground with a twelve-pointed star's scaled-copy coordinate multiplied over
it, centred low. It converges on the named point and it is **not the reference's
floor**: the scaled-copy coordinate draws nested chevrons rather than radiating
lines, and at the band counts that make the convergence legible it aliases
heavily. No scene tried here draws a fan of straight lines from a point.

### Does ADR-0225 survive? Yes, and Phase 4 runs.

The plan's stop condition is *"if the chain route wins the picture"*. It does not,
and the reason is structural rather than a limit of the presets above: this engine
draws additive light, so dark-on-light comes only from a multiply layer
(ADR-0106), a preset has exactly **one** layer slot (ADR-0090), and **the fan and
the figure cannot both be the thing that is darkened**. Route B therefore buys the
figure's tone *or* a dark floor, never the collage. Route A buys a floor at no slot
cost and can never darken a figure into it.

So the two routes buy different halves and neither buys the whole reference image
— which is the sentence ADR-0225 already wrote as *"the cheap route buys the
picture's floor and not the picture"*. What the rendering adds is that **the
expensive route does not buy the picture either**, so the slot is not a price that
purchases the collage. The angular coordinate remains the only thing on the table
that produces a converging floor, and it costs nothing.

**The layer-slot cost, named:** a preset that draws its ground as a chain scene
spends the one slot ADR-0090 gives it, on a floor, and then has no layer for the
figure, the second colour, or anything else — and it must still pay for the ground
as a full scene evaluation per frame rather than as the backdrop pass that already
runs.

### Phase 4 — what the done-when measured

Three parameters on the backdrop ramp: `bg_coord_mode` (0 straight, 1 angular, rounded),
`bg_center_x` and `bg_center_y` (the vanishing point in NDC, clamped to ±2). They went into a sixth
`vec4` row on the backdrop's uniform, whose `min_binding_size` is computed from the struct, so the
layout stays the one shape ADR-0058 requires it to keep.

**The default renders byte-identically.** Two backdrop presets — one exercising the ramp
(`bg_hue_span`, `bg_angle`, `bg_shade`/`bg_shade_end`, `bg_ramp_gamma`, `bg_vignette`) and one the
curved band as well — rendered at 128x128 over 60 frames against this tree and against
`background.rs` restored to its pre-phase state and rebuilt: **byte-identical**, as were the ten
Phase 1 probes re-run alongside them. The golden roster passes unmoved, and
`the_angular_ramps_centre_is_inert_until_its_mode_is_bound` holds the stronger claim that naming a
vanishing point while the mode rests at 0 moves **zero** levels.

**The angular mode converges on its centre, measured.** Centre at `(0, -1.2)`, just below the
frame; three rows read far-to-near:

| row | radius | boundaries crossed | mean gap | straight mode's boundaries |
|---|---|---|---|---|
| 16 | 1.684 | 4 | 14.67 px | 0 |
| 32 | 1.184 | 5 | 11.25 px | 0 |
| 61 | 0.278 | 10 | 5.00 px | 0 |

The spacing falls with the radius and the count rises, which is the pair: a gap can shrink because
the frame ran out of room, and a rising count is what says the bands genuinely crowded toward the
point. The straight mode at the same angle puts no boundary on a horizontal row at any height,
which is the control.

**The documentation states ADR-0225's limit.** Both `presets/README.md`'s new ramp section and
`docs/preset-palettes.md` open on it: the backdrop is added after the chain, so a figure over a lit
fan can only be brighter than it, and a dark-on-light collage needs a multiply layer over a
chain-drawn ground — which then spends the slot. Phase 3's two luma measurements are quoted in
`preset-palettes.md` so a reader gets the number rather than the assertion.

### Notes

- **Phase 4 touched two files its `Files touched` list does not name**, both for the same reason
  Phase 2 did: `core/tests/suite/backdrop_ramp.rs` carries the measurements, and
  `core/tests/suite/preset.rs` holds two deliberate rosters a new parameter must join — the
  per-file `set_param` list and `STRUCTURAL`, the latter because `bg_coord_mode` rounds.
- **A vanishing point is not enough on its own to draw the reference's floor, and the reason is
  `bg_hue_span`.** An angular sweep spends the span on a whole turn while the frame sees perhaps a
  fifth of one, and the span is documented at `±0.5` — so the stripe count has to come from the
  palette repeating its tones a dozen or more times. That works, and the probe's palette does it,
  but it is a real authoring cost and it is stated in both documents rather than left to be
  discovered. **Widening `bg_hue_span`'s range would remove it** — nothing clamps the value today,
  so the change would be to a declared range and a schema bound and would move no pixel. It was not
  taken: the range is an existing parameter's, ADR-0225 names only a mode and a centre, and a lane
  widening a published range on its own judgement is the kind of drift the plan/ADR split exists to
  prevent. Routed here as the one thing an author will hit first.
- **`bg_shade` and `bg_shade_end` should be equal under the angular mode.** The coordinate wraps
  where the straight one clamps, so the brightness ramp jumps across the seam ray. Documented at
  both surfaces; not enforced, because a deliberate hard edge along one ray is a legitimate look.

- **Phase 2 touched three files its `Files touched` list does not name**, and could not avoid it:
  `core/src/render/scenes/swarm.rs`, `emitter.rs` and `shape_field.rs`. A shared mark parameter is
  declared in `marks::PARAMS` but is *carried* by each scene — its field, its default, its `reset`,
  its `set_param` arm, its uniform write and its varying — and `emitter.rs`'s
  `both_particle_scenes_carry_the_same_shape_vocabulary` fails if the three rosters disagree. So
  `star_seed` as an *input*, which the phase's `What` requires, is not reachable from `marks.rs`
  alone. `core/tests/suite/preset.rs` is a fourth: its `STRUCTURAL` roster is a deliberate allowlist
  that a newly-rounding parameter must join, and it failed until `star_seed` was added to it.
- **`docs/presets.md` is in the list and was not touched.** It is the expression-language reference
  and carries no star-arm material; the star's prose lives in `presets/README.md`, which was edited
  by hand as well as regenerated. Nothing in `docs/presets.md` became false.
- **The wobble is spelled inline at both of its call sites rather than called as a function**, which
  is the opposite of `marks.rs`'s habit. Written as a function called inside the sub-segment loop —
  the first user function in that loop — it stopped the DX12 backend producing a working
  `shape_field` pipeline: all 26 of that scene's GPU tests and the golden roster came back on a lost
  device, including presets that never reach the star arm, while every hardware render and every
  `swarm`/`emitter` test passed. Inlining it made all of them green again. The constraint is
  recorded at the code.
- **The reference the curved arm normalizes by stays the unwobbled, unjittered figure's**, so the
  wobble inherits the interior inexactness `star_jitter` already has rather than adding a new kind.
- **`star_jitter`'s doc sentence still says "so the star reads as hand-drawn".** It is now the
  weaker of the two roughnesses on that claim, and `presets/README.md`'s prose says so, but the
  `ParamSpec` sentence was left as it is because changing it moves a generated surface for a
  wording rather than for a fact.

- **Phase 1's code did not land in this session's commit.** A prior session was cut off by the
  usage window mid-phase and its edits were committed unfinished as `56915750`
  (`wip(marks): phase 1 as the usage limit left it, checks re-run`); this session verified that
  tree, took the two measurements the phase owed, and committed the log row. The phase's row names
  both commits.

- **Round 1, finding 0 (major) — `shape` reached the scene rounded, so the travel was unreachable
  from a preset.** `SHAPE` is now declared `ParamKind::Modal`, the three `("*", "shape")` rows left
  `STRUCTURAL` in `core/tests/suite/preset.rs`, the four generated surfaces were regenerated, and
  `an_eased_shape_sweep_lands_between_the_arms` drives a one-pole ease through `SHAPE.kind.quantize`
  as well as `mark_shape` — the route a binding actually takes — with `POINTS` on the same route as
  the control. Fixed in the commit carrying this line.

### Close triggers

- **`presets/` touched:** yes, but **no `.toml` preset**. `presets/README.md` (its generated
  parameter block, plus three hand-written sections), `presets/preset.schema.json` and all fourteen
  `presets/schema/*.schema.json` — every one of them a generated file or a documentation edit. The
  shipped preset set is unchanged, and nothing here is a content tune.
- **Plan header `Closes:`** design-backlog 0095, 0100, 0101
- **What shipped:** a **feature**, in three parts. `mark_shape` became a position on the roster that
  blends the two arms it lies between; the `star` arm gained `star_seed`, `star_wobble` and
  `star_wobble_freq`; the backdrop ramp gained `bg_coord_mode`, `bg_center_x` and `bg_center_y`.
  Every one of the six new parameters defaults to an arithmetic identity, and `mark_shape`'s change
  is an identity at every whole index.
- **Operator docs touched:** `docs/preset-palettes.md` (a new backdrop section),
  `presets/README.md` (the star arm's table and essay, and a new ramp section) and `docs/presets.md`
  was touched in Phase 1 only. No file under `docs/` that an operator reads for running the app
  moved — no flags, no keys, no config keys.
- **Backlog probes (`node scripts/check-backlog-claims.mjs`):** exit 0 — *"backlog claims: OK — 43
  stated reductions still hold across all 20 live entries (4 unprobeable)"*, with 29 probed paths
  reported as moved since their entries were last read (an advisory, not a failure).
- **Full suite:** owed to the conductor's pre-review gate (ADR-0207). What this session ran instead,
  all green on the same tree: `cargo nextest run --workspace -P fast` — **1768 passed, 317 skipped**
  — plus the deferred `golden` suite (3 passed), run per phase because every phase here changes what
  it measures, and `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -D warnings`
  clean.
- **Outstanding `human` phases:** none. The plan declares no `human` phase.

## Close review

A conductor-run close has no reader in the room (ADR-0205), so this section is the evidence of what
was checked. Round 2's review follows in full; the round-1 findings a fix round resolved are listed
under it.

### Round 2 — architect Mode 4, conductor mode, fresh session

> **Lane:** `C:\Users\Igor Konovalov\WORK\rlx-plan-0203`, branch
> `plan-0203-the-figure-gains-the-levers-it-was-measured-to-lack`
> **Range reviewed:** `main...HEAD` — `56915750`, `7f8ca5dc`, `b8f00cd8`, `0dd39295`, `869fbbde`,
> `5174d516` (round 1's fix), `27fdb785`, and the merge `f2bf1ddc`
> **Date:** 2026-09-20

#### Verdict

**No blockers, no majors. Three minors and one nit. The close proceeds.**

Round 1's major is genuinely discharged rather than papered over: `SHAPE` is `ParamKind::Modal`, the
three `("*", "shape")` rows left `STRUCTURAL`, all four generated surfaces were regenerated, and the
new assertion drives the *whole* route a binding takes — `SHAPE.kind.quantize` **and** `mark_shape`
— with `POINTS` on the same route as a control, which is precisely the test whose absence let the
defect through. The plan's headline capability now works from a preset.

What this round adds is one class round 1 did not reach: the change that makes the travel work left
**five comments across the three mark scenes still describing the rounded selector**, two of them
asserting the exact behaviour ADR-0226 removed. Repaired here. The emitter's glint branch and the
`background.rs` clamp constant are round 1's findings re-verified and still open, and the
implementation log is still larger than the contract it reports against.

#### Evidence run

- **Full suite.** `node tools\conductor\with-lock.mjs suite -- cargo nextest run --workspace`
  printed the ledger record rather than re-running (ADR-0207):
  `with-lock: skipped cargo nextest run --workspace: tree f708024 is green in the suite ledger, run
  by gate 0203-pre-review at 2026-09-20T15:05:44.564Z: 1774 tests run: 1774 passed (33 slow), 7
  skipped`. That record is this lens's full-suite evidence, written by the process that saw the exit
  code. The count is 304 below round 1's 2078 because the merge `f2bf1ddc` brought in Plan 0199,
  whose whole subject is cutting the gate's cost; nothing in this lane removed a test.
- **Rustdoc.** `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` — clean.
- `cargo fmt --all --check` clean, `node scripts/check-comment-hygiene.mjs` clean.
- The log's `### Close triggers` block is complete and its `Full suite:` bullet correctly cites the
  conductor's `pre-review` gate, which is the right answer in conductor mode.

**Two commits on the branch were written by an earlier round-2 attempt that did not finish**:
`27fdb785` (a prose repair, its message already marked `Close review round 2, minor`) and the merge
`f2bf1ddc`. Neither is an implementer's work, so this review takes the tip as it stands and grades
it whole; `27fdb785`'s content was checked against the code and is correct, and its own slip is
finding 2 below.

#### Lens 1 — alignment with the plan and the ADRs

All four phases landed, each with a single in-vocabulary `**Owner skill:** dev` tag. The
phase-to-commit table is honest, including Phase 1's unusual two-commit row.

I opened the fix's test rather than grading its commit message.
`an_eased_shape_sweep_lands_between_the_arms` (`core/src/render/scenes/marks/tests.rs:758`) is the
right test and not a restatement of the one that passed:

- it builds a genuinely continuous one-pole sweep and **asserts the sweep is continuous first**
  (`distinct_raw.len() > 100`), so the test cannot pass vacuously on a degenerate input;
- it maps each sample through `mark_shape(SHAPE.kind.quantize(v))` — both stages, in the order the
  binding pipeline applies them — and asserts over a hundred samples land strictly between the two
  arms, with a failure message that names `Structural` as the thing that would produce the
  counterfactual;
- it asserts **nothing on the route collapses two positions into one** (`distinct_seen.len() ==
  distinct_raw.len()`), which is stronger than "some fractions survive";
- it re-checks the identity at all five whole indices *through the same two stages*, so the round-1
  fix cannot have bought the travel at the cost of the identity;
- and `POINTS` on the same route still yields exactly `[7, 8, 9]`, which is what proves the
  quantizer stage is live rather than inert.

The four generated surfaces followed: `presets/README.md` now prints `shape` under **Modal**, and
`presets/preset.schema.json`, `presets/schema/{swarm,emitter,shape_field}.schema.json` and
`docs/specs/player-schema.json` all moved with it. `core/tests/suite/preset.rs` carries a comment
naming ADR-0226 as why `shape` is the one name that ever left the `STRUCTURAL` roster — which is the
right record, because the roster's own stated rule is what convicted the old declaration.

Phase 3 remains a genuine rendering judgement rather than an argument, and Phase 4's
`the_angular_ramp_converges_on_its_centre_and_the_straight_one_does_not` asserts both halves the
done-when demands — a mean gap falling by at least 10 % per step **and** a strictly rising boundary
count — with a negative control at the same `bg_angle` that puts zero boundaries on the same rows.
Reading it again this round, the control is doing real work: it is what convicts a measurement
reading the palette or the frame instead of the coordinate.

#### Lens 2 — layering, coupling, real-time safety

Clean, and unchanged by the fix. No platform or audio-source type entered `core/`; nothing escaped
the wgpu layer; the C ABI and the control protocol are untouched; nothing was added to an audio
callback. The six new parameters travel the existing `ParamSpec` / `set_param` / uniform route, and
all three hand-drawn controls went into padding `QuadUniform` already carried, so no bind-group
layout moved (ADR-0058, stated at the code).

The fix widened nothing: `ParamKind::Modal` is an existing declaration value, and declaring a
parameter with it is the ordinary use of that seam rather than a widening of it.

#### Lens 3 — docs and release bookkeeping

Generated surfaces were regenerated, not hand-edited, both in the original phases and in the fix.
The hand-written sweeps landed where they were owed. `docs/presets.md` now states the travel with
the ADR citation inside a markdown link, which is what `check-reader-prose.mjs` wants.

**What this lens found is the other direction**: three source files still *document* the rounding
the plan removed, and one of them says in so many words that an eased `shape` "steps at the
midpoints" — the sentence ADR-0226 exists to falsify. See finding 1. Doc comments are the surface a
future reader of `swarm.rs` reaches for before `marks.rs`, so a stale one here is not cosmetic; it
is the instruction that would send the next change back to rounding.

`presets/README.md` counts three bullets above a list of four. See finding 2.

One thing checked and **not** raised as a finding because it predates this plan and this plan did
not touch it: `POINTS` declares `range: Some([3.0, 16.0])` while `mark_points` clamps to
`MIN_POINTS..=MAX_POINTS` = `3..=12`, and both `presets/README.md`'s generated row (`3` – `16`) and
its hand-written prose (`3` to `12`) are printed from the two different numbers on the same page.
That is a declared-range-versus-applied-clamp drift of the kind the plan's own Notes routed rather
than took for `bg_hue_span`; it wants a backlog entry, not a close-time edit.

**Version bump owed: `minor`** — a feature plan, six new parameters and a changed meaning for a
seventh.

#### Lens 4 — correctness, determinism, geometry

- **The blend's identity is exact at the integers, by arithmetic rather than by tolerance.**
  `mark_distance` and `mark_boundary_radius` both take `lo = floor(shape)`, `t = shape - lo`, and
  branch on `t == 0.0`. `shape` reaches the shader clamped and unrounded, so a preset's `"3"` is
  `3.0` exactly, `floor` is exact on it, and the subtraction is exactly zero. At the roster's top
  (`shape == MAX_SHAPE == 4.0`) `t` is 0 and no arm 5 is ever addressed — the clamp is what
  guarantees that, and it stays CPU-side where a NaN cannot reach WGSL's implementation-defined
  `clamp`.
- **`shape_touches_ring` is the right widening of the `== RING_SHAPE` test**, and the arithmetic
  checks out: it is true across the whole open span `(0, 2)` and false at `0`, `2`, `3`, `4`, so the
  acceptance and the refusal at every whole index are exactly where they were. The blend of two
  positive single-valued boundary radii is single-valued, so the `polygon`-through-`heart` travel is
  genuinely safe under `coord_mode = 1` — the fallback is not over-broad.
- **Determinism.** The seed strides through the hash's **input** with Knuth's 32-bit golden-ratio
  constant and integer arithmetic only; WGSL's `u32` wraps by specification and the Rust mirror uses
  `wrapping_mul`/`wrapping_add`, so the two agree. `star_seed` is rounded CPU-side inside `0..=255`,
  so `u32(seed)` in WGSL is exact rather than a truncation. Seed 0 multiplies to 0 and leaves the
  pre-existing arrangement bit-identical.
- **Numeric assertions.** The diff was re-grepped for literals inside `assert*`. The convergence
  test asserts a *direction* with a margin argued from the geometry plus a monotone count; the
  exterior-accuracy figures are `println!`ed and compared in prose against Plan 0091 Phase 5's own
  0.540 rather than pinned; the adapter probe asserts an *ordering* and reports `frame_diff` as a
  number rather than asserting one. No frozen measurement is asserted universally.
- **Aspect (ADR-0037).** `angle_pos` takes `aspect` from the uniform the backdrop is rendered with,
  and the centre is multiplied by the same aspect on both sides, so a wedge subtends an equal angle
  on screen. This is the one place the plan could have got it wrong and did not.
- **Panics.** No `unwrap`/`expect`/`panic!` entered a hot-path module, and no new module was added,
  so Plan 0002's guard set needs no extension.

#### Lens 5 — design integrity

Dependency direction, the three seams and SRP/OCP are intact. The `marks::PARAMS`-declares /
each-scene-carries split that forced Phase 2 to touch `swarm.rs`, `emitter.rs` and `shape_field.rs`
is a pre-existing shape of the code, correctly named in the log; it is a legitimate eventual backlog
entry and not a finding against this plan.

One seam question worth stating explicitly, because it is what finding 3 is about: ADR-0226's third
Positive says the particle path gets the travel "for free". That is true of `swarm`, whose fragment
shader calls `mark_distance` unconditionally, and **only partly true of `emitter`**, which keeps its
own anisotropic glint behind `if (in.shape < 0.5)`. That is a scene declining to participate in a
roster behaviour, which is a legitimate thing for a scene to do — what is missing is that it says so
nowhere.

#### Findings

**minor — five comments still describe a `shape` the engine no longer rounds.**
`core/src/render/scenes/swarm.rs:380`. The plan's central change is that `mark_shape` clamps and
does **not** quantize, so that a bound `shape` reaches the scene fractional. Five comments across
the three mark scenes were left describing the rounded selector, and two of them state the removed
behaviour outright: `swarm.rs:379-382` and `emitter.rs:1011-1014` both say *"Both are quantized on
the way to the uniform ... it just steps at the midpoints"*, which is what ADR-0226 falsified;
`shape_field.rs:238` maps the uniform slot as *"shape index (quantized CPU-side)"*;
`shape_field.rs:758-761` says *"`marks::mark_shape` / `mark_points` quantize on the way to the
uniform"*; and `shape_field.rs:989-990` justifies `applied_coord_mode`'s own rounding as
*"`marks::mark_shape`'s treatment for `marks::mark_shape`'s reason"*, citing as precedent a function
that now does the opposite. A doc comment on the struct field is what the next change to `swarm.rs`
reads, and this one instructs that reader that the value steps. **Repaired in `6e6dbc08`** — comment
text only, so it is on ADR-0209's closed list.

**minor — the travel essay counts three bullets and lists four.** `presets/README.md:1561`.
*"Three things come with that:"* stands above a list of four; the fourth (`coord_mode` falling back
across the whole `disc`-to-`polygon` span) was added by `27fdb785` without the count following it.
The bullet itself is correct, verified against `applied_coord_mode` and `marks::shape_touches_ring`;
only the count was wrong. **Repaired in `6e6dbc08`.**

**minor — the `emitter`'s `disc` freezes and then cuts, and now a preset can reach it.**
`core/src/render/scenes/emitter.rs:302`. This scene's `disc` is the anisotropic glint rather than
`mark_distance`'s circle, so `fs_main` early-outs on `if (in.shape < 0.5)`. While `mark_shape`
rounded, that test meant exactly *"the shape is index 0"*. It is now a half-open interval, and the
consequence on the `disc -> ring` pair of this one scene is worse than a cut: across `[0, 0.5)` the
figure does not move at all, and at `0.5` it jumps to a 50/50 blend of the *isotropic* disc and the
ring — two quantities discontinuous at once, the branch and the disc's own aspect. Round 1 raised
this and rated it minor partly because the travel was unreachable from a preset at all; that premise
is gone, since `shape` is `Modal` now. It stays a minor rather than rising, because every whole
index is still an exact identity, no shipped preset binds `shape` to an expression, no baseline
moves, and the emitter's `disc` is already documented on the same page as a different figure from
the roster's. **Left open**: the repair is a decision with two defensible answers — blend the glint
against the ring across the interval, or declare that this arm does not participate and say so at
the branch and in the essay — and picking one is a behaviour choice, not a text edit. The second is
the recommendation, because a glint that morphs into a ring was never a coherent figure.

**nit — `applied_coord_mode` clamps to the default rather than to a named floor.**
`core/src/render/background.rs:906`. `mode.clamp(DEFAULT_COORD_MODE, MAX_COORD_MODE)` uses the
parameter's *default* as the roster's lower bound, beside a `MAX_COORD_MODE` that is a named
constant for the other end. If a later plan moved the default off 0 the clamp would silently start
refusing arm 0, and neither of `the_coordinate_mode_clamps_rounds_and_falls_back`'s assertions would
catch it, since both are written against the default's picture. **Left open** — carried from round 1
and re-verified unchanged; a constant is code, which is not on ADR-0209's closed list.

**minor — the implementation log outweighs the contract it reports against.** `## Implementation
log` runs about 4.6x the length of `## Implementation phases`, and Mode 4 holds the report to no
more than the contract. Several sections restate measurement tables that the test doc comments
already carry verbatim, and could cite the test instead of copying it. **Left open** — trimming an
implementer's report of its own work is not on ADR-0209's closed list.

#### Close-ceremony record

- **Backlog probes:** exit 0 — *43 stated reductions still hold across all 20 live entries (4
  unprobeable)*, with 30 probed paths reported as moved (advisory). The three entries this plan
  closes are not among the live ones: ADR-0206 moved them to the archive at approval, so their
  evidence is the plan's done-whens and the archived bodies' own probe lines, read against the
  finished tree. All three are discharged — 0101 by Phase 1's blend (reachable from a preset as of
  round 1's fix), 0100 by Phase 2's `star_wobble` / `star_seed`, 0095 by Phases 3 and 4, with Phase
  3 answering by rendering exactly as that entry instructed.
- **Translation advisory:** two rows — `docs/running.ru.md` (source at `fe2b682f25`) and
  `packaging/foobar/READ-ME-FIRST.ru.md` (source at `d6e275e6db`). Neither English source was
  touched by this plan; the rows are routed, not repaired.
- **Preset curation (step 3b):** `presets/` was touched but **no `.toml` moved**, so the shipped set
  is unchanged and there is nothing that landed to judge and nothing that converged. The
  workaround grep was read whole rather than piped, and **no shipped preset is authored around any
  of the three gaps this plan closes** — `shape_facet.toml`, the one preset on the `star` arm, cites
  backlog 0097 and 0099 and Plan 0098, none of which this plan touches.
- **Version bump:** `minor`, plus the studio's two copies, with the annotated tag on the sync commit.

### Round 1 findings a fix round resolved

- **major — the roster travel was unreachable from a preset; `shape` still rounded in the binding
  pipeline** (`core/src/render/scenes/marks.rs:573`). Resolved in **`5174d516`**: `SHAPE` declared
  `ParamKind::Modal`, the three `("*", "shape")` rows dropped from `STRUCTURAL` with a comment
  naming ADR-0226, the four generated surfaces regenerated, and
  `an_eased_shape_sweep_lands_between_the_arms` added, driving the ease through
  `SHAPE.kind.quantize` **and** `mark_shape` with `POINTS` as the control. Verified against the tree
  this round.
- **minor — on `emitter`, the disc-to-ring travel is a cut at the midpoint** — not resolved; re-raised
  above as round 2's third minor, with the premise that made it unreachable now gone.
- **minor — the implementation log outweighs the contract it reports against** — not resolved;
  re-raised above.
- **nit — `applied_coord_mode` clamps to the default rather than to a named floor** — not resolved;
  re-raised above.

## Followups (after this lands)

- A `preset-author` look for each lever, which is the only thing that can say whether it earned its
  place.
- **The `emitter`'s glint does not travel** (round 2's third minor): decide between blending it
  against the ring across `[0, 0.5)` and declaring the arm out of the roster's travel in words.
- **`points` declares `3..16` and applies `3..12`**, and `presets/README.md` prints both numbers on
  one page. Predates this plan; wants a backlog entry.
