# 0260 — The studio tests leave /tmp as they found it

> **Status:** done - closed 2026-10-11 by a conductor close, round 1. Phase 1 in `400860c5`. Review:
> no blockers, no majors, two minors (one fixed at the close in `6650680c`, filing backlog 0293;
> one open). Verified: the `/tmp/*/ssr` count unchanged across a test run, the studio's tests,
> typecheck and lint, the full workspace suite and rustdoc. Version 0.175.1.
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
| 1 — Trace and stop the leak | studio-builder | done | 400860c5 |

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

- `presets/` touched: none.
- `**Closes:**` design-backlog 0291.
- Shipped: fix-only, test configuration under `studio/` (`vitest.config.ts`, new
  `studio/test/global-teardown.ts`); no shipped artifact changes.
- Operator docs moved: none.
- `node scripts/check-backlog-claims.mjs`: exit 0, 46 reductions across 22 live entries, 3
  unprobeable; 0291 is not among the advisory entries.
- Full suite: owed to the conductor's pre-review gate (ADR-0207).
- `human` phases remaining: none.

## Close review

### Plan 0260 — close review, round 1

Graded at `5ff28bdc3881d01246ce05ece05703cbfbc3c286` in `/home/igor/Work/rlx-plan-0260`.

**Verdict: Plan 0260 landed cleanly; no blockers, no majors, two minors.**

#### Evidence

- **Full suite.** `node .../with-lock.mjs suite -- cargo nextest run --workspace` printed
  `with-lock: skipped cargo nextest run --workspace: tree 999f216 is green in the suite ledger, run by
  gate 0260-pre-review at 2026-10-11T13:23:01.550Z: 1995 tests run: 1995 passed (7 slow), 106 skipped`.
  That ledger record is the full-suite evidence (ADR-0207). The plan touches no Rust.
- **rustdoc.** `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`: clean.
- **Done-when 1 (the count).** Run as a `node -e` one-liner over `fs.readdirSync('/tmp')` that counts
  entries holding an `ssr` subdirectory: **78 before, 78 after** `npm --prefix studio test`.
- **Done-when 2.** `npm --prefix studio test`: 49 files, 443 tests passed. `npm --prefix studio run
  typecheck` and `npm --prefix studio run lint`: both clean.
- **Done-when 3.** The log names the code path: `Vitest._tmpDir = join(os.tmpdir(), nanoid())`. Checked
  against the installed vitest 5.0.1 (`studio/node_modules/vitest/dist/chunks/index.DzobfTyw.js`):
  line 20431 declares `_tmpDir` that way, line 20501 hands it to `createFetchModuleFunction`, whose
  writer (lines 6314-6316) `mkdirSync`s `<tmpDir>/<environment.name>`. Only the per-project `tmpDir`
  (line 12021) is ever `rm`ed (line 12288). Nothing removes `_tmpDir`. The claim holds, and so does the
  reason the config route was not open: `tmpdir()` is read in a class field initializer, before any
  config is resolved.
- The tree was left unchanged (`git status --porcelain` empty after the run).

#### Lens 1 — alignment

The single phase did what it said. It took the teardown route the Decision allows when no option
places the directory, and gave the reason. The owner tag is present and in vocabulary. The
implementation log is present and shorter than the phases section. Its close triggers name the
backlog entry and say the full suite is owed to the pre-review gate, which is correct in conductor mode.

`studio/test/global-teardown.ts` reads the internal field `_tmpDir` defensively. It removes the path
only when the value is a string whose parent is `os.tmpdir()`, so a renamed field or a moved directory
does nothing rather than deleting something else. That is the right failure mode for depending on an
internal. The comment carries the mechanism and no plan narration. It is registered as `globalSetup`,
because the teardown is the function a `globalSetup` returns. The plan named the filename, so the
mismatch between the name and the registration key is not a finding.

#### Lens 2 / 5 — layering, seams

There are none to cross: this is test configuration under `studio/`. It adds no runtime code, no
protocol change and no dependency.

#### Lens 3 — docs and bookkeeping

- The change is invisible to operators, so no operator doc moves. Nothing in `docs/developing.md`
  describes vitest's temp directory.
- `**Closes:** design-backlog 0291` → step 3c at the close: the entry should already be in the archive's
  `### Promoted` table. Add the CLOSED marker and move the row to `### Closed`.
- Version: the plan ships a test-harness fix and no shipped artifact. That points to **none**
  (docs/chore-only) or at most **patch**. The close decides.

#### Lens 4 — correctness

No numeric assertions were added. The teardown runs when vitest closes, which also happens after a red
run. A process killed by a signal still leaks one directory, which is acceptable and outside the
done-when.

#### Findings

##### minor — the new teardown module is neither type-checked nor linted
- **Where:** `studio/tsconfig.test.json:7` (and `studio/package.json:21`, the `lint` script).
- **What:** `studio/test/global-teardown.ts` matches no tsconfig `include` and none of eslint's globs.
  The log says so itself. `npm run typecheck` and `npm run lint` therefore pass without reading the
  plan's one new source file. The file uses a `TestProject` type and a cast, so a vitest upgrade that
  changes `vitest/node`'s types would not be caught until runtime.
- **Fix:** add `"test/**/*"` to `tsconfig.test.json`'s `include`, and `"test/**/*.ts"` to the `lint`
  script's globs. This is a config change, so it goes to a fix session or the backlog, not the close.

##### minor — the rlx-* fixture leak the log measured has no carrier
- **Where:** `docs/plans/0260-the-studio-tests-leave-tmp-as-they-found-it.md:85` (Notes) against the
  empty `## Followups (after this lands)` at line 101.
- **What:** the log records that each studio run also leaves about 68 `rlx-*` directories in `/tmp`.
  These are the tests' own fixtures (`rlx-studio-` +28, `rlx-render-` +11, `rlx-service-` +10,
  `rlx-judging-` +9). They are the same class of leak backlog 0291 described, and the only record is a
  Notes bullet in a plan about to move to `done/`.
- **Fix (Markdown, close-repairable):** state it under `## Followups (after this lands)`, and file a
  live `docs/design-backlog.md` entry with a probe or an `unprobeable:` reason, so the leak outlives
  the plan.

### At the close

- Finding 1 (teardown not type-checked or linted): **open** — a config change, outside what a close
  repairs (ADR-0209).
- Finding 2 (the `rlx-*` fixture leak): **fixed** in `6650680c` — a Followups bullet here and live
  backlog 0293 with two probes.
- No earlier round: round 1 was the only review.
- Backlog 0291 closed in the archive. Version 0.175.1 (patch). Translation and upstream-CI readings
  are in the close notes of the archive write-up.

## Followups (after this lands)

- The studio tests' own `rlx-*` fixture directories (about 68 a run, measured in the Notes above)
  stay in `/tmp`. Filed as backlog 0293 in
  [`design-backlog.md`](../../design-backlog.md).
