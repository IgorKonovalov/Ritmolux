// The conductor's gate: what it runs by default, and how a step whose tool is absent skips.

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";

import { defaultGate, gateForStage, runGate } from "../lib/gate.mjs";
import { readLedger } from "../lib/ledger.mjs";
import { tmp } from "./helpers.mjs";

/**
 * A clean repository with a tracked doc, a ledger outside it, and a gate whose one `ledger` step
 * stands in for the full suite: it counts its runs in a file outside the repository, prints a
 * nextest Summary, exits red while RED exists, and leaves an untracked file behind while LITTER does.
 */
function ledgerScratch() {
  const repo = tmp("rlx-gate-repo-");
  const sh = (...a) => {
    const r = spawnSync("git", a, { cwd: repo, encoding: "utf8" });
    assert.equal(r.status, 0, `${a.join(" ")}: ${r.stderr}`);
  };
  sh("init", "-q", "-b", "main");
  sh("config", "user.email", "t@example.invalid");
  sh("config", "user.name", "T");
  sh("config", "commit.gpgsign", "false");
  sh("config", "core.autocrlf", "false");
  writeFileSync(join(repo, "docs.md"), "docs\n");
  sh("add", "docs.md");
  sh("commit", "-q", "-m", "init");
  const outside = tmp("rlx-gate-outside-");
  const count = join(outside, "count.txt");
  const red = join(outside, "RED");
  const litter = join(outside, "LITTER");
  const body =
    `const fs=require('fs');fs.appendFileSync(${JSON.stringify(count)},'x');` +
    `if(fs.existsSync(${JSON.stringify(litter)}))fs.writeFileSync('litter.txt','x');` +
    `console.log('     Summary [   1.000s] 3 tests run: 3 passed, 1 skipped');process.exit(fs.existsSync(${JSON.stringify(red)})?100:0)`;
  const commands = [{ name: "cargo nextest", cmd: [process.execPath, "-e", body], ledger: true }];
  const ledger = join(outside, "suite-ledger.jsonl");
  const skipped = [];
  const gate = (stage) =>
    runGate({ cwd: repo, commands, logDir: join(outside, "gates"), label: `0101-${stage}`, ledger, onCommandSkipped: (c, r) => skipped.push(r) });
  const runs = () => (existsSync(count) ? readFileSync(count, "utf8").length : 0);
  return { repo, sh, gate, runs, ledger, skipped, red, litter };
}

test("two gate runs on one clean tree run the suite once, and the second records a skip naming the first", async () => {
  const s = ledgerScratch();
  const first = await s.gate("pre-review");
  assert.equal(first.ok, true);
  assert.deepEqual(first.commands.map((c) => [c.name, c.suite, c.skipped ?? false]), [["cargo nextest", true, false]]);
  const second = await s.gate("post-close");
  assert.equal(second.ok, true);
  assert.equal(s.runs(), 1, "the suite ran once");
  assert.deepEqual(second.ran, []);
  assert.deepEqual(second.commands, [{ name: "cargo nextest", code: 0, ms: 0, suite: true, skipped: true, by: "gate 0101-pre-review" }]);
  assert.equal(s.skipped.length, 1);
  assert.equal(s.skipped[0].by, "gate 0101-pre-review");
  assert.equal(s.skipped[0].summary, "3 tests run: 3 passed, 1 skipped");
  const [run, skip] = readLedger(s.ledger);
  assert.equal(run.by, "gate 0101-pre-review");
  assert.deepEqual({ ...skip, at: null }, { tree: run.tree, cmd: "cargo nextest run --workspace", skip: true, by: "gate 0101-post-close", at: null, green: { by: run.by, at: run.at } });
  const third = await s.gate("remerge");
  assert.equal(third.commands[0].skipped, true, "a skip line is never what a lookup relies on");
  assert.equal(s.runs(), 1);
});

test("a one-byte change to a tracked doc makes the next gate run the suite", async () => {
  const s = ledgerScratch();
  await s.gate("pre-review");
  writeFileSync(join(s.repo, "docs.md"), "docs!\n");
  // Uncommitted, the tree is dirty: the suite runs and nothing is recorded.
  await s.gate("fix-1");
  assert.equal(s.runs(), 2);
  assert.equal(readLedger(s.ledger).length, 1);
  s.sh("commit", "-q", "-am", "docs: one byte");
  await s.gate("fix-2");
  assert.equal(s.runs(), 3, "a committed one-byte change is a new tree");
  assert.equal(readLedger(s.ledger).length, 2);
});

