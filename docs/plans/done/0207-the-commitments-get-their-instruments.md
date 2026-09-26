# 0207 — The commitments get their instruments

> **Status:** done — closed 2026-09-23. Phases 1-2 landed (`0b018885`, `add3d174`); the `human`
> Phase 3 was deferred to [On-device validation](../../on-device-validation.md) and its half that
> needed no hardware landed. Close review round 1: **no blockers, no majors, six minors, two nits**
> (five repaired in `a8f09acb`). Full suite green on the reviewed tree, 1778 passed.
> **Created:** 2026-09-19
> **Approved:** 2026-09-19 (user)
> **Owner skill(s):** dev (the `human` phase was deferred 2026-09-23 — see `## Deferred`)
> **Related ADRs:** [0231](../../adrs/0231-the-standalone-size-cap-is-re-derived-from-what-it-carries-and-the-build-reports-it.md)
> (proposed), [0232](../../adrs/0232-a-presets-frame-cost-is-measured-and-reported-never-asserted.md)
> (proposed), [0159](../../adrs/0159-the-component-gets-its-own-size-cap-and-the-recipe-carries-it.md),
> [0071](../../adrs/0071-a-numeric-test-contract-states-a-property-or-names-its-machine.md),
> [0045](../../adrs/0045-quality-tiers-floor-and-rich.md)
> **Closes:** design-backlog 0257

## TL;DR

Two of this project's headline commitments — the standalone's size cap and its ≥ 60 fps Floor —
are numbers in a document that nothing measures. This plan gives each an instrument: the exe's
composition is measured and its cap re-derived from it, the packaging recipe prints the length on
every build, the preset report gains an advisory frame-cost column, and the Floor reading is
finally taken on a named machine. The first visible behaviour is a build that tells you how big the
thing it just made is.

## Context & problem

