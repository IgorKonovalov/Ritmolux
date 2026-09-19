#!/usr/bin/env node
// Assert that every carrier of the Node gate roster runs the manifest's projection for it, in order.
//
// `scripts/gates.manifest.mjs` is the roster (ADR-0217). `tools/conductor/lib/gate.mjs` imports it,
// so that carrier cannot drift at all. The other two write their own invocations in their own
// languages — `.githooks/pre-push` in POSIX sh, CI's `links` job in YAML — and this is what holds
// them to the list. A gate added to the hook and not to CI is red here at the push.
//
// Usage:  node scripts/check-gate-carriers.mjs [root] [--roster <manifest.mjs>]
//         node scripts/check-gate-carriers.mjs --self-test
//
// Exit 0 = both carriers match. Exit 1 = the first difference per carrier, as
// `<carrier>: expected <invocation> at position N, found <invocation>`. The optional `root` reads
// some other tree's two files, following the checkers beside it, and `--roster` measures that tree
// against another manifest — which is what makes each seeded case under
// `scripts/fixtures/gate-carriers/` a root anyone can run:
//
//     node scripts/check-gate-carriers.mjs scripts/fixtures/gate-carriers/missing \
//       --roster scripts/fixtures/gate-carriers/manifest.mjs
//
// CI and the pre-push hook pass neither.
//
// WHAT IT ASSERTS IS INVOCATIONS AND ORDER, AND NEVER THE CONDITIONS A CARRIER ATTACHES. CI guards
// `check-release-tag.mjs --remote` with an `if:` for a push to `main`; this gate sees the invocation
// and not the guard, and a `when:` added or deleted moves nothing here. That bound is ADR-0217's
// Negative 2, written down here so a green run is never read as saying more than it does.
//
// Nor does it read the hook's English skip notice, which names the gates in a sentence. Holding that
// to the manifest would mean parsing prose; it drifts, and the cost of the drift is a notice that
// under-names what was skipped (ADR-0217 Negative 3).
//
// THE PARSERS ARE REGEXES OVER A SHELL SCRIPT AND A YAML FILE, which is ADR-0217's Negative 1 and
// the price of keeping each carrier's own per-step reporting. A spelling they do not recognise —
// `node "scripts/x.mjs"`, a `run: |` block, a loop, a composite action — is invisible to them, and
// an invisible invocation reads as a MISSING gate rather than as a pass, which is the safe
// direction. `--self-test` bounds the shapes they do know; it does not remove the hole.
//
// UNLIKE ITS SIBLINGS, THE PLAIN RUN CANNOT GO VACUOUSLY GREEN. A parser that stopped matching
// reports an empty list against a roster that is not empty, which is exit 1 in the loudest possible
// form. `--self-test` is here for the reporting path and the three drift shapes, not to rescue an
// exit code that could be a silence.

import { readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { CARRIERS, GATES, invocationsFor } from "./gates.manifest.mjs";

const SCRIPTS_DIR = dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = resolve(SCRIPTS_DIR, "..");

/** Where each file-backed carrier writes its invocations, relative to a root. */
export const CARRIER_FILES = {
  hook: ".githooks/pre-push",
  ci: ".github/workflows/ci.yml",
};

/** The `links` job is the only one in `ci.yml` that carries this roster. */
const CI_JOB = "links";

/**
 * The hook's Node gate invocations, in file order.
 *
 * A step is `run_step "<label>" <command…>`, and the LABEL IS NOT READ: it is a display string the
 * hook writes twice over, and holding it to the manifest would convict a reworded echo.
 */
export function hookInvocations(text) {
  const found = [];
  for (const line of text.split("\n")) {
    const m = line.match(/^\s*run_step\s+"[^"]*"\s+(\S.*?)\s*$/);
    if (m && m[1].startsWith("node scripts/")) found.push(m[1].replace(/\s+/g, " "));
  }
  return found;
}

/**
 * The `links` job's Node gate invocations, in file order.
 *
 * The job block is delimited by indentation — a job's key sits two spaces in under `jobs:` — so a
 * gate moved to another job reads as missing here rather than as present somewhere.
 */
export function ciInvocations(text, job = CI_JOB) {
  const lines = text.split("\n");
  const start = lines.findIndex((l) => l === `  ${job}:`);
  if (start < 0) return null;
  let end = lines.findIndex((l, i) => i > start && /^ {2}[A-Za-z_][\w-]*:\s*$/.test(l));
  if (end < 0) end = lines.length;
  const found = [];
  for (const line of lines.slice(start + 1, end)) {
    const m = line.match(/^\s*(?:-\s+)?run:\s*(\S.*?)\s*$/);
    if (m && m[1].startsWith("node scripts/")) found.push(m[1].replace(/\s+/g, " "));
  }
  return found;
}

/** The first place `found` and `expected` differ, as one line, or null when they are equal. */
export function firstDifference(carrier, found, expected) {
  const n = Math.max(found.length, expected.length);
  for (let i = 0; i < n; i++) {
    if (found[i] === expected[i]) continue;
    return `${carrier}: expected ${expected[i] ?? "nothing"} at position ${i + 1}, found ${found[i] ?? "nothing"}`;
  }
  return null;
}

/** Every carrier a gate names must be one this roster knows, or its projection silently loses it. */
function unknownCarriers(gates) {
  const bad = [];
  for (const g of gates) {
    for (const c of g.carriers) {
      if (!CARRIERS.includes(c)) bad.push(`${g.script} names carrier "${c}", which is not in CARRIERS`);
    }
  }
  return bad;
}

function readCarrier(root, carrier) {
  const path = join(root, CARRIER_FILES[carrier]);
  const text = readFileSync(path, "utf8").replace(/\r\n/g, "\n");
  const found = carrier === "hook" ? hookInvocations(text) : ciInvocations(text);
  return { path: CARRIER_FILES[carrier], found };
}

/** Reads both file-backed carriers under `root` and measures each against `gates`. */
function check(root, gates = GATES) {
  const problems = unknownCarriers(gates);
  const counts = [];
  for (const carrier of Object.keys(CARRIER_FILES)) {
    const expected = invocationsFor(carrier, gates);
    let read;
    try {
      read = readCarrier(root, carrier);
    } catch (e) {
      problems.push(`${carrier}: cannot read ${CARRIER_FILES[carrier]} (${e.message})`);
      continue;
    }
    if (read.found === null) {
      problems.push(`${carrier}: ${read.path} has no \`${CI_JOB}\` job to read`);
      continue;
    }
    counts.push(`${carrier} ${read.found.length}/${expected.length}`);
    const diff = firstDifference(carrier, read.found, expected);
    if (diff) problems.push(diff);
  }
  return { problems, counts };
}

// ---------------------------------------------------------------------------------------------
// --self-test

const FIXTURES = join(SCRIPTS_DIR, "fixtures", "gate-carriers");

async function selfTest() {
  const { GATES: FIXTURE_GATES } = await import(`file://${join(FIXTURES, "manifest.mjs").replace(/\\/g, "/")}`);
  const readCase = (name, carrier) => readFileSync(join(FIXTURES, name, CARRIER_FILES[carrier]), "utf8").replace(/\r\n/g, "\n");
  const expect = { hook: invocationsFor("hook", FIXTURE_GATES), ci: invocationsFor("ci", FIXTURE_GATES) };
  // Each case is a ROOT, read through the same function the exit code is taken from, so what the
  // self-test asserts and what a run of that root reports cannot come apart.
  const run = (name) => check(join(FIXTURES, name), FIXTURE_GATES);
  const results = [];
  const is = (what, actual, wanted) => results.push({ what, ok: actual === wanted, actual, wanted });

  // The roster the fixtures are measured against, so a fixture that stopped being parsed is visible
  // as a count rather than as a case that quietly passes.
  is("the fixture roster projects 3 hook and 3 ci invocations", `${expect.hook.length}/${expect.ci.length}`, "3/3");

  const green = { hook: hookInvocations(readCase("green", "hook")), ci: ciInvocations(readCase("green", "ci")) };
  is("green's hook reads the three hook invocations", green.hook.join(" | "), expect.hook.join(" | "));
  is("green's ci.yml reads the three ci invocations", green.ci.join(" | "), expect.ci.join(" | "));
  is("green: both carriers agree, so the root exits 0", run("green").problems.length, 0);
  is("green: and both were read rather than skipped", run("green").counts.join(", "), "hook 3/3, ci 3/3");

  // Case 1 — a gate missing from a carrier.
  is(
    "a gate missing from the hook names the carrier and both invocations",
    run("missing").problems.join(" / "),
    "hook: expected node scripts/fixture-two.mjs at position 2, found node scripts/fixture-three.mjs --self-test",
  );

  // Case 2 — a gate in a carrier and not in the manifest.
  is(
    "a gate CI runs and the manifest does not is reported at its position",
    run("extra").problems.join(" / "),
    "ci: expected nothing at position 4, found node scripts/fixture-rogue.mjs",
  );

  // Case 3 — a gate in the wrong position. The lengths agree and only the order does not, which is
  // the drift a set comparison cannot see and the reason this gate compares sequences.
  is(
    "two gates in the wrong order are reported at the first of them",
    run("swapped").problems.join(" / "),
    "hook: expected node scripts/fixture-one.mjs at position 1, found node scripts/fixture-two.mjs",
  );
  is("and the swapped hook still reads three invocations", run("swapped").counts.join(", "), "hook 3/3, ci 3/3");

  // Each case reports ONE problem, in ONE carrier: a root whose other carrier is also wrong would
  // make the counts above true for the wrong reason.
  for (const name of ["missing", "extra", "swapped"]) is(`${name} reports exactly one problem`, run(name).problems.length, 1);

  // The silences: what a carrier writes beside its Node gates is not this roster's business.
  is("a cargo run_step is not a Node gate", green.hook.some((i) => i.includes("cargo")), false);
  is("`node --test` outside scripts/ is not a Node gate", green.ci.some((i) => i.startsWith("node --test")), false);
  is("a run: in another job is not read", green.ci.some((i) => i.includes("other-job")), false);
  is("a job that is not there reads as null, not as an empty list", ciInvocations(readCase("green", "ci"), "no-such-job"), null);

  // The manifest's own shape, on the real roster rather than on the fixture's.
  is("every carrier the real roster names is a known one", unknownCarriers(GATES).length, 0);

  const failed = results.filter((r) => !r.ok);
  for (const r of failed) console.error(`  FAIL ${r.what}\n    expected: ${JSON.stringify(r.wanted)}\n    actual:   ${JSON.stringify(r.actual)}`);
  console.log(`gate carriers self-test: ${results.length - failed.length} of ${results.length}`);
  return failed.length === 0;
}

// ---------------------------------------------------------------------------------------------

const args = process.argv.slice(2);
if (args.includes("--self-test")) {
  process.exit((await selfTest()) ? 0 : 1);
}

const rosterAt = args.indexOf("--roster");
const positional = args.filter((a, i) => !a.startsWith("--") && i !== rosterAt + 1);
const gates = rosterAt < 0 ? GATES : (await import(`file://${resolve(args[rosterAt + 1]).replace(/\\/g, "/")}`)).GATES;

const { problems, counts } = check(resolve(positional[0] ?? REPO_ROOT), gates);
console.log(`gate carriers: ${gates.length} rostered gate(s); read ${counts.join(", ")}`);
if (problems.length === 0) {
  console.log("gate carriers: OK (every carrier runs the manifest's projection, in order)");
  process.exit(0);
}

console.error(`\ngate carriers: ${problems.length} carrier(s) disagree with the roster`);
for (const p of problems) console.error(`  ${p}`);
console.error(
  "\nThe roster is scripts/gates.manifest.mjs and the carriers follow it (ADR-0217):\n" +
    "\n" +
    `    hook        ${CARRIER_FILES.hook}\n` +
    `    ci          ${CARRIER_FILES.ci}, the \`${CI_JOB}\` job\n` +
    "    conductor   tools/conductor/lib/gate.mjs, which imports the manifest\n" +
    "    pages       .github/workflows/pages.yml, for the gates that need a built site\n" +
    "\n" +
    "Adding a gate means one manifest entry naming its carriers, plus the invocation in each\n" +
    "file-backed carrier above, in the roster's order. A gate spelled differently per carrier is\n" +
    "two entries with disjoint carrier sets, never one entry with per-carrier arguments.\n" +
    "\n" +
    "Only invocations and their order are checked. An `if:` condition, a step name and the hook's\n" +
    "English skip notice are prose this gate never reads.",
);
process.exit(1);
