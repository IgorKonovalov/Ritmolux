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
| 2 — Fold the exclusive testcases that are pure overhead | dev | done | e5a5da6e |
| 3 — Measure a sweep's fixed cost per testcase | dev | done | 0a048973 |
| 4 — The sweeps run in batches | dev | done | 4e621186 |
| 5 — A batched render equals a solo render | dev | done | committed with this row |

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

#### Phase 3 — what a sweep testcase spends on its adapter

Machine: the reference machine (x86_64-pc-windows-msvc, 16 logical cores), DX12 WARP — every one of
these sweeps builds through `common::headless`, which is `prefer_software`. Runner: cargo-nextest
0.9.140. Tree: this commit's parent, with a temporary `Instant` probe around `common::headless` and
around the whole helper in each of the three per-preset sweeps, printing the elapsed pair and the
process id; the probe was reverted with `git restore` before the commit and this phase touches no
file.

**The process model is one process per testcase, confirmed rather than assumed.** Every testcase
printed a different process id, and `nextest`'s own per-test wall exceeds the in-test total by
0.81–1.10 s in all twelve — the span outside the test function, which only exists because the test
function is the whole of a process.

Command: `cargo nextest run -p rlx-core -j 1 -E '<the twelve testcases>' --success-output immediate`
through the suite lock. **`-j 1` on purpose**: a first attempt selected all 84 representative
testcases at the default 16-way concurrency and every one of them was still running past 120 s,
because selecting only the GPU sweeps puts 16 WARP devices on 16 cores at once — a reading about
that contention and not about a testcase. It was stopped and re-taken serially. The absolute times
below are therefore *floor* times, taken with the box otherwise idle.

Four presets spanning what accumulates state: `Leviathan` (attractor, compute particles), `Etching`
(reaction-diffusion, feedback field), `Loom` (parametric curve, lines), `Whorl` (fragment field,
no accumulation). `fixed` is `common::headless` — instance, adapter, device and the pipeline set;
`total` is the whole helper; `wall` is nextest's own figure for the testcase.

| sweep | preset | fixed ms | total ms | fixed share of total | wall s | process ms |
|---|---|---|---|---|---|---|
| animation | Leviathan | 2116.6 | 6385.2 | 33 % | 7.200 | 815 |
| animation | Loom | 818.9 | 2309.1 | 35 % | 3.116 | 807 |
| animation | Whorl | 781.0 | 1850.0 | 42 % | 2.659 | 809 |
| animation | Etching | 847.6 | 2983.5 | 28 % | 3.794 | 810 |
| reactivity | Leviathan | 802.4 | 7558.4 | 11 % | 8.479 | 921 |
| reactivity | Loom | 2247.5 | 4789.3 | 47 % | 5.887 | 1098 |
| reactivity | Whorl | 1003.0 | 2825.3 | 36 % | 3.688 | 863 |
| reactivity | Etching | 943.1 | 4090.4 | 23 % | 4.953 | 863 |
| sanity_loudness | Leviathan | 889.2 | 3851.2 | 23 % | 4.719 | 868 |
| sanity_loudness | Loom | 961.1 | 1907.7 | 50 % | 2.720 | 812 |
| sanity_loudness | Whorl | 1080.6 | 1919.8 | 56 % | 2.789 | 869 |
| sanity_loudness | Etching | 1001.7 | 2436.0 | 41 % | 3.386 | 950 |

Per sweep, over these four presets:

| sweep | fixed (mean) | variable (mean) | process (mean) | wall (mean) | fixed+process share of wall |
|---|---|---|---|---|---|
| animation | 1.141 s | 2.241 s | 0.810 s | 4.192 s | **46.5 %** |
| reactivity | 1.249 s | 3.567 s | 0.936 s | 5.752 s | **38.0 %** |
| sanity_loudness | 0.983 s | 1.545 s | 0.875 s | 3.404 s | **54.6 %** |

Read the two columns that matter together: **`fixed` alone is 26–39 % of a testcase's in-test time,
and the part a batch removes — the renderer build plus the process the testcase is — is 38–55 % of
its wall.** The spread across presets is real and is the variable half moving: `Leviathan`'s
reactivity testcase renders 120 attractor frames and spends 89 % of itself doing that, while
`Whorl`'s loudness testcase renders two fragment-field captures and spends 56 % of itself on the
adapter it built to do it.

