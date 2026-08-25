#!/usr/bin/env node
// Assert that the diffusion filter's numbers live in exactly one document.
//
// Rationale: Plan 0106 shipped `tools/sd-filter/` and documented it in three
// files at once — profiles, `--size`, `--stride`, the check and the cost table
// were each written out in full, in different words, in `docs/capturing.md` and
// `tools/sd-filter/README.md`, with a third prose copy of the cost in the
// top-level README. Copies written in different words cannot be diffed, so they
// disagree silently by construction. That is not hypothetical: at Plan 0106's
// close review a correction to the cost figures enumerated two of the three
// copies and missed `tools/sd-filter/README.md`, found only by grepping the
// numerals after the file list had been written down and committed.
//
// So this gate enforces the ABSENCE of copies, not the agreement of copies. A
// same-value check across a known list would have reproduced the miss exactly —
// the copy that broke it was the one outside the list. See ADR-0120.
//
// Usage:  node scripts/check-filter-figures.mjs [root]
// Exit 0 = every cost figure is in the canonical page (or on the one whitelisted
// orientation line, matching a figure there). Exit 1 = the strays are listed as
// `file:line  <figure>`, clickable in most terminals. The optional `root` scans
// some other directory — used to run this against the committed fixture tree,
// following the three checkers beside it:
// `node scripts/check-filter-figures.mjs scripts/fixtures/filter-figures`
// expects exit 1 and five breaks. CI and the pre-push hook pass nothing.
//
// WHAT IS IN SCOPE, and why it is not simply "every file naming the filter".
// `docs/capturing.md` is two thousand lines about the capture tooling that DOES
// ship, and it is full of unrelated timings. Convicting it for a `~150 ms`
// preset-reload figure because the same file mentions the filter elsewhere would
// make the gate useless within a week. So:
//
//   - a file whose PATH names the filter is scanned whole (it is all about it);
//   - any other file is scanned only in those SECTIONS whose text names the
//     filter, a section running from its heading to the next heading of the same
//     or higher level;
//   - fenced code is never scanned — a command line is not a claim;
//   - the canonical page is exempt, and is where the figures are supposed to be.
//
// DATED RECORDS ARE OUT OF SCOPE, and deliberately: plans, ADRs and the backlog
// record what was true when they were written, and are not instructions to a
// reader. ADR-0120 accepts that half explicitly.
//
// THE GATE'S OWN HOLES, named here rather than left to be discovered:
//   1. A figure spelled in words ("roughly fifty-four minutes") is not matched.
//      Digits and the bare-article forms below are. Closing this would convict
//      ordinary prose, and the failure mode being defended against is a table
//      being copied, not a sentence being spelled out.
//   2. The whitelisted orientation line is whitelisted BY LINE. A reflow that
//      pushes its figure onto the next line reddens this gate. That is intended:
//      the figure is supposed to stay on the marked line.

import { readdirSync, readFileSync, existsSync } from "node:fs";
import { join, dirname, resolve, relative, sep } from "node:path";
import { fileURLToPath } from "node:url";

const REPO_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const REPO = resolve(process.argv[2] ?? REPO_ROOT);

const SKIP_DIRS = new Set(["target", "node_modules", ".git"]);

// Skipped BY PATH on a repo walk and scanned when it IS the root, exactly as
// check-doc-links.mjs and check-index-rows.mjs do it. The name form also
// swallowed core/tests/fixtures/ when it was tried there.
const SEEDED_TREES = new Set([resolve(REPO_ROOT, "scripts", "fixtures")]);

/** The one page the figures are allowed to live on, relative to the scan root. */
const CANONICAL = "docs/diffusion-filter.md";

/** Dated records: what was true when written, not instructions to a reader. */
const RECORD_PREFIXES = ["docs/plans/", "docs/adrs/"];
const RECORD_FILES = new Set([
  "docs/design-backlog.md",
  "docs/design-backlog-archive.md",
]);

/** The tool this gate is about, as it is spelled in prose and in paths. */
const NAMES = /sd-filter|sd_filter/i;

const FIGURES_BEGIN = /^\s*<!--\s*figures:begin\s*-->\s*$/;
const FIGURES_END = /^\s*<!--\s*figures:end\s*-->\s*$/;

