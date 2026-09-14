# ADR-0193 — A cost probe runs alone, because the suite around it is the contention

> **Status:** proposed
> **Date:** 2026-09-14
> **Related plan(s):** [0174](../plans/0174-the-cost-probes-run-alone.md)
> **Extends:** [0173](0173-a-cost-probe-takes-the-best-of-each-duration-not-the-best-difference.md)
> (a cost probe takes the best of each duration), [0156](0156-the-per-phase-gate-is-scoped-and-the-suite-is-owed-once-per-plan.md)
> (the per-phase gate is scoped)

## Context

Five test files price a rendering feature by timing it, six tests in all: `core/tests/arc_cost.rs`, `collage_cost.rs`,
`field_cost.rs`, `mark_cost.rs` and `path_cost.rs`. Each renders a short run and a long run, keeps the
minimum of each leg across `REPEATS`, and divides the difference by the frame gap. ADR-0173 moved
them onto that estimator and said plainly what it did not buy: *"`min(long)` can still fall below
`min(short)` on a machine contended enough ... a red probe on a shared runner remains a possible
outcome."* It then assumed where that machine is. Its Context says the questions *is this adapter
real?* and *is this machine quiet?* *"have the same answer on the reference machine"*.

**They do not, and the reference machine is where the probe went red.** On 2026-09-14 the pre-push
hook's `cargo nextest run --workspace -P fast` failed on the reference machine (AMD Radeon integrated,
DX12) at `rlx-core::path_cost the_contour_arity_is_priced_against_the_floor_tier`, after 34.2 s. The
same test run on its own took 16.0 s and passed. An immediate re-run of the whole `-P fast` profile
passed all 1620 tests. Nothing in the tree changed between the three runs, and the push under test
was docs-only.

The contention is the suite itself. nextest defaults to one test thread per logical CPU, and every
cost probe is in `-P fast` (ADR-0173 notes they sit outside the nine deferred suites). So each probe
is timed while every other slot runs a test binary. Many of those tests drive the same GPU through
their own headless renderer. The probe times wall-clock around `capture_preset`, which includes CPU
scheduling, driver queueing, and the one GPU readback. All of those are shared with the other slots.
A quiet machine is not a property of the reference hardware. It is a property of what nextest
schedules beside the probe, and nextest can be told to schedule nothing.

The failing run printed no panic text that survived the hook's output, so which of `path_cost`'s
three assertions fired is not recorded. All three are contention failures of the same estimator:
a non-positive reading, a figure indistinguishable from the control, or the ceiling measuring no more
than the coarse case.

## Decision

We will run every cost probe **alone** under nextest. `.config/nextest.toml` gains one override in
`profile.default`, which `profile.fast` inherits. The override selects the five binaries by an
anchored `binary(/_cost$/)` predicate and sets `threads-required = "num-test-threads"`. nextest then
starts a probe only when every test slot is free, and starts nothing else until it finishes. The
selection is by the `_cost` filename suffix, so a sixth probe named that way joins without an edit.
`"num-test-threads"` is chosen over `"num-cpus"` because it reads as the property wanted (*every
slot the run has*) and stays exact under an explicit `-j`. The estimator, the assertions and the
positivity guard from ADR-0173 are unchanged.

## Consequences

### Positive

- **The probe's own run stops being its contention.** On the reference machine a probe's reading
  in the pre-push gate becomes the reading it gives alone. That is the reading every number in these
  files' headers was taken from, so the report and the headers compare again.
- **One line, and it applies to every place the suite runs.** The pre-push hook, CI's `check` job,
  the `dev` lane's per-phase gate and the close's full `--workspace` run all read this profile.
- **It is a nextest scheduling property, not a code change.** No test is restructured, and ADR-0173's
  rejected Alternative A stays available and unused.

### Negative

- **The gate gets slower by the probes' serial time.** While a probe runs, every other slot idles.
  On the reference machine the six probe tests in the five binaries take **77 s** run one at a time
  (7.2 to 24.4 s each), and the whole `-P fast` run took 180 s and 241 s on the day of the failure.
  The added wall time is bounded by that serial figure. What it actually is depends on what the
  idled slots would have done, so it is unmeasured until Plan 0174 records it. It is paid on every
  push.
- **Alone within nextest is not alone on the machine.** A second worktree lane building, an IDE's
  `rust-analyzer`, or a browser still contend, and nothing here sees them. CI's shared runners keep
  exactly the exposure ADR-0173 recorded. This removes the contention the gate itself creates. It
  does not make a probe a property on every machine, and the positivity guard stays for that reason.
- **The selector is a naming convention.** A timing probe not named `*_cost` is not covered, and a
  non-timing test named that way is serialized for nothing. The filename is the only thing a static
  nextest predicate can read.

### Neutral

- The five probe files share clock-reading exemptions under `clippy::disallowed_methods`. Other files
  read a clock too (`core/tests/dsp.rs`, several `standalone` tests), but as timeouts or reports
  rather than as a priced slope, so this ADR does not reach them.

## Alternatives considered

### Alternative A — Retry the probes (`retries` on the same override)

Let nextest re-run a failed probe and report it as flaky. **Rejected because the retry runs under the
same contention and its reading is still what the report prints.** A pass on the second attempt is
a contaminated measurement that happened to clear its assertions. ADR-0071's point is that a
measurement is worth something only when its configuration is named, and a retried reading has no
nameable configuration.

### Alternative B — A test group with `max-threads = 1`

Put the five binaries in one `test-group` so no two probes overlap. **Rejected because it serializes
probes against each other only.** A group limits concurrency within its members, and the rest of
the suite still fills every other slot. The contention on the day of the failure came from the ~1600
tests around the probe, not from a second probe.

### Alternative C — Take the probes out of `-P fast`

Add them to the deferred set, so they run once per plan at the close's full `--workspace` run.
**Rejected because the full run schedules them beside the whole suite too**, and there it contends
with the nine heavy GPU suites `-P fast` leaves out. That moves the flake from the push to the close
and makes it more likely there. It also drops per-phase coverage of their portable assertions, which
ADR-0173 kept deliberately.

### Alternative D — ADR-0173's Alternative A: split the pixel properties from the timing

Assert the portable properties everywhere and gate the timing report to the reference machine.
**Rejected here for a stronger reason than 0173 gave:** the failure was on the reference machine. A
reference-machine gate would still run the timing beside the suite and would still have gone red.

## Notes

Filed alongside, not decided here: `path_cost.rs`'s arity probe draws a curved leaf, and its fit
collapses to 16 arc pieces. `PathShape` keeps a fit only when `pieces * 2 <= samples`. So at
`samples` of 32, 48 and 64 the probe prices the same arc chain, not the polyline arity its header
table reports. Backlog 0217 holds that.
