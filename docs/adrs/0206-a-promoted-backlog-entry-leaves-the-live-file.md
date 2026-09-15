# ADR-0206 — A promoted backlog entry leaves the live file, and the ledger lives in the archive

> **Status:** accepted 2026-09-15
> **Date:** 2026-09-15
> **Related plan(s):** none — the migration is the commit that accepts this ADR
> **Amends:** [0108](0108-a-backlog-claim-about-the-repo-carries-an-executable-probe.md) (which live
> entries carry a re-run probe) and [0116](0116-an-index-row-is-a-pointer-and-a-gate-holds-it-to-one.md)
> (which file holds the backlog roster)

## Context

On 2026-09-15 `docs/design-backlog.md` was 321 KB across 59 live entries. The file is the inbox
`architect` reads when deciding what to design next, and every session that opens it pays for all
of it. An owner's question — *can we cut what is implemented?* — was checked entry by entry against
the tree, and the answer was almost nothing: one entry (0160) was delivered in full, five were half
delivered, and none of the rest had landed. The bulk came from two other places:

- **31 entries (~128 KB, 40 %) were already promoted** into approved, unbuilt plans — twelve plans,
  most approved in the 2026-09-14 sweep. The old lifecycle kept a promoted entry in the live file
  *"because a design that has not landed is still live"*. But each of those asks already has an
  owner, ordered phases and done-whens; the entry is that plan's evidence, not a question anyone
  still has to decide. A reader looking for undecided work had to skip 40 % of the file.
- **The closed-entry ledger (~43 KB, 13 %)** sat at the head of the live file. It indexes bodies
  that live in `design-backlog-archive.md`, so the index and its targets were in different files.

## Decision

We will keep `docs/design-backlog.md` to asks **no approved plan owns**. When `architect` approves a
plan whose header names `**Closes:** design-backlog NNNN`, the same session moves each named body
verbatim to `docs/design-backlog-archive.md`, appends a `Moved to the archive … on promotion` bullet
naming the plan, and adds a row to the archive's `### Promoted` roster. At that plan's close, step 3c
of the close ceremony appends the `CLOSED` marker to the archived body and moves the row to
`### Closed`. The ledger — both tables — lives under `## The ledger` in the archive, and
`scripts/check-index-rows.mjs` measures it there. An entry only **partly** promoted stays live with a
dated bullet naming the half a plan took. A plan abandoned or ended on a negative stop gate does not
move a body back; the ask returns as a new live entry citing the archived one, as any returning
question already does.

## Consequences

### Positive
- The live file fell from 321 KB to about 136 KB at the migration, and it now reads as what it says
  it is: undecided friction.
- The promoted set is visible as one table in the archive, which is a roster of in-flight backlog
  debt that did not exist before.
- The ledger sits in the same file as the bodies it points at.

### Negative
- **A promoted entry's probes stop running.** `scripts/check-backlog-claims.mjs` reads only the live
  file, so the "red on delivery" signal some probes were written for no longer fires for a promoted
  entry. What replaces it is the plan's own done-whens and the close log's `### Close triggers`
  bullet that names the `Closes:` entries — a real check, but one the close reviewer reads rather
  than one a script re-runs at every push. A probe falsified while its plan is in flight (a premise
  that goes false before the plan runs) now surfaces at the plan's first phase, not at pre-push.
- **Approving a plan costs a move.** One more mechanical step at approval time, and its failure mode
  is the one step 3c was written for: the marker done and the move forgotten. Nothing gates it.
- Existing links written as `[design-backlog NNNN](../design-backlog.md)` for a moved entry still
  resolve — to the wrong file of the pair. Links in active plans and reader docs are re-pointed at the
  migration; those inside accepted ADRs and closed plans stay as written, and the live file's head
  says where a promoted body went.
- The archive, already 782 KB, grows to about 980 KB. Nothing reads it whole by design.

## Alternatives considered

### Alternative A — cut only what is implemented
Archive 0160 and fold the delivered halves of the partial entries, leave the lifecycle alone. It lost
because it saves about 10 KB of 321: the file is large because of promoted work and the ledger, not
because of forgotten closes — step 3c has been doing its job.

### Alternative B — summarise promoted entries in place
Keep a short stub per promoted entry in the live file. It lost on the archive's founding rule: bodies
are kept verbatim because five entries had their causal claim inverted under verification, and a
summary is where that record is lost. A ledger row already is the stub.

### Alternative C — move promoted bodies, keep the ledger in the live file
Saves the larger share without touching `check-index-rows.mjs`. It lost because it leaves the index
and its targets split across two files, which was the second source of bulk and no reader needs it
at the head of the inbox.
