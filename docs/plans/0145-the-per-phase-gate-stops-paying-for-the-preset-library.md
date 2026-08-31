# 0145 — The per-phase gate stops paying for the preset library

> **Status:** draft
> **Created:** 2026-08-31
> **Owner skill(s):** dev
> **Related ADRs:** [0156](../adrs/0156-the-per-phase-gate-is-scoped-and-the-suite-is-owed-once-per-plan.md)
> (proposed), [0033](../adrs/0033-testing-strategy-coverage-ratchet-and-pre-push-gate.md),
> [0071](../adrs/0071-a-numeric-test-contract-states-a-property-or-names-its-machine.md)

## TL;DR

The `dev` lane runs the whole test suite before every phase commit, and 27 of its 1212 tests — the
nine GPU sweeps whose price is set by the shipped preset count, not by the code under test — carry
most of that cost. This plan gives the per-phase moment the tier it never had: **narrowed per phase,
whole suite once per plan at the last phase**, with the filter defined **once** as a nextest profile
that the hook, CI and `dev` all cite. Nothing is deleted; 27 tests move to a different moment.

## Context & problem

The user's report: *"recent 10-20 plans were implemented really slowly. I believe that during
implementation we are running full test suite after each step, which maybe fine but slows process a
lot."*

The premise is half right, and the half that is wrong redirects the work. Compilation is **not** the
bottleneck — Plan 0129 already fixed that, and a warm rebuild after a one-file core edit is small.
What is left is the suite run itself plus the test-binary link, paid five to nine times per plan.

Four facts, none of them timings, establish the shape (see
[ADR-0156](../adrs/0156-the-per-phase-gate-is-scoped-and-the-suite-is-owed-once-per-plan.md) for the
full argument):

| fact | value | source |
|---|---|---|
| tests in the workspace | **1212** | `cargo nextest list --workspace` |
| tests in the nine GPU suites | **27** (2.2 %) | the same, minus the pre-push filter |
| shipped presets at Plan 0129's close → today | **54 → 81** | `git ls-tree ed8a787` vs `presets/` |
| copies of the exclusion filter that `dev` can see | **0 of 2** | `.githooks/pre-push`, `ci.yml` |

Four of the nine suites sweep the shipped preset set, so the gate's price is set by the
`preset-author` lane's output and rises every time it ships. And the uniform rule is already not
being followed: Plan 0135's `test(core): one shared harness for the integration tests` landed two
minutes after the commit before it, which no full suite run fits inside. `dev` narrows silently
today, inconsistently, and no phase records what it ran.

**The measurement this plan was supposed to open with was taken and discarded.** Three lanes (0092,
0125, 0144) were live on 2026-08-31 and two other sessions were running suites concurrently with it,
so every figure was an upper bound of unknown looseness. Phase 1 retakes it on a quiet box.

## Decision

Adopt ADR-0156: the per-phase gate is `build` + `clippy` + `fmt` + a **narrowed** nextest run; the
full `--workspace` run is owed **once per plan**, at the last phase, and recorded in the
implementation log. Two upward overrides stay with `dev` — a phase whose `Done when` names one of the
nine, and a phase that changes what those suites measure. The filter becomes a `fast` profile in
`.config/nextest.toml`, replacing the two existing copies rather than adding a third.

We rejected making the sweeps cheaper first (a coverage argument this plan does not make), a
touched-files-to-suite map (its failure mode is silent under-running), deferring the nine to CI
alone (the goldens would never run locally before a close), and an async full-suite run beside the
next phase (a red result landing after the commit it convicts). Full reasoning in ADR-0156.

## Architecture diagram

```mermaid
flowchart LR
    subgraph now["Today — one scope, three moments"]
        P1["per phase<br/>x5-9<br/><b>all 1212</b>"] --> H1["pre-push<br/>1185<br/>(own copy)"] --> C1["CI<br/>1185 + 27<br/>(own copy)"]
    end
    subgraph after["ADR-0156 — tiered, one definition"]
        F[".config/nextest.toml<br/><b>profile.fast</b>"]
        F -.-> P2["per phase<br/>x5-9<br/><b>1185</b>"]
        F -.-> H2["pre-push<br/>-P fast"]
        F -.-> C2["CI check<br/>-P fast"]
        P2 --> L["last phase<br/><b>all 1212</b><br/>recorded in the log"]
        L --> H2 --> C2
    end
    now ~~~ after
```

## Implementation phases

