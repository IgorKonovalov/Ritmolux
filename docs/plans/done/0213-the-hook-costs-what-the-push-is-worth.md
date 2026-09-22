# 0213 — The hook costs what the push is worth

> **Status:** done — closed 2026-09-22. Phases `e3498a57`, `4e08e759`, `60f4c282`, `4ef1ffff`;
> close repairs `d3aa28e4`. Mode 4 (conductor round 1): no blockers, no majors, three minors, one
> nit (two repaired). Full suite served from the pre-review gate's ledger record; push-scope
> self-test 36 of 36. Version 0.141.0.
> **Created:** 2026-09-19
> **Approved:** 2026-09-20 (user)
> **Owner skill(s):** dev
> **Related ADRs:** [0237](../../adrs/0237-the-hook-runs-cargo-only-when-the-push-moved-rust-and-serves-the-rest-from-the-ledger.md)
> (accepted), [0033](../../adrs/0033-testing-strategy-coverage-ratchet-and-pre-push-gate.md),
> [0207](../../adrs/0207-a-suite-run-the-conductor-observed-green-is-not-run-again-on-the-same-tree.md),
> [0211](../../adrs/0211-a-green-suite-record-serves-a-later-tree-when-no-deferred-suite-can-read-the-diff.md),
> [0217](../../adrs/0217-the-node-gate-roster-is-one-manifest-and-a-checker-holds-every-carrier-to-it.md)

## TL;DR

The pre-push hook spends about 400 s of cargo on every push, including the pushes that move four
Markdown files. Plan 0199 established that the work cannot be made smaller, so this plan makes it
conditional: the Node roster always runs, the cargo steps run only when the push moved a
Rust-relevant path, and the suite step is served by a ledger record for that exact tree when one
exists. The first visible behaviour is a docs-only push finishing in the Node roster's own time.

## Context & problem

Three facts, all established rather than assumed:

- **The cost is fixed.** Plan 0199 measured `-P fast` at 426.2 s, 397.8 s, 412.1 s and 368.4 s across
  four arms on one machine, against a run-to-run spread of 30.5 s measured on the same tree. Its own
  log concludes the tier is not faster at the end of that plan than at its start.
- **The evidence often already exists.** `with-lock.mjs` records a green suite against the hash of
  the tree it ran on, and refuses to record a run that started or ended dirty. `main`'s tip after a
  conductor fast-forward is the same commit the `post-close` gate proved, so its record is already in
  the ledger when the owner pushes.
- **Most pushes move no Rust.** Plans, ADRs, roster rows, backlog bodies, queue edits and
  hand-resolved merges are the bulk of what lands here, and no cargo step can fail on any of them.

[ADR-0237](../../adrs/0237-the-hook-runs-cargo-only-when-the-push-moved-rust-and-serves-the-rest-from-the-ledger.md)
records the decision and what it rejects, including why the hook is not simply deleted.

## Decision

Implement ADR-0237: a declared Rust-relevant path set, a scope helper the hook consults, and a ledger
lookup on the suite step. The Node roster is untouched.

## Implementation phases

### Phase 1 — The path set is data, and a self-test convicts it

- **Owner skill:** dev
- **What:** declare the Rust-relevant path set beside the gate roster (`scripts/gates.manifest.mjs`
  is the precedent: data a checker reads, wired into nothing by itself), and a
  `scripts/push-scope.mjs` that answers one question — does a given commit range touch any of them —
  and prints the first path that matched.
- **Files touched:** `scripts/push-scope.mjs` (new), the manifest beside
  `scripts/gates.manifest.mjs`, `scripts/fixtures/push-scope/` (new).
- **How:** the set is at least `**/*.rs`, `Cargo.toml`, `Cargo.lock`, `.config/nextest.toml`,
  `rust-toolchain*`, `.cargo/**`, `**/build.rs`, `presets/**`, `core/shaders/**`. **`presets/**` is
  not optional** — `core/build.rs` globs the library in and `core/tests/suite/preset_schema.rs` holds
  the generated schemas to the engine, so a preset edit is a Rust change in a `.toml` coat.
- **Done when** `--self-test` passes over seeded fixtures that include: a docs-only range (no
  match), a range whose only change is `presets/x.toml` (match), a range touching `Cargo.lock` alone
  (match), and a range with no common ancestor (match, because the conservative answer is yes).

### Phase 2 — The hook asks before it spends

- **Owner skill:** dev
- **What:** the four cargo steps run only when Phase 1's helper says the pushed range is
  Rust-relevant. A skip prints one line naming the reason and the range it read.
- **Files touched:** `.githooks/pre-push`.
- **How:** a pre-push hook is handed `<local ref> <local sha> <remote ref> <remote sha>` per ref on
  stdin. The range is `<remote sha>..<local sha>`; **an all-zero remote sha means the remote has no
  such branch and the range is unknown, which runs everything.** The same conservative answer covers
  a shallow clone and a stdin the hook cannot parse. The Node roster runs before this branch and is
  unaffected by it.
