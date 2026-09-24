# ADR-0249 — A human phase marked as not blocking the merge is owed after it, not waited for

> **Status:** proposed
> **Date:** 2026-09-24
> **Related plan(s):** [0226](../plans/0226-the-conductor-stops-waiting-for-the-owner.md)
> **Amends:** [0205](0205-an-approved-plan-runs-under-a-conductor-and-every-judgement-it-cannot-make-parks-the-plan.md)
> (a `human` phase as a park trigger)

## Context

ADR-0205 parks a plan at every `human` phase. It rejected stopping the whole lane there (Alternative
E) and running a human phase's mechanical parts (Alternative G). It did not weigh the choice the
record now argues for: **merging what the machine built and owing the human phase afterwards**.

Two of the 10 parks in the 2026-09-22 to 2026-09-24 run record were `human_phase`. Together they are
the largest share of idle time:

- **0207 Phase 3** parked at 20:36 and was picked up 17 h later. The close then **deferred the phase
  anyway**: the lane rewrote the plan's contract section to record the deferral, and the review
  ratified it. Two phases of finished work waited a night for a phase that did not happen.
- **0224 Phase 7** parked at 20:19 and was picked up 16 h later. Phase 7 was an on-device reading
  (a Windows bench run) and no later phase consumed its output.

Most human phases in this repository have this shape: an on-device check, a rig session, or a
judgement of what shipped. They check a result and nothing downstream builds on them. Some do not: a
signing certificate, a corpus someone has to fetch, or a measurement a later phase uses as its
threshold. Nothing in the plan format distinguishes the two today, so the conductor has to assume
the worst for every one.

## Decision

We will let a `human` phase carry **`- **Blocks merge:** no`** beside its owner tag. **Without the
field a human phase blocks, exactly as today.** The field is invalid on any other owner, and the plan
reader reports it as an error.

- **The conductor skips a non-blocking human phase** and records it as owed. It runs the phases after
  it, reviews, closes and merges as though the phase were not there. Its `## Implementation log` row
  reads `owed`, and is never `done` until the owner makes it so.
- **The close treats `owed` as a legitimate row.** The plan moves to `done/`, its `Status:` line reads
  `done` and names the owed phase, and the `## Close review` states what the owed phase has not yet
  checked. The version bump and tag are the close's as usual.
- **The owner finds the debt on the digest.** **Needs you** lists every plan under `docs/plans/done/`
  with an `owed` row, read from the main checkout's tree and not from `state/`, so it survives a
  wiped state directory and a close that happened outside the conductor. When the owner does the
  phase, they mark the row `done` in the closed plan on `main` and commit it, and the line leaves the
  digest. No command records it, because the plan file is the record.
- **A phase that finds a problem does not reopen its plan.** Its finding becomes a backlog entry or a
  new plan, as any post-close finding does.
- **The author marks the field, and the readiness check holds it to its promise.** The architect
  writes `Blocks merge: no` only on a phase whose output no later phase reads and whose absence
  leaves every claim of the plan true, if unverified. ADR-0248's readiness session rejects a plan in
  which a later phase depends on a phase marked this way.

## Consequences

### Positive

- 0207 and 0224 would have merged on the evening they were built. Their dependants (0206 behind
  0207, 0223 behind 0224) would have started that night, not the next day.
- The deferral 0207 improvised, which was a lane rewriting its own contract section, becomes a
  declared property the plan carried from approval.
- An owed on-device check is a line on the page the owner reads before every push, rather than a
  parked worktree holding a disk slot.

### Negative

- **A tagged version can carry a feature nobody has run on the device it is for.** The push is still
  the owner's, and the digest lists the owed phase beside the tag, so the mitigation is a reading, as
  with every open finding. Nothing stops pushing an unvalidated tag. That is the price.
- **"done" now means "the machine's part is done" for some plans.** Anything that reads `Status: done`
  as "every phase happened" reads these plans wrongly. The `owed` row and the status line's own text
  are what say otherwise, and the plans index's recently-closed bullet must name the owed phase too.
- **An owed phase can stay owed forever.** The digest carries it until it is done. That is a standing
  line, not a gate, and it has the same weakness as the open findings ADR-0216 lets the owner close.
- **The author's judgement moves earlier.** Whether a phase blocks is decided at approval, when the
  architect knows least about what the phases will produce. The readiness check verifies the
  dependency half, but not whether the unverified claim was an acceptable risk.

## Alternatives considered

### Alternative A — Every trailing human phase is non-blocking by default

It needs no new field. Rejected because it makes a plan's shape carry a meaning nobody wrote down: a
phase that happens to be last, such as a signing certificate needed before release, would stop
blocking silently.

### Alternative B — No human phase blocks

It is simplest and fastest. Rejected because some human phases are inputs, such as a certificate, a
fetched corpus or a measured threshold. Building past them produces work that has to be redone, or
worse, work that silently used a default in their place.

### Alternative C — Split the human phase into its own follow-up plan at authoring time

It keeps `done` meaning done. Rejected because it doubles the plan overhead for the commonest human
phase in the repository, and it separates a feature from the check written for it. That is the
record the owner reads when deciding whether the check still needs doing.
