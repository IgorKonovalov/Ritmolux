# ADR-0120 — The close brief is a section of the plan, written as the phases land

> **Status:** accepted 2026-08-25 (Plan 0112)
> **Date:** 2026-08-25
> **Related plan(s):** [0112](../plans/done/0112-the-handoff-stops-being-a-chat-message.md)

## Context

The `dev → architect` close handoff is one of three seams this project keeps manual on purpose,
because the value is the fresh-context boundary. Today it works like this: `dev` finishes the last
phase, fills in the template at
[`close-ceremony-prompt.md`](../../.claude/skills/dev/references/close-ceremony-prompt.md) and
prints it into the chat; the user carries that text by hand into a fresh `/architect` session,
which runs the Mode 4 review and the close-ceremony bookkeeping.

The *content* of that template is right. The **medium** is what fails, in three ways:

- **It is synchronous.** The brief exists only in one session's scrollback, so it must be acted on
  while that transcript is still at hand. A finished plan cannot sit and wait its turn.
- **It is single-copy, and a session clear destroys it.** The brief is written once, at the very
  end. Everything it would have said about phases 1 through N-1 lives only in a context window
  until that moment.
- **It does not cross the lane boundary.** Since
  [ADR-0053](0053-plan-lanes-run-in-git-worktrees.md) plan lanes run in git worktrees. A finished
  lane is a branch on disk that says nothing about itself — not which worktree it is in, not what
  landed, not what deviated. The only description of the work is in a chat window that belongs to
  a different session.

**The replacement has already been invented here, four times, without being written down.** Four
closed plans carry a hand-rolled `## Implementation log` section — 0100, 0108, 0110 and 0111. Plan
0110's opens:

> *"Written by `dev` at the end of the dev phases so the close ceremony and Phase 6 do not have to
> re-derive any of it. Everything below is what happened; the phases above are still the
> contract."*

and then records exactly what a handoff needs: the lane (`**Lane:** main directly, no worktree`), a
per-phase table of state and commit SHA, and what each phase found. `dev` reached for the plan file
unprompted, four times, because that is where the next reader already is. A convention that keeps
re-emerging is one worth specifying rather than one worth inventing.

There is a second cost the chat brief never addressed. **Half the close ceremony is conditional.**
Steps 3b (curate the preset set), 3c (archive discharged backlog entries) and 4 (bump the version)
fire on triggers — did the plan touch `presets/`, does the header name a `**Closes:**` entry, is
this a feature or a fix — that architect re-derives from the diff at every close. Those are
precisely the steps this project has measured itself skipping: the version sat at `0.2.0` across
five consecutive feature plans, and three separate sweeps of the backlog found **49** entries whose
`CLOSED` marker had been written but whose bodies were never archived, the third batch coming from
two closes that ran *after* the second sweep wrote the rule down. `dev` already knows every one of
those trigger facts at the moment it commits. It has never had a place to put them.

## Decision

We will make the close brief **a section of the plan document** — `## Implementation log`, the last
section, written by `dev`: **one row per phase as that phase's commit lands**, and a close block
after the final one. The plan template carries the empty skeleton, so every plan ships with the
affordance already visible.

The section records four things: the **lane** (branch and worktree path, or `main` directly), a
**phase / owner / state / commit** table, **notes** confined to deviations and judgement calls, and
the **facts behind each conditional close step** — `presets/` touched, the header's `Closes:`
entries, what the plan shipped, which operator docs moved, whether the backlog probes are green,
which `human` phases remain.

Three boundaries make this safe, and the first is the one the whole design turns on:

- **The log carries `dev`'s observations, never `dev`'s conclusions.** Architect must arrive with a
  complete map and no verdict pre-formed. A field that tells architect *where to look* is free of
  bias risk; a field that tells architect *how it went* is anchoring, and a file anchors far harder
  than a chat message does. So the log states the lane, the phase-to-commit mapping, and the raw
  `git`-derived trigger facts — and it does **not** state a per-criterion `[pass]` list, a
  self-assessment, or any narrative of how a phase felt. The `state` column says `done` or
  `not started`; it never says a quality word.

  Two consequences follow, and they pull in opposite directions on purpose. **Done-when results are
  reported by exception only** — a criterion `dev` could not satisfy as stated goes in the notes
  with what it did instead, and silence means `dev` believes the rest passed, which architect
  verifies exactly as it does today. That is asymmetric in the right direction: it surfaces
  problems and never pre-certifies a success. But **a deviation from the plan must be disclosed**,
  because a silent one is a review *failure* rather than a review *challenge* — Mode 4's lens 1
  asks whether any phase was missing or added without note, and a diff alone cannot answer that.
  What gets cut from a deviation note is the justification: it records **what `dev` did
  differently**, not **why that was fine**. Architect forms its own view of whether it was.
- **`dev`'s carve-out widens by exactly one section.** It may write `Status:` and this log, and
  nothing else in the plan. A phase block stays untouchable, and a plan that turns out wrong is
  still an escalation, never an edit.
- **The chat message survives as a pointer.** `dev` still ends with a short message naming the
  plan, the lane and the invocation — the user still has to start the fresh session, and that
  boundary is not what we are changing.

One section serves two readers, and the thin rule applies to only one of them. A **mid-session
handoff** — written when a session is cleared or compacted between phases — is read by a *resuming
`dev`*, which needs precisely what the rule forbids: the diagnosis behind an unfinished symptom,
the candidate fixes and their costs, the test state at the tip. Anchoring does not apply there,
because it is the same lane continuing. So that detail is **scaffolding**: written freely, and
removed when the phase it was written for lands. What survives to the close is the thin log
architect reads.

