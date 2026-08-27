#!/usr/bin/env node
// Reject the two mechanical rot classes in Rust comments: a relative link, and
// plan-relative narration.
//
// Rationale: a relative link in a `.rs` comment fails silently in three ways at
// once. It breaks when a plan moves to docs/plans/done/, which is a routine step
// of every close; check-doc-links.mjs cannot see it, because that script walks
// `.md` only; and it does not resolve in rendered rustdoc at all, where the href
// is emitted as written and resolves against the generated HTML tree. Eleven were
// broken on main when ADR-0127 was written. Plan-relative narration — `this
// plan`, `used to`, `no longer` — is written from inside a session and stops
// being legible at its close: there is no "this plan" any more, there is only the
// code. See ADR-0127 for the decision and its four rejected alternatives.
//
// Usage:  node scripts/check-comment-hygiene.mjs [root]
// Exit 0 = no finding. Exit 1 = they are listed as `file:line -> reason`, which
// is clickable in most terminals. The optional `root` scans some other directory
// — used to run this against the committed fixture tree, following
// check-doc-links.mjs: `node scripts/check-comment-hygiene.mjs scripts/fixtures`
// expects exit 1 and exactly two findings, one per class. CI and the pre-push
// hook pass nothing and get the repo.
//
// What is NOT a finding, and each silence is load-bearing:
//
//   - a rustdoc INTRA-DOC link, `[Self::render]` or `[x](crate::render)`. rustc
//     resolves those, so they cannot rot silently, and they are the linking
//     mechanism ADR-0127 deliberately keeps. Only a target starting `./` or `../`
//     is reported.
//   - a bare-number citation, `Plan 0045 Phase 3`. That is the form ADR-0127
//     replaces the links WITH, so a gate that fired on the word "plan" would
//     convict the fix.
//   - anything outside a comment. A `//` inside a string literal is not a
//     comment, which is why this reads Rust rather than grepping lines.
//
// COMMENT LENGTH IS NOT CHECKED, by any threshold, in any file. ADR-0127
// Alternative B: length correlates weakly with the defect, fires on headers that
// are mostly genuine mechanism, and is gamed by inserting a blank line — which
// teaches authors to fragment prose rather than cut it. That judgement belongs to
// the architect's Mode 4 review.
//
// The escape hatch: `hygiene-allow: <reason>` in a comment suppresses findings on
// its own line and on the line after it. The reason is required — an escape
// without one is itself reported, because an unexplained escape is how a gate
// stops meaning anything. The vocabulary list has real false positives ("the
// value used to compute the knee"), and the escape exists for the residue after
// rewriting, not instead of rewriting: if the count printed on success is more
// than a handful, the word list is wrong and should be narrowed.

import { readdirSync, readFileSync } from "node:fs";
import { join, dirname, resolve, relative, sep } from "node:path";
import { fileURLToPath } from "node:url";

const REPO_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const REPO = resolve(process.argv[2] ?? REPO_ROOT);

// Build output, vendored deps, and VCS internals hold Rust we do not own.
const SKIP_DIRS = new Set(["target", "node_modules", ".git"]);

// The fixture tree carries this checker's own bite check and is skipped on a
// repo walk, exactly as check-doc-links.mjs skips it; it is scanned when it IS
// the root, which is the only way its seeded findings are reachable. Skipped BY
// PATH, not by directory name — the name form also swallowed
// core/tests/fixtures/ there.
const SEEDED_TREES = new Set([resolve(REPO_ROOT, "scripts", "fixtures")]);

/** Every `.rs` file under the root, as paths relative to it. */
function rustFiles(dir = REPO, found = []) {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const full = join(dir, entry.name);
    if (entry.isDirectory()) {
      if (SKIP_DIRS.has(entry.name)) continue;
      if (SEEDED_TREES.has(full)) continue;
      rustFiles(full, found);
    } else if (entry.name.endsWith(".rs")) {
      found.push(relative(REPO, full));
    }
  }
  return found;
}

/**
 * The comment text of each line, indexed by 0-based line number.
 *
 * Rust is lexed rather than grepped because `"https://x"` holds a `//` that is
 * not a comment and `// a "quote` holds a `"` that opens nothing. Block comments
 * nest in Rust, hence the depth counter; a raw string's terminator is its own
 * hash count, hence the hash group; and `'` is a lifetime far more often than a
 * char literal, so it only opens one when a closing `'` sits where a char
 * literal's would.
 *
 * The scanner emits character SPANS and the line number is derived afterwards
 * from an index. Counting newlines as the scanner walks is the same computation
 * in principle and wrong in practice: every branch that skips a region has to
 * remember to count what it skipped, and one that forgets — `\` before a newline
 * inside a string, consumed as an escape pair — misreports every line below it
 * in the file.
 */
