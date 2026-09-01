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
//
// TWO THINGS ARE MEASURED PER ROW, AND THEY ARE DIFFERENT ASSERTIONS. Bytes is
// the cap. SHAPE is the check that a row's form matches its region's: each
// region takes its kind from its first measured row and a row of the other kind
// is reported, naming the form that was expected. docs/plans/README.md holds one
// region of each kind — the active roster is a table, `## Recently closed` is a
// bullet list — and a closed-plan bullet dropped into the table region is a
// BULLET inside a region and under cap, so a length check alone reports it as
// `0 over cap` and exits 0. That is not hypothetical: it happened at a close, in
// the one file every session opens first, and both this gate and the link gate
// waved it through (backlog 0166). A 200-byte bullet in a table region is under
// cap and still wrong, so the shape check sits BESIDE the length one rather than
// replacing it.

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

// The fixture trees carry this checker's own bite checks and are skipped on a
// repo walk, exactly as check-doc-links.mjs skips them; each is scanned when it
// IS the root, which is the only way its rows are reachable. Skipped BY PATH,
// not by directory name — the name form also swallowed core/tests/fixtures/ there.
//
// The RED tree is on this list for a second reason, and it is the one that bites:
// it sits INSIDE the green tree, whose root is `scripts/fixtures`, so without the
// skip the green run would walk it and inherit its over-cap row — turning the
// green fixture's exact counts into 2 regions and 6 rows and its exit code into 1.
// The two roots have to stay separable, because one asserts silence and the other
// asserts a conviction.
const RED_FIXTURE = resolve(REPO_ROOT, "scripts", "fixtures", "index-rows-red");
const SEEDED_TREES = new Set([resolve(REPO_ROOT, "scripts", "fixtures"), RED_FIXTURE]);

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
 *
 * A region's KIND is inferred rather than declared on the marker or hardcoded
 * per file. Inferred, because a region that holds both forms deliberately is a
 * real possibility elsewhere and the marker syntax should not have to grow a
 * second attribute to describe one that does not; per-region, because the two
 * regions in docs/plans/README.md are different kinds and a per-file
 * expectation could not tell them apart.
 *
 * THE KIND IS THE MAJORITY FORM, NOT THE FIRST ROW'S, and the difference is the
 * whole diagnostic value of the check. The observed instance put a closed-plan
 * bullet immediately under `roster:begin` and ABOVE the table header, because
 * the insertion anchored on a string rather than on a section - so the stray row
 * is the one a first-row rule adopts as the region's form, and the thirteen real
 * table rows below it become the finding. Measured on that seeded tree: 14
 * reported, and not one of them the mistake. Majority reports the one row.
 *
 * A region split evenly between the two forms is reported ONCE, at the region,
 * naming both counts. There is no majority to appeal to there and guessing which
 * half is wrong would be the same misdiagnosis in a quieter form.
 */
function scan(root, file) {
  const lines = readFileSync(join(root, file), "utf8").split(/\r?\n/);
  const regions = [];
  const rows = [];
  const errors = [];
  const shapes = [];

  let cap = null;
  let openedAt = 0;
  let inFence = false;
  let inRegion = []; // this region's rows so far; the form is judged when it closes

  /** Judge one closed region's rows against the form most of them have. */
  const judge = (begin, end) => {
    const table = inRegion.filter((r) => r.kind === "table");
    const bullet = inRegion.filter((r) => r.kind === "bullet");
    if (table.length === 0 || bullet.length === 0) return;

    if (table.length === bullet.length) {
      shapes.push(
        `${file}:${begin}  the region opened here holds ${table.length} table row(s) and ` +
          `${bullet.length} bullet row(s), and a region is one form ` +
          `(no majority to report against; it closes at line ${end})`,
      );
      return;
    }

    const majority = table.length > bullet.length ? "table" : "bullet";
    const odd = majority === "table" ? bullet : table;
    for (const r of odd) {
      shapes.push(
        `${file}:${r.line}  a ${r.kind} row in a ${majority} region ` +
          `(expected a ${majority} row; ${Math.max(table.length, bullet.length)} of the ` +
          `${inRegion.length} rows in this region have that form)`,
      );
    }
  };

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
      inRegion = [];
      return;
    }

    if (END.test(line)) {
      if (cap === null) {
        errors.push(`${file}:${lineNo} roster:end with no open region`);
        return;
      }
      judge(openedAt, lineNo);
      regions.push({ begin: openedAt, end: lineNo, cap });
      cap = null;
      inRegion = [];
      return;
    }

    if (cap === null) return; // outside every region — deliberately not measured
    if (DELIMITER.test(line)) return;
    if (!TABLE_ROW.test(line) && !BULLET.test(line)) return; // prose, subheading, blank
    // The line above a delimiter is the table's header, not a row.
    if (TABLE_ROW.test(line) && DELIMITER.test(lines[i + 1] ?? "")) return;

    const row = {
      line: lineNo,
      bytes: Buffer.byteLength(line, "utf8"),
      cap,
      kind: TABLE_ROW.test(line) ? "table" : "bullet",
    };
    inRegion.push(row);
    rows.push(row);
  });

  if (cap !== null) {
    errors.push(`${file}:${openedAt} roster:begin never closed`);
  }

  return { regions, rows, errors, shapes };
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
  const misshaped = [];
  const summary = [];
  const perFile = new Map();
  let regionTotal = 0;
  let rowTotal = 0;

  for (const file of markdownFiles(root).sort()) {
    const show = file.split(sep).join("/");
    const { regions, rows, errors: fileErrors, shapes } = scan(root, file);
    errors.push(...fileErrors.map((e) => e.split(sep).join("/")));
    misshaped.push(...shapes.map((e) => e.split(sep).join("/")));
    if (regions.length === 0 && fileErrors.length === 0) continue;

    const over = rows.filter((r) => r.bytes > r.cap);
    regionTotal += regions.length;
    rowTotal += rows.length;
    perFile.set(show, {
      regions: regions.length,
      rows: rows.length,
      over: over.length,
      misshaped: shapes.length,
    });
    summary.push(
      `  ${show}: ${regions.length} region${regions.length === 1 ? "" : "s"}, ` +
        `${rows.length} rows, ${over.length} over cap, ${shapes.length} misshaped`,
    );
    for (const r of over) {
      violations.push(`${show}:${r.line}  ${r.bytes} bytes (cap ${r.cap})`);
    }
  }

  return {
    violations,
    errors,
    misshaped,
    summary,
    perFile,
    regions: regionTotal,
    rows: rowTotal,
  };
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
// The RED half is the third, and it is the only one that runs the reporting path
// at all: `file:line  N bytes (cap C)`, two spaces before the count, which is
// what makes the line clickable in a terminal. Nothing in the repository and
// nothing in the green fixture has ever reached that formatting, so it is
// asserted by SHAPE rather than by an exit code — "exits non-zero" is also what
// a crash and a thrown ENOENT look like.
const ROSTERS = ["docs/adrs/README.md", "docs/plans/README.md", "docs/design-backlog.md"];
const ROSTER_ROW_FLOOR = 20;
const REPO_ROW_FLOOR = 100;
const REPORT_SHAPE = /^[\w./-]+\.md:\d+ {2}\d+ bytes \(cap \d+\)$/;

