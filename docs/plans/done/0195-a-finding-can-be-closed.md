# 0195 — A finding can be closed

> **Status:** done — both phases landed (`4737305`, `b38a4eb`); round 1 found two majors in the new
> command's input handling and a fix round repaired all four findings (`0bc634e`, `74b7e83`,
> `f444c30`, `6c882c1`). Round 2: **no blockers, no majors, two nits** (one repaired at the close in
> `42e6bda`). Verified in a fresh session: the full workspace suite green on the close tip, the verb
> parse and the closing-verdict test both repaired at the root, and every refusal asserted by its
> exact sentence with the state file byte-compared afterwards. Version **0.133.0**.
> **Created:** 2026-09-18
> **Approved:** 2026-09-18 (user)
> **Closed:** 2026-09-19
> **Owner skill(s):** dev
> **Related ADRs:** [0216](../../adrs/0216-a-review-finding-is-closed-by-the-owner-and-the-page-stops-carrying-it.md)
> (accepted), [0214](../../adrs/0214-the-digest-is-a-current-state-page-and-history-is-regenerated-on-demand.md),
> [0209](../../adrs/0209-a-conductor-close-repairs-the-prose-and-comments-its-findings-name.md)

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

## Close review

> Written by the fresh-session close review (ADR-0205 conductor mode), round 2, 2026-09-19. The full
> text of this round's review, then one line per finding an earlier round raised and a fix round
> resolved. The review path is `tools/conductor/state/reviews/0195-round-2.md`, which is gitignored —
> this section is the committed copy.

**Verdict: no blockers, no majors — the plan closes.** Both round-1 majors are repaired at the root
rather than patched: the verb is now the `FINDING_VERBS` entry that matched the flag in full, and the
closing verdict is the one the plan's own `closed` record names, which — with `validate`'s
`closed carries blockers or majors` refusal — makes a disposition against a blocker or a major
unreachable by construction rather than merely unlikely. The two round-1 lesser findings are repaired
as worded. Two nits remain, both about what is said rather than what runs.

- **Lane:** `C:\Users\Igor Konovalov\WORK\rlx-plan-0195` on branch `plan-0195-a-finding-can-be-closed`
- **Commits reviewed:** `4737305` (Phase 1), `b38a4eb` (Phase 2), `baa613d` (close block), and the
  round-1 fixes `0bc634e`, `74b7e83`, `f444c30`, `6c882c1`, `82fe78d`
- **Counts:** 0 blockers, 0 majors, 0 minors, 2 nits

### Evidence run in this session

| check | result |
|---|---|
| `node <with-lock> suite -- cargo nextest run --workspace` | **green** — `Summary [771.805s] 2010 tests run: 2010 passed (7 slow), 7 skipped`, `with-lock: "suite" waited 186.9s, held 773.1s`, exit 0 |
| `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` | green, exit 0 |
| `cargo fmt --all --check` | clean |
| `cargo clippy --workspace --all-targets -- -D warnings` | clean |
| `node --test "tools/conductor/test/*.test.mjs"` | 341 tests, 341 pass, 0 fail — matches the log's claim exactly |
| `node scripts/check-doc-links.mjs` | OK (492 tracked files) |
| `node scripts/check-index-rows.mjs` | OK (598 rows, 0 over cap, 0 misshaped) |
| `node scripts/check-backlog-claims.mjs` | OK — 98 reductions across 39 live entries, 3 unprobeable, 47 advisory rows; matches the log's figures |
| `node scripts/toc.mjs --check` | OK (7 blocks, 620 rows) |
| `node scripts/check-translations.mjs` | OK — advisory: `packaging/foobar/READ-ME-FIRST.ru.md` stamped `f2b0048b`, source at `d6e275e6db` |
| `node scripts/check-reader-prose.mjs`, `check-system-counts.mjs`, `check-comment-hygiene.mjs` | OK |
| `git status` | clean |

The suite ledger did **not** skip this run: the round-2 `pre-review` gate had recorded only a served
`-P fast` line for this tree (`e5b728d3`, 1709 passed, leaning on the round-1 review's `ee9a735e`
green per ADR-0211), and a served line carries `served: true` with a `cmd` that is not the suite
command, so `greenRecord` cannot read it back. The wrapper therefore ran all 2010 tests on this tip.
The log's `**Full suite:** owed to the conductor's pre-review gate` is correct for conductor mode.

### Lens 1 — alignment with the plan and ADR-0216

Both phases carry a single in-vocabulary `**Owner skill:** dev`. The log stays shorter than
`## Implementation phases`, names the lane, maps each phase to its commit, discloses the one addition
beyond the plan (the live-run refusal) and now records each round-1 repair against its commit — all as
ADR-0120 wants, and every claim in it that this session could check held.

