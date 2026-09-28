# 0001 — The routing lands

> **Status:** approved

A seeded plan for `check-claude-declarations.mjs`. Phase 9 is Plan 0190 Phase 9 as it was drafted;
Phase 3 is a bare escape; Phase 10 declares only the bare directory. Every other phase is a shape the gate must stay silent on. See
`scripts/fixtures/README.md`.

## Implementation phases

### Phase 1 — a quotation is a mention
- **Owner skill:** dev
- **What:** A gate that refuses the shape Plan 0190 drafted, a body saying *"the three
  conductor-mode sections"* with no path.
- **Files touched:** `scripts/check-something.mjs`.
- **Done when:** it fails on that shape.

### Phase 2 — a class is named with an indefinite article
- **Owner skill:** dev
- **What:** Refuse a phase whose body names a skill, a hook, a `settings.json` or a conductor-mode
  section while its files carry no path.
- **Files touched:** `scripts/check-something.mjs`.
- **Done when:** it refuses one.

### Phase 3 — a bare escape
- **Owner skill:** dev
- **What:** Reword the dev skill's handoff paragraph. <!-- claude-allow: -->
- **Files touched:** `docs/something.md`.
- **Done when:** it reads well.

### Phase 4 — a reasoned escape
- **Owner skill:** dev
- **What:** Cite the dev skill's close block from the operator guide, without editing it.
  <!-- claude-allow: the skill is cited, not edited -->
- **Files touched:** `docs/something.md`.
- **Done when:** the citation resolves.

### Phase 5 — the other hook and the other settings file
- **Owner skill:** studio-builder
- **What:** The pre-push hook runs the new gate, and the studio's `settings.json` gains a key.
- **Files touched:** `.githooks/pre-push`, `studio/electron/settings.ts`.
- **Done when:** both land.

### Phase 6 — a declared phase
- **Owner skill:** dev
- **What:** The conductor-mode section gains the `merge` kind.
- **Files touched:** `.claude/skills/dev/SKILL.md`.
- **Done when:** the section names it.

### Phase 7 — the lane and the owner tag
- **Owner skill:** dev
- **What:** Work that belongs to the skill lane named in the owner tag above.
- **Files touched:** `core/src/lib.rs`.
- **Done when:** it builds.

### Phase 8 — Stop gate: what the probe found
- **Owner skill:** human
- **What:** The owner reads Phase 7's row and decides.
- **Done when:** the Notes record the branch taken.

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
  conductor-mode sections.
- **Done when:**
  - A fake session that needs a `.claude/` edit reaches the outcome Phase 8 chose.

### Phase 10 — a bare directory is not a declaration
- **Owner skill:** dev
- **What:** The dev skill's close block names the new outcome kind.
- **Files touched:** `.claude/`.
- **Done when:** the block names it.

## Implementation log
