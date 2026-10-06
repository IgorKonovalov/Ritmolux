#!/usr/bin/env node
// The report `bless.yml` writes when it blesses named WARP baselines (ADR-0264).
//
// The workflow runs the golden and pinned tests twice on `windows-latest`: a compare run with no
// `RLX_BLESS`, then a bless run with `RLX_BLESS` set to the dispatched names. This reads both logs and
// writes `report.md`:
//
//   - one row per NAMED baseline: its mean and max outlier against the committed baseline, the two
//     tolerances, and whether the bless run wrote it;
//   - one row per OTHER baseline over tolerance, marked "not blessed, still failing";
//   - one line per named baseline the compare run printed no comparison for.
//
// Usage:  node scripts/bless-report.mjs --check-names <a,b,...>
//         node scripts/bless-report.mjs --names <a,b,...> --compare <log> --bless <log>
//                                       [--out <report.md>] [--stage <dir>]
//         node scripts/bless-report.mjs --self-test
//
// `--check-names` runs before anything is built: exit 1 when the list is not a list of stems with a
// committed PNG under `core/tests/golden/`. The report mode exits 1 when a named baseline was not
// blessed. A baseline over tolerance that nobody named never fails it: that is the report's finding,
// and the job's output is the artifact a person reads.
//
// `--stage <dir>` copies each blessed named PNG to `<dir>/core/tests/golden/<stem>.png`, so the
// artifact unpacks at its repository paths. With `GITHUB_STEP_SUMMARY` set, the report is also
// appended there.
//
// THE NAMES ARE STEMS AND ONLY STEMS. `RLX_BLESS=1` blesses every baseline locally; this job refuses
// it, because it blesses by name only. A stem is `[A-Za-z0-9_]+`, which also keeps a name from
// leaving `core/tests/golden/` when it is joined into a path.
//
// THE COMPARE LINE IS THE ONE THE GOLDEN AND PINNED TESTS PRINT:
// `<stem> mean <m> (tol <t>) max_outlier <o> (tol <t>)`, anchored at the line's start and allowed a
// suffix (`, N of M lit`, ` at WxH`). A line in any other shape is not read, and a named stem with
// no line is reported rather than guessed. The bless line is `blessed <path>`, read by the file name
// at its end with either path separator.
//
// RUNS ONLY ON DISPATCH. It is not on the pre-push roster or in `scripts/gates.manifest.mjs`, and its
// --self-test runs by hand (scripts/README.md, the named exceptions).

