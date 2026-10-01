# ADR-0261 — The conductor parks only on what the owner must settle: a human run parks once, a false repair claim reopens its finding, the close's suite is served, and a diagnosed flake retries by name

> **Status:** proposed 2026-10-01
> **Date:** 2026-10-01
> **Related plan(s):** [0241](../plans/0241-the-conductor-parks-only-on-what-the-owner-must-settle.md)
> **Amends:** [0207](0207-a-suite-run-the-conductor-observed-green-is-not-run-again-on-the-same-tree.md) (which runs a ledger may
> serve), [0209](0209-a-conductor-close-repairs-the-prose-and-comments-its-findings-name.md) (what a false `fixed_in` costs),
> [0248](0248-the-pipeline-repairs-before-it-parks.md) (the park triggers)

## Context

The run record for 2026-09-22 to 2026-09-30 (`tools/conductor/state/conductor.json`, audited
2026-10-01) covers 24 plans. Their lifetimes add up to 366 h. Sessions account for 32 h of that and
parks for 326 h. The conductor ran 29.6 h out of a 198 h span. The owner is the bottleneck, and every
park that did not need the owner spends the scarce resource.

Four shapes in that record did not need the owner, or need the owner less often than they parked:

- **A run of human phases parks once per phase.** `nextStep` returns the whole run of consecutive
  human phases, but the park records only `phases[0]`, and `parkStillTrue` checks only that one.
  The detail tells the owner "Phase 4 is owned by human", so the owner does Phase 4, the lane resumes
  itself, and it parks again on Phase 5. Plan 0232 parked at 10:34, 16:11 and 16:17 for phases 4, 5
  and 6.
- **A false `fixed_in` parks a clean close.** ADR-0209 has the conductor verify each repair a close
  claims. 0221 parked because its fix commit did not change the finding's file across the plan's
  `git mv`. 0212 parked because it named a sha that does not exist. Those two parks waited 24 h
  between them. In both cases the review was clean and the only error was a bookkeeping claim.
- **The close re-runs the full suite on a diff the gate would serve.** `lib/gate.mjs` serves a full
  suite down to `-P fast` when the tree's diff from a green record falls entirely within
  `SERVED_PATHS`. That set includes the version lines of a release bump. `with-lock.mjs` skips only
  an identical tree, so the close session's own `cargo nextest run --workspace` always runs in full.
  0230's close diff was docs, the root `Cargo.toml` and `Cargo.lock` version lines, and the studio's
  two version copies. It ran 550 s of full suite. The close step averaged about 13 min, and the suite
  was about 9 of those.
- **A flake under concurrent load.** Backlog 0219 records `a_preset_datagram_selects_by_name` losing
  a loopback datagram in 3 of 79 loaded runs. The conductor runs two lanes, so it supplies that load.

The record also holds a counterexample that shapes the fourth decision.
`a_headless_run_emits_the_roster_the_preset_and_a_preset_error` went red twice, at 0211's close and at
0212's post-close gate. It was filed as a flake. It was a real race in `Show::reload`, fixed in
`1afa2e9`. A blanket retry would have passed it both times and the race would still ship.

## Decision

We will make four changes. Every other park stays as ADR-0205 and ADR-0248 have it.

1. **A run of consecutive `human` phases parks once.** The park records every pending phase of the
   run. Its detail names them all, e.g. "Phases 4-6 are owned by human". It settles when every one of
   them reads `done`, or `owed` on a phase marked `Blocks merge: no`. A partly settled run stays parked
   and names the phases still open. The digest's settled-park reading and `resume` share that one
   reader, as they share `settledPhase` today.
2. **A false `fixed_in` reopens its finding instead of parking.** These cases count as false: the
   commit does not exist, it is not on the branch, or it changes neither the finding's file nor a
   path that file had at that commit. The conductor drops the claim and records the finding as
   **reopened** with the reason. The digest carries it under **Needs you** like any open finding, and
   `finding NNNN` lists it. The close proceeds if nothing else disagrees. ADR-0209's safety property
   holds unchanged: no finding is ever recorded as repaired on a claim `git` contradicts. The finding
   is now visible as open instead of blocking the merge.
3. **`with-lock` serves the way the gate does.** Suppose a session runs exactly
   `cargo nextest run --workspace` on a clean tree, there is no green record for that tree, and there
   is one for a tree whose diff falls entirely within `SERVED_PATHS`. Then the wrapper runs
   `-P fast` in its place under the same lock and writes a served line. **A green served line
   lets a later run skip on its own exact tree, and nothing more.** A later gate on that tree would
   compute the same serving from the same record, so it skips rather than serving again. A served
   line still never *serves* another tree. `servingRecord` keeps reading full green records only, so
   the ledger's rule that one `-P fast` never serves another stands.
4. **A test may retry once, by exact name, only while a live backlog entry diagnoses it as a
   flake.** The override sits in `.config/nextest.toml` and cites the entry by bare number. Every
   carrier inherits it from that one file: pre-push, CI and the conductor. A pass on retry is still
   reported: nextest prints it as flaky, the ledger records the flaky names, and the digest prints
   them. **The test leaves the list when its entry closes.** A red test with no such entry is never
   retried, whatever it looks like. The initial list is backlog 0219's test, and no other.

## Consequences

**Positive.**

- 0232 would have parked once instead of three times.
- The two `fixed_in` parks (24 h) would not have parked at all.
- Every close saves the difference between a full suite and `-P fast`, about 4 to 5 min on the
  reference box. A full workspace recompile is still paid, because the version bump changes
  `CARGO_PKG_VERSION`.
- A diagnosed flake stops costing a repair session or a park, and it stays visible.

**Negative.**

- **A reopened finding reaches `main` unrepaired.** That is no worse than any finding a close
  leaves open, which ADR-0209 already lets merge. But a close that claimed a repair and got it wrong
  now merges with an open finding, where before the owner saw it first.
- **The close's suite is `-P fast` on the tagged tip.** The nine deferred GPU suites ran in full on
  the reviewed tree, not on the tagged one. The diff between those two trees is, by construction,
  only paths the ledger already trusts not to move them. This is the same bet the gate makes today,
  now made in one more place.
- **A retry list can turn into a place to hide failures.** The "live entry, exact name" rule is
  enforced by review, not by a gate. A future close that finds a name without a live entry has
  found a defect.

## Alternatives considered

- **A repair session for a false `fixed_in`** (an architect session corrects the claim, then the
  conductor re-verifies). Rejected: it spends a session to restore a bookkeeping claim whose only
  consequence is whether one finding reads open or repaired. Reopening reaches the same end state
  for free.
- **Skip the close's suite outright on a served diff.** Rejected: a version bump is read at compile
  time, and `studio/shared/version.test.ts` and the help output exist precisely to catch a version
  that moved alone. `-P fast` runs them, and a skip would not.
- **Bump the version before the review, so the close's tree equals the reviewed one.** Rejected:
  ADR-0005 puts the bump at the close, chosen against the `main` the close merged. A bump before the
  review would be chosen against a `main` that may move during the review.
- **The conductor retries any red suite once before repairing.** Rejected on the record's own
  counterexample: it would have passed the `Show::reload` race at both of its reds.
- **Keep one park per human phase, and make the detail list the whole run.** Rejected: the owner
  could then mark three phases done, and the lane would still resume, re-read and re-park twice. The
  fix belongs in the reader, not the message.
