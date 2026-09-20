# ADR-0222 — A preset sweep's fixed cost is paid per process, so the lever is the batch

> **Status:** accepted 2026-09-19 (Plan 0199) — carries an Outcome
> **Date:** 2026-09-19
> **Related plan(s):** [0199](../plans/done/0199-the-gates-cost-is-measured-before-it-is-cut.md)
> **Amends:** [0157](0157-the-preset-sweeps-split-per-preset-and-the-phase-tier-samples-a-declared-representative.md)
> (what a sweep's unit is), and rests on
> [0156](0156-the-per-phase-gate-is-scoped-and-the-suite-is-owed-once-per-plan.md)

## Context

Three per-preset suites are 54 % of the workspace suite. Measured from
`0175-remerge-18-cargo_nextest.log` on 2026-09-15, over 7378 test-seconds and 73 binaries:
`reactivity` 1566 s (21 %), `animation` 1291 s (18 %), `sanity` 1099 s (15 %). They are the three
that iterate the shipped library one preset at a time, and each testcase stands up its own adapter,
device and pipeline set — so the cost grows with the library, which is the thing this project exists
to grow (backlog 0239).

[ADR-0157](0157-the-preset-sweeps-split-per-preset-and-the-phase-tier-samples-a-declared-representative.md)
made that split deliberately: a nextest predicate matches binaries and test names and cannot read a
`.toml`, so the per-preset name is the only thing a static profile can select on, and the split is
what lets `-P fast` sample declared representatives instead of skipping the sweeps whole. Nothing in
that decision was about cost, and this one does not reopen it.

Backlog 0239's leading shape was a `OnceLock`-held context the per-preset cases share, so the
adapter is built once per binary instead of once per preset. **Its premise is that the cases share a
process, and under nextest they do not.** cargo-nextest's execution model is one process per
testcase; that is the property the tool is built around, and it is why a test that reads the clock
can be given the whole machine at all
([ADR-0193](0193-a-test-that-reads-the-clock-runs-alone.md)). In-process sharing is therefore not
available to a per-testcase split, on any amount of code. What remains is the unit: how many presets
one testcase covers.

The second half of that decision is what the unit costs elsewhere. A sweep whose unit is a batch
reports a failure by naming the preset in an assertion rather than in a test name, and it renders
several presets through one device, which raises a question no per-preset run has to ask: whether
preset B renders the same after preset A in the same process as it does alone.

## Decision

We will treat a per-preset sweep's fixed cost as a **per-process** cost and make the **batch** the
lever: a sweep's unit becomes a group of presets rendered in one testcase, sized from the measured
fixed cost, with the declared representatives ([ADR-0157](0157-the-preset-sweeps-split-per-preset-and-the-phase-tier-samples-a-declared-representative.md))
kept as a batch of their own so the phase tier's filter keeps selecting exactly what it selects
today. Independence is not assumed: a batched render must equal the per-preset render for the same
preset, and that equality is asserted rather than hoped for. We will not pursue a shared device
inside a binary, because nextest gives a testcase its own process.

## Consequences

### Positive
- The fixed cost falls by roughly the batch factor on the three heaviest suites, and it stops
  growing one adapter per preset as the library grows.
- `-P fast` is unaffected by construction: the representative batch carries the same presets the
  representative tests carry now, under a name the same predicate matches.
- The independence question becomes an assertion. Today nothing in this repository knows whether two
  presets rendered through one device are independent, because nothing has ever rendered two through
  one.

### Negative
- **A failure names its preset in a message rather than in a test name.** Every tool that reads
  nextest's per-test output — the conductor's `failingTests`, a JUnit report, a CI annotation — gets
  a batch name, and the preset is one level in. That is a real loss of resolution at the exact moment
  someone is debugging.
- **A batch fails whole.** One preset's red stops its batch, so a run reports fewer independent
  failures per pass than a per-preset split does.
- **Batching re-opens a question ADR-0157 closed by construction.** The per-preset split made cross
  preset interference impossible; the batch makes it possible and then asserts it does not happen.
  An assertion is weaker than an impossibility, and that is the trade.
- The batch size is a constant chosen from one machine's measurement, and it will be wrong on a
  machine whose fixed cost differs — a slower figure means the batch should have been larger.

### Neutral
- The test count falls, so nextest's own summary line moves. Nothing reads it as a coverage figure.

## Alternatives considered

### Alternative A — A shared device per binary, via `OnceLock`
Backlog 0239's leading shape, and the one this ADR exists to retire: it rests on the per-preset cases
sharing a process, and nextest gives each testcase its own. Rejected as unavailable rather than as
unwise — under `cargo test`'s thread-per-test model it would work, and switching runners to get it
would give up the profile definitions, the per-test isolation and the clock-alone override that three
accepted ADRs rest on.

### Alternative B — Sample at the gate, cover the library nightly
Keep the per-preset split, run the representatives at every gate, and cover the library whole on a
schedule. Cheapest to build and it moves a class of failure from "before the merge" to "the next
morning". Rejected because the nine GPU suites already do not run in CI (no GPU — ADR-0016), so the
conductor's gate is the only place they meet a finished tree; deferring them further would leave the
full library tested nowhere that blocks anything.

### Alternative C — Retire the split, one testcase per suite
The pre-ADR-0157 shape: one testcase iterating the library in-process. It is the batch taken to its
limit and it undoes the sampling the phase tier now depends on, since a single testcase cannot be
filtered down to a representative. Rejected for that; the batch keeps the filter and takes most of
the saving.

## Notes

The measurement this decision is sized from does not exist yet: Plan 0199 Phase 1 measures the fixed
cost per testcase and the process model on the pinned cargo-nextest (0.9.140) before any batch size
is chosen. If that measurement finds the fixed cost is a small share of a sweep's time, the premise
of backlog 0239 falls with it and this ADR should be superseded rather than implemented — that
outcome is written into the plan as a stop condition.

**The interaction with [ADR-0211](0211-a-green-suite-record-serves-a-later-tree-when-no-deferred-suite-can-read-the-diff.md)
is not additive.** ADR-0211's saving is the difference between `-P fast` and the full suite; the
cheaper the full suite gets, the less it buys. Whichever lands second is re-measured, not assumed.

## Outcome (2026-09-19, Plan 0199)

The measurement this decision was sized from now exists, and it landed. Three corrections to what is
written above, recorded here rather than by editing it.

**The stop condition did not fire, and the premise held with room.** The fixed part — the process
plus the adapter, device and pipeline set — is 38-55 % of a per-preset testcase's wall on the
reference machine through WARP, 683 s of a 1522 s serial cost over the 114 shipped presets. `BATCH`
is 8, chosen at the knee of `B x variable / (fixed + process + B x variable)` and capped by
granularity rather than by the arithmetic: 15 scheduling units per sweep against 16 test threads.

**The first Negative overstated the loss, and the second is wrong as written.** A batch does *not*
fail whole: each sweep's per-preset helper returns `Option<String>` and the batch `filter_map`s the
convictions before asserting, so every preset is measured and every conviction is reported in one
message. What is genuinely lost is resolution in the *test name*, which the first Negative states
correctly and which stands.

**The saving is serial and has not yet been seen in the parallel suite.** `reactivity` re-timed
serially is 295.2 s against a library-corrected 514.7 s, a 43 % cut. The full workspace suite on the
closing tree is 769 s, inside the 725-841 s band of the five comparable runs before it — so the
positive Consequence above is confirmed as a *serial* reduction and is not yet demonstrated as a
wall-clock one under 16-way scheduling. Plan 0199's `## Followups` carries the re-measurement, and
it is the same re-measurement the ADR-0211 note above already asks for.
