# ADR-0074 — A ratio against an in-run control is not automatically portable

> **Status:** proposed
> **Date:** 2026-08-04
> **Related plan(s):** [0060](../plans/0060-a-test-number-states-a-property-or-names-its-machine.md)
> (Phase 3, which routed this decision here by its own instruction)
> **Supplements:** [0071](0071-a-numeric-test-contract-states-a-property-or-names-its-machine.md)
> (whose "property" form this narrows, and one of whose Positive consequences it withdraws),
> [0016](0016-gpu-tests-opt-in-ci-scope.md) (the skip-with-notice shape the rejected fallback would
> have used), [0023](0023-golden-drift-guard-uses-frozen-fixtures.md) (`MEAN_TOL`, the declared
> noise floor).

## Context

[ADR-0071](0071-a-numeric-test-contract-states-a-property-or-names-its-machine.md) says a numeric
assertion states a property or names its machine, and it offered the **ratio against a control
taken in the same run on the same adapter** as one of the three property forms — "so the machine
cancels". `dual_live_keeps_the_outgoing_side_animating` was rebuilt on that form in Plan 0060
Phase 1, deliberately without a magnitude floor, and Phase 2 pushed it to read the ratio off the
runners. Those numbers are now in, and they falsify the premise.

| statistic | local WARP (10.0.19041) | CI WARP (10.0.26100) | spread |
|---|---|---|---|
| peak signal — `max frame_diff(frozen[i], live[i])` | 0.109573 | **0.009683** | **11.3x** |
| peak control — `max frame_diff(frozen[0], frozen[i])` | 0.407826 | 0.264177 | 1.54x |
| **ratio** — peak signal / peak control | **0.268675** | **0.036654** | **7.3x** |
| `golden.rs` `MEAN_TOL`, for scale | 0.02 | 0.02 | — |

The CI reading is **stable, not noise**: `check (windows-latest)` and the instrumented `coverage`
job produced `0.036654` identically, in the same push (run 30903871856).

Three things follow, and the third is the decision.

**The ratio does not cancel the machine.** If it did, the two readings would agree; they differ by
7.3x. Signal and control move by different factors between the two builds — 11.3x against 1.54x —
because they are not the same kind of quantity. The control measures the dissolve's own
progression (the incoming side fading in). The signal measures the outgoing side continuing to
render *through* trails accumulation. An in-run control on the same adapter is **necessary** for a
portable ratio and is not **sufficient**; the two terms must also respond to the machine the same
way, and these do not.

**The numerator is dominated by a driver artifact, and that is not arguable.** The code is
identical and deterministic on both machines — the test's own opening-frame `assert_eq!` on raw
RGBA is the demonstration that two runs reproduce each other byte for byte. An 11.3x spread in the
signal between two builds of the same software rasterizer therefore cannot be the dissolve's
behaviour. It is `dissolve_at`'s documented WARP quirk: allocating the dissolve's GPU resources
mid-run resets what the trails feedback resolves to, so the outgoing side comes back at a single
stroke's brightness. How much history that costs is a driver behaviour, and the newer WARP evidently
costs more of it. Plan 0060's Risks section predicted this hazard in the abstract; the measurement
is what makes it a number.

**And the CI signal sits under the declared noise floor.** `0.009683` is below `MEAN_TOL = 0.02`.
Same-machine determinism means it is not literally noise — but ADR-0071's own corollary says a
claim of that magnitude on this statistic is not worth defending, and it applies to a claim built
*on* the quantity as much as to one asserted *about* it.

Plan 0060 named the fallback condition in advance — a CI ratio below `0.05` — and routed the
decision here rather than to `dev`. `0.036654` is below it.

## Decision

**`dual_live_keeps_the_outgoing_side_animating` keeps its property form on WARP and gains no
magnitude floor, in CI or anywhere else.** The magnitude claim is not written from these numbers;
it is deferred to hardware, where the allocation quirk that crippled the numerator does not exist.
The occasion already exists on the roster: Plan
[0053](../plans/0053-the-suite-stops-blessing-what-warp-gets-wrong.md) Phase 3 is the `human` phase
that puts a discrete GPU in front of this suite, and its sibling
`a_dual_live_dissolve_carries_the_outgoing_trail` is already hardware-only and already asserts a
magnitude there (`CARRIES = 1.5`).

The test's doc comment is **demoted to what it actually proves on a software adapter**: that
dual-live runs, that it produces a different picture from freeze somewhere in the window, and that
the dissolve is dissolving. It stops reading as a guard on the defect, because on WARP it is not
one — the allocation quirk makes the two modes differ for its own reasons, so a genuinely held
outgoing side could still satisfy it. That was already true when Phase 1 landed and was recorded
only in the plan; it now lives in the test.

