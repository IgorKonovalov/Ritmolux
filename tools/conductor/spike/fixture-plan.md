# 9999 — Probe fixture: a plan that exists only to be pointed at

> **Status:** approved (2026-09-14)
> **Created:** 2026-09-14
> **Owner skill(s):** `dev`, `human`
> **Related ADRs:** none
> **Closes:** none

## TL;DR

A disposable plan the Plan 0187 Phase 1 probe copies into a disposable worktree as
`docs/plans/9999-probe-fixture.md`, so a headless `/dev implement plan 9999` session has a real
plan-shaped file to find. Nothing here is ever implemented and the file is never committed there.

## Implementation phases

### Phase 1 — Write a probe file
- **Owner skill:** dev
- **What:** Create `probe-out.txt` in the worktree root.
- **Files touched:** `probe-out.txt`
- **Done when:** the file exists.

### Phase 2 — Look at it
- **Owner skill:** human
- **What:** A person opens the file.
- **Done when:** they have.

## Implementation log

**Lane:** _(probe worktree)_

| phase | owner | state | commit |
|---|---|---|---|
| 1 — Write a probe file | dev | not started | |
| 2 — Look at it | human | not started | |

### Notes

### Close triggers

## Followups (after this lands)
