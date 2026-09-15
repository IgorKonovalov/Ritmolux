// The machine-wide lock: two concurrent wrapped runs never overlap, a lock whose holder PID is dead
// is taken over, and the wrapper hands back the command's exit code.

import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { readLedger } from "../lib/ledger.mjs";
import { acquire, holder, isTestListing, pidAlive, runWrapped } from "../with-lock.mjs";

const WITH_LOCK = resolve(dirname(fileURLToPath(import.meta.url)), "..", "with-lock.mjs");

function run(args, env) {
  return new Promise((done) => {
    const child = spawn(process.execPath, [WITH_LOCK, ...args], {
      env: { ...process.env, ...env },
      stdio: ["ignore", "pipe", "pipe"],
    });
    let stderr = "";
    child.stderr.on("data", (d) => (stderr += d));
    child.on("exit", (code) => done({ code, stderr, pid: child.pid }));
  });
}

function freshDir() {
  return mkdtempSync(join(tmpdir(), "rlx-lock-test-"));
}

test("two concurrent invocations hold the lock one at a time", async () => {
  const dir = freshDir();
  const log = join(dir, "order.log");
  const body = (id) =>
    `const fs=require('fs');fs.appendFileSync(${JSON.stringify(log)},'start ${id}\\n');` +
    `setTimeout(()=>fs.appendFileSync(${JSON.stringify(log)},'end ${id}\\n'),600);`;
  const env = { RLX_LOCK_DIR: dir, RLX_LOCK_POLL_MS: "25" };
  const [a, b] = await Promise.all([
    run(["suite", "--", process.execPath, "-e", body("a")], env),
    run(["suite", "--", process.execPath, "-e", body("b")], env),
  ]);
  assert.equal(a.code, 0, a.stderr);
  assert.equal(b.code, 0, b.stderr);
  const lines = readFileSync(log, "utf8").trim().split("\n");
  assert.equal(lines.length, 4);
  // Whichever went first, its end precedes the other's start.
  const first = lines[0].split(" ")[1];
  const second = first === "a" ? "b" : "a";
  assert.deepEqual(lines, [`start ${first}`, `end ${first}`, `start ${second}`, `end ${second}`]);
  assert.match(a.stderr + b.stderr, /with-lock: waiting for "suite"/);
  assert.equal(holder("suite", dir), null, "the lock is released after both runs");
});

test("a lock whose holder PID is dead is taken over", async () => {
  const dir = freshDir();
  const dead = await new Promise((done) => {
    const child = spawn(process.execPath, ["-e", ""], { stdio: "ignore" });
    child.on("exit", () => done(child.pid));
  });
  assert.equal(pidAlive(dead), false, "the fixture PID must be dead before the test means anything");
  mkdirSync(dir, { recursive: true });
  writeFileSync(
    join(dir, "suite.lock"),
    JSON.stringify({ name: "suite", pid: dead, token: "stale", started: "2026-01-01T00:00:00Z" }),
  );
  const t0 = Date.now();
  const r = await run(["suite", "--", process.execPath, "-e", "process.exit(0)"], {
    RLX_LOCK_DIR: dir,
    RLX_LOCK_POLL_MS: "25",
  });
  assert.equal(r.code, 0, r.stderr);
  assert.ok(Date.now() - t0 < 10_000, "took over rather than waiting");
  assert.equal(existsSync(join(dir, "suite.lock")), false);
});

test("a lock whose holder is alive is not taken over", async () => {
  const dir = freshDir();
  const held = await acquire("close", { dir, pollMs: 25 });
  let second = null;
  const waiting = acquire("close", { dir, pollMs: 25 }).then((h) => (second = h));
  await new Promise((r) => setTimeout(r, 300));
  assert.equal(second, null, "the second acquire waits while the holder lives");
  held.release();
  await waiting;
  assert.ok(second.waitedMs >= 250);
  second.release();
});

test("only `cargo nextest list` is a listing", () => {
  assert.equal(isTestListing("cargo", ["nextest", "list", "-p", "rlx-core"]), true);
  assert.equal(isTestListing("cargo.exe", ["+nightly", "nextest", "list"]), true);
  assert.equal(isTestListing("C:/Users/x/.cargo/bin/cargo.exe", ["nextest", "list", "--workspace"]), true);
  assert.equal(isTestListing("cargo", ["nextest", "run", "-p", "rlx-core"]), false);
  assert.equal(isTestListing("cargo", ["test", "list"]), false);
  assert.equal(isTestListing("node", ["nextest", "list"]), false);
});

test("a wrapped list whose lock is held by another process starts at once, and logs no lock entry", async () => {
  const dir = freshDir();
  const logFile = join(dir, "locks.jsonl");
  const held = await acquire("suite", { dir, pollMs: 25 });
  try {
    const t0 = Date.now();
    // Whether or not nextest is installed here, the point is that the command ran without waiting.
    const r = await run(["suite", "--", "cargo", "nextest", "list", "--help"], { RLX_LOCK_DIR: dir, RLX_LOCK_LOG: logFile, RLX_LOCK_POLL_MS: "25" });
    assert.ok(Date.now() - t0 < 30_000, "did not wait for the held lock");
    assert.doesNotMatch(r.stderr, /waiting for "suite"/);
    assert.doesNotMatch(r.stderr, /with-lock: "suite" waited/);
    assert.equal(existsSync(logFile), false, "no lock log entry");
    assert.equal(holder("suite", dir).token, held.token, "the holder still holds it");
  } finally {
    held.release();
  }
});

