// The suite ledger's own rules: which argument vector it keys, which record a lookup trusts, and that
// a clean tree is the only tree it names.

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";

import {
  appendRecord,
  appendSkip,
  cleanTree,
  diffPaths,
  greenRecord,
  isFullSuite,
  readLedger,
  SERVED_PATHS,
  servesDiff,
  servingRecord,
  SUITE_COMMAND,
  skipNotice,
  summaryLine,
  treeOf,
} from "../lib/ledger.mjs";
import { defaultGate } from "../lib/gate.mjs";
import { tmp } from "./helpers.mjs";

function repo() {
  const dir = tmp("rlx-ledger-repo-");
  const sh = (...a) => assert.equal(spawnSync("git", a, { cwd: dir, encoding: "utf8" }).status, 0, a.join(" "));
  sh("init", "-q", "-b", "main");
  sh("config", "user.email", "t@example.invalid");
  sh("config", "user.name", "T");
  sh("config", "commit.gpgsign", "false");
  writeFileSync(join(dir, "a.txt"), "a\n");
  sh("add", "a.txt");
  sh("commit", "-q", "-m", "init");
  return dir;
}

/** A repository whose `commit` writes a set of files, commits them by path, and returns the new tree. */
function treeRepo() {
  const dir = tmp("rlx-ledger-trees-");
  const sh = (...a) => {
    const r = spawnSync("git", a, { cwd: dir, encoding: "utf8" });
    assert.equal(r.status, 0, `${a.join(" ")}: ${r.stderr}`);
  };
  sh("init", "-q", "-b", "main");
  sh("config", "user.email", "t@example.invalid");
  sh("config", "user.name", "T");
  sh("config", "commit.gpgsign", "false");
  sh("config", "core.autocrlf", "false");
  const commit = (files, message) => {
    for (const [p, body] of Object.entries(files)) {
      mkdirSync(dirname(join(dir, p)), { recursive: true });
      writeFileSync(join(dir, p), body);
      sh("add", p);
    }
    sh("commit", "-q", "-m", message);
    return treeOf(dir);
  };
  return { dir, sh, commit };
}

const CARGO_TOML = (v) => `[workspace.package]\nversion = "${v}"\nedition = "2021"\n\n[workspace.dependencies]\nwgpu = "=0.20.0"\n`;
const CARGO_LOCK = (v) => `version = 4\n\n[[package]]\nname = "rlx-core"\nversion = "${v}"\n\n[[package]]\nname = "wgpu"\nversion = "0.20.0"\nchecksum = "abc"\n`;

/** A ledger holding one green full-suite record for `tree`. */
function ledgerGreenFor(tree) {
  const file = join(tmp("rlx-ledger-serve-"), "suite-ledger.jsonl");
  appendRecord(file, { tree, exit: 0, summary: "1940 tests run: 1940 passed", by: "gate 0101-pre-review", ms: 737_000 });
  return file;
}

test("only the exact vector `cargo nextest run --workspace` is the full suite, and the gate marks exactly that step", () => {
  assert.equal(isFullSuite("cargo", ["nextest", "run", "--workspace"]), true);
  assert.equal(isFullSuite("cargo.exe", ["nextest", "run", "--workspace"]), true);
  assert.equal(isFullSuite("cargo", ["nextest", "run", "--workspace", "--no-fail-fast"]), false);
  assert.equal(isFullSuite("cargo", ["nextest", "run", "--workspace", "-P", "fast"]), false);
  assert.equal(isFullSuite("cargo", ["nextest", "run", "-p", "rlx-core"]), false);
  assert.equal(isFullSuite("cargo", ["test", "--workspace"]), false);
  const marked = defaultGate().filter((c) => c.ledger);
  assert.equal(marked.length, 1);
  assert.equal(marked[0].cmd.join(" "), SUITE_COMMAND);
  assert.equal(isFullSuite(marked[0].cmd[0], marked[0].cmd.slice(1)), true);
});