**A commitment with no carrier goes unchecked for years.** [NFR §4](../../nfr.md#4-size-and-dependencies)
caps the standalone at 10,000,000 B and says in its own text that the value is inherited and *"has
never been measured against what the exe actually contains."* Measured on 2026-09-19 on the
development box it is **10,971,648 B** — 9.7 % over ([backlog 0257](../../design-backlog.md)). The
foobar component has had the opposite arrangement since
[ADR-0159](../../adrs/0159-the-component-gets-its-own-size-cap-and-the-recipe-carries-it.md): a cap
derived from what it carries, and a recipe that prints its length every build and warns near it.
The standalone has no equivalent, which is exactly why the breach was found by accident while
pricing [Plan 0206](0206-the-browser-shows-the-look.md).

**The same shape holds for the thing the owner actually cares about.** The stated aim is that the
app stay *"runnable on a mid range laptop"* whatever the preset.
[NFR §1](../../nfr.md#1-performance--adaptive-quality) already promises more — ≥ 60 fps at 1080p at
`Floor` on [§2](../../nfr.md#2-platform-baseline)'s *any DX12 iGPU from ~2015* — but **no gate in the
suite measures time**. `sanity`, `reactivity`, `animation`, `beat`, `distinctness`, `golden`,
`composite`, `bloom` and the per-system gates all ask what a frame looks like; none asks what it
cost. And [§9](../../nfr.md#9-test-hardware-matrix-what-the-user-has)'s matrix names the floor's
validating machine as a *class* — "Older Windows PC (iGPU)" — not a machine, so the reading has
never been attached to a configuration.

**Size and speed are different problems and were being conflated.** A 10 MB exe and a 16 MB exe
render identically; the cap protects download size, cold start and dependency discipline. Treating
it as a performance proxy is what made "is the cap right?" feel like a performance question. The
two ADRs beside this plan separate them.

## Decision

We will **derive each number from a measurement and attach an instrument to it**, taking size and
speed as separate work that happens to share a plan.

For size, [ADR-0231](../../adrs/0231-the-standalone-size-cap-is-re-derived-from-what-it-carries-and-the-build-reports-it.md)
applies ADR-0159's rule to this artifact and adds the carrier. **The constant is set in Phase 1,
not in the ADR** — the rule needs the cost of one feature of the largest class shipped, which this
plan measures; 12,582,912 B and 16,777,216 B differ by 14.7 % versus 53 % headroom over today's
figure and choosing between them unmeasured would repeat the error being corrected. Phase 1's stop
condition may therefore supersede ADR-0231's derivation, which takes a dated `Outcome` at close.

For speed, [ADR-0232](../../adrs/0232-a-presets-frame-cost-is-measured-and-reported-never-asserted.md)
adds an **advisory** frame-cost reading to the preset report and names the Floor machine. We
rejected a hard frame-cost gate (ADR-0071's machine-dependent number, on a CI with no GPU contract
per ADR-0016, designed against a guess about whether any shipped preset is even a problem),
rejected relying on the governor alone (it demotes `Rich`→`Floor` once and cannot help a preset
expensive *at* `Floor`), and rejected restating the hardware target as a mid-range laptop (it
weakens a promise in order to pass it).

## Implementation phases

Phases 1 and 2 are `dev` and contiguous. A third phase — the Floor reading on baseline hardware —
**was deferred on 2026-09-23 and left this contract**; `## Deferred` below is the record, and the walk
itself now lives in [On-device validation](../../on-device-validation.md).

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
    [ADR-0071](../../adrs/0071-a-numeric-test-contract-states-a-property-or-names-its-machine.md): a
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

## Deferred (2026-09-23) — the Floor reading, formerly Phase 3

**The machine is not available, and the reading cannot honestly be taken anywhere else.** The phase
asked for NFR §1's floor to be read at 1080p at `Floor` on §9's *"Older Windows PC (iGPU)"*. That box
is not in hand; the two machines that are — the dev box and the Arch laptop — are both faster than the
baseline by construction, so a reading from either would answer a different question while looking
like an answer to this one. Deferring is therefore the correct outcome rather than a concession.

**The walk was extracted instead of dropped.** [On-device validation](../../on-device-validation.md)
gains a dated `iGPU-gated` section carrying the whole walk: name the machine in §9, run the shipped
set at `Floor` with Phase 2's frame-cost block as the instrument, cross-check the flagged presets
against the window's `F3` overlay, and write the result into §1's Floor line. That file is this
project's standing carrier for a check no machine here can run, and extracting a hardware-gated phase
into it at the close is the move [Plan 0152](0152-the-osc-root-becomes-rlx.md) Phase 5 and
[Plan 0158](0158-the-player-grows-a-studio-facing-surface.md) Phase 7 both made before this one.

**The gap is now visible on the page that makes the claim.** NFR §1's Floor bullet says in its own
text that the number has never been read on the hardware it names, dated, and §9's matrix row says the
machine is still a class rather than a configuration. That is the half of this phase that needed no
hardware, and it landed: an unmeasured commitment that says so is a different object from one that
reads as a measurement.

**What this does not change.** The plan's `**Closes:**` claim is untouched — [backlog
0257](../../design-backlog-archive.md) was the standalone's size cap, both halves of it, and Phase 1
discharged them. No hard frame-cost gate is written; ADR-0232 keeps the advisory shape deliberately,
and designing a gate before the evidence it would be tuned against is the error this plan exists to
correct one level up.

#### What it asked for, verbatim

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
| 1 — The exe is measured, capped and reported | dev | done | 0b018885 |
| 2 — The report says what a preset costs | dev | done | add3d174 |
| 3 — The floor is measured where it is claimed | human | done — deferred 2026-09-23, the walk moved to on-device validation; see `## Deferred` | no commit |

### Notes

- **Deviation, Phase 1 (0b018885):** `.github/workflows/release.yml` is edited and is not in the
  phase's file list. The windows job's inline staging block moved into the new
  `packaging/windows/stage.ps1` and the job became a thin caller of it, the shape the linux, macos
  and foobar jobs already have. Without that edit the script runs nowhere and the done-when
  "prints on every build" is not met.
- **Phase 1, the step size and the boundary.** No feature-sized diff of this exe was available
  (the standalone does not compile without the `text` feature), so the step is ADR-0159's
  measured `text` diff on the component, 2,104,320 B; the font stack attributes to 1,584,263 B of
  named symbols on the Linux build, a floor consistent with it. "Next round binary boundary" was
  read as ADR-0231's own candidates, 12 MiB and 16 MiB, giving 16,777,216 B; the whole-MiB
  reading gives 13,631,488 B and NFR §4 records why it was not taken. ADR-0231's `Outcome` is
  not written here.
- **Phase 1, unexercised on this box.** The session ran on Linux, so neither
  `packaging/windows/stage.ps1` nor `packaging/macos/bundle.sh` was executed; both measurement
  blocks were written against `build-component.ps1`'s and reviewed by eye. The Windows exe with
  `--features spout` and both Apple slices are unmeasured; the first tag build prints them.
- **Phase 1, noticed and not acted on:** `packaging/linux/stage.sh` does not measure (not in the
  phase's file list, and the Linux binary is not the capped one); no hygiene guard holds the exe's
  two constants across `stage.ps1`, `bundle.sh` and NFR §4 the way guard (e) holds the
  component's; `CLAUDE.md`'s layout line still says `packaging/windows/` carries no recipe;
  `packaging/foobar/rlx-version.ps1`'s comment still names a workflow copy of the version regex
  that no longer exists.
- **Phase 2, what it costs.** The cost pass adds 120 frames at 1080p per preset. On the
  development box the report's console process landed on the AMD iGPU (RADV RENOIR) and a
  five-preset family read 0.542 to 3.589 ms/frame in the debug profile; the eight `shot_cli`
  report tests took 14.3 s together on it.
- **Phase 2, the fast profile.** The first `cargo nextest run --workspace -P fast` exited 100 with
  its output truncated before the summary, so the failing test is not identified; the immediate
  re-run passed 1699 of 1699 with 86 skipped.

### Close triggers

- **`presets/` touched:** no
- **Plan header `Closes:`** design-backlog 0257 (already in the archive as promoted)
- **What shipped:** feature — a Windows packaging recipe that measures the exe, the macOS
  recipe's measurement, the re-derived cap, and the report's frame-cost block; no engine change
- **Operator docs touched:** `docs/nfr.md`, `docs/releasing.md`, `docs/developing.md`,
  `docs/capturing.md`, `docs/testing.md`
- **Backlog probes (`node scripts/check-backlog-claims.mjs`):** exit 0 — 46 stated reductions hold
  across 21 live entries, 4 unprobeable
- **Full suite:** owed to the conductor's pre-review gate (ADR-0207)
- **Outstanding `human` phases:** none — Phase 3 was deferred on 2026-09-23 (see `## Deferred`)
  and its walk now lives in `docs/on-device-validation.md`

## Close review

> Round 1, 2026-09-23, written by a conductor review session (ADR-0205) started with this plan and
> this lane and nothing an implementer wrote. Reproduced in full, because a conductor-run close has
> no reader in the room and this section is the evidence of what was checked. Review path:
> `tools/conductor/state/reviews/0207-round-1.md`.

**Verdict: Plan 0207 landed cleanly — no blockers, no majors, six minors and two nits.** Both `dev`
phases meet their done-whens; the third phase is a `human` one that was deferred with its walk
extracted rather than dropped, which is the right outcome and is recorded on the page that makes the
claim. Every finding below is documentation drift the plan created, a guard it declined to write, or
a process note — none of them changes what a program does.

### What was verified, and how

| Check | Result |
|---|---|
| `with-lock.mjs suite -- cargo nextest run --workspace` | `skipped ...: tree ef2281f is green in the suite ledger, run by gate 0207-pre-review at 2026-09-23T13:44:25.606Z: 1778 tests run: 1778 passed (3 slow), 7 skipped` — the ledger record is this round's full-suite evidence (ADR-0207) |
| `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` | clean |
| `cargo fmt --all -- --check` | clean |
| `cargo clippy --workspace --all-targets -- -D warnings` | clean |
| `node scripts/check-doc-links.mjs` | OK, 552 tracked files |
| `node scripts/check-reader-prose.mjs` | OK, 16 documents, 0 bare citations |
| `node scripts/check-comment-hygiene.mjs` | OK, 296 sources, 0 escapes |
| `node scripts/check-index-rows.mjs` | OK, 0 over cap |
| `node scripts/toc.mjs --check` | OK, 7 blocks current |
| `node scripts/check-system-counts.mjs` | OK |
| `node scripts/check-backlog-claims.mjs` | OK — 46 reductions across 21 live entries, 4 unprobeable |
| `node scripts/check-translations.mjs` | OK — 5 stamped; three sources have moved past their stamps, **none of them touched by this plan** |
| `node scripts/check-gate-carriers.mjs` | OK, 19 rostered gates, hook 16/16, ci 16/16 |

The dev box's run of the suite **did** exercise the new cost path: its adapter is a real AMD iGPU,
so `adapter_is_software()` is false and the eight `shot_cli` report tests paid the pass. That matters
because on a software rasterizer the whole of `frame_costs` is skipped, and a CI-only green would
have said nothing about it.

### Lens 1 — alignment with the plan and the ADRs

**Phase 1 (`0b018885`) meets all four done-whens.**

- *The composition is written down.* NFR §4 now carries a by-section and by-crate breakdown of the
  Linux build of the same tree, with the method stated (stripped file for sections, an unstripped
  rebuild for symbols, every per-crate figure declared a **floor** because fat LTO inlines). Two of
  its figures are independently reproducible from the checkout and both are exact: the embedded
  preset library is 116 files and 641,455 B.
- *The cap is set by the rule and the derivation is recorded.* 10,971,648 B measured + 2,104,320 B
  (ADR-0159's measured `text` step) = 13,075,968 B; 16,777,216 B is the next of the two boundaries
  ADR-0231 names above it. The arithmetic checks: the admitted step lands at 77.9 % of the cap, a
  second at 90.5 %, and under the rejected whole-MiB reading (13,631,488 B) the first step would
  land at 95.9 % — past the warning line, which is the stated reason it was not taken. The plan gave
  Phase 1 a stop condition rather than a constant, and the phase used it.
- *Both recipes print and warn, never fatally.* `packaging/windows/stage.ps1` (new) and
  `packaging/macos/bundle.sh` print the length in bytes with the share of the cap, warn above
  15,099,494 B and die on nothing. The two constants agree across the two recipes and NFR §4, and
  15,099,494 is exactly `floor(0.9 * 16,777,216)`.
- *The printed figure names its build.* Both print the cargo command, the version, the rustc version
  and — on Windows — the triple, which is ADR-0071 applied to a size.

The **deviation the log declares** — `.github/workflows/release.yml` edited, outside the phase's file
list — is the right call and is recorded. Read against the deleted inline block, the new script is a
superset: it keeps every check and adds one (no entry outside the top-level folder), and it drops the
separator normalization by writing entry names explicitly instead, which is `build-component.ps1`'s
own solution. The one behavioural difference is that it asks `cargo metadata` for `target_directory`
rather than assuming `target/`, matching `bundle.sh`.

**Phase 2 (`add3d174`) meets its done-when.** The report prints a per-preset `ms/frame` block; the
header names the adapter and the build profile; the block names the size, the tier, the method and
the headless caveat, and the `!` marks past 16.67 ms and does nothing else. `docs/capturing.md` gains
a section saying, in its own words, that the reading is comparative and predicts nothing about the
app; `docs/testing.md`'s gate roster gains the row marked ADVISORY and says in the row that it is not
a test in the suite at all. Nothing asserts a frame time anywhere, which is the whole of ADR-0232.

The method is sound and its defence is in the code: one untimed warm pass, then per-preset short and
long legs with the **minimum of each leg** kept and subtracted once (the comment correctly explains
why a minimum over the *difference* would be worse), and the presets **interleaved inside** the
repeat loop so no preset inherits a warmed-up clock state. A negative reading is printed as measured
rather than clamped, and the doc comment says that is the signal.

**Phase 3 was deferred, and the deferral is honest.** The machine is a class in NFR §9, not a machine;
the two boxes in hand are both faster than the baseline by construction. The walk was extracted
verbatim into `docs/on-device-validation.md` as a dated `iGPU-gated` section — the move Plans 0152 and
0158 each made — and the half that needed no hardware landed: NFR §1's Floor bullet now says in its
own text, dated, that the number has never been read on the hardware it names.

**Owner tags:** every phase carries exactly one in-vocabulary tag. **The log is shorter than the
contract it reports on**, declares its deviation, and its `### Close triggers` are accurate against
the diff.

### Lens 2 — layering, coupling, real-time safety

- **Nothing entered `core/`**; the whole of Phase 2 is `standalone/src/shot/`.
- **The wall clock is confined, declared and escaped properly.** `frame_costs` is the only new
  `Instant` user and carries `#[allow(clippy::disallowed_methods, reason = ...)]` — the grep-able
  escape `clippy.toml`'s own header prescribes. Analysis stays clock-free.
- **No audio-callback surface is touched**, and no C ABI or OSC vocabulary is widened.
- **The report's own shape is preserved**: a block under the table rather than a column in it, for
  the reason `geom` and the footprint reading already took.

### Lens 3 — docs, bookkeeping, release

The operator-doc sweep was done rather than promised, and a sweep for the retired `10,000,000 B`
figure finds it only in ADRs, closed plans and the archive — all dated records, correctly frozen. No
hotkey, flag, environment variable or `config.toml` key changed; no `.ru.md` source moved. Two
documents did drift and one live sequencing note was overtaken — findings 1, 2, 6 and 7.

**Version:** a feature plan, so a **minor** bump, `0.144.0 -> 0.145.0`, with the studio's two copies
following. **ADRs:** 0231 and 0232 accepted here, each with a dated `Outcome` — 0231 because its own
Decision deferred the constant to Phase 1, 0232 because the machine it said would be named in this
plan was deferred with Phase 3.

### Lens 4 — correctness and determinism

- **No numeric assertion was added anywhere.** The only new constants asserted in tests are
  `budget_ms` (1000/60, a definition) and the slope arithmetic, both properties. This is precisely
  the shape ADR-0071 asks for, and it is the reason a frame-time reading could be added at all.
- **The reading names its machine in both outputs**, so two reports cannot be compared without the
  comparison declaring itself.
- **The software-adapter path fabricates nothing**: no reading, `-` cells, the JSON key omitted (the
  discipline `in_frame_geometry` already has), and a header line saying why. Both branches tested.
- **The capture ordering is safe.** The cost pass runs last within a family and leaves the renderer at
  1080p; the next family's first act is a resize to the probe size, so no other column's capture size
  changes. The hardware run of the suite is what demonstrates this, since CI skips the pass.
- **Nothing takes an aspect from an internal grid.** The one new size is the render target's own
  1920x1080, and it is the first report capture that is not square — which widens what the report sees.
- **What the development configuration cannot see, here, is CI:** `frame_costs` never executes on a
  software adapter, so the timing loop, the interleave and the resize ordering are exercised on a
  developer's machine and nowhere else. That is ADR-0016's deliberate consequence rather than a
  defect, but the green tick from a runner says nothing about that function.

### Lens 5 — design integrity

The plan's two halves stay separate exactly as ADR-0231 and ADR-0232 split them: size is a
packaging-time property with a packaging-time carrier, speed is a report column with no authority.
Neither half reaches into the other, and neither touches the tiers or the governor. One asymmetry
survives — finding 4.

### Findings

- **minor 1 — `docs/capturing.md`: "Two more labeled blocks" is now three.** The frame-cost block
  prints between `geom` and the footprint reading, so the older sentence's count is wrong. **Repaired
  in `a8f09acb`.**
- **minor 2 — `CLAUDE.md`: `packaging/windows/` now carries a recipe.** The layout block said it
  carries *"no recipe of its own, only its reader"*, which Phase 1 falsified in the directory this
  plan changed most. **Repaired in `a8f09acb`.**
- **minor 3 — `packaging/foobar/rlx-version.ps1` names a copy that no longer exists.** Its header
  cited a third copy of the version regex in the release workflow's windows job; Phase 1 deleted it,
  and the job reaches this file through `stage.ps1`. **Repaired in `a8f09acb`.**
- **minor 4 — no guard holds the exe's two constants across its three carriers.** `core/tests/suite/hygiene.rs`'s
  guard (e) holds the component's cap and warning threshold to NFR §4 and re-derives the 90 %, and its
  own doc comment says that guard is what answers ADR-0159's stated negative *"rather than a
  comment"*. The exe's cap now lives in `stage.ps1`, `bundle.sh` and NFR §4 with nothing holding them
  equal. They agree today; nothing will notice when they stop. **Left open — a test is code, which a
  close does not write.**
- **minor 5 — the plan's contract section was rewritten in a lane commit.** `11742290` replaced Phase
  3 with `## Deferred` and amended the header's owner line. Inside a plan an implementing lane writes
  only the `Status:` line and the `## Implementation log`; a deferral is a scope decision. **The
  substance is right and this review ratifies it in full** — the phase is `human`, the hardware is not
  in hand, the walk was extracted, and the verbatim ask is preserved. The finding is that nobody
  outside the lane had seen it before it was committed. **Left open as a process note.**
- **minor 6 — `docs/plans/README.md`: a live sequencing note the close overtook.** The 2026-09-22 note
  on Plan 0223 told this plan's Phase 2 to read a per-pass table that does not exist yet. **Repaired
  in `a8f09acb`** — dated, and turned into the question 0223 inherits.
- **nit 7 — `docs/capturing.md`: "all three tables"** in the design-backlog 0131 narration, where
  there are four; the corresponding test's doc comment was reworded in this very plan and the reader
  document was not. **Repaired in `a8f09acb`.**
- **nit 8 — `packaging/linux/stage.sh` still does not measure.** Two of NFR §4's three series rows are
  Linux readings taken by hand, and the Linux recipe is the one standalone recipe left without a
  measurement block. **Left open**; the natural companion to finding 4.

No earlier round raised findings: this was round 1.

## Followups (after this lands)

- Whatever the deferred Floor reading turns up, as its own backlog entry — including the case where
  everything passes, which is worth recording so the question is not re-opened from scratch. The walk
  waits in [On-device validation](../../on-device-validation.md) until §9's iGPU box is in hand.
- A hard frame-cost gate, if and only if the advisory finds shipped offenders.
