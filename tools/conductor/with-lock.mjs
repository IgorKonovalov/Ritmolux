#!/usr/bin/env node
// A machine-wide named lock, as a command wrapper and as a module.
//
//   node tools/conductor/with-lock.mjs <name> -- <command> [args...]
//
// Takes the lock, runs the command with inherited stdio, releases the lock, and exits with the
// command's exit code. The conductor imports `acquire` to hold the close lock across several steps.
// A wrapped `cargo nextest list` runs no test, so it runs at once without the lock.
//
// The lock is a file, <lock dir>/<name>.lock, created by hard-linking a fully written temp file
// onto the lock path: the link either fails with EEXIST or produces a complete file, so a reader
// never sees a half-written holder. The holder is the PID of the process that took it. A lock whose
// holder PID is no longer alive is taken over; the takeover itself runs under a short-lived
// `<name>.lock.takeover` guard, so two waiters that both see the same dead holder cannot each
// delete the lock the other has just taken. A guard older than GUARD_STALE_MS is abandoned.
//
// Every lane on the machine shares one lock directory — os.tmpdir()/rlx-conductor-locks, or
// RLX_LOCK_DIR — which is what makes the lock machine-wide rather than per-worktree.
// RLX_LOCK_LOG, when set, receives one JSON line per wrapped run: the lock, the wait and the hold.
// The wrapper also prints the wait and the hold on stderr as it exits.
// Trap: a PID can be reused by an unrelated process after the holder dies; the lock then waits on a
// stranger until that process exits. The holder file records the command so a person can tell.

