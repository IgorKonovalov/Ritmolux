# 0059 — Lorenz finds its plane, and the attractor can trade samples for curves

> **Status:** draft 2026-08-03
> **Created:** 2026-08-03
> **Owner skill(s):** dev, human
> **Related ADRs:** [0068](../adrs/0068-the-projection-basis-is-a-per-family-property.md) (Phase 1),
> [0069](../adrs/0069-the-attractor-trades-sample-count-for-trace-length.md) (Phases 2-3).
> Depends on [ADR-0065](../adrs/0065-the-attractor-deposit-is-normalized-by-particle-count.md) —
> the normalization that makes a preset-chosen count safe — and inherits
> [ADR-0066](../adrs/0066-a-reseed-disturbs-the-cloud-rather-than-replacing-it.md)'s kick.
> **Successor to [Plan 0057](done/0057-the-attractors-compute-path.md)**, whose Phase 4 diagnosed
> this and stopped by its own instruction; closes [backlog 0048](../design-backlog.md).

## TL;DR

`attractor_lorenz` renders the wrong plane. The shared 3-D projection uses `y` as the vertical and
rotates `x` against `z`, so the rest view is x–y and the quarter turn is z–y — and the butterfly
lives in **x–z**. Plan 0057 Phase 4 proved it with a discriminating capture and routed the fix here,
because changing a shared convention for one family is a decision rather than a constant. Phase 1
takes it. Phases 2 and 3 then take the half a basis fix cannot reach: corrected to x–z the figure
still reads as **stipple**, because the scene draws 50 000 independent samples of the invariant
measure where the iconic plot follows one trajectory as a curve — so `[particles]` gains a `density`
fraction and the continuous families draw a segment instead of a point. Phase 4 is the one content
pass over all six attractor presets. First visible behavior: Lorenz has a shape.

## Context & problem

The two ADRs carry the mechanisms and the measurements. What justifies one plan rather than two is
the same coupling Plan 0057 was built around, and it is concrete: **all four phases move
`attractor_lorenz`'s look**, and Plan 0057 Phase 6 already withheld that preset's re-tune on the
grounds that judging a figure about to change shape is judging nothing. Split into two plans, that
preset is re-authored twice.

Three things carry forward from Plan 0057 and are stated here rather than only in its file:

- **The diagnosis is settled and does not need re-litigating.** Lorenz occupies 5.89 % of its own
  bounding volume, stable from 60 through 240 to 600 frames, so it is converged — this is not
  integration and not an un-converged seed. The capture that discriminates the basis needs
  `SPIN_RATE` pinned to `0`, or the frame is the 41° the spin reaches by frame 240 rather than the
  rest basis being reasoned about.
- **`shot --tier floor|rich` and `--at <hop>` both exist**, and this plan's captures use them. A
  `Rich` capture is an instrument and never a baseline.
- **Phase 2 of Plan 0057 returned 3x of headroom**, which is the budget Phase 4 here spends.

## Decision

Per ADR-0068: a **per-family projection basis in code**, Lorenz x–z, everyone else unchanged — no
preset key, because five of the six presets have no opinion. Rejected there: a preset-facing view
parameter (makes every preset answer a question five do not have, and still needs a default
underneath), re-centring Lorenz's coefficients (makes the shipped `sigma`/`rho`/`beta` name something
other than the textbook system every header cites), and keying the basis off `dim == 3.0` (the
ADR-0037 trap in another costume — `dim` and "wants a non-default basis" agree on today's roster of
four and are not the same property).

Per ADR-0069: **`density` plus the streak, as one decision**, because the arithmetic says neither
delivers alone. A `prev → current` segment is worth ~1.8x a point's footprint at Lorenz's measured
per-frame travel — it closes the beading and is not a trace — and trace length can only come from
persistence, which needs sparsity the engine cannot currently express. Rejected there: the streak
alone (a denser fog, not curves), `density` alone (ships the beading artifact it makes visible), a
4-segment sub-step polyline (the four sub-steps span the *same* 1.8 diameters, so it is 4x the fill
for less than a point radius of difference), per-particle position history (the right successor, and
undemonstrable until a sparse preset exists to compare against), and a bindable `density` (an eased
integer count re-decides the picture every frame).

**Phase order is basis → density → streak → content**, so that each capture the content pass judges
is of a figure that will not change shape again.

## Architecture diagram

```mermaid
flowchart TB
    subgraph fam["AttractorFamily — the per-family tables"]
        P["projection() — scale, dim, z-centre"]
        B["<b>basis() — NEW (ADR-0068)</b><br/>Lorenz x–z, others x–y"]
        C["<b>is_continuous() — NEW (ADR-0069)</b><br/>Thomas, Lorenz<br/><i>named, not dim == 3</i>"]
    end
    subgraph step["STEP_SHADER"]
        S1["integrate / iterate"] --> S2["<b>write prev AND pos</b>"]
    end
    subgraph draw["DRAW_SHADER"]
        D1{"is_continuous?"}
        D1 -->|"yes"| D2["quad spans prev → pos<br/>distance-to-segment falloff"]
        D1 -->|"no"| D3["point quad (today, unchanged)"]
    end
    CFG["[particles]<br/>family + <b>density</b>"] --> AC["active_count =<br/>density x tier budget"]
    AC --> DS["deposit_scale(<b>active</b>)<br/>ADR-0065 invariance carries"]
    AC --> DISP["dispatch + draw counts<br/><i>no resource rebuilt</i>"]
    B --> D2 & D3
    C --> D1
    S2 --> D2
```

