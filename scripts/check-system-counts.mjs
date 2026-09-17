#!/usr/bin/env node
// Assert that no sentence writes down how many systems the roster has.
//
// `CLAUDE.md` has always asked for count-free phrasing — "the whole embedded
// set" over a hard number — and the convention alone did not hold it. Two
// systems landed three days apart and each close found sentences carrying the
// old total: "all twelve" in a reader page, "All twelve systems have one" in
// another, and five more in `core/`, one of them inside an assertion MESSAGE
// rather than a comment. Every one was found by eye, and one a close late.
//
// So the convention takes a gate, which is the third time that substitution has
// been made here for the same reason (ADR-0116, ADR-0149, and now ADR-0202).
//
// Usage:  node scripts/check-system-counts.mjs [root]
// Exit 0 = nothing writes the count down. Exit 1 = each instance is listed as
// `file:line  <matched text>`, clickable in most terminals. The optional `root`
// scans some other directory, following the checkers beside it:
// `node scripts/check-system-counts.mjs scripts/fixtures/system-counts` expects
// exit 1 and the break count that tree's README states. CI and the pre-push hook
// pass nothing.
//
// WHY A COUNT IS REJECTED EVEN WHEN IT IS RIGHT. A true "fourteen systems" is
// wrong the day the fifteenth lands, so a gate that compared prose against the
// live roster size would let through exactly the sentences it exists to stop,
// and would fail them on the close that adds a system — the close that has
// already missed them twice. The number is the defect, not its value.
//
// THE THRESHOLD IS FIVE, AND IT IS ARGUED RATHER THAN DERIVED. It sits between
// the largest legitimate count in this repository's prose on 2026-09-14 (four —
// "the four quadrants", "two passes") and the smallest stale roster total
// (seven). ADR-0202 records it, so moving it is a change to that decision and
// not to this file.
//
// THE GATE'S OWN HOLES, named here rather than left to be discovered:
//   1. It rejects one noun. "Seven Node gates", "112 presets" and "five zips"
//      go stale identically and are unguarded; Plan 0178 made the gate
//      inventory count-free by hand instead. Extending the grammar is its own
//      decision.
//   2. A count spelled another way escapes it: "a dozen systems", a gap of
//      three words, or a count in a table cell whose noun is in the header.
//   3. Fenced code in markdown is not scanned. A command is not a claim.

import { readFileSync, readdirSync } from "node:fs";
import { join, dirname, resolve, relative, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { execFileSync } from "node:child_process";

const REPO_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const REPO = resolve(process.argv[2] ?? REPO_ROOT);

// Only the fallback walk below reaches these; git's enumeration excludes them
// without being told to.
const SKIP_DIRS = new Set(["target", "node_modules", ".git"]);

// The fixture tree carries this checker's own bite check and is skipped on a
// repo walk, exactly as its siblings skip theirs. Skipped BY PATH, not by
// directory name. It is scanned when it IS the root, which is the only way its
// seeded findings are reachable.
const SEEDED_TREES = new Set([resolve(REPO_ROOT, "scripts", "fixtures")]);

// Which extensions carry prose a reader can be misled by. `.rs` is scanned
// WHOLE — comments and string literals alike — because the instance that
// survived both closes was an assertion message, which no comment lexer sees.
const EXTENSIONS = new Set([".md", ".rs", ".ts", ".tsx"]);

// THE DATED RECORDS ARE OUT, and this is the scope boundary ADR-0202 draws. A
// plan or an ADR states what was true on its date and an ADR is append-only, so
// a count inside one is a measurement rather than a claim about today. The
// backlog is the same: an entry records what its probe saw when it was written.
const EXCLUDED = [
  "docs/plans/",
  "docs/adrs/",
  "docs/design-backlog.md",
  "docs/design-backlog-archive.md",
];

/** Whether this gate reads a path at all: the right extension, outside the dated records. */
function isScanned(path) {
  const slashed = path.split(sep).join("/");
  if (slashed.includes("node_modules/")) return false;
  if (EXCLUDED.some((prefix) => slashed === prefix || slashed.startsWith(prefix))) return false;
  const dot = slashed.lastIndexOf(".");
  return dot >= 0 && EXTENSIONS.has(slashed.slice(dot));
}

/** Whether an absolute path sits inside a seeded tree that is not the scan root. */
function inSeededTree(abs) {
  for (const tree of SEEDED_TREES) {
    if (REPO === tree || REPO.startsWith(tree + sep)) continue; // the root is that tree
    if (abs === tree || abs.startsWith(tree + sep)) return true;
  }
  return false;
}

/**
 * Every file the REPOSITORY holds under the root.
 *
 * Enumerated from git, not from the filesystem, so "prose we own" and "prose
 * this gate judges" are the same set by construction: a vendored tree that is
 * gitignored is absent from CI's fresh clone and present in every working tree,
 * and a gate that fires on code nobody here wrote teaches its users to skip it.
 * The filesystem walk stays as the fallback for a tree git cannot answer for.
 */
function proseFiles() {
  try {
    const out = execFileSync("git", ["ls-files", "-z"], {
      cwd: REPO,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "ignore"],
      maxBuffer: 64 * 1024 * 1024,
    });
    const found = [];
    for (const path of out.split("\0")) {
      if (!path) continue;
      if (!isScanned(path)) continue;
      if (inSeededTree(resolve(REPO, path))) continue;
      found.push(path);
    }
    return { files: found, source: "git" };
  } catch {
    return { files: walk(), source: "filesystem" };
  }
}

