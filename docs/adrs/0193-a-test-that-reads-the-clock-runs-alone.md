# ADR-0193 — A test that reads the clock runs alone, and the lint exemption that marks it is what selects it

> **Status:** proposed
> **Date:** 2026-09-14
> **Related plan(s):** [0174](../plans/0174-the-clock-reading-tests-run-alone.md)
> **Extends:** [0173](0173-a-cost-probe-takes-the-best-of-each-duration-not-the-best-difference.md)
> (a cost probe takes the best of each duration), [0156](0156-the-per-phase-gate-is-scoped-and-the-suite-is-owed-once-per-plan.md)
> (the per-phase gate is scoped), [0071](0071-a-numeric-test-contract-states-a-property-or-names-its-machine.md)
> (a numeric test contract states a property or names its machine)

## Context

A test that asserts on wall-clock time asserts on the machine as well as the code. Its reading
includes everything else scheduled on the machine. Under nextest's default schedule, that is the rest
of the suite.

**This has now failed the gate at least five times, always on a test that reads the clock, and every
failure passed alone.** Until today it was recorded only in close notes, never in the backlog:

| when | test | in the red run | alone |
|---|---|---|---|
| Plan 0158 close | `control_loopback a_preset_datagram_selects_by_name` | failed, full suite | passed |
| Plan 0159 close | `stream_show every_system_is_reported_by_the_key_the_schema_labels_its_roster_with` | **303.1 s**, `left: 1, right: 12`, full suite | 3.06 s |
| Plan 0163 Phase 1 | `stream_show` (same test), then `control_loopback` (same test) | failed, `-P fast` | passed |
| 2026-09-14 | `path_cost the_contour_arity_is_priced_against_the_floor_tier` | 34.2 s, `-P fast` (pre-push) | 16.0 s |

A sixth, `animation_rep_shape_facet` at Plan 0158's close, reads no clock. It is recorded here and is
not part of this decision.

Plan 0159's Mode 4 review named the mechanism as a major: *"structurally flaky and nothing isolates
it ... `.config/nextest.toml` carries no `test-threads`, no `threads-required` on the soaks and no
`slow-timeout`"*. Nothing was filed, so nothing followed.

ADR-0173 repaired the cost probes' estimator and kept the exposure it could not repair: *"a red probe
on a shared runner remains a possible outcome"*. It assumed the reference machine was quiet. It is
not quiet during the gate, because the gate is the load. Every failure in the table above happened
on the reference machine.

**The clock-reading tests already mark themselves.** `clippy.toml` bans clock reads (NFR §6), so
every test that reads one carries `#[allow(clippy::disallowed_methods, reason = ...)]`. Outside
`#[ignore]`d measurements, on 2026-09-14 the allow appears in exactly these integration tests:

| file | what the clock bounds | reason class |
|---|---|---|
| `core/tests/{arc,collage,field,mark,path}_cost.rs` | a priced frame-time slope | measurement |
| `core/tests/dsp.rs` (`one_hop_analyzes_well_under_the_hop_interval`, fn-level) | per-hop analysis under 11 ms | budget |
| `standalone/tests/control_loopback.rs` | a 5 s UDP delivery on loopback | deadline |
| `standalone/tests/help_cli.rs` | a spawned `--help` exits within 1 s | deadline |
| `standalone/tests/stream_pipe.rs` | a spawned run finishes within 20 s | deadline |
| `standalone/tests/stream_show.rs` | 60 s per stderr line from a spawned player | deadline |

**One row is not like the others, and the arithmetic says it is probably not contention.** The
failing `stream_show` test spawns the player with `--frames 9000 --fps 30`, which is 300 s of
wall-clock pacing. It walks the system roster by sending one `ctl/preset` datagram per system and
waiting for the matching `preset` event. `wait_for`'s 60 s `recv_timeout` restarts on **every**
stderr line, not only on the wanted one, and returns `None` both on a timeout and when the pipe
closes. The red run's 303.1 s is therefore the child's whole 300 s run plus startup. The child kept
pacing frames and writing lines until `--frames` ran out, and it never reported the second system
it was asked for. That fits a lost or unanswered control ask far better than a starved process. It
is unverified, because the assertion's stderr dump was not kept. **If it is a dropped ask, running
the test alone would hide a defect on the control path the studio drives the player through.**

## Decision

We will run every clock-reading test **alone** under nextest, select them by name in one override,
and hold that list to the clippy exemption with a guard test.

1. **The schedule.** `.config/nextest.toml` gains one `[[profile.default.overrides]]` block, which
   `profile.fast` inherits. It sets `threads-required = "num-test-threads"`: nextest starts the test
   only when every slot is free and starts nothing beside it. `num-test-threads` is chosen over
   `num-cpus` because it names the property wanted, every slot the run has, and stays exact under an
   explicit `-j`. The filter names each binary. It names a single test where the exemption is
   fn-level, as in `dsp.rs`.