## Implementation phases

### Phase 1 — Lorenz renders its own plane

- **Owner skill:** dev
- **What:** `AttractorFamily` gains a projection basis (ADR-0068); Lorenz returns x–z, every other
  family keeps x–y. It reaches the draw shader as axis selectors in the existing draw uniform — one
  pipeline, one draw call, no second shader. **Name the basis outright**; do not derive it from
  `projection().1 == 3.0`.
- **Files touched:** `core/src/render/scenes/particles/mod.rs`, `presets/README.md`.
- **Done when:**
  - **Lorenz reads as the butterfly at a rest angle *and* at a quarter turn.** The two-angle check is
    the point: a basis fix that only works at one rotation is the same defect moved. Capture with
    `SPIN_RATE` pinned to `0` for the rest frame, and describe both frames in the phase commit — the
    rest view should show two lobes with the notch top and bottom centre. **This is a described
    property, not a threshold**: nothing in the harness can score "looks like a butterfly", and
    inventing a number for it would be inventing a measurement.
  - **De Jong, Clifford and Thomas captures are byte-identical**, verified rather than reasoned.
    Thomas is the one to check — it is the other user of the 3-D branch, and its cyclic symmetry is
    why it does not *need* x–z, not a reason a change to it would be free.
  - **No golden baseline moves.** `core/tests/fixtures/attractor.toml` runs the default De Jong.
  - A test pins the basis per family against an explicit expected table, so a fifth family cannot be
    added without choosing one.
  - `presets/README.md`'s attractor section says which plane each family is viewed in, since an
    author tuning `zoom`/`pan_*` on Lorenz has been aiming at a different figure than they thought.

### Phase 2 — A preset can choose how many samples it draws

- **Owner skill:** dev
- **What:** `[particles] density`, a fraction of the tier's particle budget, defaulting to `1.0`
  (ADR-0069). It selects an **active count** and rebuilds nothing: the buffer stays allocated at the
  tier budget, the compute already returns early on `i >= step.count`, and the draw's instance count
  becomes the active count. `deposit_scale` takes the **active** count so ADR-0065's invariance
  carries across the new dial.
- **Files touched:** `core/src/render/scenes/particles/mod.rs`, `core/src/preset/schema.rs`,
  `presets/README.md`, `docs/presets.md` if the structural-table reference lists `[particles]` keys.
- **Done when:**
  - `density` is validated at load with a stated range and a named error, like every other structural
    key. **Pick the floor from the arithmetic, not by feel**: state what the smallest allowed value
    resolves to at both tiers and why a cloud that sparse is still a picture.
  - **Total deposited light is invariant across `density`**, asserted on the value the way ADR-0065's
    scalar already is: `active_count * deposit_scale(active_count)` is constant. This is the property
    that makes the key structural rather than an exposure control, so it is asserted, not observed.
  - **A density change rebuilds no GPU resource.** Assert the inert tail directly — after N frames at
    a reduced density, positions beyond `active_count` are **unchanged** from their seeded values,
    read through Plan 0057's `read_positions`. That also proves the early-return guard is what is
    doing the work.
  - `density = 1.0` is byte-identical to today for every shipped preset, and **no golden baseline
    moves** — no fixture declares `[particles]`.
  - `presets/README.md` documents the trade in the authoring voice: `density` and `fade` are one look
    together, low density plus high `fade` is curves, high density plus high `fade` is fog, and the
    tier caps the top rather than setting the value.

### Phase 3 — The continuous families draw a segment

- **Owner skill:** dev
- **What:** the step shader writes each particle's pre-integration position alongside its new one;
  the draw expands a continuous family's instance into a quad spanning `prev → pos`, with the
  fragment's radial falloff becoming a distance-to-segment falloff. **Discrete maps keep the point.**
  The predicate is a named `is_continuous()`, not `dim == 3.0`.
