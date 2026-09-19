# 0199 — The gate's cost is measured before it is cut

> **Status:** in-progress
> **Created:** 2026-09-19
> **Approved:** 2026-09-19 (user)
> **Owner skill(s):** dev
> **Related ADRs:** [0222](../adrs/0222-a-preset-sweeps-fixed-cost-is-paid-per-process-so-the-lever-is-the-batch.md)
> (proposed), [0193](../adrs/0193-a-test-that-reads-the-clock-runs-alone.md),
> [0157](../adrs/0157-the-preset-sweeps-split-per-preset-and-the-phase-tier-samples-a-declared-representative.md),
> [0156](../adrs/0156-the-per-phase-gate-is-scoped-and-the-suite-is-owed-once-per-plan.md),
> [0211](../adrs/0211-a-green-suite-record-serves-a-later-tree-when-no-deferred-suite-can-read-the-diff.md)
> **Closes:** design-backlog 0221, 0239

## TL;DR

Two costs are known and neither is understood. The run-alone override adds 165 s to every `-P fast`
— every push, every phase gate, every CI `check` — and three per-preset sweeps are 54 % of the
workspace suite and grow with the library. Both backlog entries say the same thing before proposing
anything: measure the thing that decides how much any shape buys. This plan takes the two
measurements first, folds the exclusive testcases that are pure overhead, and batches the sweeps to
the size the measurement names. The first visible behaviour is a shorter `-P fast` on every push.

## Context & problem

**The override.** ADR-0193 gives each selected testcase every nextest slot, so the idle slots are
paid once per *testcase* rather than once per binary. Measured at Plan 0174's close, one run per arm
on the same tree: `-P fast` took 244.6 s without the override and 409.5 s with it, while the selected
set takes about 85 s when its tests run one at a time. There are 18 selected testcases, and 11 of
them are `help_cli` (8 tests, 0.3 s of work between them) and `stream_pipe` (3). Whether nextest's
queue holds unrelated tests behind a waiting exclusive one, or lets them fill the slots while it
waits, has never been checked — and that decides how much any repair buys (backlog 0221).

**The sweeps.** `reactivity`, `animation` and `sanity` are 1566, 1291 and 1099 test-seconds of a
7378-second workspace suite, and each of their testcases stands up its own adapter, device and
pipeline set. The cost grows one adapter per preset shipped (backlog 0239). The entry's leading shape
— a `OnceLock` context the per-preset cases share — rests on those cases sharing a process, and
nextest runs each testcase in its own.
[ADR-0222](../adrs/0222-a-preset-sweeps-fixed-cost-is-paid-per-process-so-the-lever-is-the-batch.md)
records that correction and makes the batch the lever instead.

## Decision

Measure both before changing either, because both entries say the fix cannot be sized without a
number nobody has. Then: fold the exclusive testcases whose content is not what makes them exclusive
(`help_cli`'s eight, `stream_pipe`'s three) rather than exempting them from ADR-0193's class — the
tests stay clock-guarded and honest, and the per-testcase overhead falls with the count. And batch
the three sweeps per
[ADR-0222](../adrs/0222-a-preset-sweeps-fixed-cost-is-paid-per-process-so-the-lever-is-the-batch.md),
keeping the declared representatives as their own batch so the phase tier's filter selects exactly
what it selects today. We rejected exempting `help_cli` (it weakens ADR-0193's Decision point 2 for
a cost that folding removes anyway) and a fractional `threads-required` (it is ADR-0193
Alternative B returning, and it needs an amendment this plan has no measurement to justify).

## Implementation phases

### Phase 1 — Measure what the override actually costs
- **Owner skill:** dev
- **What:** answer the question backlog 0221 says to answer first — whether nextest holds unrelated
  tests behind a waiting exclusive testcase or fills the slots while it waits — on the pinned
  cargo-nextest 0.9.140, and record `-P fast`'s wall time with the override, without it, and with
  the 11 cheap exclusive testcases removed from the selection, one run per arm on the same tree,
  back to back, on the reference machine.
- **Files touched:** none — the readings go in the implementation log.
- **Done when:** the log carries the three wall times and the queue answer, each with the machine and
  the tree they were taken on, in ADR-0071's shape for a measurement; and the log states plainly
  whether the cost scales with the *count* of exclusive testcases, since every later phase of this
  half rests on that.

### Phase 2 — Fold the exclusive testcases that are pure overhead
- **Owner skill:** dev
- **What:** `help_cli`'s eight testcases become one per timing-sensitive property rather than one per
  assertion, and `stream_pipe`'s three the same, so the run-alone class holds fewer, larger cases
  without losing a property. Nothing leaves ADR-0193's class.
