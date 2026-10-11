# 0260 — The studio tests leave /tmp as they found it

> **Status:** in-progress
> **Created:** 2026-10-11
> **Owner skill(s):** studio-builder
> **Related ADRs:** none
> **Closes:** design-backlog 0291

## TL;DR

Every `npm --prefix studio test` run leaves a `/tmp/<random id>/ssr/` directory of transformed
modules and never removes it. One phase traces which vitest or vite option writes it, and then moves
the cache inside `studio/` or removes it at teardown. Done when a test run adds no directory to
`/tmp`.

## Context & problem

Backlog 0291. On the Arch box, where `/tmp` is a tmpfs, about 1,200 of these directories (6 GB of
RAM) were deleted by hand on 2026-10-07, and 76 more (about 660 MB) had come back by 2026-10-11.
Seventy-six were still present when this plan was written. The pre-push hook runs the studio's tests
whenever `studio/node_modules` exists, so every push from a full clone adds one, and so does every
conductor session that runs the studio suite. `studio/vitest.config.ts` sets no `cacheDir`, and
`studio/package.json` pins `vitest` 5.0.1 and `vite` 8.3.0. The conductor fixed the same class of
leak in its own tests (`tools/conductor/temp-dir-leak.md`).

## Decision

Trace first, then pick the smaller fix that the trace allows. Prefer a config option that writes
the cache under `studio/node_modules/.vite`, where vite's own cache already lives and which `.gitignore`
already covers. Use a `globalTeardown` that removes the directory only if no option places it. No
ADR: there is no tradeoff a later reader would need, and the fix is a config line or a teardown.

## Implementation phases

### Phase 1 — Trace and stop the leak
- **Owner skill:** studio-builder
- **What:**
  - Find which option of vitest 5.0.1 or vite 8.3.0 creates `/tmp/<id>/ssr/`: read the installed
    packages under `studio/node_modules`, and confirm by running the suite once and listing the new
    directory.
  - Point that option inside `studio/node_modules/.vite`, or, if no option places it, add a
    `globalTeardown` that removes exactly the directory the run created.
  - A comment at the config line names the mechanism (what writes it and why the default leaks), per
    the comment rule, without narrating the plan.
- **Files touched:** `studio/vitest.config.ts`, and a `studio/test/global-teardown.ts` only if the
  teardown route is needed.
- **Done when:**
  - A `node` one-liner that counts `/tmp/*/ssr` directories prints the same number before and after
    `npm --prefix studio test`.
  - `npm --prefix studio test`, `npm --prefix studio run typecheck` and `npm --prefix studio run lint`
    pass.
  - The log names the option or code path that wrote the directory.

## Risks & open questions

- **The directory may be per-worker.** If each vitest worker writes its own, a teardown in the main
  process must find all of them. The config route avoids that, which is why it is preferred.
- **The 76 directories already in `/tmp`** are not this plan's. They go at the next reboot or by hand.

## What this plan does NOT do

- It does not touch the conductor's own temp handling.
- It does not change which tests the pre-push hook runs.

## Implementation log

**Lane:** `/home/igor/Work/rlx-plan-0260` on branch `plan-0260-the-studio-tests-leave-tmp-as-they-found-it`

| phase | owner | state | commit |
|---|---|---|---|
| 1 — Trace and stop the leak | studio-builder | done | committed with this row |

### Notes

- What wrote it: `Vitest._tmpDir = join(os.tmpdir(), nanoid())` (vitest 5.0.1,
  `dist/chunks/index.DzobfTyw.js`); the shared `ModuleFetcher` writes `<that>/<environment>/`
  (`ssr/` and `client/`). A single-project run uses the shared fetcher, and close removes only the
  project's own `TestProject.tmpDir`, which nothing writes. No option places it, so the teardown route
  was taken: `studio/test/global-teardown.ts` is a `globalSetup` whose returned teardown removes
  `_tmpDir` (an internal field) when it is a direct child of `os.tmpdir()`.
- `studio/test/global-teardown.ts` falls under no tsconfig `include` and outside the `lint` script's
  globs, so neither `typecheck` nor `lint` reads it; `vitest.config.ts` is in the same position.
- Done-when one-liner run as `node -e` over `fs.readdirSync('/tmp')` counting entries with an `ssr`
  subdirectory: 77 before the unfixed run, 78 after it; 78 before and after each of two fixed runs.
- Followup: each studio test run also leaves about 68 `rlx-*` directories in `/tmp`
  (`rlx-studio-` +28, `rlx-render-` +11, `rlx-service-` +10, `rlx-judging-` +9 and others, measured
  across one run on a box with other lanes idle-or-unknown), which are the tests' own fixtures.

### Close triggers

## Followups (after this lands)
