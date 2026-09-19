# 0208 — The conductor's safety claims get their evidence

> **Status:** approved
> **Created:** 2026-09-19
> **Approved:** 2026-09-19 (user) — approved and deliberately NOT in `tools/conductor/queue.json`
> **Owner skill(s):** dev
> **Related ADRs:** [0233](../adrs/0233-a-session-allowlist-safety-claim-is-asserted-against-a-transcript.md)
> (proposed), [0208](../adrs/0208-a-patch-cli-update-runs-with-a-warning-and-every-session-proves-the-hooks-ran.md),
> [0210](../adrs/0210-a-claude-repair-is-the-owners-and-a-session-that-needs-one-parks-with-the-edit.md),
> [0071](../adrs/0071-a-numeric-test-contract-states-a-property-or-names-its-machine.md)
> **Closes:** design-backlog 0236, 0237, 0241

## TL;DR

`tools/conductor/README.md` promises that *"a path that leaves the lane is refused, whatever it is
for"*, the rules enforce that only for a path a session writes out, and the test asserting it is a
self-declared model of the CLI that the first unattended run falsified on a case it states. This plan
probes the real matcher, moves every deny case onto the recorded transcript, bounds the shell
expansions the rules miss, and makes an undeclared `.claude/` edit a drafting error instead of a late
park. The first visible behaviour is a committed table saying what the CLI actually did with
`rm -rf $HOME/.cargo`.

## Context & problem

**The unbounded rule.** [Plan 0190](done/0190-the-conductor-survives-a-run-nobody-is-watching.md)
Phase 2 allowed `Bash(rm *)` and `PowerShell(Remove-Item *)` and bounded them with deny rules for the
four ways a *written* path leaves the worktree: `..`, `~`, a leading `/`, a drive letter. A path the
shell *produces* matches none of them, so all three of these are allowed today (backlog 0237):

```
rm -rf $HOME/.cargo
rm -rf "$(git rev-parse --show-toplevel)/../rlx-plan-0180"
Remove-Item -Recurse $env:USERPROFILE\WORK
```

The blast radius of the first is the machine rather than the repository, which is the argument against
accepting the gap rather than for it.

**The floor under it is known to be wrong.** `tools/conductor/test/settings.test.mjs` decides every
case through a `decide()` it implements, and its header says so: *"It is a model of the CLI, not the
CLI."* It asserts that `cd studio && npm run typecheck` is denied. In step `0191-01-implement` on
2026-09-16, under this exact settings file on CLI 2.1.273, **26 shell calls carried a `cd` and 22
ran** — the four denials correlating with `/tmp` and with `sed` / `head` rather than with `cd`. What
the real rule is has not been established (backlog 0241). So the deny cases — the ones whose failure
costs a directory rather than a turn — are asserted against a model already falsified on a shape it
states outright.

**And the one park that exists to fail early can still fail late.**
[ADR-0210](../adrs/0210-a-claude-repair-is-the-owners-and-a-session-that-needs-one-parks-with-the-edit.md)
parks a plan in front of a phase whose files include a `.claude/` path, because the CLI refuses a
headless session an `Edit` there. `claudePaths` reads the phase's `**Files touched:**` bullet for a
literal path, so the guarantee is only as strong as a plan's prose — and the plan that built the
mechanism is its own counterexample: Plan 0190 Phase 9 edited three `.claude/skills/*/SKILL.md` files
and named them as *"the three conductor-mode sections"*, a true sentence with no path in it. Under the
conductor that phase would have run, hit the denial, and parked `check_red` on its own done-when
(backlog 0236).

All three are the same failure at different levels: a claim about what the harness refuses, asserted
against something other than the harness.

## Decision

Per [ADR-0233](../adrs/0233-a-session-allowlist-safety-claim-is-asserted-against-a-transcript.md),
**a claim that the allowlist refuses something is asserted against a recorded transcript of the real
CLI, and a claim that it permits something may stay a model** — because being wrong about an allow case
costs one visible turn and being wrong about a deny case is unbounded and silent. `spike/probe.mjs`
already runs sessions under `settings.conductor.json` and records what happened, so the first phase is
a probe and every later phase is decided from what it recorded. For the `.claude/` park we take
backlog 0236's first shape — a gate at drafting time rather than a wider parser — because a phase body
that describes a skill file without naming it is a drafting error, and no scan of that body can find a
path that is not written in it.

We rejected widening `decide()` to model whatever the matcher turns out to do (the next divergence
would be as silent as this one), allowlisting only scratch roots instead of the deletion verbs
(backlog 0231 records legitimate deletions outside `target/`), and accepting the gap.

