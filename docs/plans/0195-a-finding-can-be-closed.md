# 0195 — A finding can be closed

> **Status:** in-progress
> **Created:** 2026-09-18
> **Approved:** 2026-09-18 (user)
> **Owner skill(s):** dev
> **Related ADRs:** [0216](../adrs/0216-a-review-finding-is-closed-by-the-owner-and-the-page-stops-carrying-it.md)
> (proposed), [0214](../adrs/0214-the-digest-is-a-current-state-page-and-history-is-regenerated-on-demand.md),
> [0209](../adrs/0209-a-conductor-close-repairs-the-prose-and-comments-its-findings-name.md)

## TL;DR

The digest's **Needs you** carries every open review finding and nothing ever takes one off, so the
first page rendered under ADR-0214 was 45 lines of which 39 were findings from nine merges, the
oldest days old. This plan adds one command — `conductor.mjs finding` — that records a dated
disposition with a reason against a finding, and makes the page carry the open ones plus a single
line counting the closed.

## Context & problem

An open finding is unfinished work the owner owns: ADR-0209 lets a close repair only the class whose
repair cannot change what a program does, and everything else is left standing deliberately. Putting
those on the page was right. What is missing is the other end — the three ways a finding legitimately
stops being work: it gets repaired, it is judged not worth repairing, or it is promoted to a backlog
entry that carries its own probe. **The record can say none of them**, so the list only grows, and a
page that only grows is the one the owner stops reading — which is how the two real parks on
2026-09-18 ended up under thirty-nine lines that were not that day's business.

`openFindings()` in `tools/conductor/lib/digest.mjs` already selects them: the closing verdict's
`minor` and `nit` entries with no `fixed_in`. The gap is a disposition beside `fixed_in` and a
command that writes one.

## Decision

One command, three verbs, a required reason, recorded in `state/conductor.json` beside the finding
it is about — `--done`, `--wontfix`, `--filed`. The digest hides findings that carry a disposition
and prints one line counting them. Per ADR-0216 the owner is the only author: no session, close or
gate may write one, because a disposition is a judgement and — unlike `fixed_in`, which is checked
against the branch — nothing can verify it.

## Implementation phases

### Phase 1 — a finding carries a disposition, and one command writes it

- **Owner skill:** dev
- **What:** `conductor.mjs finding <plan> <ref> --done|--wontfix|--filed <reason>`.
- **Files touched:** `tools/conductor/conductor.mjs` (the command), `tools/conductor/lib/state.mjs`
  (if the shape needs a helper), `tools/conductor/test/cli.test.mjs`, `tools/conductor/README.md`.
- **How:**
  - **`<ref>` names the finding within the plan**, stably enough to survive a re-render: the index in
    the closing verdict's `findings`, and `file:line` accepted as an alternative spelling. A `<ref>`
    that matches no finding, or more than one, is refused naming what it saw — never guessed at.
  - The disposition is `{ verb, reason, at }` on the finding itself. **A reason is required**; the
    command refuses an empty or whitespace one, because "closed for being old" is the failure mode
    ADR-0216 exists to prevent and an optional field is how it arrives.
  - **Re-dispositioning is allowed and overwrites, with the previous one kept** in a short history on
    the finding: a `--wontfix` that someone later repairs should read as repaired, and the record of
    having first declined it is worth more than tidiness.
  - **`finding <plan>` with no verb lists that plan's findings** with index, severity, `file:line`,
    and the disposition if any — so `<ref>` never has to be guessed from the digest.
  - Refuse a plan that has no closing verdict, and say so.
- **Done when:**
  - `finding 0181 3 --wontfix "assertion message, no reader"` records verb, reason and date, and
    `finding 0181` then lists that finding as closed with its reason.
  - An empty reason is refused, a `<ref>` matching nothing is refused, and a `file:line` matching two
    findings is refused naming both.
  - A second disposition on the same finding overwrites it and the first survives in the finding's
    history.
  - `finding` on a plan with no verdict exits non-zero with a sentence saying why.

### Phase 2 — the page carries the open ones and counts the closed

- **Owner skill:** dev
- **What:** The digest's **Needs you** skips dispositioned findings; `--history` keeps everything.
- **Files touched:** `tools/conductor/lib/digest.mjs`, `tools/conductor/test/digest.test.mjs`,
  `tools/conductor/README.md`.
- **How:**
  - `openFindings()` gains the disposition test beside its existing `!f.fixed_in`.
  - **One line, after the open findings:** the count of closed ones and the command that lists them.
    Not per plan — a per-plan breakdown is the accumulation again, one indent further in.
  - **`digest --history` renders every finding with its disposition, verb, reason and date.** That is
    the record ADR-0216 leans on when it accepts a gitignored store, so it is not optional.
  - The **Needs you** summary line counts open findings only, so an empty worklist reads as empty.