- **Done when** a push whose range touches only `docs/**` runs the Node roster and no cargo step,
  saying so; a push touching one `.rs` file runs all four; and a push with an unreadable or empty
  range runs all four.

### Phase 3 — The suite step is served by the record that already exists

- **Owner skill:** dev
- **What:** when the cargo steps do run, the suite step consults the ledger for the working tree's
  hash and is served by a record for that exact tree, whoever wrote it.
- **Files touched:** `.githooks/pre-push`, a small entry point over `tools/conductor/lib/ledger.mjs`.
- **How:** reuse `greenRecord` rather than re-deriving the key — one reader, one answer, which is the
  rule Plan 0197 Phase 3 already applied to the ledger's location. The printed line names the record
  the way the conductor's own gate prints it: who ran it, when, and the summary.
- **Done when** a push of a tree with a full-suite record skips the suite step and prints the record;
  a push of a tree with only a `-P fast` record runs it (the ledger refuses that substitution by
  design); and a push of an unrecorded tree runs it.

### Phase 4 — What the operator reads is true

- **Owner skill:** dev
- **What:** `docs/developing.md`'s pre-push section and the hook's own header comment describe the
  conditional shape, and the cost figures in that section are re-measured rather than edited.
- **Files touched:** `docs/developing.md`, `.githooks/pre-push` (comment).
- **How:** three readings on the development machine — a docs-only push, a push served by the ledger,
  and a push of an unrecorded Rust change — each named with the machine, as ADR-0071 requires of any
  figure that is a measurement rather than a property.
- **Done when** the section states when each step runs and what a skip prints, the three readings are
  in it, and `node scripts/check-filter-figures.mjs` still passes — the cost prose lives under
  `### What it costs`, which Plan 0196's close separated from the roster for exactly this reason.

## Risks & open questions

- **The path set will be wrong once.** Some build input will not be on the list and a push will skip
  a suite that would have gone red. The list is data with a self-test so the repair is one line and
  one fixture, but the first instance is found by CI rather than by the hook. ADR-0237 records this
  as the price.
- **The ledger's key does not include the toolchain**, so a record written under one `rustc` serves a
  push under the next. Inherited from ADR-0207 rather than introduced here; naming it in the ledger
  key is a separate decision.
- **A `-P fast` record never serves another**, so the saving lands on close tips rather than on every
  commit. If that turns out to be most pushes anyway, the measurement in Phase 4 will say so.

## What this plan does NOT do

- **It does not change the roster.** Every gate ADR-0217's manifest projects still runs on every
  push, unconditionally.
- **It does not make the hook advisory.** A red still refuses the push.
- **It does not touch CI.** The `links` job and the workflow gates stay unconditional, which is what
  makes narrowing the hook affordable.
- **It does not revisit opt-in per clone.** ADR-0033's installation rule stands.

## Implementation log

> Written by `dev` — one row per phase as that phase's commit lands, and the close block after the
> last one. **The phases above are the contract; everything here is what happened.**
> **Observations, never conclusions:** this says where to look, architect decides how it went.
> No per-criterion pass list, no self-assessment, no narrative — but a deviation from the plan or
> an unmet done-when is always disclosed. Stays shorter than `## Implementation phases` above.

**Lane:** `WORK/rlx-plan-0213` on `plan-0213-the-hook-costs-what-the-push-is-worth` (conductor)

| phase | owner | state | commit |
|---|---|---|---|
| 1 — The path set is data, and a self-test convicts it | dev | done | `e3498a57` |
| 2 — The hook asks before it spends | dev | done | `4e08e759` |
| 3 — The suite step is served by the record that already exists | dev | done | `60f4c282` |
| 4 — What the operator reads is true | dev | done | `4ef1ffff` |

### Notes

