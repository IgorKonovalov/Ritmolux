# 0207 — The commitments get their instruments

> **Status:** in-progress
> **Created:** 2026-09-19
> **Approved:** 2026-09-19 (user)
> **Owner skill(s):** dev, human
> **Related ADRs:** [0231](../adrs/0231-the-standalone-size-cap-is-re-derived-from-what-it-carries-and-the-build-reports-it.md)
> (proposed), [0232](../adrs/0232-a-presets-frame-cost-is-measured-and-reported-never-asserted.md)
> (proposed), [0159](../adrs/0159-the-component-gets-its-own-size-cap-and-the-recipe-carries-it.md),
> [0071](../adrs/0071-a-numeric-test-contract-states-a-property-or-names-its-machine.md),
> [0045](../adrs/0045-quality-tiers-floor-and-rich.md)
> **Closes:** design-backlog 0257

## TL;DR

Two of this project's headline commitments — the standalone's size cap and its ≥ 60 fps Floor —
are numbers in a document that nothing measures. This plan gives each an instrument: the exe's
composition is measured and its cap re-derived from it, the packaging recipe prints the length on
every build, the preset report gains an advisory frame-cost column, and the Floor reading is
finally taken on a named machine. The first visible behaviour is a build that tells you how big the
thing it just made is.

## Context & problem

