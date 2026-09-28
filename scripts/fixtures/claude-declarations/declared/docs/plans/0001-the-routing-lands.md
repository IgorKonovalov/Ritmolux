# 0001 — The routing lands

> **Status:** approved

A seeded plan for `check-claude-declarations.mjs`: Plan 0190 Phase 9 with its three skill files
written into `Files touched`, which is the whole difference from `../undeclared/`. See
`scripts/fixtures/README.md`.

## Implementation phases

### Phase 9 — `.claude/` resolves the way Phase 8 chose
- **Owner skill:** dev
- **What:** Whichever Phase 8 settled.
  - **Configuration:** adopt the switch in `settings.conductor.json`, with a case in
    `settings.test.mjs`, and delete backlog 0230's premise from the README if it is stated there.
  - **Routing:** a phase or finding whose files include `.claude/` parks with the exact edit as its
    detail, *before* the rest of the phase runs rather than as a `check_red` after it. The
    conductor-mode sections and `prompts/*.md` say a session does not attempt such an edit.
- **Files touched:** decided by Phase 8; `tools/conductor/settings.conductor.json` and
  `test/settings.test.mjs`, or `lib/lane.mjs`, `lib/outcome.mjs`, `prompts/*.md` and the three
  conductor-mode sections: `.claude/skills/dev/SKILL.md`, `.claude/skills/architect/SKILL.md`,
  `.claude/skills/studio-builder/SKILL.md`.
- **Done when:**
  - A fake session that needs a `.claude/` edit reaches the outcome Phase 8 chose.

## Implementation log
