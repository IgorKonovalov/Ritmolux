# ADR-0062 — Clamp occupancy is the saturation instrument, and it is a gate

**Status:** **accepted** — implemented in full by
[Plan 0056](../plans/done/0056-clamp-occupancy-and-the-axis-anchor.md), closed 2026-08-03.
**Carries an Outcome section**: the threshold is measured on both libraries, and no shipped preset
needed the exemption.
**Date:** 2026-08-03
**Related:** [ADR-0042](0042-reachability-measured-on-the-expression-tree.md) (reachability on the
expression tree), [ADR-0043](0043-reachability-reports-comparison-nodes.md) (comparison nodes),
[ADR-0049](0049-analysis-v2-dual-resolution-axis-normalized-bands.md) (the normalization that
exposed the gap), [design-backlog 0043](../design-backlog.md).

## Context

Every instrument this project has for "does a preset react to the music" compares a driven band
against **silence**:

| instrument | comparison |
|---|---|
| `core/tests/reactivity.rs` (HARD gate) | silent baseline vs one sustained band at `1.0` |
| `--report` per-band columns | the same, at full scale and at realistic levels |
| `anim` | frame-to-frame **in silence** |
| reachability (ADR-0042/0043) | which way each **comparison** went |

A silence-relative differential answers *does this preset respond to sound at all*. It cannot
answer *does it respond across the range music actually occupies* — and a binding that saturates
just above the noise floor scores **perfectly** on the first question while failing the second
completely. It is a binary switch, and to a silence-relative test a binary switch is maximally
reactive.

ADR-0049 turned that latent blind spot into a live defect. Normalizing the four headline levels
multiplied them by 16–96x, so every gain in the shipped library — all written against the old raw
magnitudes — drove its clamp to the ceiling and held it there. Plan 0048 Phase 7 measured the
result: **263 of 332 clamped band terms pinned at the real-music median, and 14 presets with no
live audio term at all** (Rose Web, Rose Zoom, Rose Trails, Rose Overflow, all five
`reaction_diffusion`, all three `spectrum`, Cathedral, Leviathan). The full suite was green
throughout, and `--report` printed a clean table.

Reachability cannot cover this because a gain contains no fork to observe. `clamp(bass * 16, 0,
0.3)` has no `select()`, no comparison, nothing two-valued — it is an arithmetic expression that
happens to have stopped varying.

This is the mirror image of the failure Plans 0041/0042 were spent closing: a *threshold* above
anything music produces. The instruments built there watch comparisons, and were never extended to
the case one level down — a *gain* whose output stops moving below anything music produces.

The walker already has the data. `Expr::flag_gates` visits every `clamp()` on every hop and
computes `value / upper_bound`; it keeps only the peak
(`GateObservation::Clamp { peak_fraction_of_bound }`, `core/src/preset/expr.rs:1129`). The
statistic that names this defect — the **fraction of hops the value spent at or above the bound** —
falls out of the same traversal for the cost of one counter.

## Decision

We will record **clamp occupancy** — the fraction of evaluated hops in which a `clamp()`'s inner
value reached its upper bound — alongside the existing peak fraction, surface it as a `--report`
column, and **enforce it as a HARD gate** over the embedded preset set.

Occupancy is measured per `clamp()` node, so a finding names the *binding* rather than the preset.
That is the property that makes it worth building: a frame differential can only report "this
preset is flat", which is where the investigation starts rather than ends.

The gate flags a term whose occupancy exceeds a high threshold — provisionally 0.9 over the probe —
and a preset may carry an explicit, in-file exemption for a clamp that is *supposed* to pin.

## Consequences

**Positive**

- The defect class becomes visible at the binding, in CI, on the run that introduces it. Phase 7's
  263 terms would have failed the build the day ADR-0049 landed.
- Near-zero cost: one accumulator on a traversal that already runs, and no new stimulus, harness or
  render pass.
- It composes with reachability rather than duplicating it. Reachability owns forks, occupancy owns
  arithmetic that stopped varying; together they cover the two ways a binding can be a constant.
- The number is directly actionable. Occupancy `0.97` on `clamp(mid * 16, 0, 0.3)` states the fix:
  the ceiling is reached at `mid = 0.019`, so divide the gain.

**Negative**

- **A threshold has to be chosen, and it is genuinely arbitrary.** Some clamps *should* pin: a
  safety rail on an attractor coefficient exists to bind at peak, and `attractor_ink`'s `fade` is
  deliberately held near its bound. 0.9 is a starting point to be re-measured against the retuned
  library, not a derived constant. We accept a tuning parameter here because the alternative
  instruments (below) are worse for structural reasons, not tuning ones.
- **An exemption path is a place to hide.** A preset can silence the gate. This is the same trade
  the reachability check already makes by being advisory, except here it is explicit and in-file,
  which is the improvement.