/** The single allowed orientation figure outside the canonical page. */
const ORIENTATION = /<!--\s*figures:orientation\s*-->/;
const ORIENTATION_FILE = "README.md";

/**
 * A cost figure: digits, then a time or memory unit.
 *
 * Longest alternative first, so `sec` is not consumed as `s` and then rejected.
 * The lookbehind keeps the `0` of `2.6.0+cu124` and the `8` of `1080` from
 * starting a match.
 */
const NUM_UNIT =
  /(?<![\w.])~?(\d+(?:[.,]\d+)?)\s*(seconds?|secs?|minutes?|mins?|hours?|hrs?|days?|weeks?|ms|s|[KMG]iB|[KMG]B)\b/g;

/** Durations written without a numeral, which is how the third copy was worded. */
const BARE_DURATION =
  /\b(?:an?\s+(?:hour|afternoon|evening|night|day|week)|half\s+an\s+hour|overnight)\b/gi;

/** `54 minutes`, `~54 minutes` and `**54 minutes**` are one figure, not three. */
function normalize(num, unit) {
  const u = unit.toLowerCase().replace(/s$/, "");
  const canon =
    { sec: "second", min: "minute", hr: "hour", h: "hour" }[u] ?? u;
  return `${Number(String(num).replace(",", "."))}${canon}`;
}

function figuresIn(text) {
  const found = [];
  for (const m of text.matchAll(NUM_UNIT)) {
    found.push({ raw: m[0].trim(), key: normalize(m[1], m[2]) });
  }
  for (const m of text.matchAll(BARE_DURATION)) {
    found.push({ raw: m[0].trim(), key: m[0].trim().toLowerCase() });
  }
  return found;
}

/** Every `.md` under the scan root, as paths relative to it, in `/` form. */
function markdownFiles(dir = REPO, found = []) {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const full = join(dir, entry.name);
    if (entry.isDirectory()) {
      if (SKIP_DIRS.has(entry.name)) continue;
      if (SEEDED_TREES.has(full)) continue;
      markdownFiles(full, found);
    } else if (entry.name.endsWith(".md")) {
      found.push(relative(REPO, full).split(sep).join("/"));
    }
  }
  return found;
}