- Phase 1: the manifest is `scripts/push-scope.manifest.mjs`. Beyond the plan's list it names the five workspace crate directories whole, `.taplo.toml`, rustfmt/clippy configs, and eight paths outside the crates that a Rust test opens (`docs/configuration.md`, `docs/embedding.md`, `docs/nfr.md`, `docs/examples/**`, `docs/images/gallery/**`, `docs/specs/player-schema.json`, `scripts/docs-shots.mjs`, `packaging/foobar/build-component.ps1`).
- Phase 1: the fixture's explanation is `scripts/fixtures/push-scope/README.md`; `scripts/fixtures/README.md` was not given a section.
- Phase 2: the docs-only case was run through the real hook (stdin fed by a scratch Node driver, since a conductor session may not run `sh`); the one-`.rs`, empty, unparseable, new-branch and deletion-only cases were run against `rust_relevant` extracted from the hook, not through the whole hook. The whole hook on a Rust range ran at Phase 4.
- Phase 2: a push of nothing but branch deletions runs the cargo steps (no range to read).
- Phase 3: the entry point is `tools/conductor/suite-record.mjs`, reading the ledger through `with-lock.mjs`'s `suiteLedger` and `lib/ledger.mjs`'s `greenRecord`; `lib/ledger.mjs`'s header comment was edited to name it as a third reader. The hook writes no skip line to the ledger.
- Phase 3: the three done-when cases were checked against scratch ledgers handed over `RLX_SUITE_LEDGER`, with the entry point alone and not through the whole hook; no real full-suite record existed for any tree of this lane.
- Phase 4: the three readings were taken through the whole hook at `60f4c282` (before Phase 4's comment-only hook edit), in this lane with `target/` pre-built by a separate clippy, rustdoc and `nextest --no-run` pass, and with no `studio/node_modules`, so the studio steps skipped. The served reading's record came from a scratch ledger handed over `RLX_SUITE_LEDGER`, not from a real run; `docs/developing.md` says so.
- Phase 4: the ~410 s test-step figure was replaced by the new 544.8 s reading; the 165 s idle figure was kept, dated to its 2026-09-14 reading.
- Followup: `push-scope.mjs --self-test` is not on the gate roster (`gates.manifest.mjs`, CI `links`), so nothing runs it but a person.
- Followup: CLAUDE.md's `scripts/` inventory does not name `push-scope.mjs` or its manifest.

### Close triggers

- **`presets/` touched:** no
- **Plan header `Closes:`** none
- **What shipped:** feature (the pre-push hook's cargo steps become conditional)
- **Operator docs touched:** `docs/developing.md` (the pre-push section)
- **Backlog probes (`node scripts/check-backlog-claims.mjs`):** exit 0
- **Full suite:** owed to the conductor's pre-review gate (ADR-0207)
- **Outstanding `human` phases:** none

## Close review

Conductor round 1, 2026-09-22, a fresh session given the plan and the lane.

**Verdict: Plan 0213 landed cleanly; no blockers, no majors, three minors and one nit (two minors
repaired at the close, in `d3aa28e4`).**

**Evidence.** Full suite: `with-lock: skipped cargo nextest run --workspace: tree f35a67a is green in
the suite ledger, run by gate 0213-pre-review at 2026-09-22T05:16:12.320Z: 1774 tests run: 1774
passed (40 slow), 7 skipped` (ADR-0207), cited in place of a run; the lane touched no Rust.
`node scripts/push-scope.mjs --self-test`: 36 of 36, the four named ranges present by name.
`push-scope.mjs` on the lane's own ranges exits 3, correctly. `tools/conductor/suite-record.mjs`
against the real ledger, not a scratch one, served this tree from the pre-review record, and still
did with a relative `GIT_DIR` in the environment, as git exports to a hook in a main checkout.

**Lens 1.** Four phases, four commits, each one `**Owner skill:** dev`. The manifest was checked
against every Rust source that opens a file outside its crate (`include_str!`, `repo_root().join`,
`CARGO_MANIFEST_DIR/..`) and against both build scripts: every such file is listed. The hook's range
logic is conservative in every branch it cannot decide, reads stdin before any step, and returns from
a heredoc-fed loop rather than a pipe, so `scope_note` survives. `suite-record.mjs` reuses
`greenRecord` and `suiteLedger`; `greenRecord` filters on the exact `SUITE_COMMAND`, so a `-P fast`
record cannot serve. Phase 4's readings name the machine and say the served one used a seeded ledger.
The log is shorter than the phases and discloses its deviations.

**Lenses 2, 4, 5.** No Rust, C ABI or control protocol touched; `suite-record.mjs` is a read-only
third ledger reader and `lib/ledger.mjs` says so. No DSP, geometry or numeric assertion.

**Lens 3.** `docs/developing.md` states when each cargo step runs and what a skip or serve prints.
Version bump: minor, as Plan 0196 took. ADR-0237 accepted; ADR-0033's row gains the inbound
`superseded in part by 0237`.

**Findings.**

- **minor, repaired (`d3aa28e4`)** — `docs/developing.md` said only a new *branch* runs all four; any
  ref with an all-zero remote sha does, including the release tag every `git push --follow-tags`
  close push carries. The sentence now says "a ref" and names the tag case.
- **minor, repaired (`d3aa28e4`)** — `CLAUDE.md`'s `scripts/` inventory named neither
  `push-scope.mjs` nor its manifest; a fourth kind, "a hook helper", now names both.
- **minor, open** — `scripts/push-scope.manifest.mjs:4` says the self-test "holds this list", and
  nothing runs the self-test: it is on neither the gate manifest nor CI, so ADR-0237's mitigation is
  unenforced. The plan forbade a roster change; adding it to `scripts/gates.manifest.mjs` is the
  owner's call.
- **nit, open** — `.githooks/pre-push:118` treats a new tag as an unknown range even when another ref
  in the same push carries the tag's commit, so a tagged push always runs `fmt`, `clippy` and
  `rustdoc` (the test step can still be served).

**Close notes.** Presets untouched, so no curation owed; no backlog entry closed; backlog probes
exit 0. Translation advisory: `docs/running.ru.md` and `packaging/foobar/READ-ME-FIRST.ru.md` trail
their sources, neither moved by this plan.

No earlier round.
