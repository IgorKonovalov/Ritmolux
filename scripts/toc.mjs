#!/usr/bin/env node
// Regenerate the contents block in every long markdown document from the
// headings beneath it.
//
// Rationale: six documents totalling 26,500 lines had no table of contents, and
// the only instrument for finding anything inside one was find-in-page. See
// ADR-0163. The block is generated rather than hand-written for the same reason
// every other roster in this repo is gated: a hand-maintained contents list
// drifts from the headings silently, and a stale row reads exactly like a live
// one.
//
// Usage:  node scripts/toc.mjs [root]           rewrite every block in the tree
//         node scripts/toc.mjs --check [root]   rewrite nothing; exit 1 on drift
//         node scripts/toc.mjs --self-test      run the fixture assertions
//
// Same argument shape as check-doc-links.mjs and check-backlog-claims.mjs,
// deliberately. Exit 0 = every block agrees with its headings. Exit 1 = each
// stale or malformed block is listed as `file:line  reason`, which is clickable
// in most terminals.
//
// A block is the lines between `<!-- toc:begin depth=N -->` and
// `<!-- toc:end -->`. Its rows come from the headings AFTER it, at levels 2
// through N, up to the next `toc:begin` or the end of the file. A file with no
// markers is not touched; a marker pair with no headings after it gets an empty
// block rather than an error; an UNPAIRED marker is reported and its file is
// left alone, because the alternative — treating the rest of the file as block
// body — would delete a document on a typo.
//
// `depth` is per-document because the corpus is not uniform. The two backlog
// files repeat `### Priority`, `### The finding` and `### What a fix would be`
// under every entry, so at depth=3 their blocks would be hundreds of rows in
// which the same six titles alternate; they take depth=2 and get one row per
// entry. The manuals take depth=3 and get one row per section.
//
// THE ANCHOR ALGORITHM IS THE WHOLE THING, AND NOTHING DOWNSTREAM CHECKS IT.
// check-doc-links.mjs validates paths and deliberately never validates
// fragments, so an anchor that is merely plausible ships silently and every row
// in every block is wrong together. It is therefore pinned to evidence rather
// than to a specification — two anchors this repository already links, asserted
// in --self-test against the real tree:
//
//   `--render`: a music video from a track   -> --render-a-music-video-from-a-track
//   Seeded randomness — `hash`, `noise`,     -> seeded-randomness--hash-noise-and-
//   and `[generator] seed`                      generator-seed
//
// Between them they fix backtick stripping, colon removal, and the DOUBLED
// HYPHEN an em-dash leaves when it is removed from between two spaces. That
// doubling is the counter-intuitive half and the reason a hand-written anchor
// is usually wrong.
//
// The rule is GitHub's: flatten links to their text, lowercase, drop every
// character that is not a letter, a digit, `_` or `-`, then turn each remaining
// space into a hyphen. Note what is KEPT: `_` survives, so `reaction_diffusion`
// anchors as itself — this corpus is full of snake_case identifiers in headings
// and a rule that stripped `_` as emphasis would break every one of them.
// Backticks, `*` and `~~` need no special case; they are punctuation and the
// same filter removes them.
//
// Repeated heading text dedupes with `-1`, `-2`, … on the second and later
// occurrence, which is GitHub's rule and is load-bearing rather than
// theoretical: the two backlog files carry six repeated heading texts and the
// archive eight.
//
// Holes, named:
//   - A heading shape nobody has written yet is not covered. The two pinned
//     anchors and the fixture cover the shapes this corpus actually contains.
//   - Fragments stay unchecked everywhere else (ADR-0149); this script emits
//     anchors, it does not validate inbound ones.
//   - Markers inside a fenced code block are ignored, because a document that
//     DESCRIBES this syntax is not carrying a block. Plan 0151's own Data
//     shapes section is the first such document.

import { readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, dirname, resolve, relative, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { execFileSync } from "node:child_process";

const REPO_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");

const args = process.argv.slice(2);
const CHECK = args.includes("--check");
const SELF_TEST = args.includes("--self-test");
const positional = args.filter((a) => !a.startsWith("--"));
const REPO = resolve(positional[0] ?? REPO_ROOT);

// Build output, vendored deps, and VCS internals hold markdown we do not own.
const SKIP_DIRS = new Set(["target", "node_modules", ".git"]);

// The seeded tree carries blocks that exist to be measured by --self-test. It is
// skipped on an ordinary repo walk and scanned when it IS the root, which is the
// same shape check-doc-links.mjs uses and for the same reason: without the skip
// a fixture would be rewritten by every run. Skipped BY PATH, not by directory
// name — matching the name also swallowed core/tests/fixtures/ once.
const SEEDED_TREES = new Set([
  resolve(REPO_ROOT, "scripts", "fixtures"),
  // toc-red/ sits INSIDE the green tree, so without its own entry the green run
  // would walk it and inherit its two malformed markers — exit 1 where the whole
  // point of that root is exit 0. Same reason index-rows-red/ names itself.
  resolve(REPO_ROOT, "scripts", "fixtures", "toc-red"),
]);

/** Whether an absolute path sits inside a seeded tree that is not the scan root. */
function inSeededTree(abs) {
  for (const tree of SEEDED_TREES) {
    if (REPO === tree || REPO.startsWith(tree + sep)) continue; // the root is that tree
    if (abs === tree || abs.startsWith(tree + sep)) return true;
  }
  return false;
}

/**
 * Every `.md` file the REPOSITORY holds under the root, as relative paths.
 *
 * Enumerated from git for the reason check-doc-links.mjs is: a filesystem walk
 * cannot tell documents we own from a gitignored vendored README sitting in the
 * working tree, which is present locally and absent from CI's fresh clone.
 * Falls back to the walk when git cannot answer, and says so (ADR-0016).
 */
function markdownFiles() {
  try {
    const out = execFileSync("git", ["ls-files", "-z"], {
      cwd: REPO,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "ignore"],
      maxBuffer: 64 * 1024 * 1024,
    });
    const found = [];
    for (const path of out.split("\0")) {
      if (!path || !path.endsWith(".md")) continue;
      if (inSeededTree(resolve(REPO, path))) continue;
      found.push(path);
    }
    return { files: found, source: "git" };
  } catch {
    return { files: walk(), source: "filesystem" };
  }
}

/** The pre-git enumeration, kept as the fallback for a tree git cannot answer for. */
function walk(dir = REPO, found = []) {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const full = join(dir, entry.name);
    if (entry.isDirectory()) {
      if (SKIP_DIRS.has(entry.name)) continue;
      if (SEEDED_TREES.has(full)) continue;
      walk(full, found);
    } else if (entry.name.endsWith(".md")) {
      found.push(relative(REPO, full));
    }
  }
  return found;
}

const BEGIN = /^<!--\s*toc:begin\s+depth=(\d+)\s*-->\s*$/;
const END = /^<!--\s*toc:end\s*-->\s*$/;
const HEADING = /^(#{2,6})\s+(.*?)\s*$/;
const FENCE = /^\s*(```|~~~)/;

/**
 * `[text](target)` and `![alt](src)` keep their text and lose their target.
 *
 * The label accepts ONE level of balanced brackets, which CommonMark permits
 * inside link text and which this corpus contains: a close write-up whose
 * title cites another plan is a link whose label carries a shortcut reference,
 * `[0049 — ... making [0048] Phase 6 ...](done/0049-....md)`. A flat `[^\]]*`
 * label cannot cross that inner `]`, so the OUTER link never matches at all
 * and its target path stays in the text to fold into the slug. The inner
 * `[0048]` is deliberately NOT flattened: it is a shortcut reference, and the
 * character filter deletes its brackets where they sit, which is the anchor
 * GitHub computes for it.
 */
const flattenLinks = (s) =>
  s
    .replace(/!?\[((?:[^[\]]|\[[^[\]]*\])*)\]\([^)]*\)/g, "$1")
    .replace(/!?\[((?:[^[\]]|\[[^[\]]*\])*)\]\[[^\]]*\]/g, "$1");

/**
 * GitHub's heading anchor, pinned by the two in-repo anchors in this file's
 * header. Everything that is not a letter, a digit, `_` or `-` is DELETED
 * rather than replaced, which is why an em-dash between two spaces yields two
 * hyphens and a backtick against a word yields none.
 */
function anchor(headingText) {
  return flattenLinks(headingText)
    .trim()
    .toLowerCase()
    .replace(/[^\p{L}\p{N}_ -]/gu, "")
    .replace(/ /g, "-");
}

/** The row label: the heading as written, minus link syntax that cannot nest. */
const label = (headingText) => flattenLinks(headingText).trim();

/**
 * Which lines sit inside a fenced code block. Computed for the whole file in one
 * pass so that both marker detection and heading collection see the same answer
 * — a document describing this syntax is not carrying a block, and a `#` in a
 * shell sample is not a heading.
 */
