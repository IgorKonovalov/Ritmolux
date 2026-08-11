# ADR-0091 — The animation gate scores motion against the figure's own footprint

> **Status:** proposed
> **Date:** 2026-08-11
> **Related plan(s):** [0077](../plans/0077-the-quiet-sky.md)
> **Resolves:** the live half of
> [design-backlog 0009](../design-backlog.md#0009--the-animationrs-gate-penalizes-two-legitimate-designs-informational)

## Context

`core/tests/animation.rs` guards against frozen presets: it renders each preset at 96x96 in
silence, takes `metrics::frame_diff` (a mean absolute per-pixel difference **over the whole
frame**) between frames 24 and 48, and fails anything under `ANIM_FLOOR = 0.01`.

The statistic dilutes motion by emptiness. A sparse figure that moves lights few pixels, so
its real motion is averaged against a majority of unchanged black. Backlog 0009 traced the
consequences across two years of updates:

- `emitter_squall` shipped at **5x the density its author preferred** — the better-looking
  draft measured `anim 0.005` against the `0.01` floor and the author bought passage with
  density (2026-08-04).
- Plan 0067 Phase 1d measured the resolution ladder (96 / 192 / 384) and it is **flat**:
  `frame_diff` scores occupancy, and occupancy is scale-invariant, so no render size
  separates *sparse but moving* from *static*. The "just render bigger" route is closed
  empirically (2026-08-09).
- Plan 0075 cohort 4 **routed a look out of the library entirely**: Perseids' quiet
  twinkling sky — sparse marks, low coverage, slow shimmer — could not be gated at any
  tuning anyone would ship (2026-08-11).

The gate is now shaping content rather than measuring it, and the renaissance ships more of
exactly the sparse idiom it prices out. Backlog 0009's own update named the earned question:
a **coverage-aware statistic**, not a bigger render and not a lower floor.

One of 0009's two original cases is *out of scope by arithmetic*: a figure invariant under
rotation by `2*pi/k` produces an **identical image** under that rotation, so its frame
difference is zero under any statistic at any resolution. No image-domain measure can rescue
it. That case stays what it already is — a documented authoring constraint (move radially).

## Decision

The animation gate's statistic normalizes motion by the figure's own footprint: the frame
difference is measured **against the pixels the figure actually lights**, not against the
whole frame. Whether that is a masked mean (difference over the union of lit pixels in the
two frames) or a normalized whole-frame mean (`frame_diff / max(occupancy, eps)`) is chosen
at implementation and the choice recorded in the test, in the same shape Plan 0075 Phase 1
used for the sanity floor. `ANIM_FLOOR` is re-derived against the new statistic, with the
derivation stated beside the constant (ADR-0071's rule).

The safety argument is structural: a genuinely static preset has a **zero numerator** —
identical frames differ nowhere — so no normalization can help it pass. The change moves
only the class the old statistic wrongly diluted: sparse figures in real motion.

## Consequences

### Positive

- The sparse idiom becomes gateable. A quiet, low-coverage, slowly shimmering look can pass
  on its motion rather than on its density.
- The gate stops selecting for the defect (density and glow inflation) — the same repair
  shape as backlog 0072's sanity-floor fix, on the sibling gate.
- The Squall history becomes a free non-vacuity probe: the rejected fifth-density draft is
  reconstructible from the shipped preset and must pass the new statistic; the Phase 1d
  static control must keep failing.

### Negative

- A normalized statistic amplifies noise when the footprint is tiny. The denominator needs a
  stated lower bound (an epsilon floor with its derivation), or a one-pixel flicker in a
  nearly-empty frame reads as strong animation.
- The floor moves, so the whole shipped library is re-measured under the new statistic. Any
  shipped preset the re-derived floor convicts is a finding to file, not a number to tune
  around — the sweep is part of the paired plan's done-when.
- The rotational-symmetry case is **not** rescued and never can be by this route; the
  authoring docs keep carrying it.

## Alternatives considered

### Alternative A — render the gate at a higher resolution

Measured and closed: Plan 0067 Phase 1d's 96/192/384 ladder is flat because occupancy is
scale-invariant. Paying more per capture buys zero separation.

### Alternative B — lower `ANIM_FLOOR`

Backlog 0009 refuses this in its own words: a floor a genuinely static preset can clear is
worth nothing, and the shipped Squall sits at 1.8x the current floor — not enough headroom
to give away blind. The defect is the statistic, not the constant.

### Alternative C — per-preset exemptions

An allowlist of "known sparse" presets rots (this project's allowlists carry measured
evidence precisely because bare ones decayed), and it exempts exactly the class of content
the renaissance is about to ship more of — the gate would stop gating the library's future.

## Notes

The measured record this decision rests on is pinned in backlog 0009 (the Squall numbers,
the flat ladder) and `core/tests/animation.rs`'s own `#[ignore]`d ladder measurement.
