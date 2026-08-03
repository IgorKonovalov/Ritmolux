# ADR-0065 — The attractor's additive deposit is normalized by particle count: a tier buys smoothness, not brightness

> **Status:** accepted (2026-08-03) — implemented in full; see Outcome
> **Date:** 2026-08-03
> **Related plan(s):** [0057-the-attractors-compute-path](../plans/0057-the-attractors-compute-path.md) (Phase 2)
> **Supplements:** [ADR-0045](0045-quality-tiers-floor-and-rich.md) — quality tiers

## Context

`attractor_particles` is **50 000 at `Floor` and 150 000 at `Rich`**
(`core/src/render/tier.rs:219,237`). The draw blends `One, One` and its fragment emits
`in.color * g` with no division by the active count
(`core/src/render/scenes/particles/mod.rs:392`), so `Rich` deposits three times the light into the
same accumulation texels. Everything downstream to the tonemap is linear, so the whole figure moves
up three stops.

ADR-0045 and `presets/README.md` both state that a tier changes **capacity, not behavior**, and
that "no expression, param or structural field changes meaning". For an *accumulating additive*
scene that claim is false: the same `fade` and the same `size` produce a different picture at a
different count, because capacity here **is** the picture. The claim holds for every other family
and fails for this one.

The price is on record. Commit `00d99d0` (2026-08-03) retuned four shipped attractor presets
**downward** so they would survive `Rich` — Clifford furthest, `fade` 0.885 → 0.50, `size`
0.62 → 0.28, `trails` 0.62 → 0.20 — with the measured share of the lit figure inside one narrow
tone band falling from 49.8 % to 15.7 %. Those presets are now authored dim at the tier the
harness *can* see in order to be correct at the tier it cannot, which is a compensation carried in
content for a defect in the engine.

## Decision

The attractor's draw scales its deposit by `FLOOR_PARTICLES / active_count`, so the total light
deposited per frame is invariant to the particle count. At `Floor` the factor is exactly `1.0`, so
`Floor` output is byte-identical and no golden baseline moves. At `Rich` the same figure is
rendered from three times as many samples at one third the weight each — so the tier buys **less
shot noise in the same picture**, which is what a capacity tier should buy, and ADR-0045's
capacity-not-behavior claim becomes true for the last family that broke it.

## Consequences

### Positive
- A preset authored at `Floor` — the house rule, stated in `presets/README.md` — is correct at
  `Rich` for the first time.
- No golden moves, and the invariance is provable rather than observed: the scalar at `Floor` is
  `1.0` by construction, assertable on the value itself rather than inferred from pixels.
- The lever that backlog 0031 identified (`attractor_clifford` blowing to white at `Rich`) is
  removed at its cause. That entry named the particle count, not the tonemap curve, as the thing to
  move; this moves it.

### Negative
- **The four presets `00d99d0` brought down are now conservative at both tiers.** Their
  compensation was for a 3x that no longer exists, so they owe a re-raise. This is real content
  work and it is why the fix ships in a plan with a content phase rather than alone.
- **A preset can no longer buy brightness by running at `Rich`.** That was never a documented
  lever, but it is what the currently-shipped pictures do, and anyone comparing a screenshot from
  before this change will find the new `Rich` dimmer.
- A future tier with a count *below* `Floor` would put the scalar above `1.0` and amplify shot
  noise rather than reduce it. Bounded and predictable, but worth knowing before adding a third
  tier.

### Neutral
- The swarm's additive draw is not affected: it is a different scene with its own count, and no
  claim about tier neutrality has been reported broken there. If it is, this is the shape of the
  answer.

## Alternatives considered

### Alternative A — document the 3x and let authors hold headroom
Free, and it is precisely what the four repaired presets already do. Rejected because it makes
every attractor preset carry a compensation for a configuration its author cannot capture, and
because ADR-0045's stated contract would have to be weakened for all readers to accommodate one
family. Writing the trap down is not the same as disarming it.

### Alternative B — compensate at the tonemap with a per-tier `exposure`
Mathematically equivalent for this scene — the path from deposit to tonemap is linear, which is
exactly why the content lane's `exposure * 3` proxy reproduced the reported frame. Rejected because
the tonemap is **engine-wide and terminal**: dividing there to fix the attractor moves every other
scene at `Rich`, and it places the correction at maximum distance from its cause, where the next
reader has no way to connect the two.

### Alternative C — expose the normalization to content as a `[particles]` key
Rejected because the default is the decision here. All six shipped attractor presets want the same
answer, none of them wants the un-normalized behavior, and a key would let a preset re-arm the
trap silently — with the same invisibility that let it ship, since no capture can see `Rich`
without [ADR-0064](0064-a-capture-may-pin-the-rich-tier.md).

## Notes

Discovered by `preset-author` on 2026-08-03 ([backlog 0047](../design-backlog.md)) while repairing
four presets the user reported as "very dim" at `Rich`; the screenshot showed the opposite of dim.
Verified against `core/src/render/scenes/particles/mod.rs:387-393` and
`core/src/render/tier.rs:219,237`.

## Outcome (Plan 0057 Phase 2, 2026-08-03)

**Accepted, implemented as decided, and measured rather than asserted.**
`particles::deposit_scale(active_count) = FLOOR_PARTICLES / active_count` is written into the draw
uniform's `w.w` and applied in the **vertex** shader — the draw uniform is bound `VERTEX`-only, and
since the fragment multiplies by a radial falloff and both are linear, that is identical to scaling
the emitted fragment.

Clifford at 1280x720 over 120 frames, mean display luminance over the whole frame:

| tier | before | after |
|---|---|---|
| `Floor` | 10.337 | 10.337 |
| `Rich` | 17.372 | 10.863 |

The `Floor` capture is byte-identical before and after, which is the scalar being **exactly** `1.0`
rather than approximately it — and that is why no golden baseline moved. The `Rich` gap closes from
68 % hotter to 5 %; the residual is not a tolerance being spent but two samplings of the same
distribution at different rates, read through a compressive tonemap. (The 1.68x measured is the 3x
linear deposit after that roll-off, not a contradiction of this ADR's arithmetic.) At matched
exposure the `Rich` frame is visibly smoother — finer speckle, more continuous internal arcs —
which is what the tier now buys.

The invariance is pinned on the **value**, not inferred from pixels:
`the_deposit_scalar_is_exactly_one_at_the_floor_and_a_third_at_rich` asserts `1.0` at `Floor`, the
tier table's own count ratio at `Rich`, and `count * scale` equal at both — written against
`TierConfig` rather than a literal `1/3`, so Plan 0044 Phase 4's calibration will move the
expectation with it instead of failing for the wrong reason.

**Two consequences landed beyond the code.** ADR-0045's capacity-not-behaviour claim is reconciled
where it is *stated* — `core/src/render/tier.rs`'s module header and `presets/README.md` both now
say the claim was false for this family, why (for an accumulating additive scene, capacity **is**
the picture), and the general form: **a count feeding an accumulating pass is a look value until
something normalizes it.** And the four presets `00d99d0` had brought down to survive the 3x owed a
re-raise, which Phase 6 paid — halfway rather than fully, because `00d99d0` also added a bloom stage
sized to the lowered figure. Rendered rather than reasoned: a full revert puts Clifford's interior
back to a flat salmon mass, the exact failure `00d99d0` fixed arriving by another route.
