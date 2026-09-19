# ADR-0237 — The hook runs cargo only when the push moved Rust, and serves the rest from the ledger

> **Status:** proposed
> **Date:** 2026-09-19
> **Related plan(s):** [0213](../plans/0213-the-hook-costs-what-the-push-is-worth.md)
> **Supersedes in part:** [0033](0033-testing-strategy-coverage-ratchet-and-pre-push-gate.md) — its
> roster, its opt-in-per-clone nature and its Alternative F exclusions all stand; what this replaces
> is the rule that the hook's cargo steps run on every push
> **Rests on:** [0207](0207-a-suite-is-locked-and-a-green-tree-is-recorded-once.md) (the ledger and
> its tree key), [0211](0211-a-green-suite-record-serves-a-later-tree-when-no-deferred-suite-can-read-the-diff.md)
> (serving a record), [0217](0217-the-node-gate-roster-is-one-manifest-and-a-checker-holds-every-carrier-to-it.md)
> (the Node roster is a manifest), [0222](0222-a-preset-sweeps-fixed-cost-is-paid-per-process-so-the-lever-is-the-batch.md)
> (what Plan 0199 measured)

## Context

**The hook's cargo steps cost about 400 s, and Plan 0199 proved that number cannot be trimmed.**
Four arms on one machine measured `-P fast` at 426.2 s before anything, 397.8 s after the
exclusive-testcase fold, 412.1 s after the sweeps were batched, and 368.4 s after the guard landed —
every difference inside the 30.5 s run-to-run spread that plan measured on the same tree. The
conclusion its log states is the premise here: *"`-P fast` is not faster at the end of this plan than
at its start, and both halves of the plan moved it."* A step whose work cannot be made smaller can
only be made conditional.

**What the hook is for was settled by the owner on 2026-09-19: the author's loop.** It is not the
boundary that keeps a red off `origin` — CI is, because CI cannot be skipped, cannot be bypassed and
runs on a machine nobody configured for the occasion. The hook exists so that a mistake costs seven
minutes locally instead of fifteen minutes of CI and a second push. That framing is what licenses
narrowing: a check the hook skips is not a check nobody runs.

**Two things have changed under ADR-0033 since it was written.** The conductor
([ADR-0205](0205-an-approved-plan-runs-under-a-conductor-and-every-judgement-it-cannot-make-parks-the-plan.md))
now gates every step of every lane it runs, and the suite ledger
([ADR-0207](0207-a-suite-is-locked-and-a-green-tree-is-recorded-once.md),
[ADR-0211](0211-a-green-suite-record-serves-a-later-tree-when-no-deferred-suite-can-read-the-diff.md))
records a green run against the **hash of the tree it ran on**, refusing to record at all when the
tree was dirty at either end. A conductor-produced push therefore arrives at the hook carrying
machine-written evidence that its exact tree is green — and the hook re-runs the suite anyway,
because nothing told it to look.

**And most pushes here move no Rust at all.** On 2026-09-19 the pushes were plans, ADRs, roster
rows, backlog bodies, a queue edit and hand-resolved merge commits. `cargo fmt`, `clippy`, `nextest`
and `doc` cannot fail on a diff that contains no Rust, no manifest and no build input — they can only
cost 400 s to say so.

## Decision

**The Node roster stays unconditional, and the cargo steps become conditional in two independent
ways.**

1. **The Node roster runs on every push, exactly as ADR-0217's manifest projects it.** It costs tens
   of milliseconds, it is what catches the hand edits this project actually makes — a plan moved to
   `done/` breaking links in both directions, a roster row over cap, a stranded release tag — and
   nothing below may skip it.

2. **The cargo steps run only when the pushed range touches a Rust-relevant path.** The set is
   declared as data beside the manifest, not spelled inside the hook, and it is a superset of
   *"`.rs` files"*: anything a build reads or a test asserts on. `**/*.rs`, `Cargo.toml`,
   `Cargo.lock`, `.config/nextest.toml`, `rust-toolchain*`, `.cargo/**`, `**/build.rs`, `presets/**`
   and `core/shaders/**` at least. **`presets/**` is in that list because `core/build.rs` globs the
   preset library into the binary and `core/tests/suite/preset_schema.rs` holds the generated schemas
   to the engine** — a preset edit is a Rust-relevant change wearing a `.toml` extension, and a path
   rule that missed it would skip the only test that catches it.