import { spawn } from "node:child_process";
import { randomBytes } from "node:crypto";
import {
  appendFileSync,
  linkSync,
  mkdirSync,
  readFileSync,
  statSync,
  unlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { basename, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { appendRecord, appendSkip, cleanTree, greenRecord, isFullSuite, skipNotice, summaryLine } from "./lib/ledger.mjs";

const GUARD_STALE_MS = 10_000;

export function lockDir(env = process.env) {
  return env.RLX_LOCK_DIR || join(tmpdir(), "rlx-conductor-locks");
}

export function pidAlive(pid) {
  if (!Number.isInteger(pid) || pid <= 0) return false;
  try {
    process.kill(pid, 0);
    return true;
  } catch (e) {
    return e.code === "EPERM";
  }
}

function lockPath(name, dir) {
  if (!/^[a-z][a-z0-9-]*$/.test(name)) throw new Error(`invalid lock name: ${name}`);
  return join(dir, `${name}.lock`);
}

/** The current holder of `name`, or null when the lock is free or its file is unreadable. */
export function holder(name, dir = lockDir()) {
  try {
    return JSON.parse(readFileSync(lockPath(name, dir), "utf8"));
  } catch {
    return null;
  }
}

function tryCreate(path, record) {
  const tmp = `${path}.${process.pid}.${randomBytes(4).toString("hex")}.tmp`;
  writeFileSync(tmp, JSON.stringify(record));
  try {
    linkSync(tmp, path);
    return true;
  } catch (e) {
    if (e.code === "EEXIST") return false;
    throw e;
  } finally {
    try {
      unlinkSync(tmp);
    } catch {}
  }
}

function takeOverIfDead(path, seen) {
  const guard = `${path}.takeover`;
  try {
    writeFileSync(guard, String(process.pid), { flag: "wx" });
  } catch (e) {
    if (e.code !== "EEXIST") throw e;
    try {
      if (Date.now() - statSync(guard).mtimeMs > GUARD_STALE_MS) unlinkSync(guard);
    } catch {}
    return;
  }
  try {
    let current = null;
    try {
      current = JSON.parse(readFileSync(path, "utf8"));
    } catch {}
    if (current && current.token === seen.token && !pidAlive(current.pid)) unlinkSync(path);
  } finally {
    try {
      unlinkSync(guard);
    } catch {}
  }
}

/**
 * Waits for and takes the lock. Resolves to a handle whose `release()` frees it (only if this
 * process still holds it) and whose `waitedMs` is how long the wait took.
 */
export async function acquire(name, opts = {}) {
  const dir = opts.dir ?? lockDir();
  const pollMs = opts.pollMs ?? Number(process.env.RLX_LOCK_POLL_MS || 500);
  mkdirSync(dir, { recursive: true });
  const path = lockPath(name, dir);
  const record = {
    name,
    pid: opts.pid ?? process.pid,
    token: randomBytes(8).toString("hex"),
    started: new Date().toISOString(),
    what: opts.what ?? null,
  };
  const t0 = Date.now();
  let announced = false;
  for (;;) {
    if (tryCreate(path, record)) break;
    const seen = holder(name, dir);
    if (seen && !pidAlive(seen.pid)) {
      takeOverIfDead(path, seen);
      continue;
    }
    if (!announced && opts.onWait) opts.onWait(seen);
    announced = true;
    await new Promise((r) => setTimeout(r, pollMs));
  }
  const waitedMs = Date.now() - t0;
  return {
    name,
    token: record.token,
    waitedMs,
    acquiredAt: Date.now(),
    release() {
      const current = holder(name, dir);
      if (current && current.token === record.token) {
        try {
          unlinkSync(path);
        } catch {}
      }
    },
  };
}

/**
 * Runs a command, forwarding SIGINT and SIGTERM, and resolves to { code, output }. With `capture`
 * false its stdio is inherited and `output` is empty; with `capture` true its stdout and stderr are
 * piped through to this process's own as they arrive, and `output` keeps their last megabyte.
 */
function spawnRun(command, args, { capture = false } = {}) {
  return new Promise((resolveRun) => {
    let finished = false;
    let output = "";
    const finish = (code) => {
      if (finished) return;
      finished = true;
      resolveRun({ code, output });
    };
    const stdio = capture ? ["inherit", "pipe", "pipe"] : "inherit";
    const start = (shell) => {
      const quote = (a) => (/[\s"]/.test(a) ? `"${a.replace(/"/g, '\\"')}"` : a);
      const child = shell
        ? spawn([command, ...args].map(quote).join(" "), { stdio, shell: true })
        : spawn(command, args, { stdio });
      if (capture) {
        const keep = (d) => {
          output += d;
          if (output.length > 2_000_000) output = output.slice(-1_000_000);
        };
        child.stdout.on("data", (d) => (process.stdout.write(d), keep(d)));
        child.stderr.on("data", (d) => (process.stderr.write(d), keep(d)));
      }
      const forward = (sig) => child.kill(sig);
      process.on("SIGINT", forward);
      process.on("SIGTERM", forward);
      child.on("error", (e) => {
        // A .cmd shim (npm, npx) is not spawnable without a shell on Windows.
        if (e.code === "ENOENT" && !shell && process.platform === "win32") start(true);
        else {
          process.stderr.write(`with-lock: ${e.message}\n`);
          finish(127);
        }
      });
      child.on("close", (code, signal) => finish(code ?? (signal ? 1 : 0)));
    };
    start(false);
  });
}

/** True for `cargo [+toolchain] nextest list ...`: it runs no test, so it waits on no lock. */
export function isTestListing(command, args) {
  if (!/^cargo(?:\.exe)?$/i.test(basename(command))) return false;
  const rest = args[0]?.startsWith("+") ? args.slice(1) : args;
  return rest[0] === "nextest" && rest[1] === "list";
}

/**
 * The command-line wrapper, resolving to the exit code. `env`, `cwd` and `run` exist for tests:
 * `run(command, args, { capture })` stands in for spawning the command and resolves to
 * { code, output }.
 *
 * With RLX_SUITE_LEDGER set, a run of exactly `cargo nextest run --workspace` consults the suite
 * ledger (ADR-0207, lib/ledger.mjs): on a clean tree the ledger records green it prints one notice
 * naming the record and exits 0 without running, and otherwise it runs, recording the run when the
 * worktree was clean at both ends, under RLX_SUITE_LEDGER_BY as its writer. Any other argument
 * vector, and any run without the variable, neither skips nor records.
 */
export async function runWrapped(argv, { env = process.env, cwd = process.cwd(), run = spawnRun } = {}) {
  const sep = argv.indexOf("--");
  if (sep !== 1 || argv.length < 3) {
    process.stderr.write("usage: node with-lock.mjs <name> -- <command> [args...]\n");
    return 2;
  }
  const [name] = argv;
  const [command, ...args] = argv.slice(2);
  // A listing starts at once, takes no lock and writes no lock log entry.
  if (isTestListing(command, args)) return (await run(command, args, { capture: false })).code;

  const ledger = env.RLX_SUITE_LEDGER;
  const suite = Boolean(ledger) && isFullSuite(command, args);
  if (suite) {
    const green = greenRecord(ledger, cleanTree(cwd));
    if (green) {
      appendSkip(ledger, { green, by: env.RLX_SUITE_LEDGER_BY || "a session" });
      process.stdout.write(`with-lock: ${skipNotice(green)}\n`);
      return 0;
    }
  }

  const what = [command, ...args].join(" ");
  const lock = await acquire(name, {
    what,
    dir: lockDir(env),
    pollMs: Number(env.RLX_LOCK_POLL_MS || 500),
    onWait: (h) =>
      process.stderr.write(
        `with-lock: waiting for "${name}" (held by pid ${h?.pid ?? "?"}: ${h?.what ?? "unknown"})\n`,
      ),
  });
  const startTree = suite ? cleanTree(cwd) : null;
  const { code, output } = await run(command, args, { capture: suite });
  lock.release();
  const heldMs = Date.now() - lock.acquiredAt;
  if (suite && startTree && cleanTree(cwd) === startTree) {
    appendRecord(ledger, { tree: startTree, exit: code, summary: summaryLine(output), by: env.RLX_SUITE_LEDGER_BY || "a session", ms: heldMs });
  }
  // Read back by the run terminal's stream reader (lib/live.mjs lockTimes); keep the shape.
  process.stderr.write(`with-lock: "${name}" waited ${(lock.waitedMs / 1000).toFixed(1)}s, held ${(heldMs / 1000).toFixed(1)}s\n`);
  if (env.RLX_LOCK_LOG) {
    try {
      appendFileSync(
        env.RLX_LOCK_LOG,
        JSON.stringify({ lock: name, what, waited_ms: lock.waitedMs, held_ms: heldMs, exit_code: code, at: new Date().toISOString() }) + "\n",
      );
    } catch {}
  }
  return code;
}

const norm = (p) => (process.platform === "win32" ? resolve(p).toLowerCase() : resolve(p));
if (process.argv[1] && norm(fileURLToPath(import.meta.url)) === norm(process.argv[1])) {
  runWrapped(process.argv.slice(2)).then((code) => process.exit(code));
}