The log moves to `docs/plans/done/` with the plan and is never deleted.

## Consequences

**Positive.**

- The handoff becomes **asynchronous and durable**. A lane can finish and wait; the user closes it
  when convenient rather than when the transcript is still open. This is what makes parallel lanes
  practical rather than merely possible — the reason ADR-0053 exists is only paid off once a
  finished lane is self-describing.
- **A session cleared mid-plan loses nothing structural.** The log is on disk, committed, current
  as of the last phase, and a resuming `dev` session reads it to find where it is. That practice
  existed ad hoc; it now has one form instead of several.
- **Zero discovery cost.** Mode 4's first act is reading the plan. The brief is already there — no
  path convention to remember, no file to find, nothing to fetch across a worktree.
- **No lifecycle.** Nothing to fold, delete or re-link at close. It rides to `done/` with its plan,
  and `scripts/check-doc-links.mjs` sees no new class of file.
- **The conditional close steps get their triggers stated** by the lane that already knows them, at
  the moment it knows them.

**Negative.**

- **The plan document now mixes contract and report.** A reader skimming could mistake a log claim
  for agreed scope. Mitigated by placement (last section) and by the header sentence, but the
  ambiguity is real and it is the price of not having a second file.
- **`dev` now writes to the plan on every phase commit**, a wider carve-out than `Status:`. The
  prohibition becomes a boundary *inside* a file rather than a whole file, and boundaries inside a
  file are weaker.
- **A self-report can be mistaken for a verified record**, by a future session or by a person
  skimming a closed plan. The header sentence is the only defence.
- **Architect must not read silence as certification.** The exception-only rule buys a thin,
  unbiased log at the cost of an ambiguity: a criterion with no note means `dev` believes it
  passed, which is exactly the claim Mode 4 exists to test. Mode 4 has to say this out loud, or the
  thinness that was bought to protect the review gets read as a review already performed.
- **Nothing gates the log's presence.** A missing log is a Mode 4 `minor`, not a blocker, and the
  review still works from `git`. We are deliberately not building a gate yet: it would fire on
  architect at the close, after `dev`'s session is gone, which is the most expensive moment to
  learn about it. If logs go missing twice, that judgement gets revisited.
- **Log bloat is a measured hazard, and it is being held back by prose alone.** The four
  hand-rolled logs already in the tree run **86, 118, 219 and 305 lines** — 0100's is longer than
  that plan's entire phase section, which inverts the relationship the log is supposed to have to
  the contract. The rule adopted is a *relative* property rather than a constant, so it scales with
  the plan: **the log stays shorter than the plan's own `## Implementation phases` section.** It is
  stated in the template and checked by architect at Mode 4 as a `minor`, and it is deliberately
  **not** gated — a fourth doc gate was considered and declined as premature, since none of those
  four logs was written under this rule. That is the weaker kind of discipline, and this project
  has measured it failing before: ADR-0116 found *"One line per plan"* written three lines above
  the rows it governed, and the section regrew **7.1x in eight days**. **Revisit trigger: two
  logs breaking the property.** The instrument is already built — a `roster:begin cap=` region and
  `scripts/check-index-rows.mjs` — and unlike a presence gate, a size gate would fire on `dev` at
  push time, in the session that can still fix it cheaply.

## Alternatives considered

- **A separate committed file, `docs/plans/NNNN-close.md`.** Keeps the plan a clean contract, which
  is a genuine advantage. It lost because it needs a **lifecycle**: at close someone must fold it
  into the plan or `git rm` it, plus re-point anything naming it. That adds one more skippable step
  to a ceremony whose entire measured failure mode is skipped steps — and a step that, when
  skipped, leaves an orphan file sitting in `docs/plans/` looking like an active plan.

- **An untracked scratch file in the worktree** (`.lmv/close-NNNN.md`). Zero repo footprint. It
  lost decisively: it does not cross to `main`, leaves no audit trail, and `git worktree remove` —
  which ADR-0053 requires, because one lane's `target/` reached ~8 GB and filled the disk — deletes
  it. It fails at exactly the parallelization the change is for.

- **Keep the brief in chat, just shorten it.** The template's content was never the problem, and
  four independent ad-hoc reinventions of a file-based log are the evidence that a chat message
  does not suffice.

- **Write the log once, at the end, into the plan.** Smallest change; the same content, in the same
  place, with less churn. It lost on a failure this project has already hit: a session cleared
  mid-plan — and a filesystem crash mid-plan, during Plan 0110 — leaves nothing behind. Per-phase
  appends cost no extra commits, because the log edit rides inside the phase commit it describes.

- **Carry the full done-when list, every criterion prefixed `[pass]`/`[fail]`,** as the chat
  template does today. It was in the first draft of this ADR, lifted from that template without
  being questioned. It lost because it is the strongest anchoring vector the section could contain,
  and it sits at precisely the spot where Mode 4 already warns that a green `cargo test` is not a
  passing test: a column of `[pass]` beside criteria architect is supposed to independently check
  is an invitation to skim. Reporting non-passes only keeps every bit of the information architect
  cannot obtain elsewhere and discards the part that only pre-forms a verdict.

- **Let `dev` propose the version bump level.** Rejected. The level is architect-owned by
  [ADR-0005](0005-versioning-and-release-cadence.md), and a suggestion in writing is an anchor. The
  log states what shipped — feature, fix-only, docs-only — and stops there.