2. **The selector is the exemption, held by a guard.** A nextest predicate cannot read an attribute,
   so the list is explicit. A new check in `core/tests/hygiene.rs` enforces it in both directions.
   Every workspace `*/tests/*.rs` file carrying a `clippy::disallowed_methods` allow must be named in
   the override's filter, and every binary the filter names must still carry one. A clock read that
   is not scheduled alone fails the suite at the moment it is added, not at its first flake. The
   guard's only exemption list carries a reason per entry.
3. **The two control-path tests are diagnosed before they are scheduled.** These are `stream_show`
   and `control_loopback`. The second's only load-sensitive step is its 5 s wait for a `ctl/preset`
   datagram to reach the listener; its dissolve runs on a fixed `DT`. So both repeat offenders fail
   at the same step. `stream_show`'s wait first learns to bound the
   *wanted* line rather than any line, to tell a timeout from a closed pipe, and to keep what the
   child said when the walk stopped. It joins the override only if that evidence shows starvation.
   A lost, rejected or unanswered ask becomes a backlog entry instead, and the test stays on the
   guard's exemption list with that entry's number as its reason. A test that fails intermittently
   while running alone is evidence. A test that was isolated until it stopped failing is not.

The cost probes' estimator and assertions (ADR-0173) and every deadline constant are unchanged.
Isolation removes the load. It does not widen a bound to absorb the load.

## Consequences

### Positive

- **The gate stops being the contention for the whole class, not only for the test that failed
  today.** It schedules the timing-only tests alone at once. The four control-path failures in the
  table are handled by diagnosis first, so that isolation does not hide a defect.
- **The class cannot grow unscheduled.** A new timed test fails `hygiene.rs` until it is in the
  override. That is the step this repository keeps missing when a rule has no carrier: the 0159
  review named the defect, and nothing carried it forward.
- **A cost probe's gate reading becomes the reading it gives alone**, which is the configuration
  every figure in those files' headers was taken on.
- **One definition, inherited everywhere.** The pre-push hook, CI's `check` job, the `dev` lane's
  per-phase gate and the close's `--workspace` run all read these overrides.

### Negative

- **The gate gets slower.** While an isolated test runs, every other slot idles. It also waits for
  every running test to drain before it starts. On the reference machine, run one at a time, the six
  cost-probe tests take **77 s**. The deadline tests take **54 s**: `stream_show` 36.6 s over eight
  tests, `control_loopback` 8.7 s, `stream_pipe` 6.1 s, `dsp.rs`'s hop test 2.0 s, `help_cli` 0.3 s.
  That is about **131 s** in all. The whole `-P fast` run took
  180 s and 241 s on the day of the failure. The real increase depends on what drains and idles, and
  Plan 0174 measures it. It is paid on every push.
- **Alone within nextest is not alone on the machine.** Another lane building, an IDE, or a CI
  runner's neighbours still contend. ADR-0173's residual exposure stands for the cost probes, and so
  does its positivity guard.
- **In-crate test modules are outside the guard.** It scans integration test files, because an
  in-crate test runs in the library's own binary and isolating that binary is not an option. Today
  the one clock-reading test in `src/` is `#[ignore]`d (`warp_mesh`'s `mesh_cost_by_grid`). A
  non-ignored one added later would not be caught.
- **The exemption list is a way around the guard.** Every entry is a reason in the test source. It
  is reviewed like any other code, which is weaker than a gate.

### Neutral

- A deadline that still fails while its test runs alone is no longer plausibly contention. That makes
  such a failure more informative, not less.

## Alternatives considered

### Alternative A — Retry the timed tests (`retries` on the same override)

**Rejected because a retry runs under the same contention.** For a measurement, the retried reading
is still what the report prints, and it has no nameable configuration (ADR-0071). For a deadline, a
retry turns a genuine intermittent hang into a pass marked flaky. `stream_show` is the case where
that would be the wrong answer.

### Alternative B — A test group with `max-threads = 1`

**Rejected because a group only stops its members overlapping each other.** The recorded failures
were each one timed test against the rest of the suite. No two failed together.

### Alternative C — Widen the deadlines

Raise `DELIVERY`, `LINE_DEADLINE`, `RESPONDS_WITHIN` and the rest until load cannot reach them.
**Rejected because a bound that absorbs any load also absorbs the defect it exists to catch**, and a
wider bound does nothing for a measurement. `stream_show` shows the shape: its effective bound was
already the child's whole 300 s run, and the test still failed.

### Alternative D — Take the class out of `-P fast`

**Rejected because the full `--workspace` run schedules them beside the nine heavy GPU suites.** Two
of the five failures were in that run. Deferral moves the flake to the close and makes it likelier
there.

### Alternative E — A naming convention (`*_cost`, `*_deadline`) instead of the exemption

**Rejected because it asks for renames and still leaves a new timed test uncaught until someone
remembers the suffix.** The clippy exemption cannot be left off, because the lint refuses the clock
read without it. That makes it the one marker that is already complete.

## Notes

Filed alongside and not decided here: `path_cost.rs`'s arity probe prices the arc chain rather than
the polyline from `samples = 32` up (backlog 0217).
