// The conductor's own gate, run in a worktree after the implementer runs, after every fix round, on
// the close tip and after a re-merge. It does not trust any session's claim that the checks passed.
//
// Which commands run depends on the stage (`gateForStage`). A step marked `afterClose` runs only on a
// tree a close produced: the backlog probes, whose red is a judgement ADR-0108 gives to the architect's
// close, which may archive the very entry an implement commit delivered.
//
// Commands run in order and stop at the first failure. Each one's output is kept under
// state/gates/. `nextest` runs under the machine-wide suite lock. The gate never retries a red: a
// flake is a defect to fix (ADR-0193), and a retry would bury it.
//
// A step marked `ledger` is the full workspace suite, and it resolves to one of three states
// (ADR-0211, lib/ledger.mjs). A green record for this exact tree `skipped`s it and nothing runs
// (ADR-0207). Otherwise a green record for another tree whose diff is entirely served paths makes it
// `served`: `-P fast` runs in the full suite's place, under the same lock, and its record is a
// served line rather than a green one. Neither, and it `ran` — the full suite. Only one entry may
// carry the mark: `cargo nextest run --workspace`, the command the ledger's key names.

import { spawn, spawnSync } from "node:child_process";
import { existsSync, mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";

import { gatesFor } from "../../../scripts/gates.manifest.mjs";
import { appendRecord, appendServed, appendSkip, cleanTree, greenRecord, SERVED_SUITE_ARGS, servingRecord, summaryLine } from "./ledger.mjs";
import { SUITE, withLock } from "./locks.mjs";

/**
 * The one Node gate whose red is a judgement rather than a defect: a plan can deliver exactly what a
 * live backlog entry's probe says is missing, and archiving that entry is the close's job (ADR-0108).
 * The mark is this gate's stage semantics and not a roster fact, so it lives here rather than in the
 * manifest.
 */
const AFTER_CLOSE_SCRIPTS = new Set(["check-backlog-claims.mjs"]);

/** The manifest's `conductor` projection, as gate steps in roster order (ADR-0217). */
function nodeGates() {
  return gatesFor("conductor").map(({ script, args }) => ({
    name: `${script} ${args.join(" ")}`.trim(),
    cmd: ["node", `scripts/${script}`, ...args],
    ...(AFTER_CLOSE_SCRIPTS.has(script) ? { afterClose: true } : {}),
  }));
}

/**
 * What the pre-push hook runs, at full strength: the whole suite rather than `-P fast`, plus
 * `cargo doc` and the conductor's own tests, which CI runs and the hook does not. A step with
 * `onlyIf` skips when that path is absent from the worktree; one with `onlyIfCommand` skips when that
 * command does not run, as the hook skips the diffusion-filter suite with no python3 on PATH.
 *
 * The Node block is the manifest's projection for this carrier, read at run time rather than copied,
 * which is what stops it falling behind the hook and CI (ADR-0217).
 */
export function defaultGate() {
  return [
    ...nodeGates(),
    { name: "conductor tests", cmd: ["node", "--test", "tools/conductor/test/*.test.mjs"] },
    { name: "sd-filter tests", cmd: ["python3", "tools/sd-filter/test_sd_filter.py"], onlyIfCommand: ["python3", "--version"] },
    { name: "studio typecheck", cmd: ["npm", "--prefix", "studio", "run", "typecheck"], onlyIf: "studio/node_modules" },
    { name: "studio lint", cmd: ["npm", "--prefix", "studio", "run", "lint"], onlyIf: "studio/node_modules" },
    { name: "studio test", cmd: ["npm", "--prefix", "studio", "test"], onlyIf: "studio/node_modules" },
    { name: "cargo fmt", cmd: ["cargo", "fmt", "--all", "--check"] },
    { name: "cargo clippy", cmd: ["cargo", "clippy", "--workspace", "--all-targets", "--", "-D", "warnings"] },
    { name: "cargo nextest", cmd: ["cargo", "nextest", "run", "--workspace"], lock: SUITE, ledger: true },
    { name: "cargo doc", cmd: ["cargo", "doc", "--workspace", "--no-deps"], env: { RUSTDOCFLAGS: "-D warnings" } },
  ];
}

/** The stages whose tree a close produced. Every other stage is `pre-review` or `fix-N`. */
export const AFTER_CLOSE_STAGES = new Set(["post-close", "remerge"]);

/** The commands the gate runs at `stage`: `afterClose` steps drop out before a close. */
export function gateForStage(stage, commands = defaultGate()) {
  return AFTER_CLOSE_STAGES.has(stage) ? commands : commands.filter((c) => !c.afterClose);
}

function runCommand(cmd, cwd, env) {
  return new Promise((done) => {
    const [bin, ...args] = cmd;
    let output = "";
    const collect = (d) => {
      output += d;
      if (output.length > 2_000_000) output = output.slice(-1_000_000);
    };
    const start = (shell) => {
      const quote = (a) => (/[\s"]/.test(a) ? `"${a.replace(/"/g, '\\"')}"` : a);
      const child = shell
        ? spawn(cmd.map(quote).join(" "), { cwd, env: { ...process.env, ...env }, shell: true })
        : spawn(bin, args, { cwd, env: { ...process.env, ...env } });
      child.stdout.on("data", collect);
      child.stderr.on("data", collect);
      // Node emits `close` (code -4058 on Windows) after `error` for a child that never started, so a
      // child handed to the shell retry must not settle the result: the retried child does.
      let retried = false;
      child.on("error", (e) => {
        // A .cmd shim (npm, npx) is not spawnable without a shell on Windows.
        if (e.code === "ENOENT" && !shell && process.platform === "win32") {
          retried = true;
          start(true);
        } else done({ code: 127, output: output + `\n${e.message}` });
      });
      child.on("close", (code) => {
        if (!retried) done({ code: code ?? 1, output });
      });
    };
    start(false);
  });
}

/** nextest's per-test failure lines, e.g. `        FAIL [   1.234s] rlx-core::golden name`. */
export function failingTests(output) {
  return [...output.matchAll(/^\s*(?:FAIL|TIMEOUT|SIGSEGV|SIGABRT) \[[^\]]*\]\s+(.+?)\s*$/gm)].map((m) => m[1]);
}

/**
 * Runs the gate. Resolves to
 *   { ok, ran: [names], commands: [{ name, code, ms, suite?, skipped?, served?, by? }],
 *     failed?: { name, code, log, tail, tests } }.
 * `onLockWait(name, ms)` reports time spent waiting on a lock. `onCommandStart(command)` and
 * `onCommandEnd(command, { code, ms, output })` bracket each command that runs; `ms` excludes the
 * lock wait. `ledger` is the suite ledger's path; without it a `ledger` step always runs and nothing
 * is recorded. `onCommandSkipped(command, record)` reports a step the ledger skipped, which is not
 * in `ran`; `onCommandServed(command, serving)` reports one a record served down to `-P fast`,
 * which does run and is in `ran`.
 */
export async function runGate({
  cwd,
  commands = defaultGate(),
  logDir,
  label,
  lockDir,
  lockPollMs,
  onLockWait,
  onCommandStart,
  onCommandEnd,
  ledger,
  onCommandSkipped,
  onCommandServed,
}) {
  mkdirSync(logDir, { recursive: true });
  const ran = [];
  const timed = [];
  for (const [i, c] of commands.entries()) {
    if (c.onlyIf && !existsSync(join(cwd, c.onlyIf))) continue;
    if (c.onlyIfCommand && spawnSync(c.onlyIfCommand[0], c.onlyIfCommand.slice(1), { cwd, stdio: "ignore" }).status !== 0) continue;
    const suite = Boolean(ledger && c.ledger);
    let serving = null;
    if (suite) {
      const tree = cleanTree(cwd);
      const green = greenRecord(ledger, tree);
      if (green) {
        appendSkip(ledger, { green, by: `gate ${label}` });
        timed.push({ name: c.name, code: 0, ms: 0, suite: true, skipped: true, by: green.by });
        onCommandSkipped?.(c, green);
        continue;
      }
      serving = servingRecord(ledger, tree, cwd);
      if (serving) onCommandServed?.(c, serving);
    }
    // A served step runs the same command with `-P fast` appended, so the vector the ledger keys is
    // never what ran; everything downstream — the lock, the log, the failure extraction — is shared.
    const cmd = serving ? [...c.cmd, ...SERVED_SUITE_ARGS] : c.cmd;
    let t0 = Date.now();
    let startTree = null;
    const exec = () => {
      t0 = Date.now();
      // Read under the lock, as the run starts: the tree the record will name.
      if (suite) startTree = cleanTree(cwd);
      onCommandStart?.(c);
      return runCommand(cmd, cwd, c.env ?? {});
    };
    const r = c.lock
      ? await withLock(
          c.lock,
          { dir: lockDir, pollMs: lockPollMs, what: `gate ${label}: ${c.name}`, onWaited: (ms) => onLockWait?.(c.lock, ms) },
          exec,
        )
      : await exec();
    const ms = Date.now() - t0;
    const log = join(logDir, `${label}-${String(i).padStart(2, "0")}-${c.name.replace(/[^\w.-]+/g, "_")}.log`);
    writeFileSync(log, r.output);
    ran.push(c.name);
    timed.push({ name: c.name, code: r.code, ms, ...(suite ? { suite: true } : {}), ...(serving ? { served: true, by: serving.record.by } : {}) });
    if (suite && startTree && cleanTree(cwd) === startTree) {
      const record = { tree: startTree, exit: r.code, summary: summaryLine(r.output), by: `gate ${label}`, ms };
      if (serving) appendServed(ledger, { ...record, green: serving.record, paths: serving.paths });
      else appendRecord(ledger, record);
    }
    onCommandEnd?.(c, { code: r.code, ms, output: r.output });
    if (r.code !== 0) {
      return {
        ok: false,
        ran,
        commands: timed,
        failed: { name: c.name, code: r.code, log, tail: r.output.trim().split("\n").slice(-15).join("\n"), tests: failingTests(r.output) },
      };
    }
  }
  return { ok: true, ran, commands: timed };
}
