#!/usr/bin/env node
// Assert that every row inside a marked roster region is a pointer, not an abstract.
//
// Rationale: three files in docs/ are rosters — adrs/README.md, plans/README.md's
// `## Recently closed`, and the `## Closed entries` ledger in design-backlog.md.
// Each exists so a session can find the right document without opening a hundred
// of them. All three grew rows that summarize the document they point at:
// docs/adrs/README.md reached 188,820 bytes, 16 % of the ADR corpus it indexes,
// with rows 0101-0115 averaging 3,302 bytes against 152 for rows 0001-0020.
//
// The convention alone has already been tried here and already failed. Plan 0061
// Phase 7b moved the plan close write-ups verbatim into README-archive.md and
// wrote "One line per plan." three lines above the rows; eight days later that
// section had regrown 7.1x under its own rule. A rule nothing re-runs is a rule
// nobody follows (ADR-0033), so this is a gate rather than a paragraph.
// See ADR-0116 for the decision and the arithmetic behind the cap.
//
// Usage:  node scripts/check-index-rows.mjs [root]
//         node scripts/check-index-rows.mjs --self-test
// Exit 0 = every measured row is within its region's cap. Exit 1 = the over-cap
// ones are listed as `file:line  N bytes (cap C)`, which is clickable in most
// terminals. The optional `root` scans some other directory — used to run this
// against the committed fixture tree, following check-doc-links.mjs:
// `node scripts/check-index-rows.mjs scripts/fixtures` expects exit 0, because
// that tree's marked rows are all under cap and its one fat row sits OUTSIDE the
// markers. CI and the pre-push hook pass nothing and get the repo.
//
// WHY A `--self-test` EXISTS, AND WHY EXIT 0 IS NOT ENOUGH ON ITS OWN. A byte cap
// convicts nothing on a tree with no fat row in it, so both this gate's roots —
// the fixture and the repository — legitimately exit 0, and a detector that has
// stopped matching anything exits 0 the same way. Replace TABLE_ROW and BULLET
// with regexes that match nothing and the fixture reports `3 regions, 0 rows,
// 0 over cap`, the repository reports the same shape, and all three call sites
// go green. The per-file counts this prints were the mitigation and they are
// PRINTED, not asserted. So `--self-test` asserts them: the fixture's exact
// counts, and a floor on the repository's, which is the number a dead detector
// collapses. Backlog 0104 is the demonstration; ADR-0033's argument is that a
// rule nothing re-runs is a rule nobody follows, and its corollary is that a
// check which re-runs and cannot fail is the same rule wearing a green tick.
//
// A region is delimited by HTML comments, so the markers are invisible in a
// rendered page and survive a table being reflowed:
//
//   <!-- roster:begin cap=320 -->
//   | [0001](0001-....md) | Title | accepted |
//   <!-- roster:end -->
//
// The region rather than the markdown is the unit, which is what lets one
// mechanism and one cap cover three different row syntaxes (an ADR table, plan
// bullets, a ledger table). Inside a region a ROW is a table data row or a list
// bullet; a table's header line and its `|---|` delimiter are structure, not
// rows, and prose and subheadings between two table blocks are measured by
// nothing. A row OUTSIDE every region is not measured at all — which is the
// documented way this gate is defeated (ADR-0116, Negative 3), and the reason
// the per-file region count is printed on success as well as on failure: a
// deleted marker shows up as a region that vanished rather than as silence.

import { readdirSync, readFileSync } from "node:fs";
import { join, dirname, resolve, relative, sep } from "node:path";
import { fileURLToPath } from "node:url";

const REPO_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");

const argv = process.argv.slice(2);
const SELF_TEST = argv.includes("--self-test");
const ROOT_ARG = argv.find((a) => !a.startsWith("--"));
const REPO = resolve(ROOT_ARG ?? REPO_ROOT);

// Build output, vendored deps, and VCS internals hold markdown we do not own.
const SKIP_DIRS = new Set(["target", "node_modules", ".git"]);

// The fixture tree carries this checker's own bite check and is skipped on a
// repo walk, exactly as check-doc-links.mjs skips it; it is scanned when it IS
// the root, which is the only way its rows are reachable. Skipped BY PATH, not
// by directory name — the name form also swallowed core/tests/fixtures/ there.
const SEEDED_TREES = new Set([resolve(REPO_ROOT, "scripts", "fixtures")]);

const DEFAULT_CAP = 320;

const BEGIN = /^\s*<!--\s*roster:begin(?:\s+cap=(\d+))?\s*-->\s*$/;
const END = /^\s*<!--\s*roster:end\s*-->\s*$/;