### Phase 1 — Take the baseline on a quiet box
- **Owner skill:** dev
- **What:** The readings this plan will be judged against, taken with nothing else building.
- **Files touched:** none (this plan's `## Implementation log` only).
- **Done when:** the log records, from this machine, the wall time of each of `cargo build`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check` and
  `cargo nextest run --workspace --no-run` after a one-file edit to `core/src/lib.rs`, plus
  `cargo nextest run --workspace` and the same run under the current pre-push filter — **and** the
  machine identification ADR-0071 requires. **No threshold is asserted and none is owed**: the phase
  produces readings, not a bar. Quiet is a stated precondition, not an assumption: enumerate
  `cargo`/`cargo-nextest`/`rustc` processes immediately before and after the run and record that
  none but this one was present. If that cannot be achieved, record the readings **and** what else
  was running, and mark them contended — a contended reading is reportable, a laundered one is not.

### Phase 2 — Define the filter once, as a nextest profile
- **Owner skill:** dev
- **What:** A `fast` profile carrying the exclusion, so one file defines what three consumers run.
- **Files touched:** `.config/nextest.toml`.
- **Done when:** `cargo nextest list --workspace -P fast` and `cargo nextest list --workspace -E`
  with the expression currently in `.githooks/pre-push` enumerate the **identical** test set —
  compared as sorted lists with an empty diff, not as equal counts. A **union check** proves nothing
  fell out: the `-P fast` set plus the tests in the nine excluded binaries equals the unfiltered
  `--workspace` set exactly. The existing `[[profile.default.overrides]]` that keeps the four
  reporting tests audible must still apply under `-P fast` — verify by running one of those tests
  under the profile and seeing its output; if a custom profile does not inherit the override, the
  override is restated under `profile.fast` and the file says why the duplication exists.

### Phase 3 — Point the hook and CI at the profile
- **Owner skill:** dev
- **What:** Delete both copies of the expression in favour of the profile.
- **Files touched:** `.githooks/pre-push`, `.github/workflows/ci.yml`.
- **Done when:** neither file contains the filter expression, both invoke `-P fast`, and
  `cargo nextest list` under each file's new command enumerates the same set as under its old one
  (empty diff, captured before the edit). The hook's header comment still explains *why* the nine
  are excluded and now says where the list lives; the reasoning does not move into the toml as a
  second prose copy. `.githooks/pre-push` exits 0 on a clean tree.

### Phase 4 — Give the per-phase loop its tier
- **Owner skill:** dev
- **What:** State in `dev`'s own instructions what a phase owes and what the last phase owes.
- **Files touched:** `.claude/skills/dev/SKILL.md`,
  `.claude/skills/dev/references/project-context.md`.
- **Done when:** `project-context.md`'s *"All four of build / test / clippy / fmt-check must be green
  before you commit a phase"* is replaced by ADR-0156's tier — narrowed per phase, full at the last
  phase, and the two upward overrides stated as `dev`'s to apply. The canonical-commands table
  carries a `-P fast` row and marks the bare `--workspace` run as the last-phase and close scope,
  keeping ADR-0072's warning that `--workspace` is load-bearing for `lmv-core-cabi` on **both** rows.
  `SKILL.md` Step 3 item 3 names the tier, and its close block requires the full run before the
  handoff. `node scripts/check-doc-links.mjs` exits 0.

### Phase 5 — Make the full-suite run a recorded fact
- **Owner skill:** dev
- **What:** The once-per-plan run leaves evidence, so the close review reads it instead of assuming.
- **Files touched:** `.claude/skills/architect/references/templates/plan.md`,
  `.claude/skills/architect/SKILL.md`.
- **Done when:** the plan template's `### Close triggers` carries a `**Full suite:**` bullet naming
  the command run, its exit code and the pass/skip counts; the architect skill's Mode 4 lens 1 names
  that bullet among the claims it verifies rather than trusts, consistent with the section's existing
  rule that the log is claims and not evidence. `node scripts/check-doc-links.mjs` and
  `node scripts/check-index-rows.mjs` both exit 0.

### Phase 6 — Re-measure, and do the per-plan arithmetic
- **Owner skill:** dev
- **What:** What the change actually bought, on the same machine under the same quiet precondition.
- **Files touched:** none (this plan's `## Implementation log` only).
- **Done when:** the log records Phase 1's readings retaken after Phases 2–5, under the same stated
  quiet precondition and with the same machine identification, **and** the per-plan arithmetic
  written out for a 6-phase plan (this repo's median): the old shape as six full gates, the new shape
  as six narrow gates plus one full run. The plan records the difference it measured and asserts no
  target — if the difference is small, that is the finding and it is reported as such.

## Data shapes

```toml
# illustrative — the profile Phase 2 adds to .config/nextest.toml.
# One line on purpose: a backslash continuation is NOT a line continuation inside a
# TOML literal string, so the expression either stays on one line or moves to a
# multi-line literal ('''...''').
[profile.fast]
default-filter = 'not (binary(golden) + binary(attractor) + binary(reaction_diffusion) + binary(background_composite) + binary(ink) + binary(reactivity) + binary(animation) + binary(sanity) + binary(distinctness))'
```

Verified 2026-08-31 on cargo-nextest 0.9.140 via `--config-file` against a scratch copy: the profile
form and the `-E` form both enumerate **1185** tests, against **1212** unfiltered.

## Risks & open questions

- **Deferred detection is the price, and it is real.** A `golden` or `sanity` regression introduced
  in phase 2 of a nine-phase plan surfaces at phase 9, and bisecting costs a full suite per candidate
  commit. Mitigation is the upward override in Phase 4: a phase touching a scene, the composite, the
  preset engine or the embedded set runs the affected suite. That rests on `dev`'s judgement and no
  gate enforces it — stated in ADR-0156's Negative section rather than papered over.
- **Editing `.github/workflows/ci.yml` needs the `workflow` OAuth scope on the credential**, or the
  push is rejected after the commits are already made. Known trap on this machine.
- **A custom nextest profile may not inherit `[[profile.default.overrides]]`.** Phase 2's done-when
  tests this rather than assuming it, and states the fallback.
- **Phase 1 and Phase 6 need the other lanes idle.** Three worktrees are live (0092, 0125, 0144) and
  the GPU suites serialize on one adapter, so a concurrent lane inflates any reading. Both phases
  verify and record the condition instead of asserting it held.
- **Open:** whether once-per-plan is the right cadence for a plan of 9+ phases, or whether the full
  run should also fire at a midpoint. Deliberately not decided here — Phase 6's arithmetic is the
  evidence to decide it from, and ADR-0156 Alternative D is the fallback if it proves too coarse.

## What this plan does NOT do

- **It does not make the sweeps cheaper.** Sampling the preset set rather than sweeping it is
  permitted by the lifted budget and is ADR-0156 Alternative A; it is a coverage decision needing its
  own argument about what a sample proves, and it is not smuggled in here.
- **It does not touch the 42 integration-test binaries or their link cost**, which is paid per phase
  regardless of which suites run. Scoped out by the user on 2026-08-31 ("gate scoping only, for now").
- **It does not change the coverage ratchet, CI's job structure, or what any test asserts.** The same
  1212 tests run; 27 of them run at a different moment.
- **It does not address concurrent lanes contending for one GPU**, which the discarded measurement
  exposed as a second, independent cost centre. Filed as a followup.

## Implementation log

> Written by `dev` — one row per phase as that phase's commit lands, and the close block after the
> last one. **The phases above are the contract; everything here is what happened.**

**Lane:** _(to be filled by `dev`)_

| phase | owner | state | commit |
|---|---|---|---|
| 1 — Take the baseline on a quiet box | dev | not started | |
| 2 — Define the filter once, as a nextest profile | dev | not started | |
| 3 — Point the hook and CI at the profile | dev | not started | |
| 4 — Give the per-phase loop its tier | dev | not started | |
| 5 — Make the full-suite run a recorded fact | dev | not started | |
| 6 — Re-measure, and do the per-plan arithmetic | dev | not started | |

### Notes

### Close triggers

- **`presets/` touched:**
- **Plan header `Closes:`** none
- **What shipped:**
- **Operator docs touched:**
- **Backlog probes (`node scripts/check-backlog-claims.mjs`):**
- **Outstanding `human` phases:**

## Followups (after this lands)

- **Concurrent lanes serialize on one GPU.** Three lanes running the nine suites at once cost each
  of them roughly three times as long. Worth a look now that the per-phase gate no longer runs them
  — the contention may largely evaporate, which would itself be the finding.
- **The sweeps' cost scales with the preset library** (54 → 81 in two days). ADR-0156 Alternative A
  is available under the lifted budget; the argument it owes is what a sampled sweep proves that a
  full one does not.
- **CI pays the cost this plan reduces locally, on every push** — Plan 0129 left the same followup
  and it is still open.
