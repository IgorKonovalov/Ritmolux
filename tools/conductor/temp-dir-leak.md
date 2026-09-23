# Brief: the conductor tests leak temp directories

> **Found:** 2026-09-23, while diagnosing a Windows laptop whose logon had grown from 3 s to 3–10 min.
> **Status:** fixed on 2026-09-23 in `8822bd48` (suggested fixes 1–3; 4 not taken). Kept for the
> Arch box: see "On Arch" below.

## What happened

`node --test "tools/conductor/test/*.test.mjs"` creates temp directories and never removes them.
The suite is a gate (`"conductor tests"` in the gate manifest), so every conductor step, every lane
worktree (`rlx-plan-*`, `rlx-gate-*`) and every manual run adds another batch.

State of `%LOCALAPPDATA%\Temp` on 2026-09-23:

- **52,666** top-level `rlx-*` directories, **~500,000 files, 2.9 GB**, all created since
  2026-09-14 17:16 (the first `rlx-lock-test-*`). Peak: ~140,000 files on 2026-09-19.
- Biggest groups: `rlx-conductor-test-*` 7,487 · `rlx-digest-*` 5,173 · `rlx-locks-*` 4,830 ·
  `rlx-lock-test-*` 3,791 · `rlx-cli-repo-*` / `-tool-*` / `-lanes-*` / `-locks-*` ~2,900 each ·
  `rlx-worktree-*` 2,518.
- Every fixture repo (`sh(["init"…])`) carries a full `.git` with 13 hook `.sample` files —
  78 files per `rlx-lane-repo-*`. That is where most of the file count comes from.

**Why it matters beyond disk space:** on Windows, the User Profile Service walks the user's Temp
folder at every logon (it tags every entry for Storage Reserve: `CreateFile` +
`SetStorageReservedIdInformation` per file, captured with a Procmon boot log — 18.8 M operations
in one logon). The logon went 2–5 s (until 09-14) → 22 s (09-15) → 117 s (09-18) → ~180 s (09-20 on)
→ ~10 min. The screen stays black until it finishes. Linux has the same leak in `/tmp`, just
without that symptom (and `/tmp` is usually tmpfs, cleared at reboot).

The directories were deleted by hand on 2026-09-23; the code was fixed the same day (`8822bd48`).

## Where

- `test/helpers.mjs:15` — `tmp(prefix)` is `mkdtempSync(join(tmpdir(), prefix))` and nothing
  removes it. 33 bare `tmp()` calls plus ~35 prefixed ones across 12 test files.
- Direct `mkdtempSync(join(tmpdir(), …))` that bypass the helper:
  `hooks.test.mjs:171` (`rlx-hooklog-`), `with-lock.test.mjs:30` (`rlx-lock-test-`),
  `with-lock.test.mjs:120` (`rlx-wrap-repo-`), `with-lock.test.mjs:320` (`rlx-wrap-sep-`).
- `rlx-worktree-*`: git worktrees added from fixture repos — removing the directory alone leaves a
  dangling worktree entry in the (also temporary) fixture repo, which is harmless once both go.
- Not a leak: `with-lock.mjs:134` `tmpdir()/rlx-conductor-locks` is the real, shared lock dir.

## Suggested fix

1. **One owner for temp dirs.** Route every temp directory through `helpers.mjs`
   (replace the four direct `mkdtempSync` calls) and register each one for removal:

   ```js
   import { mkdtempSync, rmSync } from "node:fs";
   const created = new Set();
   process.on("exit", () => {
     for (const d of created) {
       try { rmSync(d, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 }); } catch {}
     }
   });
   export function tmp(prefix = "rlx-conductor-test-") {
     const d = mkdtempSync(join(tmpdir(), prefix));
     created.add(d);
     return d;
   }
   ```

   `node --test` runs each file in its own process, so a per-process `exit` hook covers every file.
   `maxRetries` matters on Windows: a just-exited child (`git`, the fake CLI) can still hold a
   handle for a moment (`EBUSY`/`EPERM`). An env escape hatch (e.g. `RLX_KEEP_TMP=1`) keeps the
   dirs for debugging a failure.

2. **A backstop for killed runs.** The `exit` hook does not run when the runner is killed
   (timeout, Ctrl+C, a lane aborted by the conductor). Either nest everything under one root —
   `join(tmpdir(), "rlx-test", String(process.pid))` — and sweep roots whose PID is gone at the
   start of a run, or sweep `rlx-*` entries in `tmpdir()` older than a few hours. Exclude
   `rlx-conductor-locks`.

3. **A gate that holds it.** A test (or a check in the suite wrapper) that counts `rlx-*` entries
   in `tmpdir()` before and after the suite and fails if the count grew. Without it the next new
   test file reintroduces the leak.

4. **Optional, cheaper fixtures.** `git init --template=` (empty template dir) or
   `-c init.templateDir=` skips the 13 `.sample` hooks per fixture repo — 78 → ~20 files each.

## Done when

- A full `node --test "tools/conductor/test/*.test.mjs"` run leaves the `rlx-*` count in
  `tmpdir()` unchanged, on Windows and on Linux, including after a run killed mid-way and a
  following clean run.
- The lane worktrees (`rlx-plan-*`, `rlx-gate-*`) pick the fix up when they next rebase.

## On Arch

The fix is platform-neutral, so a checkout at or after `8822bd48` no longer leaks. What an earlier
run left in `/tmp` stays until a reboot or systemd-tmpfiles' 10-day age sweep. On a tmpfs `/tmp`
that is RAM, and a long conductor run can exhaust its size or inode cap (`ENOSPC` in git fixtures).
Check once, and clean without touching the live lock directory:

```sh
ls /tmp | grep -c '^rlx-'
find /tmp -maxdepth 1 -name 'rlx-*' ! -name rlx-conductor-locks -exec rm -rf {} +
```

After a full `node --test "tools/conductor/test/*.test.mjs"`, the only new entry is the empty
`/tmp/rlx-conductor-test`.