/** A `|---|:--:|` delimiter: pipes, dashes, colons and space, at least one dash. */
const DELIMITER = /^ {0,3}\|[\s:|-]*-[\s:|-]*$/;

/** A table data row, or a list bullet in any of markdown's three markers. */
const TABLE_ROW = /^ {0,3}\|/;
const BULLET = /^ {0,3}[-*+]\s/;

/**
 * Every `.md` file under the scan root, as paths relative to it.
 *
 * The root is a parameter rather than the module-level `REPO` because
 * `--self-test` measures TWO trees in one process — the fixture and the
 * repository — and a walk closed over one global can only ever answer for the
 * tree the command line named.
 */
function markdownFiles(root, dir = root, found = []) {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const full = join(dir, entry.name);
    if (entry.isDirectory()) {
      if (SKIP_DIRS.has(entry.name)) continue;
      if (SEEDED_TREES.has(full)) continue;
      markdownFiles(root, full, found);
    } else if (entry.name.endsWith(".md")) {
      found.push(relative(root, full));
    }
  }
  return found;
}

/**
 * Rows and structural lines inside one file's marked regions.
 *
 * A table's header is identified by the delimiter beneath it rather than by
 * position, because the ledger holds seven separate table blocks under their own
 * subheadings — "the first table line in the region" would let six headers
 * through as rows and inflate every count this prints.
 */
