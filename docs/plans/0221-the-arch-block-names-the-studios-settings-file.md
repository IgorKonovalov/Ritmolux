# 0221 — The Arch block names the studio's settings file

> **Status:** in-progress
> **Created:** 2026-09-22
> **Owner skill(s):** dev
> **Related ADRs:** [0205](../adrs/0205-an-approved-plan-runs-under-a-conductor-and-every-judgement-it-cannot-make-parks-the-plan.md),
> [0210](../adrs/0210-a-claude-repair-is-the-owners-and-a-session-that-needs-one-parks-with-the-edit.md),
> [0243](../adrs/0243-the-reference-boxs-hardware-adapter-is-its-discrete-gpu-and-a-reading-names-it.md) (proposed)
> **Serves:** [Plan 0219](0219-the-arch-box-builds-tests-and-runs-every-lane.md) Phase 5, the done-when
> "one real conductor run on this box".

## TL;DR

A fixture plan: the smallest plan the conductor can take from `start` to a closed outcome on the Arch
box, written so that Plan 0219 Phase 5 has a real run to record. Its one `dev` phase adds a paragraph
to `docs/developing.md`'s "A fresh Arch Linux checkout" section saying where the studio reads
`playerPath` on Linux. The content is small but true: 0219 Phase 4 found the gap and left it
unedited.

## Context & problem

0219 Phase 5 has one open done-when: a real conductor run on this box, from `start` to a parked or
closed outcome. The owner chose a docs-only fixture over a queued real plan. The run is there to
exercise the conductor on Linux, so the content should cost as little as possible. That rules out:

- **Rust.** A code change reopens clippy, the suite and a version bump for nothing the run needs.
- **`presets/`.** That brings in curation at the close.
- **`.claude/`.** A headless session cannot write there, so the conductor parks the phase before it
  starts (ADR-0210).
- **`studio/`.** A phase declaring a path under it makes the conductor run `npm --prefix studio ci`
  in the lane (ADR-0218). On Node 26 that install leaves Electron half-extracted, which is recorded in
  `docs/developing.md`'s Arch block. The fixture would then be testing that bug and not the conductor.
- **A `human` phase.** That parks the plan instead of running it to a close.

The gap it fills is real. 0219 Phase 4's log says `studio/README.md` "Finding the player" names the
settings directory for Windows and macOS only, and that on Linux it is `~/.config/ritmolux-studio`.
`studio/README.md` belongs to `studio-builder` and is under `studio/`, so the fixture does not edit
it. It puts the Linux location where a developer bringing up this box looks first: the Arch block in
`docs/developing.md`.

## Decision

One `dev` phase edits one reader document, and every done-when is a command's output. There is no
diagram, because nothing in it has a data flow. The close is docs/chore-only, so it takes **no
version bump**: the close outcome carries `null` for the version and the tag. That is also the one
close path the conductor has not yet run on this box.

We rejected queueing the smallest real approved plan, because every one of them touches Rust, a
`human` phase or `.claude/`. We rejected a fixture that edits only its own plan file, because `dev`
writes only a plan's `Status:` line and its `## Implementation log`. A phase whose whole content sits
there would be a phase with nothing to review.

## Implementation phases

### Phase 1 — The Arch block names the studio's settings file
- **Owner skill:** `dev`
- **What:** one short paragraph in `docs/developing.md`, inside `### A fresh Arch Linux checkout`,
  after the Electron half-install paragraph and before `**No linker override on Linux.**`. It says
  that the studio run from source reads `"playerPath"` from `settings.json` in Electron's per-user
  directory, which on Linux is `$XDG_CONFIG_HOME/ritmolux-studio/`, meaning
  `~/.config/ritmolux-studio/settings.json` when `XDG_CONFIG_HOME` is unset. That is where a developer
  points it at their own `target/release/ritmolux`. The directory is named after `studio/package.json`'s
  `"name"` and is lowercase, unlike the player's `~/.local/share/Ritmolux/`. Check the location against
  `settingsFile` in `studio/electron/settings.ts` and Electron's `userData` rule. Read those files but
  edit neither. Cite no Plan or ADR in the paragraph: it addresses a developer bringing up the box,
  not the record. (`docs/developing.md` is in the Contribute group that
  `scripts/check-reader-prose.mjs` leaves out, so no gate would fail on a bare citation.)
- **Files touched:** `docs/developing.md`
- **Done when:**
  - `awk '/^### A fresh Arch Linux checkout/,/^## Editing presets/' docs/developing.md | grep -c 'ritmolux-studio/settings.json'`
    prints `1`.
  - `git diff --name-only main...HEAD` lists `docs/developing.md` and this plan's file, and nothing
    else.
  - `node scripts/check-reader-prose.mjs`, `node scripts/check-doc-links.mjs` and
    `node scripts/toc.mjs --check` each exit 0. The conductor's gate runs all three anyway. They are
    named here because they are the gates a docs edit can break.

## Risks & open questions

- **The gate's cargo steps still run.** The conductor's gate is the whole roster on every stage,
  `cargo nextest run --workspace` included. The suite ledger may serve or skip it, depending on the
  paths the diff touches (ADR-0207, ADR-0211). A docs-only diff costs wall time here, not session
  money. The Arch figure for a full suite is 454 s.
- **0219 Phase 6 may also edit `docs/developing.md`**, but only if the sd-filter venv is cited there.
  The two edits land in different sections, and Phase 6 runs after this plan merges, so the merge is
  textual at worst.
- **If the run parks**, that is still a valid reading for 0219 Phase 5, whose done-when accepts
  "parked or closed". The park reason and its state directory go into 0219's log. It is not repaired
  here.

## What this plan does NOT do

- Edit `studio/README.md`. The same gap stays there, and it is `studio-builder`'s to fix, outside
  any conductor run.
- Change the conductor, its settings or its queue beyond listing this plan. 0219 Phase 5 owns
  recording the run.
- Move the version.

## Implementation log

**Lane:** `/home/igor/Work/rlx-plan-0221` on `plan-0221-the-arch-block-names-the-studios-settings-file`

| phase | owner | state | commit |
|---|---|---|---|
| 1 — The Arch block names the studio's settings file | dev | done | `fc8f45d2` |

### Notes

- The first done-when's `awk ... | grep -c` pipe was refused by the headless session's allowlist, so it
  was not run as written. `grep -n 'ritmolux-studio/settings.json' docs/developing.md` was run instead:
  one match in the whole file, at line 79, inside `### A fresh Arch Linux checkout` (lines 28-90).

### Close triggers

- **`presets/` touched:** no
- **Plan header `Closes:`** none
- **What shipped:** docs-chore-only
- **Operator docs touched:** `docs/developing.md`
- **Backlog probes (`node scripts/check-backlog-claims.mjs`):** exit 0, 43 reductions across 20 live
  entries, 4 unprobeable; the moved-path advisory rows name no path this plan touched
- **Full suite:** owed to the conductor's pre-review gate (ADR-0207)
- **Outstanding `human` phases:** none