**Phase 1's fifth done-when is now met.** *"Refuse a plan that has no closing verdict, and say so"* is
decided on the close and not on the array: `conductor.mjs:428` reads
`rec?.closed ? rec.verdicts?.at(-1) : null`, and both writers of `rec.closed` push the closing verdict
immediately before setting it (`lane.mjs:531-532` for a session's `closed` outcome,
`conductor.mjs:505-506` and `lane.mjs:405-406` for an adopted one), so `at(-1)` is the verdict the
close closed with in every path that sets the field. The refusal now says where the plan stands —
`planStanding` at `conductor.mjs:392-395` — and `cli.test.mjs:551-572` seeds the case the round-1
fixture stepped around: a plan parked two fix rounds in, *carrying* a round verdict with a blocker,
refused for both listing and recording, with the state file byte-compared to prove no refusal wrote
to it.

The round-1 severity worry is closed by a second, independent guard rather than by the fix alone:
`validate` in `lib/outcome.mjs:210` rejects a `closed` outcome whose verdict carries blockers or
majors, and `validateVerdict` allows `fixed_in` only on a `minor` or `nit`, so a closing verdict's
findings can only be the class ADR-0209 leaves open and ADR-0216 scopes dispositions to. Nothing
narrower is needed in `cmdFinding`.

**Phase 1's verb parse is repaired at the root.** `conductor.mjs:409` takes the verb from the entry
that matched the flag in full, so the declaration that reaches the usage strings by interpolation is
now the same declaration the parse uses — a fourth verb cannot half-arrive on either side.
`cli.test.mjs:589` adds the bare `done` spelling to the usage table beside `--nope`, which is the case
that produced `ne` in the record.

Every test the plan named exists and asserts the behavioural claim: the listing is compared
line-for-line before and after a disposition, the disposition is read back out of
`state/conductor.json`, each refusal's exact sentence is asserted, the overwrite is asserted to keep
the first in `dispositionHistory`, and `digest.test.mjs` asserts the finding lines disappear, the
summary count follows, the closed-count line appears verbatim, the render stays a pure function of
state, and a reopened finding comes back. All three Phase 2 done-whens are met, including the
ADR-0214 property: with nothing parked and every finding disposed of, the section's summary is
`Nothing: no park, no lane stopped at the worktree cap, no open finding.` — because the summary is
built from `counts`, which the closed-count line does not touch. That separation is the right one and
is what makes the promised property reachable without hiding the count.

### Lens 2 — layering and contracts

Nothing outside `tools/conductor/` is touched. No Rust, no C++, no C ABI, no OSC address, no audio or
render path: the real-time, source-agnostic and spec-0001/0003 questions do not arise, as the plan's
own risk section says (*"This plan changes no run behaviour"*) and the diff bears out.

The new record fields are additive and cannot invalidate anything: `validateVerdict`
(`lib/outcome.mjs:157-179`) checks a session-emitted outcome, never the stored record, and rejects no
unknown key, so a `disposition` or `dispositionHistory` written after a merge leaves later outcome
validation untouched. Only the owner writes one — no session prompt, gate, close or lane path calls
`disposeFinding` — which is ADR-0216's central rule, and the live-run refusal
(`conductor.mjs:420-423`) closes the one window where a running conductor's next `saveState` would
have overwritten it. Listing stays read-only and runs during a run, deliberately.

### Lens 3 — docs and bookkeeping

`tools/conductor/README.md` is the only operator doc this plan touches and it is swept: the command
table row, the two digest-page descriptions, and a new `## Closing a finding` section. The round-1
minor is repaired in `f444c30` with the replacement text as worded — *recording* is what a live run
refuses, and listing is read-only. Nothing in the canonical operator-doc table applies: no hotkey,
flag, config key, preset, scene, OSC address or CLI surface of the shipped binaries moved. `presets/`
untouched; the plan header names no `Closes:`.

This plan ships a **feature** — a new operator command and a digest change — so the close owes a
**minor** version bump plus the studio's two version copies, which is the level the immediately
preceding conductor-only plan took for the same reason (Plan 0193, `0.131.2 -> 0.132.0`).

### Lens 4 — correctness

