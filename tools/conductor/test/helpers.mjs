// Shared scaffolding for the conductor tests: temp directories, plan documents in the shape
// the architect's template produces, and the path to the fake CLI.

import { mkdirSync, mkdtempSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const TEST_DIR = dirname(fileURLToPath(import.meta.url));
export const TOOL_DIR = resolve(TEST_DIR, "..");
export const REPO = resolve(TOOL_DIR, "..", "..");
export const FAKE_CLAUDE = join(TEST_DIR, "fake-claude.mjs");
export const FAKE = [process.execPath, FAKE_CLAUDE];

// Every temp directory a test makes lives under TMP_ROOT/<pid>, one root per test process
// (`node --test` runs each file in its own), and that root is removed when the process exits.
// A run that is killed never reaches the exit hook, so the next process to load this module
// sweeps every root whose PID is gone. Nothing may call mkdtempSync(tmpdir()) directly: on
// Windows the User Profile Service walks %TEMP% file by file at every logon, and a leaked
// fixture repo is ~80 files. RLX_KEEP_TMP=1 keeps the directories for debugging a failure.
export const TMP_ROOT = join(tmpdir(), "rlx-conductor-test");
const OWN_ROOT = join(TMP_ROOT, String(process.pid));
const KEEP = process.env.RLX_KEEP_TMP === "1";

function alive(pid) {
  try {
    process.kill(pid, 0);
    return true;
  } catch (e) {
    return e.code === "EPERM";
  }
}

function remove(dir) {
  // maxRetries: a just-exited child (git, the fake CLI) can hold a handle for a moment on Windows.
  try {
    rmSync(dir, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 });
  } catch {}
}

/** Removes every per-process root under `root` whose PID is no longer running. */
export function sweepDeadRoots(root = TMP_ROOT) {
  let entries = [];
  try {
    entries = readdirSync(root);
  } catch {
    return;
  }
  for (const name of entries) {
    const pid = Number(name);
    if (pid === process.pid) continue;
    if (!Number.isInteger(pid) || pid <= 0 || !alive(pid)) remove(join(root, name));
  }
}

if (!KEEP) {
  sweepDeadRoots();
  process.on("exit", () => remove(OWN_ROOT));
}

export function tmp(prefix = "rlx-conductor-test-") {
  mkdirSync(OWN_ROOT, { recursive: true });
  return mkdtempSync(join(OWN_ROOT, prefix));
}

/**
 * spec: { number, title?, status?, phases: [{ id, owner, title?, stop?, files?, blocksMerge? }],
 *         rows?: { [id]: { state, commit? } }, closeReview?: string, lane? }
 */