- Occupancy is measured against one probe (`dynamic:110`), so it inherits every limitation the
  reachability probe has — including the standing single-BPM `tempo` false positive.
- It says nothing about a preset whose bindings vary but vary *wrongly*. This is a saturation
  instrument, not a taste one.

## Alternatives considered

**A mid-scale rung in the reactivity gate** — drive each band to ~0.4 as well as 1.0 and require
the two frames to differ from *each other*, not merely from silence. Rejected as the primary
mechanism: it is a frame differential, so it reports "this preset did not change between 0.4 and
1.0" and names no binding. On a preset with twelve band terms that is the beginning of a bisection,
where occupancy hands over the answer. It is also strictly more expensive — another full render
pass per band per preset on the software adapter, where the existing gate already runs 120 s. Worth
revisiting only if occupancy proves to miss a real case, since the two are complementary rather
than exclusive.

**A response-slope or monotonicity assertion** — require the render to change in a consistent
direction as a band rises. Rejected, and this is the one worth recording, because it would fail
*correct* presets. The attractor family is built on the explicit principle **peak buys structure,
not brightness**: `attractor_dejong` and `attractor_clifford` deliberately *lower* `fade` and
`size` as the track hits, so a drop sharpens the filigree instead of blooming it into a white blot.
A directional assertion marks that inversion as broken. The generalization is that this codebase
has no house direction for "louder", by design — only a house rule that the picture must *change* —
so a difference test is the strongest assertion that is actually true of the content.

**A ceiling-occupancy column in `--report` with no gate** — the cheap half. Rejected as the whole
answer: it catches the defect only when someone runs the report, and the entire lesson of Phase 7
is that nobody ran anything for the whole window between ADR-0049 landing and the retune, because
every automated signal was green. An instrument that requires suspicion to fire does not address
the failure that there was nothing to be suspicious of. It ships anyway as part of this decision —
as the diagnostic half, not as the guard.

**Leave it, and document the three-excitation contact sheet as the standing audit** — the
manual-process option. Rejected on the same ground: it is what Phase 7 actually did, it worked, and
it took a human deciding to look. Documenting it is worth doing regardless and is not an
alternative to a gate.

## Outcome (Plan 0056, closed 2026-08-03)

Implemented as designed and in full: one counter pair on the traversal that already computed the
peak, an `occ` column and per-binding `SAT` lines in `--report`, `core/tests/saturation.rs` as a
HARD gate, and the `[occupancy] exempt = [...]` table. Nothing about the mechanism moved. What the
measurement added:

**The threshold is `0.9`, and it is measured on *both* libraries rather than defended on one.** The
plan required the distribution instead of this ADR's suggested starting value taken on faith, and
`dev` walked 339 clamped bindings twice — today's library, and the pre-retune library at `80c5dff^`
that Plan 0048 Phase 7 found saturated:

```text
occupancy      today   pre-retune
[0.00, 0.10)      29        6
[0.10, 0.25)     171       11
[0.25, 0.50)     138       22
[0.50, 0.75)       1       51
[0.75, 0.90)       0      104
[0.90, 1.01)       0      145
```

So the answer to this ADR's implied question is **yes**: the gate would have failed the build the
day ADR-0049 landed, naming 145 bindings across 23 of 35 presets. Today's highest is `0.609`
(`Aurora.warp`) and its next is `0.444`, so `0.9` clears the legitimate maximum by `0.29`. It is
deliberately **not** the most sensitive separating value — `0.75` would catch 249 pre-retune
bindings rather than 145, but it would sit `0.14` above a shipped, reviewed preset, and a HARD gate
that fires on good content buys exemptions, which are the thing that dulls the instrument.

**No shipped preset needed the exemption.** This ADR and the plan both expected to find clamps that
legitimately pin; the measurement says there are none, so `[occupancy]` ships exercised only by
fixtures. That is a better outcome than the design anticipated and it should be re-checked whenever
the library changes materially — the constant has a shelf life, and it is documented on itself.

**What the gate does not see, recorded so nobody re-derives it:** the *marginal* form. One binding
pinned for 50-90 % of a track, in a preset with no severe case beside it, passes. What makes that
acceptable is that the defect arrives in clusters — every affected preset in the pre-retune set
carried a severe case too.

**One implementation detail worth knowing.** The failure message states the level at which the
ceiling is already reached, and it is measured rather than derived: inverting an arbitrary
expression to solve for its input is not possible in general, but asking the probe *which of its
hops still pinned the bound* is, and it answers in the units gains are written in. Over a single hop
occupancy is 0 or 1, so a `Saturated` flag from a fresh `Observations` **is** the per-hop question —
no new observation field was needed for it.

**A correction the plan did not ask for and needed.** `--report`'s `ceils` count was "everything
that is not a dead gate", which was a ceiling count only by coincidence; a third kind would have
made it report a saturated clamp as its exact opposite. Every count is now by kind.
