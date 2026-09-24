#!/usr/bin/env node
// Read the newest conclusion of the `CI` workflow for `main` on `origin`, through `gh`, before a close
// merges onto it. ADR-0251.
//
// A close cannot read the commit it tags: the tag is written before the push, and CI runs only after
// it. What a close CAN read is the tip it merges onto, and a red one there means a job on `main` has
// been failing with nothing stopping the next release. This script is that reading.
//
// Usage:
//   node scripts/check-upstream-ci.mjs [root]      read it; `root` is the checkout whose `origin` names
//                                                  the repository (default: this one)
//   node scripts/check-upstream-ci.mjs --json      the same reading as one JSON object on stdout
//   node scripts/check-upstream-ci.mjs --self-test every case below against a fake `gh`
//
// Three outcomes, and each is spelled differently so no reader can confuse two of them:
//
//   GREEN   exit 0, stdout `upstream CI: OK - run <id> ...`
//   RED     exit 1, stderr `upstream CI: RED - run <id> ...` naming every failing JOB, not only the run
//   UNREAD  exit 0, stderr `upstream CI: skipped: not read (<case>) - ...`, in ADR-0016's shape. The
//           cases: no `origin` remote, an origin that is not GitHub, `gh` absent, `gh`
//           unauthenticated, no network, no completed CI run on main, and any other `gh` failure.
//           Not reading is never a pass and never a refusal: the close proceeds and the line says so.
//
// ONLY THE `CI` WORKFLOW (ci.yml). `Pages` and, on a tag, `Release` run on the same push; "the latest
// run" would read whichever finished last, and a red `Pages` would stop a close. The query names the
// workflow, and the result is filtered by name as well, so a `gh` that ignored the filter still could
// not hand back another workflow's run.
//
// A CANCELLED OR SKIPPED RUN IS PASSED OVER, not read as red: a push that lands while CI is running
// cancels nothing here, but a manually cancelled run says nothing about the tree. The newest run that
// concluded anything else is the reading.
//
// NOT A PRE-PUSH OR CI GATE, and not in scripts/gates.manifest.mjs: it needs the network and its answer
// changes without a commit, which is why ADR-0033 keeps `cargo deny` out of the hook too. The
// conductor's close imports `readUpstream`; a human-started close runs this by hand.
//
// RLX_GH names the `gh` to run, for the self-test and the conductor's suite: a path ending in `.mjs`
// or `.js` runs under this Node, anything else is spawned as it is.

import { execFileSync, spawnSync } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const SCRIPT = fileURLToPath(import.meta.url);
const REPO_ROOT = resolve(dirname(SCRIPT), "..");
const FIXTURES = join(REPO_ROOT, "scripts", "fixtures", "upstream-ci");

/** The workflow file and the name it declares; both are matched. */
export const WORKFLOW_FILE = "ci.yml";
export const WORKFLOW_NAME = "CI";
export const BRANCH = "main";

/** Runs whose conclusion says nothing about the tree, and are passed over for an older one. */
const PASSED_OVER = new Set(["cancelled", "skipped"]);
/** Job conclusions that are not a failure. */
const JOB_OK = new Set(["success", "skipped", "neutral"]);
/** How many completed runs are asked for, so a streak of cancelled ones does not read as none. */
const RUN_WINDOW = 20;

/** The `gh` command prefix: RLX_GH when set, run under Node when it is a script. */
function ghCommand(env) {
  const g = env.RLX_GH;
  if (!g) return ["gh"];
  return /\.m?js$/.test(g) ? [process.execPath, g] : [g];
}

/** `owner/name` from a GitHub remote URL in any of its spellings, or null. */
export function githubSlug(url) {
  const m = String(url).trim().match(/github\.com[:/]+([^/\s]+)\/([^/\s]+?)(?:\.git)?\/?$/);
  return m ? `${m[1]}/${m[2]}` : null;
}

/**
 * Why a failed `gh` call could not read, as one of the named cases. The exit code 4 is `gh`'s own
 * "authentication required"; the text is matched as a second route because a proxy or an older `gh`
 * reports the same thing with a different code.
 */
function ghFailureCase(r) {
  const text = `${r.stderr ?? ""}${r.stdout ?? ""}`;
  if (r.status === 4 || /gh auth login|authentication|not logged in|GH_TOKEN|Bad credentials|HTTP 401/i.test(text)) return "gh unauthenticated";
  if (/error connecting|could not resolve|no such host|dial tcp|network is unreachable|timed? ?out|ENOTFOUND|ECONNREFUSED|EAI_AGAIN/i.test(text)) {
    return "no network";
  }
  return "gh failed";
}

const firstLine = (s) => String(s ?? "").trim().split(/\r?\n/)[0] ?? "";