**The stop condition: it does not fire.** The plan's condition is *"if the fixed share is a small
part of a sweep's time"*. It is not: the arithmetic over the shipped library is 114 presets ×
(fixed + process) = 249 s in `reactivity`, 222 s in `animation` and 212 s in `sanity_loudness`,
**683 s in total of a 1522 s serial cost for the three sweeps — 45 %.** Backlog 0239's premise
stands, [ADR-0222](../adrs/0222-a-preset-sweeps-fixed-cost-is-paid-per-process-so-the-lever-is-the-batch.md)
is to be implemented rather than superseded, and Phases 4 and 5 run.

**The batch size Phase 4 takes from this is 8.** A batch of `B` presets costs `fixed + process +
B × variable`, so its efficiency is `B × variable / (fixed + process + B × variable)`: at the
sweeps' means that is 87 % at `B = 4`, **93 % at `B = 8`**, and 96 % at `B = 16` — the knee is at 8,
and past it the return is bought with granularity. Granularity is the other constraint and it is
what caps the size rather than the arithmetic: 114 presets in batches of 8 is 15 scheduling units
per sweep against 16 slots, and a batch of 16 would halve that to 8 and leave slots idle on the
tail.

#### Phase 4 — the sweeps in batches

`build.rs` partitions the roster into the declared representatives and the rest, chunks each run
into batches of `BATCH = 8`, and emits `<sweep>_rep_batch_<nn>` / `<sweep>_batch_<nn>`, each handing
its sweep's `_batch` helper the display names it holds. The per-family sweeps (`sanity_shape`,
`distinctness`) keep the old one-test-per-family emitter untouched. The 114 shipped presets become
**15 testcases per sweep** — 11 ordinary batches over 86 presets, 4 representative batches over 28 —
where there were 114.

**`.config/nextest.toml`'s filter expression needed no change.** The `-P fast` predicate is
`test(/^(animation|reactivity|sanity_loudness)_rep_/)`, which matches `..._rep_batch_01` exactly as
it matched `..._rep_<stem>`. Its surrounding comment block did change, because it described the old
naming — including a hazard that the batching retires: a preset filed as `rep_*.toml` used to join
the sample without declaring the flag, and a batch name carries no filename, so the `.toml` flag is
now the only way in.

**`-P fast` selects the same presets, by name.** The four representative batches in each sweep hold
exactly the 28 presets the 28 `_rep_<stem>` testcases held, read off the generated file:

- batch 01 — Stained Glass, Standing Wave, Leviathan, Rho Walk, Ember Life, Spiral Bloom,
  Collage Mono, Suprematist
- batch 02 — Loom, Nightbloom, Heartfall, Perseids, Tiled Rosette Mono, Whorl, Coral, Rime
- batch 03 — Etching, Flux Mono, Contour Mono, Facet, Ridge, Skyline, Corona,
  Star Mandala Bordered
- batch 04 — Drift, Stipple, Cauldron, Millrace

**Coverage against the library: 114 presets in each of the three sweeps.** Counted from the runs'
own per-preset lines — `reactivity` printed 114 band vectors, `animation` 114 `frames 24/48:` rows,
`sanity`'s loudness gate 114 `excitation ratio` blocks — against the 114 `presets/*.toml` the glob
embeds.

**A deliberately dead preset is convicted with its own name.** `presets/curve_turnabout.toml` had
every audio reference stripped from its `[params]` (`n`, `pen`, `draw_progress`, `scale`, `hue`) and
its batch was run; it was reverted with `git restore` before the commit and this phase touches no
preset. `reactivity_batch_06` failed with

```
1 of the 8 presets in this batch react to no band:
Turnabout reacts to no band above 0.02 (per-band [("bass", 0.0), ("mid", 0.0), ("treb", 0.0), ("onset", 0.0)])
```

and the other seven presets in the batch were still measured and still printed their vectors, which
is the property the helpers' `filter_map`-then-assert shape exists for.

