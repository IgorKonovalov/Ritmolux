// The machine-wide lock: two concurrent wrapped runs never overlap, a lock whose holder PID is dead
// is taken over, and the wrapper hands back the command's exit code.

import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { readLedger } from "../lib/ledger.mjs";
import { acquire, holder, isTestListing, pidAlive, runWrapped, suiteLedger } from "../with-lock.mjs";
import { RED_NEXTEST_OUTPUT, tmp } from "./helpers.mjs";

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
  return tmp("rlx-lock-test-");
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
  const repo = tmp("rlx-wrap-repo-");
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
  // Only strings are captured. A test file runs as a child of `node --test`, and the runner reports
  // through this process's stdout as serialized Buffers, which on Node 26 can land while a sibling
  // test completes inside the patched window. A Buffer goes on to the real stream.
  process.stdout.write = (s, ...rest) => (typeof s === "string" ? (out.push(s), true) : o.call(process.stdout, s, ...rest));
  process.stderr.write = (s, ...rest) => (typeof s === "string" ? (out.push(s), true) : e.call(process.stderr, s, ...rest));
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
  const lines = readLedger(s.ledger);
  assert.equal(lines.length, 2);
  assert.deepEqual({ ...lines[1], at: null }, { tree: rec.tree, cmd: "cargo nextest run --workspace", skip: true, by: "0101-04-review", at: null, green: { by: rec.by, at: rec.at } });
});