ADR-0071's rule gains one corollary: **a ratio is a property only when numerator and denominator
are the same kind of quantity.** Same run and same adapter are the entry requirements, not the
proof.

## Consequences

### Positive

- No number enters the suite that would break on the next runner image. Setting a floor from these
  two readings would have been a measurement asserted universally — the exact shape ADR-0071
  exists to forbid, at a different magnitude.
- CI keeps a green, non-flaky, cheap exercise of the dual-live code path, and keeps printing the
  full signal/control series through the `.config/nextest.toml` override — so the next time these
  numbers move, the log says so.
- The magnitude claim lands where it can be trusted, on the machine that already owes this suite a
  visit, next to the sibling that already makes a magnitude claim about the same dissolve.
- One ADR-0071 Positive is withdrawn honestly rather than left standing: *"the ratio form
  generalizes — any future differential pixel test has a control available"* is now qualified by
  the corollary above.

### Negative

- **Nothing in CI defends the dual-live magnitude, and the check that remains is weaker than it
  reads.** This is the cost, it is larger than the fallback's headline suggests, and it is written
  here rather than inferred: on WARP the surviving assertions can pass with the outgoing side held.
  CI has smoke coverage of the dissolve, not a guard on the defect.
- **The magnitude claim has no date.** It is deferred to a `human` phase gated on hardware this
  side does not have, and Plan 0053 Phase 3 has been waiting since 2026-08-02. If that hardware
  never materializes, the claim is never restored — which is a decision to accept, not a plan step
  to assume will run.
- The two WARP readings are now a known, unexplained 11x apart on a statistic nothing asserts.
  That is recorded and not investigated; if the newer WARP is losing *more* trail history than the
  older one, the same artifact is silently present in every other WARP capture this suite takes.

### Neutral

- No production code changes; no golden baseline moves; no CI workflow change. The whole decision
  is a test's doc comment, an assertion not added, and this record.
- The DSP half of Plan 0060 Phase 3 is untouched by this — the arm64 values and the determinism
  spec's scoping sentence proceed as written.

## Alternatives considered

### Alternative A — Set the floor anyway, at half the lower reading

`0.036654 / 2 = 0.018`, keeping the stated 2x headroom on the weaker machine. Rejected because the
headroom is measured against the wrong thing. The ratio varies 7.3x between two builds of one
rasterizer; 2x of headroom under the *lower of two samples* is not headroom, it is a coin flip on
the next runner image. It would also put the floor at `0.018` on a numerator whose CI value is
`0.0097` — a threshold calibrated against a quantity beneath the project's own declared noise floor.

### Alternative B — The documented fallback: make the whole test hardware-only

Plan 0060 named this in advance: skip on software alongside
`a_dual_live_dissolve_carries_the_outgoing_trail`, accepting that CI stops checking the dissolve.
Rejected because it gives up more than the decision requires. What failed to travel is the
**magnitude**, not the property: the exact-difference assertion and the non-trivial-control
assertion are green on both WARP builds, cannot fail spuriously, cost nothing, were verified
non-vacuous by mutation (freeze against freeze reads exactly `0.000000` at all 40 frames and
fails), and carry the printed series. Deleting a working instrument to settle a question about a
number that is not being added is a trade with no upside. The substance of the fallback — no ratio
floor — is taken; its mechanism is declined.

### Alternative C — Re-pose the control so the two terms are the same kind of quantity

Compare against the outgoing preset's *own* frame-to-frame motion rendered without a dissolve, so
signal/control reads as "how much of the ordinary motion survives into the dissolve" — near 1.0
when live, near 0 when held, and dimensionally the claim. Better-posed, and rejected for now on
mechanism rather than taste: on WARP the numerator is crippled by the allocation quirk before any
denominator is chosen, so no choice of control repairs it, and confirming that would cost another
push-and-read round trip to learn something the hardware session answers directly. Named as the
successor if the hardware never arrives.

### Alternative D — Delete the test

Rejected on ADR-0071's own grounds: the claim is worth defending. What is wrong is where it can be
proved, not whether.

## Notes

Readings taken 2026-08-04. Local: commit `1d56600`, `Microsoft Basic Render Driver`, DX12, driver
10.0.19041.5794, Windows 10 10.0.19045. CI: run **30903871856**, `windows-latest`, WARP
10.0.26100, read through the `.config/nextest.toml` `success-output` override added in `31073f6`
(without which both readings would have been invisible on exactly the green run that produced
them).

Two of Plan 0060's other predictions resolved in the same run and are recorded with the plan rather
than here: `onset_raw`'s arm64 divergence is `1.86e-5` relative — 2.2x `bass_raw`'s, not the orders
of magnitude the plan flagged as possible — and the coverage ratchet evaluated for the first time
since 2026-07-30 at **93.34 %** against `COVERAGE_FLOOR = 88`.