/**
 * The reading. Returns one of:
 *   { state: "green", run, sha, url }
 *   { state: "red",   run, sha, url, conclusion, jobs: [names] }   `jobs` empty when they could not be listed
 *   { state: "unread", case, detail }
 * and never throws.
 */
export function readUpstream({ cwd = REPO_ROOT, env = process.env } = {}) {
  const unread = (c, detail) => ({ state: "unread", case: c, detail });

  const remote = spawnSync("git", ["remote", "get-url", "origin"], { cwd, env, encoding: "utf8" });
  if (remote.error || remote.status !== 0) return unread("no origin remote", firstLine(remote.stderr) || "git remote get-url origin failed");
  const slug = githubSlug(remote.stdout);
  if (!slug) return unread("origin is not GitHub", `origin is ${remote.stdout.trim()}`);

  const [bin, ...pre] = ghCommand(env);
  const gh = (args) => spawnSync(bin, [...pre, ...args], { cwd, env, encoding: "utf8" });

  const list = gh([
    "run", "list",
    "--repo", slug,
    "--workflow", WORKFLOW_FILE,
    "--branch", BRANCH,
    "--event", "push",
    "--status", "completed",
    "--limit", String(RUN_WINDOW),
    "--json", "databaseId,conclusion,headSha,url,workflowName",
  ]);
  if (list.error?.code === "ENOENT") return unread("gh absent", `\`${bin}\` is not on PATH`);
  if (list.error) return unread("gh failed", list.error.message);
  if (list.status !== 0) return unread(ghFailureCase(list), firstLine(list.stderr) || `gh run list exited ${list.status}`);

  let runs;
  try {
    runs = JSON.parse(list.stdout || "[]");
  } catch {
    return unread("gh failed", "gh run list did not print JSON");
  }
  const run = (Array.isArray(runs) ? runs : [])
    .filter((r) => r.workflowName === WORKFLOW_NAME)
    .find((r) => !PASSED_OVER.has(r.conclusion));
  if (!run) return unread("no completed CI run", `${slug} has no completed ${WORKFLOW_NAME} run on ${BRANCH} to read`);

  const base = { run: run.databaseId, sha: run.headSha, url: run.url };
  if (run.conclusion === "success") return { state: "green", ...base };

  const view = gh(["run", "view", String(run.databaseId), "--repo", slug, "--json", "jobs"]);
  let jobs = [];
  let jobsError = null;
  if (view.error || view.status !== 0) jobsError = firstLine(view.stderr) || view.error?.message || `gh run view exited ${view.status}`;
  else {
    try {
      jobs = (JSON.parse(view.stdout).jobs ?? []).filter((j) => !JOB_OK.has(j.conclusion)).map((j) => j.name);
    } catch {
      jobsError = "gh run view did not print JSON";
    }
  }
  return { state: "red", ...base, conclusion: run.conclusion, jobs, ...(jobsError ? { jobsError } : {}) };
}

/** The one-line report for a reading, and the stream it belongs on. */
export function describe(r) {
  const at = (x) => `run ${x.run} (${WORKFLOW_NAME} on ${BRANCH} at ${String(x.sha ?? "").slice(0, 7)})`;
  switch (r.state) {
    case "green":
      return { stream: "stdout", text: `upstream CI: OK - ${at(r)} concluded success` };
    case "red": {
      const jobs = r.jobs.length ? `failing job(s): ${r.jobs.join(", ")}` : `failing job(s) not listed: ${r.jobsError ?? "none reported a failure"}`;
      return {
        stream: "stderr",
        text:
          `upstream CI: RED - ${at(r)} concluded ${r.conclusion}; ${jobs}\n` +
          `  ${r.url}\n` +
          `A close reports this and still merges (ADR-0251): repair ${BRANCH} and push; the digest line clears when a later close reads it green.`,
      };
    }
    default:
      return {
        stream: "stderr",
        text: `upstream CI: skipped: not read (${r.case}) - ${r.detail}. Nothing was checked; this is not a green reading.`,
      };
  }
}

/** The shorthand a live line or the digest carries: the failing jobs, or the run when none were listed. */
export function redSubject(r) {
  return r.jobs?.length ? r.jobs.join(", ") : `run ${r.run}`;
}

// ---------------------------------------------------------------------------
// --self-test
// ---------------------------------------------------------------------------

/**
 * Each case runs this script as a child, against a throwaway repository whose `origin` is a GitHub
 * URL (or absent), with RLX_GH naming the fixture's fake `gh` and RLX_FAKE_GH its scenario. The exit
 * code AND a phrase are asserted, plus a phrase that must NOT appear, so an unread case that printed
 * `OK` - the one confusion this script exists to prevent - fails.
 *
 * Every GIT_* variable is dropped first: a git hook runs with GIT_DIR set, and inheriting it would
 * point `git remote` back at the real repository.
 */