## Architecture diagram

```mermaid
flowchart TB
    subgraph evidence["Phase 1 — evidence (new)"]
        SHAPES[shape roster: compound, expansion, escape] --> PROBE[spike probe session under settings.conductor.json]
        PROBE --> TRANSCRIPT[committed RAN/DENIED table + CLI version]
    end
    subgraph asserted["Phases 2-3 — what the claims rest on"]
        TRANSCRIPT --> DENY[settings.test.mjs deny cases]
        MODEL[the decide model] --> ALLOW[settings.test.mjs allow cases]
        TRANSCRIPT --> RULES[settings.conductor.json expansion bound]
    end
    subgraph drafting["Phase 5 — before a session exists"]
        PLAN[a plan's phase body] --> GATE[check-claude-declarations.mjs]
        GATE -->|names a skill or hook with no path| RED[drafting error]
    end
    RULES --> LANE[headless session in a lane]
    DENY -.asserts.-> RULES
```

## Implementation phases

### Phase 1 — the matcher gets a transcript
- **Owner skill:** dev
- **What:** A probe session attempts a fixed roster of shapes under `settings.conductor.json` and its
  RAN/DENIED verdict per shape is committed beside the CLI version it was taken on.
- **Files touched:** `tools/conductor/spike/` (a new probe script beside `probe.mjs`, and its recorded
  table in `tools/conductor/spike/README.md`).
- **Roster, at minimum:** a bare `cd`; `cd <lane> && <allowed verb>`; `cd /tmp && <allowed verb>`;
  `<allowed verb> | sed`; `rm -rf $HOME/.cargo`; `rm -rf "$(git rev-parse --show-toplevel)/.."`;
  `Remove-Item -Recurse $env:USERPROFILE\WORK`; and one of each of the four literal escapes the rules
  already deny, as the control that the file is in force at all.
- **Done when:** `tools/conductor/spike/README.md` carries a table naming each shape and what the CLI
  did with it, on a named CLI version, and the four literal-escape controls read DENIED — without which
  the run proves nothing. The table states in one line whether `cd <lane> && ...` ran, which is the
  question `settings.test.mjs` asserts the opposite of.

### Phase 2 — the deny cases move onto the transcript
- **Owner skill:** dev
- **What:** `settings.test.mjs` keeps `decide()` for allow cases and asserts every deny case the probe
  covers against the recorded outcome instead of the model; its header stops claiming what it cannot
  see.
- **Files touched:** `tools/conductor/test/settings.test.mjs`.
- **Done when:** a deny case whose recorded outcome is RAN is a **red** test rather than a green one —
  demonstrated by the compound `cd` cases, which the transcript from Phase 1 either confirms or
  contradicts. If it contradicts them, they are corrected to what was observed and the correction is
  what this phase ships. The allow cases still run through `decide()` and the header says which half
  rests on which.

### Phase 3 — the deletion bound reaches an expanded path
- **Owner skill:** dev
- **What:** Deny rules that bound a path the shell produces, in the shape Phase 1's transcript shows
  actually bites.
- **Files touched:** `tools/conductor/settings.conductor.json`,
  `tools/conductor/test/settings.test.mjs`.
- **Done when:** `rm -rf $HOME/.cargo` and `Remove-Item -Recurse $env:USERPROFILE\WORK` are refused
  **under the real CLI**, shown by re-running Phase 1's probe and recording the new verdict beside the
  old one; and a literal in-lane deletion a session legitimately makes — `rm -rf target/debug` — still
  runs, so the bound did not buy safety by refusing correct work. ADR-0233's Alternative D is the
  likely shape and **the transcript decides**: if denying the expansion syntax does not bite, the shape
  that does is what ships and the ADR gains an `Outcome` rather than being obeyed.

### Phase 4 — the prose stops promising more than the rules enforce
- **Owner skill:** dev
- **What:** `README.md`'s guarantee is rewritten to the bound Phase 3 actually enforces, and the
  prompt's unenforceable rule is resolved in whichever direction Phase 1 showed.
- **Files touched:** `tools/conductor/README.md`, `tools/conductor/prompts/implement.md`.
- **Done when:** the README states the bound in terms a reader can check against the rules, and cites
  the transcript for the part that is measured. And the prompt's *"Shell calls run one command per
  call ... No `cd`"* is either dropped — if Phase 1 showed `cd <lane> && ...` is safe, in which case it
  is noise the sessions correctly ignored 22 times — or kept with the reason it exists. Compound rates
  across the 15 recorded sessions run 12 % to 89 % and 0191's 81 % sits inside that spread, so "the
  rule changed behaviour" is not available as the reason.

