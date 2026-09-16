// The suite ledger, tools/conductor/state/suite-ledger.jsonl (ADR-0207): one JSON line per run of
// exactly `cargo nextest run --workspace` that conductor code observed, keyed by the tree it ran on.
//
//   { tree, cmd, exit, summary, by, at, ms }                     a run
//   { tree, cmd, skip: true, by, at, green: { by, at } }         a skip, and the run it relied on
//   { tree, cmd, served: true, exit, ..., green, diff }          a `-P fast` run a record served
//
// A run is recorded only when the worktree was clean both when it started and when it ended, so the
// tree hash names exactly what was tested. A lookup skips only when the newest run recorded for that
// tree has exit 0: a red run is recorded, and a tree whose latest run was red is never skipped on.
// Skip lines are the record of what was not re-run; a lookup never reads them.
//
// A served line is a `-P fast` run the conductor's gate made in the full suite's place because a
// green record for another tree served this one (ADR-0211). It carries `served: true` AND a `cmd`
// that is not `SUITE_COMMAND`, so neither lookup below can read it back as a full-suite green: one
// `-P fast` never chains off another.
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

/** What a served tree runs in the full suite's place: the suite command plus these, and nothing else. */
export const SERVED_SUITE_ARGS = ["-P", "fast"];
export const SERVED_COMMAND = `${SUITE_COMMAND} ${SERVED_SUITE_ARGS.join(" ")}`;

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
    .filter((e) => e.tree === tree && e.cmd === SUITE_COMMAND && !e.skip)
    .at(-1);
  return latest && latest.exit === 0 ? latest : null;
}

// ---------------------------------------------------------------------------------------------
// Serving a record forward (ADR-0211)

/**
 * The paths a green record may be served across, as data: a path is served only by being named
 * here, and anything unnamed falls through to the full suite. The direction is the safety surface —
 * a list that forgets a path is merely slow, while the same list written as a denylist would
 * under-gate a path type nobody thought of, silently.
 *
 * `versionLineOnly` files are served only when their whole diff is a `version = "x.y.z"` line, the
 * shape a release bump produces; a dependency edit in the same file is unserved.
 */
export const SERVED_PATHS = {
  dirs: ["docs/", ".claude/", "tools/", "site/", "studio/", "packaging/", "renders/"],
  suffixes: [".md"],
  versionLineOnly: ["Cargo.toml", "Cargo.lock"],
};

// A full three-part semver, so a two-part dependency requirement (`version = "0.20"`) is not one.
const VERSION_LINE = /^[+-]version = "\d+\.\d+\.\d+"$/;

/** The paths `git diff --name-only a b` reports, forward-slashed, or null when git refused. */
export function diffPaths(a, b, cwd) {
  // --no-renames so a rename is a delete plus an add: both sides must be served, not just the new one.
  const r = git(["diff", "--name-only", "--no-renames", a, b], cwd);
  if (r.code !== 0) return null;
  return r.stdout ? r.stdout.split("\n").map((p) => p.trim().replace(/\\/g, "/")).filter(Boolean) : [];
}

/** True when nothing but a `version = "x.y.z"` line changed in `file` between `a` and `b`. */
function versionLineOnly(file, cwd, a, b) {
  const r = git(["diff", "--unified=0", a, b, "--", file], cwd);
  if (r.code !== 0) return false;
  const body = r.stdout.split("\n").filter((l) => /^[+-]/.test(l) && !/^(\+\+\+|---)/.test(l));
  return body.length > 0 && body.every((l) => VERSION_LINE.test(l));
}

function servesPath(p, cwd, a, b) {
  const path = String(p).replace(/\\/g, "/");
  if (SERVED_PATHS.dirs.some((d) => path.startsWith(d))) return true;
  if (SERVED_PATHS.suffixes.some((s) => path.toLowerCase().endsWith(s))) return true;
  if (SERVED_PATHS.versionLineOnly.includes(basename(path))) return versionLineOnly(path, cwd, a, b);
  return false;
}

/** True when **every** path in `paths` is served, so trees `a` and `b` differ in nothing a deferred suite reads. */
export function servesDiff(paths, cwd, a, b) {
  return paths.every((p) => servesPath(p, cwd, a, b));
}

/** True when `git` in `cwd` can still resolve the object `sha` names. */
function resolves(sha, cwd) {
  return git(["cat-file", "-e", `${sha}^{tree}`], cwd).code === 0;
}

/**
 * The green record that serves `tree` without being `tree`'s own, as `{ record, paths }`, or null.
 * Consulted only after `greenRecord` returned null, and never a substitute for it: the exact tree is
 * skipped as a candidate, because its own diff is empty and a tree whose latest run was red would
 * otherwise serve itself.
 *
 * The walk is newest-first, one decision per tree — the newest run for a tree is the one that counts,
 * so a later red disqualifies it exactly as it does a skip. A record whose tree this worktree can no
 * longer resolve is passed over rather than fatal: `git gc` in another lane is not an error here.
 */
export function servingRecord(path, tree, cwd) {
  if (!tree) return null;
  const decided = new Set();
  for (const e of readLedger(path).reverse()) {
    if (e.skip || e.served || e.cmd !== SUITE_COMMAND) continue;
    if (decided.has(e.tree)) continue;
    decided.add(e.tree);
    if (e.exit !== 0 || e.tree === tree || !resolves(e.tree, cwd)) continue;
    const paths = diffPaths(e.tree, tree, cwd);
    if (paths && servesDiff(paths, cwd, e.tree, tree)) return { record: e, paths };
  }
  return null;
}

export function appendRecord(path, { tree, exit, summary, by, ms, at = new Date().toISOString() }) {
  mkdirSync(dirname(path), { recursive: true });
  appendFileSync(path, JSON.stringify({ tree, cmd: SUITE_COMMAND, exit, summary: summary ?? null, by, at, ms }) + "\n");
}

/** Records that `by` skipped a full suite on `green.tree`, relying on `green`. */
export function appendSkip(path, { green, by, at = new Date().toISOString() }) {
  mkdirSync(dirname(path), { recursive: true });
  appendFileSync(path, JSON.stringify({ tree: green.tree, cmd: SUITE_COMMAND, skip: true, by, at, green: { by: green.by, at: green.at } }) + "\n");
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