function fencedLines(lines) {
  const fenced = new Array(lines.length).fill(false);
  let inFence = false;
  lines.forEach((line, i) => {
    if (FENCE.test(line)) {
      fenced[i] = true; // the fence line itself is never a heading or a marker
      inFence = !inFence;
      return;
    }
    fenced[i] = inFence;
  });
  return fenced;
}

/**
 * The blocks in one file, plus any unpaired marker.
 * Returns { blocks: [{ begin, end, depth }], errors: [{ line, reason }] } with
 * 0-based line indices.
 */
function parseBlocks(lines, fenced) {
  const blocks = [];
  const errors = [];
  let i = 0;
  while (i < lines.length) {
    if (fenced[i]) {
      i += 1;
      continue;
    }
    const open = lines[i].match(BEGIN);
    if (open) {
      let close = -1;
      for (let j = i + 1; j < lines.length; j += 1) {
        if (!fenced[j] && END.test(lines[j])) {
          close = j;
          break;
        }
        if (!fenced[j] && BEGIN.test(lines[j])) break; // a second open before any close
      }
      if (close === -1) {
        errors.push({ line: i + 1, reason: "toc:begin with no toc:end after it" });
        i += 1;
        continue;
      }
      blocks.push({ begin: i, end: close, depth: Number(open[1]) });
      i = close + 1;
      continue;
    }
    if (END.test(lines[i])) {
      errors.push({ line: i + 1, reason: "toc:end with no toc:begin before it" });
    }
    i += 1;
  }
  return { blocks, errors };
}

/**
 * The rows one block should hold: every heading after it, at levels 2..depth,
 * stopping at the next block's opening marker.
 */
function rowsFor(lines, fenced, block, nextBegin) {
  const stop = nextBegin ?? lines.length;
  const found = [];
  for (let i = block.end + 1; i < stop; i += 1) {
    if (fenced[i]) continue;
    const m = lines[i].match(HEADING);
    if (!m) continue;
    const level = m[1].length;
    if (level < 2 || level > block.depth) continue;
    found.push({ level, text: m[2] });
  }
  if (found.length === 0) return [];

  // Indent relative to the shallowest level actually present, so a document
  // whose sections start at `###` is not uniformly indented by one step.
  const base = Math.min(...found.map((h) => h.level));
  const seen = new Map();
  return found.map((h) => {
    const slug = anchor(h.text);
    const n = seen.get(slug) ?? 0;
    seen.set(slug, n + 1);
    const unique = n === 0 ? slug : `${slug}-${n}`;
    const indent = "  ".repeat(h.level - base);
    return `${indent}- [${label(h.text)}](#${unique})`;
  });
}

/**
 * One file's blocks, regenerated. Returns the new text and a per-block verdict.
 * Never writes; the caller decides, which is what makes --check and the rewrite
 * the same code path rather than two that can disagree.
 */