function commentSpans(source) {
  const spans = [];
  let i = 0;
  const n = source.length;

  while (i < n) {
    const c = source[i];

    // Line comment: everything to the newline.
    if (c === "/" && source[i + 1] === "/") {
      let j = i;
      while (j < n && source[j] !== "\n") j++;
      spans.push([i, j]);
      i = j;
      continue;
    }

    // Block comment, nesting, possibly multi-line.
    if (c === "/" && source[i + 1] === "*") {
      let depth = 1;
      let j = i + 2;
      const start = j;
      while (j < n && depth > 0) {
        if (source[j] === "/" && source[j + 1] === "*") {
          depth++;
          j += 2;
        } else if (source[j] === "*" && source[j + 1] === "/") {
          depth--;
          j += 2;
        } else {
          j++;
        }
      }
      spans.push([start, Math.max(start, j - 2)]);
      i = j;
      continue;
    }

    // Raw string: r"..." / r#"..."# / br#"..."#.
    const raw = /^b?r(#*)"/.exec(source.slice(i, i + 40));
    if (raw && (i === 0 || !/[A-Za-z0-9_]/.test(source[i - 1]))) {
      const terminator = '"' + raw[1];
      let j = i + raw[0].length;
      while (j < n && source.slice(j, j + terminator.length) !== terminator) j++;
      i = j + terminator.length;
      continue;
    }

    // Ordinary string, with backslash escapes.
    if (c === '"') {
      let j = i + 1;
      while (j < n) {
        if (source[j] === "\\") {
          j += 2;
          continue;
        }
        if (source[j] === '"') break;
        j++;
      }
      i = j + 1;
      continue;
    }

    // A char literal, or a lifetime. `'\n'` and `'a'` close; `'a` does not.
    if (c === "'") {
      const escaped = source[i + 1] === "\\";
      const close = escaped ? source.indexOf("'", i + 2) : i + 2;
      if (!escaped && source[close] === "'") {
        i = close + 1;
        continue;
      }
      if (escaped && close !== -1 && close - i <= 8) {
        i = close + 1;
        continue;
      }
      i++; // a lifetime
      continue;
    }

    i++;
  }

  return spans;
}

/** The comment text of each line, indexed by 0-based line number. */
function commentLines(source) {
  const out = new Map();
  const add = (line, text) => out.set(line, (out.get(line) ?? "") + text);

  // Line starts, so an index maps to a line by binary search.
  const starts = [0];
  for (let i = 0; i < source.length; i++) {
    if (source[i] === "\n") starts.push(i + 1);
  }
  const lineOf = (idx) => {
    let lo = 0;
    let hi = starts.length - 1;
    while (lo < hi) {
      const mid = (lo + hi + 1) >> 1;
      if (starts[mid] <= idx) lo = mid;
      else hi = mid - 1;
    }
    return lo;
  };

  for (const [start, end] of commentSpans(source)) {
    let line = lineOf(start);
    let from = start;
    while (from < end) {
      const lineEnd = line + 1 < starts.length ? starts[line + 1] - 1 : source.length;
      add(line, source.slice(from, Math.min(end, lineEnd)));
      from = lineEnd + 1;
      line++;
    }
    if (start === end) add(lineOf(start), "");
  }

  return out;
}

// A relative link target, in markdown's two forms. Only `./` and `../` — an
// intra-doc link resolves through rustc and is left alone.
const DEFINITION = /\[[^\]]*\]:\s*(\.\.?\/[^\s)]+)/g;
const INLINE = /\]\((\.\.?\/[^)\s]+)\)/g;

// Written from inside a plan session, and undecodable once that session closes.
const VOCABULARY = /\b(this plan|the plan|used to|no longer|is new|previously)\b/gi;

// `hygiene-allow: <reason>` — the reason is what makes it an escape rather than
// a silencer, so it is required.
const ESCAPE = /hygiene-allow:\s*(\S.*)?$/;

const findings = [];
let escapes = 0;

for (const file of rustFiles()) {
  const show = file.split(sep).join("/");
  const comments = commentLines(readFileSync(join(REPO, file), "utf8"));

  // An escape covers its own line and the one after it, so a marker can sit
  // above the sentence it is excusing rather than inside it.
  const allowed = new Set();
  for (const [line, text] of comments) {
    const escape = ESCAPE.exec(text);
    if (!escape) continue;
    if (!escape[1]) {
      findings.push(`${show}:${line + 1} -> hygiene-allow with no reason given`);
      continue;
    }
    escapes++;
    allowed.add(line);
    allowed.add(line + 1);
  }

  for (const [line, text] of [...comments].sort((a, b) => a[0] - b[0])) {
    if (allowed.has(line)) continue;

    for (const re of [DEFINITION, INLINE]) {
      re.lastIndex = 0;
      for (const m of text.matchAll(re)) {
        findings.push(`${show}:${line + 1} -> relative link \`${m[1]}\` (cite the bare number)`);
      }
    }

    VOCABULARY.lastIndex = 0;
    for (const m of text.matchAll(VOCABULARY)) {
      findings.push(`${show}:${line + 1} -> plan-relative narration \`${m[1]}\``);
    }
  }
}

if (findings.length === 0) {
  console.log(`comment hygiene: OK (no relative links, no plan-relative narration; ${escapes} escapes in use)`);
  process.exit(0);
}

console.error(`comment hygiene: ${findings.length} findings`);
for (const f of findings) console.error(`  ${f}`);
console.error(
  "\nA comment carries the mechanism, the invariant, the trap, and any formula or\n" +
    "constant a reader cannot re-derive. Why an approach beat the alternative, what\n" +
    "was measured, and what the code did before belong in docs/ and are cited by\n" +
    "BARE NUMBER — `ADR-0046`, `Plan 0045 Phase 3`. `grep -rn 0046 docs/adrs`\n" +
    "resolves that in one command and there is no path left to rot.\n" +
    "\n" +
    "Narration is restated as a property of the code: `used to be free-running\n" +
    "until Plan 0095` becomes `the phase is locked, not free-running` — same fact,\n" +
    "no expiry. Where a sentence is a genuine false positive, `hygiene-allow: <why>`\n" +
    "in a comment suppresses its own line and the next one. See ADR-0127.",
);
process.exit(1);
