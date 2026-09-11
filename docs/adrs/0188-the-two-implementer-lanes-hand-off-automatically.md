# ADR-0188 — The two implementer lanes hand off to each other automatically, and every other seam stays manual

> **Status:** accepted 2026-09-10 (no plan; the rollout lands in the same commit)
> **Date:** 2026-09-10
> **Related plan(s):** none — the four-document rollout is this ADR's own commit.
> Evidence comes from [0159 — The studio opens](../plans/done/0159-the-studio-opens.md) and
> [0167 — The studio becomes handable](../plans/done/0167-the-studio-becomes-handable.md).
> **Related:** [ADR-0177](0177-a-fourth-skill-lane-builds-the-studio.md) (added the lane, and
> decided the opposite of this; **amended here**), [ADR-0017](0017-preset-author-skill-lane.md)
> (the lane-by-ADR precedent), [ADR-0120](0120-the-close-brief-is-a-section-of-the-plan.md)
> (the `## Implementation log`, which is why the payload can be three lines)

## Context

ADR-0177 added `studio-builder` and, in the same breath, refused to automate its handoff with
`dev`:

> **Handoffs stay manual.** [...] The market-analyzer repository automates the `ui-builder` to
> `dev` handoff through the Skill tool; this repository does not adopt that, because every
> handoff here is manual on purpose, and one automated seam would be the exception that has to
> be explained.

That is a uniformity argument, and it is the only one offered. It does not claim the pause buys
anything at this particular seam — it claims that all four seams should look alike. They do not
do the same work:

- **`architect → dev` and the close review are gates.** The user's "go" is an approval, and the
  close review is worthless from inside the session that wrote the code. Fresh context *is* the
  mechanism at both.
- **`preset-author → architect` is a routing decision.** Whether a feedback note becomes an ADR
  is a judgement the content lane does not make.
- **`dev ↔ studio-builder` is transport.** The plan is already written, already interviewed,
  already approved by the user's "go" at the start of the session. Both ends are implementers
  working from the same document, and the receiver loads its own `SKILL.md`, its own rules and
  its own gate whether it was reached by a Skill call or by the user opening a new session. What
  the manual step moves across that seam is a copy-paste.

ADR-0177 also named the condition under which the precedent repo widened its own automation —
volume at one pairing — and that condition is already met here. **Plan 0159 crossed this seam
five times**: phases 1–2 `studio-builder`, 3 `dev`, 4 `studio-builder`, 5 `dev`, 6–8
`studio-builder`, 9 `dev`. Plan 0167 crosses it once, at phase 3 to 4, and that crossing is what
prompted this decision: the owner passed the plan across by hand on 2026-09-10 and said they did
not want to keep doing so.

ADR-0177's own mitigation for the cost was *"a plan that has to alternate should be split
instead"*, and the two plans written since show that is not what happens. Plan 0159 alternated
anyway, because the studio and the player it drives are developed against each other — a preview
that paints the wrong colours is fixed in `standalone/` and consumed in `studio/`, and splitting
that into two plans separates a defect from its own repair.

There is one real thing the manual step buys, and it is not the fresh-context gate: a Skill
invocation does not clear the conversation, so the receiving lane runs with the sending lane's
context still loaded.

## Decision

**The `dev ↔ studio-builder` boundary is automatic, in both directions.** When an implementing
lane reaches a phase owned by the other implementing lane, it commits its work, verifies
`git status` is clean, announces the handoff in one line, and invokes the sibling directly —
`Skill(skill="studio-builder", args="<handoff>")` or `Skill(skill="dev", args="<handoff>")`. The
sending lane's part of the session ends when that call returns; it never loops back to pick up a
later phase it owns.

**The receiving lane still restates and still waits for an explicit "go".** It re-reads the plan,
states which contiguous run of phases it owns, where it will stop and who owns the next phase,
and writes nothing until the user says go. **Automation removes the copy-paste, not the
approval.**

**The payload is three lines and a pointer** — the plan number and title, the phase number the
receiver picks up at, and the commits that just landed with the tree confirmed clean. It is
deliberately not a self-contained restatement of the plan: the plan is in the repository, and
[ADR-0120](0120-the-close-brief-is-a-section-of-the-plan.md) already requires the sending lane to
have written its rows of the `## Implementation log` as the phases landed. A payload that
duplicated those is a second copy of a document the receiver is about to open, and the second
copy is the one that drifts.

**Every other seam stays manual, and the reason is now stated per seam rather than as a rule
about all of them:** `architect → dev` and `architect → studio-builder` (the "go" is an
approval), any implementer `→ architect` (the close review needs the fresh session — never
invoke `/architect` through the Skill tool), `preset-author → architect` (routing judgement),
and any `→ human` phase (there is no skill to invoke; surface it and stop).

**The user's override is unchanged and still outranks this.** A user who says "go, and do the
studio phases too" gets exactly that, echoed back in one sentence so the decision is on the
record.

## Consequences

### Positive