- **Files touched:** `standalone/tests/help_cli.rs`, `standalone/tests/stream_pipe.rs`,
  `.config/nextest.toml` if the selection expression needs no change (state so if it does not)
- **Done when:** every property the folded tests asserted is still asserted, named in the log
  property by property; the count of selected exclusive testcases falls from 18 to the number the
  fold produces; `-P fast`'s wall time is re-measured on the same machine and the saving is recorded
  against Phase 1's baseline; and a deliberate regression in one folded property still fails, so the
  fold did not turn eight assertions into one weaker one.

### Phase 3 — Measure a sweep's fixed cost per testcase
- **Owner skill:** dev
- **What:** on the pinned cargo-nextest, confirm the process model a batch decision rests on (one
  process per testcase) and measure how much of a `reactivity`, `animation` and `sanity` testcase is
  adapter, device and pipeline construction versus the render and the assertions.
- **Files touched:** none necessarily — the readings go in the log.
- **Done when:** the log carries, per sweep, the fixed share and the variable share of a single
  testcase, on a named machine; and **the stop condition is evaluated in writing**: if the fixed
  share is a small part of a sweep's time, backlog 0239's premise is falsified, ADR-0222 is to be
  superseded rather than implemented, and Phases 4 and 5 do not run. Say which way it went.

### Phase 4 — The sweeps run in batches
- **Owner skill:** dev
- **What:** the generated per-preset tests in the three sweeps become batched testcases sized from
  Phase 3's measurement, with the ADR-0157 representatives in a batch of their own whose name the
  `-P fast` predicate matches unchanged. A failure names the preset it convicted in its message.
- **Files touched:** `core/build.rs`, `core/tests/reactivity.rs`, `core/tests/animation.rs`,
  `core/tests/sanity.rs`, `.config/nextest.toml`, `docs/testing.md`
- **Done when:** `-P fast` selects the same presets it selects today, shown by name; the full suite
  covers every shipped preset in each of the three sweeps, shown by count against the library; a
  preset made deliberately dead is convicted with its own name in the failure message; and the three
  sweeps' wall time is re-measured against the Phase 3 baseline on the same machine.

### Phase 5 — A batched render equals a solo render
- **Owner skill:** dev
- **What:** the independence ADR-0222 refuses to assume becomes an assertion: a preset rendered after
  another preset in the same process produces what it produces alone.
- **Files touched:** `core/tests/` (the sweep harness the batches share)
- **Done when:** for a named sample spanning the systems that accumulate state — a feedback world, a
  particle world and a plain one — the batched render matches the solo render within the project's
  own declared rasterizer-drift floor and no tighter (ADR-0071: a threshold at or below the noise
  floor measures the noise); and the test states in its own header what it would take for this to go
  red, so a future reader knows what it is guarding.

## Risks & open questions

- **Phase 3's stop condition may fire**, and then Phases 4 and 5 do not run and ADR-0222 is
  superseded by its own plan. That is the right outcome of a measurement and is written in rather
  than discovered.
