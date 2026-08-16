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
// Exit 0 = every measured row is within its region's cap. Exit 1 = the over-cap
// ones are listed as `file:line  N bytes (cap C)`, which is clickable in most
// terminals. The optional `root` scans some other directory — used to run this
// against the committed fixture tree, following check-doc-links.mjs:
// `node scripts/check-index-rows.mjs scripts/fixtures` expects exit 0, because
// that tree's marked rows are all under cap and its one fat row sits OUTSIDE the
// markers. CI and the pre-push hook pass nothing and get the repo.
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
const REPO = resolve(process.argv[2] ?? REPO_ROOT);

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

/** Every `.md` file under the scan root, as paths relative to it. */
function markdownFiles(dir = REPO, found = []) {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const full = join(dir, entry.name);
    if (entry.isDirectory()) {
      if (SKIP_DIRS.has(entry.name)) continue;
      if (SEEDED_TREES.has(full)) continue;
      markdownFiles(full, found);
    } else if (entry.name.endsWith(".md")) {
      found.push(relative(REPO, full));
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
function scan(file) {
  const lines = readFileSync(join(REPO, file), "utf8").split(/\r?\n/);
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

const files = markdownFiles();
const violations = [];
const errors = [];
const summary = [];

for (const file of files.sort()) {
  const show = file.split(sep).join("/");
  const { regions, rows, errors: fileErrors } = scan(file);
  errors.push(...fileErrors.map((e) => e.split(sep).join("/")));
  if (regions.length === 0 && fileErrors.length === 0) continue;

  const over = rows.filter((r) => r.bytes > r.cap);
  summary.push(
    `  ${show}: ${regions.length} region${regions.length === 1 ? "" : "s"}, ` +
      `${rows.length} rows, ${over.length} over cap`,
  );
  for (const r of over) {
    violations.push(`${show}:${r.line}  ${r.bytes} bytes (cap ${r.cap})`);
  }
}

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
