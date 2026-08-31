# ADR-0157 — The preset sweeps split per preset, and the per-phase tier samples a declared representative

> **Status:** proposed
> **Date:** 2026-08-31
> **Related plan(s):** [0146](../plans/0146-the-preset-sweeps-stop-being-one-long-test.md)

## Context

[ADR-0156](0156-the-per-phase-gate-is-scoped-and-the-suite-is-owed-once-per-plan.md) took the nine
GPU suites out of the per-phase gate and left them owed once per plan and in CI. That fixed the
implementation loop and deliberately did not touch what those suites cost when they *do* run. This
ADR takes up what it recorded as Alternative A.

**Five `#[test]` functions each loop the whole preset roster serially, and nextest parallelizes
across tests and never inside one.** Measured on an idle box, 2026-08-31:

| roster monolith | wall |
|---|---|
| `animation::every_preset_animates_over_time` | **758 s — 87 % of the suite's 869 s** |
| `reactivity::every_preset_reacts_to_at_least_one_band` | 667 s |
| `distinctness::report_family_distinctness` | 665 s |
| `sanity::a_louder_frame_is_reported_against_a_quieter_one` | 473 s |
| `sanity::every_preset_draws_a_real_shape` | 432 s |

**The cost is a scheduling failure, not a volume of work.** The full run sums 7,965 s of test-wall
into 869 s elapsed — an average concurrency of **9.2** on a 16-CPU box, so **43 % of the machine sits
idle**. The narrow run reaches **13.7** on the same box. The gap is entirely the tail, where only
these five processes remain and twelve threads have nothing to do. Perfect packing of the same work
would finish in **~500 s**.

**Splitting redistributes work; only sampling removes it.** Because the five are similar sizes,
splitting one merely promotes the next: splitting `animation` alone moves the wall from 869 s to
roughly `reactivity`'s 667 s. Splitting all five reaches the ~500 s packing floor, a **1.6x** win and
no more. Cutting the roster the sweeps traverse is the only lever that reduces the 7,965 s itself.

Three facts constrain how the roster may be cut. The shipped set is **81 presets across 12 families**
and the distribution is lopsided — `attractor` 19 and `fragment_field` 13 against four families of 4
— so a flat "2 per family" takes 24 presets and draws almost all of its saving from two families.
`distinctness` compares presets **pairwise within a family** to catch near-duplicates, so sampling it
to two leaves one comparison per family and retires the check rather than narrowing it. And
[ADR-0081](0081-the-content-lane-lands-presets-and-architect-curates-the-set.md) rests these sweeps
on the `preset-author` lane: they are why a preset may land without `dev` couriering it, because *"a
weak preset fails CI for everyone."* A sample that reached CI would remove that gate silently.

`core/build.rs` already globs `presets/*.toml` to generate the `EMBEDDED` table
([ADR-0022](0022-build-time-preset-embedding.md)), so the mechanism for generating one test per
preset exists and needs no new dependency.

## Decision

**The roster sweeps stop being single tests.** The four per-preset sweeps — `animation`'s roster
test, `reactivity`'s, and `sanity`'s two — generate **one test per preset** from `build.rs`'s
existing glob, named after the preset so a failure names it and `-E` can select it.
`distinctness` splits **per family** (12 tests) and never per preset, because its claim is pairwise
within a family and does not decompose further.

**A preset declares whether it is a representative.** A `representative` key in the preset `.toml`
marks the sample the per-phase tier runs; **the close and CI always run the whole set**, so the
ADR-0081 curation gate keeps its full strength and only the per-phase tier sees a sample. Selection
is *declared, not derived*: the curator sets it at the plan-close curation cadence ADR-0081 already
establishes. A test enforces the floor — **at least two per family, and every family represented** —
so the sample cannot silently rot to nothing as content lands.

The initial values are seeded from `distinctness`'s own pairwise matrix, taking the two furthest-apart
presets in each family, and are curated from there. That makes the starting sample defensible rather
than arbitrary without making the mechanism a derivation.

## Consequences

### Positive
- The full suite drops toward its packing floor — ~869 s to ~550 s from splitting alone, and further
  once the per-phase tier samples. CI pays the same reduction on every push.
- **A failure names the preset.** Today a roster sweep reports a loop index inside a 758 s test; a
  generated test is selectable, re-runnable and bisectable on its own.
- Sampling becomes a **filter over test names**, not a second code path through the sweeps. Once the
  tests are split, changing the sample is a profile edit.
- The idle-machine waste is fixed permanently, not just for today's roster.

### Negative
- **Per-test device creation replaces per-sweep device creation.** Each sweep builds one headless
  renderer today and reuses it for all 81 presets; nextest runs each test in its own process, so the
  split pays 81 device creations instead of 1. If that setup cost is large relative to the ~9 s of
  per-preset work, **the split can lose**, and it loses worst on exactly the fast presets. Plan 0146
  Phase 1 measures this on the worst sweep before the other four are touched, and may abort there.
- **A non-representative preset is unrendered until the close.** That is a real per-phase gap, and it
  is widest in the two large families where sampling saves the most.
- **A new schema key and a standing curation duty.** A family whose representatives drift out of date
  keeps testing the wrong two, and nothing but the two-per-family floor notices. The floor catches
  absence, never staleness.
- **Generated tests are harder to read than a loop**, and a `build.rs` defect becomes a test-suite
  defect. The generator is now load-bearing for five suites rather than one table.

### Neutral
- No test's *assertion* changes. The same claims are made about the same presets; what changes is how
  many processes make them and, in the per-phase tier only, how many presets are visited.
- `distinctness` is untouched by the sampling half by construction.

## Alternatives considered

### Alternative A — Sample only, leave the monoliths whole
Cheapest change: an env var read by the three roster helpers. Rejected as insufficient rather than
wrong — it cuts the sweeps' work ~3.4x but leaves them serial, so the machine stays 43 % idle, the
wall stays pinned to whichever monolith is longest, and a failure still reports a loop index. It also
forecloses nothing, which is why the split comes first.

### Alternative B — Split only, never sample
Zero coverage loss and no schema change. Rejected as the *whole* answer: it reaches the ~500 s
packing floor and stops, which is not enough to return the sweeps to the per-phase gate. Retained as
the fallback if Phase 1's measurement kills the sampling half.

### Alternative C — Derive the sample instead of declaring it
Two forms, both rejected. **First-N per family by filename** never samples a newly landed preset,
which is precisely when a render is most wanted. **Rotation by commit hash** covers everything over
time but makes the same tree gate differently on different commits, so a red phase may be a selection
artifact rather than a regression — which this project rejects for analysis and should reject here.

### Alternative D — Run the sweeps in CI only
Already rejected by ADR-0156 Alternative C: a plan would close having never run its own drift guards.

## Notes

The measurements above are Plan 0145's, taken 2026-08-31 on an idle box at `fd7f55b` (AMD Ryzen 9
5900HS, 16 logical CPUs, rustc 1.97.1, cargo-nextest 0.9.140). That plan's
`### The measured baseline` carries the caveats, including a full-suite spread of 489–885 s across
three readings within an hour — which is why this ADR argues from the **concurrency figure and the
critical-path structure**, both stable, rather than from any single wall time.
