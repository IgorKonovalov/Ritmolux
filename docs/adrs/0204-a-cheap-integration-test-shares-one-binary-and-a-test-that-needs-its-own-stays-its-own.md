# ADR-0204 — A cheap integration test shares one binary per package, and a test that needs its own binary keeps one

> **Status:** proposed
> **Date:** 2026-09-14
> **Related plan(s):** [0177](../plans/0177-the-test-tree-stops-costing-disk-and-touching-the-machine.md)
> **Extends:** [0156](0156-the-per-phase-gate-is-scoped-and-the-suite-is-owed-once-per-plan.md)
> (the per-phase gate is scoped), [0193](0193-a-test-that-reads-the-clock-runs-alone.md)
> (a test that reads the clock runs alone), [0165](0165-dependencies-compile-without-debug-info-and-one-line-buys-it-back.md)
> (dependencies compile without debug info)

## Context

Cargo links every file in a package's `tests/` directory into its own executable, and each one
carries the whole dependency graph. On Windows MSVC that means one `.exe` and one `.pdb` per file,
and nothing is shared between them. On 2026-09-14 the tree holds **59** integration test files, 48
in `core/tests/` and 11 in `standalone/tests/`, so an engine edit relinks 59 binaries. Backlog
0182 measured the payload before ADR-0165 at 40.5 MB `.pdb` plus 13.3 MB `.exe` for
`core/tests/easing.rs`, which does pure arithmetic. ADR-0165 took the `.pdb` to 15.0 MB. The count
it multiplies has not moved.

The textbook fix is one `tests/main.rs` per package that declares every other file as a module.
**That fix is not available here, because nextest selects tests by binary.** Two committed
selectors depend on binary names, and a predicate cannot name a module inside a merged binary:

- `profile.fast`'s `default-filter` excludes nine GPU-heavy suites by `binary()` (ADR-0156). If
  they were merged, the per-phase tier would widen to the whole suite. That regression would show up
  as a slow gate, not as an error.
- ADR-0193's override schedules the clock-reading tests alone with `binary(/_cost$/)`,
  `binary(help_cli)`, `binary(stream_pipe)` and `binary(dsp) & test(...)`. Its guard in
  `core/tests/hygiene.rs` holds each file carrying a `clippy::disallowed_methods` exemption to a
  binary named in that filter.

Two more files need a process of their own for a reason no selector records: `console_preview_memory`
and `frame_tap_memory` read process memory.

## Decision

We will fold every integration test file that **needs no binary of its own** into one test target per
package. The target is a directory Cargo discovers by itself, `tests/suite/main.rs`, with each
folded file as a module under it. `autotests` stays on. A file keeps its own target if **any** of
the following holds:

1. A committed nextest selector names it by `binary()`: the nine `default-filter` suites, and every
   binary in ADR-0193's override.
2. It reads the clock (it carries a `clippy::disallowed_methods` exemption), whether or not ADR-0193's
   override has scheduled it yet. `stream_show` and `control_loopback` are in this class while Plan
   0174 diagnoses them.
3. It reads a process-level quantity another test in the same process could perturb: memory,
   environment, working directory.

The set is **re-derived at implementation time** from `.config/nextest.toml` and a grep, not copied
from this document. Derived at HEAD on 2026-09-14, rule 1 keeps 15 core files and 2 standalone files
separate (the nine suites, five `_cost` probes, `dsp`, `help_cli`, `stream_pipe`). Rule 2 adds
`stream_show` and `control_loopback`, and rule 3 adds the two `*_memory` files. That folds **33 of
48** core files and **5 of 11** standalone files, which removes **36** links.

ADR-0193's guard widens so a `clippy::disallowed_methods` exemption anywhere under `tests/suite/`
fails the suite. A clock-reading test cannot enter the shared binary.

## Consequences

### Positive

- **An engine edit relinks 23 test binaries instead of 59** (at the 2026-09-14 count), with 36 fewer
  `.exe`/`.pdb` pairs on disk per generation. Linking dominates this workspace's cold path, and
  backlog 0184 found 4 to 8 retained generations of most test binaries, so every retained generation
  shrinks as well.
- **Neither selector needs an edit.** `default-filter` excludes by name and none of the excluded
  names moves. ADR-0193's override names only binaries that stay separate.
- **The default for a new file stays safe.** A new top-level `tests/*.rs` builds as its own target,
  which is today's behaviour: it costs a link and never goes silently unbuilt. The alternative was
  `autotests = false` with explicit `[[test]]` entries, where a new file that nobody declares is
  never compiled.

### Negative

- **Editing one folded test file recompiles all of its package's folded tests.** Today it recompiles
  one file. That trade is right only if engine edits dominate the loop, and Plan 0177 measures both
  arms instead of assuming it.
- **Every citation of a folded target breaks.** `cargo nextest run -p rlx-core --test line_joints`
  becomes a filter on `binary(suite)` and a `line_joints::` test-name prefix. On 2026-09-14 a grep
  of `docs/`, the skills, the hook, the workflows and the two crates' sources finds 33 such
  citations across ten folded targets (`line_joints` alone has eight), in test assertion messages
  and module headers as well as in prose. Closed plans keep theirs as history.
- **Test names gain a module prefix.** Name-based filters that use `test(...)`'s default `contains`
  match keep working. An anchored `test(/^.../)` regex over a folded test would not. None exists at
  HEAD, so a new one has to be written knowing this.
- **Under `cargo test` the folded tests share one process.** Nothing in this project runs
  integration tests under `cargo test` (CI runs `cargo test --doc` only), and rule 3 keeps the tests
  that would notice out of the shared binary. A contributor who runs plain `cargo test` gets
  threads, not processes.

## Alternatives considered

### Alternative A — One `tests/main.rs` per package, every file folded

Rejected because `binary()` cannot name a module. Folding the nine suites widens `profile.fast` to
the whole suite (the ADR-0156 regression), and folding the clock readers breaks ADR-0193's override
and its guard at the same time.

### Alternative B — `autotests = false` and an explicit `[[test]]` roster

This keeps every file in place and declares the merged target with `#[path]` modules. Rejected
because turning autodiscovery off turns a forgotten manifest line into a test that is never built.
That is a silent loss of coverage, where the chosen layout costs at worst one extra link.

### Alternative C — Leave the layout alone

ADR-0165 already cut the dominant `.pdb` payload by 63 %. Rejected **provisionally**, and the plan
can reverse this: if the measured engine-edit loop is not faster after the fold, Plan 0177's merge
phase reverts it, and this ADR is accepted with an `Outcome` that records the measurement.
