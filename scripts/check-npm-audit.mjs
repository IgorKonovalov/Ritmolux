#!/usr/bin/env node
// Gate the two npm graphs on their published advisories (ADR-0244).
//
// `cargo deny` has watched `Cargo.lock` from the start; this is the same watch over `studio/` and
// `site/`. It runs `npm audit --json` three times and holds each reading to its own line:
//
//     graph                          command                              fails at
//     studio, shipped                npm audit --json --omit=dev          high
//     studio, full                   npm audit --json                     critical
//     site, full                     npm audit --json                     critical
//
// The shipped graph is what reaches a tester inside the studio zip, Electron itself included. The
// full graphs are every package, dev and build tooling too, because a critical in a test runner or
// a packager still runs on a developer's machine and in CI. Anything under a line is printed and
// never failed on.
//
// Usage:  node scripts/check-npm-audit.mjs
//         node scripts/check-npm-audit.mjs --self-test
//
// Exit 0 = no advisory at or over its graph's line that the allow file does not excuse. Exit 1 =
// each one listed as `<graph>  <severity>  <GHSA id>  <package>  <title>`, or an allow file this
// gate refuses, or an audit that did not answer.
//
// THE ALLOW FILE is `npm-audit.allow.json` at the repository root: `{ "allow": [ { "id":
// "GHSA-....", "reason": "..." } ] }`. An entry excuses its id in every graph. An entry with no
// reason is a failure, so the list cannot become an off switch - the same rule `deny.toml`'s
// ignores follow. An entry whose id no graph reports any more is PRINTED and does not fail: the
// advisory went away, and the entry is owed a deletion rather than a red build.
//
// A FAILED AUDIT IS A FAILURE, NEVER A PASS. `npm audit` exits non-zero both when it finds
// something and when it cannot reach the registry, so its exit status says nothing here. What is
// read is the JSON: a report carrying `auditReportVersion` and a `vulnerabilities` object is an
// answer; an `error` object, output that is not JSON, or no npm at all is not, and fails the run.
//
// ONLY ADVISORIES ARE COUNTED, NOT THE PACKAGES THEY REACH. A report lists every package that sits
// above a vulnerable one, with a bare package name in `via`; those entries repeat an advisory
// already counted at its source and are skipped. An advisory's id is the GHSA id at the end of its
// `url`, which is the id the allow file names and the one GitHub's advisory database is keyed on.
//
// NOT ON THE PRE-PUSH ROSTER, AND NOT IN `scripts/gates.manifest.mjs`. It needs the network and its
// answer changes without a commit, so it runs in its own CI job, beside `deny` (ADR-0244, which
// follows ADR-0033's reason for keeping `cargo deny` out of the hook).

import { readFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const SCRIPTS_DIR = dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = resolve(SCRIPTS_DIR, "..");
const ALLOW_FILE = "npm-audit.allow.json";

const SEVERITIES = ["info", "low", "moderate", "high", "critical"];

/** The three readings, each with the line it fails at. */
export const GRAPHS = [
  { name: "studio (shipped)", dir: "studio", omitDev: true, failAt: "high" },
  { name: "studio (full)", dir: "studio", omitDev: false, failAt: "critical" },
  { name: "site (full)", dir: "site", omitDev: false, failAt: "critical" },
];

const rank = (severity) => SEVERITIES.indexOf(severity);

/** `https://github.com/advisories/GHSA-xxxx-xxxx-xxxx` -> `GHSA-xxxx-xxxx-xxxx`, else null. */
function ghsaOf(url) {
  const m = typeof url === "string" ? url.match(/(GHSA(?:-[0-9a-z]{4}){3})\b/i) : null;
  return m ? m[1] : null;
}

/**
 * Parse one `npm audit --json` run into its advisories, or into the reason it is not an answer.
 *
 * `raw` is `{ stdout, error }` as `spawnSync` returns them. The exit status is deliberately not an
 * input: see the header.
 */
export function readAudit(raw) {
  if (raw.error) return { failed: `npm did not run: ${raw.error.message ?? raw.error}` };
  let report;
  try {
    report = JSON.parse(raw.stdout);
  } catch {
    const head = String(raw.stdout ?? "").trim().split("\n")[0] ?? "";
    return { failed: `the audit printed no JSON${head ? `: ${head.slice(0, 120)}` : ""}` };
  }
  if (report && typeof report === "object" && report.error) {
    const e = report.error;
    return { failed: `the audit request failed: ${e.code ?? "?"} ${e.summary ?? ""}`.trim() };
  }
  if (
    !report ||
    typeof report !== "object" ||
    report.auditReportVersion === undefined ||
    typeof report.vulnerabilities !== "object" ||
    report.vulnerabilities === null
  ) {
    return { failed: "the audit printed JSON that is not an audit report" };
  }

  const seen = new Set();
  const advisories = [];
  for (const [pkg, entry] of Object.entries(report.vulnerabilities)) {
    for (const via of entry?.via ?? []) {
      if (typeof via !== "object" || via === null) continue;
      const id = ghsaOf(via.url) ?? (via.source !== undefined ? `npm-${via.source}` : null);
      const name = via.name ?? pkg;
      const key = `${id}|${name}`;
      if (id === null || seen.has(key)) continue;
      seen.add(key);
      advisories.push({ id, package: name, severity: via.severity ?? entry.severity, title: via.title ?? "" });
    }
  }
  return { advisories };
}

/**
 * Read the allow file's text into its entries and the problems with it.
 *
 * `text` null means the file is absent, which is an empty list rather than a failure.
 */
export function readAllow(text) {
  if (text === null) return { entries: [], problems: [] };
  let doc;
  try {
    doc = JSON.parse(text);
  } catch (e) {
    return { entries: [], problems: [`${ALLOW_FILE} is not JSON: ${e.message}`] };
  }
  if (!doc || !Array.isArray(doc.allow)) {
    return { entries: [], problems: [`${ALLOW_FILE} has no "allow" array`] };
  }
  const entries = [];
  const problems = [];
  doc.allow.forEach((entry, i) => {
    const id = typeof entry?.id === "string" ? entry.id.trim() : "";
    const reason = typeof entry?.reason === "string" ? entry.reason.trim() : "";
    if (!ghsaOf(id) || ghsaOf(id) !== id) {
      problems.push(`${ALLOW_FILE} entry ${i + 1}: "${id}" is not a GHSA id`);
      return;
    }
    if (!reason) {
      problems.push(`${ALLOW_FILE} entry ${i + 1}: ${id} gives no reason`);
      return;
    }
    entries.push({ id, reason });
  });
  return { entries, problems };
}

/**
 * Judge the readings against their lines and the allow file.
 *
 * `readings` is `[{ graph, audit }]`, `audit` being what `readAudit` returned. Everything the exit
 * code is taken from is computed here, which is what the self-test calls.
 */
export function judge(readings, allow) {
  const allowed = new Map(allow.entries.map((e) => [e.id, e]));
  const reported = new Set();
  const blocking = [];
  const excused = [];
  const below = [];
  const failedRequests = [];

  for (const { graph, audit } of readings) {
    if (audit.failed) {
      failedRequests.push(`${graph.name}: ${audit.failed}`);
      continue;
    }
    for (const a of audit.advisories) {
      reported.add(a.id);
      const row = { graph: graph.name, ...a };
      if (rank(a.severity) < rank(graph.failAt)) below.push(row);
      else if (allowed.has(a.id)) excused.push({ ...row, reason: allowed.get(a.id).reason });
      else blocking.push(row);
    }
  }

  // Staleness is only knowable when every graph answered: an id missing from a graph that failed to
  // report is not evidence that its advisory went away.
  const stale = failedRequests.length === 0 ? allow.entries.filter((e) => !reported.has(e.id)) : [];
  const ok = blocking.length === 0 && failedRequests.length === 0 && allow.problems.length === 0;
  return { ok, blocking, excused, below, stale, failedRequests, allowProblems: allow.problems };
}

const line = (r) => `  ${r.graph.padEnd(17)} ${r.severity.padEnd(8)} ${r.id}  ${r.package}  ${r.title}`;

function report(verdict) {
  if (verdict.below.length > 0) {
    console.log(`\nnpm audit: ${verdict.below.length} advisory reading(s) under their graph's line (printed, not failed on):`);
    for (const r of verdict.below) console.log(line(r));
  }
  if (verdict.excused.length > 0) {
    console.log(`\nnpm audit: ${verdict.excused.length} excused by ${ALLOW_FILE}:`);
    for (const r of verdict.excused) console.log(`${line(r)}\n      reason: ${r.reason}`);
  }
  if (verdict.stale.length > 0) {
    console.log(`\nnpm audit: ${verdict.stale.length} ${ALLOW_FILE} entr(y/ies) no graph reports any more - delete them:`);
    for (const e of verdict.stale) console.log(`  ${e.id}  (${e.reason})`);
  }
  if (verdict.ok) {
    console.log("\nnpm audit: OK (nothing high in the shipped studio graph, nothing critical in any graph)");
    return;
  }
  if (verdict.failedRequests.length > 0) {
    console.error(`\nnpm audit: ${verdict.failedRequests.length} audit(s) did not answer, which fails rather than passes:`);
    for (const f of verdict.failedRequests) console.error(`  ${f}`);
  }
  if (verdict.allowProblems.length > 0) {
    console.error(`\nnpm audit: ${verdict.allowProblems.length} problem(s) with ${ALLOW_FILE}:`);
    for (const p of verdict.allowProblems) console.error(`  ${p}`);
  }
  if (verdict.blocking.length > 0) {
    console.error(`\nnpm audit: ${verdict.blocking.length} advisory reading(s) at or over their graph's line:`);
    for (const r of verdict.blocking) console.error(line(r));
  }
  console.error(
    "\nThe lines are ADR-0244's: high in studio's shipped graph (--omit=dev), critical in every\n" +
      "full graph. The repair is a bump that clears the advisory, or an entry in\n" +
      `${ALLOW_FILE} naming its GHSA id with the reason the studio or the site is not exposed.\n` +
      "The reason is reviewed rather than checked, so write the one you would defend.",
  );
}

// ---------------------------------------------------------------------------------------------
// --self-test

const FIXTURES = join(SCRIPTS_DIR, "fixtures", "npm-audit");

function selfTest() {
  const fixture = (name) => readFileSync(join(FIXTURES, name), "utf8");
  const audit = (name) => readAudit({ stdout: fixture(name) });
  const allowOf = (name) => readAllow(name === null ? null : fixture(name));
  const [shipped, studioFull, siteFull] = GRAPHS;
  // One reading per graph, the fixture named standing in for what npm printed for it.
  const verdictOf = (shippedName, fullName, allowName = null) =>
    judge(
      [
        { graph: shipped, audit: audit(shippedName) },
        { graph: studioFull, audit: audit(fullName) },
        { graph: siteFull, audit: audit("clean.json") },
      ],
      allowOf(allowName),
    );

  const results = [];
  const is = (what, actual, wanted) => results.push({ what, ok: actual === wanted, actual, wanted });

  // The reader, before anything is judged: an advisory is counted once, at its source.
  const high = audit("shipped-high.json");
  is("a report reads its advisories, not the packages above them", high.advisories?.length, 1);
  is("an advisory's id is the GHSA id at the end of its url", high.advisories?.[0].id, "GHSA-fxhi-fxhi-fxhi");
  is("a clean report reads as zero advisories, not as a failure", audit("clean.json").advisories?.length, 0);

  // A high in the shipped graph fails.
  const v1 = verdictOf("shipped-high.json", "shipped-high.json");
  is("a high in the shipped graph fails", v1.ok, false);
  is("and it is the shipped graph's reading that blocks", v1.blocking.map((r) => r.graph).join(","), shipped.name);
  is("and the same high in a full graph is under that graph's line", v1.below.map((r) => r.graph).join(","), studioFull.name);

  // A critical in a dev-only package fails, through the full graph alone.
  const v2 = verdictOf("clean.json", "dev-critical.json");
  is("a critical in a dev-only package fails", v2.ok, false);
  is("through the full graph, the shipped one being clean", v2.blocking.map((r) => `${r.graph} ${r.package}`).join(","), `${studioFull.name} vitest`);

  // A moderate passes, and is printed.
  const v3 = verdictOf("moderate.json", "moderate.json");
  is("a moderate passes", v3.ok, true);
  is("and is printed under the line in both studio graphs", v3.below.length, 2);

  // An allowed id with a reason passes.
  const v4 = verdictOf("shipped-high.json", "shipped-high.json", "allow-reasoned.json");
  is("an allowed id with a reason passes", v4.ok, true);
  is("and is reported as excused, with its reason", v4.excused[0]?.reason?.length > 0, true);
  is("and is not stale", v4.stale.length, 0);

  // An allow entry without a reason fails, even when it would excuse the only blocking advisory.
  const v5 = verdictOf("shipped-high.json", "shipped-high.json", "allow-no-reason.json");
  is("an allow entry without a reason fails", v5.ok, false);
  is("and names the entry", v5.allowProblems.join(" / "), `${ALLOW_FILE} entry 1: GHSA-fxhi-fxhi-fxhi gives no reason`);
  is("an allow file that is not JSON fails", readAllow("{ not json").problems.length, 1);
  is("an allow id that is not a GHSA id fails", readAllow('{"allow":[{"id":"CVE-2026-1","reason":"r"}]}').problems.length, 1);

  // An allow entry whose id no longer appears is reported, and does not fail.
  const v6 = verdictOf("clean.json", "clean.json", "allow-stale.json");
  is("an allow entry nothing reports is reported as stale", v6.stale.map((e) => e.id).join(","), "GHSA-fxst-fxst-fxst");
  is("and does not fail the run", v6.ok, true);

  // A failed request fails, never passes.
  const v7 = verdictOf("request-failed.json", "clean.json");
  is("an audit request that failed fails the run", v7.ok, false);
  is("and says which graph did not answer", v7.failedRequests[0]?.startsWith(shipped.name), true);
  is("output that is not JSON is a failed request", Boolean(readAudit({ stdout: "npm ERR! network" }).failed), true);
  is("empty output is a failed request", Boolean(readAudit({ stdout: "" }).failed), true);
  is("npm that did not start is a failed request", Boolean(readAudit({ stdout: "", error: new Error("ENOENT") }).failed), true);
  is("JSON that is not a report is a failed request", Boolean(readAudit({ stdout: "{}" }).failed), true);
  const v8 = verdictOf("request-failed.json", "clean.json", "allow-stale.json");
  is("and a failed request withholds staleness rather than guessing it", v8.stale.length, 0);

  const failed = results.filter((r) => !r.ok);
  for (const r of failed) console.error(`  FAIL ${r.what}\n    expected: ${JSON.stringify(r.wanted)}\n    actual:   ${JSON.stringify(r.actual)}`);
  console.log(`npm audit self-test: ${results.length - failed.length} of ${results.length}`);
  return failed.length === 0;
}

// ---------------------------------------------------------------------------------------------

if (process.argv.slice(2).includes("--self-test")) {
  process.exit(selfTest() ? 0 : 1);
}

function runAudit(graph) {
  const args = ["audit", "--json"];
  if (graph.omitDev) args.push("--omit=dev");
  // `npm` is `npm.cmd` on Windows, which only a shell resolves.
  return spawnSync("npm", args, {
    cwd: join(REPO_ROOT, graph.dir),
    encoding: "utf8",
    shell: process.platform === "win32",
    maxBuffer: 64 * 1024 * 1024,
  });
}

let allowText = null;
try {
  allowText = readFileSync(join(REPO_ROOT, ALLOW_FILE), "utf8");
} catch {
  allowText = null;
}

const readings = GRAPHS.map((graph) => ({ graph, audit: readAudit(runAudit(graph)) }));
for (const { graph, audit } of readings) {
  const summary = audit.failed ? "no answer" : `${audit.advisories.length} advisory reading(s)`;
  console.log(`npm audit: ${graph.name.padEnd(17)} fails at ${graph.failAt.padEnd(8)} ${summary}`);
}
const verdict = judge(readings, readAllow(allowText));
report(verdict);
process.exit(verdict.ok ? 0 : 1);
