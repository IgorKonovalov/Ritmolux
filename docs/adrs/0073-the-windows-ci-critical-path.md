# ADR-0073 — The Windows CI critical path: the sweep gets one owner, and a shape claim stops sweeping

> **Status:** proposed
> **Date:** 2026-08-04
> **Related plan(s):** [0061](../plans/0061-the-build-stops-paying-for-what-it-is-not-building.md)
> **Supplements:** [ADR-0033](0033-testing-strategy-coverage-ratchet-and-pre-push-gate.md)
> **Related:** [ADR-0016](0016-gpu-tests-opt-in-ci-scope.md),
> [ADR-0023](0023-golden-drift-guard-uses-frozen-fixtures.md),
> [ADR-0053](0053-plan-lanes-run-in-git-worktrees.md),
> [ADR-0071](0071-a-numeric-test-contract-states-a-property-or-names-its-machine.md)

## Context

[ADR-0033](0033-testing-strategy-coverage-ratchet-and-pre-push-gate.md) placed the coverage ratchet
in its own CI job and wrote down why: *"Separate job, so it stays off the critical path even though
instrumented binaries make the WARP suites slower."* That sentence had never been tested against a
measurement, because until 2026-08-04 there was no green CI run to measure. The five pushes since
2026-07-30 all died inside `cargo nextest run`, and nextest's fail-fast cancellation made the red
runs read as ~4 minutes — fast-looking runs that were not runs at all. Plan
[0060](../plans/done/0060-a-test-number-states-a-property-or-names-its-machine.md) turned the gate green;
run **30903871856** is the first complete measurement this project has ever had of its own CI.

| Job | Runner | Wall | Of which `nextest` |
|---|---|---|---|
| `check` | macos-latest | 3m23s | 132.9 s / 460 tests (every GPU suite skips, ADR-0016) |
| `check` | windows-latest | **> 20m** | ≈ 1555 s, derived below — essentially the whole job |
| `coverage` | windows-latest | **15m32s** | 726.6 s / 368 tests |
| `miri` | ubuntu-latest | 2m19s | — |
| `deny` | ubuntu-latest | 19s | — |

**Two independent findings came out of it, and they are not the same finding.**

### The shipped preset library is rendered twice per push, in parallel, on two identical runners

Both Windows jobs run on `windows-latest`, and **both execute the same `lmv-core` WARP preset-sweep
suites** — `check` uninstrumented across the workspace, `coverage` instrumented under `-p lmv-core`.
The eight slowest tests in the coverage job — reactivity 409.6 s, animation 364.4 s, sanity 315.2 s,
reaction_diffusion 237.9 s, attractor_contract 188.5 s, sanity/shape 185.0 s, attractor_aspect
114.8 s, distinctness 114.3 s — total ≈ **1930 CPU-seconds**, and that ≈ 1930 is spent a second time,
concurrently, a few metres away.

**The exclusion that fixes it already exists, and it already names exactly these tests.**
`.githooks/pre-push` carries a `binary(...)` filter excluding nine suites — `golden`, `attractor`,
`reaction_diffusion`, `background_composite`, `ink`, `reactivity`, `animation`, `sanity`,
`distinctness` — chosen in Plan 0032 by *local* measurement, with the times in the hook's own comment
(reactivity 89 s, animation 73 s, sanity 46 s, distinctness 41 s). Every one of the eight slowest CI
tests falls inside one of those nine binaries: `attractor_contract` and `attractor_aspect` are tests
within `attractor`, `sanity/shape` within `sanity`. A list chosen on a different machine against a
different adapter at a different scale names the same suites — independent corroboration that the
cost is a property of the sweeps, not of the runner.

**Only one of the two jobs can give up the sweep.** The coverage number is line coverage of
`lmv-core`, a crate that is mostly render code; a coverage job that does not render would collapse
the percentage and make the ratchet meaningless. So `check (windows-latest)` is the job that stops
sweeping.

### The critical path is one test, and it is not in the duplication at all

