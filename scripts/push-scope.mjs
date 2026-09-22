#!/usr/bin/env node
// Does a commit range touch a Rust-relevant path? ADR-0237.
//
// Usage:
//   node scripts/push-scope.mjs <base> <tip>    compare the two trees against RUST_PATHS
//   node scripts/push-scope.mjs --self-test     prove the answer on the seeded cases, in a
//                                               throwaway repository
//
// Exit 0 = YES, the range is Rust-relevant, and the first matching path is printed with the rule
// that matched it. Exit 3 = NO, nothing in the range is. Anything else is a failure to answer.
//
// THE ONLY "NO" IS EXIT 3, and that is deliberate: `.githooks/pre-push` skips its cargo steps on
// exactly that code, so a crash, a missing node or a usage error all read as "run everything".
// Every case this script cannot decide answers YES for the same reason — a shallow clone, a base
// or tip git cannot resolve, and two commits with no common ancestor.
//
// THE COMPARISON IS TREE TO TREE (`git diff <base> <tip>`), not the commits between them. The
// question is whether the tree being pushed differs from the tree already there in anything cargo
// reads, so a Rust change the push REMOVES — a force push over someone else's commit — counts as
// much as one it adds. Renames are split into a delete and an add so both sides are judged.

import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { RUST_PATHS } from "./push-scope.manifest.mjs";

const SCRIPT = fileURLToPath(import.meta.url);
const REPO_ROOT = resolve(dirname(SCRIPT), "..");
const FIXTURES = join(REPO_ROOT, "scripts", "fixtures", "push-scope");

/** The exit code that means "no Rust-relevant path"; every other code means run. */
export const NONE = 3;

/** `git` in `cwd` as `{ code, stdout }`, stdout trimmed. */
function git(cwd, args, env = process.env) {
  const r = spawnSync("git", args, { cwd, env, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] });
  return { code: r.status ?? 1, stdout: (r.stdout ?? "").trim() };
}

/**
 * One manifest pattern as an anchored regular expression over a repository-relative path. See the
 * manifest's header for the four forms; a pattern with no `/` matches at the root only.
 */
export function patternRegex(pattern) {
  let re = "";
  let i = 0;
  while (i < pattern.length) {
    if (pattern.startsWith("**/", i)) {
      re += "(?:[^/]+/)*";
      i += 3;
    } else if (pattern.startsWith("/**", i) && i + 3 === pattern.length) {
      re += "/.+";
      i += 3;
    } else if (pattern[i] === "*") {
      re += "[^/]*";
      i += 1;
    } else {
      re += pattern[i].replace(/[.+?^${}()|[\]\\]/g, "\\$&");
      i += 1;
    }
  }
  return new RegExp(`^${re}$`);
}

/** The first pattern in `patterns` that `path` matches, or null. */
export function matchingRule(path, patterns = RUST_PATHS) {
  const p = String(path).replace(/\\/g, "/");
  return patterns.find((pattern) => patternRegex(pattern).test(p)) ?? null;
}

/**
 * The answer for `base`..`tip` in the repository at `cwd`, as
 * `{ relevant, reason, path?, rule?, paths? }`. `relevant` is true whenever the answer is not a
 * proven NO.
 */
export function scopeOf(base, tip, cwd = REPO_ROOT, { env = process.env, patterns = RUST_PATHS } = {}) {
  const yes = (reason) => ({ relevant: true, reason });
  if (git(cwd, ["rev-parse", "--is-shallow-repository"], env).stdout === "true") {
    return yes("the clone is shallow, so the range cannot be read");
  }
  for (const rev of [base, tip]) {
    if (git(cwd, ["cat-file", "-e", `${rev}^{commit}`], env).code !== 0) {
      return yes(`${String(rev).slice(0, 12)} is not a commit this clone holds`);
    }
  }
  if (git(cwd, ["merge-base", base, tip], env).code !== 0) {
    return yes(`${short(base)} and ${short(tip)} have no common ancestor`);
  }
  const diff = git(cwd, ["diff", "--name-only", "--no-renames", base, tip], env);
  if (diff.code !== 0) return yes(`git diff ${short(base)} ${short(tip)} failed`);
  const paths = diff.stdout ? diff.stdout.split("\n").map((p) => p.trim()).filter(Boolean) : [];
  for (const path of paths) {
    const rule = matchingRule(path, patterns);
    if (rule) return { relevant: true, reason: `touches ${path} (rule ${rule})`, path, rule, paths };
  }
  return { relevant: false, reason: `touches no Rust-relevant path (${paths.length} changed)`, paths };
}

const short = (sha) => String(sha).slice(0, 7);

