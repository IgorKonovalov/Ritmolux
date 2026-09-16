// The allowlist every conductor session runs under. A session must be able to put a file back with
// `git restore`, and must never reach `git checkout`, which can move the lane's branch, or
// `git stash`, whose stack every worktree shares.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";

import { TOOL_DIR } from "./helpers.mjs";

const settings = JSON.parse(readFileSync(join(TOOL_DIR, "settings.conductor.json"), "utf8"));
const allow = settings.permissions.allow;
const deny = settings.permissions.deny;

test("sessions may not arm a Monitor", () => {
  // Nothing re-invokes a headless session, so a Monitor armed on a backgrounded command never fires
  // and the session ends having lost that work.
  assert.ok(deny.includes("Monitor"));
  assert.ok(!allow.includes("Monitor"));
});

test("sessions may run git restore from either shell", () => {
  assert.ok(allow.includes("Bash(git restore *)"));
  assert.ok(allow.includes("PowerShell(git restore *)"));
});

test("no allow entry reaches git checkout or git stash", () => {
  const reaching = allow.filter((e) => /\bgit\s+(checkout|stash)\b/.test(e) || /^(Bash|PowerShell)$/.test(e) || /\((git|git \*)\)$/.test(e));
  assert.deepEqual(reaching, []);
});
