# ADR-0207 — A suite run the conductor observed green is not run again on the same tree

> **Status:** proposed
> **Date:** 2026-09-15
> **Related plan(s):** [0189](../plans/0189-the-conductor-can-be-watched-and-stops-re-proving-a-green-tree.md)
> **Amends:** [0205](0205-an-approved-plan-runs-under-a-conductor-and-every-judgement-it-cannot-make-parks-the-plan.md)
> (where a conductor-run plan's full suite runs), [0156](0156-the-per-phase-gate-is-scoped-and-the-suite-is-owed-once-per-plan.md)
> (who owes the once-per-plan run, in conductor mode only)

## Context

On the 2026-09-15 pilot a full `cargo nextest run --workspace` held the suite lock for 10.4 to 11.0
min. Thirteen of them ran across three plans: seven inside sessions, six in the conductor's own gate.
A plan with no fix round runs it up to five times: `dev`'s Step 4 at the last implementer run, the
conductor's `pre-review` gate, the review's Mode 4 lens 1, the close sequence's step 2, and the
conductor's `post-close` gate. Two of those pairs run on byte-identical trees. `pre-review` reads the
tree `dev`'s Step 4 just passed plus a close block in the plan file, and the review reads exactly
the tree `pre-review` passed. Backlog 0223 has the wall-clock figure: 72 min of a 218 min run was
gate.

Each repetition exists for a reason, and it is the same reason every time: **a session's claim
that the suite passed is not evidence** (ADR-0205, "the conductor believes the repository, not the
session"). The review runs the suite itself because `dev`'s `Full suite:` bullet is prose. The
conductor gates because the review's run is prose too. But when the run went through
`tools/conductor/with-lock.mjs`, the process that saw the exit code is conductor code, not the
model. The evidence already exists, and nothing keeps it.

The obvious narrower rule is wrong for this repository. "Skip when only docs changed" fails because
Rust tests read `docs/configuration.md`, `docs/nfr.md`, `docs/embedding.md`, `docs/examples/` and
`docs/images/`. A path classifier would need a guard holding it to every test's reads, and it would
stay a heuristic.

## Decision

**The conductor keeps a suite ledger, and a full workspace suite is skipped on a tree the ledger
already records green.** One JSON line in `tools/conductor/state/suite-ledger.jsonl` per run of exactly
`cargo nextest run --workspace` that conductor code observed: the tree (`HEAD^{tree}`), the exit
code, nextest's `Summary` line, who ran it (a gate stage or a session label) and when. A run is
recorded only if the worktree was clean (no tracked or untracked changes) both when it started and
when it ended, so the tree hash names exactly what was tested. Two writers and two readers, and no
others:

- **The conductor's gate** records every full-suite run it makes. Before running one, it looks the
  worktree's tree up and **skips on an exact match with exit 0**, logging the skip and the record it
  relied on.
- **The suite wrapper, in a conductor-run session** (the conductor passes the ledger path in the
  environment), does the same for a session's run. The printed notice names the record: tree, who
  ran it, when, and its `Summary` line. That notice is what a conductor-mode review cites in place
  of re-running the suite.

**The match is tree identity, never a judgement about what changed.** The saving comes from ordering
the work so identical trees happen, not from deciding that a difference does not matter:

- **`dev`, in conductor mode, stops running the full suite at the last implementer run.** The
  conductor's `pre-review` gate runs next on the same code, and a red there parks as `gate_red`. The
  close block's `Full suite:` bullet says the run is owed to that gate.
- **A conductor-mode review runs its Mode 4 suite through the wrapper**, which skips on the tree
  `pre-review` passed.
- **A conductor-mode close runs its whole gate last, on the tip it will tag.** It repairs, merges
  `main`, does the bookkeeping, bumps the version and syncs the studio first. The tag is written
  after the gate, and a tag does not change the tree, so `post-close` finds the close's run in the
  ledger.

A plan with no fix round, whose `main` did not move, then runs the full suite **twice**: at
`pre-review`, and in the close's gate. Every other gate command (`fmt`, `clippy`, `doc`, the node
checks, the studio's) still runs at every stage. They cost minutes, not the ten the suite costs.

Outside conductor mode nothing changes: no ledger path is set, and a human-started review runs the
suite itself exactly as Mode 4 says.

## Consequences

### Positive

- **About half of a plan's suite time goes.** From up to five runs to two, at ~10.7 min each. That
  is a rough estimate of 25 min per plan, or about 75 min over the pilot's three.
- **The evidence gets stronger, not weaker.** A review cites a record the wrapper wrote from an exit
  code, where it used to compare its own run against a prose bullet.
- **It is the first number lane b needs.** ADR-0205's Outcome makes the serialized suite fraction the
  reason a second lane buys little, and this lowers that fraction.

### Negative

- **Fewer repetitions find fewer flakes.** A clock-reading test that fails one run in twenty had five
  chances per plan to show itself, and now has two. ADR-0193 calls a flake a defect, not noise,
  but the conductor's repetitions were catching some of them for free.
- **Anything a test reads that is outside the tree is not in the key.** That covers gitignored files,
  the GPU adapter, and the installed toolchain. A toolchain update between two runs on one tree is
  the realistic case. The ledger is per machine, which bounds it, and the risk is written here
  rather than guarded.
- **The close's ordering changes in conductor mode only**, so the human-started worktree close
  sequence and the conductor-mode one now differ at a second point.
- **A close whose final gate goes red has already moved its plan to `done/`** on the branch. It parks
  `check_red` in that state, and the owner's recovery reads a branch where the bookkeeping ran ahead
  of the evidence.

## Alternatives considered

### Alternative A — Skip on a changed tree whose changes are only documentation
Skip a full suite when the diff since the last green run touches only non-test paths. Rejected
because this repository's Rust tests read five places under `docs/`. The skip would need a path
list, plus a guard holding that list to every test's reads. A guard like that finds a
`.join("docs")` only by heuristic. Tree identity needs neither.

### Alternative B — Run the heavy gate commands concurrently
Rejected for now. `clippy`, `nextest` and `doc` contend on one `target/` and on cargo's own build
lock, so the likely result is serial execution with extra scheduling. It was never measured, and it
saves nothing on repeated runs over one tree, which is where the time goes.

### Alternative C — Keep every run
Rejected on the pilot's figure. Unattended machine time costs no attention, but a third of the wall
clock re-proves a result the conductor's own code already observed, and it is the constraint
ADR-0205's Outcome names against lane b.

### Alternative D — Trust a session's `Full suite:` bullet
Rejected. It is exactly the claim ADR-0205 refuses. The wrapper's record is different in kind: it is
written by the process that observed the exit code, not by the model that reads it.