// ---------------------------------------------------------------------------
// --self-test
// ---------------------------------------------------------------------------

/**
 * Two halves, both read from `scripts/fixtures/push-scope/cases.json`: `paths` asserts the matcher
 * against the real manifest one path at a time, and `ranges` builds each range as a real commit in
 * a throwaway repository and asserts `scopeOf`'s answer — including the path it names, so a YES
 * for the wrong reason does not pass.
 *
 * Every GIT_* variable is dropped first: a git hook runs with GIT_DIR set, and inheriting it would
 * point every command here back at the real repository.
 */
function selfTest() {
  const env = Object.fromEntries(Object.entries(process.env).filter(([k]) => !k.startsWith("GIT_")));
  const cases = JSON.parse(readFileSync(join(FIXTURES, "cases.json"), "utf8"));
  const results = [];
  const check = (ok, name, detail) => results.push({ ok, name, detail });

  for (const { path, expect } of cases.paths) {
    const rule = matchingRule(path);
    check((rule !== null) === (expect === "match"), `path ${path} -> ${expect}`, `matched ${rule ?? "nothing"}`);
  }

  const dir = mkdtempSync(join(tmpdir(), "rlx-push-scope-"));
  try {
    const g = (...a) => {
      const r = git(dir, a, env);
      if (r.code !== 0) throw new Error(`git ${a.join(" ")} failed in the self-test repository`);
      return r.stdout;
    };
    const write = (rel, text) => {
      mkdirSync(dirname(join(dir, rel)), { recursive: true });
      writeFileSync(join(dir, rel), text);
    };
    g("init", "-q");
    g("config", "user.name", "self-test");
    g("config", "user.email", "self-test@example.invalid");
    g("config", "commit.gpgsign", "false");
    for (const rel of cases.base) write(rel, `base ${rel}\n`);
    g("add", "--", ...cases.base);
    g("commit", "-q", "-m", "base");
    const base = g("rev-parse", "HEAD");

    for (const c of cases.ranges) {
      g("checkout", "-q", "--detach", base);
      for (const rel of c.write ?? []) write(rel, `changed ${rel}\n`);
      for (const rel of c.delete ?? []) g("rm", "-q", "--", rel);
      if (c.write?.length) g("add", "--", ...c.write);
      g("commit", "-q", "--allow-empty", "-m", c.name);
      let tip = g("rev-parse", "HEAD");
      // A parentless commit with the same tree: the history a force push over an unrelated branch has.
      if (c.orphan) tip = g("commit-tree", "HEAD^{tree}", "-m", `${c.name} (orphan)`);
      const got = scopeOf(base, tip, dir, { env });
      const wantRelevant = c.expect !== "none";
      const pathOk = c.path === undefined || got.path === c.path;
      const reasonOk = c.reason === undefined || got.reason.includes(c.reason);
      check(got.relevant === wantRelevant && pathOk && reasonOk, `range ${c.name} -> ${c.expect}`, got.reason);
    }

    const bogus = scopeOf(base, "0".repeat(40), dir, { env });
    check(bogus.relevant && bogus.reason.includes("not a commit"), "range to an unknown tip -> match", bogus.reason);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }

  for (const r of results) {
    console.log(`  ${r.ok ? "ok  " : "FAIL"} ${r.name}`);
    if (!r.ok) console.log(`       ${r.detail}`);
  }
  const passed = results.filter((r) => r.ok).length;
  const expected = cases.paths.length + cases.ranges.length + 1;
  // The four the plan names must be present by name, so trimming the fixture cannot keep this green.
  const named = ["docs-only", "preset-only", "lockfile-only", "no-common-ancestor"];
  const missing = named.filter((n) => !cases.ranges.some((c) => c.name === n));
  if (missing.length) console.log(`  FAIL cases.json lacks the named range(s): ${missing.join(", ")}`);
  console.log(`push scope self-test: ${passed} of ${results.length}`);
  process.exit(passed === results.length && results.length === expected && missing.length === 0 ? 0 : 1);
}

// ---------------------------------------------------------------------------

const args = process.argv.slice(2);
if (args.includes("--self-test")) {
  selfTest();
} else if (args.length === 2 && !args.some((a) => a.startsWith("-"))) {
  const [base, tip] = args;
  const r = scopeOf(base, tip);
  const verdict = r.relevant ? "yes" : "no";
  console.log(`push-scope: ${short(base)}..${short(tip)} ${r.reason}: ${verdict}`);
  process.exit(r.relevant ? 0 : NONE);
} else {
  console.error("usage: node scripts/push-scope.mjs <base> <tip> | --self-test");
  process.exit(2);
}