// The shape report has to name the form it EXPECTED, not only the one it found —
// a message reading "wrong kind of row" sends its reader to the cap. Both halves
// are pinned here, and the red fixture's third region is the silence beside it:
// the byte-identical bullet in a bullet region must not be reported at all.
const SHAPE_REPORT = /^[\w./-]+\.md:\d+ {2}a (table|bullet) row in a (table|bullet) region \(expected a (table|bullet) row/;
const TIE_REPORT = /^[\w./-]+\.md:\d+ {2}the region opened here holds \d+ table row\(s\) and \d+ bullet row\(s\)/;

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
    "fixture: nothing over cap, misshaped, or malformed",
    fixture.violations.length === 0 &&
      fixture.errors.length === 0 &&
      fixture.misshaped.length === 0,
    `${fixture.violations.length} over cap, ${fixture.misshaped.length} misshaped, ` +
      `${fixture.errors.length} malformed`,
  );

  const red = measure(RED_FIXTURE);
  record(
    "red fixture: exactly 4 regions, 8 rows, 1 over cap, 2 misshaped",
    red.regions === 4 && red.rows === 8 && red.violations.length === 1 && red.misshaped.length === 2,
    `${red.regions} regions, ${red.rows} rows, ${red.violations.length} over cap, ` +
      `${red.misshaped.length} misshaped`,
  );
  record(
    "red fixture: the over-cap report reads `file:line  N bytes (cap C)`",
    REPORT_SHAPE.test(red.violations[0] ?? ""),
    red.violations[0] ?? "nothing reported",
  );
  record(
    "red fixture: the shape report names the form it expected",
    SHAPE_REPORT.test(red.misshaped[0] ?? ""),
    red.misshaped[0] ?? "nothing reported",
  );
  record(
    "red fixture: a region with no majority is reported once, at the region",
    TIE_REPORT.test(red.misshaped[1] ?? ""),
    red.misshaped[1] ?? "nothing reported",
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

const { violations, errors, misshaped, summary } = measure(REPO);

console.log("index rows: regions found");
for (const s of summary) console.log(s);

if (errors.length > 0) {
  console.error(`\nindex rows: ${errors.length} malformed marker(s)`);
  for (const e of errors) console.error(`  ${e}`);
}

if (violations.length === 0 && errors.length === 0 && misshaped.length === 0) {
  console.log(
    "index rows: OK (every row inside a marked region is within its cap and matches its region)",
  );
  process.exit(0);
}

if (misshaped.length > 0) {
  console.error(`\nindex rows: ${misshaped.length} row(s) of the wrong shape for their region`);
  for (const m of misshaped) console.error(`  ${m}`);
  console.error(
    "\nA region's rows are all one form, and the form is read off the rows\n" +
      "themselves — the majority — rather than declared on the marker. This is a\n" +
      "SHAPE break, not a length one: the row above is under its cap and still\n" +
      "belongs in a different list.\n" +
      "\n" +
      "docs/plans/README.md is where this bites, because its two regions are\n" +
      "different kinds — the active roster is a table, `## Recently closed` is a\n" +
      "bullet list — and a close ceremony rewrites both. A closed-plan bullet that\n" +
      "lands in the table region reads as an active plan, resolves as a link, and\n" +
      "sits under cap, so nothing else in the toolchain reports it.\n" +
      "\n" +
      "  table region      | [NNNN](NNNN-slug.md) | <title> | <status> |\n" +
      "  bullet region     - [NNNN - Title](done/NNNN-slug.md) - closed <date>. Review: <verdict>\n" +
      "\n" +
      "Move the row into the region whose form it has. A region split evenly\n" +
      "between the two forms is reported at its own opening line instead, with\n" +
      "both counts: there is no majority to appeal to, and guessing which half is\n" +
      "wrong would send its reader to the wrong rows.",
  );
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