3. **When the cargo steps do run, the suite step consults the ledger by tree hash and is served by a
   record for that exact tree, whoever wrote it** — a conductor gate, a review session, or an
   operator's `hand` run. The writer does not matter because the record's meaning does not depend on
   it: `with-lock.mjs` writes one only for a run it saw exit zero on a tree that was clean when the
   run started and when it finished.

4. **A served or skipped step prints why, and a red still refuses the push.** Nothing here makes the
   hook advisory; it makes it quiet when it has nothing to add.

**The two mechanisms compose without overlapping, which is the point.** A conductor-produced push
carries a full-suite record for its tree — `main`'s tip after a fast-forward *is* the close tip the
`post-close` gate proved — so it is served. A hand push of docs moves no Rust-relevant path, so the
cargo steps do not run. A hand push that *does* move Rust has neither a record nor an excuse, and
pays the full 400 s, which is correct: that is the one case where the hook is the only machine that
has ever seen the code.

## Consequences

### Positive

- **A docs push costs the Node roster alone.** That is the majority of pushes on this project, and
  the saving is the whole cargo bill rather than a trim of it.
- **A conductor-produced push costs the roster plus a ledger lookup.** The suite that would have run
  is the same suite that already ran, on the same tree, minutes earlier.
- **The hook stops competing with the conductor for the machine.** Two full suites on one box
  serialize on the suite lock; 2026-09-19 saw 6-minute lock waits with three lanes live.
- **The expensive case is the honest one.** Hand-written Rust — the class with no lane, no gate and
  no record — is exactly what still pays in full.

### Negative

- **The tree hash cannot see the toolchain.** `lib/ledger.mjs` says so in its own comment: *"The key
  leaves out everything outside the tree: gitignored files, the GPU adapter, the installed
  toolchain."* A record written under one `rustc` serves a push under the next. This ADR **inherits**
  that risk rather than inventing it — ADR-0207 accepted it for the conductor's own gates, which is a
  strictly larger exposure than one opt-in hook — but the hook widens who is exposed to it. A
  toolchain change that breaks a build will be caught by CI on the same push, which is the backstop
  the author's-loop framing already relies on.
- **A path list is a judgement that will be wrong once.** Some file will turn out to be a build input
  nobody listed, and a push will skip a suite that would have gone red. The mitigation is that the
  list is data with a self-test rather than a condition buried in shell, so the repair is a line in a
  manifest and a fixture — but the first instance will be found by CI, not by the hook.
- **`-P fast` is never served by another `-P fast`.** The ledger deliberately refuses that
  (`lib/ledger.mjs`: *"one `-P fast` never serves another"*), so a push whose tree was proved only by
  a phase gate — not by a full suite — runs the suite. The saving therefore lands on close tips and
  not on every intermediate commit, which is a narrower win than it first appears.
- **Two carriers of the same roster now behave differently.** CI runs everything unconditionally; the
  hook does not. ADR-0217 exists because carriers drift, and this deliberately introduces a
  difference between them — bounded, because the *roster* stays identical and only the hook's
  execution is conditional.

## Alternatives considered

- **Remove the hook entirely and let CI be the only gate.** Rejected on the owner's framing rather
  than on safety: the value is a failure the author sees before the push, and CI reports after it.
  Once the cost is conditional the hook is nearly free, so removing it buys the loop nothing and
  costs it the fast failure.
- **Keep the cargo steps unconditional (status quo).** Rejected by measurement. Plan 0199 spent a
  whole plan establishing that the work cannot be made smaller; paying 400 s on a push that changed
  four Markdown files is the cost with none of the benefit.
- **Trust only records written by a conductor gate.** Rejected because the distinction is not real:
  `with-lock.mjs` applies the same clean-at-both-ends rule to a gate, a session and a `hand` run, and
  ADR-0207 already counts a hand run as evidence. Narrowing the writer would forfeit the operator's
  own suite — the one a person runs precisely when they are about to push.
- **Add a freshness window to the record.** Rejected as a guard that does not guard: the risk is a
  toolchain change, and a toolchain can change in an hour or stand for a month, so an age threshold
  approximates the wrong variable. If the toolchain is to be guarded, the repair is to put its
  identity in the ledger key, which is an ADR of its own.
- **Drop `nextest` from the hook and keep `fmt`/`clippy`/`doc`.** Rejected because it gives up the
  case this decision most wants to keep: hand-written Rust with no record anywhere. That is the
  configuration that produced Plan 0095's cross-crate red, which the hook caught and a scoped run
  could not.