test("a wrapped full suite that fails records both failing tests' names from nextest's output", async () => {
  const s = wrapperScratch();
  const red = async () => ({ code: 100, output: RED_NEXTEST_OUTPUT });
  const r = await quietly(() => runWrapped(SUITE_ARGV, { env: s.env, cwd: s.repo, run: red }));
  assert.equal(r.value, 100);
  const [rec] = readLedger(s.ledger);
  assert.equal(rec.exit, 100);
  assert.equal(rec.summary, "1805 tests run: 1803 passed (11 slow), 2 failed, 11 skipped");
  assert.deepEqual(rec.failed, ["rlx-core::golden golden_rose_star", "standalone::shot_cli the_count_column"]);
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

test("outside this repository the wrapper neither reads nor writes a ledger, and its output is the command's own", async () => {
  const s = wrapperScratch();
  const { RLX_SUITE_LEDGER, ...env } = s.env;
  // A ledger that records this tree green: with no variable and a scratch repository that is not the
  // one this script lives in, the wrapper never looks at it.
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

test("a .cmd shim named without its extension exits with the shim's own code, not the failed direct spawn's", { skip: process.platform !== "win32" && "a .cmd shim and its PATHEXT lookup exist only on Windows" }, async () => {
  const dir = freshDir();
  writeFileSync(join(dir, "rlx-shim.cmd"), "@exit /b 3\r\n");
  const logFile = join(dir, "locks.jsonl");
  const r = await run(["suite", "--", join(dir, "rlx-shim")], { RLX_LOCK_DIR: dir, RLX_LOCK_LOG: logFile });
  assert.equal(r.code, 3);
  assert.equal(JSON.parse(readFileSync(logFile, "utf8").trim()).exit_code, 3);
});

// Backlog 0232: a suite an operator ran by hand through the wrapper left no ledger record, so the
// conductor's next gate on the same tree ran it again — 12.7 minutes, twice, on 2026-09-15.

/**
 * A scratch repository standing in for this one: the wrapper's own directory is inside it, and
 * `state/` is gitignored there as it is here — otherwise the ledger the wrapper writes would itself
 * dirty the tree and stop the next lookup ever matching.
 */
function handScratch() {
  const s = wrapperScratch();
  const selfDir = join(s.repo, "tools", "conductor");
  mkdirSync(selfDir, { recursive: true });
  writeFileSync(join(s.repo, ".gitignore"), "tools/conductor/state/\n");
  const sh = (...a) => assert.equal(spawnSync("git", a, { cwd: s.repo, encoding: "utf8" }).status, 0, a.join(" "));
  sh("add", ".gitignore");
  sh("commit", "-q", "-m", "chore: ignore the conductor's runtime state");
  const { RLX_SUITE_LEDGER, RLX_SUITE_LEDGER_BY, ...env } = s.env;
  return { ...s, selfDir, env, ledger: join(selfDir, "state", "suite-ledger.jsonl") };
}

test("a full suite run by hand from the repository records itself as hand, and the next run on that tree skips", async () => {
  const s = handScratch();
  const first = await quietly(() => runWrapped(SUITE_ARGV, { env: s.env, cwd: s.repo, run: s.run, selfDir: s.selfDir }));
  assert.equal(first.value, 0);
  const [rec] = readLedger(s.ledger);
  assert.equal(rec.by, "hand", "the writer says a person ran it");
  assert.equal(rec.exit, 0);
  assert.equal(rec.summary, "5 tests run: 5 passed");

  // What the conductor's gate does next on the same tree: it finds the record and names it.
  const second = await quietly(() => runWrapped(SUITE_ARGV, { env: { ...s.env, RLX_SUITE_LEDGER: s.ledger, RLX_SUITE_LEDGER_BY: "gate 0101-pre-review" }, cwd: s.repo, run: s.run }));
  assert.equal(second.value, 0);
  assert.equal(s.calls.length, 1, "the gate did not run it again");
  assert.match(second.out, /is green in the suite ledger, run by hand at /);
  assert.equal(readLedger(s.ledger).at(-1).green.by, "hand");
});

/** A worktree of `s.repo` with a conductor directory of its own, as a lane of this repository has. */
function lane(s, name) {
  const dir = join(freshDir(), name);
  assert.equal(spawnSync("git", ["worktree", "add", "-q", "-b", name, dir], { cwd: s.repo, encoding: "utf8" }).status, 0);
  const selfDir = join(dir, "tools", "conductor");
  mkdirSync(selfDir, { recursive: true });
  return { dir, selfDir };
}

test("a hand run in a worktree of the repository records into the same ledger", async () => {
  const s = handScratch();
  // A lane is a worktree: `git rev-parse --git-common-dir` is the main checkout's either way.
  const l = lane(s, "rlx-plan-0190");
  const r = await quietly(() => runWrapped(SUITE_ARGV, { env: s.env, cwd: l.dir, run: s.run, selfDir: s.selfDir }));
  assert.equal(r.value, 0);
  assert.deepEqual(readLedger(s.ledger).map((e) => e.by), ["hand"]);
});

// Backlog 0247: the operator repairs inside a lane, and the wrapper there is the lane's own copy of
// this script, so the record used to land in the one place no gate reads.
test("a hand run through a lane's own copy of the wrapper records into the main checkout's ledger", async () => {
  const s = handScratch();
  const l = lane(s, "rlx-plan-0197");
  const picked = suiteLedger(l.dir, {}, l.selfDir);
  assert.equal(picked.by, "hand");
  assert.equal(picked.notice, undefined, "the main checkout was derived, so there is nothing to say");
  assert.ok(!picked.path.toLowerCase().startsWith(l.dir.toLowerCase()), `${picked.path} is outside the lane`);

  const first = await quietly(() => runWrapped(SUITE_ARGV, { env: s.env, cwd: l.dir, run: s.run, selfDir: l.selfDir }));
  assert.equal(first.value, 0);
  assert.deepEqual(readLedger(s.ledger).map((e) => e.by), ["hand"], "recorded where the gate reads");
  assert.deepEqual(readLedger(join(l.selfDir, "state", "suite-ledger.jsonl")), [], "and not in the lane");

  // The gate the conductor runs next, from the main checkout, on the tree the lane is at.
  const gate = await quietly(() =>
    runWrapped(SUITE_ARGV, { env: { ...s.env, RLX_SUITE_LEDGER: s.ledger, RLX_SUITE_LEDGER_BY: "gate 0101-pre-review" }, cwd: s.repo, run: s.run }),
  );
  assert.equal(gate.value, 0);
  assert.equal(s.calls.length, 1, "the gate did not run the suite again");
  assert.match(gate.out, /is green in the suite ledger, run by hand at /);
});

// ADR-0016's shape: what cannot be derived is said in one line, and the old behaviour carries on.
test("a common directory with no checkout beside it falls back to this script's own state, with a notice", async () => {
  const repo = tmp("rlx-wrap-sep-");
  const gitDir = join(freshDir(), "relocated.git");
  const sh = (...a) => assert.equal(spawnSync("git", a, { cwd: repo, encoding: "utf8" }).status, 0, a.join(" "));
  sh("init", "-q", "-b", "main", `--separate-git-dir=${gitDir}`);
  sh("config", "user.email", "t@example.invalid");
  sh("config", "user.name", "T");
  sh("config", "commit.gpgsign", "false");
  writeFileSync(join(repo, ".gitignore"), "tools/conductor/state/\n");
  sh("add", ".gitignore");
  sh("commit", "-q", "-m", "init");
  const selfDir = join(repo, "tools", "conductor");
  mkdirSync(selfDir, { recursive: true });

  const picked = suiteLedger(repo, {}, selfDir);
  assert.equal(picked.path, join(selfDir, "state", "suite-ledger.jsonl"), "today's destination, unchanged");
  assert.equal(picked.by, "hand");
  assert.match(picked.notice, /^no main checkout beside .*relocated\.git, so the suite ledger stays beside this script/);

  const calls = [];
  const run = async (command, args, opts) => {
    calls.push(opts.capture);
    return { code: 0, output: "     Summary [   2.000s] 5 tests run: 5 passed\n" };
  };
  const r = await quietly(() => runWrapped(SUITE_ARGV, { env: { RLX_LOCK_DIR: freshDir() }, cwd: repo, run, selfDir }));
  assert.equal(r.value, 0);
  assert.match(r.out, /^with-lock: notice: no main checkout beside /m);
  assert.deepEqual(readLedger(picked.path).map((e) => e.by), ["hand"], "it still records, where it always did");
});

test("a hand run that starts or ends dirty records nothing", async () => {
  const s = handScratch();
  writeFileSync(join(s.repo, "b.txt"), "untracked\n");
  await quietly(() => runWrapped(SUITE_ARGV, { env: s.env, cwd: s.repo, run: s.run, selfDir: s.selfDir }));
  assert.equal(s.calls.length, 1, "it still ran");
  assert.deepEqual(readLedger(s.ledger), [], "dirty at both ends: nothing recorded");

  rmSync(join(s.repo, "b.txt"));
  const dirtying = async (command, args, opts) => {
    const out = await s.run(command, args, opts);
    writeFileSync(join(s.repo, "c.txt"), "the run left this\n");
    return out;
  };
  await quietly(() => runWrapped(SUITE_ARGV, { env: s.env, cwd: s.repo, run: dirtying, selfDir: s.selfDir }));
  assert.deepEqual(readLedger(s.ledger), [], "clean at the start, dirty at the end: nothing recorded");
});

test("an explicit RLX_SUITE_LEDGER still selects that file, and a run outside any repository selects none", async () => {
  const s = handScratch();
  const explicit = join(freshDir(), "elsewhere.jsonl");
  await quietly(() => runWrapped(SUITE_ARGV, { env: { ...s.env, RLX_SUITE_LEDGER: explicit, RLX_SUITE_LEDGER_BY: "0101-03-review" }, cwd: s.repo, run: s.run, selfDir: s.selfDir }));
  assert.deepEqual(readLedger(explicit).map((e) => e.by), ["0101-03-review"]);
  assert.deepEqual(readLedger(s.ledger), [], "the default was not written");

  // Not a repository at all, and a different repository: neither is this one's evidence.
  const nowhere = freshDir();
  await quietly(() => runWrapped(SUITE_ARGV, { env: s.env, cwd: nowhere, run: s.run, selfDir: s.selfDir }));
  const other = wrapperScratch();
  await quietly(() => runWrapped(SUITE_ARGV, { env: s.env, cwd: other.repo, run: s.run, selfDir: s.selfDir }));
  assert.deepEqual(readLedger(s.ledger), []);
  assert.equal(s.calls.length, 3, "all three ran, none recorded");
});

test("suiteLedger is what decides, and says so for each case", () => {
  const s = handScratch();
  assert.deepEqual(suiteLedger(s.repo, { RLX_SUITE_LEDGER: "x.jsonl", RLX_SUITE_LEDGER_BY: "0101-02-review" }, s.selfDir), { path: "x.jsonl", by: "0101-02-review" });
  assert.deepEqual(suiteLedger(s.repo, { RLX_SUITE_LEDGER: "x.jsonl" }, s.selfDir), { path: "x.jsonl", by: "a session" });
  assert.deepEqual(suiteLedger(s.repo, {}, s.selfDir), { path: s.ledger, by: "hand" });
  assert.equal(suiteLedger(freshDir(), {}, s.selfDir), null);
});