- **Batching costs failure resolution** (ADR-0222's first Negative). The conductor's `failingTests`
  parser reads nextest's per-test lines and will report a batch; the preset is one level in, in the
  message. Worth knowing before the first red.
- **This plan's own gate gets faster as it runs.** Phases 2 and 4 change what `-P fast` costs, so the
  per-phase timings in the log are not comparable across phases. Each measurement names its tree.
- **The interaction with ADR-0211 is not additive.** The cheaper the full suite gets, the less a
  served record buys. Re-measure rather than add the two savings.

## What this plan does NOT do

- It does not touch ADR-0193's rule. No test leaves the run-alone class, and no fractional
  `threads-required` is introduced — that is Alternative B of ADR-0193 returning, and it would need
  an amendment this plan has not earned.
- It does not move any sweep to a nightly or scheduled run (ADR-0222 Alternative B): CI has no GPU,
  so the conductor's gate is the only place these meet a finished tree.
- It does not change what a sweep asserts. Only the unit a sweep runs in, and how many exclusive
  testcases the clock class holds.
- It does not take backlog 0094 (the `frame_ms_p99` tail), which is a measurement question about the
  player rather than about the gate.

## Implementation log

**Lane:** `plan-0199-the-gates-cost-is-measured-before-it-is-cut`, worktree
`C:\Users\Igor Konovalov\WORK\rlx-plan-0199`

| phase | owner | state | commit |
|---|---|---|---|
| 1 — Measure what the override actually costs | dev | done | 2e04f9a5 |
| 2 — Fold the exclusive testcases that are pure overhead | dev | done | committed with this row |
| 3 — Measure a sweep's fixed cost per testcase | dev | not started | |
| 4 — The sweeps run in batches | dev | not started | |
| 5 — A batched render equals a solo render | dev | not started | |

### Notes

#### Phase 1 — what the run-alone override costs

Machine: the reference machine (x86_64-pc-windows-msvc, 16 logical cores). Tree: `1df6ba33`,
lane worktree above, `target/` built once before the first arm and untouched between them.
Runner: cargo-nextest 0.9.140 (a9fef2964 2026-07-05), the pinned one. Command in every arm:
`cargo nextest run --workspace -P fast`, through the conductor's suite lock, one run per arm,
back to back, nothing else running on the box. Wall time is nextest's own `Summary [...]` figure,
which equals the span of its JUnit report to 0.1 s where both were captured.

A JUnit report was temporarily switched on (`[profile.fast.junit]`) for the three arms so the
per-testcase start times could be read; `.config/nextest.toml` was restored before the commit and
this phase touches no file.

| arm | `.config/nextest.toml` | selected run-alone testcases | wall | tests |
|---|---|---|---|---|
| A | as committed | 20 | **426.2 s** | 1709 run, 311 skipped, 0 failed |
| B | `threads-required = 1` | 0 | **277.1 s** | 1709 run, 311 skipped, 1 failed |
| C | `binary(help_cli)` and `binary(stream_pipe)` dropped from the filter | 8 | **382.9 s** | 1709 run, 311 skipped, 1 failed |

Arms B and C ran `--no-fail-fast` and each fails exactly one test —
`hygiene::every_clock_reading_test_is_scheduled_alone`, which reads `.config/nextest.toml` and
convicts the arm's own edit. No other test failed in any arm, including every clock-reading test
in arm B, where they ran under the full 16-way load.

**The selection is 20 testcases, not the 18 the plan's Context states**, and the cheap half is 12,
not 11: `help_cli` carries nine tests and `stream_pipe` three. Listed by
`cargo nextest list --workspace -P fast -E '<the override filter>'` on this tree.

**The queue answer: nextest holds.** From arm A's JUnit report, over the five contiguous blocks of
exclusive testcases: **zero** non-exclusive testcases started while a block was running, and none
started for 3.8 s, 6.9 s, 10.0 s, 17.2 s and 42.4 s respectively before each block opened — the
drain, during which 8 to 16 tests were still finishing and no new one was admitted. Arm B is the
contrast on the same window: with the override off, 130 non-exclusive testcases started while the
seven cost probes ran. So the slots sit idle through the drain; nextest does not fill them with
work further down its queue while an exclusive testcase waits.

**Whether the cost scales with the *count* of exclusive testcases: it does not — it scales with the
number of contiguous *blocks* and with the serialized work inside them.** Consecutive exclusive
testcases start with a 0.00 s gap in every block of arm A, so a drain is paid once at a block's
leading edge however many testcases follow it. The 12 cheap testcases were one block, opening at
387.0 s and closing at 394.7 s: 6.9 s of drain plus 7.7 s of serialized work, 14.6 s of the run's
own timeline.

Arm C moved the wall by 43.3 s against arm A, which is larger than that 14.6 s, and the excess is
not a per-testcase constant: the eight cost probes that ran in **both** arms took 134.1 s of
exclusive time in arm A and 93.1 s in arm C, a 30.5 s run-to-run spread on the same tree and the
same machine. One run per arm cannot separate the two, and the plan asked for one run per arm. The
mechanism above is read off start times rather than off the difference between arms, and it is the
part later phases can rest on. The ceiling a fold can reach on this reading is the cheap block's
own 14.6 s.

Serialized time in arm A, as the sum over blocks of (drain + work): 80.3 s of drain + 134.1 s of
work = 214.4 s of a 426.2 s run, against arm B's 277.1 s with no serialization at all.

#### Phase 2 — the fold

`.config/nextest.toml` **needed no change**: the run-alone filter names `binary(help_cli)` and
`binary(stream_pipe)`, so it selects whatever testcases those binaries hold, and
`hygiene::every_clock_reading_test_is_scheduled_alone` matches on the binary too.

The selection falls from **20 testcases to 13**, listed by
`cargo nextest list --workspace -P fast -E '<the override filter>'` on this tree: `help_cli` 9 → 3,
`stream_pipe` 3 → 2, the eight cost probes and the `dsp` case untouched.

| was | is now |
|---|---|
| `help_prints_the_roster_and_exits_zero` | `every_query_answers_within_the_bound_and_exits_zero` |
| `help_is_answered_even_beside_an_unrecognized_argument` | ” |
| `schema_answers_on_stdout_and_exits_without_starting_the_app` | ” |
| `an_unrecognized_argument_exits_non_zero_and_names_it` | `an_unhonourable_command_line_is_refused_before_anything_is_built` |
| `a_stream_only_flag_without_stream_exits_without_starting` | ” |
| `the_windowed_preset_flag_is_not_refused_for_a_missing_stream` | ” |
| `an_unknown_preset_exits_without_opening_a_window` | ” |
| `an_unknown_preview_sink_exits_without_starting` | ” |
| `list_presets_names_each_files_status_and_exits_zero` | unchanged, and still its own case |
| `a_bounded_run_writes_whole_frames_and_announces_the_geometry_first` | `whole_frames_reach_the_pipe_at_the_announced_geometry` |
| `an_explicit_size_overrides_the_preview_default` | ” |
| `a_stalled_reader_costs_no_frames_and_the_run_catches_up` | unchanged, and still its own case |

Every property, one line each, all still asserted and in the same order the spawns run:

`every_query_answers_within_the_bound_and_exits_zero` — `--help` and `-h` each exit 0; each within
`RESPONDS_WITHIN`; each prints `usage: ritmolux`; each opens with the `Ritmolux — ` banner; each
names `--osc` and `--sender`. `--help` beside a misspelt flag exits 0 and still prints the usage.
`--schema` exits 0; within `RESPONDS_WITHIN`; stderr empty; stdout is one `{`…`}` object; on one
line; carrying `"hash":"` and `"systems":[`.

`an_unhonourable_command_line_is_refused_before_anything_is_built` — `--ocs …` exits 2 naming
`--ocs` and the nearest flag `--osc`. `--definitely-not-a-flag` exits 2 naming itself. Each of
`--fps`, `--size`, `--sender=`, `--frames` alone exits 2 naming the missing `--stream`.
`--preset <unknown>` exits 2, does **not** name `--stream`, and names what was typed.
`--preset definitely-not-a-preset` exits 2 and lists the roster (`this launch holds`).
`--preview syphon` exits 2, within `RESPONDS_WITHIN`, naming both the value and the sink that
exists. The elapsed bound stays on exactly the one case that carried it.

`whole_frames_reach_the_pipe_at_the_announced_geometry` — the preview-default run puts
30 × 640×360×4 bytes on the pipe; the `stream` event carries width 640, height 360, fps 60,
format `rgba8`; `hello` is the first event; stdout is a whole number of frames; the cost line names
`pipe write` and `render+readback`. The `--size 320x180` run puts 8 × 320×180×4 bytes on the pipe
and its announcement carries width 320 and height 180.

`list_presets_…` and `a_stalled_reader_…` are untouched.

**The deliberate regression.** Three defects were introduced in `standalone/src/`, each behind a
property that sits *late* in a folded case, so a fold that stopped at its first spawn would have
passed. All three were convicted, and the probes were reverted with `git restore` before the
commit:

| probe | caught by | at |
|---|---|---|
| `--schema` also writes a line to stderr | `every_query_answers_…` (3rd spawn) | `--schema wrote to stderr, so a parent reading both streams sees noise beside the document` |
| the unknown-preset refusal drops its `this launch holds` roster line | `an_unhonourable_command_line_…` (5th spawn) | `the refusal did not list the roster` |
| `Show::emit_stream` announces 640×360 whatever it was given | `whole_frames_reach_the_pipe_…` (2nd run) | `the announcement does not follow --size` |

**Wall time.** Same machine and runner as Phase 1 (reference machine, 16 logical cores,
cargo-nextest 0.9.140), tree = this commit's parent plus this phase's two test files, `target/`
built before the run. `cargo nextest run --workspace -P fast --no-fail-fast` through the suite
lock: **397.8 s**, 1702 run / 311 skipped / 0 failed — against Phase 1 arm A's **426.2 s**,
1709 run. The test count falls by exactly the 7 the fold removed.

**That 28.4 s is not 28.4 s of saving, and the mechanism says so.** Phase 1 measured a 30.5 s
run-to-run spread on the exclusive block alone between arms A and C on the same tree, so a
single-run delta of this size is inside the noise. What the mechanism predicts is smaller: the
cheap testcases were one contiguous block costing 14.6 s of arm A's timeline (6.9 s of drain plus
7.7 s of serialized work), the fold keeps them one block, so the drain is still paid once and only
the per-process share of 7 fewer test processes is removed. The honest reading is that the fold
saves a few seconds of serialized work and cannot save more than the block's own 14.6 s, and that
one run per arm cannot resolve it. No further arms were run: the plan asks for one re-measurement
against Phase 1's baseline and this is it.

### Close triggers

- **`presets/` touched:**
- **Plan header `Closes:`** design-backlog 0221, 0239
- **What shipped:**
- **Operator docs touched:**
- **Backlog probes (`node scripts/check-backlog-claims.mjs`):**
- **Full suite:**
- **Outstanding `human` phases:**

## Followups (after this lands)

- Re-measure ADR-0211's saving against the new full-suite cost, per ADR-0222's closing note.