function walk(dir = REPO, found = []) {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const full = join(dir, entry.name);
    if (entry.isDirectory()) {
      if (SKIP_DIRS.has(entry.name)) continue;
      if (inSeededTree(full)) continue;
      walk(full, found);
    } else {
      const path = relative(REPO, full);
      if (isScanned(path)) found.push(path);
    }
  }
  return found;
}

// A count token: an English number word from five to thirty, or a numeral from 5
// to 99. Below five the forms are all legitimate — a singular, a pair, "the four
// quadrants" — and above 99 nothing in this repository counts systems.
const WORDS = [
  "thirty",
  "twenty",
  "nineteen",
  "eighteen",
  "seventeen",
  "sixteen",
  "fifteen",
  "fourteen",
  "thirteen",
  "twelve",
  "eleven",
  "ten",
  "nine",
  "eight",
  "seven",
  "six",
  "five",
];
const UNITS = "one|two|three|four|five|six|seven|eight|nine";
const COUNT = String.raw`(?:${WORDS.join("|")})(?:[- ](?:${UNITS}))?|(?:[5-9]|[1-9][0-9])`;

// At most two words between the count and the noun, which is what carries "the
// other ten systems" and "twelve built-in scene systems" while stopping the gap
// from swallowing a whole sentence. Punctuation ends it by construction: a
// comma is not part of a word, so "seven, and the systems" never matches.
const GAP = String.raw`(?:[ \t]+[A-Za-z][A-Za-z'’-]*){0,2}`;

// The singular possessive is excluded: `seven of its system's twelve params`
// counts params, not systems, and the roster is never written that way. The
// plural possessive — `the twelve systems' palettes` — is a roster count and
// stays matched, which is why the exclusion names `system's` alone.
const SYSTEM_COUNT = new RegExp(
  String.raw`\b(?:${COUNT})\b${GAP}[ \t]+systems?\b(?!['’]s)`,
  "gi",
);

// `file system`, `operating system` and `build system` are a different noun that
// happens to share a word. They are excluded by what sits immediately before it,
// so "seven file systems" is not a roster count either.
const OTHER_SYSTEMS = /\b(?:file|operating|build|ecosystem|sub)$/i;

// `count-allow: <reason>` on the line suppresses it — an HTML comment in
// markdown, an ordinary comment in source. The reason is what makes it an escape
// rather than an off switch, so a marker without one is itself a finding. Same
// shape as ADR-0127's `hygiene-allow:`.
const ESCAPE = /count-allow:\s*(\S.*)?$/;

/** Blank out fenced code, preserving offsets so a match keeps its real line. */
function maskFences(src) {
  let inFence = false;
  return src
    .split("\n")
    .map((line) => {
      if (/^\s*(```|~~~)/.test(line)) {
        inFence = !inFence;
        return " ".repeat(line.length);
      }
      return inFence ? " ".repeat(line.length) : line;
    })
    .join("\n");
}

const findings = [];
const { files, source } = proseFiles();

for (const path of files) {
  let src;
  try {
    src = readFileSync(join(REPO, path), "utf8");
  } catch {
    continue;
  }
  const scanned = path.endsWith(".md") ? maskFences(src) : src;
  const lines = src.split("\n");

  for (const m of scanned.matchAll(SYSTEM_COUNT)) {
    const line = scanned.slice(0, m.index).split("\n").length;
    const text = lines[line - 1] ?? "";

    // The word immediately before the noun decides whether this is the roster's
    // "systems" or another one that shares the word.
    const head = m[0].slice(0, m[0].toLowerCase().lastIndexOf("system")).trimEnd();
    if (OTHER_SYSTEMS.test(head)) continue;

    const escape = text.match(ESCAPE);
    if (escape) {
      if (!escape[1]) {
        findings.push({ path, line, text: "count-allow with no reason given", full: text });
      }
      continue;
    }
    findings.push({ path, line, text: m[0].replace(/\s+/g, " "), full: text });
  }
}

console.log(
  `system counts: ${files.length} file(s) scanned, enumerated from the ${source}`,
);

if (findings.length === 0) {
  console.log("system counts: OK (no sentence writes the roster's size down)");
  process.exit(0);
}

console.error(`\nsystem counts: ${findings.length} written count(s) of the systems`);
for (const f of findings) {
  console.error(`  ${f.path}:${f.line}  ${f.text}`);
  const shown = f.full.trim();
  console.error(`    ${shown.length > 96 ? shown.slice(0, 93) + "…" : shown}`);
}
console.error(
  "\nThe roster's size is not written down outside a dated record (ADR-0202):\n" +
    "\n" +
    "    Say what the set is, not how many are in it.\n" +
    "\n" +
    "A count goes stale whether or not it is right today, and the sentence almost\n" +
    "always reads better without it.\n" +
    "\n" +
    "  before:  All twelve systems have a palette.\n" +
    "  after:   Every system has a palette.\n" +
    "\n" +
    "  before:  the other ten systems take exactly the path they took before\n" +
    "  after:   every other system takes exactly the path it took before\n" +
    "\n" +
    "Where the number is genuinely the point — a dated record inside a live\n" +
    "document, or a figure that is not a roster count at all — put\n" +
    "`count-allow: <why>` on the line, as an HTML comment in markdown. The reason\n" +
    "is reviewed rather than checked, so write the one you would defend.\n" +
    "\n" +
    "Plans, ADRs and the design backlog are out of scope: they state what was true\n" +
    "on their date.",
);
process.exit(1);