function scan(root, file) {
  const lines = readFileSync(join(root, file), "utf8").split(/\r?\n/);
  const regions = [];
  const rows = [];
  const errors = [];

  let cap = null;
  let openedAt = 0;
  let inFence = false;

  lines.forEach((line, i) => {
    const lineNo = i + 1;

    // A fence inside a region is an example of a roster, not a roster.
    if (/^\s*(```|~~~)/.test(line)) {
      inFence = !inFence;
      return;
    }
    if (inFence) return;

    const begin = line.match(BEGIN);
    if (begin) {
      if (cap !== null) {
        errors.push(`${file}:${lineNo} roster:begin inside a region opened at line ${openedAt}`);
        return;
      }
      cap = begin[1] ? Number(begin[1]) : DEFAULT_CAP;
      openedAt = lineNo;
      return;
    }

    if (END.test(line)) {
      if (cap === null) {
        errors.push(`${file}:${lineNo} roster:end with no open region`);
        return;
      }
      regions.push({ begin: openedAt, end: lineNo, cap });
      cap = null;
      return;
    }

    if (cap === null) return; // outside every region — deliberately not measured
    if (DELIMITER.test(line)) return;
    if (!TABLE_ROW.test(line) && !BULLET.test(line)) return; // prose, subheading, blank
    // The line above a delimiter is the table's header, not a row.
    if (TABLE_ROW.test(line) && DELIMITER.test(lines[i + 1] ?? "")) return;

    rows.push({ line: lineNo, bytes: Buffer.byteLength(line, "utf8"), cap });
  });

  if (cap !== null) {
    errors.push(`${file}:${openedAt} roster:begin never closed`);
  }

  return { regions, rows, errors };
}

/**
 * Measure every marked region in one tree.
 *
 * `regions` and `rows` are returned as totals rather than only printed, which is
 * the whole mechanism behind `--self-test`: a count nobody compares to an
 * expected number is a count that cannot convict.
 */
function measure(root) {
  const violations = [];
  const errors = [];
  const summary = [];
  const perFile = new Map();
  let regionTotal = 0;
  let rowTotal = 0;

  for (const file of markdownFiles(root).sort()) {
    const show = file.split(sep).join("/");
    const { regions, rows, errors: fileErrors } = scan(root, file);
    errors.push(...fileErrors.map((e) => e.split(sep).join("/")));
    if (regions.length === 0 && fileErrors.length === 0) continue;

    const over = rows.filter((r) => r.bytes > r.cap);
    regionTotal += regions.length;
    rowTotal += rows.length;
    perFile.set(show, { regions: regions.length, rows: rows.length, over: over.length });
    summary.push(
      `  ${show}: ${regions.length} region${regions.length === 1 ? "" : "s"}, ` +
        `${rows.length} rows, ${over.length} over cap`,
    );
    for (const r of over) {
      violations.push(`${show}:${r.line}  ${r.bytes} bytes (cap ${r.cap})`);
    }
  }

  return { violations, errors, summary, perFile, regions: regionTotal, rows: rowTotal };
}

// --- self-test ---------------------------------------------------------------
//
// Two halves, and they fail to different mutations.
//
// The FIXTURE half pins exact numbers, because the fixture tree is committed and
// only changes when someone changes it deliberately. `> 0` would survive a
// detector that matches one row in ten, and 4 rather than 6 is what asserts the
// header and delimiter lines are still structure rather than rows.
//
// The REPOSITORY half is the one backlog 0104's demonstration collapses, and it
// is a FLOOR rather than an equality: these three rosters gain a row at every
// close, so an exact count would be red on the next one and would be raised
// without being read. Measured 2026-09-01: 159 / 123 / 120 rows across 4
// regions. The floors below sit far under those and still go to zero the moment
// TABLE_ROW and BULLET stop matching, which is the only thing they are for.
const ROSTERS = ["docs/adrs/README.md", "docs/plans/README.md", "docs/design-backlog.md"];
const ROSTER_ROW_FLOOR = 20;
const REPO_ROW_FLOOR = 100;

function selfTest() {
  const results = [];
  const record = (label, ok, detail) => results.push({ label, ok, detail });

  const fixture = measure(resolve(REPO_ROOT, "scripts", "fixtures"));
  record(
    "fixture: exactly 3 regions and 4 rows",
    fixture.regions === 3 && fixture.rows === 4,
    `${fixture.regions} regions, ${fixture.rows} rows`,
  );
  record(
    "fixture: nothing over cap and no malformed marker",
    fixture.violations.length === 0 && fixture.errors.length === 0,
    `${fixture.violations.length} over cap, ${fixture.errors.length} malformed`,
  );

  const repo = measure(REPO_ROOT);
  for (const roster of ROSTERS) {
    const seen = repo.perFile.get(roster);
    record(
      `non-vacuity: ${roster}`,
      seen !== undefined && seen.regions >= 1 && seen.rows >= ROSTER_ROW_FLOOR,
      seen
        ? `${seen.regions} regions, ${seen.rows} rows (floor ${ROSTER_ROW_FLOOR})`
        : "measured no region at all",
    );
  }
  record(
    `non-vacuity: the repository measures at least ${REPO_ROW_FLOOR} rows`,
    repo.rows >= REPO_ROW_FLOOR,
    `${repo.rows} rows across ${repo.regions} regions`,
  );

  const width = Math.max(...results.map((r) => r.label.length));
  for (const r of results) {
    const pad = " ".repeat(width - r.label.length + 1);
    console.log(`${r.label}${pad} ${r.ok ? "OK" : "FAILED"} (${r.detail})`);
  }
  const passed = results.filter((r) => r.ok).length;
  console.log(`self-test: ${passed}/${results.length}`);
  if (passed !== results.length) {
    console.error(
      "\nA failing self-test means this gate has stopped measuring, not that a row\n" +
        "is too long. Check TABLE_ROW, BULLET, DELIMITER and the region markers\n" +
        "before touching the numbers above — a detector that matches nothing\n" +
        "reports `0 rows, 0 over cap` and exits 0 at every call site (backlog 0104).",
    );
  }
  process.exit(passed === results.length ? 0 : 1);
}

// --- main --------------------------------------------------------------------

if (SELF_TEST) selfTest();

const { violations, errors, summary } = measure(REPO);

console.log("index rows: regions found");
for (const s of summary) console.log(s);

if (errors.length > 0) {
  console.error(`\nindex rows: ${errors.length} malformed marker(s)`);
  for (const e of errors) console.error(`  ${e}`);
}

if (violations.length === 0 && errors.length === 0) {
  console.log("index rows: OK (every row inside a marked region is within its cap)");
  process.exit(0);
}

if (violations.length > 0) {
  console.error(`\nindex rows: ${violations.length} over cap`);
  for (const v of violations) console.error(`  ${v}`);
  console.error(
    "\nAn index row is a pointer: a link, a title, and a status (ADR-0116).\n" +
      "It is not a place to summarize the document it points at — that prose\n" +
      "already exists one click away, and a second copy is the copy that drifts.\n" +
      "\n" +
      "  ADR roster        | [NNNN](NNNN-slug.md) | <the ADR body's H1> | <status> |\n" +
      "  Closed plans      - [NNNN - Title](done/NNNN-slug.md) - closed <date>. Review: <verdict>\n" +
      "  Backlog ledger    | NNNN | <one-line what> | <where it went> |\n" +
      "\n" +
      "Detail that is not already in the linked document goes INTO that document\n" +
      "(an ADR gets a dated `## Outcome` section; a close write-up goes to\n" +
      "docs/plans/README-archive.md), never into the row.\n" +
      "\n" +
      "If a row genuinely cannot fit, the answer is new arithmetic in ADR-0116 --\n" +
      "not a raised constant, and not a row nudged outside the markers.",
  );
}
process.exit(1);