`standalone::shot_cli::the_json_report_is_well_formed_and_carries_its_top_level_keys` runs **948.9 s**
on `check (windows-latest)` — **61 %** of that job's `nextest` wall clock, which puts the whole step
at ≈ 1555 s and makes this single test the run's critical path. It costs **85 s** on the development
machine: a **11.2x** runner penalty against the ≈ 4.6x the WARP suites pay, because it is a
*subprocess* sweeping the library serially, so `nextest`'s parallelism cannot touch it.

It lives in `standalone/tests/`, so `coverage`'s `-p lmv-core` never runs it and it is **not** part of
the duplication above. It is a *third* rendering of the shipped library per push, on top of the two.

What it asserts is that `--report --json --presets presets` exits zero, emits balanced JSON, and
carries the top-level keys `source` and `families`. **None of those three claims depends on the preset
count.** The sweep is how the test is written, not what it proves — and the neighbouring test
`the_report_transient_columns_separate_the_two_easing_fixtures` already demonstrates the alternative,
pointing the same CLI at `core/tests/fixtures/` because two twin fixtures are the right subject for
its claim.

**These two findings have opposite shapes, and only one of them was visible from the coverage job.**
The duplication is ≈ 1930 CPU-seconds paid twice in parallel — a *cost* finding, worth little wall
time on its own. The `shot_cli` sweep is 948.9 serial seconds on the long pole — a *wall time*
finding that costs nothing extra in duplication. Fixing either alone leaves the other in place.

## Decision

We take both, as one decision, because they are the same principle applied twice: **stop paying again
for evidence the run already has.**

**One.** `check (windows-latest)` stops running the nine GPU-heavy suites, using the same
`binary(...)` exclusion `.githooks/pre-push` already carries, and the `coverage` job becomes the
single place the Windows GPU tier is proved. `check` keeps proving everything else on Windows — the
non-GPU suites, `core/tests/chain.rs`, `standalone/tests/shot_cli.rs`, `lmv-ring`, doctests,
`clippy --all-targets -D warnings`, `fmt`. This retires ADR-0033's *"separate job, so it stays off
the critical path"* as a statement of intent: the coverage job is now load-bearing for correctness,
not only for the ratchet, and a red `coverage` means either *a test failed* or *the floor was missed*.

**Two.** `the_json_report_is_well_formed_and_carries_its_top_level_keys` is scoped to a small fixture
directory instead of `presets/`. A claim about the *shape* of an output does not sweep the library the
output happens to describe — it needs enough presets to make the shape non-degenerate and no more.
Concretely: **at least two distinct `SystemKind`s**, so the `families` key is genuinely plural and the
grouping it names is exercised rather than assumed.

Two properties bound the whole change, and both are checkable rather than asserted:

- **No test loses its Windows run.** The union of the two Windows jobs' `cargo nextest list` output is
  identical before and after. This is the entire safety argument for part one, and it is a diff, not a
  claim.
- **`coverage` ends up the longest job in the workflow.** This is the property to check *after both
  parts*, and deliberately not after either alone: part one leaves `shot_cli`'s 948.9 serial seconds
  as a floor under `check`, which is within noise of `coverage`'s whole 932-second job, so after part
  one the long pole is a coin toss. **No target wall time is stated**, because none has been earned —
  the runner is shared, `nextest` parallelism differs between the two jobs (726.6 s of wall against
  ≈ 1930 CPU-seconds implies ≈ 2.7 tests in flight), and the change lands beside a profile edit that
  invalidates the build cache. The plan records what it measures; it does not aim at a number.

The exclusion list lives in **one** place in `.github/workflows/ci.yml`, commented with this ADR's
number, so "which job proves the goldens" is answerable by reading the workflow.

## Consequences

### Positive

- **The shipped library is rendered once per push instead of three times.** Two of the three
  renderings were paying for evidence the third already produced.
- **The critical path stops being a test that does not need to be on it.** 948.9 s of serial
  subprocess rendering, for a claim about balanced braces and two key names.
- **Cheap failures still report fast.** `fmt`, `clippy` and the non-GPU tests stay in a job that is
  not gated behind a 12-minute sweep, so a formatting error is red in minutes rather than at the end
  of the longest run. This is the property Alternative A gives up.
- **CI and the pre-push hook share one list.** The nine suites a developer skips locally are the nine
  CI concentrates in one job — same names, same measured reason, two files that can be diffed.