test("a lookup skips only on the newest record for that tree, and only when it is green", () => {
  const file = join(tmp(), "suite-ledger.jsonl");
  assert.equal(greenRecord(file, "t1"), null, "no ledger, no skip");
  appendRecord(file, { tree: "t1", exit: 0, summary: "3 tests run: 3 passed", by: "gate 0101-pre-review", ms: 10 });
  assert.equal(greenRecord(file, "t1").by, "gate 0101-pre-review");
  assert.equal(greenRecord(file, "t2"), null);
  assert.equal(greenRecord(file, null), null, "no tree (a dirty worktree) never skips");
  appendSkip(file, { green: greenRecord(file, "t1"), by: "gate 0101-post-close" });
  assert.equal(greenRecord(file, "t1").by, "gate 0101-pre-review", "a skip line is not a run");
  appendRecord(file, { tree: "t1", exit: 100, summary: "3 tests run: 2 passed, 1 failed", by: "0101-03-review", ms: 10 });
  assert.equal(greenRecord(file, "t1"), null, "a later red on the same tree is never skipped on");
  appendSkip(file, { green: { tree: "t1", by: "x", at: "y" }, by: "stray" });
  assert.equal(greenRecord(file, "t1"), null, "a skip after the red does not revive the green");
  writeFileSync(file, "not json\n", { flag: "a" });
  assert.equal(readLedger(file).length, 4, "a torn line is ignored");
  assert.match(skipNotice(readLedger(file)[0]), /^skipped cargo nextest run --workspace: tree t1 is green in the suite ledger, run by gate 0101-pre-review at \S+: 3 tests run: 3 passed$/);
});

test("a clean worktree names its tree and a dirty one names none", () => {
  const dir = repo();
  const tree = treeOf(dir);
  assert.match(tree, /^[0-9a-f]{40}$/);
  assert.equal(cleanTree(dir), tree);
  writeFileSync(join(dir, "untracked.txt"), "x\n");
  assert.equal(cleanTree(dir), null);
});

// ADR-0211: a green record serves a later tree when every path in the diff is on the declared list.
// The list is an allowlist, so each case below is one class of path that refuses a record by not
// being named — the strongest mechanical check there is on what a deferred suite could read.

test("one unserved path anywhere in the diff refuses the record, one case per class", () => {
  const cwd = tmp();
  const served = ["docs/plans/0191-a.md", "tools/conductor/lib/ledger.mjs"];
  for (const unserved of [
    "core/src/scene/attractor.rs",
    "core/src/render/shaders/composite.wgsl",
    "presets/rose_star.toml",
    "core/tests/goldens/attractor_1280x800.png",
    ".config/nextest.toml",
    "spike/whatever.txt",
  ]) {
    assert.equal(servesDiff([unserved], cwd, "A", "B"), false, unserved);
    assert.equal(servesDiff([...served, unserved], cwd, "A", "B"), false, `${unserved} beside served paths`);
  }
  // The same paths' own directories are unlisted, so nothing under them is served by accident.
  assert.deepEqual(
    SERVED_PATHS.dirs.filter((d) => ["core/", "standalone/", "presets/", "milkconv/", "rlx-ring/", ".config/", ".github/"].includes(d)),
    [],
  );
});

test("a diff of only served paths is served, over the shape a real close produces", () => {
  const cwd = tmp();
  const close = [
    "docs/plans/0191-a-green-tree-is-not-tested-four-times.md",
    "docs/plans/done/0191-a-green-tree-is-not-tested-four-times.md",
    "docs/plans/README.md",
    "docs/adrs/README.md",
    "tools/conductor/queue.json",
    "presets/README.md",
  ];
  assert.equal(servesDiff(close, cwd, "A", "B"), true);
  assert.equal(servesDiff([], cwd, "A", "B"), true, "two identical trees");
  // Every directory the list names, and a `*.md` outside all of them.
  for (const d of SERVED_PATHS.dirs) assert.equal(servesDiff([`${d}some/file.txt`], cwd, "A", "B"), true, d);
  assert.equal(servesDiff(["README.md"], cwd, "A", "B"), true);
});

/** Trees A and B of a fresh repository, where B is A plus `files`, and the paths between them. */
function cargoCase(files) {
  const { dir, commit } = treeRepo();
  const a = commit({ "Cargo.toml": CARGO_TOML("0.124.0"), "Cargo.lock": CARGO_LOCK("0.124.0"), "README.md": "r\n" }, "init");
  const b = commit(files, "next");
  return { dir, a, b, paths: diffPaths(a, b, dir) };
}