/** A clean scratch repository and a stand-in for spawning the command, counting its calls. */
function wrapperScratch() {
  const repo = mkdtempSync(join(tmpdir(), "rlx-wrap-repo-"));
  const sh = (...a) => assert.equal(spawnSync("git", a, { cwd: repo, encoding: "utf8" }).status, 0, a.join(" "));
  sh("init", "-q", "-b", "main");
  sh("config", "user.email", "t@example.invalid");
  sh("config", "user.name", "T");
  sh("config", "commit.gpgsign", "false");
  writeFileSync(join(repo, "a.txt"), "a\n");
  sh("add", "a.txt");
  sh("commit", "-q", "-m", "init");
  const calls = [];
  const run = async (command, args, opts) => {
    calls.push({ argv: [command, ...args], capture: opts.capture });
    return { code: 0, output: "     Summary [   2.000s] 5 tests run: 5 passed\n" };
  };
  const ledger = join(freshDir(), "suite-ledger.jsonl");
  const env = { RLX_LOCK_DIR: freshDir(), RLX_SUITE_LEDGER: ledger, RLX_SUITE_LEDGER_BY: "0101-03-review" };
  return { repo, calls, run, ledger, env };
}

async function quietly(fn) {
  const out = [];
  const [o, e] = [process.stdout.write, process.stderr.write];
  process.stdout.write = (s) => (out.push(String(s)), true);
  process.stderr.write = (s) => (out.push(String(s)), true);
  try {
    return { value: await fn(), out: out.join("") };
  } finally {
    process.stdout.write = o;
    process.stderr.write = e;
  }
}

const SUITE_ARGV = ["suite", "--", "cargo", "nextest", "run", "--workspace"];

test("a wrapped full suite with RLX_SUITE_LEDGER records its run, and skips the next on the same tree, naming the record", async () => {
  const s = wrapperScratch();
  const first = await quietly(() => runWrapped(SUITE_ARGV, { env: s.env, cwd: s.repo, run: s.run }));
  assert.equal(first.value, 0);
  assert.deepEqual(s.calls, [{ argv: ["cargo", "nextest", "run", "--workspace"], capture: true }]);
  const [rec] = readLedger(s.ledger);
  assert.equal(rec.by, "0101-03-review");
  assert.equal(rec.summary, "5 tests run: 5 passed");
  assert.equal(rec.exit, 0);

  const second = await quietly(() => runWrapped(SUITE_ARGV, { env: { ...s.env, RLX_SUITE_LEDGER_BY: "0101-04-review" }, cwd: s.repo, run: s.run }));
  assert.equal(second.value, 0);
  assert.equal(s.calls.length, 1, "the second run did not execute");
  assert.equal(
    second.out,
    `with-lock: skipped cargo nextest run --workspace: tree ${rec.tree.slice(0, 7)} is green in the suite ledger, run by 0101-03-review at ${rec.at}: 5 tests run: 5 passed\n`,
  );
  assert.equal(readLedger(s.ledger).length, 1);
});

test("any other argument vector neither skips nor records", async () => {
  const s = wrapperScratch();
  await quietly(() => runWrapped(SUITE_ARGV, { env: s.env, cwd: s.repo, run: s.run }));
  for (const extra of [["--no-fail-fast"], ["-P", "fast"]]) {
    await quietly(() => runWrapped([...SUITE_ARGV, ...extra], { env: s.env, cwd: s.repo, run: s.run }));
    assert.equal(s.calls.at(-1).capture, false);
  }
  assert.equal(s.calls.length, 3, "both ran on a tree the ledger records green");
  assert.equal(readLedger(s.ledger).length, 1, "neither recorded");
});

test("without RLX_SUITE_LEDGER the wrapper neither reads nor writes a ledger, and its output is the command's own", async () => {
  const s = wrapperScratch();
  const { RLX_SUITE_LEDGER, ...env } = s.env;
  // A ledger that records this tree green: without the variable the wrapper never looks at it.
  const tree = spawnSync("git", ["rev-parse", "HEAD^{tree}"], { cwd: s.repo, encoding: "utf8" }).stdout.trim();
  writeFileSync(RLX_SUITE_LEDGER, JSON.stringify({ tree, cmd: "cargo nextest run --workspace", exit: 0, summary: "x", by: "gate", at: "2026-09-15T00:00:00Z", ms: 1 }) + "\n");
  for (let i = 0; i < 2; i++) await quietly(() => runWrapped(SUITE_ARGV, { env, cwd: s.repo, run: s.run }));
  assert.deepEqual(s.calls.map((c) => c.capture), [false, false], "ran both times, stdio inherited");
  assert.equal(readLedger(RLX_SUITE_LEDGER).length, 1);

  // As a process, the wrapper's stdout is exactly the command's.
  const dir = freshDir();
  const r = await new Promise((done) => {
    const child = spawn(process.execPath, [WITH_LOCK, "suite", "--", process.execPath, "-e", "process.stdout.write('the command output')"], {
      env: { ...process.env, RLX_LOCK_DIR: dir, RLX_SUITE_LEDGER: "" },
      stdio: ["ignore", "pipe", "pipe"],
    });
    let stdout = "";
    child.stdout.on("data", (d) => (stdout += d));
    child.on("close", (code) => done({ code, stdout }));
  });
  assert.equal(r.code, 0);
  assert.equal(r.stdout, "the command output");
});

test("the wrapper exits with the command's exit code and logs the run", async () => {
  const dir = freshDir();
  const logFile = join(dir, "locks.jsonl");
  const r = await run(["suite", "--", process.execPath, "-e", "process.exit(3)"], {
    RLX_LOCK_DIR: dir,
    RLX_LOCK_LOG: logFile,
  });
  assert.equal(r.code, 3);
  const entry = JSON.parse(readFileSync(logFile, "utf8").trim());
  assert.equal(entry.lock, "suite");
  assert.equal(entry.exit_code, 3);
  assert.equal(typeof entry.waited_ms, "number");
});