- **The ratchet keeps its meaning.** `coverage` still renders, so `lmv-core`'s line percentage still
  reflects a crate that is majority render code.
- **The pre-push hook gets faster too, without being edited.** `shot_cli` is not in the hook's
  exclusion list, so its 85 s local cost sits inside the ~98 s gate the hook advertises. Scoping the
  test is the single largest cut available to that number.

### Negative

- **The Windows GPU evidence now comes only from instrumented binaries.** llvm-cov instrumentation
  adds counters and does not alter float semantics or shader generation, so a golden baseline should
  be bit-identical either way — but that is an argument, not a measurement, and if it ever stops
  holding, a golden failure and an instrumentation artifact become indistinguishable. **The escape is
  one line**: drop the `-E` filter from `check (windows-latest)` and the uninstrumented run is back.
- **Disabling or breaking the coverage job now silently removes an entire test tier.** Before this,
  `check (windows-latest)` was a backstop; after it, a `cargo-llvm-cov` install failure or a
  commented-out job takes the golden guard with it and nothing says so. The mitigation is only a
  comment in the workflow naming what the job carries.
- **Nothing renders every shipped preset through the real CLI any more.** That is a genuine loss of
  coverage-in-the-informal-sense, and it is accepted on two grounds: the in-process suites still
  render every shipped preset on WARP in the `coverage` job, and Plan 0061 Phase 4 moves the report
  machinery into `standalone/src/shot/` where it gets `#[test]`s that actually run. **The order
  matters** — scoping the subprocess test before that move would leave the report generator's own
  logic tested only through a three-preset invocation. The plan sequences it accordingly.
- **A red `coverage` is now ambiguous at a glance** — a failed test and a missed floor produce the
  same red X and demand different responses. The step names distinguish them; the badge does not.
- **`.githooks/pre-push`'s comment becomes half-true.** It says *"CI runs the full suite on every push
  regardless"*. CI still does, but in one job rather than both, so the hook's promise that skipping
  locally costs nothing is now underwritten by a single job. The comment needs correcting or it will
  be read as describing a redundancy that no longer exists.
- **This makes the coverage floor a schedule risk as well as a quality one.** Under
  [ADR-0053](0053-plan-lanes-run-in-git-worktrees.md) a lane's close re-runs the gate; a floor miss
  now blocks on the same job that carries the goldens, so the two cannot be triaged independently.

### Neutral

- **The `check` matrix stays two-runner.** macOS is untouched: every GPU suite already skips there per
  [ADR-0016](0016-gpu-tests-opt-in-ci-scope.md), so the exclusion removes nothing it was actually
  running. The filter is applied on both arms rather than branched per-OS, because a per-OS branch in
  a matrix step is a second thing to keep true and buys nothing measurable.
- No test is deleted, no preset leaves any sweep, and no assertion about the library is weakened. What
  changes is **where** the sweep runs and **what one test's fixture set is**, not what the suite
  proves about the presets.

## Alternatives considered

### Alternative A — Merge the two Windows jobs into one instrumented run

Run `cargo llvm-cov nextest --workspace --no-report`, then `cargo llvm-cov report -p lmv-core
--fail-under-lines`, plus doctests, clippy and fmt, in a single Windows job. The truest
deduplication, and it saves more than test time — it also saves one entire Windows build of the
workspace, and `wgpu` + `naga` is the expensive part of that. Rejected on failure latency: it puts
`fmt --check` and `clippy` behind the longest test run in the workflow, so the cheapest and most
common failure in the repo is reported roughly twelve minutes later than today. It also instruments
`standalone/tests/shot_cli.rs`, which runs the built `shot` example as a subprocess — adding
llvm-cov's profile environment to a test whose whole point is to exercise the binary as a user
invokes it. Revisit if the surviving `check (windows-latest)` turns out to be dominated by its
*build* rather than its tests; that measurement is Plan 0061 Phase 9's, and it is the fact that would
flip this.

### Alternative B — Move `coverage` off branch pushes, keep the sweep in `check`

