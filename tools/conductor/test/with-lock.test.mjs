// The machine-wide lock: two concurrent wrapped runs never overlap, a lock whose holder PID is dead
// is taken over, and the wrapper hands back the command's exit code.

import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { acquire, holder, pidAlive } from "../with-lock.mjs";

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
