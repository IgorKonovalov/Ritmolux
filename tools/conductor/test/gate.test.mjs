// The conductor's gate: what it runs by default, and how a step whose tool is absent skips.

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";

import { GATES, invocation, invocationsFor } from "../../../scripts/gates.manifest.mjs";
import { defaultGate, gateForStage, runGate } from "../lib/gate.mjs";
import { greenRecord, readLedger, SERVED_COMMAND, SERVED_SUITE_ARGS, servingRecord, SUITE_COMMAND } from "../lib/ledger.mjs";
import { tmp } from "./helpers.mjs";

/**
 * A clean repository with a tracked doc, a ledger outside it, and a gate whose one `ledger` step
 * stands in for the full suite: it records its own argument vector and the suite lock's holder in
 * files outside the repository, prints a nextest Summary, exits red while RED exists, and leaves an
 * untracked file behind while LITTER does.
 *
 * The stand-in is a script file rather than `node -e`, because node parses `-P` after `-e <code>` as
 * one of its own options: a step whose tier appends `-P fast` has to reach the script as arguments.
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
  mkdirSync(join(repo, "core", "src"), { recursive: true });
  writeFileSync(join(repo, "core", "src", "lib.rs"), "pub fn a() {}\n");
  sh("add", "docs.md", "core/src/lib.rs");
  sh("commit", "-q", "-m", "init");
  const outside = tmp("rlx-gate-outside-");
  const argv = join(outside, "argv.jsonl");
  const held = join(outside, "held.jsonl");
  const red = join(outside, "RED");
  const litter = join(outside, "LITTER");
  const lockDir = join(outside, "locks");
  const script = join(outside, "suite-stand-in.cjs");
  const q = (p) => JSON.stringify(p);
  writeFileSync(
    script,
    `const fs=require('fs');fs.appendFileSync(${q(argv)},JSON.stringify(process.argv.slice(2))+'\\n');\n` +
      `let holder='none';try{holder=JSON.parse(fs.readFileSync(${q(join(lockDir, "suite.lock"))},'utf8')).what}catch(e){}\n` +
      `fs.appendFileSync(${q(held)},holder+'\\n');\n` +
      `if(fs.existsSync(${q(litter)}))fs.writeFileSync('litter.txt','x');\n` +
      `const red=fs.existsSync(${q(red)});\n` +
      `if(red)console.log('        FAIL [   1.000s] rlx-core::reactivity pulse_holds');\n` +
      `console.log('     Summary [   1.000s] 3 tests run: 3 passed, 1 skipped');process.exit(red?100:0)\n`,
  );
  const commands = [{ name: "cargo nextest", cmd: [process.execPath, script], ledger: true, lock: "suite" }];
  const ledger = join(outside, "suite-ledger.jsonl");
  const skipped = [];
  const served = [];
  const gate = (stage) =>
    runGate({
      cwd: repo,
      commands,
      logDir: join(outside, "gates"),
      label: `0101-${stage}`,
      ledger,
      lockDir,
      lockPollMs: 20,
      onCommandSkipped: (c, r) => skipped.push(r),
      onCommandServed: (c, s) => served.push(s),
    });
  const readLines = (file) => (existsSync(file) ? readFileSync(file, "utf8").split("\n").filter(Boolean) : []);
  const runs = () => readLines(argv).length;
  return { repo, sh, gate, runs, ledger, skipped, served, red, litter, vectors: () => readLines(argv).map((l) => JSON.parse(l)), holders: () => readLines(held) };
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
  // The tier did not disturb this: an exact green record still runs no suite command at all, so the
  // one vector recorded is the first gate's full suite.
  assert.deepEqual(s.vectors(), [[]]);
  assert.deepEqual(s.served, []);
});

test("a one-byte change to a tracked Rust file makes the next gate run the full suite", async () => {
  const s = ledgerScratch();
  await s.gate("pre-review");
  writeFileSync(join(s.repo, "core", "src", "lib.rs"), "pub fn b() {}\n");
  // Uncommitted, the tree is dirty: the suite runs and nothing is recorded.
  await s.gate("fix-1");
  assert.equal(s.runs(), 2);
  assert.equal(readLedger(s.ledger).length, 1);
  s.sh("commit", "-q", "-am", "feat: one byte");
  await s.gate("fix-2");
  assert.equal(s.runs(), 3, "a committed one-byte change is a new tree");
  assert.equal(readLedger(s.ledger).length, 2);
  assert.deepEqual(s.vectors(), [[], [], []], "one `.rs` in the diff is never served down to `-P fast`");
  assert.deepEqual(
    readLedger(s.ledger).map((e) => e.cmd),
    [SUITE_COMMAND, SUITE_COMMAND],
  );
});

// ADR-0211's three states. `skipped` is the test above this block; these are `served` and `ran`.

test("a served tree runs the suite command plus -P fast, under the same lock, and records a served line", async () => {
  // The vector the tier produces, verbatim, off the real default gate rather than off a stand-in.
  const suiteStep = defaultGate().find((c) => c.ledger);
  assert.deepEqual([...suiteStep.cmd, ...SERVED_SUITE_ARGS], ["cargo", "nextest", "run", "--workspace", "-P", "fast"]);
  assert.equal([...suiteStep.cmd, ...SERVED_SUITE_ARGS].join(" "), SERVED_COMMAND);

  const s = ledgerScratch();
  await s.gate("pre-review");
  const greenTree = readLedger(s.ledger)[0].tree;
  // A close's shape: a tracked document and nothing else.
  writeFileSync(join(s.repo, "docs.md"), "docs, repaired by the close\n");
  s.sh("commit", "-q", "-am", "docs: the close repairs a finding");

  const g = await s.gate("post-close");
  assert.equal(g.ok, true);
  assert.deepEqual(g.ran, ["cargo nextest"], "a served step runs, unlike a skipped one");
  assert.deepEqual(s.vectors().at(-1), ["-P", "fast"]);
  assert.equal(s.runs(), 2);
  assert.deepEqual(s.skipped, [], "nothing was skipped");
  assert.equal(s.served.length, 1);
  assert.equal(s.served[0].record.tree, greenTree);
  assert.deepEqual(s.served[0].paths, ["docs.md"]);
  assert.equal(g.commands[0].served, true);
  assert.equal(g.commands[0].suite, true);
  assert.equal(g.commands[0].by, "gate 0101-pre-review");

  // The suite lock was held while the served command ran, as it is for a full one.
  assert.deepEqual(s.holders(), ["gate 0101-pre-review: cargo nextest", "gate 0101-post-close: cargo nextest"]);

  const line = readLedger(s.ledger).at(-1);
  assert.equal(line.served, true);
  assert.equal(line.cmd, SERVED_COMMAND);
  assert.equal(line.exit, 0);
  assert.equal(line.by, "gate 0101-post-close");
  assert.equal(line.green.tree, greenTree);
  assert.equal(line.green.by, "gate 0101-pre-review");
  assert.deepEqual(line.diff, ["docs.md"]);

  // The served line is not a green record for its own tree, and it serves no later tree: the next
  // stage leans on the full-suite record again, never on one `-P fast` pass off another.
  const servedTree = line.tree;
  assert.equal(greenRecord(s.ledger, servedTree), null);
  writeFileSync(join(s.repo, "docs.md"), "docs, and one more repair\n");
  s.sh("commit", "-q", "-am", "docs: one more");
  await s.gate("remerge");
  const next = readLedger(s.ledger).at(-1);
  assert.equal(next.served, true);
  assert.equal(next.green.tree, greenTree, "the full-suite record, not the served line");
  assert.notEqual(next.green.tree, servedTree);
  // And the served line alone serves nothing: a ledger holding only it has no candidate.
  const onlyServed = join(tmp("rlx-gate-served-only-"), "suite-ledger.jsonl");
  writeFileSync(onlyServed, JSON.stringify(line) + "\n");
  assert.equal(servingRecord(onlyServed, next.tree, s.repo), null);
});

test("a red -P fast on a served tree fails the gate, names its failing tests, and writes its log", async () => {
  const s = ledgerScratch();
  await s.gate("pre-review");
  writeFileSync(join(s.repo, "docs.md"), "docs, repaired\n");
  s.sh("commit", "-q", "-am", "docs: a repair");
  writeFileSync(s.red, "");

  const g = await s.gate("post-close");
  assert.equal(g.ok, false);
  assert.equal(g.failed.name, "cargo nextest");
  assert.equal(g.failed.code, 100);
  assert.deepEqual(g.failed.tests, ["rlx-core::reactivity pulse_holds"]);
  assert.deepEqual(s.vectors().at(-1), ["-P", "fast"], "the red run was the served tier's");
  assert.match(readFileSync(g.failed.log, "utf8"), /FAIL \[   1\.000s\] rlx-core::reactivity pulse_holds/);
  assert.equal(join(g.failed.log, "..").endsWith("gates"), true, "logged under the gate log directory");
  const line = readLedger(s.ledger).at(-1);
  assert.equal(line.served, true);
  assert.equal(line.exit, 100);
  // A red served run is a record like any other red: it is never leaned on, and never skipped.
  assert.equal(greenRecord(s.ledger, line.tree), null);
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

// ADR-0217: the gate's Node block is the manifest's projection, not a copy of it.

test("the gate's node steps are exactly the manifest's conductor projection, in order", () => {
  const steps = defaultGate()
    .filter((c) => c.cmd[0] === "node" && String(c.cmd[1]).startsWith("scripts/"))
    .map((c) => c.cmd.join(" "));
  assert.deepEqual(steps, invocationsFor("conductor"));
  // A name added to one and not the other is this assertion, and the two the conductor was missing
  // when the manifest was written are the reason it exists.
  for (const script of ["check-translations.mjs", "check-system-counts.mjs"]) {
    assert.ok(
      steps.some((s) => s.includes(script)),
      `${script} is in the gate`,
    );
  }
});

test("the site gates are in the manifest and not in the gate, because they need a built site", () => {
  const site = GATES.filter((g) => g.script.startsWith("check-site-"));
  assert.deepEqual(
    site.map(invocation),
    ["node scripts/check-site-links.mjs --require-api", "node scripts/check-site-routes.mjs"],
    "both are rostered",
  );
  for (const g of site) assert.deepEqual(g.carriers, ["pages"], `${g.script} is carried by pages alone`);
  const names = defaultGate().map((c) => c.cmd.join(" "));
  assert.ok(!names.some((n) => n.includes("check-site-")), "and neither is a gate step");
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

test("a .cmd shim named without its extension reports the shim's own exit code and output, not the failed direct spawn's", { skip: process.platform !== "win32" }, async () => {
  const dir = tmp();
  writeFileSync(join(dir, "rlx-shim.cmd"), "@echo shim ran\r\n@exit /b 3\r\n");
  const g = await runGate({ cwd: dir, logDir: tmp(), label: "shim", commands: [{ name: "shim", cmd: [join(dir, "rlx-shim")] }] });
  assert.equal(g.ok, false);
  assert.equal(g.failed.code, 3);
  assert.match(readFileSync(g.failed.log, "utf8"), /shim ran/);
});
