// The suite ledger's own rules: which argument vector it keys, which record a lookup trusts, and that
// a clean tree is the only tree it names.

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { writeFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";

import { appendRecord, appendSkip, cleanTree, greenRecord, isFullSuite, readLedger, SUITE_COMMAND, skipNotice, summaryLine, treeOf } from "../lib/ledger.mjs";
import { defaultGate } from "../lib/gate.mjs";
import { tmp } from "./helpers.mjs";

function repo() {
  const dir = tmp("rlx-ledger-repo-");
  const sh = (...a) => assert.equal(spawnSync("git", a, { cwd: dir, encoding: "utf8" }).status, 0, a.join(" "));
  sh("init", "-q", "-b", "main");
  sh("config", "user.email", "t@example.invalid");
  sh("config", "user.name", "T");
  sh("config", "commit.gpgsign", "false");
  writeFileSync(join(dir, "a.txt"), "a\n");
  sh("add", "a.txt");
  sh("commit", "-q", "-m", "init");
  return dir;
}

test("only the exact vector `cargo nextest run --workspace` is the full suite, and the gate marks exactly that step", () => {
  assert.equal(isFullSuite("cargo", ["nextest", "run", "--workspace"]), true);
  assert.equal(isFullSuite("cargo.exe", ["nextest", "run", "--workspace"]), true);
  assert.equal(isFullSuite("cargo", ["nextest", "run", "--workspace", "--no-fail-fast"]), false);
  assert.equal(isFullSuite("cargo", ["nextest", "run", "--workspace", "-P", "fast"]), false);
  assert.equal(isFullSuite("cargo", ["nextest", "run", "-p", "rlx-core"]), false);
  assert.equal(isFullSuite("cargo", ["test", "--workspace"]), false);
  const marked = defaultGate().filter((c) => c.ledger);
  assert.equal(marked.length, 1);
  assert.equal(marked[0].cmd.join(" "), SUITE_COMMAND);
  assert.equal(isFullSuite(marked[0].cmd[0], marked[0].cmd.slice(1)), true);
});

test("a lookup skips only on the newest record for that tree, and only when it is green", () => {
  const file = join(tmp(), "suite-ledger.jsonl");
  assert.equal(greenRecord(file, "t1"), null, "no ledger, no skip");
  appendRecord(file, { tree: "t1", exit: 0, summary: "3 tests run: 3 passed", by: "gate 0101-pre-review", ms: 10 });
  assert.equal(greenRecord(file, "t1").by, "gate 0101-pre-review");
  assert.equal(greenRecord(file, "t2"), null);
  assert.equal(greenRecord(file, null), null, "no tree (a dirty worktree) never skips");
  appendSkip(file, { green: greenRecord(file, "t1"), by: "gate 0101-post-close" });
  assert.equal(greenRecord(file, "t1").by, "gate 0101-pre-review", "a skip line is not a run");
  appendRecord(file, { tree: "t1", exit: 100, summary: "3 tests run: 2 passed, 1 failed", by: "0101-03-review", ms: 10 });
  assert.equal(greenRecord(file, "t1"), null, "a later red on the same tree is never skipped on");
  appendSkip(file, { green: { tree: "t1", by: "x", at: "y" }, by: "stray" });
  assert.equal(greenRecord(file, "t1"), null, "a skip after the red does not revive the green");
  writeFileSync(file, "not json\n", { flag: "a" });
  assert.equal(readLedger(file).length, 4, "a torn line is ignored");
  assert.match(skipNotice(readLedger(file)[0]), /^skipped cargo nextest run --workspace: tree t1 is green in the suite ledger, run by gate 0101-pre-review at \S+: 3 tests run: 3 passed$/);
});

test("a clean worktree names its tree and a dirty one names none", () => {
  const dir = repo();
  const tree = treeOf(dir);
  assert.match(tree, /^[0-9a-f]{40}$/);
  assert.equal(cleanTree(dir), tree);
  writeFileSync(join(dir, "untracked.txt"), "x\n");
  assert.equal(cleanTree(dir), null);
});

test("the summary is nextest's last Summary line", () => {
  assert.equal(summaryLine("     Summary [   1.0s] 1 test run: 1 passed\n     Summary [ 652.000s] 1940 tests run: 1940 passed, 6 skipped\n"), "1940 tests run: 1940 passed, 6 skipped");
  assert.equal(summaryLine("error: no tests"), null);
});