**A commitment with no carrier goes unchecked for years.** [NFR §4](../nfr.md#4-size-and-dependencies)
caps the standalone at 10,000,000 B and says in its own text that the value is inherited and *"has
never been measured against what the exe actually contains."* Measured on 2026-09-19 on the
development box it is **10,971,648 B** — 9.7 % over ([backlog 0257](../design-backlog.md)). The
foobar component has had the opposite arrangement since
[ADR-0159](../adrs/0159-the-component-gets-its-own-size-cap-and-the-recipe-carries-it.md): a cap
derived from what it carries, and a recipe that prints its length every build and warns near it.
The standalone has no equivalent, which is exactly why the breach was found by accident while
pricing [Plan 0206](0206-the-browser-shows-the-look.md).

**The same shape holds for the thing the owner actually cares about.** The stated aim is that the
app stay *"runnable on a mid range laptop"* whatever the preset.
[NFR §1](../nfr.md#1-performance--adaptive-quality) already promises more — ≥ 60 fps at 1080p at
`Floor` on [§2](../nfr.md#2-platform-baseline)'s *any DX12 iGPU from ~2015* — but **no gate in the
suite measures time**. `sanity`, `reactivity`, `animation`, `beat`, `distinctness`, `golden`,
`composite`, `bloom` and the per-system gates all ask what a frame looks like; none asks what it
cost. And [§9](../nfr.md#9-test-hardware-matrix-what-the-user-has)'s matrix names the floor's
validating machine as a *class* — "Older Windows PC (iGPU)" — not a machine, so the reading has
never been attached to a configuration.

**Size and speed are different problems and were being conflated.** A 10 MB exe and a 16 MB exe
render identically; the cap protects download size, cold start and dependency discipline. Treating
it as a performance proxy is what made "is the cap right?" feel like a performance question. The
two ADRs beside this plan separate them.

## Decision

We will **derive each number from a measurement and attach an instrument to it**, taking size and
speed as separate work that happens to share a plan.

For size, [ADR-0231](../adrs/0231-the-standalone-size-cap-is-re-derived-from-what-it-carries-and-the-build-reports-it.md)
applies ADR-0159's rule to this artifact and adds the carrier. **The constant is set in Phase 1,
not in the ADR** — the rule needs the cost of one feature of the largest class shipped, which this
plan measures; 12,582,912 B and 16,777,216 B differ by 14.7 % versus 53 % headroom over today's
figure and choosing between them unmeasured would repeat the error being corrected. Phase 1's stop
condition may therefore supersede ADR-0231's derivation, which takes a dated `Outcome` at close.

For speed, [ADR-0232](../adrs/0232-a-presets-frame-cost-is-measured-and-reported-never-asserted.md)
adds an **advisory** frame-cost reading to the preset report and names the Floor machine. We
rejected a hard frame-cost gate (ADR-0071's machine-dependent number, on a CI with no GPU contract
per ADR-0016, designed against a guess about whether any shipped preset is even a problem),
rejected relying on the governor alone (it demotes `Rich`→`Floor` once and cannot help a preset
expensive *at* `Floor`), and rejected restating the hardware target as a mid-range laptop (it
weakens a promise in order to pass it).

## Implementation phases

Phases 1 and 2 are `dev` and contiguous; Phase 3 is `human` and needs hardware only the owner has,
so a conductor run parks in front of it.

### Phase 1 — The exe is measured, capped and reported

- **Owner skill:** dev
- **What:** Measure what the release exe carries, set the cap by ADR-0231's rule, and make the
  packaging recipe print it.
- **Files touched:** `packaging/windows/` (a build script, mirroring
  `packaging/foobar/build-component.ps1`'s measurement block), `packaging/macos/bundle.sh`,
  `docs/nfr.md`, `docs/releasing.md`, `docs/developing.md`.
- **Done when:**
  - **The composition is written down**: what the exe carries, broken down far enough to price *one
    more feature of the largest class this project has shipped*. That figure is the one input
    ADR-0231's rule lacks, and it is the whole reason this phase precedes the constant.
  - **The cap is set by the rule and the derivation is recorded in NFR §4**, replacing the
    inherited figure. **If the rule yields a number that looks wrong, the stop condition is to say
    so and stop** — the rule is ADR-0159's and superseding it is architect's call at the close, not
    a mid-phase adjustment.
  - **The Windows and macOS recipes print the artifact's length on every build and warn near the
    cap**, in the shape `build-component.ps1` already uses. **Never fatal** — ADR-0159's caps
    *"never fail a release over a size"*, and this phase does not narrow that.
  - The printed figure names the build it measured, per
    [ADR-0071](../adrs/0071-a-numeric-test-contract-states-a-property-or-names-its-machine.md): a
    size is a property of a build, not of the tree.

### Phase 2 — The report says what a preset costs

- **Owner skill:** dev
- **What:** An advisory per-preset frame-cost reading in `shot --presets … --report`.
- **Files touched:** `standalone/src/shot/report/`, `standalone/src/shot/report.rs`,
  `docs/capturing.md`, `docs/testing.md`.
- **Done when:** the report prints a frame-cost reading per preset beside the columns it already
  carries, and **names the machine and the tier it was taken at**. It is ADVISORY in the sense
  `distinctness` already is: it prints, it may flag, **it never fails a run**. The reading is
  comparative between presets rather than a prediction of the app — the headless path has no
  window, no present and no audio thread — and `docs/capturing.md` says so where the column is
  documented, because a number that looks like an fps and is not will otherwise be read as one.
  `docs/testing.md`'s gate table gains the row, marked ADVISORY, so the roster stays honest about
  what is and is not enforced.

### Phase 3 — The floor is measured where it is claimed

- **Owner skill:** human
- **What:** Name the machine and take the reading NFR §1 has always asserted.
- **Files touched:** `docs/nfr.md` (§9's matrix and §1's floor line),
  `docs/on-device-validation.md`.
- **Done when:** §9 names the actual machine — make, GPU, OS build — rather than the class *"Older
  Windows PC (iGPU)"*, and the Floor claim carries a dated reading taken on it at 1080p: the shipped
  set walked at `Floor`, with the frame-cost column from Phase 2 as the instrument and the app's own
  `F3` overlay as the cross-check. **A miss is a result, not a failure of this phase** — if some
  preset cannot hold the floor on baseline hardware, that is the first real evidence this project
  has had on the question, and it belongs in the close notes and then in a backlog entry, not in a
  silent retune. The checklist in `docs/on-device-validation.md` gains the walk so the reading is
  repeatable rather than a one-off.

## Risks & open questions

- **Phase 1 may not be able to price "one more feature of the largest class".** Rust binaries do not
  decompose cleanly by crate without extra tooling, and adding a tool for this is itself a
  dependency question. If the breakdown cannot be got cheaply, the honest fallback is to state the
  measured total, pick the boundary that ADR-0159's *proportional* headroom implies, and record that
  the step size was estimated rather than measured — flagged, not hidden.
- **A re-derived cap will be larger, and that reads as moving the goalposts.** It is the correct
  outcome of finding that the goalpost was never placed, but the derivation has to survive being
  quoted back later.
- **An advisory that nobody reads is worth nothing.** `distinctness` is the cautionary example in
  this very repository — its family roster went stale precisely because nothing fails when it is
  wrong. ADR-0232 accepts the risk knowingly; Phase 3 is what makes the new column get read at
  least once.
- **Phase 3 depends on hardware and on the owner's time**, so this plan parks rather than
  completes under a conductor. That is by design and not a defect.
- **Open question this plan does not answer:** if Phase 3 finds shipped presets that miss the floor,
  is the response a hard gate, a content retirement, or a tier below `Floor`? All three are
  plausible and all three need the evidence first.

## What this plan does NOT do

- **It does not optimise anything.** No binary is shrunk and no preset is made faster; this plan
  builds the instruments that would tell you whether either is needed.
- **It does not add a hard frame-cost gate** — ADR-0232's Alternative A, deliberately deferred until
  there is evidence to design it against.
- **It does not change the tiers or the governor.** ADR-0045 stands untouched.
- **It does not loosen NFR §2's baseline.** The 2015-era iGPU floor is stricter than the stated
  mid-range-laptop goal, so meeting it meets the goal; restating it downward was rejected.
- **It does not make either cap fatal.** Both stay soft, per ADR-0159.
- **It does not re-price [Plan 0206](0206-the-browser-shows-the-look.md).** That plan rejected
  embedding thumbnails on the codec dependency as well as the byte count, so a raised cap does not
  reopen it.

## Implementation log

> Written by the lane — one row per phase as that phase's commit lands, and the close block after
> the last one. **The phases above are the contract; everything here is what happened.**

**Lane:** `plan-0207-the-commitments-get-their-instruments` at `/home/igor/Work/rlx-plan-0207`

| phase | owner | state | commit |
|---|---|---|---|
| 1 — The exe is measured, capped and reported | dev | done | committed with this row |
| 2 — The report says what a preset costs | dev | not started | |
| 3 — The floor is measured where it is claimed | human | not started | |

### Notes

### Close triggers

- **`presets/` touched:**
- **Plan header `Closes:`** design-backlog 0257
- **What shipped:**
- **Operator docs touched:**
- **Backlog probes (`node scripts/check-backlog-claims.mjs`):**
- **Full suite:**
- **Outstanding `human` phases:**

## Followups (after this lands)

- Whatever Phase 3's reading turns up, as its own backlog entry — including the case where
  everything passes, which is worth recording so the question is not re-opened from scratch.
- A hard frame-cost gate, if and only if the advisory finds shipped offenders.