test("Cargo.toml and Cargo.lock are served for a version line and nothing else", () => {
  const bump = cargoCase({ "Cargo.toml": CARGO_TOML("0.124.1"), "Cargo.lock": CARGO_LOCK("0.124.1") });
  assert.deepEqual(bump.paths, ["Cargo.lock", "Cargo.toml"]);
  assert.equal(servesDiff(bump.paths, bump.dir, bump.a, bump.b), true, "the shape a release bump produces");

  const dep = cargoCase({ "Cargo.toml": CARGO_TOML("0.124.0").replace('wgpu = "=0.20.0"', 'wgpu = "=0.21.0"') });
  assert.deepEqual(dep.paths, ["Cargo.toml"]);
  assert.equal(servesDiff(dep.paths, dep.dir, dep.a, dep.b), false, "a dependency requirement is not a version line");

  const locked = cargoCase({ "Cargo.lock": CARGO_LOCK("0.124.0").replace('checksum = "abc"', 'checksum = "def"') });
  assert.deepEqual(locked.paths, ["Cargo.lock"]);
  assert.equal(servesDiff(locked.paths, locked.dir, locked.a, locked.b), false, "a checksum is not a version line");

  // A two-part requirement under a dependency table is the trap the three-part pattern closes.
  const table = cargoCase({ "Cargo.toml": `${CARGO_TOML("0.124.0")}\n[workspace.dependencies.naga]\nversion = "0.20.1"\n` });
  assert.equal(servesDiff(table.paths, table.dir, table.a, table.b), false, "an added dependency table carries added lines");

  // Both files bumped plus one unserved path: the whole diff refuses.
  const mixed = cargoCase({ "Cargo.toml": CARGO_TOML("0.124.1"), "core/src/lib.rs": "fn main() {}\n" });
  assert.equal(servesDiff(mixed.paths, mixed.dir, mixed.a, mixed.b), false);
});

test("the forward lookup takes the newest resolvable green record whose diff serves, and never the exact tree's own", () => {
  const { dir, commit } = treeRepo();
  const a = commit({ "README.md": "r\n", "core/src/lib.rs": "fn a() {}\n" }, "init");
  const docsOnly = commit({ "docs/plans/0191-a.md": "plan\n", "tools/conductor/queue.json": "{}\n" }, "docs: a close's shape");
  const withCode = commit({ "core/src/lib.rs": "fn b() {}\n" }, "feat: one rs file");

  const file = ledgerGreenFor(a);
  const serving = servingRecord(file, docsOnly, dir);
  assert.equal(serving.record.tree, a);
  assert.equal(serving.record.by, "gate 0101-pre-review");
  assert.deepEqual(serving.paths, ["docs/plans/0191-a.md", "tools/conductor/queue.json"]);
  assert.equal(servingRecord(file, withCode, dir), null, "one .rs in the diff runs the full suite");
  assert.equal(servingRecord(file, a, dir), null, "a tree is never served by its own record");
  assert.equal(servingRecord(file, null, dir), null, "a dirty worktree names no tree");

  // A later red on the serving tree disqualifies it, exactly as it does a skip.
  appendRecord(file, { tree: a, exit: 100, summary: "1940 tests run: 1939 passed, 1 failed", by: "gate 0101-fix-1", ms: 10 });
  assert.equal(servingRecord(file, docsOnly, dir), null);
  appendRecord(file, { tree: a, exit: 0, summary: "1940 tests run: 1940 passed", by: "gate 0101-fix-2", ms: 10 });
  assert.equal(servingRecord(file, docsOnly, dir).record.by, "gate 0101-fix-2", "the newest run for that tree decides");
});

test("a green record for a tree this worktree cannot resolve is passed over, not fatal", () => {
  const { dir, commit } = treeRepo();
  const a = commit({ "README.md": "r\n" }, "init");
  const b = commit({ "docs/a.md": "a\n" }, "docs: one file");
  const gone = "0".repeat(40);
  const file = ledgerGreenFor(gone);
  assert.equal(servingRecord(file, b, dir), null);
  // The unresolvable record does not stop a resolvable one behind it from serving.
  appendRecord(file, { tree: a, exit: 0, summary: "3 tests run: 3 passed", by: "gate 0101-pre-review", ms: 10 });
  assert.equal(servingRecord(file, b, dir).record.tree, a);
});

test("the summary is nextest's last Summary line", () => {
  assert.equal(summaryLine("     Summary [   1.0s] 1 test run: 1 passed\n     Summary [ 652.000s] 1940 tests run: 1940 passed, 6 skipped\n"), "1940 tests run: 1940 passed, 6 skipped");
  assert.equal(summaryLine("error: no tests"), null);
});