function selfTest() {
  const base = Object.fromEntries(Object.entries(process.env).filter(([k]) => !k.startsWith("GIT_") && !k.startsWith("RLX_")));
  const withOrigin = mkdtempSync(join(tmpdir(), "rlx-upstream-ci-"));
  const noOrigin = mkdtempSync(join(tmpdir(), "rlx-upstream-ci-bare-"));
  const results = [];
  try {
    for (const dir of [withOrigin, noOrigin]) execFileSync("git", ["init", "-q"], { cwd: dir, env: base });
    execFileSync("git", ["remote", "add", "origin", "https://github.com/example/ritmolux.git"], { cwd: withOrigin, env: base });

    const fake = join(FIXTURES, "fake-gh.mjs");
    const run = (name, { dir = withOrigin, scenario = null, gh = fake, status, has, lacks = [] }) => {
      const env = { ...base, RLX_GH: gh, ...(scenario ? { RLX_FAKE_GH: join(FIXTURES, `${scenario}.json`) } : {}) };
      const r = spawnSync(process.execPath, [SCRIPT, dir], { env, encoding: "utf8" });
      const out = `${r.stdout}${r.stderr}`;
      const ok = r.status === status && has.every((p) => out.includes(p)) && !lacks.some((p) => out.includes(p));
      results.push({ ok, name, detail: `exit ${r.status}, expected ${status}; needs ${JSON.stringify(has)}, must lack ${JSON.stringify(lacks)}`, out });
    };
    const unreadLacks = ["upstream CI: OK", "RED"];

    run("green -> exit 0, names the run", { scenario: "green", status: 0, has: ["upstream CI: OK - run 1001"] });
    run("red -> exit 1, names the failing job", { scenario: "red", status: 1, has: ["upstream CI: RED - run 2001", "check (macos-latest)"], lacks: ["deny", "links"] });
    run("a red Pages run newer than a green CI run -> exit 0", { scenario: "pages-red", status: 0, has: ["upstream CI: OK - run 3001"] });
    run("a cancelled newest CI run is passed over", { scenario: "cancelled-newest", status: 1, has: ["RED - run 4001", "check (ubuntu-latest)"] });
    run("gh unauthenticated -> notice, exit 0", { scenario: "unauthenticated", status: 0, has: ["skipped: not read (gh unauthenticated)"], lacks: unreadLacks });
    run("no network -> notice, exit 0", { scenario: "offline", status: 0, has: ["skipped: not read (no network)"], lacks: unreadLacks });
    run("gh absent -> notice, exit 0", { gh: join(FIXTURES, "no-such-gh"), status: 0, has: ["skipped: not read (gh absent)"], lacks: unreadLacks });
    run("no origin remote -> notice, exit 0", { dir: noOrigin, scenario: "green", status: 0, has: ["skipped: not read (no origin remote)"], lacks: unreadLacks });
    run("no completed CI run -> notice, exit 0", { scenario: "no-runs", status: 0, has: ["skipped: not read (no completed CI run)"], lacks: unreadLacks });
  } finally {
    rmSync(withOrigin, { recursive: true, force: true });
    rmSync(noOrigin, { recursive: true, force: true });
  }

  const slugs = [
    ["https://github.com/o/r.git", "o/r"],
    ["git@github.com:o/r.git", "o/r"],
    ["https://github.com/o/r", "o/r"],
    ["https://gitlab.com/o/r.git", null],
  ];
  for (const [url, want] of slugs) {
    const got = githubSlug(url);
    results.push({ ok: got === want, name: `githubSlug(${url}) -> ${want}`, detail: `got ${got}`, out: "" });
  }

  for (const r of results) {
    console.log(`  ${r.ok ? "ok  " : "FAIL"} ${r.name}`);
    if (!r.ok) {
      console.log(`       ${r.detail}`);
      for (const l of r.out.trim().split(/\r?\n/)) console.log(`       | ${l}`);
    }
  }
  const passed = results.filter((r) => r.ok).length;
  console.log(`upstream CI self-test: ${passed} of ${results.length}`);
  process.exit(passed === results.length && results.length === 13 ? 0 : 1);
}

// ---------------------------------------------------------------------------

if (process.argv[1] && resolve(process.argv[1]) === SCRIPT) {
  const args = process.argv.slice(2);
  const flags = new Set(args.filter((a) => a.startsWith("--")));
  const root = resolve(args.find((a) => !a.startsWith("--")) ?? REPO_ROOT);
  if (flags.has("--self-test")) selfTest();
  const r = readUpstream({ cwd: root });
  if (flags.has("--json")) console.log(JSON.stringify(r));
  else {
    const d = describe(r);
    (d.stream === "stdout" ? console.log : console.error)(d.text);
  }
  process.exit(r.state === "red" ? 1 : 0);
}