export function planText(spec) {
  const title = spec.title ?? `Plan ${spec.number} fixture`;
  const owners = [...new Set(spec.phases.map((p) => p.owner))].map((o) => `\`${o}\``).join(", ");
  const lines = [
    `# ${spec.number} — ${title}`,
    "",
    `> **Status:** ${spec.status ?? "approved (2026-09-14)"}`,
    "> **Created:** 2026-09-14",
    `> **Owner skill(s):** ${owners}`,
    "> **Closes:** none",
    "",
    "## TL;DR",
    "",
    "A fixture.",
    "",
    "## Implementation phases",
    "",
  ];
  for (const p of spec.phases) {
    lines.push(`### Phase ${p.id} — ${p.title ?? `Step ${p.id}`}`);
    lines.push(`- **Owner skill:** ${p.owner}`);
    lines.push(`- **What:** phase ${p.id}.`);
    lines.push(`- **Files touched:** \`phase-${p.id}.txt\`${p.files ? `, ${p.files}` : ""}`);
    lines.push(`- **Done when:** the file exists.`);
    if (p.stop) lines.push(`- **Stop condition:** ${p.stop}`);
    if (p.blocksMerge) lines.push(`- **Blocks merge:** ${p.blocksMerge}`);
    lines.push("");
  }
  lines.push("## Implementation log", "", `**Lane:** ${spec.lane ?? "_(unset)_"}`, "");
  lines.push("| phase | owner | state | commit |", "|---|---|---|---|");
  for (const p of spec.phases) {
    const row = spec.rows?.[p.id] ?? { state: "not started" };
    lines.push(`| ${p.id} — ${p.title ?? `Step ${p.id}`} | ${p.owner} | ${row.state} | ${row.commit ? `\`${row.commit}\`` : ""} |`);
  }
  lines.push("", "### Notes", "", "### Close triggers", "");
  if (spec.closeReview) lines.push("## Close review", "", spec.closeReview, "");
  lines.push("## Followups (after this lands)", "");
  return lines.join("\n");
}

export function slugFor(spec) {
  return `${spec.number}-fixture`;
}

export function writePlan(repo, spec, { done = false } = {}) {
  const dir = done ? join(repo, "docs", "plans", "done") : join(repo, "docs", "plans");
  mkdirSync(dir, { recursive: true });
  const path = join(dir, `${slugFor(spec)}.md`);
  writeFileSync(path, planText(spec));
  return path;
}

/**
 * A red nextest run, recorded verbatim through the suite wrapper's pipe on cargo-nextest 0.9.143: a
 * scratch crate with two tests made to fail. Each failure carries its `(n/total)` progress counter,
 * is printed as it happens with its captured output, and is printed again under the closing
 * `Summary`. Re-record it rather than edit it when nextest's shape moves.
 */
export const RED_NEXTEST_OUTPUT = [
  "   Compiling red-scratch v0.0.0 (/home/igor/Work/rlx-plan-0229/target/rlx-red-0229)",
  "    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.14s",
  "────────────",
  " Nextest run ID 7a59127d-7fd3-4f72-95e6-2b0c32b0ffbb with nextest profile: default",
  "    Starting 6 tests across 3 binaries",
  "        FAIL [   0.004s] (1/6) red-scratch::golden golden_rose_star",
  "  stdout ───",
  "",
  "    running 1 test",
  "    running the golden",
  "    test golden_rose_star ... FAILED",
  "",
  "    failures:",
  "",
  "    failures:",
  "        golden_rose_star",
  "",
  "    test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.00s",
  "",
  "  stderr ───",
  "",
  "    thread 'golden_rose_star' (227006) panicked at tests/golden.rs:4:5:",
  "    assertion `left == right` failed: golden drifted",
  "      left: 2",
  "     right: 3",
  "    note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace",
  "",
  "        PASS [   0.005s] (2/6) red-scratch tests::passes_one",
  "        PASS [   0.005s] (3/6) red-scratch tests::passes_two",
  "        PASS [   0.005s] (4/6) red-scratch::golden golden_steady",
  "        PASS [   0.005s] (5/6) red-scratch tests::passes_three",
  "        FAIL [   0.005s] (6/6) red-scratch::shot_cli the_count_column",
  "  stdout ───",
  "",
  "    running 1 test",
  "    test the_count_column ... FAILED",
  "",
  "    failures:",
  "",
  "    failures:",
  "        the_count_column",
  "",
  "    test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s",
  "",
  "  stderr ───",
  "",
  "    thread 'the_count_column' (227011) panicked at tests/shot_cli.rs:3:5:",
  "    count column missing",
  "    note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace",
  "",
  "────────────",
  "     Summary [   0.005s] 6 tests run: 4 passed, 2 failed, 0 skipped",
  "        FAIL [   0.004s] (1/6) red-scratch::golden golden_rose_star",
  "        FAIL [   0.005s] (6/6) red-scratch::shot_cli the_count_column",
  "error: test run failed",
  "",
].join("\n");

/**
 * A flaky pass, recorded verbatim on cargo-nextest 0.9.143: a scratch crate whose
 * `a_preset_datagram_selects_by_name` fails its first try and passes its second under a
 * `retries = 1` override, beside one steady test. The first try prints `TRY 1 FAIL`, the retry
 * `TRY 2 PASS`, the summary counts it `(1 flaky)`, and a closing `FLAKY 2/2` line names it.
 * Re-record it rather than edit it when nextest's shape moves.
 */
export const FLAKY_NEXTEST_OUTPUT = [
  "    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.01s",
  "────────────",
  " Nextest run ID 687ede7f-9c50-4459-98f5-8f57bd819cc8 with nextest profile: default",
  "    Starting 2 tests across 2 binaries",
  "        PASS [   0.005s] (1/2) flaky-scratch::control_loopback steady",
  "  TRY 1 FAIL [   0.005s] (───) flaky-scratch::control_loopback a_preset_datagram_selects_by_name",
  "  stdout ───",
  "",
  "    running 1 test",
  "    test a_preset_datagram_selects_by_name ... FAILED",
  "",
  "    failures:",
  "",
  "    failures:",
  "        a_preset_datagram_selects_by_name",
  "",
  "    test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.00s",
  "",
  "  stderr ───",
  "",
  "    thread 'a_preset_datagram_selects_by_name' (1878477) panicked at tests/control_loopback.rs:6:9:",
  "    datagram lost",
  "    note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace",
  "",
  "  TRY 2 PASS [   0.006s] (2/2) flaky-scratch::control_loopback a_preset_datagram_selects_by_name",
  "────────────",
  "     Summary [   0.014s] 2 tests run: 2 passed (1 flaky), 0 skipped",
  "   FLAKY 2/2 [   0.006s] (2/2) flaky-scratch::control_loopback a_preset_datagram_selects_by_name",
  "",
].join("\n");

/**
 * The same scratch crate on cargo-nextest 0.9.143 with the test failing both tries: each failure is
 * printed `TRY n FAIL`, and the closing summary repeats the last as `TRY 2 FAIL`, never as a bare
 * `FAIL`. Re-record it rather than edit it when nextest's shape moves.
 */
export const RETRIED_RED_NEXTEST_OUTPUT = [
  "   Compiling flaky-scratch v0.0.0 (/tmp/claude-1000/-home-igor-Work-Ritmolux/19835313-9815-4d90-8317-bac9b1d917c0/scratchpad/flaky-scratch)",
  "    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.11s",
  "────────────",
  " Nextest run ID da9e0676-8472-4194-9a8d-4844cba76524 with nextest profile: default",
  "    Starting 2 tests across 2 binaries",
  "        PASS [   0.006s] (1/2) flaky-scratch::control_loopback steady",
  "  TRY 1 FAIL [   0.006s] (───) flaky-scratch::control_loopback a_preset_datagram_selects_by_name",
  "  stdout ───",
  "",
  "    running 1 test",
  "    test a_preset_datagram_selects_by_name ... FAILED",
  "",
  "    failures:",
  "",
  "    failures:",
  "        a_preset_datagram_selects_by_name",
  "",
  "    test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.00s",
  "",
  "  stderr ───",
  "",
  "    thread 'a_preset_datagram_selects_by_name' (1878718) panicked at tests/control_loopback.rs:6:9:",
  "    datagram lost",
  "    note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace",
  "",
  "  TRY 2 FAIL [   0.005s] (2/2) flaky-scratch::control_loopback a_preset_datagram_selects_by_name",
  "  stdout ───",
  "",
  "    running 1 test",
  "    test a_preset_datagram_selects_by_name ... FAILED",
  "",
  "    failures:",
  "",
  "    failures:",
  "        a_preset_datagram_selects_by_name",
  "",
  "    test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.00s",
  "",
  "  stderr ───",
  "",
  "    thread 'a_preset_datagram_selects_by_name' (1878720) panicked at tests/control_loopback.rs:6:9:",
  "    datagram lost",
  "    note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace",
  "",
  "  Cancelling due to test failure: ",
  "────────────",
  "     Summary [   0.013s] 2 tests run: 1 passed, 1 failed, 0 skipped",
  "  TRY 2 FAIL [   0.005s] (2/2) flaky-scratch::control_loopback a_preset_datagram_selects_by_name",
  "error: test run failed",
  "",
].join("\n");

export function outcomeBlock(obj) {
  return "Done.\n\n```rlx-outcome\n" + JSON.stringify(obj) + "\n```\n";
}
