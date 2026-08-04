# ADR-0071 — A numeric test contract states a property, or names the machine it was measured on

> **Status:** **accepted** (2026-08-04, at Plan 0060's close)
> **Date:** 2026-08-04
> **Related plan(s):** [0060](../plans/0060-a-test-number-states-a-property-or-names-its-machine.md)
> **Supplements:** [0016](0016-gpu-tests-opt-in-ci-scope.md) (the skip-with-notice shape this
> reuses), [0023](0023-golden-drift-guard-uses-frozen-fixtures.md) (whose `MEAN_TOL` is the
> declared noise floor this ADR measures one threshold against),
> [0033](0033-testing-strategy-coverage-ratchet-and-pre-push-gate.md) (the testing strategy this
> adds a rule to).

## Context

CI went red on 2026-07-30 and stayed red across five consecutive pushes. Two tests failed, on two
different runners, for what looks like two unrelated reasons — and is one.

**`raw_levels_are_bit_identical_to_the_pre_normalization_build`** (`core/tests/dsp.rs`) asserts
that four `f32` levels reproduce, **bit for bit**, literals measured against commit `92579ef` on
the developer's x86_64 box. It landed in `ef3b772` and reached `main` in the `554c3aa` release. It
has **never passed on macOS** — not once. On `macos-26-arm64` the first value diverges by
`8.4e-6` relative (about 71 ULP), and it cannot do otherwise: the fixture generates its own input
with `f32::sin`, which lowers to the platform libm, and `rustfft` dispatches NEON on aarch64 where
it dispatches AVX/SSE on x86_64. Two different sets of rounding, applied to two slightly different
inputs. Bit-exactness across architectures was never available to be asserted.

**`dual_live_keeps_the_outgoing_side_animating`** (`core/src/render/mod.rs`) asserts
`frame_diff(frozen[20], live[20]) > 0.01`. On the developer's WARP (10.0.19041) that reads
`0.045`; on the CI runner's WARP (Windows Server 2025, 10.0.26100) it reads `0.0059`. The test
began failing when the Plan 0045 linear-light series merged, so a code change moved it — but the
threshold was never sound. `core/tests/golden.rs` sets `MEAN_TOL = 0.02` as the mean per-channel
difference that counts as **cross-rasterizer drift rather than signal**. This test's floor is
`0.01`, half of that, and its healthy local reading of `0.045` is only 2.25x it. The assertion has
always been made inside the band this project already declares to be noise.

What the two share is the shape, not the subsystem. Each freezes a number produced by **one
configuration** — one CPU architecture, one rasterizer build — and asserts it as though it were a
property of the code. Both passed review, and both passed on the machine they were written on,
because the configuration that disagrees is not the one we develop at. That is the same blind spot
[ADR-0037](0037-internal-grid-is-a-resolution-not-a-shape.md) records for geometry, where an
internal grid and a render target happened to agree at 16:9 and no test at that aspect could tell
which one the code had used. Here it is not two sources agreeing on one configuration; it is one
number that only exists on one configuration. The generalization is the same, and it is worth
stating once rather than rediscovering per subsystem.

This is a real decision because the obvious repair — re-measure the constants per target and keep
asserting them — is available, cheap, and wrong in a way that takes another runner-image change to
expose.

## Decision

A numeric assertion in this suite states **either a property or a measurement**, and says which.

A **property** holds on every configuration CI runs. It is dimensionless (a ratio against a
control taken in the same run, on the same adapter, so the machine cancels), or exact (the defect
it guards produces an exact zero or an exact equality, so no threshold is needed), or carries a
tolerance derived from the mechanism rather than from an observed run. A property-form assertion
runs everywhere and is expected to pass everywhere.

A **measurement** is a frozen number from a specific configuration. It is legitimate, and it stays
— but the test **names that configuration and does not run outside it**, skipping with a printed
notice in the [ADR-0016](0016-gpu-tests-opt-in-ci-scope.md) shape. A skipped measurement prints
what it observed instead, so the configuration it declines to gate is still visible in the log.

A frozen number asserted universally is neither, and is the thing this ADR forbids.

One corollary is load-bearing enough to state separately: **a threshold at or below this
project's own declared noise floor for the same quantity is not a property.** `MEAN_TOL = 0.02`
is what `golden.rs` calls rasterizer drift for a mean channel difference; an assertion on that
same statistic below `0.02` is measuring the noise, whatever it claims to measure.

## Consequences

### Positive

- Whether `main` is green stops depending on which machine measured a constant. Both live failures
  get repairs that follow from a rule instead of from a nudge.
- The rule is checkable at design time. A done-when carrying a bare number now has a question to
  answer — property or measurement? — and the second answer obliges naming the machine.
- The ratio form generalizes. Any future differential pixel test has a control available: the same
  run with the variable held, which scales with the adapter exactly as the signal does.
- Comparing a proposed threshold against `MEAN_TOL` is a one-line check that would have caught the
  render test at review.

### Negative

- **A pinned measurement buys honesty by giving up coverage.** After this, nothing checks the raw
  band path on aarch64. That is real: the raw levels are the escape hatch
  [ADR-0049](0049-analysis-v2-dual-resolution-axis-normalized-bands.md) added, and macOS will no
  longer defend them. Mitigation is partial and should not be oversold — the portable sibling
  (`normalized_levels_are_portable_across_absolute_gain`, tolerance `1e-4`) does run on arm64 and
  passes, so the analyzer is exercised there; only the bit-frozen reference is not.
- **A skipped test is a silent no-op**, ADR-0016's known cost, inherited here. The printed
  observation is the mitigation and it is weaker than an assertion.
- A ratio is harder to read than an absolute floor, and it needs a control that is itself
  non-trivial. If a future control collapses toward zero the ratio becomes unstable in a way a
  bare threshold would not have been — so the control's own magnitude has to be asserted too.
- A property-form contract can be **weaker** than the measurement it replaces. Exact-zero
  positivity states precisely the defect and nothing about magnitude; recovering the magnitude
  claim costs a second, calibrated assertion.

### Neutral

- No production code changes and no CI workflow change. The rule lives entirely in test code and
  in what a plan is allowed to write as a done-when.
- The C ABI, the audio path, and every shipped preset are untouched.

## Alternatives considered

### Alternative A — Re-measure the constants per target and keep them absolute

Keep bit-exact literals and the `0.01` floor, but carry a `cfg(target_arch)` table of expected
values and a per-runner threshold. Rejected because it is the same machine-pinning multiplied by
the number of configurations, and it inverts the maintenance cost: every new runner image, every
new architecture, and every `rustfft` version bump needs a fresh measurement round trip before
`main` can be green. It also does not survive its own premise — the CI WARP build is not a stable
identity we can pin against, as the 10.0.19041 / 10.0.26100 split here demonstrates.

### Alternative B — Loosen every threshold until all runners pass

Lower the render floor under the CI reading and widen the DSP comparison to a tolerance that
admits the arm64 values. Rejected because it sets numbers from what happens to pass rather than
from what the claim needs. For the render test the arithmetic makes it concrete: a floor beneath
the observed `0.0059` sits at less than a third of `MEAN_TOL`, which is a test that cannot fail
for the reason it exists — the noise alone would satisfy it.

### Alternative C — Delete both tests

Both defend claims that matter: the raw levels are ADR-0049's escape hatch and the reason
`*_raw` exists at all, and dual-live's entire point is that the outgoing visual keeps rendering
through a dissolve. Rejected because the failures are in how the claims were written down, not in
whether they are worth defending.

## Notes

Measurements taken 2026-08-04 at `4ab383c`, on the software adapter
(`Microsoft Basic Render Driver`, DX12, driver 10.0.19041.5794):

| statistic | local WARP | CI WARP (10.0.26100) |
|---|---|---|
| `frame_diff(frozen[20], live[20])` — what is asserted | 0.0450 | **0.0059** |
| peak over the dissolve window | 0.1096 | not measured |
| mean over the dissolve window | 0.0714 | not measured |
| control: `frame_diff(frozen[0], frozen[20])` | 0.2853 | not measured |
| control peak: `max frame_diff(frozen[0], frozen[i])` | 0.4078 | not measured |
| ratio, peak signal / peak control | **0.269** | not measured |
| `golden.rs` `MEAN_TOL`, for scale | 0.02 | 0.02 |

The control is 20x the declared noise floor, which is what makes it usable as a denominator.

DSP divergence, `bass_raw` at hop 200: expected `0x3865_9855` = `5.473972490e-5`, observed on
`macos-26-arm64` `5.473926290e-5` — absolute `4.6e-10`, relative `8.4e-6`. The other three
literals' arm64 values are **unknown**: `assert_eq!` inside the loop stops at the first mismatch.
`onset_raw` (`2.502e-7`) is the one to watch, being small enough that a difference-derived value
can lose relative precision badly.

Corroborating evidence that the arm64 divergence is precision and not breakage: the macOS run
completes 253 other tests green, including
`normalized_levels_are_portable_across_absolute_gain`, which asserts a `1e-4` agreement through
the same analyzer.
