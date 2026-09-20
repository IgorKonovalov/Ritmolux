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
[ADR-0225](../adrs/0225-the-backdrop-ramp-gets-an-angular-coordinate-and-the-floor-stays-out-of-the-chain.md)
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

## Followups (after this lands)

- A `preset-author` look for each lever, which is the only thing that can say whether it earned its
  place.
