# ADR-0210 — A `.claude/` repair is the owner's, and a session that needs one parks with the edit

> **Status:** accepted 2026-09-16
> **Date:** 2026-09-16
> **Related plan(s):** [0190](../plans/done/0190-the-conductor-survives-a-run-nobody-is-watching.md) (Phase 8 stop gate)
> **Supersedes in part:** [0209](0209-a-conductor-close-repairs-the-prose-and-comments-its-findings-name.md)
> (the clause granting a close every file under `.claude/skills/`)

## Context

[ADR-0209](0209-a-conductor-close-repairs-the-prose-and-comments-its-findings-name.md) gave a
conductor-run close a closed list of what it may repair, and the third item on that list reads
*"Markdown prose anywhere in the repository, including every file under `.claude/skills/`"*. That
clause was written to answer backlog 0225, which had recorded a close declining a repair *"because
this session may not edit `.claude/`"* against no rule that said so. The reading at the time was that
the restriction, if any, was one of **ownership** — a `dev`-lane plan's close editing another lane's
skill — and ADR-0209 settled ownership with its fact-versus-rule line.

The reading was wrong, and Plan 0190 Phase 7 established that by asking the CLI rather than inferring
again. On **2.1.273**, two headless `claude -p` sessions ran in one probe worktree against
`.claude/skills/probe-scratch/NOTES.md`, differing only in the settings in force: session C under
`tools/conductor/settings.conductor.json` exactly as the conductor passes it, session D under a
generated file additionally naming `Edit(.claude/**)`, `Write(.claude/**)`, the `//`-rooted spellings
and the same two written with the worktree's absolute path. Both used `--permission-mode dontAsk`.
`Read` of that file was allowed in both. `Edit` of it and `Write` of a sibling were **denied in
both**, with *"Permission to use Edit has been denied because Claude Code is running in don't ask
mode"*. A `Write` outside `.claude/`, in the same worktree and the same turn, was allowed in both.
Read back from the parent after both sessions exited, nothing under `.claude/` had changed. **No
spelling reached it**: this is the CLI protecting a project's configuration directory, not a gap in
the allowlist. `tools/conductor/spike/README.md` carries the table under *"May a headless session
edit `.claude/`? Observed on 2.1.273"*.

So the clause is unreachable as written, and it has already cost three times. 0182's close left
`.claude/skills/preset-author/references/render-loop.md:170` open. 0177 Phase 8 did the whole
phase's work and then parked `check_red` on its own done-when, which greps `.claude/skills/`. The
owner committed that repair by hand as `f0cf263`. In each case the conductor promised something the
tool below it refuses, and the failure arrived late — as an open finding nobody routes, or as a park
after the work.

What is left to decide is not whether to except the directory but **who holds the repair once the
close cannot make it**, and whether a *phase* that needs such an edit is the same case as a
*finding*. They are not: a finding is one line of prose a close hands on, and a phase is work the
conductor should never have started.

## Decision

**`.claude/` is excepted from ADR-0209's closed list, the owner repairs what falls there, and a
session that needs such an edit parks before it does the work rather than after.** Concretely:

- **A close does not attempt an edit under `.claude/`.** The third item of ADR-0209's closed list
  reads, from here on, *Markdown prose anywhere in the repository except under `.claude/`, and
  except a generated region, which is repaired by regenerating it.* The other two items — comment
  and doc-comment text, assertion and panic message text — are unchanged, as is everything else
  ADR-0209 decided: the fact-versus-rule line, `fixed_in` on each repaired finding, and the
  conductor's check of it.
- **Such a finding stays open and is the owner's.** It carries no `fixed_in`, and the digest's
  **Needs you** lists it like any other open finding. One addition: because the close read the file
  and the reviewer composed the fix, the finding's text **names the replacement**, so the owner
  applies a repair rather than re-derives one. That is the only route this decision gives it —
  there is no queue, no follow-up plan, and no side channel through the conductor.
- **A phase whose declared files include `.claude/` parks before the phase runs**, with the exact
  edit as its detail, instead of running the phase and parking `check_red` on a done-when it was
  never able to satisfy.
- **A lane's skill material is still corrected by whoever the plan makes responsible** — the
  fact-versus-rule line of ADR-0209 is untouched. What changes is that the correction is applied by
  the owner, in a session or an editor that is not a headless `claude -p` run.

The exception is stated against a CLI version, as every other reading in
`tools/conductor/spike/README.md` is. Re-running the probe on a later CLI is how a switch, if one
ever appears, gets noticed; nothing here assumes the refusal is permanent.

## Consequences

### Positive

- **The conductor stops promising what the CLI refuses.** No close writes a repair it cannot make,
  and no phase reaches `check_red` for a reason it could have known before starting.
- **The park moves to the front of the phase.** The detail is the edit itself, so the owner acts on a
  line rather than on a red gate whose cause is a permission denial buried in a transcript.
- **The reading is evidence, not inference.** Backlog 0230's premise — "find the CLI's switch, if one
  exists" — is answered for 2.1.273 by a recorded probe, and the record names the settings it was
  observed under so a later reader can tell this apart from a configuration mistake.

### Negative

- **A prose error under `.claude/` now waits for a person**, which is precisely the accumulation
  ADR-0209 exists to stop. It is bounded only by how rare the class is: across the four pilot runs it
  was one open finding and one parked phase. If it stops being rare, this decision is what to revisit.
- **A plan that touches `.claude/` cannot run under the conductor at all**, and nothing about that is
  fixed here. Plan 0190 is itself such a plan — its own Phase 1 and Phase 9 edit three conductor-mode
  sections — and it is deliverable only because its header already makes it human-started.
- **The repair route rests on the owner's own commit, not on a measured permission.** The probe
  covers a headless session under `--permission-mode dontAsk`; whether an attended session is
  permitted was not probed. What is known is that `f0cf263` landed.
- **The exception is a version-scoped fact with no scheduled re-check.** A CLI that opened the
  directory would leave this ADR over-restrictive and silent about it.

## Alternatives considered

### Alternative A — Keep the clause and give the close a side channel
The close writes the intended edit to a file, and either the owner applies it or the conductor's
parent process does. Rejected in both halves. The parent applying an edit a denied session composed
routes around the exact boundary the CLI is enforcing, on a file that governs how every later session
behaves. And a patch file the owner applies by hand is a longer path to the same act as reading one
finding line in the digest, for a class seen twice.

### Alternative B — Route it to a backlog entry, as code-shaped minors are routed
Rejected: a one-line prose repair does not survive the trip. ADR-0209's own Alternative A rejected a
follow-up plan for this class on the grounds that the repair would land weeks after the reader was
misled; a backlog entry is cheaper than a plan and has the same defect, with a second place to lose
the finding. The digest already reaches the owner every morning, which is sooner than either.

### Alternative C — Move the skill sources out of `.claude/`
Keep the skills somewhere writable and generate or link `.claude/skills/` from it. Rejected: the CLI
reads `.claude/`, so whatever sits there is what a session actually loads, and the writable copy
would be a second source that drifts from it — with no gate able to tell which one a session obeyed.
It also bends the repository's layout around one tool's permission model, for a class of finding
measured in single figures.

## Notes

The probe, its two sessions and the full decision table are in
`tools/conductor/spike/README.md`, section *"May a headless session edit `.claude/`? Observed on
2.1.273"*. One trap recorded beside it is worth repeating: give a session `.claude/` paths
**absolute**. In an earlier run the path was relative and the model expanded it to the user's own
`~/.claude/`, so the tool answered *"File does not exist"* — a model choosing a path, not the CLI
resolving one, and a reading that would have looked like a third kind of refusal.