function regenerate(text) {
  const eol = text.includes("\r\n") ? "\r\n" : "\n";
  const lines = text.split(/\r?\n/);
  const fenced = fencedLines(lines);
  const { blocks, errors } = parseBlocks(lines, fenced);
  if (blocks.length === 0) return { text, blocks: [], errors, stale: [] };

  const stale = [];
  const out = [];
  let cursor = 0;
  blocks.forEach((block, index) => {
    const rows = rowsFor(lines, fenced, block, blocks[index + 1]?.begin);
    const current = lines.slice(block.begin + 1, block.end);
    const same = current.length === rows.length && current.every((l, i) => l === rows[i]);
    if (!same) {
      // Name the FIRST differing row, not just the counts. The common drift is a
      // reworded heading, where both counts are equal and a count-only message
      // says nothing about what moved.
      const at = rows.findIndex((r, i) => current[i] !== r);
      stale.push({
        line: block.begin + 1,
        was: current.length,
        now: rows.length,
        first: at === -1 ? null : rows[at].trim(),
      });
    }
    out.push(...lines.slice(cursor, block.begin + 1), ...rows);
    cursor = block.end;
  });
  out.push(...lines.slice(cursor));

  return { text: out.join(eol), blocks, errors, stale };
}

// ---------------------------------------------------------------------------
// self-test
// ---------------------------------------------------------------------------

/**
 * Exit 0 is not on its own an assertion — a generator that collected no
 * headings would leave every block empty, and on a tree whose blocks were also
 * empty that reads exactly like agreement. So the counts and the two pinned
 * anchors are asserted here rather than printed, on the model
 * check-index-rows.mjs --self-test established.
 *
 * Three halves, and they die to different mutations: the REPOSITORY pins the
 * two committed anchors, the FIXTURE pins the shapes this corpus contains, and
 * the STRUCTURAL cases pin what the parser must refuse to do.
 */
/** The rows a regenerated document actually holds, in order. */
function generatedRows(text) {
  const lines = text.split(/\r?\n/);
  const fenced = fencedLines(lines);
  const { blocks } = parseBlocks(lines, fenced);
  return blocks.flatMap((b) => lines.slice(b.begin + 1, b.end));
}