/** Lines outside fenced code, as `{ line, text }`. */
function proseLines(file) {
  const out = [];
  let inFence = false;
  readFileSync(join(REPO, file), "utf8")
    .split(/\r?\n/)
    .forEach((text, i) => {
      if (/^\s*(```|~~~)/.test(text)) {
        inFence = !inFence;
        return;
      }
      if (!inFence) out.push({ line: i + 1, text });
    });
  return out;
}

/**
 * The prose lines this gate looks at in one file.
 *
 * The whole file when its path names the filter; otherwise only the sections
 * whose own text does. A section runs from its heading to the next heading of
 * the same or higher level, so a `####` about the filter does not drag in the
 * `###` that follows it.
 *
 * "Its own text" means the lines directly under the heading, NOT those of its
 * subsections. Counting descendants would make every ancestor a match, and the
 * document's `# H1` would put the whole file in scope — which is exactly what
 * this gate must not do to a two-thousand-line document about other tooling.
 */
function scannedLines(file) {
  const lines = proseLines(file);
  if (NAMES.test(file)) return lines;

  const sections = [];
  const open = [];
  for (const entry of lines) {
    const heading = entry.text.match(/^(#{1,6})\s/);
    if (heading) {
      const level = heading[1].length;
      while (open.length > 0 && open[open.length - 1].level >= level) open.pop();
      const section = { level, own: [], all: [] };
      sections.push(section);
      open.push(section);
    }
    if (open.length === 0) continue; // preamble, before any heading
    open[open.length - 1].own.push(entry);
    for (const s of open) s.all.push(entry);
  }

  const seen = new Set();
  const out = [];
  for (const s of sections) {
    if (!NAMES.test(s.own.map((l) => l.text).join(" "))) continue;
    for (const entry of s.all) {
      if (seen.has(entry.line)) continue;
      seen.add(entry.line);
      out.push(entry);
    }
  }
  return out.sort((a, b) => a.line - b.line);
}

// ---------------------------------------------------------------- the canonical page

const errors = [];
const violations = [];
let canonicalKeys = new Set();

if (!existsSync(join(REPO, CANONICAL))) {
  errors.push(`${CANONICAL} does not exist — the canonical page is the whole mechanism`);
} else {
  const lines = readFileSync(join(REPO, CANONICAL), "utf8").split(/\r?\n/);
  const regions = [];
  let openedAt = null;
  lines.forEach((text, i) => {
    if (FIGURES_BEGIN.test(text)) {
      if (openedAt !== null) errors.push(`${CANONICAL}:${i + 1} figures:begin inside a region`);
      else openedAt = i + 1;
    } else if (FIGURES_END.test(text)) {
      if (openedAt === null) errors.push(`${CANONICAL}:${i + 1} figures:end with no open region`);
      else {
        regions.push(lines.slice(openedAt, i).join("\n"));
        openedAt = null;
      }
    }
  });
  if (openedAt !== null) errors.push(`${CANONICAL}:${openedAt} figures:begin never closed`);
  if (regions.length !== 1) {
    errors.push(
      `${CANONICAL} has ${regions.length} figures region(s); expected exactly 1`,
    );
  }
  canonicalKeys = new Set(regions.flatMap((r) => figuresIn(r).map((f) => f.key)));
  if (regions.length === 1 && canonicalKeys.size === 0) {
    errors.push(`${CANONICAL} figures region holds no figure at all`);
  }
}

// ---------------------------------------------------------------- everything else

const files = markdownFiles()
  .filter((f) => f !== CANONICAL)
  .filter((f) => !RECORD_FILES.has(f))
  .filter((f) => !RECORD_PREFIXES.some((p) => f.startsWith(p)))
  .sort();

let orientationLines = 0;
const scanned = [];

for (const file of files) {
  const lines = scannedLines(file);
  if (lines.length === 0) continue;
  scanned.push(`  ${file}: ${lines.length} prose line(s) in scope`);

  for (const { line, text } of lines) {
    const figures = figuresIn(text);
    if (figures.length === 0) continue;

    if (ORIENTATION.test(text)) {
      orientationLines += 1;
      if (file !== ORIENTATION_FILE) {
        violations.push(
          `${file}:${line}  the orientation whitelist is only for ${ORIENTATION_FILE}`,
        );
        continue;
      }
      for (const f of figures) {
        if (!canonicalKeys.has(f.key)) {
          violations.push(
            `${file}:${line}  orientation figure "${f.raw}" is in no figures region of ${CANONICAL}`,
          );
        }
      }
      continue;
    }

    for (const f of figures) {
      violations.push(`${file}:${line}  ${f.raw}`);
    }
  }
}

if (orientationLines > 1) {
  violations.push(
    `${orientationLines} orientation lines carry figures; ADR-0120 allows exactly one`,
  );
}

// ---------------------------------------------------------------- the report

console.log("filter figures: files in scope");
for (const s of scanned) console.log(s);
console.log(
  `  ${CANONICAL}: canonical, ${canonicalKeys.size} figure(s) in its marked region`,
);

if (errors.length > 0) {
  console.error(`\nfilter figures: ${errors.length} structural problem(s)`);
  for (const e of errors) console.error(`  ${e}`);
}

if (violations.length === 0 && errors.length === 0) {
  console.log("filter figures: OK (every cost figure is in the canonical page)");
  process.exit(0);
}

if (violations.length > 0) {
  console.error(`\nfilter figures: ${violations.length} figure(s) outside the canonical page`);
  for (const v of violations) console.error(`  ${v}`);
  console.error(
    "\nThe diffusion filter's numbers live in docs/diffusion-filter.md and\n" +
      "nowhere else (ADR-0120). Every other mention is a pointer that carries no\n" +
      "figure of its own, so a correction has exactly one place to land.\n" +
      "\n" +
      "  - move the figure into the <!-- figures:begin --> region of the\n" +
      "    canonical page, and leave a link where it was; or\n" +
      "  - if it is genuinely the orientation figure, it goes on README.md's one\n" +
      "    <!-- figures:orientation --> line and must match a figure in that region.\n" +
      "\n" +
      "This gate checks that duplicates do NOT EXIST, not that they agree: the\n" +
      "copy that broke this last time was the one nobody had enumerated.",
  );
}
process.exit(1);
