# 0213 — The hook costs what the push is worth

> **Status:** draft
> **Created:** 2026-09-19
> **Owner skill(s):** dev
> **Related ADRs:** [0237](../adrs/0237-the-hook-runs-cargo-only-when-the-push-moved-rust-and-serves-the-rest-from-the-ledger.md)
> (proposed), [0033](../adrs/0033-testing-strategy-coverage-ratchet-and-pre-push-gate.md),
> [0207](../adrs/0207-a-suite-is-locked-and-a-green-tree-is-recorded-once.md),
> [0211](../adrs/0211-a-green-suite-record-serves-a-later-tree-when-no-deferred-suite-can-read-the-diff.md),
> [0217](../adrs/0217-the-node-gate-roster-is-one-manifest-and-a-checker-holds-every-carrier-to-it.md)

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

[ADR-0237](../adrs/0237-the-hook-runs-cargo-only-when-the-push-moved-rust-and-serves-the-rest-from-the-ledger.md)
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