**Wall time, against Phase 3's baseline.** Same machine and runner as Phases 1–3. The comparable
measurement is a **serial** one, because Phase 3's baseline is serial: `cargo nextest run -p rlx-core
-j 1 -E 'binary(reactivity)'` through the suite lock, **299.0 s** for the binary, of which the 15
batches are **295.2 s** (the two non-generated tests in the file take 3.8 s). Phase 3 measured a
per-testcase wall mean of 5.752 s for `reactivity`, which over 114 per-preset testcases is 655.7 s.

**Believe the smaller of the two reductions.** Phase 3's four-preset sample overstates the library's
variable cost — `Leviathan` alone is 7.6 s — so 655.7 s is an overestimate of the "before". Solving
this run for the library's own variable cost gives `295.2 = 15 × 2.185 + 114 × V`, so `V = 2.33 s`
per preset, and the same 114 presets as separate testcases would have been
`114 × (2.33 + 2.185) = 514.7 s`. **295.2 s against 514.7 s is a 43 % cut on this sweep**; the
naive comparison against Phase 3's own walls reads 55 % and is the flattered one.

`animation` and `sanity`'s loudness sweep were run at `-j 4` to confirm green and count coverage
rather than to time them (160.2 s for the 32 testcases, not comparable to anything above). All
testcases in all three sweeps passed.

**`-P fast` did not get faster, and may have got slower.** `cargo nextest run --workspace -P fast
--no-fail-fast` on this tree: **412.1 s**, 1630 run / 86 skipped / 0 failed, against Phase 2's
397.8 s and 1702 run. The 72 fewer tests are the 84 representative testcases becoming 12 batches.
Two things are in that number and they pull opposite ways: the batches remove 72 renderer builds and
72 processes, and they coarsen the phase tier's scheduling from 84 units to 12 — a batch is one slot
for as long as its eight presets take, and a tail of twelve long units packs worse across sixteen
slots than a tail of eighty-four short ones. Phase 1 measured a 30.5 s run-to-run spread on this
machine, so a 14.3 s difference on one run per arm resolves neither effect. What the serial
measurement above does show is that the saving is real where the work is: the whole library, which
is the full suite's problem and not this tier's.

**One forward reference, disclosed.** `docs/testing.md` names
`core/tests/batch_independence.rs` as the guard on the independence a batch rests on. That file is
Phase 5's and does not exist at this commit; Phase 5's own file list is `core/tests/` and does not
include `docs/testing.md`, so this is where it had to be written.

#### Phase 5 — the independence is asserted

`core/tests/suite/batch_independence.rs`, one test:
`a_preset_renders_the_same_after_another_preset_as_it_does_alone`. Each subject is captured on a
renderer that has already rendered every subject before it — the batch — and on a renderer of its
own that has rendered nothing else, with the same roster loaded either way, so the only difference
between the arms is what rendered first. Both capture primitives the sweeps use are compared:
`capture_preset` (animation, sanity) and `capture_audio_after_warmup` (reactivity).

Subjects, in batch order, from `core/tests/fixtures/` rather than from the shipped library so a
content tune cannot reach this guard: `reaction_diffusion.toml` (a feedback world, whose field is
the previous frame), `attractor.toml` (a particle world, whose positions are a GPU buffer) and
`fragment_field.toml` (a plain one, integrating nothing, which is the control).

Threshold: `MEAN_TOL = 0.02` and `MAX_OUTLIER = 48`, the same two `golden.rs` compares a fresh render
against its committed baseline with (ADR-0023) — the project's declared rasterizer-drift floor, and
no tighter, per the plan and ADR-0071.

Measured on the reference machine through WARP, all six comparisons:

```
fixture_reaction_diffusion capture_preset             mean 0.000000 (tol 0.02) max_outlier 0 (tol 48)
fixture_reaction_diffusion capture_audio_after_warmup mean 0.000000 (tol 0.02) max_outlier 0 (tol 48)
fixture_attractor          capture_preset             mean 0.000000 (tol 0.02) max_outlier 0 (tol 48)
fixture_attractor          capture_audio_after_warmup mean 0.000000 (tol 0.02) max_outlier 0 (tol 48)
fixture_fragment_field     capture_preset             mean 0.000000 (tol 0.02) max_outlier 0 (tol 48)
fixture_fragment_field     capture_audio_after_warmup mean 0.000000 (tol 0.02) max_outlier 0 (tol 48)
```

Bit-identical on this adapter. The threshold stays at the drift floor regardless: a zero is a
reading about WARP today, not a licence to assert bit-equality.

**Non-vacuity, since six zeros invite the question.** The comparison was temporarily pointed at a
*different* subject's solo frame — the coarsest leak it could be asked to catch — and the three
separated at mean 0.221, 0.464 and 0.548 with outliers of 255, 206 and 217, which is 11x to 27x over
`MEAN_TOL`. The probe was reverted before the commit; its figures are recorded in the test's own
header, where the "what would take this red" section is.

**The test lives in `core/tests/suite/`, not as its own binary.** It reads no clock, reads no
process-level quantity, and no `binary()` selector in `.config/nextest.toml` names it, so ADR-0204's
rule puts it in the shared binary. The consequence is that it runs under `-P fast` on every push:
22.7 s under that run's load, 11.6 s alone.

**Deviation: `docs/testing.md` was edited in this phase, and it is not in Phase 5's file list.** One
word. Phase 4's own text named the guard `core/tests/batch_independence.rs` before it existed, and
ADR-0204 put it at `core/tests/suite/batch_independence.rs` instead; the path is corrected here
rather than left wrong in a shipped document, because Phase 4 is already committed and
`docs/testing.md` is not in Phase 5's list to begin with.

**Gate:** `cargo nextest run --workspace -P fast --no-fail-fast` through the suite lock — 368.4 s,
**1631 run / 86 skipped / 0 failed**. (Against Phase 4's 412.1 s on one fewer test; Phase 1's 30.5 s
run-to-run spread covers that difference, so read neither as a trend.)

#### Deviations and unmet done-whens

Everything not listed here passed as the phase stated it.

- **Phase 4's *"the three sweeps' wall time is re-measured against the Phase 3 baseline"* is
  satisfied for one of the three, not all three.** `reactivity` — the largest of them, 1566 of the
  three sweeps' test-seconds — was re-timed serially against Phase 3's baseline; `animation` and
  `sanity`'s loudness gate were run only to confirm green and count coverage, at `-j 4`, and their
  serial before/after is not measured. The reason is session time: a serial pass over all three is
  roughly 25 minutes of wall in a session whose individual commands are bounded at ten, and the
  mechanism is shared — one renderer build and one process per batch instead of per preset — so the
  second and third readings would confirm the first rather than test it. **What is therefore not
  known is each sweep's own ratio**, which Phase 3 shows varies (the fixed share is 38 % of a
  `reactivity` testcase and 55 % of a `sanity_loudness` one), so the 43 % cut measured on
  `reactivity` is not a figure to quote for the other two.
- **Phase 2's saving is inside the noise and the log says so rather than claiming it.** 397.8 s
  against 426.2 s on one run per arm, where Phase 1 measured a 30.5 s run-to-run spread on the same
  tree. The fold's mechanism caps it at the cheap block's own 14.6 s.
- **`docs/testing.md` was edited in Phase 5, which does not list it** — a one-word path correction to
  a filename Phase 4's own text named before the file existed. Written up under Phase 5.
- **`-P fast` is not faster at the end of this plan than at its start**, and both halves of the plan
  moved it. The three readings on this machine are 426.2 s (Phase 1, before anything), 397.8 s
  (after the fold), 412.1 s (after the batching) and 368.4 s (after the guard was added). The spread
  between any two of them is inside Phase 1's measured run-to-run spread. Where this plan's saving
  is real and large is the **serial** cost of the whole library, which is the full suite's bill and
  not this tier's.

No followup was noticed that is not already in the plan's own `## Followups`.

