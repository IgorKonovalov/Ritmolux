# ADR-0233 — A session-allowlist safety claim is asserted against a transcript, not a model

> **Status:** proposed
> **Date:** 2026-09-19
> **Related plan(s):** [0208](../plans/0208-the-conductors-safety-claims-get-their-evidence.md)

## Context

The conductor runs headless sessions unattended, bounded by `tools/conductor/settings.conductor.json`.
Two of that file's rules are safety rules rather than convenience rules: `Bash(rm *)` and
`PowerShell(Remove-Item *)` are allowed wholesale, and the bound on them is a set of deny rules for
the four ways a written path leaves the worktree — `..`, `~`, a leading `/`, and a drive letter.
`tools/conductor/README.md` states the guarantee as *"a path that leaves the lane is refused,
whatever it is for"*.

**That is not the bound the rules enforce.** A glob over command text cannot bound a path that does
not exist until the shell runs it, so `rm -rf $HOME/.cargo`,
`rm -rf "$(git rev-parse --show-toplevel)/../rlx-plan-0180"` and
`Remove-Item -Recurse $env:USERPROFILE\WORK` all match no deny rule and are allowed
(backlog 0237). The blast radius of the first is the machine, not the repository.

**What asserts the guarantee cannot see that either, and is already known to be wrong about a case
it states.** `tools/conductor/test/settings.test.mjs` decides every case through a `decide()` it
implements itself, and its own header says *"It is a model of the CLI, not the CLI: it pins what this
file means, and a CLI that changed its matcher would not turn it red."* One clause of that model is
that a compound command splits at `&&`, `||`, `;`, `|`, `&` and newlines and every part must be
allowed on its own, from which it asserts that `cd studio && npm run typecheck` is denied. Step
`0191-01-implement` on 2026-09-16 ran 26 shell calls carrying a `cd` under this exact file on CLI
2.1.273; **22 of them ran.** The denials correlated with `/tmp` and with `sed` / `head`, not with
`cd`. What the real rule is has not been established (backlog 0241).

So the deny half — the half whose failure costs a deleted directory rather than a refused command —
rests on a model that has been falsified on a compound shape it asserts outright, and the README
repeats a guarantee neither the rules nor the test can support. The decision is what a safety claim
about this file must be asserted against.

## Decision

**A claim that the allowlist refuses something is asserted against a recorded transcript of the real
CLI; a claim that it permits something may stay a model.** `spike/probe.mjs` already runs sessions
under `settings.conductor.json` and records what the CLI did, so the instrument exists: a probe
session attempts a fixed roster of compound, expansion and escape shapes, and its RAN/DENIED verdict
per shape is committed beside the CLI version it was taken on, in the shape ADR-0208 already uses for
the version table. `settings.test.mjs` keeps `decide()` for the allow cases and moves every deny case
the probe covers onto the recorded outcome, so a deny case whose transcript says RAN is a red test
rather than a green one.

The asymmetry is the whole decision: being wrong about an allow case costs a session one turn and is
visible in its log, and being wrong about a deny case is unbounded and silent. We pay for evidence
only where the cost of being wrong is unbounded.

**What the deny rules then become is chosen from that transcript and not before it.** The plan's
first phase is the probe; the shape of the expansion bound is decided from what it records, and a
Phase 1 result that contradicts this ADR's premise supersedes it.

## Consequences

### Positive
- The one claim in this file whose failure is unbounded stops being underwritten by a model.
- A CLI version that changes its matcher now turns something red. Today it cannot, by the test's own
  admission — which is also the gap ADR-0208 closes for the version table, so this extends a
  discipline the conductor already has rather than inventing one.
- The `cd` question gets answered. Both consequences backlog 0241 names hang on it: whether the
  prompt's *"one command per call, no `cd`"* rule is guidance the sessions correctly ignore, and
  whether compound splitting happens at all.

### Negative
- **A transcript is a measurement, so it names its CLI version and goes stale on the next one.**
  ADR-0071's rule applies in full: the committed table is evidence about 2.1.273 and nothing more, and
  a version spike is now a reason to re-run the probe rather than only to re-read the version table.
  That is recurring cost with no gate behind it.
- **A probe session spends money to assert a negative.** It is cheap next to a plan run, and it is
  paid again on every CLI bump we choose to re-verify.
- **The transcript can only cover shapes someone thought of.** It converts "we modelled the matcher"
  into "we observed these shapes", which is stronger and still not a proof. A shape absent from the
  roster is as unasserted as it was before.

### Neutral
- The allow cases stay a model, so most of `settings.test.mjs` does not change.

## Alternatives considered

### Alternative A — keep the model and widen it
Teach `decide()` whatever the real matcher turns out to do. Rejected because it repairs the model
without changing what the model *is*: the next divergence is as silent as this one was, and the
thing that found this divergence was a transcript rather than a re-reading.

### Alternative B — bound the deletion verbs by allowlisting scratch roots instead
Replace `Bash(rm *)` with `Bash(rm -rf target/*)` and nothing else. Narrower and it needs no probe —
but backlog 0231 records deletions outside `target/` that sessions legitimately make, so this trades
an unbounded rule for one that refuses correct work, and it still leaves the README's claim asserted
against nothing.

### Alternative C — accept the gap
The lane is a worktree holding nothing irreplaceable and a sibling lane is recoverable from its
branch. Rejected on the ground backlog 0237 states against itself: the blast radius of `$HOME` is the
machine rather than the repository, which is the argument against accepting it rather than for.

### Alternative D — deny the expansion syntax and stop there
`rm *$*`, `rm *%*`, `Remove-Item *$*`, with no probe. This is likely to *be* the chosen bound, and it
is rejected as the whole answer: it is a guess about what the matcher does with a glob, made by the
same reasoning that produced the falsified compound clause, and one probe run tells us whether it
holds. Deciding the shape is Phase 3's job precisely because Phase 1 is what earns it.

## Notes

The falsifying transcript is `0191-01-implement`, 2026-09-16, the first unattended two-lane run.
Backlog 0237 carries the escaping shapes; 0241 carries the compound finding and the two consequences.
