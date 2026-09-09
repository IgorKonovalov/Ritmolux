# ADR-0177 — A fourth skill lane, `studio-builder`, builds the studio and never touches the engine

> **Status:** proposed
> **Date:** 2026-09-09
> **Related plan(s):** [0159 — The studio opens](../plans/0159-the-studio-opens.md)
> **Related:** [ADR-0017](0017-preset-author-skill-lane.md) (the precedent: a lane is added by ADR, not by widening `dev`),
> [ADR-0175](0175-the-studio-is-a-separate-application-that-never-draws-a-frame.md) (what the studio is),
> [ADR-0178](0178-the-studio-shell-conventions.md) (how the studio is built)

## Context

ADR-0175 makes the studio a separate Electron application under `studio/`, in TypeScript. The
project's implementing lane, `dev`, is defined as *"all code — Rust (core + standalone) and C++
(foobar plugin)"*, and every rule it carries is shaped by that: the audio callback, the hot-path
pragma, `cargo nextest -P fast`, the C ABI. None of it applies to a renderer process, a preload
bridge, or a Content Security Policy, and the rules that *do* apply there, which are the ones
that stop an Electron app from acquiring a CVE, are not written anywhere in this repository.

ADR-0017 faced the same question when presets became a third artifact type, and its answer is
the precedent: a lane is added when the artifact is genuinely different, with its own boundary
written down, rather than by folding it into `dev` and blurring the distinction. The value of
the harness is the clean-context boundary between lanes, and a lane that owns both the engine
and the editor that drives it would review its own protocol from both sides.

There is a worked example one directory over. The market-analyzer repository, which this
harness was adapted from, runs a `ui-builder` lane that owns its Electron shell end to end,
against an ADR that fixes the shell's conventions. That lane has shipped a large application
and has already found the rules worth keeping: the process-boundary discipline, the security
defaults, the disposal contract for non-React resources, the boundary validation, and the
hard rule about what does and does not go through Electron IPC.

## Decision

We will add a **fourth skill, `studio-builder`**, as a peer of `architect`, `dev` and
`preset-author`. It owns **`studio/` and nothing else**: the Electron main process, the preload
bridge, the renderer, the shared protocol types, the studio's tests, and its packaging
configuration. It never edits Rust or C++, and it never widens the protocol the player speaks:
a control action or an event the studio needs and the player lacks is a feedback note to
`architect`, exactly as an engine gap is for `preset-author`. It reads ADR-0175, ADR-0176,
ADR-0178 and `docs/specs/0003-studio-control-protocol.md` before touching a file, and on any
conflict the ADR wins over the skill's own text.

The **owner vocabulary for plan phases grows to `dev`, `studio-builder` and `human`.** Every
document that states the vocabulary changes in the same commit as the skill lands: `CLAUDE.md`,
the `architect` skill's Mode 1 and Mode 4 text, and the `dev` skill's owner-tag branch. A plan
with a phase tagged `studio-builder` is reviewed by `architect` at its close in the same
ceremony `dev`'s plans get, and the lane writes only the plan's `Status:` line and its own rows
of the `## Implementation log`, as `dev` does.

**Handoffs stay manual.** A plan whose phases alternate between `dev` and `studio-builder` is
implemented lane by lane: the lane stops at the first phase it does not own, commits, and its
final message names the next owner and the phase. The market-analyzer repository automates the
`ui-builder` to `dev` handoff through the Skill tool; this repository does not adopt that,
because every handoff here is manual on purpose, and one automated seam would be the exception
that has to be explained.

## Consequences

### Positive

- **The engine and the editor are reviewed from opposite sides of the protocol.** The lane that
  adds an event to the player is not the lane that consumes it, so a widening has to be argued
  for in an ADR rather than slipped in on both ends of one session.
- **The Electron rules have a home.** The security defaults, the boundary discipline and the
  disposal contract live in the skill and ADR-0178, where a session building a panel reads them,
  rather than in `dev`, where a session tuning the FFT never would.
- **The skill starts from a shipped example.** Its shape is `ui-builder`'s, and its rules are the
  ones that survived a large application, with the divergences named rather than rediscovered.

### Negative

- **A fourth lane is a fourth thing to keep true.** Its `references/` will rot the way
  `preset-author`'s private catalogue did before ADR-0017's rewrite pointed it at the docs. The
  mitigation is the same: the skill keeps no copy of the protocol or the schema, and points at
  the spec and at `ritmolux --schema`.
- **Mixed-owner plans cost a session boundary per handoff.** Plan 0159 is written so that the
  `dev` work lands first in Plan 0158 and the studio plan is `studio-builder` throughout, with
  `human` gates; a plan that has to alternate should be split instead.
- **The pre-push gate does not see the studio yet.** `.githooks/pre-push` runs cargo and the
  Node doc gates; the studio's typecheck, lint and tests join it only when `studio/node_modules`
  exists, so a clone that never built the studio is not slowed by it. Until Plan 0159 wires that,
  CI is the studio's only gate.

### Neutral

- `skill-creator` exists in this repository and is the tool for editing the skill's description
  and running its evals; the skill's *boundary* is fixed here and does not move without an ADR.

## Alternatives considered

### Alternative A — Widen `dev` to "all code, in any language"
One lane, no handoff. Rejected because the lane's rules are engine rules, and a session that
carries both the hot-path pragma and the Content Security Policy in one context has the wrong
half loaded for whichever file it is editing. It also makes the protocol self-reviewed.

### Alternative B — `preset-author` grows the studio, since the studio authors presets
The studio's *purpose* is content, but its *artifact* is an application, and `preset-author` is
defined by never writing code. Rejected on that boundary; the content lane will use the studio
and hand its friction to `architect`, which is unchanged.

### Alternative C — No lane; the user builds the studio by hand
The honest option if the studio were a weekend tool. Rejected because ADR-0175 makes it a shipped
artifact with a release job, and every shipped artifact here has an owning lane.

## Notes

The `ui-builder` skill the shape is lifted from lives at
`../trading/market-analyzer/.claude/skills/ui-builder/SKILL.md`, beside its ADR-0008. What was
not lifted is listed in ADR-0178 rather than here, because those are shell conventions, not lane
boundaries.