### Close triggers

- **`presets/` touched:** no — `git diff main...HEAD -- presets/` is empty. One preset was
  temporarily made dead inside Phase 4 to check a batch convicts by name, and reverted with
  `git restore` before that commit.
- **Plan header `Closes:`** design-backlog 0221, 0239
- **What shipped:** neither a feature nor a fix. Every commit is test harness plus the two documents
  that describe it: the run-alone class holds 13 testcases instead of 20, the three preset sweeps
  fan out in batches of eight instead of one testcase per preset, and one new guard asserts the
  independence that makes the batching sound. No engine, player, plugin or preset behaviour changes,
  and no assertion in any sweep changes.
- **Operator docs touched:** none. `docs/testing.md` is the contributor's harness page;
  `running.md`, `configuration.md` and `capturing.md` are untouched.
- **Backlog probes (`node scripts/check-backlog-claims.mjs`):** exit 0 — *55 stated reductions still
  hold across all 25 live entries (3 unprobeable)*. 28 advisory rows for probed paths that moved
  since their entry was stamped, one of which this plan caused: 0248's `core/tests/sanity.rs`.
- **Full suite:** owed to the conductor's pre-review gate (ADR-0207). The last per-phase gate run
  was `cargo nextest run --workspace -P fast --no-fail-fast` through the suite lock at Phase 5:
  368.4 s, 1631 run / 86 skipped / 0 failed.
- **Outstanding `human` phases:** none — every phase in this plan is `dev`.

## Followups (after this lands)

- Re-measure ADR-0211's saving against the new full-suite cost, per ADR-0222's closing note.