function selfTest() {
  const results = [];
  const check = (name, actual, expected) => {
    const ok = actual === expected;
    results.push({ ok, name, actual, expected });
  };

  // -- the repository: the two anchors this repo already links -------------
  check(
    "anchor: `--render`: a music video from a track",
    anchor("`--render`: a music video from a track"),
    "--render-a-music-video-from-a-track",
  );
  check(
    "anchor: Seeded randomness — `hash`, `noise`, and `[generator] seed`",
    anchor("Seeded randomness — `hash`, `noise`, and `[generator] seed`"),
    "seeded-randomness--hash-noise-and-generator-seed",
  );
  // Both are reachable only because those headings are still spelled that way.
  // Assert the headings themselves, or the two above become a test of a string
  // literal against another string literal.
  const capturing = readFileSync(join(REPO_ROOT, "docs", "capturing.md"), "utf8");
  const presets = readFileSync(join(REPO_ROOT, "presets", "README.md"), "utf8");
  check(
    "the pinned heading is still in docs/capturing.md",
    capturing.includes("\n### `--render`: a music video from a track\n"),
    true,
  );
  check(
    "the pinned heading is still in presets/README.md",
    presets.includes("\n### Seeded randomness — `hash`, `noise`, and `[generator] seed`\n"),
    true,
  );
  check(
    "docs/capturing.md still links the first anchor",
    capturing.includes("#--render-a-music-video-from-a-track"),
    true,
  );
  check(
    "presets/README.md still links the second anchor",
    presets.includes("#seeded-randomness--hash-noise-and-generator-seed"),
    true,
  );

  // -- the shapes this corpus contains ------------------------------------
  check("snake_case survives (GitHub keeps `_`)", anchor("reaction_diffusion glows"), "reaction_diffusion-glows");
  check("emphasis markers are punctuation", anchor("Asserting that something *moved*"), "asserting-that-something-moved");
  check("strikethrough is punctuation", anchor("~~0007 — a hollow ring~~"), "0007--a-hollow-ring");
  check("a percent sign doubles the hyphen", anchor("92 % of the suite"), "92--of-the-suite");
  check("a slash doubles the hyphen", anchor("DX12 / Vulkan"), "dx12--vulkan");
  check("a link keeps its text and loses its target", anchor("[0150 — Ritmolux](done/0150-x.md)"), "0150--ritmolux");
  // A close write-up whose title cites another plan: the whole heading is a
  // link and its label carries a shortcut reference. Asserted with the real
  // archive title, because the flat label matcher failed on THIS string and
  // folded `done/0049-analysis-diagnostics-surface.md` into the slug.
  check(
    "a bracketed reference inside a link label does not fold the target into the slug",
    anchor(
      "[0049 — the analysis diagnostics surface: making [0048] Phase 6 measurable (and the kaleidoscope seam)](done/0049-analysis-diagnostics-surface.md)",
    ),
    "0049--the-analysis-diagnostics-surface-making-0048-phase-6-measurable-and-the-kaleidoscope-seam",
  );
  check(
    "...and the row label is the title alone, inner reference intact",
    label(
      "[0049 — the analysis diagnostics surface: making [0048] Phase 6 measurable (and the kaleidoscope seam)](done/0049-analysis-diagnostics-surface.md)",
    ),
    "0049 — the analysis diagnostics surface: making [0048] Phase 6 measurable (and the kaleidoscope seam)",
  );

  // -- the fixture: generated blocks must equal the committed ones ---------
  const fixture = join(REPO_ROOT, "scripts", "fixtures", "toc");
  const seeded = readFileSync(join(fixture, "seeded.md"), "utf8");
  const seededOut = regenerate(seeded);
  check("fixture seeded.md: one block", seededOut.blocks.length, 1);
  check("fixture seeded.md: no malformed markers", seededOut.errors.length, 0);
  check("fixture seeded.md: the committed block is current", seededOut.stale.length, 0);
  // Read the GENERATED rows, not the committed ones. Asserting against the file
  // on disk would pass under a generator that had stopped collecting headings
  // entirely, which is the mutation these counts exist to catch.
  const seededRows = generatedRows(seededOut.text);
  check("fixture seeded.md: row count", seededRows.length, 13);
  check("fixture seeded.md: level-2 rows are flush", seededRows.filter((r) => r.startsWith("- ")).length, 5);
  check("fixture seeded.md: level-3 rows indent one step", seededRows.filter((r) => r.startsWith("  - ")).length, 8);
  check(
    "fixture seeded.md: the repeated heading dedupes as -1",
    seededRows.filter((r) => r.endsWith("(#a-repeated-heading-1)")).length,
    1,
  );
  check(
    "fixture seeded.md: the first occurrence keeps the bare anchor",
    seededRows.filter((r) => r.endsWith("(#a-repeated-heading)")).length,
    1,
  );
  check(
    "fixture seeded.md: a bracketed reference survives in the row and vanishes from the anchor",
    seededRows.some(
      (r) => r === "  - [A heading that is a link, citing [0048]](#a-heading-that-is-a-link-citing-0048)",
    ),
    true,
  );
  check("fixture seeded.md: the level-4 heading is not a row", seededRows.some((r) => r.includes("level 4")), false);

  const empty = readFileSync(join(fixture, "empty-block.md"), "utf8");
  const emptyOut = regenerate(empty);
  check("fixture empty-block.md: one block", emptyOut.blocks.length, 1);
  check("fixture empty-block.md: emits an empty block, not an error", emptyOut.stale.length, 0);
  check("fixture empty-block.md: text is unchanged", emptyOut.text, empty);

  const bare = readFileSync(join(fixture, "no-markers.md"), "utf8");
  const bareOut = regenerate(bare);
  check("fixture no-markers.md: no block", bareOut.blocks.length, 0);
  check("fixture no-markers.md: text is unchanged", bareOut.text, bare);

  const unpaired = readFileSync(join(REPO_ROOT, "scripts", "fixtures", "toc-red", "unpaired.md"), "utf8");
  const unpairedOut = regenerate(unpaired);
  check("fixture unpaired.md: reported, not rewritten", unpairedOut.errors.length, 2);
  check("fixture unpaired.md: text is unchanged", unpairedOut.text, unpaired);

  const fenced = readFileSync(join(fixture, "fenced.md"), "utf8");
  const fencedOut = regenerate(fenced);
  check("fixture fenced.md: a marker inside a fence is not a block", fencedOut.blocks.length, 1);
  check("fixture fenced.md: a heading inside a fence is not a row", fencedOut.stale.length, 0);

  const passed = results.filter((r) => r.ok).length;
  for (const r of results) {
    if (r.ok) continue;
    console.error(`  FAIL  ${r.name}\n        expected ${JSON.stringify(r.expected)}\n        actual   ${JSON.stringify(r.actual)}`);
  }
  console.log(`toc self-test: ${passed} of ${results.length}`);
  process.exit(passed === results.length ? 0 : 1);
}