No numeric assertion, no aspect, no DSP, no hot path, no grid. What this lens reaches is the command's
input handling, and it is where the round-1 majors lived: a disposition is unverifiable by
construction (ADR-0216's own third Negative), so the command's refusals are the entire guard on the
record. All of them now hold, and each is asserted by its exact sentence with the state file
byte-compared afterwards: an empty or whitespace reason, a bare verb, an unknown verb, a missing ref,
an index past the end, a `file:line` matching nothing, a `file:line` matching two findings (named with
both), a plan that never started, a plan parked mid-fix-round, and a plan that closed with no
findings. `findingRef` refuses a prefix and a nearest match by construction — `/^\d+$/` for an index
and string equality for a `file:line` — which is the right shape for an operation that leaves no trace
when it hits the wrong target.

The one guard nothing asserts is the live-run refusal (nit 1), and it is untested in exactly the way
its three siblings are.

### Lens 5 — design integrity

The helpers sit where the record they are about sits: `FINDING_VERBS`, `findingWhere`, `findingRef`
and `disposeFinding` are in `lib/state.mjs`, and both digest pages and the CLI now name a finding
through one function, so the three spellings of `file:line` that existed before this plan are one.
`closedCount` is a sibling of `openFindings` reading the same array with the same test inverted, and
the count it produces is deliberately kept out of `counts` so the summary line stays a count of work.
`digest --history` needed no new code at all: `closedFindings` already walks every verdict, and
`findingLine` gained the disposition suffix, so the record ADR-0216 leans on falls out of the page
that already existed rather than being assembled a second time.

### Findings

#### nit 1 — the live-run refusal is the only guard on the record that nothing asserts

**Where:** `tools/conductor/conductor.mjs:420-423`. **Left open** — a test's logic, which a close may
not repair (ADR-0209).

`if (verb && runningPid(p))` is the addition beyond the plan, disclosed in the log, and its reason is
the strongest in the command: a live run holds `state` in memory and its next `saveState` writes the
whole file, so a disposition recorded beside it is lost with no trace — the exact failure this plan
exists to end, arriving silently. Every other refusal in `cmdFinding` is asserted by its exact
sentence with the state file byte-compared afterwards; this one is asserted nowhere.

It is a `nit` and not a `minor` because it is not drift this plan introduced: `resume`
(`conductor.mjs:327`), `park` (`:359`) and `adopt-close` (`:481`) each carry the same refusal and none
of the three is tested either, so the gap is the house's and the decision is one decision for all
four. Testing it needs a live-pid fixture the suite does not yet have, which is why it is worth
deciding once rather than four times. The repair is one fixture that writes a pid file naming a live
process and asserts all four refusals — or a `--filed` disposition promoting it to a backlog entry
carrying that probe.

#### nit 2 — `## Closing a finding` omitted the refusal an operator meets first

**Where:** `tools/conductor/README.md:213-243`. **Repaired at the close in `42e6bda`.**

The section stated five rules — the three verbs, the required reason, the `<ref>` spellings,
re-dispositioning, and who may write one — and said nothing about the case round 1 made deliberate: a
plan with no close has no findings, so `finding NNNN` refuses it whether or not a verb was typed,
because a plan still in or parked at a fix round carries verdicts whose blockers are the conductor's
own work in flight. That refusal is the second-most-likely thing an operator will meet after a
successful call, and the page documenting the command was silent on it. A sixth bullet now says so.

### Advisories carried forward (not findings)

- `packaging/foobar/READ-ME-FIRST.ru.md` is stale against its English source (stamped `f2b0048b`,
  source at `d6e275e6db`), as it was at round 1. Unrelated to this plan; ADR-0185 says a row is a
  reading and not a repair. This is the second close running that it has been named — a third makes
  it the signal ADR-0185 described, which is an ADR-worthy call and not a close-time edit.
- The plan's own followup stands, and nit 1 above is its first customer: the findings standing on the
  page when this lands are the first triage, and the real test of whether three verbs are the right
  three.

### Earlier rounds, and the commit that resolved each finding

- **Round 1, major — a verb typed without `--` was accepted and silently truncated into the record**
  (`tools/conductor/conductor.mjs:402`): resolved in `0bc634e`.
- **Round 1, major — a plan with verdicts but no close was treated as closed, so a live blocker could
  be disposed of** (`tools/conductor/conductor.mjs:419`): resolved in `74b7e83`.
- **Round 1, minor — the README said the whole command is refused while a conductor is live; only
  recording is** (`tools/conductor/README.md:237`): resolved in `f444c30`.
- **Round 1, nit — a refusal that exits 1 printed its only sentence on stdout**
  (`tools/conductor/conductor.mjs:425`): resolved in `6c882c1`.

Round 1's review is at `tools/conductor/state/reviews/0195-round-1.md` (gitignored); its four
findings are quoted above in the form the record carries them.

## Followups (after this lands)

- The thirty-nine findings standing on the page when this lands are the first thing to triage with
  it, and that pass is the real test of whether the three verbs are the right three.