- **Done when:**
  - A fixture where every finding of a merge is dispositioned renders no finding lines for it, and
    the closed-count line names the total.
  - The same fixture under `--history` renders each one with its verb, reason and date.
  - A fixture with no parks, no cap stops and no open findings renders a **Needs you** that says so
    in one line — the ADR-0214 property, re-asserted here because this plan is what finally makes it
    reachable.

## Risks & open questions

- **The store is gitignored** (ADR-0216's own Negative): losing `state/conductor.json` returns every
  closed finding. The finding text survives in each plan's committed `## Close review`; the judgement
  does not.
- **A bookkeeping command gets skipped**, and then the page is stale in the direction that teaches
  the reader to ignore it. The count line is the only pressure this plan applies.
- **`<ref>` by index is stable only while the verdict is**, which it is — a merged plan's verdict is
  never re-rendered. If that ever changes, `file:line` is the spelling that survives.
- **This plan changes no run behaviour.** No lane, gate, lock or session path is touched; a defect
  here misinforms and cannot corrupt a run.

## What this plan does NOT do

- **It does not let anything but a human close a finding.** Not a session, not a close, not a gate.
- **It does not change how findings are produced or what a close may repair** — ADR-0209's class and
  its `fixed_in` evidence are untouched.
- **It does not file the backlog entry `--filed` refers to.** The verb records that someone did;
  writing the entry, with the probe ADR-0108 wants, stays the owner's.
- **It does not verify a disposition.** Nothing can.

## Implementation log

> Written by `dev` — one row per phase as that phase's commit lands, and the close block after the
> last one. **The phases above are the contract; everything here is what happened.**
> **Observations, never conclusions:** this says where to look, architect decides how it went.
> No per-criterion pass list, no self-assessment, no narrative — but a deviation from the plan or
> an unmet done-when is always disclosed. Stays shorter than `## Implementation phases` above.

**Lane:** `C:\Users\Igor Konovalov\WORK\rlx-plan-0195` on branch `plan-0195-a-finding-can-be-closed`

| phase | owner | state | commit |
|---|---|---|---|
| 1 — a finding carries a disposition, and one command writes it | dev | done | `4737305` |
| 2 — the page carries the open ones and counts the closed | dev | done | `b38a4eb` |

### Notes

- **Added beyond the plan, Phase 1 (`4737305`): `finding` refuses to record a disposition while a
  conductor is running**, the way `resume`, `park` and `adopt-close` do. A live run holds `state` in
  memory and its next `saveState` writes the whole file, so a disposition recorded beside it is lost
  with no trace. Listing is read-only and is not refused.
- **`<ref>` by index is 0-based** — the array index in the closing verdict's `findings`, which is
  what `fixes[].resolved[].finding` already means in the record. `finding NNNN` prints it per row.
- **The history's own `Needs you` hides a disposed finding too**, because both pages share
  `openFindings`. Every finding with its verb, reason and date is in that run's **Closed** section,
  which is where `digest --history` keeps the record.
- `node --test "tools/conductor/test/*.test.mjs"`: 341 tests, 341 pass, 0 fail.
- **Round 1 finding 0 (major), the verb parse** — `0bc634e5`: the verb is the `FINDING_VERBS` entry
  that matched the flag in full, and a bare `done` is a usage refusal.
- **Round 1 finding 1 (major), the closing verdict** — `74b7e833`: `finding` refuses a plan with no
  `closed`, so a round's verdict on a plan still in fix rounds neither lists nor takes a
  disposition; the refusal says where the plan stands, and the test seeds a parked plan with a
  verdict.
- **Round 1 finding 2 (minor), the README bullet** — `f444c306`: only recording is refused mid-run.
- **Round 1 finding 3 (nit), the nothing-to-close refusal** — `6c882c1e`: it prints on `o.err`.

### Close triggers

- **`presets/` touched:** no.
- **Plan header `Closes:`** the header declares none.
- **What shipped:** a feature, inside `tools/conductor/` only — one new operator command and a digest
  change. No cargo crate, no shipped artifact and no C ABI surface was touched.
- **Operator docs touched:** `tools/conductor/README.md` — the command table, a new
  `## Closing a finding` section, and the two digest-page descriptions.
- **Backlog probes (`node scripts/check-backlog-claims.mjs`):** exit 0 — 98 stated reductions across
  39 live entries, 3 unprobeable, 47 advisory moved-path rows.
- **Full suite:** owed to the conductor's pre-review gate (ADR-0207).
- **Outstanding `human` phases:** none; both phases are `dev`.

## Followups (after this lands)

- The thirty-nine findings standing on the page when this lands are the first thing to triage with
  it, and that pass is the real test of whether the three verbs are the right three.
