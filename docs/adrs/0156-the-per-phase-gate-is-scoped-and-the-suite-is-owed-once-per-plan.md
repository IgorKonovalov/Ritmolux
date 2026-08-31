# ADR-0156 — The per-phase gate is scoped, and the whole suite is owed once per plan

> **Status:** proposed
> **Date:** 2026-08-31
> **Related plan(s):** [0145](../plans/0145-the-per-phase-gate-stops-paying-for-the-preset-library.md)

## Context

The `dev` lane's per-phase rule is uniform and undifferentiated. `project-context.md` states that
*"all four of build / test / clippy / fmt-check must be green before you commit a phase"*, and the
canonical-commands table hands it `cargo nextest run --workspace`. Plans run between 2 and 11
phases — the last eighteen closed plans have a median of 6 — so that gate is paid once per phase,
five to nine times in a typical plan.

**The suite's cost is concentrated in 27 tests.** `cargo nextest list --workspace` enumerates 1212
tests today; the nine GPU suites the pre-push hook already excludes (`golden`, `attractor`,
`reaction_diffusion`, `background_composite`, `ink`, `reactivity`, `animation`, `sanity`,
`distinctness`) hold **27** of them, and the remaining 1185 are what the hook and CI's `check` job
run. Those 27 are sweeps: one test iterates every shipped preset or every scene through a real
adapter. Plan 0129 measured the whole suite at **341 s** on this machine (1122 tests, 54 binaries)
with `reactivity` alone at **126 s**, and the hook's own header records warm per-suite figures
that sum well past the wall time of everything else combined. So 2.2 % of the tests carry the
majority of the gate, and they are the 2.2 % least likely to be affected by any given phase.

**The exclusion already exists twice, and is absent from the one place that runs most often.**
`.githooks/pre-push` defines `NEXTEST_FILTER` and explains that it was chosen by measurement;
`.github/workflows/ci.yml` carries a byte-copy of the same expression. Neither is visible to the
`dev` lane, whose instructions offer no narrowed form and no statement that narrowing is permitted.
[ADR-0033](0033-testing-strategy-coverage-ratchet-and-pre-push-gate.md) sorted tests into five
tiers by *kind* and assigned a *when* to exactly two moments — pre-push and CI. The per-phase
moment, the one that recurs five to nine times per plan, was never given a tier.

**The uniform rule is not being followed, and the record cannot tell which phases obeyed it.** In
Plan 0135, `test(core): one shared harness for the integration tests` landed two minutes after the
commit before it; Plan 0129's phases landed 7, 9, 4, 10 and 2 minutes apart. A 341 s suite plus a
test-binary link does not fit inside two minutes. `dev` therefore already narrows, silently and
inconsistently, and no phase records what it ran — so *"the suite was green at every phase"* is a
belief about this project, not a fact anyone can check.

**The cost grows for reasons unrelated to the code being gated.** Four of the nine suites sweep the
shipped preset set, which held **54** `.toml` files at Plan 0129's close on 2026-08-29 and holds
**81** two days later. The `preset-author` lane lands presets on its own cadence
([ADR-0081](0081-the-content-lane-lands-presets-and-architect-curates-the-set.md)), so every preset
it ships taxes every engine phase of every future plan. A gate whose price is set by another lane's
output will keep rising whatever is decided about the code it guards.

Plan 0129 named this lever and did not pull it: its quality budget was *"tooling only, zero coverage
change"*, and it records the two levers that budget excluded as *"deferring GPU suites to CI, and
consolidating the 41 test binaries"*. The user lifted that budget on 2026-08-31.

## Decision

The per-phase gate is `cargo build`, `cargo clippy --workspace --all-targets -- -D warnings`,
`cargo fmt --all --check`, and a **narrowed** `cargo nextest run --workspace` that excludes the nine
GPU suites — the same 1185 tests the pre-push hook and CI's `check` job already run. The **full**
`cargo nextest run --workspace` is owed **once per plan**, at the last phase, before the close
handoff, and its result is recorded in the implementation log's close block so the close review
reads a fact rather than an assumption.

Two overrides, both upward and both `dev`'s to apply: a phase whose own `Done when` names one of the
nine runs that suite regardless of the default, and a phase that changes what those suites measure —
a scene, the composite, the preset engine, or the embedded set — runs the affected suite. The
default narrows; it never caps.

The filter is defined **once**, as a `fast` profile in `.config/nextest.toml`, and the hook, CI and
the `dev` lane cite the profile instead of restating the expression. A profile `default-filter`
reproduces the current expression exactly — both list 1185 tests against 1212 unfiltered, verified
2026-08-31 on cargo-nextest 0.9.140 — so this replaces two copies with one definition rather than
adding a third.

## Consequences

### Positive
- The per-phase gate stops being dominated by work whose price is set by the preset library rather
  than by the change under test, and stops rising every time the content lane ships.
- The narrowing becomes stated, uniform and auditable, replacing an improvisation that varied per
  phase and left no record of what any phase actually ran.
