// The suite ledger, tools/conductor/state/suite-ledger.jsonl (ADR-0207): one JSON line per run of
// exactly `cargo nextest run --workspace` that conductor code observed, keyed by the tree it ran on.
//
//   { tree, cmd, exit, summary, by, at, ms }
//
// A run is recorded only when the worktree was clean both when it started and when it ended, so the
// tree hash names exactly what was tested. A lookup skips only when the newest record for that tree
// has exit 0: a red run is recorded, and a tree whose latest run was red is never skipped on.
//
// Two writers and two readers: the conductor's gate, and the suite wrapper in a conductor-run session
// (RLX_SUITE_LEDGER names the file). Nothing else reads or writes it.
//
// The key leaves out everything outside the tree: gitignored files, the GPU adapter, the installed
// toolchain. ADR-0207 records that risk rather than guarding it.

import { appendFileSync, existsSync, mkdirSync, readFileSync } from "node:fs";
import { basename, dirname } from "node:path";

import { git, isClean } from "./git.mjs";

export const SUITE_COMMAND = "cargo nextest run --workspace";

/** True only for the exact argument vector `cargo nextest run --workspace`; any extra argument is another run. */
export function isFullSuite(command, args) {
  return /^cargo(?:\.exe)?$/i.test(basename(String(command ?? ""))) && args.length === 3 && args[0] === "nextest" && args[1] === "run" && args[2] === "--workspace";
}

/** `HEAD^{tree}` of `cwd`, or null. */
export function treeOf(cwd) {
  const r = git(["rev-parse", "HEAD^{tree}"], cwd);
  return r.code === 0 && r.stdout ? r.stdout : null;
}

/** The tree `cwd` is at when it is clean, or null when it is dirty or not a repository. */
export function cleanTree(cwd) {
  return isClean(cwd) ? treeOf(cwd) : null;
}

export function readLedger(path) {
  if (!path || !existsSync(path)) return [];
  return readFileSync(path, "utf8")
    .split("\n")
    .filter(Boolean)
    .map((l) => {
      try {
        return JSON.parse(l);
      } catch {
        return null;
      }
    })
    .filter((e) => e && typeof e.tree === "string");
}

/** The green record a full suite on `tree` may skip on, or null. */
export function greenRecord(path, tree) {
  if (!tree) return null;
  const latest = readLedger(path)
    .filter((e) => e.tree === tree && e.cmd === SUITE_COMMAND)
    .at(-1);
  return latest && latest.exit === 0 ? latest : null;
}

export function appendRecord(path, { tree, exit, summary, by, ms, at = new Date().toISOString() }) {
  mkdirSync(dirname(path), { recursive: true });
  appendFileSync(path, JSON.stringify({ tree, cmd: SUITE_COMMAND, exit, summary: summary ?? null, by, at, ms }) + "\n");
}

/** nextest's last `Summary` line after its bracketed time, e.g. `1940 tests run: 1940 passed, 6 skipped`. */
export function summaryLine(output) {
  const m = [...String(output ?? "").matchAll(/^\s*Summary \[[^\]]*\]\s+(.+?)\s*$/gm)].at(-1);
  return m ? m[1] : null;
}

/** The one-line notice a skip prints: the record's tree, writer, time and summary. */
export function skipNotice(record) {
  return `skipped ${SUITE_COMMAND}: tree ${record.tree.slice(0, 7)} is green in the suite ledger, run by ${record.by} at ${record.at}: ${record.summary ?? "no summary"}`;
}
