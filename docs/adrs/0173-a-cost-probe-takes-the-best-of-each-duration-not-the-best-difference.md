# ADR-0173 — A cost probe takes the best of each duration, not the best difference

> **Status:** proposed
> **Date:** 2026-09-07
> **Related plan(s):** [0157](../plans/0157-the-cost-probes-estimate-a-duration.md)
> **Extends:** [0071](0071-a-numeric-test-contract-states-a-property-or-names-its-machine.md)
> (a numeric test contract states a property or names its machine),
> [0016](0016-gpu-tests-opt-in-ci-scope.md) (the software-rasterizer skip)

## Context

Four tests price one rendering feature against the case it replaces: `core/tests/arc_cost.rs`,
`collage_cost.rs`, `field_cost.rs` and `mark_cost.rs`. Each renders the same preset twice — a short
run of `FRAMES_SHORT` frames and a long run of `FRAMES_LONG` — and divides the difference by the
frame delta. **The slope is the right instrument and it is not what this ADR changes**: subtracting
the short run cancels shader compilation, first-use allocation and every other fixed setup cost, so
what is left is per-frame work.

The estimator around that slope is what fails. All four repeat the pair `REPEATS = 3` times and keep
the **minimum of the difference**:

```rust
*slot = slot.min((long - short) / f64::from(FRAMES_LONG - FRAMES_SHORT));
```

The comment above `REPEATS` states the reasoning: *"The **minimum** is kept, not the mean: a
scheduler hiccup can only add time, so the smallest reading is the one closest to the truth."*

**That reasoning is sound for a duration and inverted for a difference of durations.** A hiccup
landing in `long` adds to `long - short`, and a hiccup landing in `short` *subtracts* from it. So
taking the minimum across repeats does not reject noise — it selects the repeat whose `short` leg
was most inflated. The bias is systematic, it points downward, and it grows with the noise on the
machine. With enough contention the estimate crosses zero.

On 2026-09-07, `check (macos-latest)` reported:

```
polygon12  shape=2 points=12  -5.319 ms/frame  (-11.941 ms, -180.3 % vs disc)
the polygon12 reading is not a time: -5.318557116666667
```

A negative duration, caught by `mark_cost.rs`'s own `is_finite() && ms > 0.0` guard.

**ADR-0016's skip does not cover this, and the reason is the interesting part.** All four probes
skip on a software rasterizer, because a WARP frame time says nothing about real hardware. macOS
runners have a real Metal adapter, so the skip does not fire and the probes run. The skip asks *is
this adapter real?*; the estimator needs *is this machine quiet?* **Those two questions have the
same answer on the reference machine and different answers on a shared CI runner** — which is
precisely why no run on the development configuration could expose this. The probes are also outside
the nine suites [ADR-0156](0156-the-per-phase-gate-is-scoped-and-the-suite-is-owed-once-per-plan.md)
defers, so they run in the narrowed `-P fast` profile on every arm of every push.

One asymmetry compounds it: three of the four assert the reading is a time. `arc_cost.rs` does not —
its header says it *"asserts only that it measured genuinely different figures"* — so it is the one
probe that would print a negative ms/frame and pass.

## Decision

We will minimize `short` and `long` **independently** across the repeats and subtract **once**,
after the loop, in all four cost probes. Each term is then a minimum over a set of measurements of
the same quantity, which is the shape the existing comment's reasoning is actually valid for, and
the estimator's remaining bias is the difference of two small minima rather than a one-sided
selection of the worst-contaminated sample. The cases stay interleaved within each repeat, so
thermal and scheduler drift still lands on every case alike.

We will also **add the positivity guard to `arc_cost.rs`**, matching the other three. A duration is
positive; that is a property in ADR-0071's sense, it holds on every machine CI runs, and it is the
assertion that converts a silently meaningless report into a failure.

## Consequences

### Positive

- **The systematic downward bias is gone.** What remains is `min(noise in long) − min(noise in
  short)`, which is centered near zero and small, rather than a term that grows with contention and
  has only one sign.
- **The probes keep running on every CI arm.** Their portable assertions — that each case really
  rendered, and really rendered a *different* shader than the baseline — keep their coverage on
  every adapter CI offers, which a machine-scoped skip would have discarded.
- **The change is the estimator, not the contract.** No threshold is added, no reading is asserted
  against a frozen number, and the report keeps printing what it saw. ADR-0071's rule is satisfied
  before and after; what changes is that the number the report prints is now an estimate of the
  thing it claims to estimate.
- **Cost is unchanged** — the same `2 × REPEATS` timed runs per case.

### Negative

- **This is a robustness improvement, not a proof.** `min(long)` can still fall below `min(short)`
  on a machine contended enough that the noise on a 30-frame run exceeds the true cost of 240
  frames. The positivity guard stays because it is the thing that catches that, and a red probe on a
  shared runner remains a possible outcome. **Nothing here promises a green macOS arm**; it removes
  the mechanism that made a negative reading systematic rather than merely unlucky.
- **Readings do not compare across this change.** A figure quoted from a report written before it
  was produced by a different estimator. The old numbers are not wrong as observations, but a
  before/after comparison of two reports measures the estimator as well as the code.
- **Two accumulators where there was one.** Marginally more state in each `per_frame_ms`, and four
  files carry the same shape independently rather than sharing a helper — consistent with how these
  probes are already written, and a shared helper is not proposed here.
- **`arc_cost` now asserts something its header says it does not.** The header has to change with
  it, and the test claims slightly more than it did.

### Neutral

- The reference machine is quiet enough that the two estimators plausibly agree there, so this is
  expected to move CI behaviour and not the numbers anyone has quoted from a local run. Unverified —
  the comparison has not been run, and it is not worth a phase.

## Alternatives considered

### Alternative A — Split the portable properties from the measurement
Run the pixel-difference assertions on every arm and gate the timing report to the reference
machine, which is the cleanest reading of ADR-0071's property/measurement distinction. **Rejected
because it restructures four tests to fix what one line fixes**, and because the timing report has
real value on any hardware arm as a smoke signal that a feature did not become catastrophically
expensive. The distinction it draws is right, and it stays available if the estimator repair proves
insufficient.

### Alternative B — Widen the skip so the probes run only on the reference machine
Follow ADR-0071's letter for a measurement: name the configuration and do not run outside it.
**Rejected because it discards coverage that is genuinely portable.** The assertions that each case
rendered, that it lit more pixels than the baseline, and that the dense case is denser than the
sparse one are properties, not measurements; they hold on every adapter and they are the assertions
that catch a probe silently measuring nothing. Skipping the whole test to dodge a noise problem in
one of its numbers throws the rest away.

### Alternative C — Drop or loosen the positivity assertion
Let the report print whatever it measured and assert only the pixel properties, as `arc_cost`
already does. **Rejected because it makes the defect invisible rather than absent.** A negative
duration would be published in the report, the estimator would stay biased, and the one signal that
something is wrong with the measurement would be gone. This ADR moves `arc_cost` the other way for
the same reason.

### Alternative D — Increase `REPEATS` and keep the minimum of the difference
Buy accuracy with samples. **Rejected because more samples make this estimator worse, not better.**
`min` over a difference converges toward the most-contaminated `short` in the set, so raising
`REPEATS` deepens the downward bias while multiplying the runtime of four GPU tests on every push.

## Notes

The failure was surfaced by a `fix(docs)` commit that repaired an unrelated rustdoc error. That
commit unmasked it in the literal sense — the macOS arm had been failing earlier, at `cargo doc`, so
its `nextest` step had not been reached on the previous push. The estimator defect predates it and
is not attributable to it.