Run coverage on `main` pushes and pull requests only. Rejected by arithmetic: `check
(windows-latest)` is the > 20m job and this does not touch it, so it saves ≈ 31 Windows
runner-minutes and zero wall time. It also stops evaluating the ratchet on the pushes where code is
actually changing — precisely the state the last five pushes were accidentally in, where
`COVERAGE_FLOOR` went unenforced for a week and nothing surfaced it.

### Alternative C — Sample the preset sweep on branch pushes, sweep fully on `main` or nightly

Have `reactivity`, `animation`, `sanity` and `distinctness` render a representative subset per push
and the whole library on `main` and on a nightly cron. The largest possible saving, and it attacks the
real driver (presets × systems) rather than the duplication. Rejected (user call), for two reasons.
Mechanically, the expensive suites are **single tests** — `reactivity` at 409.6 s is one `#[test]`,
`every_preset_reacts_to_at_least_one_band` — so sampling cannot be a `nextest` filter; it must be an
environment variable read inside the assertion, which makes it an edit to what the test *claims*. And
in kind, this project's recurring failure is a defect shipping behind a green suite because *nothing
could render the configuration the defect occurs in* — ADR-0058, ADR-0067 and ADR-0070 are each that
story. Deliberately reducing what a push renders runs straight at it, and the miss would surface after
the fast-forward rather than before. **Note what this ADR does instead**, and why it is not the same
thing wearing a different hat: it removes *repeat* renderings and narrows one test whose claim was
never about the library. Every gate that asserts something about a preset still sees every preset.

### Alternative D — Narrow `coverage` instead and leave the sweep in `check`

The mirror image: let the uninstrumented job own the sweep and make the coverage job cheap. Rejected
immediately — `lmv-core` is majority render code, so a coverage run that skips the render suites
reports the render paths as dead and collapses the percentage. The floor would then have to fall to
accommodate its own instrument, which is the ratchet arguing with itself (ADR-0033 Alternative C's
failure mode in a different disguise).

### Alternative E — Keep `shot_cli`'s full sweep and buy the wall time with a bigger runner

Rejected: it treats a duplication as a capacity problem, paying three times for the library on faster
hardware and leaving the structural finding in place. A self-hosted Windows runner is also a machine
to maintain and a secret to manage, against a repo whose stated posture is that every added cost gets
argued (NFR §4).

### Alternative F — Keep `shot_cli`'s sweep but shrink each render (`--size 32x32`, fewer frames)

Reduce the per-preset cost rather than the preset count, the way
`the_presets_flag_is_reported_as_the_source` already does for its single capture. Rejected as the
*primary* answer because it leaves the test's cost proportional to a number that has grown with every
content plan and will keep growing — 35 presets today, and every future one silently buys another
slice of the critical path. It is worth taking *as well* if `--report` honours a size flag; it is not
worth taking instead.

## Notes

- The measurement is run **30903871856**, 2026-08-04, the first green CI run since 2026-07-30. There
  is **no green baseline before it** — the ~4-minute runs in the history are `nextest` fail-fast
  cancellations, not fast full runs, so "CI got slower" is not a claim this data supports. It is the
  first time the number has been visible at all.
- Coverage at that run measures **93.34 %** lines against `COVERAGE_FLOOR = 88`, so the floor has 5.3
  points of slack. Plan 0061 Phase 2 moves that number for an unrelated reason (removing `ffi.rs`
  from the gated crate), and Plan 0060 may have moved it first. Three plans touching one constant is
  worth naming; none competes with the others, and this ADR does not move it at all.
- A cache caveat that will mislead the first reader of the first post-change run: a `[profile.dev]`
  edit or a new workspace member invalidates `Swatinem/rust-cache` wholesale, so the run immediately
  after Plan 0061's Phases 1, 1b and 2 is a cold build and is **not** the steady state. Any wall-time
  number taken from it is wrong in the pessimistic direction.
- The 11.2x runner penalty on `shot_cli` against ≈ 4.6x on the WARP suites is itself a small finding:
  the gap is what serialization costs. `nextest` overlaps the suites across cores; a subprocess
  sweeping a directory in a loop gets one core and the runner's clock speed, and GitHub's is slow.
  Any future test that loops over the library inside one process inherits that multiplier.