- One definition replaces two copies and forecloses a third. Adding or removing a suite from the
  exclusion is a one-file edit that all three consumers inherit.
- The close gains a recorded full-suite result, which is a fact the Mode 4 review can check rather
  than a silence it has to interpret.

### Negative
- **A regression in one of the nine now surfaces later, potentially at the last phase of a
  nine-phase plan, and bisecting it costs a full suite run per candidate commit.** This is the real
  price. `golden` and `sanity` are the drift guards for exactly the visual regressions no other test
  catches, and the moment of detection moves from "the phase that caused it" to "some phase since
  the last full run".
- The override rule rests on `dev` judging blast radius, and no gate enforces that judgement. A
  scene edit whose author does not think of `sanity` gets the narrow gate and nothing objects.
- The last phase acquires a new stopping point. A plan whose final phase is trivial pays the full
  suite there regardless.

### Neutral
- Nothing is deleted and no test is weakened. The same 1212 tests run; 27 of them run at a different
  moment. The coverage ratchet, the pre-push hook's own scope and CI are untouched.
- The budget lifted on 2026-08-31 also permits cutting genuinely redundant test work — sampling the
  preset set rather than sweeping it. **This ADR uses only the deferral half**; the sampling half is
  a separate decision with its own coverage argument to make, and Plan 0145 does not make it.

## Alternatives considered

### Alternative A — Keep the full suite every phase and make it cheaper
Attack the sweeps directly: sample the preset set per phase, sweep it at close. Rejected as the
*first* move, not on merit. The dominant term is set by the content lane, so this is a coverage
decision requiring its own argument about what a sample proves, and it leaves the actual defect
untouched — the per-phase moment would still have no defined tier, and `dev` would still be
improvising. Now permitted by the lifted budget and worth taking up once the tier exists.

### Alternative B — Map touched files to the suites they affect
Per-phase runs only the suites a phase's changed paths imply. Rejected: it needs a files-to-suite
map that nothing can hold true as scenes and suites move, and its failure mode is **silent
under-running** — a stale map quietly drops a suite with no signal. A stated default plus an
explicit upward override fails loudly instead, because the override is written in the plan.

### Alternative C — Defer the nine to CI alone
Rejected: CI runs them in the `coverage` job on `windows-latest`, which reports *after* the push. A
plan would close having never run its own drift guards locally, and on this project the goldens are
the only defence against a silent visual regression.

### Alternative D — Run the full suite asynchronously beside the next phase
Commit on the narrow gate, run the full suite in the background, read it before the next commit.
Rejected: a red result then lands *after* the commit it convicts, and this project never rewrites
history, so the repair is a follow-up commit whose relationship to the defect is legible only from
the log. Worth revisiting if once-per-plan proves too coarse.

## Notes

**The timing measurements intended for this ADR were taken and discarded.** Three worktree lanes
(0092, 0125, 0144) were live on 2026-08-31, and two other sessions were running test suites
concurrently with the measurement — one `nextest run -p lmv-core --no-fail-fast` from 10:38:52 and
one `nextest run --workspace` from 10:42:08, against a measurement that started at 10:44:03. Three
suites contended for one GPU and cargo's lock, so every figure was an upper bound of unknown
looseness. Per [ADR-0071](0071-a-numeric-test-contract-states-a-property-or-names-its-machine.md) a
number names the machine it was measured on; a number that cannot name its conditions does not
belong in a decision record. Plan 0145 Phase 1 takes the baseline on a quiet box instead.

The counts in this ADR are not timings and are unaffected: 1212 / 1185 / 27 tests come from
`cargo nextest list`, and 54 / 81 presets from `git ls-tree` at two named commits.

**A clean pair was obtained later the same day**, on an idle box with the precondition verified
before and after: the full suite **869 s** (1217 tests) against **163 s** under the pre-push filter
(1190 tests), with a 64 s compile/lint/link floor either way.

**The mechanism is a critical path, not a share of the work.** The nine excluded binaries hold only
**49 %** of the suite's 7,965 CPU-seconds — near parity with the narrow set. What separates them is
that four `#[test]` functions each loop the whole preset roster *serially*, and nextest parallelizes
across tests and never inside one: `animation::every_preset_animates_over_time` alone runs **758 s,
87 % of the entire suite's wall time**. The narrow set's 4,036 CPU-seconds compress to 163 s across
1,190 tests; the excluded set cannot compress below its longest single test.

That distinction matters for what comes next. It means the deferral this ADR decides is worth its
706 s for a structural reason, and it means **Alternative A's sampling is not the only way to attack
the sweeps** — splitting a monolith into one test per preset would recover most of the same wall time
at *no* coverage cost. Plan 0145 carries the table and the followup.

Those readings and their caveats live in Plan 0145's `### The measured baseline`, including the
finding that three readings of the same command spanned **489–885 s** within an hour — so the
decision rests on the critical-path structure, not on any single wall-time figure.

**Concurrent lanes are a second, independent cost centre.** The GPU suites serialize on one adapter,
so N lanes running them at once cost N times as long each. That is not addressed here and is filed
as a followup on Plan 0145.