- **Files touched:** `core/src/render/scenes/particles/mod.rs`, `presets/README.md`.
- **Done when:**
  - **The segment's endpoints are this frame's `pos` and the previous frame's `pos`**, asserted as a
    property of the shader's inputs rather than of the pixels — a readback of `prev` after frame N
    equals the readback of `pos` after frame N-1. Zero gap by construction is what "the beading
    closes" means, and it is checkable where "is the stroke connected" is not.
  - `is_continuous()` is pinned against an explicit per-family table, **with the test stating that
    its agreement with `dim == 3.0` is a coincidence of the current roster** — the same hazard
    ADR-0068 Alternative C declines. A 2-D flow or a 3-D map must force a choice.
  - **De Jong and Clifford captures are byte-identical and no golden baseline moves.** A chord across
    a discrete map's scattered successive points is meaningless geometry drawn brightly; the test
    that the maps are unchanged is what proves they never take the branch.
  - The reseed's long streak is **reachable both ways** — drawn, and suppressed on the jitter frame —
    behind whichever mechanism is cheapest to flip for a capture, because Phase 4 decides it and this
    phase must not. Say in the commit which is the shipped default and that it is provisional.
  - `Particle`'s "std430 stride is a tight 16" note is corrected, and the memory cost at both tier
    budgets is stated.
  - **Record the exposure change rather than normalizing it.** A segment emits more light than a
    point, by a ratio that varies with local speed; ADR-0069 accepts that deliberately and names the
    content pass as the lever. Measure the frame's mean luminance before and after on one continuous
    preset so Phase 4 knows what it is compensating for.

### Phase 4 — The one content pass

- **Owner skill:** human
- **What:** run the `preset-author` lane over the attractor family **once**, judged at both tiers
  with `--tier`. Four things settle in the same pass: `attractor_lorenz` re-authored against a figure
  that now has a shape (Plan 0057 Phase 6 withheld this deliberately); whether `density` + `fade` can
  hold a legible curve, which is the question ADR-0069's Alternative D waits on; the reseed-streak
  A/B from Phase 3; and the exposure shift the streak introduces on `attractor_thomas` as well as
  Lorenz.
- **Files touched:** `presets/attractor_*.toml`.
- **Done when:**
  - Each preset touched is judged at both tiers and the commit states which levers moved per preset.
    Tonal flatness and coverage are recorded before and after — both are printed by `sanity` on every
    run, and the coverage floors are live gates now, so a re-raise that moves the family minimum will
    fail `report_coverage_distribution` with the number rather than drifting.
  - **The reseed-streak question is answered from a rendered A/B**, not from argument, and the answer
    is written onto the constant or the flag that carries it.
  - **`density` + `fade` gets a verdict in writing.** If a legible curve is reachable, say at what
    values. If it is **not**, that is the finding ADR-0069's Alternative D is waiting for, and it
    routes back to `architect` with the captures rather than being worked around in content.
  - **No preset's identity changes**: no palette, family or coefficient base moves. This is a re-gain
    and a re-aim, not a re-design.

## Data shapes

```rust
// illustrative — not the final interface

// Phase 1 (ADR-0068): named, exhaustive, beside the tables it belongs with.
fn basis(self) -> Basis { match self { Lorenz => Basis::XZ, _ => Basis::XY } }

// Phase 2 (ADR-0069): a fraction of the tier's budget, resolved once at load.
struct RawParticles { family: String, density: Option<f32> }
let active = (tier.attractor_particles as f32 * density).round() as u32;
let deposit = deposit_scale(active);   // ADR-0065 invariance, over the ACTIVE count

// Phase 3: the sub-step positions are NOT kept — only the frame's endpoints.
struct Particle { pos: [f32; 3], seed: f32, prev: [f32; 3], _pad: f32 }
```

No `Scene` trait change, no C ABI change (stays v4), no new dependency, no new pass, no new pipeline.

## Risks & open questions

- **Phase 4 may route back, and ADR-0069 names where.** If `density` + `fade` cannot hold a curve,
  per-particle position history (Alternative D) is the successor — and it will then have the rendered
  case it currently lacks, which is exactly why it was not taken first.
- **The streak's exposure shift is unnormalized on purpose.** A length normalization is a second
  constant with no measurement behind it, and speed-dependent brightness may be the correct rendering
  of a trajectory. If Phase 4 finds it unmanageable, that is the measurement the normalization needs.
- **`density` is a new way to be wrong**, and it is not independent of `fade` or the tier. The
  mitigation is documentation plus the fact that the tier caps the top; there is no gate that can
  tell a deliberate sparse look from an accidental one.
- **`attractor_lorenz` has never been authored against its real figure**, so Phase 4's work on it is
  closer to first authoring than to a re-tune. Budget accordingly.
- **No real-time hazard.** `density` is load-time and changes two integers; `prev` is written by the
  dispatch that already runs; the streak is the same draw call with a differently-shaped quad. No new
  allocation, no audio-thread contact.

## What this plan does NOT do

- **No preset-facing view basis.** ADR-0068 declines it; the expressive version is a superset that
  can supersede later if a look wants an animated viewing plane.
- **No per-particle position history.** ADR-0069's Alternative D, deliberately deferred to Phase 4's
  verdict.
- **No sub-step polyline.** Rejected by measurement, not by taste.
- **No `Rich` golden baselines**, and no tier calibration — Plan 0044 Phase 4 stays open and
  on-device.
- **No change to the reseed gates or to `JITTER_FRACTION`.** Plan 0057 Phase 6 set both against
  measured onset levels; this plan changes what a kick *draws*, not when it fires or how far it goes.

## Followups (after this lands)

- Re-check whether the streak wants a length normalization, with Phase 4's numbers.
- Whether the swarm — the other GPU particle scene — has anything to gain from `density`. It has its
  own tier field and its own draw, and nobody has asked.