### Phase 5 — an undeclared `.claude/` edit is a drafting error
- **Owner skill:** dev
- **What:** A gate that refuses a plan whose phase body names a skill, a hook, `settings.json` or a
  conductor-mode section while its `**Files touched:**` bullet carries no `.claude/` path.
- **Files touched:** a new `scripts/check-claude-declarations.mjs`, `scripts/gates.manifest.mjs`,
  `.githooks/pre-push`, `.github/workflows/ci.yml`, `scripts/fixtures/`.
- **Done when:** the gate fails on a seeded fixture in Plan 0190 Phase 9's exact shape — a phase body
  saying *"the three conductor-mode sections"* with no path in `Files touched` — and passes on the same
  phase once the paths are declared. It joins `gates.manifest.mjs` with the three checkout carriers, so
  `check-gate-carriers.mjs` holds all three to it; and it carries `--self-test`, as every gate in that
  roster does. `node scripts/check-claude-declarations.mjs` exits 0 over the plans in `docs/plans/` and
  `docs/plans/done/` as they stand, or the plans it convicts are named for the architect rather than
  edited by this phase.

## Risks & open questions

- **Phase 1 may show the matcher does something neither backlog entry guessed**, which would make
  Phase 3's shape and possibly ADR-0233's Alternative D wrong. That is the plan working: Phase 1 is
  allowed to supersede the ADR, and the ADR says so.
- **A probe session costs money and a turn budget**, and it is the cheapest thing here by a wide
  margin. If the probe cannot be made to attempt a denied shape without the session abandoning the
  step, record that as the finding — a shape the harness will not let a session even try is evidence
  about the harness.
- **Phase 3 can over-refuse.** A bound on `$` in a deletion command is crude, and a session that
  legitimately writes `rm -rf "$SCRATCH"` is refused. The done-when checks the one legitimate shape we
  know of; others surface as denied commands in a lane's log, which is the recoverable direction.
- **Phase 5's gate reads prose and will have false positives.** A phase that mentions `.claude/` in
  passing is not touching it. The fixture-driven done-when is the floor; the escape hatch is the one
  every gate here has, a named allow comment, and the gate is worth nothing if it is noisy enough to be
  bypassed.
- **The transcript is a measurement and goes stale on the next CLI.** ADR-0071's rule applies: the
  table names its version. Nothing gates re-running it, and ADR-0208's version check is the nearest
  trigger.
- **This plan edits the file the running conductor reads.** It is approved unqueued for that reason —
  see "What this plan does NOT do".

## What this plan does NOT do

- **It does not run under the conductor.** Phases 3 and 4 edit `settings.conductor.json` and the
  prompts, which are what a conductor session is started with, and Phase 5 edits the gate roster every
  carrier runs. This plan is approved and left **out of `queue.json`** deliberately; it is taken in an
  interactive session, or queued only after the current lanes drain.
- **It does not settle whether a session should be allowed `rm` at all.** The verb stays allowed and
  bounded, per Plan 0190 Phase 2's brief.
- **It does not touch `Monitor`, backgrounding, or the suite lock** — ADR-0205 and ADR-0207's hooks are
  a different surface and are not implicated by any of the three entries.
- **It does not make an undeclared `.claude/` phase runnable.** ADR-0210's park stands; Phase 5 moves
  the failure from a session to a drafting gate and buys nothing else.

## Implementation log

> Written by `dev` — one row per phase as that phase's commit lands, and the close block after the
> last one. **The phases above are the contract; everything here is what happened.**

**Lane:** _(to be filled by `dev`)_

| phase | owner | state | commit |
|---|---|---|---|
| 1 — the matcher gets a transcript | dev | not started | |
| 2 — the deny cases move onto the transcript | dev | not started | |
| 3 — the deletion bound reaches an expanded path | dev | not started | |
| 4 — the prose stops promising more than the rules enforce | dev | not started | |
| 5 — an undeclared `.claude/` edit is a drafting error | dev | not started | |

### Notes

### Close triggers

- **`presets/` touched:**
- **Plan header `Closes:`** design-backlog 0236, 0237, 0241
- **What shipped:**
- **Operator docs touched:**
- **Backlog probes (`node scripts/check-backlog-claims.mjs`):**
- **Full suite:**
- **Outstanding `human` phases:**

## Followups (after this lands)

- Re-running the Phase 1 probe on a CLI version bump has no carrier. ADR-0208 verifies the version
  table; nothing says the matcher transcript is owed a re-run.