if (SELF_TEST) selfTest();

// ---------------------------------------------------------------------------
// the tree
// ---------------------------------------------------------------------------

const { files, source: enumeration } = markdownFiles();
const problems = [];
let blockCount = 0;
let rowCount = 0;
const rewritten = [];

for (const file of files) {
  const abs = join(REPO, file);
  const text = readFileSync(abs, "utf8");
  if (!text.includes("toc:begin") && !text.includes("toc:end")) continue;

  const result = regenerate(text);
  const show = file.split(sep).join("/");

  for (const e of result.errors) {
    problems.push(`${show}:${e.line}  ${e.reason}`);
  }
  blockCount += result.blocks.length;
  // Counted from inside the blocks, not by matching row-shaped lines across the
  // file: a manual's body is full of bullets that begin with a link, and folding
  // those into the total would make the summary read high and unfalsifiable.
  rowCount += generatedRows(result.text).length;

  if (result.text === text) continue;
  if (CHECK) {
    for (const s of result.stale) {
      const detail =
        s.was === s.now
          ? `first differing row: ${s.first}`
          : `${s.was} rows, ${s.now} expected`;
      problems.push(`${show}:${s.line}  contents block is stale (${detail})`);
    }
  } else {
    writeFileSync(abs, result.text);
    rewritten.push(show);
  }
}

if (enumeration !== "git") {
  console.log(
    "note: git could not list this tree, so the file set came from a filesystem\n" +
      "      walk. That set includes anything gitignored sitting in the working\n" +
      "      tree, whose contents blocks are not ours to regenerate.",
  );
}

if (problems.length > 0) {
  console.error(`contents blocks: ${problems.length} problem${problems.length === 1 ? "" : "s"}`);
  for (const p of problems) console.error(`  ${p}`);
  console.error(
    "\nRun `node scripts/toc.mjs` to regenerate every block from its headings.\n" +
      "A block is never hand-edited: its rows come from the headings that follow\n" +
      "it, at levels 2 through the marker's own `depth=N` (ADR-0163).\n" +
      "\n" +
      "`toc:begin with no toc:end after it` is a typo, not drift. The file is left\n" +
      "alone rather than rewritten, because treating the rest of a document as\n" +
      "block body would delete it.",
  );
  process.exit(1);
}

if (CHECK) {
  console.log(`contents blocks: OK (${blockCount} block${blockCount === 1 ? "" : "s"}, ${rowCount} rows, current)`);
} else if (rewritten.length === 0) {
  console.log(`contents blocks: ${blockCount} block${blockCount === 1 ? "" : "s"} already current, nothing rewritten`);
} else {
  console.log(`contents blocks: rewrote ${rewritten.length} file${rewritten.length === 1 ? "" : "s"}`);
  for (const f of rewritten) console.log(`  ${f}`);
}
process.exit(0);