import { copyFileSync, existsSync, appendFileSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const SCRIPTS_DIR = dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = resolve(SCRIPTS_DIR, "..");
const GOLDEN_REL = join("core", "tests", "golden");
const STEM = /^[A-Za-z0-9_]+$/;

/** Parse the dispatched list into stems, or the problems with it. */
export function parseNames(value) {
  const trimmed = String(value ?? "").trim();
  if (trimmed === "1") {
    return { names: [], problems: ["`1` blesses every baseline; this job blesses named baselines only"] };
  }
  const names = [];
  const problems = [];
  for (const raw of trimmed.split(",")) {
    const name = raw.trim();
    if (name === "") continue;
    if (!STEM.test(name)) problems.push(`${JSON.stringify(name)} is not a baseline stem`);
    else if (!names.includes(name)) names.push(name);
  }
  if (names.length === 0 && problems.length === 0) problems.push("the list names no baseline");
  return { names, problems };
}

/** The names with no committed `core/tests/golden/<stem>.png` under `root`. */
export function missingBaselines(names, root = REPO_ROOT) {
  return names.filter((n) => !existsSync(join(root, GOLDEN_REL, `${n}.png`)));
}

/** Strip ANSI colour codes and a trailing CR, which a Windows runner's log carries. */
const clean = (line) => line.replace(/\x1b\[[0-9;]*m/g, "").replace(/\r$/, "");

const COMPARE = /^(\S+)\s+mean (\d+(?:\.\d+)?) \(tol (\d+(?:\.\d+)?)\) max_outlier (\d+) \(tol (\d+)\)/;

/** Every comparison line in a compare log, by stem. A stem printed twice keeps its last reading. */
export function readCompare(text) {
  const rows = new Map();
  for (const line of text.split("\n").map(clean)) {
    const m = line.match(COMPARE);
    if (!m) continue;
    const [, stem, mean, meanTol, outlier, outlierTol] = m;
    const row = { stem, mean: Number(mean), meanTol: Number(meanTol), outlier: Number(outlier), outlierTol: Number(outlierTol) };
    row.over = row.mean > row.meanTol || row.outlier > row.outlierTol;
    rows.set(stem, row);
  }
  return rows;
}

/** The stems a bless log says were written. */
export function readBlessed(text) {
  const stems = new Set();
  for (const line of text.split("\n").map(clean)) {
    const m = line.match(/^blessed (.+\.png)\s*$/);
    if (!m) continue;
    const file = m[1].split(/[\\/]/).pop();
    stems.add(file.slice(0, -".png".length));
  }
  return stems;
}

/** Everything the exit code and `report.md` are taken from. */
export function judge({ names, compare, blessed }) {
  const named = names.map((stem) => ({ stem, row: compare.get(stem) ?? null, blessed: blessed.has(stem) }));
  const others = [...compare.values()].filter((r) => r.over && !names.includes(r.stem));
  const noComparison = named.filter((n) => n.row === null).map((n) => n.stem);
  const notBlessed = named.filter((n) => !n.blessed).map((n) => n.stem);
  return { named, others, noComparison, notBlessed, read: compare.size, ok: notBlessed.length === 0 };
}

const reading = (r) => `${r.mean.toFixed(4)} | ${r.meanTol} | ${r.outlier} | ${r.outlierTol}`;

/** `report.md`'s text. */
export function render(verdict, names) {
  const out = [];
  out.push(`# Bless report: ${names.join(", ")}`, "");
  out.push(
    `The compare run printed ${verdict.read} comparison(s) against the committed baselines, before ` +
      "anything was blessed. The bless run then wrote the named baselines only.",
    "",
  );
  out.push("## Named", "");
  out.push("| baseline | mean | mean tol | max outlier | outlier tol | over tolerance | blessed |");
  out.push("|---|---|---|---|---|---|---|");
  for (const n of verdict.named) {
    const cells = n.row ? `${reading(n.row)} | ${n.row.over ? "yes" : "no"}` : "- | - | - | - | no comparison";
    out.push(`| \`${n.stem}\` | ${cells} | ${n.blessed ? "yes" : "**NO**"} |`);
  }
  out.push("");
  out.push("## Not named, over tolerance", "");
  if (verdict.others.length === 0) {
    out.push("None: every baseline nobody named is within tolerance.");
  } else {
    out.push("| baseline | mean | mean tol | max outlier | outlier tol | |");
    out.push("|---|---|---|---|---|---|");
    for (const r of verdict.others) out.push(`| \`${r.stem}\` | ${reading(r)} | not blessed, still failing |`);
  }
  if (verdict.noComparison.length > 0) {
    out.push("", "## Named, no comparison found", "");
    for (const s of verdict.noComparison) out.push(`- \`${s}\`: the compare run printed no comparison line for it.`);
  }
  if (verdict.notBlessed.length > 0) {
    out.push("", "## Named, not blessed", "");
    for (const s of verdict.notBlessed) out.push(`- \`${s}\`: the bless run printed no \`blessed\` line for it, and it is not in the artifact.`);
  }
  out.push("");
  return out.join("\n");
}

function argValue(args, flag) {
  const i = args.indexOf(flag);
  return i >= 0 && i + 1 < args.length ? args[i + 1] : null;
}

// ---------------------------------------------------------------------------------------------
// --self-test

const FIXTURES = join(SCRIPTS_DIR, "fixtures", "bless-report");

function selfTest() {
  const fixture = (name) => readFileSync(join(FIXTURES, name), "utf8");
  const results = [];
  const is = (what, actual, wanted) => results.push({ what, ok: actual === wanted, actual, wanted });

  // The names.
  is("`1` is refused", parseNames("1").problems.length, 1);
  is("` 1 ` is refused too", parseNames(" 1 ").problems.length, 1);
  is("an empty list is refused", parseNames("").problems.length, 1);
  is("a list of only commas is refused", parseNames(" , ,").problems.length, 1);
  is("whitespace around names is dropped", parseNames(" waterfall ,\tparametric_torus_knot ,").names.join(","), "waterfall,parametric_torus_knot");
  is("a name repeated is read once", parseNames("waterfall,waterfall").names.join(","), "waterfall");
  is("a path is not a stem", parseNames("../waterfall").problems.length, 1);
  is("a stem with its extension is not a stem", parseNames("waterfall.png").problems.length, 1);
  is("a committed baseline is found", missingBaselines(["waterfall"]).length, 0);
  is("an uncommitted one is named", missingBaselines(["waterfall", "no_such_stem"]).join(","), "no_such_stem");

  // The compare log: real output of the golden and pinned tests.
  const compare = readCompare(fixture("compare.log"));
  is("every comparison line in the fixture is read", compare.size, 53);
  is("a line_joints probe line is not a comparison", compare.has("element"), false);
  is("the attractor_trails suffix does not stop the read", compare.get("attractor_trails")?.outlier, 89);
  is("the warp_mesh_wide suffix does not stop the read", compare.get("warp_mesh_wide")?.mean, 0.0006);
  is("a stem wider than the column is read", compare.get("parametric_lissajous_3d")?.outlier, 224);
  is("the tolerances are read from the line", compare.get("waterfall")?.outlierTol, 48);
  is("a skip notice's quoted drift is not a comparison", [...compare.keys()].some((k) => k.startsWith('"')), false);
  const crlf = readCompare("\x1b[32mwaterfall          mean 0.0017 (tol 0.02) max_outlier 213 (tol 48)\x1b[0m\r\n");
  is("a CRLF line carrying colour codes is read", crlf.get("waterfall")?.outlier, 213);

  // The bless log.
  const blessed = readBlessed(fixture("bless.log"));
  is("a Windows path's file name is the stem", blessed.has("waterfall"), true);
  is("both blessed lines are read", blessed.size, 2);
  is("a forward-slash path reads the same", readBlessed("blessed /x/core/tests/golden/waterfall_ramp.png\n").has("waterfall_ramp"), true);

  // The verdict: two named baselines over tolerance and blessed, one named and absent from both logs.
  const names = ["waterfall", "parametric_torus_knot", "no_such_stem"];
  const v = judge({ names, compare, blessed });
  is("a named baseline over tolerance is a named row", v.named[0].row?.over, true);
  is("and is blessed", v.named[0].blessed, true);
  is(
    "every other baseline over tolerance is listed, and no named one",
    v.others.map((r) => r.stem).join(","),
    "reaction_diffusion,attractor,attractor_depth,parametric_lissajous_3d,waterfall_ramp,attractor_trails",
  );
  is("a named baseline with no comparison is reported", v.noComparison.join(","), "no_such_stem");
  is("a named baseline the bless run did not write fails the report", v.ok, false);
  is("and is the only one", v.notBlessed.join(","), "no_such_stem");
  const within = judge({ names: ["swarm"], compare, blessed: new Set(["swarm"]) });
  is("a named baseline within tolerance reads as not over", within.named[0].row?.over, false);
  is("and a report whose named baselines were all blessed passes", within.ok, true);

  const md = render(v, names);
  is("an unnamed one is marked not blessed, still failing", md.includes("| `reaction_diffusion` | 0.0110 | 0.02 | 190 | 48 | not blessed, still failing |"), true);
  is("a named one is not", md.includes("`waterfall` | 0.0017 | 0.02 | 213 | 48 | not blessed"), false);
  is("the named row carries its reading", md.includes("| `waterfall` | 0.0017 | 0.02 | 213 | 48 | yes | yes |"), true);
  is("the missing comparison has its line", md.includes("- `no_such_stem`: the compare run printed no comparison line for it."), true);

  const failed = results.filter((r) => !r.ok);
  for (const r of failed) console.error(`  FAIL ${r.what}\n    expected: ${JSON.stringify(r.wanted)}\n    actual:   ${JSON.stringify(r.actual)}`);
  console.log(`bless report self-test: ${results.length - failed.length} of ${results.length}`);
  return failed.length === 0;
}

// ---------------------------------------------------------------------------------------------

const args = process.argv.slice(2);

if (args.includes("--self-test")) {
  process.exit(selfTest() ? 0 : 1);
}

function namesOrExit(value) {
  const { names, problems } = parseNames(value);
  for (const m of missingBaselines(names)) problems.push(`${m}: no committed ${GOLDEN_REL.split("\\").join("/")}/${m}.png`);
  if (problems.length > 0) {
    console.error(`bless report: the baseline list is refused:\n${problems.map((p) => `  ${p}`).join("\n")}`);
    process.exit(1);
  }
  return names;
}

const checkOnly = argValue(args, "--check-names");
if (checkOnly !== null) {
  const names = namesOrExit(checkOnly);
  console.log(`bless report: ${names.length} baseline(s) named, each committed: ${names.join(", ")}`);
  process.exit(0);
}

const namesArg = argValue(args, "--names");
const compareArg = argValue(args, "--compare");
const blessArg = argValue(args, "--bless");
if (namesArg === null || compareArg === null || blessArg === null) {
  console.error("usage: node scripts/bless-report.mjs --names <a,b> --compare <log> --bless <log> [--out <file>] [--stage <dir>]");
  process.exit(2);
}
const names = namesOrExit(namesArg);
const verdict = judge({
  names,
  compare: readCompare(readFileSync(compareArg, "utf8")),
  blessed: readBlessed(readFileSync(blessArg, "utf8")),
});
const md = render(verdict, names);
process.stdout.write(md);

const outArg = argValue(args, "--out");
if (outArg !== null) {
  mkdirSync(dirname(outArg), { recursive: true });
  writeFileSync(outArg, md);
}
const stageArg = argValue(args, "--stage");
if (stageArg !== null) {
  const dest = join(stageArg, GOLDEN_REL);
  mkdirSync(dest, { recursive: true });
  for (const n of verdict.named.filter((x) => x.blessed)) {
    copyFileSync(join(REPO_ROOT, GOLDEN_REL, `${n.stem}.png`), join(dest, `${n.stem}.png`));
  }
}
if (process.env.GITHUB_STEP_SUMMARY) appendFileSync(process.env.GITHUB_STEP_SUMMARY, `${md}\n`);

process.exit(verdict.ok ? 0 : 1);