test("a run that started dirty, or ended dirty, is not recorded", async () => {
  const s = ledgerScratch();
  writeFileSync(join(s.repo, "untracked.txt"), "x\n");
  await s.gate("pre-review");
  assert.equal(readLedger(s.ledger).length, 0, "started dirty");
  rmSync(join(s.repo, "untracked.txt"));

  writeFileSync(s.litter, "");
  await s.gate("fix-1");
  assert.equal(readLedger(s.ledger).length, 0, "ended dirty");
  rmSync(s.litter);
  rmSync(join(s.repo, "litter.txt"));

  await s.gate("fix-2");
  assert.equal(s.runs(), 3, "nothing was skipped on an unrecorded run");
  assert.equal(readLedger(s.ledger).length, 1);
});

test("a red run is recorded and never skipped on", async () => {
  const s = ledgerScratch();
  writeFileSync(s.red, "");
  const red = await s.gate("pre-review");
  assert.equal(red.ok, false);
  assert.deepEqual(readLedger(s.ledger).map((r) => r.exit), [100]);
  const again = await s.gate("fix-1");
  assert.equal(again.ok, false);
  assert.equal(s.runs(), 2, "the red tree ran again");
});

test("a gate given no ledger runs its ledger step every time and records nothing", async () => {
  const s = ledgerScratch();
  const commands = [{ name: "cargo nextest", cmd: [process.execPath, "-e", "0"], ledger: true }];
  for (let i = 0; i < 2; i++) {
    const g = await runGate({ cwd: s.repo, commands, logDir: tmp(), label: "0101-pre-review" });
    assert.deepEqual(g.ran, ["cargo nextest"]);
  }
  assert.equal(existsSync(s.ledger), false);
});

test("the default gate carries the pre-push suites it once missed", () => {
  const byName = Object.fromEntries(defaultGate().map((c) => [c.name, c]));
  assert.deepEqual(byName["conductor tests"].cmd, ["node", "--test", "tools/conductor/test/*.test.mjs"]);
  assert.deepEqual(byName["sd-filter tests"].cmd, ["python3", "tools/sd-filter/test_sd_filter.py"]);
  assert.deepEqual(byName["sd-filter tests"].onlyIfCommand, ["python3", "--version"]);
  // The suites run before the cargo steps, so a red there costs no Rust build.
  const names = defaultGate().map((c) => c.name);
  assert.ok(names.indexOf("sd-filter tests") < names.indexOf("cargo fmt"));
});

test("the backlog probes run only on a tree a close produced; every other default step runs at every stage", () => {
  const PROBE = "check-backlog-claims.mjs";
  const names = (stage) => gateForStage(stage).map((c) => c.name);
  const all = defaultGate().map((c) => c.name);
  assert.ok(all.includes(PROBE));
  const others = all.filter((n) => n !== PROBE);
  for (const stage of ["post-close", "remerge"]) {
    assert.ok(names(stage).includes(PROBE), `${stage} runs the probes`);
    assert.deepEqual(names(stage).filter((n) => n !== PROBE), others, `${stage} keeps every other step`);
  }
  for (const stage of ["pre-review", "fix-1", "fix-2", "fix-3"]) {
    assert.ok(!names(stage).includes(PROBE), `${stage} runs no probe`);
    assert.deepEqual(names(stage), others, `${stage} keeps every other step`);
  }
});

test("a step whose onlyIfCommand does not run is skipped, and one whose command runs is not", async () => {
  const node = process.execPath;
  const g = await runGate({
    cwd: tmp(),
    logDir: tmp(),
    label: "skip",
    commands: [
      { name: "absent tool", cmd: [node, "-e", "process.exit(1)"], onlyIfCommand: ["rlx-no-such-command-anywhere", "--version"] },
      { name: "present tool", cmd: [node, "-e", "process.exit(0)"], onlyIfCommand: [node, "--version"] },
    ],
  });
  assert.equal(g.ok, true);
  assert.deepEqual(g.ran, ["present tool"]);
});
