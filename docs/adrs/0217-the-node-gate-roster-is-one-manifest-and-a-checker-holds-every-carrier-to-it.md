# ADR-0217 — The Node gate roster is one manifest, and a checker holds every carrier to it

> **Status:** proposed
> **Date:** 2026-09-19
> **Related plan(s):** [0196](../plans/0196-the-gate-roster-stops-drifting.md)
> **Rests on:** [0033](0033-testing-strategy-coverage-ratchet-and-pre-push-gate.md) (the hook and its
> budget), [0205](0205-an-approved-plan-runs-under-a-conductor-and-every-judgement-it-cannot-make-parks-the-plan.md)
> (the conductor's own gate)

## Context

Three carriers run the same roster of Node gates, for three different reasons.
`.githooks/pre-push` runs it before a push and is opt-in per clone and bypassable with
`--no-verify`. CI's `links` job runs it on `ubuntu-latest`, where it can be neither skipped nor
bypassed, and reports **after** the push. `defaultGate()` in `tools/conductor/lib/gate.mjs` runs it
at `pre-review` and `post-close` for a plan the conductor runs itself — the only one of the three
that reads a conductor-run plan **before** the owner pushes it.

A plan that adds a gate is told, by CLAUDE.md and by every ADR that has added one, to edit the first
two. Nothing points at the third, and it has fallen behind twice: `check-translations.mjs`
(Plan 0166) and `check-system-counts.mjs` (ADR-0202, Plan 0178) are in the hook and in CI and are
absent from `defaultGate()`. Neither plan did anything wrong — each wired its gate exactly where it
was told — so the defect is that a third list exists and no rule, gate or ceremony step names it.

The same class arrives from a second direction. The hook's `cargo doc` step is scoped to
`-p rlx-core` while CI documents `--workspace`, so four of the workspace's five members are
documented in CI and nowhere else. A rustdoc error in one of the four is structurally unreachable
before a push, and it has shipped a red `main` under a release tag twice in sixteen days
(Plan 0137, Plan 0180). That gap was written down in a comment beside the CI job at the moment it
was created, which is the one place a reader looking for open work never looks.

This repository has made the same substitution three times: when a convention keeps failing, it
takes a checker — [ADR-0116](0116-an-index-row-is-a-pointer-and-a-gate-holds-it-to-one.md) for index
rows, [ADR-0149](0149-a-backlog-reference-is-a-bare-number-and-a-file-link.md) for backlog
references, [ADR-0202](0202-a-written-count-of-the-systems-is-refused-by-a-gate.md)
for system counts. What is new here is that the convention spans three files in three languages, one
of which is already data.

The carriers are not interchangeable and a single runner cannot replace them: the hook is shell and
skips its whole Node block when `node` is absent; CI is YAML and is the un-bypassable one; the
conductor's is JavaScript that it already reduces to commands, keeps a per-step log for under
`state/gates/`, and stops at the first red. Two further gates — `check-site-links.mjs` and
`check-site-routes.mjs` — need a **built** site and so run in neither the hook nor the `links` job,
only in `.github/workflows/pages.yml`. That absence is correct, and any shape has to keep it
expressible rather than reading it as drift.

## Decision

We will hold the Node gate roster in one committed manifest, `scripts/gates.manifest.mjs`, as an
ordered list of invocations, each declaring the carriers it belongs to. `defaultGate()` imports the
manifest and splices its conductor projection into the gate, so the conductor's list cannot fall
behind by construction. The hook and the `links` job keep their own native invocations, and
`scripts/check-gate-carriers.mjs` — itself a gate in the roster, carried by all three — asserts that
each carrier's invocation list equals the manifest's projection for that carrier, in the same order.
A gate whose spelling differs between carriers is **two entries with disjoint carrier sets**, never
one entry with per-carrier arguments, so the checker stays a plain equality over projections; a gate
a carrier deliberately lacks says so in its carrier set, which is how the two site gates are
recorded.

## Consequences

### Positive
- The conductor's roster is no longer a copy. It is the manifest, read at run time, and the two
  gates it is missing today arrive with the import.
- An absence becomes a declared fact. `check-site-links.mjs` not being in the hook is a carrier set,
  readable next to the gate it belongs to, rather than a hole nobody can distinguish from a mistake.
- Adding a gate edits the manifest plus its carriers, and forgetting a carrier is red at the push
  rather than a year later.
- The class is closed for the next gate, not only for the two missing now — which is the distinction
  that made the obvious repair (adding two names) the wrong one.

### Negative
- **The checker reads a shell script and a YAML file with regexes.** That is a fourth parser to keep
  working, and it can be evaded by a spelling it does not recognise — `node "scripts/x.mjs"`, a
  loop, a composite action. A fixture self-test bounds this; it does not remove it.
- **It asserts invocations and order, never the conditions a carrier attaches.** CI runs
  `check-release-tag.mjs --remote` under an `if:` guard for a push to `main`; the checker sees the
  invocation and not the guard. A condition is prose the manifest records and nothing enforces, and
  that bound is stated here so nobody later reads a green checker as saying more than it does.
- **The hook's skip notice stays hand-written.** Its `if command -v node` block names the gates in an
  English sentence, and holding that sentence to the manifest would mean parsing prose. It will drift
  the next time a gate is added, and the cost of that drift is a notice that under-names what was
  skipped.

### Neutral
- Non-Node steps stay out of the manifest. `fmt`, `clippy`, `nextest`, `cargo doc`, the studio's
  three and the sd-filter suite differ between carriers for reasons that are decisions rather than
  drift — the hook's narrowed `-P fast`, CI's GPU-less profile, the conductor's full suite.

## Alternatives considered

### Alternative A — One runner every carrier calls
A single `node scripts/gates.mjs` that runs the roster, invoked identically by the hook, the `links`
job and the conductor. It is the smallest source of truth and it loses the thing that makes a red
cheap to read: fifteen CI steps collapse into one opaque step, the hook's per-step timings into one
number, and the conductor's per-step logs under `state/gates/` into a single file. Rejected because
the conductor needs the steps as data regardless, which means the roster has to be data anyway — at
which point the runner buys nothing the manifest does not.

### Alternative B — Derive the roster by parsing the hook
The hook already lists every gate in order, so a conductor that read its `run_step "node scripts/…"`
lines could not fall behind. Rejected because it makes a shell script the source of truth for three
consumers, and it leaves CI — the un-bypassable carrier, and the one whose red is discovered last —
entirely outside the derivation. Same parser cost as the checker, with less coverage.

### Alternative C — Add the two missing names
One edit, and `defaultGate()` is correct again. Rejected because the defect is a list nobody is told
to update, not two absent lines: the repair restores the invariant for a day and rebuilds the trap
underneath it. That judgement is backlog 0252's own, written before any plan existed to take it.

## Notes

Raised as backlog 0252 (2026-09-18, at Plan 0178's close) and backlog 0246 (2026-09-17, after
Plan 0180's close), which is the same class reaching the roster through `cargo doc` rather than
through a Node gate. Both are taken by Plan 0196.