- **A mixed-owner plan runs to its next `human` phase without the owner in the loop.** That is
  the whole ask, and on Plan 0159's shape it is five interruptions removed from one plan.
- **The cost that made ADR-0177 prescribe splitting plans mostly goes away**, which matters
  because the two plans written since both declined to split — the studio and the player are
  developed against each other, and the alternation is a symptom of that, not of bad planning.
- **The seams now differ because they do different work.** "All handoffs are manual" was easy to
  state and, as ADR-0177 shows, easy to reason from incorrectly: it lent the fresh-context
  argument to a seam that has no fresh-context requirement. Each boundary now carries its own
  reason, which is falsifiable one at a time.
- **The approval gate survives the change**, so the failure mode automation could have
  introduced — a mis-scoped studio phase writing files unasked — is still fenced by the same
  restate-and-wait the manual handoff had.

### Negative

- **The receiving lane inherits the sender's context.** A `Skill` call loads the receiver's
  `SKILL.md` into the running conversation; it does not clear what is already there, so the
  `studio-builder` half of Plan 0167 runs with Phases 1–3's Rust diff still in the window. Two
  effects, and the first is the one to watch: the receiver may reason from a fact it read in
  `standalone/` rather than from the protocol spec, which is precisely the self-review ADR-0177
  built the lane to prevent. It is bounded — the receiver still cannot *edit* Rust, and a
  protocol widening is still an `architect` note — but it is a real narrowing of the boundary,
  traded for the interruption. **The escape hatch is unchanged and costs one sentence:** the user
  can decline the auto-handoff and open a fresh lane, and should when the crossing is a protocol
  question rather than a phase.
- **The context window is longer at the far end of a mixed plan**, so a long alternating plan is
  more likely to be summarized mid-flight than the same plan run as two sessions.
- **A fifth document can now drift.** The rule is stated in `CLAUDE.md` and in three skills; there
  is no gate behind that, exactly as ADR-0108's rollout note warns in the precedent repository. It
  is the standing cost of a process contract that implementing lanes must read without opening
  `docs/`.

### Neutral

- No code, no schema, no protocol change. The owner-skill vocabulary is untouched — `dev`,
  `studio-builder`, `human` — and so is every rule about what each lane may edit.
- The precedent repository's structured `cross-skill-handoff.md` template is deliberately **not**
  adopted; see Alternative C.

## Alternatives considered

### Alternative A — Keep every handoff manual (ADR-0177 as written)
The status quo. Rejected because its argument is uniformity, and the seams are not uniform: it
borrows the fresh-context justification from the `architect` boundaries and spends it at a
boundary where the plan is already approved and both parties are implementers. The prediction it
made — that alternating plans would be split — was falsified by the next two plans.

### Alternative B — Automate `dev → studio-builder` only
Exactly what was asked for on 2026-09-10, and no more. Rejected on the evidence: Plan 0159
crossed this seam five times and **three of those crossings were `studio-builder → dev`**. Half
an automation leaves an alternating plan stopping dead just as often, and it would need the
asymmetry explained in four documents.

### Alternative C — Adopt the precedent repository's full structured payload
A `cross-skill-handoff.md` template with completed commits, a remaining-phase table, and the next
phase's spec pre-filled, recognised by a literal `# Cross-skill plan handoff` heading. Rejected as
solving a problem this repository does not have: that template makes the payload self-contained
because it must survive a copy-paste into a session that may not re-read the plan. Here the plan
is one file, the receiver opens it as its first act, and ADR-0120 already puts the phase-to-commit
mapping inside it. Pre-filling the phase spec into the payload creates a second copy of the
contract, and the copy would be written by the lane that does not own the next phase.

### Alternative D — Automate the boundary *and* drop the receiver's "go"
One approval per plan rather than one per lane. Rejected: the "go" is where a lane says which
phases it believes it owns and where it will stop, and the two lanes read the same plan
differently often enough that the restatement is worth its half-minute — `studio-builder` picking
up an extra phase unasked is a scope error nothing else in the ceremony would catch until the
close review.

### Alternative E — Hand the sibling phases to a subagent instead of a Skill call
`Agent(subagent_type=…)` gives the receiver genuinely fresh context, which is the one thing this
decision gives up. Rejected: the receiving lane commits, runs gates, and must be able to stop and
ask the user a question — a background agent can do none of that visibly, and the user would lose
sight of a lane that is writing to their working tree.

## Notes

- This ADR **amends** ADR-0177's *"Handoffs stay manual"* paragraph and the negative consequence
  *"Mixed-owner plans cost a session boundary per handoff"*. ADR-0177 is append-only and is not
  edited; the rest of it — the lane's boundary, what it owns, what it may never touch — stands
  unchanged and is not reopened here.
- The shape is lifted from the precedent repository's ADR-0108 and its `dev ↔ ui-builder`
  automation, with the payload replaced (Alternative C) and the scoping argument re-derived from
  this repository's own two plans rather than inherited.
