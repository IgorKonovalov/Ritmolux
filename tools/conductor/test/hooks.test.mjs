// Bite checks for the two PreToolUse hooks the conductor depends on. Each case runs the hook the
// way the harness does — a child process fed the tool-call JSON on stdin — and reads its decision.
// A hook that stopped denying shows up as a named case going red, and so does one that started
// denying a near miss.

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const REPO = resolve(dirname(fileURLToPath(import.meta.url)), "..", "..", "..");
const PUSH_HOOK = join(REPO, ".claude", "hooks", "block-push-and-history-rewrite.js");
const SUITE_HOOK = join(REPO, ".claude", "hooks", "conductor-suite-lock.js");

function decision(hook, command, env = {}) {
  const base = { ...process.env };
  delete base.RLX_CONDUCTOR;
  const r = spawnSync(process.execPath, [hook], {
    input: JSON.stringify({ tool_name: "Bash", tool_input: { command } }),
    env: { ...base, ...env },
    encoding: "utf8",
  });
  assert.equal(r.status, 0, `hook exited ${r.status}: ${r.stderr}`);
  const out = JSON.parse(r.stdout);
  return out.hookSpecificOutput?.permissionDecision ?? "allow";
}

const PUSH_DENIED = [
  "git push",
  "git push origin main --follow-tags",
  "cargo fmt --all --check && git push",
  'git -C "C:/Users/Some One/WORK/Ritmolux" push origin v0.1.0',
  "git.exe push",
  "& git push",
  'bash -c "git push origin main"',
  "powershell -NoProfile -Command 'git push'",
  "git reset --hard",
  "git reset --hard HEAD~1",
  "git rebase main",
  "git rebase -i HEAD~3",
  "git commit --amend --no-edit",
  'git commit --amend -m "fix"',
  "git filter-branch --tree-filter 'rm x' HEAD",
  "git status\ngit push",
];

const PUSH_ALLOWED = [
  "git log --oneline origin/main",
  "git stash push -m wip",
  "git reset --soft HEAD~1",
  "git reset HEAD core/src/lib.rs",
  'git commit -m "docs: the owner runs git push; nobody runs git rebase"',
  "git commit -m @'\ndocs(plans): note\n\ngit push is the owner's\ngit commit --amend is never used\n'@",
  "git commit -F msg.txt <<'EOF'\ngit push\nEOF",
  'git log --grep "rebase" --oneline',
  "echo git push",
  "git worktree remove ../rlx-plan-0175",
  "git fetch origin && git status 2>&1",
  "git tag -a -f v0.1.0 -m 'chore: Release v0.1.0'",
];

for (const command of PUSH_DENIED) {
  test(`push hook denies: ${JSON.stringify(command)}`, () => {
    assert.equal(decision(PUSH_HOOK, command), "deny");
  });
}

for (const command of PUSH_ALLOWED) {
  test(`push hook allows the near miss: ${JSON.stringify(command)}`, () => {
    assert.equal(decision(PUSH_HOOK, command), "allow");
  });
}

const CONDUCTOR = { RLX_CONDUCTOR: "1" };

const SUITE_DENIED_UNDER_CONDUCTOR = [
  "cargo nextest run --workspace",
  "cargo nextest run --workspace -P fast",
  "cargo test -p rlx-core",
  "cargo +nightly test",
  "cd core && cargo nextest run",
  "cargo llvm-cov nextest --workspace",
  "node tools/conductor/with-lock.mjs close -- cargo nextest run --workspace",
  "cargo nextest run -p rlx-core",
  "cargo nextest list --workspace && cargo nextest run",
  "cargo nextest list -p rlx-core; cargo test -p rlx-core",
];

const SUITE_ALLOWED_UNDER_CONDUCTOR = [
  "node tools/conductor/with-lock.mjs suite -- cargo nextest run --workspace",
  'node "C:/Users/Some One/WORK/Ritmolux/tools/conductor/with-lock.mjs" suite -- cargo nextest run -P fast',
  "cargo clippy --workspace --all-targets -- -D warnings",
  "cargo fmt --all --check",
  "echo cargo test",
  'git commit -m "test(core): cargo nextest run is green"',
  "cargo nextest list -p rlx-core",
  "cargo nextest list --workspace",
  "node tools/conductor/with-lock.mjs suite -- cargo nextest list -p rlx-core",
];

for (const command of SUITE_DENIED_UNDER_CONDUCTOR) {
  test(`suite hook denies under RLX_CONDUCTOR=1: ${JSON.stringify(command)}`, () => {
    assert.equal(decision(SUITE_HOOK, command, CONDUCTOR), "deny");
  });
  test(`suite hook allows outside the conductor: ${JSON.stringify(command)}`, () => {
    assert.equal(decision(SUITE_HOOK, command), "allow");
  });
}

for (const command of SUITE_ALLOWED_UNDER_CONDUCTOR) {
  test(`suite hook allows under RLX_CONDUCTOR=1: ${JSON.stringify(command)}`, () => {
    assert.equal(decision(SUITE_HOOK, command, CONDUCTOR), "allow");
  });
}

test("the suite hook logs every call to RLX_HOOK_LOG under the conductor, whatever it decides, and nothing outside it", () => {
  const dir = mkdtempSync(join(tmpdir(), "rlx-hooklog-"));
  const log = join(dir, "0101-01-implement.log");
  assert.equal(decision(SUITE_HOOK, "git status", { ...CONDUCTOR, RLX_HOOK_LOG: log }), "allow");
  assert.equal(decision(SUITE_HOOK, "cargo test -p rlx-core", { ...CONDUCTOR, RLX_HOOK_LOG: log }), "deny");
  const lines = readFileSync(log, "utf8").trim().split("\n").map((l) => JSON.parse(l));
  assert.deepEqual(lines.map((l) => [l.hook, l.tool, l.decision]), [
    ["conductor-suite-lock", "Bash", "allow"],
    ["conductor-suite-lock", "Bash", "deny"],
  ]);

  const outside = join(dir, "outside.log");
  assert.equal(decision(SUITE_HOOK, "git status", { RLX_HOOK_LOG: outside }), "allow");
  assert.equal(existsSync(outside), false, "no RLX_CONDUCTOR, no log");
  // An unwritable log path never changes the decision.
  assert.equal(decision(SUITE_HOOK, "cargo test", { ...CONDUCTOR, RLX_HOOK_LOG: join(dir, "missing-dir", "x.log") }), "deny");
});
